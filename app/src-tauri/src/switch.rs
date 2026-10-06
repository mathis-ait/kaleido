//! Jeux Nintendo Switch de la bibliothèque (`.xci`, `.nsp`, `.xcz`, `.nsz`).
//!
//! Le title ID (16 chiffres hexadécimaux, `0100…`) est cherché, du plus simple au
//! plus coûteux :
//! 1. dans le nom du fichier (`Jeu [01001F5010DFA000][v0].nsp`) ;
//! 2. NSP : dans le nom du ticket (`<rights ID>.tik`, le title ID en est la première
//!    moitié) ou dans un `.cnmt.xml` ;
//! 3. dans l'en-tête d'un NCA (NSP, ou partition `secure` d'un XCI). Ses 0xC00
//!    premiers octets sont chiffrés en AES-128-XTS (secteurs de 0x200 octets, tweak
//!    = numéro de secteur en gros-boutiste) avec la clé `header_key` de `prod.keys`
//!    (celle d'Eden : `%APPDATA%\eden\keys`). Champs utiles : `NCA3` en 0x200, type
//!    de contenu en 0x205, title ID (petit-boutiste) en 0x210.
//!
//! Formats : PFS0 (NSP) = magic, nombre d'entrées, taille de la table des noms, puis
//! entrées de 0x18 octets (offset, taille, offset du nom). XCI : en-tête `HEAD` en
//! 0x100 (0x1100 pour les dumps avec zone de clés), adresse de la partition HFS0
//! racine en +0x130 ; HFS0 = comme PFS0 mais entrées de 0x40 octets.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const EXTENSIONS: &[&str] = &["xci", "nsp", "xcz", "nsz"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchGame {
    pub path: String,
    pub file_name: String,
    /// Title ID du jeu de base, 16 chiffres hexadécimaux majuscules.
    pub title_id: String,
    pub title: String,
    pub size: u64,
    /// Une mise à jour du jeu a aussi été trouvée.
    pub has_update: bool,
}

/// Nature d'un title ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleKind {
    Base,
    Update,
    Dlc,
}

pub fn title_kind(tid: u64) -> TitleKind {
    match tid & 0xFFF {
        0 => TitleKind::Base,
        0x800 => TitleKind::Update,
        _ => TitleKind::Dlc,
    }
}

/// Title ID du jeu de base (mise à jour : `…800` → `…000` ; DLC : `base + 0x1000 + n`).
pub fn base_title_id(tid: u64) -> u64 {
    match title_kind(tid) {
        TitleKind::Base => tid,
        TitleKind::Update => tid & !0xFFF,
        TitleKind::Dlc => (tid - 0x1000) & !0xFFF,
    }
}

fn parse_hex16(s: &str) -> Option<u64> {
    (s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| u64::from_str_radix(s, 16).ok()).flatten()
}

/// Title ID écrit entre crochets dans un nom de fichier (`[0100ABF008968000]`).
pub fn title_id_in_name(name: &str) -> Option<u64> {
    name.split(['[', ']', '(', ')', ' ', '_', '-', '.'])
        .filter_map(parse_hex16)
        .find(|t| t >> 56 == 0x01)
}

// ---------------------------------------------------------------------------
// Conteneurs

struct Entry {
    name: String,
    offset: u64,
    size: u64,
}

fn read_at(f: &mut File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// Liste d'un PFS0 (`entry_size` 0x18) ou HFS0 (0x40) situé à `base`. Les offsets rendus sont absolus.
fn read_partition(f: &mut File, base: u64, magic: &[u8; 4], entry_size: usize) -> Option<Vec<Entry>> {
    let head = read_at(f, base, 0x10).ok()?;
    if &head[..4] != magic {
        return None;
    }
    let count = u32le(&head, 4) as usize;
    let strtab = u32le(&head, 8) as usize;
    if count == 0 || count > 10_000 || strtab > 1 << 20 {
        return None;
    }
    let table = read_at(f, base + 0x10, count * entry_size + strtab).ok()?;
    let names = &table[count * entry_size..];
    let data = base + 0x10 + table.len() as u64;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let e = &table[i * entry_size..];
        let name_off = u32le(e, 0x10) as usize;
        let end = names.get(name_off..)?.iter().position(|&b| b == 0).map_or(names.len(), |p| name_off + p);
        out.push(Entry { name: String::from_utf8_lossy(&names[name_off..end]).into_owned(), offset: data + u64le(e, 0), size: u64le(e, 8) });
    }
    Some(out)
}

/// NCA d'un XCI : partition `secure` de la partition racine.
fn xci_ncas(f: &mut File) -> Option<Vec<Entry>> {
    for head in [0x100u64, 0x1100] {
        let Ok(h) = read_at(f, head, 0x40) else { continue };
        if &h[..4] != b"HEAD" {
            continue;
        }
        let root = u64le(&h, 0x30);
        let parts = read_partition(f, head - 0x100 + root, b"HFS0", 0x40)?;
        let secure = parts.iter().find(|p| p.name == "secure")?;
        return read_partition(f, secure.offset, b"HFS0", 0x40);
    }
    None
}

// ---------------------------------------------------------------------------
// En-tête NCA

/// Clé `header_key` (32 octets) d'un fichier `prod.keys`.
pub fn header_key_from(text: &str) -> Option<[u8; 32]> {
    let line = text.lines().find(|l| l.split('=').next().is_some_and(|k| k.trim().eq_ignore_ascii_case("header_key")))?;
    let hex = line.split('=').nth(1)?.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, k) in key.iter_mut().enumerate() {
        *k = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(key)
}

/// Multiplication par α dans GF(2¹²⁸) (convention XTS, petit-boutiste).
fn xts_next(t: &mut [u8; 16]) {
    let mut carry = 0u8;
    for b in t.iter_mut() {
        let next = *b >> 7;
        *b = (*b << 1) | carry;
        carry = next;
    }
    if carry != 0 {
        t[0] ^= 0x87;
    }
}

/// Déchiffre `data` (multiple de 0x200) en AES-128-XTS « Nintendo » à partir du secteur `first`.
pub fn xts_decrypt(key: &[u8; 32], data: &mut [u8], first: u64) {
    let k1 = Aes128::new(GenericArray::from_slice(&key[..16]));
    let k2 = Aes128::new(GenericArray::from_slice(&key[16..]));
    for (i, sector) in data.chunks_mut(0x200).enumerate() {
        let mut tweak = [0u8; 16];
        tweak[8..].copy_from_slice(&(first + i as u64).to_be_bytes());
        let mut t = GenericArray::clone_from_slice(&tweak);
        k2.encrypt_block(&mut t);
        let mut t: [u8; 16] = t.into();
        for block in sector.chunks_mut(16) {
            let mut b = GenericArray::default();
            for j in 0..16 {
                b[j] = block[j] ^ t[j];
            }
            k1.decrypt_block(&mut b);
            for j in 0..16 {
                block[j] = b[j] ^ t[j];
            }
            xts_next(&mut t);
        }
    }
}

/// Title ID et type de contenu d'un NCA (`None` si la clé ne convient pas).
fn nca_title(f: &mut File, offset: u64, key: &[u8; 32]) -> Option<(u64, u8)> {
    let mut header = read_at(f, offset, 0x400).ok()?;
    xts_decrypt(key, &mut header, 0);
    if !matches!(&header[0x200..0x204], b"NCA3" | b"NCA2") {
        return None;
    }
    Some((u64le(&header, 0x210), header[0x205]))
}

fn title_from_ncas(f: &mut File, entries: &[Entry], key: Option<&[u8; 32]>) -> Option<u64> {
    let key = key?;
    // Métadonnées (1) d'abord : dans une mise à jour, le programme porte le title ID du jeu
    // de base, seules les métadonnées portent celui de la mise à jour (…800).
    let mut found = Vec::new();
    for e in entries.iter().filter(|e| e.name.ends_with(".nca") || e.name.ends_with(".ncz")).take(16) {
        if let Some(t) = nca_title(f, e.offset, key) {
            found.push(t);
        }
    }
    found.iter().find(|t| t.1 == 1).or_else(|| found.iter().find(|t| t.1 == 0)).or_else(|| found.first()).map(|t| t.0)
}

/// Title ID d'un fichier Switch.
pub fn title_id_of(path: &Path, key: Option<&[u8; 32]>) -> Option<u64> {
    if let Some(t) = path.file_name().and_then(|n| title_id_in_name(&n.to_string_lossy())) {
        return Some(t);
    }
    let mut f = File::open(path).ok()?;
    if let Some(entries) = read_partition(&mut f, 0, b"PFS0", 0x18) {
        if let Some(t) = entries.iter().filter_map(|e| e.name.strip_suffix(".tik")).filter_map(|r| r.get(..16)).find_map(parse_hex16) {
            return Some(t);
        }
        if let Some(xml) = entries.iter().find(|e| e.name.ends_with(".cnmt.xml") && e.size < 1 << 20) {
            let text = read_at(&mut f, xml.offset, xml.size as usize).ok()?;
            let text = String::from_utf8_lossy(&text);
            if let Some(t) = text.split("<Id>0x").nth(1).and_then(|s| s.get(..16)).and_then(parse_hex16) {
                return Some(t);
            }
        }
        return title_from_ncas(&mut f, &entries, key);
    }
    let entries = xci_ncas(&mut f)?;
    title_from_ncas(&mut f, &entries, key)
}

// ---------------------------------------------------------------------------
// Noms des jeux

/// Noms français des jeux Pokémon (les bases en ligne ne donnent que l'anglais).
pub fn pokemon_name(tid: u64) -> Option<&'static str> {
    Some(match tid {
        0x010003F003A34000 => "Pokémon : Let's Go, Pikachu !",
        0x0100187003A36000 => "Pokémon : Let's Go, Évoli !",
        0x0100ABF008968000 => "Pokémon Épée",
        0x01008DB008C2C000 => "Pokémon Bouclier",
        0x0100000011D90000 => "Pokémon Diamant Étincelant",
        0x010018E011D92000 => "Pokémon Perle Scintillante",
        0x01001F5010DFA000 => "Légendes Pokémon : Arceus",
        0x0100A3D008C5C000 => "Pokémon Écarlate",
        0x01008F6008C5E000 => "Pokémon Violet",
        0x0100F43008C44000 => "Légendes Pokémon : Z-A",
        0x01003D200BAA2000 => "Pokémon Donjon Mystère : Équipe de Secours DX",
        0x0100B3F000BE2000 => "Pokkén Tournament DX",
        0x01003E5017BF6000 => "Détective Pikachu : Le retour",
        0x0100D0101A548000 => "New Pokémon Snap",
        _ => return None,
    })
}

/// Nom tiré du fichier : `Pokemon Legends Arceus [01001F…][v0].nsp` → « Pokemon Legends Arceus ».
pub fn name_from_file(file: &str) -> String {
    let stem = Path::new(file).file_stem().map_or(file.to_string(), |s| s.to_string_lossy().into_owned());
    let mut out = String::new();
    let mut depth = 0;
    for c in stem.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(if c == '_' { ' ' } else { c }),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.is_empty() {
        stem
    } else {
        out
    }
}

const USER_AGENT: &str = concat!("Kaleido/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mathis-ait/kaleido)");

#[derive(Deserialize)]
struct NlibTitle {
    name: Option<String>,
}

/// Nom officiel (anglais) via api.nlib.cc, gardé en cache dans `switch-titles.json`.
fn online_name(cache: &mut HashMap<String, String>, tid: &str) -> Option<String> {
    if let Some(n) = cache.get(tid) {
        return (!n.is_empty()).then(|| n.clone());
    }
    let name = ureq::get(&format!("https://api.nlib.cc/nx/{tid}"))
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(8))
        .call()
        .ok()
        .and_then(|r| serde_json::from_reader::<_, NlibTitle>(r.into_reader()).ok())
        .and_then(|t| t.name)
        .map(|n| n.replace(['™', '®'], "").trim().to_string())
        .unwrap_or_default();
    cache.insert(tid.to_string(), name.clone());
    (!name.is_empty()).then_some(name)
}

// ---------------------------------------------------------------------------
// Recherche

/// Fichiers Switch des dossiers suivis (3 niveaux de sous-dossiers).
pub fn find_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                if depth > 0 {
                    walk(&p, depth - 1, out);
                }
            } else if is_switch_file(&p) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    if dir.is_file() {
        if is_switch_file(dir) {
            out.push(dir.to_path_buf());
        }
    } else {
        walk(dir, 3, &mut out);
    }
    out
}

pub fn is_switch_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Dossiers où chercher `prod.keys` (Eden, puis yuzu et forks, Ryujinx).
pub fn keys_candidates(appdata: Option<&Path>, eden_user: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = eden_user.map(|u| u.join("keys").join("prod.keys")).into_iter().collect();
    if let Some(a) = appdata {
        for d in ["eden", "citron", "sudachi", "torzu", "suyu", "yuzu"] {
            out.push(a.join(d).join("keys").join("prod.keys"));
        }
        out.push(a.join("Ryujinx").join("system").join("prod.keys"));
    }
    out
}

static NAMES: OnceLock<Mutex<Option<HashMap<String, String>>>> = OnceLock::new();

fn names_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_cache_dir().ok().map(|d| d.join("switch-titles.json"))
}

/// Jeux Switch (un par title ID de base) des dossiers et fichiers donnés.
pub fn scan(app: &AppHandle, roots: &[PathBuf], hidden: &[String]) -> Vec<SwitchGame> {
    let config = crate::play::load_config(app);
    let env = crate::play::Env::system(&config.search_dirs);
    let eden = crate::play::resolve(crate::play::EmulatorId::Eden, &config, &env);
    let key = keys_candidates(env.appdata.as_deref(), eden.ctr_user_dir().as_deref())
        .iter()
        .find_map(|p| fs::read_to_string(p).ok().and_then(|t| header_key_from(&t)));

    let mut files: Vec<PathBuf> = roots.iter().flat_map(|r| find_files(r)).collect();
    files.sort();
    files.dedup();
    files.retain(|p| !hidden.iter().any(|h| Path::new(h) == p.as_path()));

    let lock = NAMES.get_or_init(Default::default);
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(|| {
        names_path(app).and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
    });
    let before = cache.len();

    let mut games: Vec<SwitchGame> = Vec::new();
    let mut updates = Vec::new();
    for path in files {
        let Some(tid) = title_id_of(&path, key.as_ref()) else { continue };
        match title_kind(tid) {
            TitleKind::Update => updates.push(base_title_id(tid)),
            TitleKind::Dlc => {}
            TitleKind::Base => {
                let hex = format!("{tid:016X}");
                if games.iter().any(|g| g.title_id == hex) {
                    continue;
                }
                let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let title = pokemon_name(tid).map(str::to_string).or_else(|| online_name(cache, &hex)).unwrap_or_else(|| name_from_file(&file_name));
                games.push(SwitchGame {
                    size: fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                    path: path.display().to_string(),
                    file_name,
                    title_id: hex,
                    title,
                    has_update: false,
                });
            }
        }
    }
    for g in &mut games {
        g.has_update = updates.iter().any(|u| format!("{u:016X}") == g.title_id);
    }
    if cache.len() != before {
        if let (Some(p), Ok(json)) = (names_path(app), serde_json::to_vec(&*cache)) {
            let _ = p.parent().map(fs::create_dir_all);
            let _ = fs::write(p, json);
        }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn ids() {
        assert_eq!(title_id_in_name("Pokemon Legends Arceus [01001F5010DFA000][v0].nsp"), Some(0x01001F5010DFA000));
        assert_eq!(title_id_in_name("Pokemon Legends Arceus.xci"), None);
        assert_eq!(base_title_id(0x01001F5010DFA800), 0x01001F5010DFA000);
        assert_eq!(base_title_id(0x01001F5010DFB001), 0x01001F5010DFA000);
        assert_eq!(title_kind(0x01001F5010DFA800), TitleKind::Update);
        assert_eq!(name_from_file("Pokemon Legends Arceus Upd v1.1.1 [01001F5010DFA800][v196608].nsp"), "Pokemon Legends Arceus Upd v1.1.1");
    }

    #[test]
    fn keys_file() {
        let text = "aes_kek_generation_source = 4D870986C45D20722FBA1053DA92E8A9\nheader_key = 000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F\n";
        let k = header_key_from(text).unwrap();
        assert_eq!(k[0], 0) ;
        assert_eq!(k[31], 0x1F);
        assert!(header_key_from("header_key = 00").is_none());
    }

    /// Chiffre comme le ferait Nintendo puis vérifie qu'on relit bien l'en-tête.
    fn xts_encrypt(key: &[u8; 32], data: &mut [u8], first: u64) {
        let k1 = Aes128::new(GenericArray::from_slice(&key[..16]));
        let k2 = Aes128::new(GenericArray::from_slice(&key[16..]));
        for (i, sector) in data.chunks_mut(0x200).enumerate() {
            let mut tweak = [0u8; 16];
            tweak[8..].copy_from_slice(&(first + i as u64).to_be_bytes());
            let mut t = GenericArray::clone_from_slice(&tweak);
            k2.encrypt_block(&mut t);
            let mut t: [u8; 16] = t.into();
            for block in sector.chunks_mut(16) {
                let mut b = GenericArray::default();
                for j in 0..16 {
                    b[j] = block[j] ^ t[j];
                }
                k1.encrypt_block(&mut b);
                for j in 0..16 {
                    block[j] = b[j] ^ t[j];
                }
                xts_next(&mut t);
            }
        }
    }

    #[test]
    fn xts_vector() {
        // IEEE 1619 vecteur 1 (clés nulles, tweak nul) : un secteur de 32 octets nuls.
        let mut data = hex("917cf69ebd68b2ec9b9fe9a3eadda692cd43d2f59598ed858c02c2652fbf922e");
        xts_decrypt(&[0u8; 32], &mut data, 0);
        assert_eq!(data, vec![0u8; 32]);
    }

    /// Vrais fichiers : `KALEIDO_SWITCH_GAME=<xci|nsp> cargo test real_switch_game -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_switch_game() {
        let path = PathBuf::from(std::env::var("KALEIDO_SWITCH_GAME").unwrap());
        let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
        let key = keys_candidates(appdata.as_deref(), None).iter().find_map(|p| fs::read_to_string(p).ok().and_then(|t| header_key_from(&t)));
        // Sans passer par le nom du fichier.
        let mut f = File::open(&path).unwrap();
        let tid = match read_partition(&mut f, 0, b"PFS0", 0x18) {
            Some(e) => title_from_ncas(&mut f, &e, key.as_ref()),
            None => xci_ncas(&mut f).and_then(|e| title_from_ncas(&mut f, &e, key.as_ref())),
        };
        println!("{} → {:016X?} (clé trouvée : {})", path.display(), tid, key.is_some());
        assert!(tid.is_some());
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn pfs0(files: &[(&str, Vec<u8>)], magic: &[u8; 4], entry_size: usize) -> Vec<u8> {
        let mut names = Vec::new();
        let mut table = Vec::new();
        let mut offset = 0u64;
        for (name, data) in files {
            let mut e = vec![0u8; entry_size];
            e[0..8].copy_from_slice(&offset.to_le_bytes());
            e[8..16].copy_from_slice(&(data.len() as u64).to_le_bytes());
            e[16..20].copy_from_slice(&(names.len() as u32).to_le_bytes());
            table.extend(e);
            names.extend(name.as_bytes());
            names.push(0);
            offset += data.len() as u64;
        }
        while names.len() % 0x20 != 0 {
            names.push(0);
        }
        let mut out = magic.to_vec();
        out.extend((files.len() as u32).to_le_bytes());
        out.extend((names.len() as u32).to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(table);
        out.extend(names);
        for (_, data) in files {
            out.extend(data);
        }
        out
    }

    fn nca(key: &[u8; 32], tid: u64, content: u8) -> Vec<u8> {
        let mut h = vec![0u8; 0xC00];
        h[0x200..0x204].copy_from_slice(b"NCA3");
        h[0x205] = content;
        h[0x210..0x218].copy_from_slice(&tid.to_le_bytes());
        xts_encrypt(key, &mut h, 0);
        h
    }

    #[test]
    fn containers() {
        let dir = std::env::temp_dir().join(format!("kaleido-switch-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let key: [u8; 32] = std::array::from_fn(|i| i as u8 * 7 + 1);

        // NSP avec ticket.
        let nsp = dir.join("jeu.nsp");
        fs::write(&nsp, pfs0(&[("0100abf0089680000000000000000004.tik", vec![1; 0x40])], b"PFS0", 0x18)).unwrap();
        assert_eq!(title_id_of(&nsp, None), Some(0x0100ABF008968000));

        // NSP de mise à jour sans ticket : en-têtes NCA chiffrés ; le programme porte l'ID du
        // jeu de base, les métadonnées celui de la mise à jour.
        let nsp2 = dir.join("jeu2.nsp");
        fs::write(&nsp2, pfs0(&[("b.nca", nca(&key, 0x01001F5010DFA000, 0)), ("a.cnmt.nca", nca(&key, 0x01001F5010DFA800, 1))], b"PFS0", 0x18)).unwrap();
        assert_eq!(title_id_of(&nsp2, Some(&key)), Some(0x01001F5010DFA800));
        assert_eq!(title_id_of(&nsp2, None), None);

        // XCI : HEAD en 0x100, HFS0 racine → secure → NCA.
        let secure = pfs0(&[("c.nca", nca(&key, 0x0100A3D008C5C000, 0))], b"HFS0", 0x40);
        let root = pfs0(&[("update", vec![]), ("secure", secure)], b"HFS0", 0x40);
        let mut xci = vec![0u8; 0x200];
        xci[0x100..0x104].copy_from_slice(b"HEAD");
        xci[0x130..0x138].copy_from_slice(&0x200u64.to_le_bytes());
        xci.extend(root);
        let path = dir.join("jeu.xci");
        File::create(&path).unwrap().write_all(&xci).unwrap();
        assert_eq!(title_id_of(&path, Some(&key)), Some(0x0100A3D008C5C000));

        fs::remove_dir_all(&dir).unwrap();
    }
}
