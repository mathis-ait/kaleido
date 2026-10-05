//! Textes Gen 4 (Diamant, Perle, Platine, HeartGold, SoulSilver).
//!
//! Chaque fichier de messages commence par le nombre d'entrées et une graine.
//! La table des entrées est chiffrée avec une clé dérivée de la graine, et
//! chaque chaîne avec une clé qui dépend de son index. Les caractères suivent
//! une table propre à la Gen 4 (pas d'Unicode).

use std::collections::HashMap;
use std::sync::LazyLock;

use super::{parse_escape, push_escape, unpack_9bit, TextError};

const END: u16 = 0xFFFF;
const COMMAND: u16 = 0xFFFE;
const COMPRESSED: u16 = 0xF100;
const NEWLINE: u16 = 0xE000;
const SCROLL: u16 = 0x25BC;
const CLEAR: u16 = 0x25BD;

/// Fichier de messages : chaînes déchiffrées, sous forme de codes bruts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgFile {
    pub seed: u16,
    pub entries: Vec<Vec<u16>>,
}

fn table_key(seed: u16, index: usize) -> u32 {
    let k = (seed as u32 * 0x2FD * (index as u32 + 1)) & 0xFFFF;
    k | k << 16
}

fn string_key(index: usize) -> u16 {
    ((0x91BD3 * (index as u32 + 1)) & 0xFFFF) as u16
}

fn crypt_string(codes: &mut [u16], index: usize) {
    let mut key = string_key(index);
    for c in codes {
        *c ^= key;
        key = key.wrapping_add(0x493D);
    }
}

impl MsgFile {
    pub fn parse(d: &[u8]) -> Result<Self, TextError> {
        const BAD: TextError = TextError::Invalid("table des entrées hors du fichier");
        let u16_at = |at: usize| d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(BAD);
        let u32_at = |at: usize| d.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(BAD);

        let count = u16_at(0)? as usize;
        let seed = u16_at(2)?;
        let mut entries = Vec::with_capacity(count);
        for i in 0..count {
            let key = table_key(seed, i);
            let offset = (u32_at(4 + i * 8)? ^ key) as usize;
            let len = (u32_at(8 + i * 8)? ^ key) as usize;
            let mut codes = (0..len).map(|j| u16_at(offset + j * 2)).collect::<Result<Vec<_>, _>>()?;
            crypt_string(&mut codes, i);
            entries.push(codes);
        }
        Ok(Self { seed, entries })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let count = self.entries.len();
        let mut out = Vec::new();
        out.extend((count as u16).to_le_bytes());
        out.extend(self.seed.to_le_bytes());

        let mut offset = 4 + count * 8;
        for (i, e) in self.entries.iter().enumerate() {
            let key = table_key(self.seed, i);
            out.extend((offset as u32 ^ key).to_le_bytes());
            out.extend((e.len() as u32 ^ key).to_le_bytes());
            offset += e.len() * 2;
        }
        for (i, e) in self.entries.iter().enumerate() {
            let mut codes = e.clone();
            crypt_string(&mut codes, i);
            out.extend(codes.iter().flat_map(|c| c.to_le_bytes()));
        }
        out
    }

    pub fn strings(&self) -> Vec<String> {
        self.entries.iter().map(|e| decode(e)).collect()
    }

    pub fn from_strings(seed: u16, strings: &[String]) -> Result<Self, TextError> {
        let entries = strings.iter().map(|s| encode(s)).collect::<Result<_, _>>()?;
        Ok(Self { seed, entries })
    }
}

/// Codes → texte lisible.
pub fn decode(codes: &[u16]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < codes.len() {
        let c = codes[i];
        i += 1;
        match c {
            END => break,
            NEWLINE => out.push('\n'),
            SCROLL => out.push('\r'),
            CLEAR => out.push('\u{c}'),
            COMMAND => {
                let cmd = codes.get(i).copied().unwrap_or(0);
                let argc = codes.get(i + 1).copied().unwrap_or(0) as usize;
                let args = codes.get(i + 2..(i + 2 + argc).min(codes.len())).unwrap_or(&[]);
                let mut values = vec![cmd];
                values.extend_from_slice(args);
                push_escape(&mut out, "VAR", &values);
                i += 2 + argc;
            }
            COMPRESSED => {
                for code in unpack_9bit(&codes[i..]) {
                    push_char(&mut out, code);
                }
                break;
            }
            _ => push_char(&mut out, c),
        }
    }
    out
}

fn push_char(out: &mut String, code: u16) {
    match CHARMAP.decode.get(&code) {
        Some(s) => out.push_str(s),
        None => push_escape(out, "X", &[code]),
    }
}

/// Texte → codes (terminés par `0xFFFF`).
pub fn encode(text: &str) -> Result<Vec<u16>, TextError> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        match c {
            '\n' => out.push(NEWLINE),
            '\r' => out.push(SCROLL),
            '\u{c}' => out.push(CLEAR),
            '{' => {
                let (tag, values, used) = parse_escape(rest)?;
                match (tag, values.as_slice()) {
                    ("X", [code]) => out.push(*code),
                    ("VAR", [cmd, args @ ..]) => {
                        out.extend([COMMAND, *cmd, args.len() as u16]);
                        out.extend_from_slice(args);
                    }
                    _ => return Err(TextError::BadEscape(rest[..used].into())),
                }
                rest = &rest[used..];
                continue;
            }
            _ => {
                // Les jetons de plusieurs caractères (« ᵉʳ », « ʳᵉ ») sont essayés en premier.
                let two: String = rest.chars().take(2).collect();
                if let Some(&code) = CHARMAP.encode.get(two.as_str()).filter(|_| two.chars().count() == 2) {
                    out.push(code);
                    rest = &rest[two.len()..];
                    continue;
                }
                let code = CHARMAP.encode.get(&rest[..c.len_utf8()]).ok_or(TextError::Unencodable(c))?;
                out.push(*code);
            }
        }
        rest = &rest[c.len_utf8()..];
    }
    out.push(END);
    Ok(out)
}

struct Charmap {
    decode: HashMap<u16, String>,
    encode: HashMap<String, u16>,
}

static CHARMAP: LazyLock<Charmap> = LazyLock::new(|| {
    let mut pairs: Vec<(u16, String)> = Vec::new();
    let mut range = |first: u16, chars: &mut dyn Iterator<Item = char>| {
        for (i, c) in chars.enumerate() {
            pairs.push((first + i as u16, c.to_string()));
        }
    };
    range(0x0121, &mut ('0'..='9'));
    range(0x012B, &mut ('A'..='Z'));
    range(0x0145, &mut ('a'..='z'));
    // 0x015F-0x019E : même ordre que Latin-1 de À (0xC0) à ÿ (0xFF).
    range(0x015F, &mut (0xC0u32..=0xFF).filter_map(char::from_u32));
    range(0x019F, &mut "ŒœŞşªº".chars());
    range(0x01A8, &mut "₽¡¿!?,.…･/‘’“”„«»()♂♀+-*#=&~:;♠♣♥♦★◎○□△◇@♪%☀☁☂☃".chars());
    range(0x01DE, &mut " ".chars());
    pairs.push((0x01A5, "ᵉʳ".into()));
    pairs.push((0x01A6, "ʳᵉ".into()));
    pairs.push((0x01A7, "ʳ".into()));
    // Petites capitales « PK » / « MN » de « Pokémon Trainer ».
    pairs.push((0x01E0, "ᴾᴷ".into()));
    pairs.push((0x01E1, "ᴹᴺ".into()));
    // Espace de largeur fixe, utilisé pour aligner les tailles et poids du Pokédex.
    pairs.push((0x01E2, "\u{2007}".into()));
    pairs.push((0x01E8, "°".into()));
    // Les codes 0x0001-0x0120 sont les kana / kanji japonais, non gérés : ils restent en `{X:…}`.

    let encode = pairs.iter().map(|(code, s)| (s.clone(), *code)).collect();
    Charmap { decode: pairs.into_iter().collect(), encode }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        for s in ["Bulbizarre", "Salamèche", "Nidoran♀", "M. Mime", "1ᵉʳ étage\nPoké Ball !", "{VAR:0100,0000,0001} a gagné !"] {
            let codes = encode(s).unwrap();
            assert_eq!(decode(&codes), s, "{s}");
        }
    }

    #[test]
    fn file_roundtrip() {
        let strings = vec!["Œuf".to_string(), "Bulbizarre".into(), "Herbizarre".into()];
        let file = MsgFile::from_strings(0x1234, &strings).unwrap();
        let bytes = file.to_bytes();
        let parsed = MsgFile::parse(&bytes).unwrap();
        assert_eq!(parsed, file);
        assert_eq!(parsed.strings(), strings);
    }

    #[test]
    fn rejects_unknown_char() {
        assert!(matches!(encode("日"), Err(TextError::Unencodable('日'))));
    }
}
