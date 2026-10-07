//! Lecture de la mémoire de l'émulateur, en direct, pour le compagnon de partie.
//!
//! Le compagnon lit la sauvegarde à chaque sauvegarde en jeu ; ce module lui permet de voir
//! la partie entre deux sauvegardes : PV, statuts, Pokémon adverse dès l'entrée en combat.
//!
//! **Lecture seule stricte.** Le processus de l'émulateur est ouvert avec les seuls droits
//! `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ` : aucune écriture, aucune injection, aucun
//! script Lua ni débogueur. Si l'émulateur n'est pas reconnu, ou le jeu pas cartographié, le
//! compagnon retombe sur la sauvegarde sans erreur.
//!
//! Étapes :
//! 1. [`MemorySource`] : lire des octets à une adresse d'un processus (ou d'un dump figé).
//! 2. [`scan`] : trouver la RAM émulée par une signature propre à la console (en-tête de
//!    cartouche recopié en RAM principale sur DS ; bloc dresseur de la sauvegarde sur 3DS).
//! 3. [`reader`] : trouver l'équipe en RAM (Pokémon chiffrés au format PK4 à PK7, reconnus par
//!    leur PID ou constante de chiffrement lus dans la sauvegarde, puis validés par leur somme
//!    de contrôle) et l'équipe adverse en combat.
//! 4. [`maps`] : cartes mémoire par jeu (`memory_maps.json`), pour ce que la recherche par
//!    signature ne trouve pas seule.

pub mod maps;
pub mod reader;
pub mod scan;
#[cfg(windows)]
pub mod windows;

use std::fmt;

/// Zone de mémoire lisible d'un processus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub base: u64,
    pub size: u64,
    /// Début de l'allocation qui contient la zone (`AllocationBase`) : un tampon alloué d'un
    /// seul bloc (RAM émulée) peut être découpé en plusieurs zones de protections différentes.
    pub allocation: u64,
}

impl Region {
    pub fn end(&self) -> u64 {
        self.base + self.size
    }

    pub fn contains(&self, addr: u64, len: u64) -> bool {
        addr >= self.base && addr.saturating_add(len) <= self.end()
    }
}

#[derive(Debug)]
pub enum LiveError {
    /// Le processus s'est terminé (ou ne peut plus être lu).
    Gone,
    /// Ouverture refusée (droits insuffisants, processus protégé).
    Denied(String),
    /// Lecture impossible à cette adresse.
    Read { addr: u64, len: usize },
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiveError::Gone => f.write_str("l'émulateur s'est fermé"),
            LiveError::Denied(e) => write!(f, "lecture de la mémoire refusée : {e}"),
            LiveError::Read { addr, len } => write!(f, "lecture impossible : {len} octets à {addr:#x}"),
        }
    }
}

impl std::error::Error for LiveError {}

/// Mémoire lisible : un processus vivant, ou un dump figé pour les tests.
pub trait MemorySource {
    /// Remplit `buf` avec la mémoire à partir de `addr`. Échoue si une partie est illisible.
    fn read(&self, addr: u64, buf: &mut [u8]) -> Result<(), LiveError>;

    /// Zones lisibles (validées et accessibles en lecture), par adresse croissante.
    fn regions(&self) -> Vec<Region>;

    /// Le processus tourne-t-il encore ?
    fn alive(&self) -> bool {
        true
    }

    fn read_vec(&self, addr: u64, len: usize) -> Result<Vec<u8>, LiveError> {
        let mut v = vec![0; len];
        self.read(addr, &mut v)?;
        Ok(v)
    }

    fn read_u32(&self, addr: u64) -> Result<u32, LiveError> {
        let mut b = [0; 4];
        self.read(addr, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }
}

/// Mémoire figée : une ou plusieurs zones copiées depuis un émulateur (tests, diagnostics).
#[derive(Debug, Clone, Default)]
pub struct DumpSource {
    zones: Vec<(Region, Vec<u8>)>,
}

impl DumpSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une zone de mémoire à l'adresse `base` (zones triées, sans chevauchement).
    pub fn with_zone(mut self, base: u64, bytes: Vec<u8>) -> Self {
        let region = Region { base, size: bytes.len() as u64, allocation: base };
        self.zones.push((region, bytes));
        self.zones.sort_by_key(|(r, _)| r.base);
        self
    }

    /// Dump creux : `KLDRAM1\0`, adresse de base (`u64`), taille totale (`u32`), puis des morceaux
    /// `[offset u32][longueur u32][octets]`. Le reste de la zone vaut zéro. Sert à figer dans les
    /// tests quelques Kio utiles d'une RAM de 4 Mio.
    pub fn from_sparse(bytes: &[u8]) -> Option<Self> {
        let rd32 = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if bytes.get(..8)? != b"KLDRAM1\0" {
            return None;
        }
        let base = u64::from_le_bytes(bytes.get(8..16)?.try_into().ok()?);
        let size = rd32(16)? as usize;
        let mut data = vec![0u8; size];
        let mut at = 20;
        while at < bytes.len() {
            let (ofs, len) = (rd32(at)? as usize, rd32(at + 4)? as usize);
            data.get_mut(ofs..ofs + len)?.copy_from_slice(bytes.get(at + 8..at + 8 + len)?);
            at += 8 + len;
        }
        Some(Self::new().with_zone(base, data))
    }

    /// Écrit un dump creux (voir [`Self::from_sparse`]) de `size` octets à `base`, en ne gardant
    /// que les morceaux `keep` (offset, longueur).
    pub fn to_sparse(src: &dyn MemorySource, base: u64, size: u32, keep: &[(u32, u32)]) -> Result<Vec<u8>, LiveError> {
        let mut out = b"KLDRAM1\0".to_vec();
        out.extend_from_slice(&base.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        for &(ofs, len) in keep {
            let len = len.min(size.saturating_sub(ofs));
            out.extend_from_slice(&ofs.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&src.read_vec(base + ofs as u64, len as usize)?);
        }
        Ok(out)
    }

    /// Modifie la mémoire figée (simule le jeu qui avance, dans les tests).
    pub fn poke(&mut self, addr: u64, bytes: &[u8]) {
        for (r, data) in &mut self.zones {
            if r.contains(addr, bytes.len() as u64) {
                let at = (addr - r.base) as usize;
                data[at..at + bytes.len()].copy_from_slice(bytes);
                return;
            }
        }
        panic!("adresse {addr:#x} hors du dump");
    }
}

impl MemorySource for DumpSource {
    fn read(&self, addr: u64, buf: &mut [u8]) -> Result<(), LiveError> {
        for (r, data) in &self.zones {
            if r.contains(addr, buf.len() as u64) {
                let at = (addr - r.base) as usize;
                buf.copy_from_slice(&data[at..at + buf.len()]);
                return Ok(());
            }
        }
        Err(LiveError::Read { addr, len: buf.len() })
    }

    fn regions(&self) -> Vec<Region> {
        self.zones.iter().map(|(r, _)| *r).collect()
    }
}

#[cfg(test)]
mod tests;
