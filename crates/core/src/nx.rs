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
use std::path::{Path, PathBuf};

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

/// Octets qui manquent à la fin d'un `.nsp` ou `.xci` (téléchargement ou copie interrompus) :
/// le contenu annoncé dépasse la taille du fichier. `None` si le fichier est complet ou illisible.
pub fn missing_bytes(path: &Path) -> Option<u64> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let end = container_entries(&mut f).ok()?.iter().map(|e| e.offset + e.size).max()?;
    (end > len).then(|| end - len)
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
    /// Début du PFS0 (ExeFS d'un programme), relatif à la section.
    pfs0: Option<u64>,
    /// Mise à jour (BKTR) : tables de relocalisation et de sous-sections (offset, taille).
    bktr: Option<[(u64, u64); 2]>,
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
            // Hachage SHA-256 hiérarchique (type 2) : la 2e région est le PFS0.
            let pfs0 = (hash_type == 2).then(|| u64le(fs, 0x08 + 0x28 + 0x10));
            let bktr = (fs[4] == 4).then(|| [(u64le(fs, 0x100), u64le(fs, 0x108)), (u64le(fs, 0x120), u64le(fs, 0x128))]);
            sections.push(Section { start, end, crypto: fs[4], ctr: fs[0x140..0x148].try_into().unwrap(), romfs, pfs0, bktr });
        }
        Ok(Nca { file, base: entry.offset, info, sections, key })
    }

    /// Lit `len` octets déchiffrés d'une section, à partir de `offset` (relatif à la section).
    fn read_section(&mut self, s: usize, offset: u64, len: usize) -> Result<Vec<u8>> {
        self.read_section_gen(s, offset, len, None)
    }

    /// Comme `read_section`, avec la « génération » d'une sous-section de mise à jour (BKTR),
    /// qui remplace les octets 4 à 7 du compteur AES-CTR.
    fn read_section_gen(&mut self, s: usize, offset: u64, len: usize, generation: Option<u32>) -> Result<Vec<u8>> {
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
                if let Some(g) = generation {
                    ctr[4..8].copy_from_slice(&g.to_be_bytes());
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

    /// Fichiers du PFS0 de la section qui en contient un (ExeFS, ou contenu « méta ») :
    /// (section, nom, offset dans la section, taille).
    fn pfs0_files(&mut self) -> Result<Vec<(usize, String, u64, u64)>> {
        let (index, base) = self.sections.iter().enumerate().find_map(|(i, s)| s.pfs0.map(|o| (i, o))).ok_or_else(|| bad("pas de PFS0 dans ce contenu"))?;
        let head = self.read_section(index, base, 0x10)?;
        if &head[..4] != b"PFS0" {
            return Err(bad("PFS0 illisible"));
        }
        let (count, strtab) = (u32le(&head, 4) as usize, u32le(&head, 8) as usize);
        if count == 0 || count > 4096 || strtab > 1 << 20 {
            return Err(bad("PFS0 illisible"));
        }
        let table = self.read_section(index, base + 0x10, count * 0x18 + strtab)?;
        let names = &table[count * 0x18..];
        let data = base + 0x10 + table.len() as u64;
        Ok((0..count)
            .map(|i| {
                let e = &table[i * 0x18..];
                let off = u32le(e, 0x10) as usize;
                let end = names.get(off..).and_then(|n| n.iter().position(|&b| b == 0)).map_or(names.len(), |p| off + p);
                (index, String::from_utf8_lossy(names.get(off..end).unwrap_or_default()).into_owned(), data + u64le(e, 0), u64le(e, 8))
            })
            .collect())
    }

    /// Contenu « méta » (CNMT) d'un NCA de type Meta.
    pub fn cnmt(&mut self) -> Result<Cnmt> {
        let files = self.pfs0_files()?;
        let (s, _, off, size) = files.into_iter().find(|(_, n, _, _)| n.ends_with(".cnmt")).ok_or_else(|| bad("pas de fichier .cnmt"))?;
        let data = self.read_section(s, off, size as usize)?;
        Cnmt::parse(&data)
    }

    /// Build ID (ModuleId, 32 octets) de l'exécutable `main` de l'ExeFS.
    pub fn main_build_id(&mut self) -> Result<[u8; 32]> {
        let (index, base) = self.sections.iter().enumerate().find_map(|(i, s)| s.pfs0.map(|o| (i, o))).ok_or_else(|| bad("pas d'ExeFS dans ce contenu"))?;
        let head = self.read_section(index, base, 0x10)?;
        if &head[..4] != b"PFS0" {
            return Err(bad("ExeFS illisible"));
        }
        let (count, strtab) = (u32le(&head, 4) as usize, u32le(&head, 8) as usize);
        if count == 0 || count > 64 || strtab > 1 << 16 {
            return Err(bad("ExeFS illisible"));
        }
        let table = self.read_section(index, base + 0x10, count * 0x18 + strtab)?;
        let names = &table[count * 0x18..];
        let data = base + 0x10 + table.len() as u64;
        for i in 0..count {
            let e = &table[i * 0x18..];
            let off = u32le(e, 0x10) as usize;
            let end = names.get(off..).and_then(|n| n.iter().position(|&b| b == 0)).map_or(names.len(), |p| off + p);
            if names.get(off..end) == Some(b"main".as_slice()) {
                let nso = self.read_section(index, data + u64le(e, 0), 0x60)?;
                if &nso[..4] != b"NSO0" {
                    return Err(bad("exécutable main illisible"));
                }
                return Ok(nso[0x40..0x60].try_into().unwrap());
            }
        }
        Err(bad("pas d'exécutable main dans l'ExeFS"))
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
        Ok(RomFs { src: Source::Plain(self, index), data_base: data + at(9), dirs, files })
    }

    /// RomFS d'une mise à jour (section BKTR) : les données non modifiées sont lues dans le
    /// jeu de base `base`.
    pub fn patched_romfs<'a>(&'a mut self, base: &'a mut Nca) -> Result<RomFs<'a>> {
        let (index, data) = self.sections.iter().enumerate().find_map(|(i, s)| s.romfs.filter(|_| s.crypto == 4).map(|o| (i, o))).ok_or_else(|| bad("pas de RomFS de mise à jour dans ce contenu"))?;
        let base_index = base.sections.iter().position(|s| s.romfs.is_some() && s.crypto != 4).ok_or_else(|| bad("pas de RomFS dans le jeu de base"))?;
        let [(reloc_off, reloc_size), (subs_off, subs_size)] = self.sections[index].bktr.ok_or_else(|| bad("table de mise à jour absente"))?;
        let reloc_raw = self.read_section(index, reloc_off, reloc_size as usize)?;
        let subs_raw = self.read_section(index, subs_off, subs_size as usize)?;
        let (reloc, reloc_end) = bucket_entries(&reloc_raw, 0x14);
        let (subs, subs_end) = bucket_entries(&subs_raw, 0x10);
        let reloc: Vec<(u64, u64, u32)> = reloc.iter().map(|e| (u64le(e, 0), u64le(e, 8), u32le(e, 16))).collect();
        let subs: Vec<(u64, u32)> = subs.iter().map(|e| (u64le(e, 0), u32le(e, 12))).collect();
        if reloc.is_empty() || subs.is_empty() {
            return Err(bad("tables de mise à jour vides"));
        }
        let mut src = Source::Patched(Box::new(Patched { patch: self, section: index, base, base_section: base_index, reloc, reloc_end, subs, subs_end }));
        let header = src.read(data, 0x50)?;
        let at = |i: usize| u64le(&header, i * 8);
        let dirs = src.read(data + at(3), at(4) as usize)?;
        let files = src.read(data + at(7), at(8) as usize)?;
        Ok(RomFs { src, data_base: data + at(9), dirs, files })
    }
}

/// Entrées d'un arbre de compartiments (BKTR) : nœud racine de 0x4000 octets
/// (`nombre de compartiments` en +4), puis les compartiments de 0x4000 octets
/// (`nombre d'entrées` en +4, entrées dès +0x10). Rend aussi l'offset de fin (+8 de la racine).
fn bucket_entries(buf: &[u8], entry_size: usize) -> (Vec<&[u8]>, u64) {
    const NODE: usize = 0x4000;
    if buf.len() < 0x10 {
        return (vec![], 0);
    }
    let buckets = u32le(buf, 4) as usize;
    let end = u64le(buf, 8);
    let mut out = Vec::new();
    for b in 0..buckets {
        let at = NODE * (1 + b);
        if at + 0x10 > buf.len() {
            break;
        }
        let n = (u32le(buf, at + 4) as usize).min((NODE - 0x10) / entry_size);
        for i in 0..n {
            let e = at + 0x10 + i * entry_size;
            if let Some(slice) = buf.get(e..e + entry_size) {
                out.push(slice);
            }
        }
    }
    (out, end)
}

/// Mise à jour ouverte par-dessus le jeu de base.
struct Patched<'a> {
    patch: &'a mut Nca,
    section: usize,
    base: &'a mut Nca,
    base_section: usize,
    /// (offset virtuel, offset physique, 0 = jeu de base / 1 = mise à jour), triés.
    reloc: Vec<(u64, u64, u32)>,
    reloc_end: u64,
    /// (offset physique dans la mise à jour, génération du compteur), triés.
    subs: Vec<(u64, u32)>,
    subs_end: u64,
}

impl Patched<'_> {
    fn read_patch(&mut self, mut phys: u64, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let i = self.subs.partition_point(|s| s.0 <= phys).checked_sub(1).ok_or_else(|| bad("sous-section de mise à jour introuvable"))?;
            let next = self.subs.get(i + 1).map_or(self.subs_end.max(phys + 1), |s| s.0);
            let chunk = ((next - phys) as usize).min(len - out.len());
            out.extend(self.patch.read_section_gen(self.section, phys, chunk, Some(self.subs[i].1))?);
            phys += chunk as u64;
        }
        Ok(out)
    }

    fn read(&mut self, mut virt: u64, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let i = self.reloc.partition_point(|r| r.0 <= virt).checked_sub(1).ok_or_else(|| bad("donnée de mise à jour introuvable"))?;
            let (v, p, storage) = self.reloc[i];
            let next = self.reloc.get(i + 1).map_or(self.reloc_end.max(virt + 1), |r| r.0);
            let chunk = ((next - virt) as usize).min(len - out.len());
            let phys = p + (virt - v);
            let data = if storage == 0 { self.base.read_section(self.base_section, phys, chunk)? } else { self.read_patch(phys, chunk)? };
            out.extend(data);
            virt += chunk as u64;
        }
        Ok(out)
    }
}

/// D'où une RomFS lit ses octets.
enum Source<'a> {
    Plain(&'a mut Nca, usize),
    Patched(Box<Patched<'a>>),
}

impl Source<'_> {
    fn read(&mut self, offset: u64, len: usize) -> Result<Vec<u8>> {
        match self {
            Source::Plain(nca, section) => nca.read_section(*section, offset, len),
            Source::Patched(p) => p.read(offset, len),
        }
    }
}

/// Ouvre le NCA « programme » d'une mise à jour (RomFS BKTR).
pub fn open_update_program(path: &Path, keys: &Keys) -> Result<Nca> {
    let mut f = File::open(path)?;
    let entries = container_entries(&mut f)?;
    let mut titlekeys = HashMap::new();
    for t in entries.iter().filter(|e| e.name.ends_with(".tik")) {
        if let Some((rights, key)) = ticket_key(&read_at(&mut f, t.offset, t.size.min(0x400) as usize)?) {
            titlekeys.insert(rights, key);
        }
    }
    for e in entries.iter().filter(|e| e.name.ends_with(".nca")) {
        let Ok(nca) = Nca::open(path, e, keys, &titlekeys) else { continue };
        if nca.info.content == ContentType::Program && nca.sections.iter().any(|s| s.romfs.is_some() && s.crypto == 4) {
            return Ok(nca);
        }
    }
    Err(bad("aucune RomFS de mise à jour dans ce fichier"))
}

/// Contenu d'un fichier de la RomFS du jeu, mise à jour comprise quand elle est donnée.
pub fn read_game_file(base: &Path, update: Option<&Path>, keys: &Keys, path: &str) -> Result<Vec<u8>> {
    let want = format!("/{}", path.trim_start_matches('/'));
    let mut base_nca = open_program(base, keys)?;
    let mut patch_nca;
    let mut romfs = match update {
        Some(u) => {
            patch_nca = open_update_program(u, keys)?;
            patch_nca.patched_romfs(&mut base_nca)?
        }
        None => base_nca.romfs()?,
    };
    let file = romfs.list().into_iter().find(|f| f.path.eq_ignore_ascii_case(&want)).ok_or_else(|| bad(format!("fichier {path} absent du jeu")))?;
    romfs.read_all(&file)
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

/// Build ID de l'exécutable d'un jeu ou d'une mise à jour (.xci, .nsp).
pub fn program_build_id(path: &Path, keys: &Keys) -> Result<[u8; 32]> {
    let all = program_build_ids(path, keys)?;
    // Le jeu de base d'abord (un .xci peut contenir aussi une mise à jour).
    all.iter().find(|(tid, _)| tid & 0xFFF == 0).or(all.first()).map(|(_, id)| *id).ok_or_else(|| bad("aucun exécutable dans ce fichier"))
}

/// Build ID de chaque exécutable d'un fichier : (title ID du contenu, build ID). Un .xci
/// « + Expansion Pass » contient le jeu (…000) et sa mise à jour (…800).
pub fn program_build_ids(path: &Path, keys: &Keys) -> Result<Vec<(u64, [u8; 32])>> {
    let mut f = File::open(path)?;
    let entries = container_entries(&mut f)?;
    let mut titlekeys = HashMap::new();
    for t in entries.iter().filter(|e| e.name.ends_with(".tik")) {
        if let Some((rights, key)) = ticket_key(&read_at(&mut f, t.offset, t.size.min(0x400) as usize)?) {
            titlekeys.insert(rights, key);
        }
    }
    let mut last = bad("aucun exécutable dans ce fichier");
    let mut out = Vec::new();
    for e in entries.iter().filter(|e| e.name.ends_with(".nca")) {
        let Ok(mut nca) = Nca::open(path, e, keys, &titlekeys) else { continue };
        if nca.info.content != ContentType::Program || nca.sections.iter().all(|s| s.pfs0.is_none()) {
            continue;
        }
        match nca.main_build_id() {
            // Le programme d'une mise à jour garde le title ID du jeu : on le reconnaît à sa
            // RomFS de mise à jour (BKTR) et on le range sous …800.
            Ok(id) => out.push((if nca.sections.iter().any(|s| s.crypto == 4) { nca.info.title_id | 0x800 } else { nca.info.title_id }, id)),
            Err(err) => last = err,
        }
    }
    if out.is_empty() {
        return Err(last);
    }
    Ok(out)
}

/// Contenu « méta » d'un titre : son identifiant, sa version et la liste de ses NCA.
#[derive(Debug, Clone, PartialEq)]
pub struct Cnmt {
    pub title_id: u64,
    pub version: u32,
    /// (identifiant du NCA, type de contenu : 0 méta, 1 programme, 3 contrôle, 6 fragment delta…).
    pub contents: Vec<([u8; 16], u8)>,
}

impl Cnmt {
    pub fn parse(d: &[u8]) -> Result<Cnmt> {
        if d.len() < 0x20 {
            return Err(bad("CNMT tronqué"));
        }
        let ext = u16::from_le_bytes([d[0x0E], d[0x0F]]) as usize;
        let count = u16::from_le_bytes([d[0x10], d[0x11]]) as usize;
        let mut contents = Vec::new();
        for i in 0..count {
            let at = 0x20 + ext + i * 0x38;
            let rec = d.get(at..at + 0x38).ok_or_else(|| bad("CNMT tronqué"))?;
            contents.push((rec[0x20..0x30].try_into().unwrap(), rec[0x36]));
        }
        Ok(Cnmt { title_id: u64le(d, 0), version: u32le(d, 8), contents })
    }
}

fn hex_id(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_hex16(s: &str) -> Option<[u8; 16]> {
    let s = s.trim();
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Chemin d'un NCA dans `nand/user/Contents/registered`, comme Eden l'écrit :
/// `000000XX/<identifiant>.nca`, XX = premier octet du SHA-256 de l'identifiant.
pub fn registered_path(id: &[u8; 16]) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(id);
    format!("000000{:02X}/{}.nca", h[0], hex_id(id))
}

/// Clés de titre (chiffrées) connues d'Eden : `keys/title.keys` et `title.keys_autogenerated`.
pub fn eden_title_keys(keys_dir: &Path) -> HashMap<[u8; 16], [u8; 16]> {
    let mut out = HashMap::new();
    for name in ["title.keys_autogenerated", "title.keys"] {
        let Ok(text) = std::fs::read(keys_dir.join(name)) else { continue };
        for line in String::from_utf8_lossy(&text).lines() {
            if let Some((r, k)) = line.split_once('=') {
                if let (Some(r), Some(k)) = (parse_hex16(r), parse_hex16(k)) {
                    out.insert(r, k);
                }
            }
        }
    }
    out
}

/// NCA présents dans un dossier `registered` (fichiers `000000XX/<id>.nca`).
fn registered_ncas(registered: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in std::fs::read_dir(registered).into_iter().flatten().flatten() {
        if !d.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
            let p = f.path();
            if p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nca")) {
                out.push(p);
            }
        }
    }
    out
}

fn open_loose(path: &Path, keys: &Keys, titlekeys: &HashMap<[u8; 16], [u8; 16]>) -> Result<Nca> {
    let size = std::fs::metadata(path)?.len();
    Nca::open(path, &Entry { name: String::new(), offset: 0, size }, keys, titlekeys)
}

/// Build ID et version de la mise à jour d'un jeu installée dans la NAND d'Eden, s'il y en a une.
pub fn nand_update(registered: &Path, keys_dir: &Path, keys: &Keys, base_tid: u64) -> Option<([u8; 32], u32)> {
    let titlekeys = eden_title_keys(keys_dir);
    let update_tid = base_tid | 0x800;
    let ncas = registered_ncas(registered);
    let meta = ncas.iter().filter_map(|p| open_loose(p, keys, &titlekeys).ok()).filter(|n| n.info.content == ContentType::Meta && n.info.title_id == update_tid).find_map(|mut n| n.cnmt().ok())?;
    for (id, kind) in &meta.contents {
        if *kind != 1 {
            continue;
        }
        let path = registered.join(registered_path(id));
        let mut nca = open_loose(&path, keys, &titlekeys).ok()?;
        if let Ok(b) = nca.main_build_id() {
            return Some((b, meta.version));
        }
    }
    None
}

/// Ce qu'une installation dans la NAND a fait.
#[derive(Debug, Clone, PartialEq)]
pub struct NandInstall {
    pub title_id: u64,
    pub version: u32,
    pub files: usize,
    pub bytes: u64,
    pub replaced: usize,
}

/// Installe une mise à jour (`.nsp`) dans la NAND d'Eden, comme son menu « Installer des
/// fichiers dans la NAND » : NCA copiés tels quels dans `registered`, l'ancienne version du
/// même titre retirée, clés des tickets ajoutées à `keys/title.keys_autogenerated`.
/// Seules les mises à jour sont acceptées.
pub fn install_update_to_nand(nsp: &Path, registered: &Path, keys_dir: &Path, keys: &Keys, mut progress: impl FnMut(u64, u64)) -> Result<NandInstall> {
    let mut f = File::open(nsp)?;
    let entries = container_entries(&mut f)?;
    let mut titlekeys = HashMap::new();
    for t in entries.iter().filter(|e| e.name.ends_with(".tik")) {
        if let Some((rights, key)) = ticket_key(&read_at(&mut f, t.offset, t.size.min(0x400) as usize)?) {
            titlekeys.insert(rights, key);
        }
    }
    let meta_entry = entries.iter().find(|e| e.name.ends_with(".cnmt.nca")).ok_or_else(|| bad("pas de contenu méta dans ce fichier"))?;
    let meta = Nca::open(nsp, meta_entry, keys, &titlekeys)?.cnmt()?;
    if meta.title_id & 0xFFF != 0x800 {
        return Err(bad("ce fichier n'est pas une mise à jour"));
    }
    let meta_id = parse_hex16(meta_entry.name.trim_end_matches(".cnmt.nca")).ok_or_else(|| bad("nom du contenu méta inattendu"))?;
    // Fichiers à copier : le contenu méta, puis chaque NCA listé (hors fragments delta).
    let mut copies: Vec<([u8; 16], &Entry)> = vec![(meta_id, meta_entry)];
    for (id, kind) in &meta.contents {
        if *kind == 6 {
            continue;
        }
        let name = format!("{}.nca", hex_id(id));
        let e = entries.iter().find(|e| e.name.eq_ignore_ascii_case(&name)).ok_or_else(|| bad(format!("contenu {name} absent du fichier")))?;
        copies.push((*id, e));
    }
    let total: u64 = copies.iter().map(|(_, e)| e.size).sum();
    if let Some(missing) = missing_bytes(nsp) {
        return Err(bad(format!("fichier incomplet (il manque {} Mo)", missing >> 20)));
    }

    // Ancienne version du même titre : retirée (comme Eden).
    let known = eden_title_keys(keys_dir);
    let mut replaced = 0;
    for p in registered_ncas(registered) {
        let Ok(mut n) = open_loose(&p, keys, &known) else { continue };
        if n.info.content != ContentType::Meta || n.info.title_id != meta.title_id {
            continue;
        }
        if let Ok(old) = n.cnmt() {
            for (id, _) in &old.contents {
                let _ = std::fs::remove_file(registered.join(registered_path(id)));
            }
        }
        drop(n);
        let _ = std::fs::remove_file(&p);
        replaced += 1;
    }

    // Copie.
    let mut done = 0u64;
    for (id, e) in &copies {
        let dest = registered.join(registered_path(id));
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let part = dest.with_extension("nca.part");
        {
            let mut out = std::io::BufWriter::new(File::create(&part)?);
            f.seek(SeekFrom::Start(e.offset))?;
            let mut left = e.size;
            let mut buf = vec![0u8; 1 << 20];
            while left > 0 {
                let n = (buf.len() as u64).min(left) as usize;
                f.read_exact(&mut buf[..n])?;
                std::io::Write::write_all(&mut out, &buf[..n])?;
                left -= n as u64;
                done += n as u64;
                progress(done, total);
            }
            std::io::Write::flush(&mut out)?;
        }
        std::fs::rename(&part, &dest)?;
    }

    // Clés des tickets, au format d'Eden (« identifiant = clé »).
    let auto = keys_dir.join("title.keys_autogenerated");
    let mut add = String::new();
    for (rights, key) in &titlekeys {
        if !known.contains_key(rights) {
            let hex = |b: &[u8; 16]| b.iter().map(|x| format!("{x:02X}")).collect::<String>();
            add.push_str(&format!("
{} = {}", hex(rights), hex(key)));
        }
    }
    if !add.is_empty() {
        std::fs::create_dir_all(keys_dir)?;
        let mut text = std::fs::read(&auto).unwrap_or_default();
        if text.is_empty() {
            text.extend_from_slice(b"# This file is autogenerated by Yuzu
# It serves to store keys that were automatically generated from the normal keys
# If you are experiencing issues involving keys, it may help to delete this file
");
        }
        text.extend_from_slice(add.as_bytes());
        std::fs::write(&auto, text)?;
    }
    Ok(NandInstall { title_id: meta.title_id, version: meta.version, files: copies.len(), bytes: total, replaced })
}

/// Version affichée d'un numéro de version de titre (`196608` → « 1.2.0 » n'est pas déductible) :
/// la convention Nintendo est `version >> 16` = numéro de mise à jour.
pub fn title_version_number(version: u32) -> u32 {
    version >> 16
}

/// Build ID tel que l'écrivent les correctifs (`@nsobid`, nom des .ips) : hexadécimal
/// majuscule, zéros de fin retirés (comme yuzu et Eden).
pub fn build_id_hex(id: &[u8]) -> String {
    let hex: String = id.iter().map(|b| format!("{b:02X}")).collect();
    hex.trim_end_matches('0').to_string()
}

/// Build ID visé par un correctif ExeFS : ligne `@nsobid-…` d'un .pchtxt, ou nom d'un .ips.
pub fn patch_build_id(file_name: &str, content: &[u8]) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    let norm = |s: &str| {
        let s = s.trim();
        (s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit())).then(|| s.to_ascii_uppercase().trim_end_matches('0').to_string())
    };
    if lower.ends_with(".pchtxt") {
        let text = String::from_utf8_lossy(content);
        return text.lines().find_map(|l| l.trim().strip_prefix("@nsobid-")).and_then(|id| norm(id.split_whitespace().next().unwrap_or("")));
    }
    if lower.ends_with(".ips") {
        return norm(&file_name[..file_name.len() - 4]);
    }
    None
}

/// Le correctif s'applique-t-il à l'exécutable dont le build ID est `game` (forme `build_id_hex`) ?
pub fn build_id_matches(patch: &str, game: &str) -> bool {
    let (p, g) = (patch.trim_end_matches('0'), game.trim_end_matches('0'));
    !p.is_empty() && (p.eq_ignore_ascii_case(g) || (p.len() >= 16 && g.len() >= p.len() && g[..p.len()].eq_ignore_ascii_case(p)))
}

// ---------------------------------------------------------------------------
// RomFS

pub struct RomFs<'a> {
    src: Source<'a>,
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
        self.src.read(self.data_base + file.offset + offset, len)
    }

    pub fn read_all(&mut self, file: &RomFile) -> Result<Vec<u8>> {
        self.read(file, 0, file.size as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `KALEIDO_NX_FILE=<jeu de base> KALEIDO_NX_UPDATE=<mise à jour> KALEIDO_NX_KEYS=<prod.keys> cargo test -p kaleido-core real_patched_romfs -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_patched_romfs() {
        let (Ok(file), Ok(update), Ok(keys)) = (std::env::var("KALEIDO_NX_FILE"), std::env::var("KALEIDO_NX_UPDATE"), std::env::var("KALEIDO_NX_KEYS")) else { return };
        let keys = Keys::load(Path::new(&keys)).unwrap();
        let mut base = open_program(Path::new(&file), &keys).unwrap();
        let base_files = base.romfs().unwrap().list();
        let mut patch = open_update_program(Path::new(&update), &keys).unwrap();
        let mut romfs = patch.patched_romfs(&mut base).unwrap();
        let files = romfs.list();
        println!("jeu de base : {} fichiers, avec la mise à jour : {}", base_files.len(), files.len());
        let new: Vec<&RomFile> = files.iter().filter(|f| !base_files.iter().any(|b| b.path == f.path)).collect();
        println!("nouveaux : {}", new.len());
        for f in files.iter().filter(|f| f.size > 0).take(3).chain(new.iter().copied().filter(|f| f.size > 0).take(3)) {
            let data = romfs.read(f, 0, 16).unwrap();
            println!("{} ({} o) : {:02X?}", f.path, f.size, data);
        }
    }

    /// `KALEIDO_NX_FILE=<jeu> KALEIDO_NX_UPDATE=<mise à jour> KALEIDO_NX_KEYS=<prod.keys> KALEIDO_NX_PATH=<chemin du romfs>`
    #[test]
    #[ignore]
    fn real_game_file() {
        let (Ok(file), Ok(update), Ok(keys), Ok(path)) = (std::env::var("KALEIDO_NX_FILE"), std::env::var("KALEIDO_NX_UPDATE"), std::env::var("KALEIDO_NX_KEYS"), std::env::var("KALEIDO_NX_PATH")) else { return };
        let keys = Keys::load(Path::new(&keys)).unwrap();
        let data = read_game_file(Path::new(&file), Some(Path::new(&update)), &keys, &path).unwrap();
        println!("{path} : {} octets, début {:02X?}", data.len(), &data[..16.min(data.len())]);
    }

    /// `KALEIDO_NX_FILE=<jeu ou mise à jour> KALEIDO_NX_KEYS=<prod.keys> cargo test -p kaleido-core real_build_id -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_build_id() {
        let (Ok(file), Ok(keys)) = (std::env::var("KALEIDO_NX_FILE"), std::env::var("KALEIDO_NX_KEYS")) else { return };
        let id = program_build_id(Path::new(&file), &Keys::load(Path::new(&keys)).unwrap()).unwrap();
        println!("build id : {}", build_id_hex(&id));
    }

    /// `KALEIDO_NX_FILES=<fichiers séparés par ;> cargo test -p kaleido-core real_missing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_missing() {
        let Ok(files) = std::env::var("KALEIDO_NX_FILES") else { return };
        for f in files.split(';') {
            println!("{f} : {:?}", missing_bytes(Path::new(f)).map(|m| format!("{} Mo manquants", m / 1024 / 1024)));
        }
    }

    #[test]
    fn truncated_container() {
        // PFS0 d'un seul fichier de 0x100 octets, coupé avant la fin.
        let mut nsp = b"PFS0".to_vec();
        nsp.extend_from_slice(&1u32.to_le_bytes());
        nsp.extend_from_slice(&8u32.to_le_bytes());
        nsp.extend_from_slice(&0u32.to_le_bytes());
        nsp.extend_from_slice(&0u64.to_le_bytes());
        nsp.extend_from_slice(&0x100u64.to_le_bytes());
        nsp.extend_from_slice(&0u32.to_le_bytes());
        nsp.extend_from_slice(&0u32.to_le_bytes());
        nsp.extend_from_slice(b"a.nca\0\0\0");
        let header = nsp.len() as u64;
        let dir = std::env::temp_dir().join(format!("kaleido-trunc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.nsp");
        let mut full = nsp.clone();
        full.resize(nsp.len() + 0x100, 0);
        std::fs::write(&path, &full).unwrap();
        assert_eq!(missing_bytes(&path), None);
        std::fs::write(&path, &full[..full.len() - 0x40]).unwrap();
        assert_eq!(missing_bytes(&path), Some(0x40));
        let _ = header;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nand_paths_and_cnmt() {
        // Dossiers créés par Eden chez l'utilisateur pour la mise à jour d'Épée.
        assert_eq!(registered_path(&parse_hex16("f13bc7b678b5c86998ff4e847e180142").unwrap()), "00000005/f13bc7b678b5c86998ff4e847e180142.nca");
        assert_eq!(registered_path(&parse_hex16("fe093f2da598b8c24110d19133c4a6d2").unwrap()), "000000D5/fe093f2da598b8c24110d19133c4a6d2.nca");
        let mut d = vec![0u8; 0x20];
        d[..8].copy_from_slice(&0x0100ABF008968800u64.to_le_bytes());
        d[8..12].copy_from_slice(&0x60000u32.to_le_bytes());
        d[0x0E] = 0x10;
        d[0x10] = 2;
        d.extend_from_slice(&[0u8; 0x10]);
        for (i, kind) in [(1u8, 1u8), (2, 6)] {
            let mut rec = vec![0u8; 0x38];
            rec[0x20] = i;
            rec[0x36] = kind;
            d.extend_from_slice(&rec);
        }
        let c = Cnmt::parse(&d).unwrap();
        assert_eq!((c.title_id, c.version, c.contents.len()), (0x0100ABF008968800, 0x60000, 2));
        assert_eq!(c.contents[1].1, 6);
        assert_eq!(c.contents[0].0[0], 1);
    }

    #[test]
    fn patch_build_ids() {
        let mut id = [0u8; 32];
        id[..20].copy_from_slice(&[0xAE, 0xE8, 0xF1, 0x50, 0xDD, 0xA1, 0xB5, 0xA8, 0x38, 0x80, 0x6E, 0x1A, 0x5E, 0xA6, 0x82, 0x7A, 0xD9, 0xF3, 0xC5, 0x1E]);
        let game = build_id_hex(&id);
        assert_eq!(game, "AEE8F150DDA1B5A838806E1A5EA6827AD9F3C51E");
        let pchtxt = b"@nsobid-AEE8F150DDA1B5A838806E1A5EA6827AD9F3C51E\n\n# Pokemon Legends: Arceus v1.1.1 - 60FPS\n@enabled\n";
        let p = patch_build_id("1.1.1.pchtxt", pchtxt).unwrap();
        assert!(build_id_matches(&p, &game));
        assert!(!build_id_matches("0123456789ABCDEF0123", &game));
        // Nom d'un .ips : 64 chiffres avec zéros de fin, ou raccourci.
        assert!(build_id_matches(&patch_build_id("AEE8F150DDA1B5A838806E1A5EA6827AD9F3C51E000000000000000000000000.ips", b"").unwrap(), &game));
        assert!(build_id_matches(&patch_build_id("aee8f150dda1b5a8.ips", b"").unwrap(), &game));
        assert_eq!(patch_build_id("readme.txt", b"@nsobid-AEE8"), None);
        assert_eq!(patch_build_id("1.0.0.pchtxt", b"# pas de build id"), None);
    }

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

