//! Textes Gen 1 et 2 (Rouge, Bleu, Jaune, Or, Argent, Cristal), jeux occidentaux.
//!
//! Un octet par caractère, terminé par 0x50. Tables « rby_english » / « rby_freger » et
//! « gsc_freger » de l'Universal Pokémon Randomizer (UPR-ZX, GPLv3), comme
//! `StringConverter12` de PKHeX pour les caractères latins : majuscules 0x80-0x99,
//! minuscules 0xA0-0xB9, accents 0xBA-0xCC, chiffres 0xF6-0xFF, espace 0x7F.

/// Fin de chaîne.
pub const TERMINATOR: u8 = 0x50;

const TABLE: [Option<char>; 256] = build();

const fn build() -> [Option<char>; 256] {
    let mut t: [Option<char>; 256] = [None; 256];
    let pairs: &[(u8, char)] = &[
        (0x7F, ' '),
        (0x9A, '('),
        (0x9B, ')'),
        (0x9C, ':'),
        (0x9D, ';'),
        (0x9E, '['),
        (0x9F, ']'),
        (0xBA, 'à'),
        (0xBB, 'è'),
        (0xBC, 'é'),
        (0xBD, 'ù'),
        (0xBE, 'ß'),
        (0xBF, 'ç'),
        (0xC0, 'Ä'),
        (0xC1, 'Ö'),
        (0xC2, 'Ü'),
        (0xC3, 'ä'),
        (0xC4, 'ö'),
        (0xC5, 'ü'),
        (0xC6, 'ë'),
        (0xC7, 'ï'),
        (0xC8, 'â'),
        (0xC9, 'ô'),
        (0xCA, 'û'),
        (0xCB, 'ê'),
        (0xCC, 'î'),
        (0xE0, '\''),
        (0xE3, '-'),
        (0xE4, '+'),
        (0xE6, '?'),
        (0xE7, '!'),
        (0xE8, '.'),
        (0xE9, '&'),
        (0xEF, '♂'),
        (0xF0, '$'),
        (0xF1, '×'),
        (0xF3, '/'),
        (0xF4, ','),
        (0xF5, '♀'),
    ];
    let mut i = 0;
    while i < pairs.len() {
        t[pairs[i].0 as usize] = Some(pairs[i].1);
        i += 1;
    }
    let mut c = 0;
    while c < 26 {
        t[0x80 + c] = Some((b'A' + c as u8) as char);
        t[0xA0 + c] = Some((b'a' + c as u8) as char);
        c += 1;
    }
    let mut d = 0;
    while d < 10 {
        t[0xF6 + d] = Some((b'0' + d as u8) as char);
        d += 1;
    }
    t
}

/// Décode jusqu'au terminateur 0x50 (les codes inconnus deviennent `{X:hh}`).
pub fn decode(data: &[u8]) -> String {
    let mut out = String::new();
    for &b in data {
        if b == TERMINATOR {
            break;
        }
        match TABLE[b as usize] {
            Some(c) => out.push(c),
            None => out.push_str(&format!("{{X:{b:02X}}}")),
        }
    }
    out
}

fn code_of(c: char) -> Option<u8> {
    let c = match c {
        '’' => '\'',
        _ => c,
    };
    (0..=0xFFu8).find(|&b| TABLE[b as usize] == Some(c))
}

/// Encode sur `len` octets : caractères, terminateur 0x50 puis remplissage `pad`.
/// Les caractères inconnus deviennent des espaces.
pub fn encode(text: &str, len: usize, pad: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if out.len() >= len {
            break;
        }
        if c == '{' {
            let esc: String = chars.by_ref().take_while(|&c| c != '}').collect();
            if let Some(v) = esc.strip_prefix("X:").and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
            }
            continue;
        }
        out.push(code_of(c).unwrap_or(0x7F));
    }
    out.truncate(len);
    if out.len() < len {
        out.push(TERMINATOR);
    }
    out.resize(len, pad);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        // « PIKACHU » des tests de PKHeX.
        let raw = [0x8F, 0x88, 0x8A, 0x80, 0x82, 0x87, 0x94, 0x50];
        assert_eq!(decode(&raw), "PIKACHU");
        assert_eq!(encode("PIKACHU", 8, 0x50), raw);
        assert_eq!(decode(&encode("Mélo ♀ 42!", 11, 0)), "Mélo ♀ 42!");
    }
}
