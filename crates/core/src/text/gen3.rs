//! Textes Gen 3 (Rubis, Saphir, Émeraude, Rouge Feu, Vert Feuille), jeux occidentaux.
//!
//! Un octet par caractère, terminé par 0xFF. Table « gba_english » de l'Universal
//! Pokémon Randomizer (UPR-ZX, GPLv3), la même que `StringConverter3` de PKHeX pour les
//! caractères latins ; les symboles propres au jeu (Pk, Mn…) deviennent `{X:hh}`.

/// Fin de chaîne.
pub const TERMINATOR: u8 = 0xFF;

/// Caractère de chaque octet (`None` : code sans équivalent, conservé en `{X:hh}`).
const TABLE: [Option<char>; 256] = build();

const fn build() -> [Option<char>; 256] {
    let mut t: [Option<char>; 256] = [None; 256];
    let pairs: &[(u8, char)] = &[
        (0x00, ' '),
        (0x01, 'À'),
        (0x02, 'Á'),
        (0x03, 'Â'),
        (0x04, 'Ç'),
        (0x05, 'È'),
        (0x06, 'É'),
        (0x07, 'Ê'),
        (0x08, 'Ë'),
        (0x09, 'Ì'),
        (0x0B, 'Î'),
        (0x0C, 'Ï'),
        (0x0D, 'Ò'),
        (0x0E, 'Ó'),
        (0x0F, 'Ô'),
        (0x10, 'Œ'),
        (0x11, 'Ù'),
        (0x12, 'Ú'),
        (0x13, 'Û'),
        (0x14, 'Ñ'),
        (0x15, 'ß'),
        (0x16, 'à'),
        (0x17, 'á'),
        (0x19, 'ç'),
        (0x1A, 'è'),
        (0x1B, 'é'),
        (0x1C, 'ê'),
        (0x1D, 'ë'),
        (0x1E, 'ì'),
        (0x20, 'î'),
        (0x21, 'ï'),
        (0x22, 'ò'),
        (0x23, 'ó'),
        (0x24, 'ô'),
        (0x25, 'œ'),
        (0x26, 'ù'),
        (0x27, 'ú'),
        (0x28, 'û'),
        (0x29, 'ñ'),
        (0x2A, 'º'),
        (0x2B, 'ª'),
        (0x2D, '&'),
        (0x2E, '+'),
        (0x35, '='),
        (0x36, ';'),
        (0x51, '¿'),
        (0x52, '¡'),
        (0x5A, 'Í'),
        (0x5B, '%'),
        (0x5C, '('),
        (0x5D, ')'),
        (0x68, 'â'),
        (0x6F, 'í'),
        (0xA1, '0'),
        (0xA2, '1'),
        (0xA3, '2'),
        (0xA4, '3'),
        (0xA5, '4'),
        (0xA6, '5'),
        (0xA7, '6'),
        (0xA8, '7'),
        (0xA9, '8'),
        (0xAA, '9'),
        (0xAB, '!'),
        (0xAC, '?'),
        (0xAD, '.'),
        (0xAE, '-'),
        (0xAF, '·'),
        (0xB0, '…'),
        (0xB1, '«'),
        (0xB2, '»'),
        (0xB3, '‘'),
        (0xB4, '\''),
        (0xB5, '♂'),
        (0xB6, '♀'),
        (0xB7, '$'),
        (0xB8, ','),
        (0xB9, '×'),
        (0xBA, '/'),
        (0xF0, ':'),
        (0xF1, 'Ä'),
        (0xF2, 'Ö'),
        (0xF3, 'Ü'),
        (0xF4, 'ä'),
        (0xF5, 'ö'),
        (0xF6, 'ü'),
        (0xFE, '\n'),
    ];
    let mut i = 0;
    while i < pairs.len() {
        t[pairs[i].0 as usize] = Some(pairs[i].1);
        i += 1;
    }
    let mut c = 0;
    while c < 26 {
        t[0xBB + c] = Some((b'A' + c as u8) as char);
        t[0xD5 + c] = Some((b'a' + c as u8) as char);
        c += 1;
    }
    t
}

/// Décode jusqu'au terminateur (ou la fin de `data`).
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

/// Code d'un caractère (quelques équivalents tolérés : guillemets droits, apostrophe typographique).
fn code_of(c: char) -> Option<u8> {
    let c = match c {
        '’' => '\'',
        '“' | '"' => '«',
        '”' => '»',
        _ => c,
    };
    (0..=0xFEu8).find(|&b| TABLE[b as usize] == Some(c))
}

/// Encode `text` sur `len` octets : caractères, terminateur 0xFF puis remplissage `pad`.
/// Les caractères inconnus deviennent des espaces ; le texte est tronqué si besoin
/// (le terminateur est toujours présent quand il reste la place).
pub fn encode(text: &str, len: usize, pad: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if out.len() >= len {
            break;
        }
        if c == '{' {
            // {X:hh}
            let esc: String = chars.by_ref().take_while(|&c| c != '}').collect();
            if let Some(v) = esc.strip_prefix("X:").and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
            }
            continue;
        }
        out.push(code_of(c).unwrap_or(0));
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
        // « PIKACHU » tel qu'il est stocké dans les jeux occidentaux.
        let raw = [0xCA, 0xC3, 0xC5, 0xBB, 0xBD, 0xC2, 0xCF, 0xFF, 0x00];
        assert_eq!(decode(&raw), "PIKACHU");
        assert_eq!(encode("PIKACHU", 9, 0), raw);
        assert_eq!(decode(&encode("Élekid, Mélo ♀ !", 20, 0xFF)), "Élekid, Mélo ♀ !");
        assert_eq!(encode("ABCDEFGHIJKL", 4, 0), [0xBB, 0xBC, 0xBD, 0xBE]);
        assert_eq!(decode(&[0x53, 0x54, 0xFF]), "{X:53}{X:54}");
        assert_eq!(encode("{X:53}", 3, 0), [0x53, 0xFF, 0x00]);
    }
}
