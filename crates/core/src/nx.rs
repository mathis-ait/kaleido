//! Lecture des jeux Nintendo Switch (`.xci`, `.nsp`) : conteneurs, NCA chiffrés et
//! système de fichiers RomFS, à partir des clés de la console de l'utilisateur
//! (`prod.keys` de son émulateur). Kaleido ne fournit aucune clé.
//!
//! Formats (documentés par switchbrew, implémentés dans hactool / LibHac) :
//! - **PFS0** (NSP) et **HFS0** (partitions d'un XCI) : magic, nombre d'entrées, taille
//!   de la table des noms, puis entrées de 0x18 (PFS0) ou 0x40 (HFS0) octets.
//! - **XCI** : en-tête `HEAD` en 0x100, adresse de la partition racine en +0x130 ;
//!   les NCA du jeu sont dans la partition `secure`.
//! - **NCA** : en-tête de 0xC00 octets chiffré en AES-128-XTS (`header_key`, secteurs de
//!   0x200, tweak gros-boutiste). Sections décrites en 0x240 (début et fin en blocs de
//!   0x200) et en 0x400 + n × 0x200 (en-têtes FS). Données en AES-128-CTR : compteur =
//!   `section_ctr` (FS + 0x140) à l'envers, puis offset / 16 en gros-boutiste. Clé : la
//!   3ᵉ clé de la zone de clés (0x300), déchiffrée en AES-ECB avec
//!   `key_area_key_application_<génération>` ; ou la clé du ticket (`rights ID` non nul),
//!   déchiffrée avec `titlekek_<génération>`.
//! - **RomFS** (IVFC) : données au niveau 6 de l'arbre de hachage (FS + 0x90) ; en-tête
//!   de 0x50 octets puis tables des dossiers et des fichiers.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;

#[derive(Debug, thiserror::Error)]
pub enum NxError {
    #[error("lecture impossible : {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Format(String),
    #[error("clé manquante dans prod.keys : {0}")]
    MissingKey(String),
}

type Result<T> = std::result::Result<T, NxError>;

fn bad(msg: impl Into<String>) -> NxError {
    NxError::Format(msg.into())
}

// ---------------------------------------------------------------------------
// Clés

#[derive(Debug, Clone, Default)]
pub struct Keys {
    map: HashMap<String, Vec<u8>>,
}

impl Keys {
    /// Lit un fichier `prod.keys` (`nom = hexa` par ligne).
    pub fn parse(text: &str) -> Keys {
        let mut map = HashMap::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            if v.len() % 2 != 0 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            let bytes = (0..v.len()).step_by(2).filter_map(|i| u8::from_str_radix(&v[i..i + 2], 16).ok()).collect();
            map.insert(k.trim().to_ascii_lowercase(), bytes);
        }
        Keys { map }
    }

    pub fn load(path: &Path) -> Result<Keys> {
        Ok(Keys::parse(&std::fs::read_to_string(path)?))
    }

    fn get<const N: usize>(&self, name: &str) -> Result<[u8; N]> {
        self.map.get(name).and_then(|v| v.as_slice().try_into().ok()).ok_or_else(|| NxError::MissingKey(name.into()))
    }
}

fn aes_ecb_decrypt(key: &[u8; 16], data: &[u8; 16]) -> [u8; 16] {
    let c = Aes128::new(GenericArray::from_slice(key));
    let mut b = GenericArray::clone_from_slice(data);
    c.decrypt_block(&mut b);
    b.into()
}

/// Multiplication par α dans GF(2¹²⁸) (convention XTS).
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

/// AES-128-XTS « Nintendo » (tweak = numéro de secteur en gros-boutiste).
pub fn xts_decrypt(key: &[u8; 32], data: &mut [u8], first_sector: u64) {
    let k1 = Aes128::new(GenericArray::from_slice(&key[..16]));
    let k2 = Aes128::new(GenericArray::from_slice(&key[16..]));
    for (i, sector) in data.chunks_mut(0x200).enumerate() {
        let mut tweak = [0u8; 16];
        tweak[8..].copy_from_slice(&(first_sector + i as u64).to_be_bytes());
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

// ---------------------------------------------------------------------------
// Conteneurs

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub offset: u64,
    pub size: u64,
}

fn read_at(f: &mut File, offset: u64, len: usize) -> Result<Vec<u8>> {
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

/// PFS0 (`entry_size` 0x18) ou HFS0 (0x40) situé à `base` ; offsets absolus.
pub fn read_partition(f: &mut File, base: u64, magic: &[u8; 4], entry_size: usize) -> Option<Vec<Entry>> {
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
    (0..count)
        .map(|i| {
            let e = &table[i * entry_size..];
            let name_off = u32le(e, 0x10) as usize;
            let end = names.get(name_off..)?.iter().position(|&b| b == 0).map_or(names.len(), |p| name_off + p);
            Some(Entry { name: String::from_utf8_lossy(&names[name_off..end]).into_owned(), offset: data + u64le(e, 0), size: u64le(e, 8) })
        })
        .collect()
}

/// Contenus (NCA) et tickets d'un fichier `.xci` ou `.nsp`.
pub fn container_entries(f: &mut File) -> Result<Vec<Entry>> {
    if let Some(entries) = read_partition(f, 0, b"PFS0", 0x18) {
        return Ok(entries);
    }
    for head in [0x100u64, 0x1100] {
        let h = read_at(f, head, 0x40)?;
        if &h[..4] != b"HEAD" {
            continue;
        }
        let root = u64le(&h, 0x30);
        let parts = read_partition(f, head - 0x100 + root, b"HFS0", 0x40).ok_or_else(|| bad("partition racine du XCI illisible"))?;
        let secure = parts.iter().find(|p| p.name == "secure").ok_or_else(|| bad("pas de partition secure dans le XCI"))?;
        return read_partition(f, secure.offset, b"HFS0", 0x40).ok_or_else(|| bad("partition secure illisible"));
    }
    Err(bad("ce n'est pas un jeu Switch (.xci ou .nsp)"))
}

// ---------------------------------------------------------------------------
// NCA

/// Type de contenu d'un NCA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Program,
    Meta,
    Control,
    Manual,
    Data,
    PublicData,
    Other(u8),
}

#[derive(Debug, Clone)]
pub struct NcaInfo {
    pub title_id: u64,
    pub content: ContentType,
    pub size: u64,
}

#[derive(Debug, Clone)]
struct Section {
    start: u64,
    end: u64,
    /// 1 : aucun, 2 : XTS, 3 : CTR, 4 : BKTR (mise à jour).
    crypto: u8,
    ctr: [u8; 8],
    /// Début des données RomFS (niveau 6 de l'IVFC), relatif à la section.
    romfs: Option<u64>,
}

/// Un NCA ouvert, avec sa clé de section déchiffrée.
pub struct Nca {
    file: File,
    base: u64,
    pub info: NcaInfo,
    sections: Vec<Section>,
    key: Option<[u8; 16]>,
}

fn decrypt_header(f: &mut File, offset: u64, keys: &Keys) -> Result<Vec<u8>> {
    let header_key: [u8; 32] = keys.get("header_key")?;
    let mut h = read_at(f, offset, 0xC00)?;
    xts_decrypt(&header_key, &mut h, 0);
    if &h[0x200..0x204] != b"NCA3" {
        return Err(bad("en-tête NCA illisible (clé header_key incorrecte ?)"));
    }
    Ok(h)
}

/// Clé de titre d'un ticket (`.tik`) : bloc de 16 octets en 0x180, chiffré avec `titlekek`.
pub fn ticket_key(ticket: &[u8]) -> Option<([u8; 16], [u8; 16])> {
    let rights: [u8; 16] = ticket.get(0x2A0..0x2B0)?.try_into().ok()?;
    let key: [u8; 16] = ticket.get(0x180..0x190)?.try_into().ok()?;
    Some((rights, key))
}

impl Nca {
    /// `titlekeys` : clés de titre (chiffrées) des tickets du conteneur, par rights ID.
    pub fn open(path: &Path, entry: &Entry, keys: &Keys, titlekeys: &HashMap<[u8; 16], [u8; 16]>) -> Result<Nca> {
        let mut file = File::open(path)?;
        let h = decrypt_header(&mut file, entry.offset, keys)?;
        let content = match h[0x205] {
            0 => ContentType::Program,
            1 => ContentType::Meta,
            2 => ContentType::Control,
            3 => ContentType::Manual,
            4 => ContentType::Data,
            5 => ContentType::PublicData,
            n => ContentType::Other(n),
        };
        let info = NcaInfo { title_id: u64le(&h, 0x210), content, size: u64le(&h, 0x208) };
        let generation = h[0x206].max(h[0x220]).saturating_sub(1);
        let rights: [u8; 16] = h[0x230..0x240].try_into().unwrap();
        let key = if rights.iter().any(|&b| b != 0) {
            match titlekeys.get(&rights) {
                Some(enc) => {
                    let kek: [u8; 16] = keys.get(&format!("titlekek_{generation:02x}"))?;
                    Some(aes_ecb_decrypt(&kek, enc))
                }
                None => None,
            }
        } else {
            let index = h[0x207];
            let kak: [u8; 16] = match index {
                0 => keys.get(&format!("key_area_key_application_{generation:02x}"))?,
                1 => keys.get(&format!("key_area_key_ocean_{generation:02x}"))?,
                _ => keys.get(&format!("key_area_key_system_{generation:02x}"))?,
            };
            let enc: [u8; 16] = h[0x300 + 0x20..0x300 + 0x30].try_into().unwrap();
            Some(aes_ecb_decrypt(&kak, &enc))
        };
        let mut sections = Vec::new();
        for i in 0..4 {
            let e = &h[0x240 + i * 0x10..];
            let (start, end) = (u32le(e, 0) as u64 * 0x200, u32le(e, 4) as u64 * 0x200);
            if end <= start {
                continue;
            }
            let fs = &h[0x400 + i * 0x200..0x600 + i * 0x200];
            let hash_type = fs[3];
            let romfs = (hash_type == 3 && &fs[0x08..0x0C] == b"IVFC").then(|| u64le(fs, 0x08 + 0x10 + 5 * 0x18));
            sections.push(Section { start, end, crypto: fs[4], ctr: fs[0x140..0x148].try_into().unwrap(), romfs });
        }
        Ok(Nca { file, base: entry.offset, info, sections, key })
    }

    /// Lit `len` octets déchiffrés d'une section, à partir de `offset` (relatif à la section).
    fn read_section(&mut self, s: usize, offset: u64, len: usize) -> Result<Vec<u8>> {
        let sec = self.sections[s].clone();
        let abs = sec.start + offset;
        if abs + len as u64 > sec.end {
            return Err(bad("lecture hors de la section"));
        }
        let aligned = abs & !0xF;
        let pad = (abs - aligned) as usize;
        let mut buf = read_at(&mut self.file, self.base + aligned, (pad + len + 15) & !15)?;
        match sec.crypto {
            1 => {}
            3 | 4 => {
                let key = self.key.ok_or_else(|| bad("clé du jeu introuvable (ticket manquant ?)"))?;
                let cipher = Aes128::new(GenericArray::from_slice(&key));
                let mut ctr = [0u8; 16];
                for j in 0..8 {
                    ctr[j] = sec.ctr[7 - j];
                }
                let mut block_index = aligned / 16;
                for chunk in buf.chunks_mut(16) {
                    ctr[8..].copy_from_slice(&block_index.to_be_bytes());
                    let mut ks = GenericArray::clone_from_slice(&ctr);
                    cipher.encrypt_block(&mut ks);
                    for (b, k) in chunk.iter_mut().zip(ks.iter()) {
                        *b ^= k;
                    }
                    block_index += 1;
                }
            }
            n => return Err(bad(format!("chiffrement de section {n} non pris en charge"))),
        }
        buf.drain(..pad);
        buf.truncate(len);
        Ok(buf)
    }

    /// Système de fichiers RomFS du NCA (le premier trouvé).
    pub fn romfs(&mut self) -> Result<RomFs<'_>> {
        let (index, data) = self.sections.iter().enumerate().find_map(|(i, s)| s.romfs.map(|o| (i, o))).ok_or_else(|| bad("pas de RomFS dans ce contenu"))?;
        if self.sections[index].crypto == 4 {
            return Err(bad("RomFS de mise à jour (BKTR) : utilise le jeu de base"));
        }
        let header = self.read_section(index, data, 0x50)?;
        let at = |i: usize| u64le(&header, i * 8);
        let dirs = self.read_section(index, data + at(3), at(4) as usize)?;
        let files = self.read_section(index, data + at(7), at(8) as usize)?;
        Ok(RomFs { nca: self, section: index, data_base: data + at(9), dirs, files })
    }
}

/// Ouvre le NCA « programme » d'un jeu (celui qui contient la RomFS).
pub fn open_program(path: &Path, keys: &Keys) -> Result<Nca> {
    let mut f = File::open(path)?;
    let entries = container_entries(&mut f)?;
    let mut titlekeys = HashMap::new();
    for t in entries.iter().filter(|e| e.name.ends_with(".tik")) {
        if let Some((rights, key)) = ticket_key(&read_at(&mut f, t.offset, t.size.min(0x400) as usize)?) {
            titlekeys.insert(rights, key);
        }
    }
    let mut best: Option<Nca> = None;
    for e in entries.iter().filter(|e| e.name.ends_with(".nca")) {
        let Ok(nca) = Nca::open(path, e, keys, &titlekeys) else { continue };
        if nca.info.content == ContentType::Program && nca.sections.iter().any(|s| s.romfs.is_some() && s.crypto != 4) && best.as_ref().is_none_or(|b| nca.info.size > b.info.size) {
            best = Some(nca);
        }
    }
    best.ok_or_else(|| bad("aucun contenu « programme » avec RomFS dans ce fichier"))
}

// ---------------------------------------------------------------------------
// RomFS

pub struct RomFs<'a> {
    nca: &'a mut Nca,
    section: usize,
    data_base: u64,
    dirs: Vec<u8>,
    files: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RomFile {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

const NONE: u32 = 0xFFFF_FFFF;

impl RomFs<'_> {
    fn name(table: &[u8], at: usize, name_off: usize) -> String {
        let len = u32le(table, at + name_off - 4) as usize;
        String::from_utf8_lossy(&table[at + name_off..at + name_off + len]).into_owned()
    }

    /// Tous les fichiers, avec leur chemin complet.
    pub fn list(&self) -> Vec<RomFile> {
        let mut out = Vec::new();
        let mut stack = vec![(0u32, String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let d = dir as usize;
            if d + 0x18 > self.dirs.len() {
                continue;
            }
            // Fichiers du dossier.
            let mut file = u32le(&self.dirs, d + 0x0C);
            let mut guard = 0;
            while file != NONE && guard < 1_000_000 {
                let f = file as usize;
                if f + 0x20 > self.files.len() {
                    break;
                }
                let name = Self::name(&self.files, f, 0x20);
                out.push(RomFile { path: format!("{prefix}/{name}"), offset: u64le(&self.files, f + 0x08), size: u64le(&self.files, f + 0x10) });
                file = u32le(&self.files, f + 0x04);
                guard += 1;
            }
            // Sous-dossiers.
            let mut child = u32le(&self.dirs, d + 0x08);
            while child != NONE && guard < 1_000_000 {
                let c = child as usize;
                if c + 0x18 > self.dirs.len() {
                    break;
                }
                let name = Self::name(&self.dirs, c, 0x18);
                stack.push((child, format!("{prefix}/{name}")));
                child = u32le(&self.dirs, c + 0x04);
                guard += 1;
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    /// Lit `len` octets d'un fichier à partir de `offset`.
    pub fn read(&mut self, file: &RomFile, offset: u64, len: usize) -> Result<Vec<u8>> {
        let len = len.min(file.size.saturating_sub(offset) as usize);
        self.nca.read_section(self.section, self.data_base + file.offset + offset, len)
    }

    pub fn read_all(&mut self, file: &RomFile) -> Result<Vec<u8>> {
        self.read(file, 0, file.size as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_file() {
        let k = Keys::parse("header_key = 00112233445566778899AABBCCDDEEFF00112233445566778899AABBCCDDEEFF\nbad = xyz\n");
        let h: [u8; 32] = k.get("header_key").unwrap();
        assert_eq!(h[1], 0x11);
        assert!(k.get::<16>("titlekek_00").is_err());
    }

    #[test]
    fn xts_vector() {
        // IEEE 1619, vecteur 1 : clés et tweak nuls, 32 octets nuls.
        let hex = "917cf69ebd68b2ec9b9fe9a3eadda692cd43d2f59598ed858c02c2652fbf922e";
        let mut data: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        xts_decrypt(&[0u8; 32], &mut data, 0);
        assert_eq!(data, vec![0u8; 32]);
    }

    /// Vrai jeu : `KALEIDO_NX_GAME=<xci|nsp> KALEIDO_NX_KEYS=<prod.keys> cargo test -p kaleido-core real_nx_romfs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_nx_romfs() {
        let game = std::path::PathBuf::from(std::env::var("KALEIDO_NX_GAME").unwrap());
        let keys = Keys::load(Path::new(&std::env::var("KALEIDO_NX_KEYS").unwrap())).unwrap();
        let mut nca = open_program(&game, &keys).unwrap();
        println!("title ID {:016X}, {} Mo", nca.info.title_id, nca.info.size >> 20);
        let romfs = nca.romfs().unwrap();
        let files = romfs.list();
        println!("{} fichiers", files.len());
        let filter = std::env::var("KALEIDO_NX_FILTER").unwrap_or_default().to_lowercase();
        for f in files.iter().filter(|f| filter.is_empty() || f.path.to_lowercase().contains(&filter)).take(400) {
            println!("{:>12}  {}", f.size, f.path);
        }
        assert!(!files.is_empty());
        // Morceaux de fichiers : KALEIDO_NX_SLICES="fichier|offset|taille;…" vers KALEIDO_NX_OUT.
        if let (Ok(spec), Ok(out)) = (std::env::var("KALEIDO_NX_SLICES"), std::env::var("KALEIDO_NX_OUT")) {
            let mut romfs = romfs;
            for part in spec.split(';').filter(|s| !s.is_empty()) {
                let v: Vec<&str> = part.split('|').collect();
                let f = files.iter().find(|f| f.path.contains(v[0])).unwrap();
                let (off, len): (u64, usize) = (v[1].parse().unwrap(), v[2].parse().unwrap());
                std::fs::write(Path::new(&out).join(format!("{off}.wem")), romfs.read(f, off, len).unwrap()).unwrap();
            }
            return;
        }
        // Recherche d'un texte dans les fichiers de moins de 8 Mo : KALEIDO_NX_GREP=<texte>.
        if let Ok(needle) = std::env::var("KALEIDO_NX_GREP") {
            let needle = needle.as_bytes().to_ascii_uppercase();
            let mut romfs = romfs;
            let mut found = std::collections::BTreeSet::new();
            for f in files.iter().filter(|f| f.size < 8 << 20) {
                let data = romfs.read_all(f).unwrap();
                let upper = data.to_ascii_uppercase();
                for (i, w) in upper.windows(needle.len()).enumerate() {
                    if w == needle.as_slice() {
                        let start = (0..i).rev().take_while(|&j| data[j].is_ascii_graphic()).last().unwrap_or(i);
                        let end = (i..data.len()).take_while(|&j| data[j].is_ascii_graphic()).last().unwrap_or(i) + 1;
                        found.insert(format!("{}  ({})", String::from_utf8_lossy(&data[start..end]), f.path));
                    }
                }
            }
            for s in found.iter().take(300) {
                println!("{s}");
            }
            return;
        }
        // Extraction : KALEIDO_NX_OUT=<dossier>, fichiers filtrés (16 premiers Mo de chacun).
        if let Ok(out) = std::env::var("KALEIDO_NX_OUT") {
            let mut romfs = romfs;
            for f in files.iter().filter(|f| f.path.to_lowercase().contains(&filter)) {
                let data = romfs.read(f, 0, 16 << 20).unwrap();
                let name = f.path.trim_start_matches('/').replace('/', "_");
                std::fs::write(Path::new(&out).join(name), data).unwrap();
            }
        }
    }
}
