//! Archives NARC (« Nitro ARChive ») : la plupart des données des jeux DS.
//!
//! Structure : en-tête `NARC`, puis les sections `BTAF` (table des fichiers),
//! `BTNF` (noms, généralement vide) et `GMIF` (données).

use crate::util::{slice, u16le, u32le};
use crate::{FormatError, Result};

const HEADER_SIZE: usize = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Narc {
    pub files: Vec<Vec<u8>>,
    /// Section BTNF conservée telle quelle (en-tête compris).
    names_section: Vec<u8>,
}

impl Narc {
    pub fn is_narc(data: &[u8]) -> bool {
        data.starts_with(b"NARC")
    }

    pub fn parse(d: &[u8]) -> Result<Self> {
        if !Self::is_narc(d) {
            return Err(FormatError::Invalid("ce n'est pas une archive NARC"));
        }
        let mut at = u16le(slice(d, 0, HEADER_SIZE as u32)?, 0x0C) as u32;

        let btaf = section(d, at, b"BTAF")?;
        let count = u16le(btaf, 8) as u32;
        let entries = slice(btaf, 12, count * 8)?;
        at += btaf.len() as u32;

        let names_section = section(d, at, b"BTNF")?.to_vec();
        at += names_section.len() as u32;

        let gmif = section(d, at, b"GMIF")?;
        let files = entries
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| {
                let (s, e) = (u32le(c, 0), u32le(c, 4));
                if s > e {
                    return Err(FormatError::Invalid("entrée NARC incohérente"));
                }
                Ok(slice(gmif, 8 + s, e - s)?.to_vec())
            })
            .collect::<Result<_>>()?;

        Ok(Self { files, names_section })
    }

    pub fn from_files(files: Vec<Vec<u8>>) -> Self {
        // BTNF minimal : un seul dossier racine, sans nom.
        let mut names_section = b"BTNF".to_vec();
        names_section.extend(16u32.to_le_bytes());
        names_section.extend(4u32.to_le_bytes());
        names_section.extend(0u16.to_le_bytes());
        names_section.extend(1u16.to_le_bytes());
        Self { files, names_section }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        let mut entries = Vec::with_capacity(self.files.len() * 8);
        for f in &self.files {
            entries.extend((data.len() as u32).to_le_bytes());
            data.extend_from_slice(f);
            entries.extend((data.len() as u32).to_le_bytes());
            while data.len() % 4 != 0 {
                data.push(0xFF);
            }
        }

        let btaf_size = 12 + entries.len();
        let gmif_size = 8 + data.len();
        let total = HEADER_SIZE + btaf_size + self.names_section.len() + gmif_size;

        let mut out = Vec::with_capacity(total);
        out.extend(b"NARC");
        out.extend(0xFFFEu16.to_le_bytes());
        out.extend(0x0100u16.to_le_bytes());
        out.extend((total as u32).to_le_bytes());
        out.extend((HEADER_SIZE as u16).to_le_bytes());
        out.extend(3u16.to_le_bytes());

        out.extend(b"BTAF");
        out.extend((btaf_size as u32).to_le_bytes());
        out.extend((self.files.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(entries);

        out.extend(&self.names_section);

        out.extend(b"GMIF");
        out.extend((gmif_size as u32).to_le_bytes());
        out.extend(data);
        out
    }
}

/// Section `magic` commençant à `at`, taille lue dans son en-tête.
fn section<'a>(d: &'a [u8], at: u32, magic: &[u8; 4]) -> Result<&'a [u8]> {
    let head = slice(d, at, 8)?;
    if &head[..4] != magic {
        return Err(FormatError::Invalid("section NARC manquante"));
    }
    slice(d, at, u32le(head, 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let narc = Narc::from_files(vec![b"abc".to_vec(), vec![], b"defgh".to_vec()]);
        let bytes = narc.to_bytes();
        assert_eq!(Narc::parse(&bytes).unwrap(), narc);
        assert_eq!(Narc::parse(&bytes).unwrap().to_bytes(), bytes);
    }
}
