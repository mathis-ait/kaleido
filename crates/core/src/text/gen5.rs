//! Textes Gen 5 (Noire, Blanche, Noire 2, Blanche 2).
//!
//! Les chaînes sont en UTF-16, chiffrées par une clé qui tourne de 3 bits à
//! chaque caractère. Le fichier peut contenir plusieurs blocs (une variante de
//! texte par bloc, ex. kana / kanji en japonais).

use super::{parse_escape, push_escape, TextError};

const END: u16 = 0xFFFF;
const NEWLINE: u16 = 0xFFFE;
const COMMAND: u16 = 0xF000;
/// Clé initiale utilisée pour chiffrer les nouvelles chaînes.
const DEFAULT_KEY: u16 = 0x7C89;

/// Caractères privés des jeux et leur équivalent Unicode. Les noms de Pokémon
/// utilisent ces codes ; les vrais ♂ / ♀ Unicode (plus rares, dans les dialogues)
/// sont donc gardés sous forme `{X:2642}` pour une réécriture exacte.
const SPECIAL: [(u16, char); 2] = [(0x246D, '♂'), (0x246E, '♀')];
const SPECIAL_LOOKALIKES: [u16; 2] = [0x2642, 0x2640];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Codes déchiffrés, terminateur compris.
    pub codes: Vec<u16>,
    pub flags: u16,
    /// Clé du premier caractère, conservée pour une réécriture identique.
    pub key: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgFile {
    pub blocks: Vec<Vec<Entry>>,
    unknown: u32,
    /// Octets de remplissage d'origine de chaque bloc (valeurs arbitraires du jeu),
    /// réutilisés tant que la taille de remplissage ne change pas.
    padding: Vec<Vec<u8>>,
}

fn decrypt(enc: &[u16]) -> (Vec<u16>, u16) {
    let Some(&last) = enc.last() else { return (Vec::new(), DEFAULT_KEY) };
    let mut key = last ^ END;
    let mut dec = vec![0; enc.len()];
    for i in (0..enc.len()).rev() {
        dec[i] = enc[i] ^ key;
        if i > 0 {
            key = key.rotate_right(3);
        }
    }
    (dec, key)
}

fn encrypt(dec: &[u16], mut key: u16) -> Vec<u16> {
    dec.iter()
        .map(|&c| {
            let e = c ^ key;
            key = key.rotate_left(3);
            e
        })
        .collect()
}

impl MsgFile {
    pub fn parse(d: &[u8]) -> Result<Self, TextError> {
        const BAD: TextError = TextError::Invalid("structure hors du fichier");
        let u16_at = |at: usize| d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or(BAD);
        let u32_at = |at: usize| d.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(BAD);

        let block_count = u16_at(0)? as usize;
        let entry_count = u16_at(2)? as usize;
        let unknown = u32_at(8)?;

        let mut blocks = Vec::with_capacity(block_count);
        let mut padding = Vec::with_capacity(block_count);
        for b in 0..block_count {
            let block = u32_at(12 + b * 4)? as usize;
            let block_size = u32_at(block)? as usize;
            let mut entries = Vec::with_capacity(entry_count);
            let mut content_end = 4 + entry_count * 8;
            for e in 0..entry_count {
                let at = block + 4 + e * 8;
                let rel = u32_at(at)? as usize;
                let len = u16_at(at + 4)? as usize;
                let flags = u16_at(at + 6)?;
                let enc = (0..len).map(|j| u16_at(block + rel + j * 2)).collect::<Result<Vec<_>, _>>()?;
                let (codes, key) = decrypt(&enc);
                entries.push(Entry { codes, flags, key });
                content_end = content_end.max(rel + len * 2);
            }
            padding.push(d.get(block + content_end..block + block_size).unwrap_or_default().to_vec());
            blocks.push(entries);
        }
        Ok(Self { blocks, unknown, padding })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let entry_count = self.blocks.first().map_or(0, Vec::len);
        let header_size = 12 + self.blocks.len() * 4;

        let encoded: Vec<Vec<u8>> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, entries)| encode_block(entries, self.padding.get(i).map_or(&[][..], Vec::as_slice)))
            .collect();
        let largest = encoded.iter().map(Vec::len).max().unwrap_or(0);

        let mut out = Vec::new();
        out.extend((self.blocks.len() as u16).to_le_bytes());
        out.extend((entry_count as u16).to_le_bytes());
        out.extend((largest as u32).to_le_bytes());
        out.extend(self.unknown.to_le_bytes());
        let mut offset = header_size;
        for block in &encoded {
            out.extend((offset as u32).to_le_bytes());
            offset += block.len();
        }
        for block in encoded {
            out.extend(block);
        }
        out
    }

    /// Chaînes du premier bloc (le seul dans les versions occidentales).
    pub fn strings(&self) -> Vec<String> {
        self.blocks.first().map(|b| b.iter().map(|e| decode(&e.codes)).collect()).unwrap_or_default()
    }

    /// Remplace les chaînes de tous les blocs, en gardant drapeaux et clés.
    pub fn set_strings(&mut self, strings: &[String]) -> Result<(), TextError> {
        let encoded = strings.iter().map(|s| encode(s)).collect::<Result<Vec<_>, _>>()?;
        for block in &mut self.blocks {
            block.resize_with(encoded.len(), || Entry { codes: vec![END], flags: 0, key: DEFAULT_KEY });
            for (entry, codes) in block.iter_mut().zip(&encoded) {
                entry.codes = codes.clone();
            }
        }
        Ok(())
    }
}

fn encode_block(entries: &[Entry], original_padding: &[u8]) -> Vec<u8> {
    let table_end = 4 + entries.len() * 8;
    let mut table = Vec::with_capacity(table_end);
    let mut strings = Vec::new();
    for e in entries {
        table.extend(((table_end + strings.len()) as u32).to_le_bytes());
        table.extend((e.codes.len() as u16).to_le_bytes());
        table.extend(e.flags.to_le_bytes());
        strings.extend(encrypt(&e.codes, e.key).iter().flat_map(|c| c.to_le_bytes()));
    }
    let pad = (4 - (table_end + strings.len()) % 4) % 4;
    if original_padding.len() == pad {
        strings.extend_from_slice(original_padding);
    } else {
        strings.resize(strings.len() + pad, 0);
    }
    let mut out = ((table_end + strings.len()) as u32).to_le_bytes().to_vec();
    out.extend(table);
    out.extend(strings);
    out
}

pub fn decode(codes: &[u16]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < codes.len() {
        let c = codes[i];
        i += 1;
        match c {
            END => break,
            NEWLINE => out.push('\n'),
            COMMAND => {
                let cmd = codes.get(i).copied().unwrap_or(0);
                let argc = codes.get(i + 1).copied().unwrap_or(0) as usize;
                let args = codes.get(i + 2..(i + 2 + argc).min(codes.len())).unwrap_or(&[]);
                let mut values = vec![cmd];
                values.extend_from_slice(args);
                push_escape(&mut out, "VAR", &values);
                i += 2 + argc;
            }
            _ => match SPECIAL.iter().find(|(code, _)| *code == c) {
                Some(&(_, ch)) => out.push(ch),
                None => match char::from_u32(c as u32).filter(|ch| *ch != '{' && !ch.is_control() && !SPECIAL_LOOKALIKES.contains(&c)) {
                    Some(ch) => out.push(ch),
                    None => push_escape(&mut out, "X", &[c]),
                },
            },
        }
    }
    out
}

pub fn encode(text: &str) -> Result<Vec<u16>, TextError> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
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
        match c {
            '\n' => out.push(NEWLINE),
            _ => match SPECIAL.iter().find(|(_, ch)| *ch == c) {
                Some(&(code, _)) => out.push(code),
                None => {
                    let mut buf = [0u16; 2];
                    let units = c.encode_utf16(&mut buf);
                    if units.len() != 1 {
                        return Err(TextError::Unencodable(c));
                    }
                    out.push(units[0]);
                }
            },
        }
        rest = &rest[c.len_utf8()..];
    }
    out.push(END);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypt_roundtrip() {
        let codes = encode("Salamèche").unwrap();
        let enc = encrypt(&codes, DEFAULT_KEY);
        assert_eq!(decrypt(&enc), (codes, DEFAULT_KEY));
    }

    #[test]
    fn file_roundtrip() {
        let mut file = MsgFile { blocks: vec![vec![]], unknown: 0, padding: vec![] };
        let strings = vec!["Œuf".to_string(), "Nidoran♀".into(), "{VAR:0100,0000} gagne !\nBravo".into()];
        file.set_strings(&strings).unwrap();
        let parsed = MsgFile::parse(&file.to_bytes()).unwrap();
        assert_eq!(parsed.strings(), strings);
        assert_eq!(parsed.to_bytes(), file.to_bytes());
    }
}
