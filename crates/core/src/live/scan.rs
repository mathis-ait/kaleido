//! Recherche de la RAM émulée dans la mémoire de l'émulateur.
//!
//! **DS** : au démarrage, l'en-tête de la cartouche (titre sur 12 octets, code jeu sur 4,
//! code éditeur « 01 ») est recopié en RAM principale à `0x027FFE00`. La RAM principale fait
//! 4 Mio (adresses `0x02000000` à `0x023FFFFF`, répétées jusqu'à `0x02FFFFFF`) : l'en-tête est
//! donc à l'offset `0x3FFE00` du tampon qui la contient, chez melonDS comme chez DeSmuME. Une
//! copie de la ROM chargée en mémoire a le même en-tête, mais au début de son allocation : elle
//! est écartée parce que la RAM supposée commencerait avant.
//!
//! **3DS** : la FCRAM (128 ou 256 Mio) ne contient pas d'en-tête reconnaissable ; on y cherche
//! directement les Pokémon de l'équipe (voir [`super::reader`]), dans les grandes zones seulement.

use super::{MemorySource, Region};

/// Taille de la RAM principale DS.
pub const DS_RAM_SIZE: u64 = 0x40_0000;
/// Offset de la copie de l'en-tête de cartouche dans la RAM principale DS (`0x027FFE00`).
pub const DS_HEADER_OFFSET: u64 = 0x3F_FE00;
/// RAM principale en mode DSi (16 Mio) : en-tête recopié en `0x02FFFE00`.
pub const DSI_RAM_SIZE: u64 = 0x100_0000;
pub const DSI_HEADER_OFFSET: u64 = 0xFF_FE00;
/// Formats essayés : DS, puis DSi (Noir / Blanc et Noir 2 / Blanc 2 lancés en console DSi).
const LAYOUTS: [(u64, u64); 2] = [(DS_RAM_SIZE, DS_HEADER_OFFSET), (DSI_RAM_SIZE, DSI_HEADER_OFFSET)];
/// Adresse de la RAM principale vue par le jeu.
pub const DS_RAM_ADDRESS: u32 = 0x0200_0000;

/// Lecture par morceaux : assez gros pour peu d'appels système, assez petit pour la mémoire.
const CHUNK: usize = 4 << 20;

/// RAM principale DS trouvée dans l'émulateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsRam {
    /// Adresse, dans l'émulateur, de l'octet `0x02000000` de la DS.
    pub base: u64,
    /// Code jeu complet (`CPUF`, `IPGF`, `IRAF`…).
    pub game_code: String,
    /// Titre interne (`POKEMON PL`).
    pub title: String,
    /// Taille de la RAM principale : 4 Mio (DS) ou 16 Mio (DSi).
    pub size: u64,
}

impl DsRam {
    /// Adresse dans l'émulateur d'une adresse DS (`0x02xxxxxx`, miroirs compris).
    pub fn host(&self, ds_address: u32) -> u64 {
        self.base + (ds_address as u64 & (self.size - 1))
    }
}

/// Toutes les occurrences de `needle` dans les zones retenues par `keep`.
pub fn find_all(src: &dyn MemorySource, regions: &[Region], needle: &[u8], align: usize, max: usize) -> Vec<u64> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut buf = vec![0u8; CHUNK + needle.len()];
    for r in regions {
        let mut at = r.base;
        while at < r.end() {
            let len = ((r.end() - at) as usize).min(CHUNK + needle.len() - 1);
            let chunk = &mut buf[..len];
            if src.read(at, chunk).is_err() {
                at += CHUNK as u64;
                continue;
            }
            let step = align.max(1);
            let mut i = ((step as u64 - at % step as u64) % step as u64) as usize;
            while i + needle.len() <= chunk.len() {
                if chunk[i] == needle[0] && &chunk[i..i + needle.len()] == needle && i < CHUNK {
                    out.push(at + i as u64);
                    if out.len() >= max {
                        return out;
                    }
                }
                i += step;
            }
            at += CHUNK as u64;
        }
    }
    out.dedup();
    out
}

/// Mots `u32` alignés égaux à l'une des `keys` : une seule lecture des zones pour toutes les clés
/// (la FCRAM 3DS fait 128 à 256 Mio). Renvoie (adresse, clé).
pub fn find_u32s(src: &dyn MemorySource, regions: &[Region], keys: &[u32], max: usize) -> Vec<(u64, u32)> {
    let mut out = Vec::new();
    let keys: Vec<u32> = keys.iter().copied().filter(|&k| k != 0).collect();
    if keys.is_empty() {
        return out;
    }
    let mut buf = vec![0u8; CHUNK];
    for r in regions {
        let mut at = r.base & !3;
        while at < r.end() {
            let len = ((r.end() - at) as usize).min(CHUNK) & !3;
            if len == 0 {
                break;
            }
            let chunk = &mut buf[..len];
            if src.read(at, chunk).is_ok() {
                for (i, w) in chunk.as_chunks::<4>().0.iter().enumerate() {
                    let v = u32::from_le_bytes(*w);
                    if keys.contains(&v) {
                        out.push((at + 4 * i as u64, v));
                        if out.len() >= max {
                            return out;
                        }
                    }
                }
            }
            at += len as u64;
        }
    }
    out
}

fn ascii_upper_alnum(b: &[u8]) -> bool {
    b.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Lit un en-tête de cartouche DS à `at` : (titre, code jeu) si c'en est un.
pub fn ds_header_at(src: &dyn MemorySource, at: u64) -> Option<(String, String)> {
    let mut h = [0u8; 0x20];
    src.read(at, &mut h).ok()?;
    let title_len = h[..12].iter().position(|&c| c == 0).unwrap_or(12);
    let title = &h[..title_len];
    let code = &h[0xC..0x10];
    let printable = title.iter().all(|c| (0x20..0x7F).contains(c));
    (title_len >= 4 && printable && ascii_upper_alnum(code) && &h[0x10..0x12] == b"01")
        .then(|| (String::from_utf8_lossy(title).into_owned(), String::from_utf8_lossy(code).into_owned()))
}

/// Cherche la RAM principale DS d'un jeu Pokémon (titre interne « POKEMON … »).
pub fn find_ds_ram(src: &dyn MemorySource) -> Option<DsRam> {
    let regions: Vec<Region> = src.regions().into_iter().filter(|r| r.size >= DS_RAM_SIZE).collect();
    // Chemin rapide : RAM au début d'une zone ou d'une allocation (melonDS). Quelques centaines
    // de petites lectures au lieu de parcourir des centaines de Mio.
    for r in &regions {
        for base in [r.allocation, r.base] {
            for (size, header) in LAYOUTS {
                if !r.contains(base, size) {
                    continue;
                }
                if let Some((title, game_code)) = ds_header_at(src, base + header).filter(|(t, _)| t.starts_with("POKEMON ")) {
                    return Some(DsRam { base, game_code, title, size });
                }
            }
        }
    }
    let mut best: Option<(u8, DsRam)> = None;
    for hit in find_all(src, &regions, b"POKEMON ", 4, 64) {
        let Some((title, game_code)) = ds_header_at(src, hit) else { continue };
        for (size, header) in LAYOUTS {
            let Some(base) = hit.checked_sub(header) else { continue };
            let Some(region) = regions.iter().find(|r| r.contains(base, size)) else { continue };
            // Préférence : RAM au tout début d'une allocation (tampon dédié, melonDS), sinon au milieu
            // d'une zone de données (DeSmuME, RAM dans une variable globale).
            let score = if region.allocation == base || region.base == base { 2 } else { 1 };
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, DsRam { base, game_code: game_code.clone(), title: title.clone(), size }));
            }
        }
    }
    best.map(|(_, r)| r)
}

/// Zones assez grandes pour contenir la FCRAM 3DS (128 Mio au moins), ou toutes les zones
/// d'au moins 1 Mio si aucune ne l'est (émulateur qui découpe la FCRAM).
pub fn ctr_candidate_regions(src: &dyn MemorySource) -> Vec<Region> {
    let all = src.regions();
    let big: Vec<Region> = all.iter().copied().filter(|r| r.size >= 64 << 20).collect();
    if big.is_empty() {
        all.into_iter().filter(|r| r.size >= 1 << 20).collect()
    } else {
        big
    }
}
