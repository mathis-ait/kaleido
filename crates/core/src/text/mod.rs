//! Textes des jeux : décodage, chiffrement et réécriture des fichiers de messages.
//!
//! Les codes de contrôle sont représentés entre accolades, pour pouvoir
//! réencoder un texte à l'identique :
//! - `{VAR:0100,0000}` : commande (identifiant puis arguments, en hexadécimal) ;
//! - `{X:01E0}` : caractère sans équivalent Unicode connu.
//!
//! `\n` = retour à la ligne, `\r` = défilement, `\f` = nouvelle boîte de dialogue.

pub mod gen4;
pub mod gen5;

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("fichier de texte invalide : {0}")]
    Invalid(&'static str),
    #[error("caractère « {0} » impossible à encoder dans ce jeu")]
    Unencodable(char),
    #[error("code de contrôle mal formé : {0}")]
    BadEscape(String),
}

/// Écrit `{TAG:a,b,c}` avec des valeurs hexadécimales sur 4 chiffres.
fn push_escape(out: &mut String, tag: &str, values: &[u16]) {
    out.push('{');
    out.push_str(tag);
    out.push(':');
    let hex: Vec<String> = values.iter().map(|v| format!("{v:04X}")).collect();
    out.push_str(&hex.join(","));
    out.push('}');
}

/// Lit une séquence `{TAG:...}` au début de `s` (qui commence par `{`).
/// Renvoie le tag, les valeurs et le nombre d'octets consommés.
fn parse_escape(s: &str) -> Result<(&str, Vec<u16>, usize), TextError> {
    let end = s.find('}').ok_or_else(|| TextError::BadEscape(s.chars().take(20).collect()))?;
    let body = &s[1..end];
    let (tag, values) = body.split_once(':').ok_or_else(|| TextError::BadEscape(body.into()))?;
    let values = values
        .split(',')
        .filter(|v| !v.is_empty())
        .map(|v| u16::from_str_radix(v, 16).map_err(|_| TextError::BadEscape(body.into())))
        .collect::<Result<_, _>>()?;
    Ok((tag, values, end + 1))
}

/// Décode une chaîne compressée « 9 bits » (code `0xF100`) : les caractères sont
/// empaquetés par 9 bits dans les `word_bits` bits de poids faible des mots
/// suivants, `0x1FF` termine. Gen 4 : 15 bits (le bit 15 n'est pas utilisé ;
/// vérifié sur les noms des dresseurs de Platine, fichier 618) ; Gen 5 : 16 bits
/// (noms des dresseurs de Blanche, fichier 190).
fn unpack_9bit(words: &[u16], word_bits: u32) -> Vec<u16> {
    let mut out = Vec::new();
    let mask = (1u32 << word_bits) - 1;
    let (mut container, mut bits) = (0u32, 0u32);
    for &w in words {
        container |= (w as u32 & mask) << bits;
        bits += word_bits;
        while bits >= 9 {
            let c = (container & 0x1FF) as u16;
            if c == 0x1FF {
                return out;
            }
            out.push(c);
            container >>= 9;
            bits -= 9;
        }
    }
    out
}
