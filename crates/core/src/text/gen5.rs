//! Textes Gen 5 (Noire, Blanche, Noire 2, Blanche 2), Gen 6 et Gen 7 (3DS).
//!
//! Les chaînes sont en UTF-16, chiffrées par une clé qui tourne de 3 bits à
//! chaque caractère. Le fichier peut contenir plusieurs blocs (une variante de
//! texte par bloc, ex. kana / kanji en japonais).
//!
//! Le conteneur et le chiffrement sont identiques sur 3DS ; seuls les codes
//! changent (voir [`Variant`]).

use super::{parse_escape, push_escape, TextError};

/// Clé initiale utilisée pour chiffrer les nouvelles chaînes en Gen 5.
const DEFAULT_KEY: u16 = 0x7C89;
/// Gen 6/7 : la clé de la ligne `i` vaut `0x7C89 + i × 0x2983`.
const LINE_KEY_STEP: u16 = 0x2983;

/// Jeu de codes d'un fichier de texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// Noire / Blanche (1 et 2) : terminateur `0xFFFF`, commandes `0xF000`.
    #[default]
    Gen5,
    /// X/Y, ROSA, Soleil/Lune, USUL : terminateur `0x0000`, retour à la ligne `0x000A`,
    /// commandes `0x0010`, ♂ / ♀ en `0xE08E` / `0xE08F`.
    Gen6,
}

impl Variant {
    pub fn for_generation(generation: u8) -> Self {
        if generation >= 6 {
            Variant::Gen6
        } else {
            Variant::Gen5
        }
    }

    fn end(self) -> u16 {
        match self {
            Variant::Gen5 => 0xFFFF,
            Variant::Gen6 => 0x0000,
        }
    }

    fn newline(self) -> u16 {
        match self {
            Variant::Gen5 => 0xFFFE,
            Variant::Gen6 => 0x000A,
        }
    }

    fn command(self) -> u16 {
        match self {
            Variant::Gen5 => 0xF000,
            Variant::Gen6 => 0x0010,
        }
    }

    /// Caractères privés des jeux et leur équivalent Unicode. Les noms de Pokémon
    /// utilisent ces codes ; les vrais ♂ / ♀ Unicode (plus rares, dans les dialogues)
    /// sont donc gardés sous forme `{X:2642}` pour une réécriture exacte.
    fn special(self) -> [(u16, char); 2] {
        match self {
            Variant::Gen5 => [(0x246D, '♂'), (0x246E, '♀')],
            Variant::Gen6 => [(0xE08E, '♂'), (0xE08F, '♀')],
        }
    }

    /// Clé d'une nouvelle ligne.
    fn line_key(self, line: usize) -> u16 {
        match self {
            Variant::Gen5 => DEFAULT_KEY,
            Variant::Gen6 => DEFAULT_KEY.wrapping_add((line as u16).wrapping_mul(LINE_KEY_STEP)),
        }
    }
}

const SPECIAL_LOOKALIKES: [u16; 2] = [0x2642, 0x2640];

/// Début d'une chaîne compressée (Gen 5), comme en Gen 4.
const COMPRESSED: u16 = 0xF100;

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
    pub variant: Variant,
    unknown: u32,
    /// Octets de remplissage d'origine de chaque bloc (valeurs arbitraires du jeu),
    /// réutilisés tant que la taille de remplissage ne change pas.
    padding: Vec<Vec<u8>>,
}

/// Gen 5 : la clé se déduit du dernier caractère (le terminateur).
fn decrypt_from_end(enc: &[u16], end: u16) -> (Vec<u16>, u16) {
    let Some(&last) = enc.last() else { return (Vec::new(), DEFAULT_KEY) };
    let mut key = last ^ end;
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
    /// Fichier Gen 5.
    pub fn parse(d: &[u8]) -> Result<Self, TextError> {
        Self::parse_with(d, Variant::Gen5)
    }

    pub fn parse_with(d: &[u8], variant: Variant) -> Result<Self, TextError> {
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
                let (codes, key) = match variant {
                    Variant::Gen5 => decrypt_from_end(&enc, variant.end()),
                    Variant::Gen6 => {
                        // Le chiffrement est symétrique : chiffrer avec la clé de la ligne déchiffre.
                        let key = variant.line_key(e);
                        (encrypt(&enc, key), key)
                    }
                };
                entries.push(Entry { codes, flags, key });
                content_end = content_end.max(rel + len * 2);
            }
            padding.push(d.get(block + content_end..block + block_size).unwrap_or_default().to_vec());
            blocks.push(entries);
        }
        Ok(Self { blocks, variant, unknown, padding })
    }

    /// Nouveau fichier vide (un bloc).
    pub fn new(variant: Variant) -> Self {
        Self { blocks: vec![Vec::new()], variant, unknown: 0, padding: Vec::new() }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let entry_count = self.blocks.first().map_or(0, Vec::len);
        let header_size = 12 + self.blocks.len() * 4;

        let encoded: Vec<Vec<u8>> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, entries)| encode_block(entries, self.variant, self.padding.get(i).map_or(&[][..], Vec::as_slice)))
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
        self.blocks.first().map(|b| b.iter().map(|e| decode_with(&e.codes, self.variant)).collect()).unwrap_or_default()
    }

    /// Remplace les chaînes de tous les blocs, en gardant drapeaux et clés.
    pub fn set_strings(&mut self, strings: &[String]) -> Result<(), TextError> {
        let variant = self.variant;
        let encoded = strings.iter().map(|s| encode_with(s, variant)).collect::<Result<Vec<_>, _>>()?;
        for block in &mut self.blocks {
            while block.len() < encoded.len() {
                let key = variant.line_key(block.len());
                block.push(Entry { codes: vec![variant.end()], flags: 0, key });
            }
            block.truncate(encoded.len());
            for (entry, codes) in block.iter_mut().zip(&encoded) {
                // Certaines lignes ont une taille fixe, complétée par des terminateurs : on la garde.
                let end = variant.end();
                let original_len = entry.codes.len();
                let fixed_size = entry.codes.iter().position(|&c| c == end).is_some_and(|p| p + 1 < original_len);
                entry.codes = codes.clone();
                if fixed_size && original_len > entry.codes.len() {
                    entry.codes.resize(original_len, end);
                }
            }
        }
        Ok(())
    }
}

fn encode_block(entries: &[Entry], variant: Variant, original_padding: &[u8]) -> Vec<u8> {
    let table_end = 4 + entries.len() * 8;
    let mut table = Vec::with_capacity(table_end);
    let mut strings = Vec::new();
    for e in entries {
        table.extend(((table_end + strings.len()) as u32).to_le_bytes());
        table.extend((e.codes.len() as u16).to_le_bytes());
        table.extend(e.flags.to_le_bytes());
        strings.extend(encrypt(&e.codes, e.key).iter().flat_map(|c| c.to_le_bytes()));
        // Gen 6/7 : chaque ligne est alignée sur 4 octets, avec des zéros.
        if variant == Variant::Gen6 && strings.len() % 4 != 0 {
            strings.extend([0, 0]);
        }
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

/// Décode une chaîne Gen 5.
pub fn decode(codes: &[u16]) -> String {
    decode_with(codes, Variant::Gen5)
}

/// Encode une chaîne Gen 5.
pub fn encode(text: &str) -> Result<Vec<u16>, TextError> {
    encode_with(text, Variant::Gen5)
}

pub fn decode_with(codes: &[u16], variant: Variant) -> String {
    let special = variant.special();
    let mut out = String::new();
    let mut i = 0;
    while i < codes.len() {
        let c = codes[i];
        i += 1;
        if c == variant.end() {
            break;
        }
        if c == variant.newline() {
            out.push('\n');
            continue;
        }
        if variant == Variant::Gen5 && c == COMPRESSED {
            // Chaîne compressée (noms des dresseurs de Noire/Blanche) : caractères sur 9 bits.
            for code in super::unpack_9bit(&codes[i..], 16) {
                match special.iter().find(|(s, _)| *s == code) {
                    Some(&(_, ch)) => out.push(ch),
                    None => match char::from_u32(code as u32) {
                        Some(ch) if !ch.is_control() => out.push(ch),
                        _ => push_escape(&mut out, "X", &[code]),
                    },
                }
            }
            break;
        }
        if c == variant.command() {
            // Gen 5 : commande, nombre d'arguments, arguments.
            // Gen 6 : nombre de mots (commande comprise), commande, arguments.
            let (cmd, argc) = match variant {
                Variant::Gen5 => (codes.get(i).copied().unwrap_or(0), codes.get(i + 1).copied().unwrap_or(0) as usize),
                Variant::Gen6 => (codes.get(i + 1).copied().unwrap_or(0), (codes.get(i).copied().unwrap_or(1) as usize).saturating_sub(1)),
            };
            // Un nombre d'arguments nul est impossible à réécrire en Gen 6 (compteur 0) : on garde le code brut.
            if variant == Variant::Gen6 && codes.get(i) == Some(&0) {
                push_escape(&mut out, "X", &[c]);
                continue;
            }
            let args = codes.get(i + 2..(i + 2 + argc).min(codes.len())).unwrap_or(&[]);
            let mut values = vec![cmd];
            values.extend_from_slice(args);
            push_escape(&mut out, "VAR", &values);
            i += 2 + argc;
            continue;
        }
        match special.iter().find(|(code, _)| *code == c) {
            Some(&(_, ch)) => out.push(ch),
            None => match char::from_u32(c as u32).filter(|ch| *ch != '{' && !ch.is_control() && !SPECIAL_LOOKALIKES.contains(&c)) {
                Some(ch) => out.push(ch),
                None => push_escape(&mut out, "X", &[c]),
            },
        }
    }
    out
}

pub fn encode_with(text: &str, variant: Variant) -> Result<Vec<u16>, TextError> {
    let special = variant.special();
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            let (tag, values, used) = parse_escape(rest)?;
            match (tag, values.as_slice()) {
                ("X", [code]) => out.push(*code),
                ("VAR", [cmd, args @ ..]) => {
                    match variant {
                        Variant::Gen5 => out.extend([variant.command(), *cmd, args.len() as u16]),
                        Variant::Gen6 => out.extend([variant.command(), args.len() as u16 + 1, *cmd]),
                    }
                    out.extend_from_slice(args);
                }
                _ => return Err(TextError::BadEscape(rest[..used].into())),
            }
            rest = &rest[used..];
            continue;
        }
        match c {
            '\n' => out.push(variant.newline()),
            _ => match special.iter().find(|(_, ch)| *ch == c) {
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
    out.push(variant.end());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypt_roundtrip() {
        let codes = encode("Salamèche").unwrap();
        let enc = encrypt(&codes, DEFAULT_KEY);
        assert_eq!(decrypt_from_end(&enc, 0xFFFF), (codes, DEFAULT_KEY));
    }

    #[test]
    fn file_roundtrip() {
        for variant in [Variant::Gen5, Variant::Gen6] {
            let mut file = MsgFile::new(variant);
            let strings = vec!["Œuf".to_string(), "Nidoran♀".into(), "{VAR:0100,0000} gagne !\nBravo".into()];
            file.set_strings(&strings).unwrap();
            let parsed = MsgFile::parse_with(&file.to_bytes(), variant).unwrap();
            assert_eq!(parsed.strings(), strings);
            assert_eq!(parsed.to_bytes(), file.to_bytes());
        }
    }

    #[test]
    fn gen6_codes() {
        let codes = encode_with("Nidoran♂\n{VAR:BE01,0010}", Variant::Gen6).unwrap();
        assert_eq!(codes[7], 0xE08E);
        assert_eq!(&codes[8..], &[0x000A, 0x0010, 2, 0xBE01, 0x0010, 0x0000]);
        assert_eq!(decode_with(&codes, Variant::Gen6), "Nidoran♂\n{VAR:BE01,0010}");
        // Gen 6 : la clé de chaque ligne ne dépend que de son numéro.
        assert_eq!(Variant::Gen6.line_key(1), 0x7C89 + 0x2983);
    }
}
