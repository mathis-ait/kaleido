//! ROM Game Boy et Game Boy Color : en-tête, sommes de contrôle, banques.
//!
//! En-tête (Pan Docs, « The Cartridge Header ») :
//!
//! | Offset | Contenu |
//! |---|---|
//! | 0x104 | logo Nintendo (48 octets) |
//! | 0x134 | titre ASCII (16 octets en GB, 11 + code fabricant 0x13F..0x142 en GBC) |
//! | 0x143 | drapeau GBC (0x80 compatible, 0xC0 GBC seulement) |
//! | 0x147 | type de cartouche (MBC), 0x148 taille de la ROM, 0x149 taille de la RAM |
//! | 0x14A | destination (0 Japon, 1 ailleurs) |
//! | 0x14C | version |
//! | 0x14D | somme de l'en-tête : `x = x - octet - 1` sur 0x134..=0x14C |
//! | 0x14E | somme globale (u16 gros-boutiste) de tous les octets sauf ces deux |
//!
//! La ROM est découpée en banques de 16 Kio : un pointeur 16 bits ≥ 0x4000 désigne une
//! adresse dans la banque commutable courante (`AbstractGBCRomHandler.calculateOffset`
//! de l'Universal Pokémon Randomizer).

use std::io::{Read, Seek, SeekFrom};

use crate::{FormatError, Result};

pub const HEADER_END: usize = 0x150;
pub const BANK_SIZE: usize = 0x4000;
const SIGNATURE_MAGIC: &[u8; 8] = b"KALEIDO1";
/// La signature (code de partage compris) est cherchée dans les derniers 8 Kio.
const SIGNATURE_WINDOW: usize = 0x2000;

/// Les 8 premiers octets du logo Nintendo (suffisants pour reconnaître une cartouche).
const LOGO_START: [u8; 8] = [0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbHeader {
    /// Titre (11 caractères pour les jeux GBC qui ont un code fabricant).
    pub title: String,
    /// Code fabricant GBC (`BYTE` pour Cristal), vide pour un jeu GB.
    pub code: String,
    pub cgb: bool,
    pub version: u8,
    /// Destination : `true` hors du Japon.
    pub non_japanese: bool,
    pub header_checksum_ok: bool,
    /// Somme globale enregistrée dans l'en-tête.
    pub global_checksum: u16,
}

/// Somme de l'en-tête (0x14D).
pub fn header_checksum(d: &[u8]) -> u8 {
    d[0x134..=0x14C].iter().fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1))
}

/// Somme globale (0x14E) : tous les octets sauf ceux de la somme elle-même.
pub fn global_checksum(d: &[u8]) -> u16 {
    d.iter().enumerate().filter(|&(i, _)| i != 0x14E && i != 0x14F).fold(0u16, |s, (_, &b)| s.wrapping_add(b as u16))
}

fn ascii(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '?' }).collect::<String>().trim_end().to_string()
}

impl GbHeader {
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < HEADER_END || d[0x104..0x10C] != LOGO_START {
            return None;
        }
        let cgb = d[0x143] & 0x80 != 0;
        let code = &d[0x13F..0x143];
        let has_code = cgb && code.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        let title = if has_code { ascii(&d[0x134..0x13F]) } else { ascii(&d[0x134..0x144]) };
        Some(Self {
            title,
            code: if has_code { ascii(code) } else { String::new() },
            cgb,
            version: d[0x14C],
            non_japanese: d[0x14A] != 0,
            header_checksum_ok: header_checksum(d) == d[0x14D],
            global_checksum: u16::from_be_bytes([d[0x14E], d[0x14F]]),
        })
    }

    pub fn probe<R: Read + Seek>(r: &mut R) -> Result<Option<Self>> {
        r.seek(SeekFrom::Start(0))?;
        let mut h = vec![0u8; HEADER_END];
        let mut read = 0;
        while read < HEADER_END {
            match r.read(&mut h[read..])? {
                0 => return Ok(None),
                n => read += n,
            }
        }
        Ok(Self::parse(&h))
    }

    /// Titre complet tel que comparé par l'UPR (`romSig` sur 0x134).
    pub fn raw_title(d: &[u8]) -> String {
        ascii(&d[0x134..0x144])
    }
}

/// Adresse d'un pointeur de banque → offset de fichier.
pub fn bank_offset(bank: usize, pointer: u16) -> usize {
    let p = pointer as usize;
    if p < BANK_SIZE {
        p
    } else {
        (p % BANK_SIZE) + bank * BANK_SIZE
    }
}

/// Offset de fichier → pointeur 16 bits dans sa banque.
pub fn offset_pointer(offset: usize) -> u16 {
    if offset < BANK_SIZE {
        offset as u16
    } else {
        ((offset % BANK_SIZE) + BANK_SIZE) as u16
    }
}

/// ROM GB / GBC chargée en mémoire (1 à 2 Mio pour les jeux Pokémon).
#[derive(Debug, Clone)]
pub struct GbRom {
    data: Vec<u8>,
    header: GbHeader,
}

impl GbRom {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let header = GbHeader::parse(&data).ok_or(FormatError::Invalid("ce n'est pas une ROM Game Boy"))?;
        Ok(Self { data, header })
    }

    pub fn open(path: &std::path::Path) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn header(&self) -> &GbHeader {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn u8(&self, at: usize) -> Option<u8> {
        self.data.get(at).copied()
    }

    pub fn u16(&self, at: usize) -> Option<u16> {
        self.data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn bytes(&self, at: usize, len: usize) -> Option<&[u8]> {
        self.data.get(at..at.checked_add(len)?)
    }

    pub fn write(&mut self, at: usize, bytes: &[u8]) -> Result<()> {
        let end = at.checked_add(bytes.len()).filter(|&e| e <= self.data.len()).ok_or(FormatError::Invalid("écriture hors de la ROM"))?;
        self.data[at..end].copy_from_slice(bytes);
        Ok(())
    }

    pub fn write_u8(&mut self, at: usize, v: u8) -> Result<()> {
        self.write(at, &[v])
    }

    pub fn write_u16(&mut self, at: usize, v: u16) -> Result<()> {
        self.write(at, &v.to_le_bytes())
    }

    /// Pointeur 16 bits lu en `at`, résolu dans la banque de `at` (ou `bank`).
    pub fn pointer_in(&self, at: usize, bank: usize) -> Option<usize> {
        self.u16(at).map(|p| bank_offset(bank, p))
    }

    /// Signature Kaleido, si présente (dans la zone libre de la dernière banque).
    pub fn signature(&self) -> Option<&[u8]> {
        find_signature(&self.data)
    }

    /// Pose la signature dans les derniers octets libres (0x00 ou 0xFF) de la ROM. Les ROM
    /// GB sont pleines : `false` si la place manque (la ROM reste alors sans signature).
    pub fn set_signature(&mut self, content: &[u8]) -> bool {
        let len = 12 + content.len();
        let end = self.data.len();
        // Octet de remplissage : celui qui précède l'ancienne signature, sinon le dernier.
        let mut fill = self.data[end - 1];
        if let Some(at) = find_signature_at(&self.data) {
            fill = self.data[at.saturating_sub(1)];
            self.data[at..end].fill(fill);
        }
        if len + 16 > end {
            return false;
        }
        let at = end - len;
        if !matches!(fill, 0x00 | 0xFF) || self.data[at - 16..end].iter().any(|&b| b != fill) {
            return false;
        }
        self.data[at..at + 8].copy_from_slice(SIGNATURE_MAGIC);
        self.data[at + 8..at + 12].copy_from_slice(&(content.len() as u32).to_le_bytes());
        self.data[at + 12..end].copy_from_slice(content);
        true
    }

    /// Image finale : somme de l'en-tête recalculée (vérifiée par la console au démarrage).
    /// La somme globale (0x14E) est laissée d'origine : aucun jeu ni émulateur courant ne la
    /// contrôle, et c'est elle qui distingue les versions de même titre (Rouge français ou
    /// américain) dans le fichier d'offsets : une ROM randomisée reste donc reconnue.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        out[0x14D] = header_checksum(&out);
        out
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        std::fs::write(path, self.to_bytes())?;
        Ok(())
    }
}

fn find_signature_at(data: &[u8]) -> Option<usize> {
    let window = data.len().saturating_sub(SIGNATURE_WINDOW);
    (window..data.len().saturating_sub(12)).rev().find(|&at| data[at..].starts_with(SIGNATURE_MAGIC)).filter(|&at| {
        let size = u32::from_le_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize;
        at + 12 + size == data.len()
    })
}

fn find_signature(data: &[u8]) -> Option<&[u8]> {
    find_signature_at(data).map(|at| &data[at + 12..])
}

/// Signature lue depuis un flux (fin du fichier).
pub fn read_signature<R: Read + Seek>(r: &mut R) -> Result<Option<Vec<u8>>> {
    let len = crate::stream_len(r)? as usize;
    let start = len.saturating_sub(SIGNATURE_WINDOW);
    r.seek(SeekFrom::Start(start as u64))?;
    let mut tail = Vec::new();
    r.take((len - start) as u64).read_to_end(&mut tail)?;
    Ok(find_signature(&tail).map(<[u8]>::to_vec))
}

/// En-tête synthétique valide (tests) : logo, titre, code GBC éventuel, version.
pub fn synthetic(title: &str, code: &str, version: u8, size: usize) -> Vec<u8> {
    let mut d = vec![0u8; size];
    d[0x104..0x10C].copy_from_slice(&LOGO_START);
    for (i, b) in title.bytes().take(16).enumerate() {
        d[0x134 + i] = b;
    }
    if !code.is_empty() {
        d[0x13F..0x143].copy_from_slice(&code.as_bytes()[..4]);
        d[0x143] = 0x80;
    }
    d[0x14A] = 1;
    d[0x14C] = version;
    d[0x14D] = header_checksum(&d);
    let sum = global_checksum(&d);
    d[0x14E..0x150].copy_from_slice(&sum.to_be_bytes());
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_checksums() {
        let d = synthetic("POKEMON RED", "", 0, 0x10000);
        let h = GbHeader::parse(&d).unwrap();
        assert_eq!((h.title.as_str(), h.code.as_str(), h.cgb, h.version), ("POKEMON RED", "", false, 0));
        assert!(h.header_checksum_ok && h.non_japanese);
        assert_eq!(h.global_checksum, global_checksum(&d));
        let c = synthetic("PM_CRYSTAL", "BYTF", 0, 0x10000);
        let h = GbHeader::parse(&c).unwrap();
        assert_eq!((h.title.as_str(), h.code.as_str(), h.cgb), ("PM_CRYSTAL", "BYTF", true));
        assert!(GbHeader::parse(&vec![0u8; 0x200]).is_none());
    }

    #[test]
    fn banks_and_signature() {
        assert_eq!(bank_offset(0x0E, 0x4000), 0x38000);
        assert_eq!(bank_offset(0x0E, 0x1234), 0x1234);
        assert_eq!(offset_pointer(0x383DE), 0x43DE);
        let mut g = GbRom::from_bytes(synthetic("POKEMON BLUE", "", 0, 0x10000)).unwrap();
        assert!(g.signature().is_none());
        assert!(g.set_signature(b"{\"seed\":3}"));
        assert_eq!(g.signature(), Some(&b"{\"seed\":3}"[..]));
        assert!(g.set_signature(b"{\"seed\":44}"));
        assert_eq!(g.signature(), Some(&b"{\"seed\":44}"[..]));
        let bytes = g.to_bytes();
        let h = GbHeader::parse(&bytes).unwrap();
        assert!(h.header_checksum_ok);
        // Somme globale d'origine conservée.
        assert_eq!(h.global_checksum, GbHeader::parse(&synthetic("POKEMON BLUE", "", 0, 0x10000)).unwrap().global_checksum);
        assert_eq!(read_signature(&mut std::io::Cursor::new(&bytes)).unwrap().unwrap(), b"{\"seed\":44}");
        // ROM pleine : pas de signature.
        let mut full = synthetic("POKEMON BLUE", "", 0, 0x8000);
        full[0x7F00..].fill(0x37);
        assert!(!GbRom::from_bytes(full).unwrap().set_signature(b"x"));
    }
}
