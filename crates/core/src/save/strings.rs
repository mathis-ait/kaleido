//! Chaînes des Pokémon et des sauvegardes.
//!
//! - Gen 4 : table de caractères propre au jeu, terminateur `0xFFFF` ([`crate::text::gen4`]) ;
//! - Gen 5 : UTF-16, terminateur `0xFFFF`, ♂/♀ en caractères privés ([`crate::text::gen5`]) ;
//! - Gen 6/7 : UTF-16, terminateur `0x0000`, ♂/♀ en `0xE08E`/`0xE08F` (d'après PKHeX).

use super::pkm::{PkmError, PkmFormat};
use crate::text::{gen4, gen5, TextError};

const MALE_3DS: u16 = 0xE08E;
const FEMALE_3DS: u16 = 0xE08F;

fn words(bytes: &[u8]) -> Vec<u16> {
    bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

/// Décode une chaîne de taille fixe (s'arrête au terminateur).
pub(super) fn decode(format: PkmFormat, bytes: &[u8]) -> String {
    let w = words(bytes);
    match format {
        PkmFormat::Gen4 | PkmFormat::Gen5 => {
            // Les tampons jamais écrits sont remplis de zéros : on s'arrête aussi là.
            let end = w.iter().position(|&c| c == 0xFFFF || c == 0).unwrap_or(w.len());
            if format == PkmFormat::Gen4 {
                gen4::decode(&w[..end])
            } else {
                gen5::decode(&w[..end])
            }
        }
        PkmFormat::Gen6 | PkmFormat::Gen7 => {
            let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
            let units = w[..end].iter().map(|&c| match c {
                MALE_3DS => 0x2642,
                FEMALE_3DS => 0x2640,
                c => c,
            });
            char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
        }
    }
}

/// Encode une chaîne dans un tampon de `max_chars + 1` mots (terminateur compris),
/// complété par des zéros.
pub(super) fn encode(format: PkmFormat, text: &str, max_chars: usize) -> Result<Vec<u8>, PkmError> {
    let mut codes = match format {
        PkmFormat::Gen4 => gen4::encode(text)?,
        PkmFormat::Gen5 => gen5::encode(text)?,
        PkmFormat::Gen6 | PkmFormat::Gen7 => {
            let mut out = Vec::with_capacity(text.len() + 1);
            for c in text.chars() {
                out.push(match c {
                    '♂' => MALE_3DS,
                    '♀' => FEMALE_3DS,
                    _ => {
                        let mut buf = [0u16; 2];
                        match c.encode_utf16(&mut buf) {
                            [unit] if *unit != 0 => *unit,
                            _ => return Err(TextError::Unencodable(c).into()),
                        }
                    }
                });
            }
            out.push(0);
            out
        }
    };
    let len = codes.len() - 1;
    if len > max_chars {
        return Err(PkmError::TooLong { len, max: max_chars });
    }
    codes.resize(max_chars + 1, 0);
    Ok(codes.iter().flat_map(|c| c.to_le_bytes()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_formats() {
        for format in [PkmFormat::Gen4, PkmFormat::Gen5, PkmFormat::Gen6, PkmFormat::Gen7] {
            for s in ["Carchacrok", "Nidoran♀", "Élise", ""] {
                let bytes = encode(format, s, 10).unwrap();
                assert_eq!(bytes.len(), 22);
                assert_eq!(decode(format, &bytes), s, "{format:?} {s}");
            }
        }
    }

    #[test]
    fn terminators() {
        let gen4 = encode(PkmFormat::Gen4, "Abc", 7).unwrap();
        assert_eq!(&gen4[6..8], &[0xFF, 0xFF]);
        let gen6 = encode(PkmFormat::Gen6, "♂", 12).unwrap();
        assert_eq!(&gen6[..4], &[0x8E, 0xE0, 0, 0]);
    }

    #[test]
    fn too_long() {
        assert!(matches!(encode(PkmFormat::Gen6, "Abcdefghijklm", 12), Err(PkmError::TooLong { len: 13, max: 12 })));
        assert!(matches!(encode(PkmFormat::Gen6, "😀", 12), Err(PkmError::Text(_))));
    }
}
