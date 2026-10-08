//! ROM Game Boy Advance : en-tête, lecture et écriture en mémoire, pointeurs.
//!
//! La GBA n'a pas de système de fichiers : la cartouche est une image plate
//! (8, 16 ou 32 Mio) projetée à l'adresse 0x0800_0000. Chaque table de données est
//! trouvée par un pointeur placé dans le code ou dans une autre table, comme le fait
//! l'Universal Pokémon Randomizer (UPR-ZX, GPLv3) avec ses fichiers d'offsets et ses
//! recherches par signature (`Gen3RomHandler.java`, `RomFunctions.search`).
//!
//! En-tête (GBATEK, « GBA Cartridge Header ») :
//!
//! | Offset | Taille | Contenu |
//! |---|---|---|
//! | 0x00 | 4 | instruction ARM de saut vers le code (`EA` en octet de poids fort) |
//! | 0xA0 | 12 | titre en majuscules ASCII |
//! | 0xAC | 4 | code jeu (`BPRE` : Rouge Feu américain) |
//! | 0xB0 | 2 | code éditeur (`01` : Nintendo) |
//! | 0xB2 | 1 | valeur fixe 0x96 |
//! | 0xBC | 1 | version du logiciel |
//! | 0xBD | 1 | complément de l'en-tête : `-(somme 0xA0..0xBC) - 0x19` |

use std::io::{Read, Seek, SeekFrom};

use crate::{FormatError, Result};

/// Taille de l'en-tête lu.
pub const HEADER_SIZE: usize = 0xC0;
/// Adresse de la cartouche dans l'espace mémoire de la GBA.
pub const ROM_BASE: u32 = 0x0800_0000;
/// Plus grande cartouche adressable (32 Mio).
pub const MAX_ROM_SIZE: usize = 0x0200_0000;
/// Octet des zones libres d'une ROM GBA.
pub const FREE_BYTE: u8 = 0xFF;

const SIGNATURE_MAGIC: &[u8; 8] = b"KALEIDO1";
/// La signature est cherchée dans les derniers 64 Kio de la ROM.
const SIGNATURE_WINDOW: usize = 0x10000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbaHeader {
    pub title: String,
    pub game_code: String,
    pub maker: String,
    pub version: u8,
    pub complement: u8,
    pub complement_ok: bool,
}

/// Complément de l'en-tête (octet 0xBD), calculé sur 0xA0..0xBD.
pub fn header_complement(h: &[u8]) -> u8 {
    let sum = h[0xA0..0xBD].iter().fold(0u8, |acc, &b| acc.wrapping_sub(b));
    sum.wrapping_sub(0x19)
}

fn ascii(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '?' }).collect::<String>().trim_end().to_string()
}

impl GbaHeader {
    /// Lit l'en-tête s'il ressemble à une cartouche GBA (saut ARM, octet fixe 0x96,
    /// code jeu alphanumérique).
    pub fn parse(h: &[u8]) -> Option<Self> {
        if h.len() < HEADER_SIZE || h[3] != 0xEA || h[0xB2] != 0x96 {
            return None;
        }
        let code = &h[0xAC..0xB0];
        if !code.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return None;
        }
        let complement = h[0xBD];
        Some(Self {
            title: ascii(&h[0xA0..0xAC]),
            game_code: ascii(code),
            maker: ascii(&h[0xB0..0xB2]),
            version: h[0xBC],
            complement,
            complement_ok: header_complement(h) == complement,
        })
    }

    /// Lit l'en-tête depuis un flux ; `None` si ce n'est pas une ROM GBA.
    pub fn probe<R: Read + Seek>(r: &mut R) -> Result<Option<Self>> {
        r.seek(SeekFrom::Start(0))?;
        let mut h = vec![0u8; HEADER_SIZE];
        let mut read = 0;
        while read < HEADER_SIZE {
            match r.read(&mut h[read..])? {
                0 => return Ok(None),
                n => read += n,
            }
        }
        Ok(Self::parse(&h))
    }

    /// Dernière lettre du code jeu : région et langue (`E` États-Unis, `F` France…).
    pub fn region(&self) -> Option<char> {
        self.game_code.chars().nth(3)
    }
}

/// Lit la signature Kaleido d'une ROM GBA (contenu du bloc, sans « KALEIDO1 » + taille).
pub fn read_signature<R: Read + Seek>(r: &mut R) -> Result<Option<Vec<u8>>> {
    let len = crate::stream_len(r)? as usize;
    let start = len.saturating_sub(SIGNATURE_WINDOW);
    r.seek(SeekFrom::Start(start as u64))?;
    let mut tail = Vec::with_capacity(len - start);
    r.take((len - start) as u64).read_to_end(&mut tail)?;
    Ok(find_signature(&tail).map(|(_, content)| content.to_vec()))
}

/// Cherche le bloc de signature (aligné sur 16 octets) ; renvoie sa position et son contenu.
fn find_signature(data: &[u8]) -> Option<(usize, &[u8])> {
    let mut at = data.len().saturating_sub(16) & !0xF;
    loop {
        if data[at..].starts_with(SIGNATURE_MAGIC) && at + 12 <= data.len() {
            let size = u32::from_le_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize;
            if let Some(content) = data.get(at + 12..at + 12 + size) {
                return Some((at, content));
            }
        }
        if at < 16 {
            return None;
        }
        at -= 16;
    }
}

/// ROM GBA chargée en mémoire (16 Mio pour les jeux Pokémon : on lit tout).
#[derive(Debug, Clone)]
pub struct GbaRom {
    data: Vec<u8>,
    header: GbaHeader,
}

impl GbaRom {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let header = GbaHeader::parse(&data).ok_or(FormatError::Invalid("ce n'est pas une ROM Game Boy Advance"))?;
        if data.len() > MAX_ROM_SIZE {
            return Err(FormatError::Invalid("ROM GBA de plus de 32 Mio"));
        }
        Ok(Self { data, header })
    }

    pub fn open(path: &std::path::Path) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn header(&self) -> &GbaHeader {
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

    /// Image finale, avec le complément de l'en-tête recalculé.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        out[0xBD] = header_complement(&out);
        out
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        std::fs::write(path, self.to_bytes())?;
        Ok(())
    }

    pub fn u8(&self, at: usize) -> Option<u8> {
        self.data.get(at).copied()
    }

    pub fn u16(&self, at: usize) -> Option<u16> {
        self.data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&self, at: usize) -> Option<u32> {
        self.data.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }

    pub fn bytes(&self, at: usize, len: usize) -> Option<&[u8]> {
        self.data.get(at..at.checked_add(len)?)
    }

    /// Écrit `bytes` en `at`. Erreur si la zone dépasse la ROM.
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

    pub fn write_u32(&mut self, at: usize, v: u32) -> Result<()> {
        self.write(at, &v.to_le_bytes())
    }

    /// Lit un pointeur (adresse 0x08xxxxxx ou 0x09xxxxxx) et le convertit en offset de fichier.
    pub fn pointer(&self, at: usize) -> Option<usize> {
        self.u32(at).and_then(|p| pointer_to_offset(p, self.data.len()))
    }

    /// Écrit un pointeur vers l'offset `target`.
    pub fn write_pointer(&mut self, at: usize, target: usize) -> Result<()> {
        self.write_u32(at, offset_to_pointer(target))
    }

    /// Toutes les positions de `pattern` (comme `RomFunctions.search`).
    pub fn find_all(&self, pattern: &[u8]) -> Vec<usize> {
        find_all(&self.data, pattern)
    }

    pub fn find(&self, pattern: &[u8]) -> Option<usize> {
        find_all(&self.data, pattern).into_iter().next()
    }

    /// Emplacements (alignés sur 4) des pointeurs vers `target`.
    pub fn pointers_to(&self, target: usize) -> Vec<usize> {
        let want = offset_to_pointer(target).to_le_bytes();
        (0..self.data.len().saturating_sub(3)).step_by(4).filter(|&at| self.data[at..at + 4] == want).collect()
    }

    /// Zone libre (octets 0xFF) de `len` octets, alignée sur `align`, à partir de `from`.
    /// Une marge de 16 octets libres est laissée avant la zone, pour ne pas coller à
    /// une donnée qui se terminerait par 0xFF.
    pub fn find_free_space(&self, len: usize, from: usize, align: usize) -> Option<usize> {
        let align = align.max(1);
        let mut at = from.div_ceil(align) * align + 16;
        let data = &self.data;
        while at + len <= data.len() {
            let margin = at.saturating_sub(16);
            match data[margin..at + len].iter().rposition(|&b| b != FREE_BYTE) {
                None => return Some(at),
                Some(bad) => at = (margin + bad + 1 + 16).div_ceil(align) * align,
            }
        }
        None
    }

    /// Agrandit la ROM (remplie de 0xFF) jusqu'à `size`, au plus 32 Mio.
    pub fn grow_to(&mut self, size: usize) -> Result<()> {
        if size > MAX_ROM_SIZE {
            return Err(FormatError::Invalid("une ROM GBA ne peut pas dépasser 32 Mio"));
        }
        if size > self.data.len() {
            self.data.resize(size, FREE_BYTE);
        }
        Ok(())
    }

    /// Signature lue dans la ROM, si elle en a une.
    pub fn signature(&self) -> Option<&[u8]> {
        let start = self.data.len().saturating_sub(SIGNATURE_WINDOW) & !0xF;
        find_signature(&self.data[start..]).map(|(_, c)| c)
    }

    /// Pose (ou remplace) la signature à la fin de la ROM, dans la zone libre finale.
    /// La ROM est agrandie si la fin n'est pas libre (jusqu'à 32 Mio).
    pub fn set_signature(&mut self, content: &[u8]) -> Result<()> {
        let window = self.data.len().saturating_sub(SIGNATURE_WINDOW) & !0xF;
        if let Some((at, c)) = find_signature(&self.data[window..]) {
            let (at, len) = (window + at, 12 + c.len());
            self.data[at..at + len].fill(FREE_BYTE);
        }
        let block_len = (12 + content.len()).div_ceil(16) * 16;
        if block_len > SIGNATURE_WINDOW / 2 {
            return Err(FormatError::Invalid("signature trop longue"));
        }
        let end = self.data.len();
        let at = (end - block_len) & !0xF;
        let free = at >= 16 && self.data[at - 16..end].iter().all(|&b| b == FREE_BYTE);
        let at = if free {
            at
        } else {
            let grown = (end + block_len + 16).div_ceil(0x10000) * 0x10000;
            self.grow_to(grown)?;
            (grown - block_len) & !0xF
        };
        self.data[at..at + 8].copy_from_slice(SIGNATURE_MAGIC);
        self.data[at + 8..at + 12].copy_from_slice(&(content.len() as u32).to_le_bytes());
        self.data[at + 12..at + 12 + content.len()].copy_from_slice(content);
        Ok(())
    }

    /// Données brutes modifiables (pour les lecteurs de tables du cœur).
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// Adresse GBA → offset de fichier (`None` si hors de la ROM).
pub fn pointer_to_offset(p: u32, rom_len: usize) -> Option<usize> {
    let hi = p >> 24;
    if hi != 0x08 && hi != 0x09 {
        return None;
    }
    let off = (p - ROM_BASE) as usize;
    (off < rom_len).then_some(off)
}

pub fn offset_to_pointer(off: usize) -> u32 {
    ROM_BASE + off as u32
}

/// Toutes les positions de `pattern` dans `data`.
pub fn find_all(data: &[u8], pattern: &[u8]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > data.len() {
        return Vec::new();
    }
    let first = pattern[0];
    let mut out = Vec::new();
    let mut i = 0;
    while i + pattern.len() <= data.len() {
        match data[i..=data.len() - pattern.len()].iter().position(|&b| b == first) {
            None => break,
            Some(p) => {
                let at = i + p;
                if &data[at..at + pattern.len()] == pattern {
                    out.push(at);
                }
                i = at + 1;
            }
        }
    }
    out
}

/// « 0348048009E0 » → octets.
pub fn hex(s: &str) -> Vec<u8> {
    let s: Vec<u8> = s.bytes().filter(|b| b.is_ascii_hexdigit()).collect();
    s.chunks(2).filter(|c| c.len() == 2).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect()
}

/// En-tête synthétique valide (pour les tests du cœur, aucune vraie ROM n'étant fournie).
pub fn synthetic_header(title: &str, code: &str, version: u8) -> Vec<u8> {
    let mut h = vec![0u8; HEADER_SIZE];
    h[..4].copy_from_slice(&[0x2E, 0x00, 0x00, 0xEA]);
    for (i, b) in title.bytes().take(12).enumerate() {
        h[0xA0 + i] = b;
    }
    h[0xAC..0xB0].copy_from_slice(&code.as_bytes()[..4]);
    h[0xB0..0xB2].copy_from_slice(b"01");
    h[0xB2] = 0x96;
    h[0xBC] = version;
    h[0xBD] = header_complement(&h);
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn rom(size: usize) -> Vec<u8> {
        let mut d = vec![FREE_BYTE; size];
        d[..HEADER_SIZE].copy_from_slice(&synthetic_header("POKEMON FIRE", "BPRF", 0));
        d
    }

    #[test]
    fn header_and_complement() {
        let d = rom(0x1000);
        let h = GbaHeader::parse(&d).unwrap();
        assert_eq!(h.title, "POKEMON FIRE");
        assert_eq!(h.game_code, "BPRF");
        assert_eq!(h.region(), Some('F'));
        assert!(h.complement_ok);
        let mut bad = d.clone();
        bad[0xBC] = 1;
        assert!(!GbaHeader::parse(&bad).unwrap().complement_ok);
        assert!(GbaHeader::parse(&vec![0u8; 0x200]).is_none());
        assert_eq!(GbaHeader::probe(&mut Cursor::new(d)).unwrap().unwrap().game_code, "BPRF");
        assert!(GbaHeader::probe(&mut Cursor::new(vec![0u8; 10])).unwrap().is_none());
    }

    #[test]
    fn pointers_and_search() {
        let mut g = GbaRom::from_bytes(rom(0x4000)).unwrap();
        g.write(0x1000, &hex("0348048009E00000FFFF0000")).unwrap();
        g.write_pointer(0x100C, 0x2345).unwrap();
        assert_eq!(g.find(&hex("0348048009E00000FFFF0000")), Some(0x1000));
        assert_eq!(g.pointer(0x100C), Some(0x2345));
        assert_eq!(g.pointers_to(0x2345), vec![0x100C]);
        assert_eq!(pointer_to_offset(0x0200_0000, 0x4000), None);
        assert_eq!(pointer_to_offset(0x0800_5000, 0x4000), None);
        assert!(g.write(0x3FFF, &[1, 2]).is_err());
    }

    #[test]
    fn free_space() {
        let mut g = GbaRom::from_bytes(rom(0x1000)).unwrap();
        g.write(0x200, &[0; 0x100]).unwrap();
        assert_eq!(g.find_free_space(0x40, 0x100, 4), Some(0x110));
        let at = g.find_free_space(0x100, 0x180, 4).unwrap();
        assert!(at >= 0x310 && at % 4 == 0);
        assert!(g.find_free_space(0x2000, 0, 4).is_none());
    }

    #[test]
    fn signature_round_trip() {
        let mut g = GbaRom::from_bytes(rom(0x20000)).unwrap();
        assert!(g.signature().is_none());
        g.set_signature(b"{\"seed\":1}").unwrap();
        assert_eq!(g.signature(), Some(&b"{\"seed\":1}"[..]));
        g.set_signature(b"{\"seed\":22}").unwrap();
        assert_eq!(g.signature(), Some(&b"{\"seed\":22}"[..]));
        assert_eq!(g.len(), 0x20000);
        let bytes = g.to_bytes();
        assert_eq!(read_signature(&mut Cursor::new(&bytes)).unwrap().unwrap(), b"{\"seed\":22}");
        assert!(GbaHeader::parse(&bytes).unwrap().complement_ok);

        // Fin de ROM occupée : la ROM grandit.
        let mut full = rom(0x10000);
        full[0xFFF0..].fill(0);
        let mut g = GbaRom::from_bytes(full).unwrap();
        g.set_signature(b"x").unwrap();
        assert_eq!(g.len(), 0x20000);
        assert_eq!(g.signature(), Some(&b"x"[..]));
    }
}
