//! Romhacks : catalogue, application du patch officiel et traductions.
//!
//! Kaleido ne distribue ni ROM ni ROM patchée : il télécharge le patch publié par
//! l'auteur du hack et l'applique à la ROM du joueur, vérifiée par son SHA-1.
//!
//! Traduction des textes Gen 4 : chaque ligne d'une banque de messages du hack
//! vient, par ordre de priorité,
//! 1. du paquet de traduction embarqué (noms et descriptions des nouveautés,
//!    dialogues ajoutés ou réécrits par le hack) ;
//! 2. de la ROM française officielle du joueur, à la même position, quand la
//!    ligne existait déjà dans le jeu d'origine (le hack ne fait qu'ajouter des
//!    lignes en fin de banque ou reformuler l'anglais) ;
//! 3. du texte anglais du hack (marqueurs « ----- », lignes vides).

use std::collections::HashMap;

use kaleido_formats::{narc::Narc, nds::NdsRom};
use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::text::gen4::MsgFile;

#[derive(Debug, thiserror::Error)]
pub enum RomhackError {
    #[error("{0}")]
    Format(#[from] kaleido_formats::FormatError),
    #[error("{0}")]
    Text(#[from] crate::text::TextError),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, RomhackError>;

/// Traduction proposée pour un hack.
#[derive(Debug, Clone, Copy)]
pub struct Translation {
    /// Langue du résultat (« fr »).
    pub lang: &'static str,
    /// Codes de jeu acceptés comme ROM de référence officielle (même jeu, dans la langue visée).
    pub reference_codes: &'static [&'static str],
    /// Nom lisible de la ROM de référence attendue.
    pub reference_label: &'static str,
    /// Archive des messages dans la ROM.
    pub msg_path: &'static str,
    /// Paquet de traduction (JSON, voir [`TextPack`]).
    pub pack: &'static str,
}

/// Romhack proposé à l'installation.
#[derive(Debug, Clone, Copy)]
pub struct HackDef {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub summary: &'static str,
    /// Page du fil officiel (documentation, crédits).
    pub thread_url: &'static str,
    /// Image affichée comme jaquette dans la Bibliothèque (écran titre du hack).
    pub cover_url: &'static str,
    /// Lien de téléchargement publié par l'auteur (archive contenant le patch).
    pub download_page: &'static str,
    /// Extension du patch dans l'archive.
    pub patch_ext: &'static str,
    /// ROM d'origine exigée par le patch.
    pub base_label: &'static str,
    pub base_code: &'static str,
    pub base_sha1: &'static str,
    /// SHA-1 de la ROM obtenue après le patch (avant traduction).
    pub patched_sha1: &'static str,
    /// Avertissements à montrer avant de jouer.
    pub notes: &'static [&'static str],
    pub translation: Option<Translation>,
}

pub const HACKS: &[HackDef] = &[HackDef {
    id: "heartgold-generations",
    name: "HeartGold Generations",
    version: "2.0",
    author: "spearmintz",
    summary: "HeartGold avec plus de 900 Pokémon des 9 générations, Méga-Évolution, type Fée, mécaniques modernes et beaucoup de confort (hg-engine).",
    thread_url: "https://www.pokecommunity.com/threads/pok%C3%A9mon-heartgold-generations-v2-0.537551/",
    cover_url: "https://images.hackdex.app/heartgold-generations/1772673805609-0.png",
    download_page: "https://www.mediafire.com/file/5znwk83g90kigjx/HeartGold_Generations_v2.0.zip/file",
    patch_ext: "xdelta",
    base_label: "Pokémon HeartGold (USA)",
    base_code: "IPKE",
    base_sha1: "4fcded0e2713dc03929845de631d0932ea2b5a37",
    patched_sha1: "d8fcc9de74ecc7e38caed8be883bd602653c3736",
    notes: &[
        "Bug connu de hg-engine : le jeu plante au Maître de la Ligue et au Panthéon sauf avec DeSmuME. Sauvegarde avant, passe sur DeSmuME pour ce combat, puis reviens sur ton émulateur habituel.",
        "Le framerate est débloqué à 60 images/s : si le jeu va trop vite, limite la vitesse de l'émulateur à 100 %.",
    ],
    translation: Some(Translation {
        lang: "fr",
        reference_codes: &["IPKF"],
        reference_label: "Pokémon Version Or HeartGold (France)",
        msg_path: "a/0/2/7",
        pack: include_str!("../data/romhacks/heartgold-generations-fr.json"),
    }),
}];

pub fn find(id: &str) -> Option<&'static HackDef> {
    HACKS.iter().find(|h| h.id == id)
}

/// Nom du fichier créé par l'installation (rangé à côté de la ROM d'origine).
pub fn output_name(def: &HackDef, french: bool) -> String {
    format!("Pokemon - {} v{} ({}).nds", def.name, def.version, if french { "France" } else { "USA" })
}

/// Romhack installé par Kaleido, reconnu à son nom de fichier ;  = version française.
pub fn from_file_name(name: &str) -> Option<(&'static HackDef, bool)> {
    HACKS.iter().find_map(|d| [true, false].into_iter().find(|&fr| output_name(d, fr).eq_ignore_ascii_case(name)).map(|fr| (d, fr)))
}

pub fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Code de jeu (4 lettres) d'une ROM DS, lu dans l'en-tête.
pub fn nds_game_code(data: &[u8]) -> Option<String> {
    data.get(0x0C..0x10).map(|c| String::from_utf8_lossy(c).into_owned())
}

/// Applique un patch (xdelta / VCDIFF, BPS ou IPS, reconnu à sa signature).
pub fn apply_patch(base: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    if patch.starts_with(&[0xD6, 0xC3, 0xC4]) {
        return Ok(kaleido_formats::vcdiff::apply(base, patch)?);
    }
    if patch.starts_with(b"BPS1") {
        return Ok(kaleido_formats::bps::apply(base, patch)?);
    }
    if patch.starts_with(b"PATCH") {
        let mut out = base.to_vec();
        kaleido_formats::ips::apply(&mut out, patch)?;
        return Ok(out);
    }
    Err(RomhackError::Invalid("format de patch inconnu".into()))
}

/// Paquet de traduction : `banks[banque][index]` = texte, au format d'export de Kaleido
/// (`\n`, `\r`, `\f` écrits avec une barre oblique inverse, codes `{VAR:…}`).
/// `remap[banque] = autre banque` : la ROM de référence range ce texte ailleurs
/// (le Pokédex HGSS a une banque par langue ; le hack lit celle de l'anglais).
#[derive(Debug, Default, Deserialize)]
pub struct TextPack {
    #[serde(default)]
    pub remap: HashMap<String, String>,
    pub banks: HashMap<String, HashMap<String, String>>,
}

impl TextPack {
    pub fn parse(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| RomhackError::Invalid(format!("paquet de traduction invalide : {e}")))
    }
}

/// `\n` écrit → retour à la ligne réel (et `\r`, `\f`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => { chars.next(); out.push('\n'); continue; }
                Some('r') => { chars.next(); out.push('\r'); continue; }
                Some('f') => { chars.next(); out.push('\u{c}'); continue; }
                _ => {}
            }
        }
        out.push(c);
    }
    out
}

/// Bilan d'une traduction.
#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct TranslateStats {
    /// Lignes reprises du jeu officiel.
    pub official: usize,
    /// Lignes venant du paquet de traduction.
    pub pack: usize,
    /// Lignes laissées telles quelles (vides, marqueurs, anglais sans équivalent).
    pub kept: usize,
}

fn messages(rom: &NdsRom, path: &str) -> Result<Vec<MsgFile>> {
    let narc = Narc::parse(rom.file_by_path(path)?)?;
    narc.files.iter().map(|f| Ok(MsgFile::parse(f)?)).collect()
}

/// Traduit les textes d'une ROM Gen 4 patchée (`hack`) à l'aide du jeu d'origine
/// (`base`, même langue que le hack) et du même jeu dans la langue visée (`reference`).
pub fn translate_gen4(hack: &mut NdsRom, base: &NdsRom, reference: &NdsRom, msg_path: &str, pack: &TextPack) -> Result<TranslateStats> {
    let mut narc = Narc::parse(hack.file_by_path(msg_path)?)?;
    let base_msgs = messages(base, msg_path)?;
    let ref_msgs = messages(reference, msg_path)?;
    let mut stats = TranslateStats::default();

    for (b, file) in narc.files.iter_mut().enumerate() {
        let key = format!("{b:04}");
        let mut msg = MsgFile::parse(file)?;
        let hack_lines = msg.strings();
        let base_lines = base_msgs.get(b).map(MsgFile::strings).unwrap_or_default();
        let ref_bank = pack.remap.get(&key).and_then(|k| k.parse::<usize>().ok()).unwrap_or(b);
        let ref_lines = ref_msgs.get(ref_bank).map(MsgFile::strings).unwrap_or_default();
        let overrides = pack.banks.get(&key);
        let mut changed = false;

        let mut out = Vec::with_capacity(hack_lines.len());
        for (i, line) in hack_lines.iter().enumerate() {
            let text = if let Some(t) = overrides.and_then(|o| o.get(&i.to_string())) {
                stats.pack += 1;
                unescape(t)
            } else if line.trim().is_empty() && !line.is_empty() || i >= base_lines.len() || i >= ref_lines.len() {
                stats.kept += 1;
                line.clone()
            } else {
                stats.official += 1;
                ref_lines[i].clone()
            };
            changed |= text != *line;
            out.push(text);
        }
        if changed {
            msg = MsgFile::from_strings(msg.seed, &out)?;
            *file = msg.to_bytes();
        }
    }
    hack.replace_file_by_path(msg_path, narc.to_bytes())?;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paquets_embarques_valides() {
        for h in HACKS {
            if let Some(t) = h.translation {
                let pack = TextPack::parse(t.pack).unwrap();
                for lines in pack.banks.values() {
                    for s in lines.values() {
                        crate::text::gen4::encode(&unescape(s)).unwrap_or_else(|e| panic!("{}: {e} dans {s:?}", h.id));
                    }
                }
            }
        }
    }

    #[test]
    fn echappements() {
        assert_eq!(unescape("a\\nb\\rc\\fd"), "a\nb\rc\u{c}d");
        assert_eq!(unescape("{VAR:0101,0000,0000}!"), "{VAR:0101,0000,0000}!");
    }
}
