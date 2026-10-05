//! Archives GARC des jeux Pokémon 3DS (fichiers `a/x/y/z` du RomFS).
//!
//! Structure : en-tête `CRAG`, puis les sections `OTAF` (FATO : position de
//! chaque entrée dans la FATB), `BTAF` (FATB : pour chaque entrée, un champ de
//! bits des sous-fichiers présents puis `début, fin, longueur` de chacun ; son
//! compteur est le nombre total de sous-fichiers) et `BMIF` (FIMB : les données).
//!
//! Version `0x0400` pour X/Y et ROSA (vérifié : les 298 GARC de Rubis Oméga sont
//! reconstruits à l'octet près), `0x0600` pour la Gen 7 (d'après pk3DS, non vérifié).
//! Chaque sous-fichier est aligné (4 octets en v4, valeur de l'en-tête en v6) ;
//! `fin` inclut le remplissage, `longueur` non. L'en-tête v4 donne la taille du plus
//! grand sous-fichier sans remplissage.

use crate::util::{slice, u16le, u32le};
use crate::{FormatError, Result};

/// X/Y et Rubis Oméga / Saphir Alpha.
pub const VERSION_4: u16 = 0x0400;
/// Soleil/Lune et Ultra-Soleil/Ultra-Lune.
pub const VERSION_6: u16 = 0x0600;

const FATO_HEADER: u32 = 0x0C;
const FATB_HEADER: u32 = 0x0C;
const FIMB_HEADER: u32 = 0x0C;

/// Une entrée de l'archive : jusqu'à 32 sous-fichiers, repérés par leur bit.
/// Presque toujours un seul sous-fichier, au bit 0.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GarcEntry {
    /// `(bit, contenu)`, dans l'ordre croissant des bits.
    pub subfiles: Vec<(u8, Vec<u8>)>,
}

impl GarcEntry {
    pub fn single(data: Vec<u8>) -> Self {
        Self { subfiles: vec![(0, data)] }
    }

    /// Premier sous-fichier (le seul dans la plupart des archives).
    pub fn data(&self) -> Option<&[u8]> {
        self.subfiles.first().map(|(_, d)| d.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Garc {
    pub version: u16,
    pub entries: Vec<GarcEntry>,
    /// Alignement des données de chaque sous-fichier.
    alignment: u32,
    /// Octet de remplissage entre les sous-fichiers.
    pad_byte: u8,
}

impl Garc {
    pub fn is_garc(data: &[u8]) -> bool {
        data.starts_with(b"CRAG")
    }

    /// Nouvelle archive vide (alignement 4, remplissage `0xFF`).
    pub fn new(version: u16) -> Self {
        Self { version, entries: Vec::new(), alignment: 4, pad_byte: 0xFF }
    }

    pub fn parse(d: &[u8]) -> Result<Self> {
        const BAD: FormatError = FormatError::Invalid("entrée GARC incohérente");
        if !Self::is_garc(d) {
            return Err(FormatError::Invalid("ce n'est pas une archive GARC"));
        }
        let h = slice(d, 0, 0x1C)?;
        let header_size = u32le(h, 0x04);
        let version = u16le(h, 0x0A);
        let data_offset = u32le(h, 0x10);
        let alignment = match version {
            VERSION_4 => 4,
            VERSION_6 => u32le(slice(d, 0, 0x24)?, 0x20),
            _ => return Err(FormatError::Invalid("version de GARC inconnue")),
        };
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(FormatError::Invalid("alignement GARC invalide"));
        }

        let fato = section(d, header_size, b"OTAF")?;
        let count = u16le(fato, 8) as u32;
        let fatb_at = header_size + fato.len() as u32;
        let fatb = section(d, fatb_at, b"BTAF")?;

        let fimb = section(d, fatb_at + fatb.len() as u32, b"BMIF")?;
        let data = slice(d, data_offset, u32le(fimb, 8))?;

        let mut entries = Vec::with_capacity(count as usize);
        let mut pad_byte = None;
        for c in slice(fato, FATO_HEADER, count * 4)?.as_chunks::<4>().0 {
            let at = FATB_HEADER.saturating_add(u32le(c, 0));
            let bits = u32le(slice(fatb, at, 4)?, 0);
            let mut subfiles = Vec::with_capacity(bits.count_ones() as usize);
            let mut sub_at = at + 4;
            for bit in (0..32u8).filter(|b| bits & (1 << b) != 0) {
                let e = slice(fatb, sub_at, 12)?;
                let (start, end, len) = (u32le(e, 0), u32le(e, 4), u32le(e, 8));
                if start > end || len > end - start {
                    return Err(BAD);
                }
                let content = slice(data, start, end - start)?;
                if pad_byte.is_none() && len < end - start {
                    pad_byte = Some(content[len as usize]);
                }
                subfiles.push((bit, content[..len as usize].to_vec()));
                sub_at += 12;
            }
            entries.push(GarcEntry { subfiles });
        }
        // La FATB compte les sous-fichiers, pas les entrées.
        if entries.iter().map(|e| e.subfiles.len()).sum::<usize>() != u32le(fatb, 8) as usize {
            return Err(FormatError::Invalid("FATO et FATB ne concordent pas"));
        }
        Ok(Self { version, entries, alignment, pad_byte: pad_byte.unwrap_or(0xFF) })
    }

    /// Archive où chaque entrée contient un seul fichier.
    pub fn from_files(version: u16, files: Vec<Vec<u8>>) -> Self {
        Self { entries: files.into_iter().map(GarcEntry::single).collect(), ..Self::new(version) }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Premier sous-fichier de l'entrée `index`.
    pub fn file(&self, index: usize) -> Option<&[u8]> {
        self.entries.get(index).and_then(GarcEntry::data)
    }

    /// Remplace le premier sous-fichier de l'entrée `index`.
    pub fn set_file(&mut self, index: usize, data: Vec<u8>) -> Result<()> {
        let entry = self.entries.get_mut(index).ok_or_else(|| FormatError::NotFound(format!("entrée GARC n°{index}")))?;
        match entry.subfiles.first_mut() {
            Some((_, d)) => *d = data,
            None => entry.subfiles.push((0, data)),
        }
        Ok(())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let header_size: u32 = if self.version == VERSION_4 { 0x1C } else { 0x24 };
        let align = |v: u32| v.div_ceil(self.alignment) * self.alignment;

        let mut fato = Vec::with_capacity(self.entries.len() * 4);
        let mut fatb = Vec::new();
        let mut data = Vec::new();
        let (mut largest_padded, mut largest) = (0u32, 0u32);
        for entry in &self.entries {
            fato.extend((fatb.len() as u32).to_le_bytes());
            let bits = entry.subfiles.iter().fold(0u32, |acc, (b, _)| acc | 1 << (b & 31));
            fatb.extend(bits.to_le_bytes());
            for (_, sub) in &entry.subfiles {
                let start = data.len() as u32;
                let len = sub.len() as u32;
                data.extend_from_slice(sub);
                data.resize(align(data.len() as u32) as usize, self.pad_byte);
                let end = data.len() as u32;
                fatb.extend(start.to_le_bytes());
                fatb.extend(end.to_le_bytes());
                fatb.extend(len.to_le_bytes());
                largest_padded = largest_padded.max(end - start);
                largest = largest.max(len);
            }
        }

        let fato_size = FATO_HEADER + fato.len() as u32;
        let fatb_size = FATB_HEADER + fatb.len() as u32;
        let data_offset = header_size + fato_size + fatb_size + FIMB_HEADER;
        let total = data_offset + data.len() as u32;

        let mut out = Vec::with_capacity(total as usize);
        out.extend(b"CRAG");
        out.extend(header_size.to_le_bytes());
        out.extend(0xFEFFu16.to_le_bytes());
        out.extend(self.version.to_le_bytes());
        out.extend(4u32.to_le_bytes());
        out.extend(data_offset.to_le_bytes());
        out.extend(total.to_le_bytes());
        if self.version == VERSION_4 {
            // v4 : taille du plus grand sous-fichier, sans remplissage (vérifié sur ROSA).
            out.extend(largest.to_le_bytes());
        } else {
            // v6 : avec puis sans remplissage, puis l'alignement.
            out.extend(largest_padded.to_le_bytes());
            out.extend(largest.to_le_bytes());
            out.extend(self.alignment.to_le_bytes());
        }

        out.extend(b"OTAF");
        out.extend(fato_size.to_le_bytes());
        out.extend((self.entries.len() as u16).to_le_bytes());
        out.extend(0xFFFFu16.to_le_bytes());
        out.extend(fato);

        out.extend(b"BTAF");
        out.extend(fatb_size.to_le_bytes());
        out.extend((self.entries.iter().map(|e| e.subfiles.len()).sum::<usize>() as u32).to_le_bytes());
        out.extend(fatb);

        out.extend(b"BMIF");
        out.extend(FIMB_HEADER.to_le_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
        out
    }
}

/// Section `magic` commençant à `at`, taille lue dans son en-tête.
fn section<'a>(d: &'a [u8], at: u32, magic: &[u8; 4]) -> Result<&'a [u8]> {
    let head = slice(d, at, 8)?;
    if &head[..4] != magic {
        return Err(FormatError::Invalid("section GARC manquante"));
    }
    slice(d, at, u32le(head, 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_both_versions() {
        for version in [VERSION_4, VERSION_6] {
            let mut garc = Garc::from_files(version, vec![b"abc".to_vec(), vec![], b"defgh".to_vec()]);
            garc.entries.push(GarcEntry { subfiles: vec![(1, b"x".to_vec()), (4, b"yz".to_vec())] });
            let bytes = garc.to_bytes();
            let parsed = Garc::parse(&bytes).unwrap();
            assert_eq!(parsed, garc);
            assert_eq!(parsed.to_bytes(), bytes);
            assert_eq!(parsed.file(2), Some(&b"defgh"[..]));
            assert_eq!(parsed.entries[3].subfiles[1], (4, b"yz".to_vec()));
        }
    }

    #[test]
    fn set_file_and_errors() {
        let mut garc = Garc::from_files(VERSION_6, vec![vec![1, 2]]);
        garc.set_file(0, vec![9; 7]).unwrap();
        assert!(garc.set_file(5, vec![]).is_err());
        assert_eq!(Garc::parse(&garc.to_bytes()).unwrap().file(0), Some(&[9u8; 7][..]));
        assert!(Garc::parse(b"NARC").is_err());
    }
}
