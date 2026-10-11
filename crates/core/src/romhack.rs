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
use std::io::{Read, Seek, SeekFrom, Write};

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

/// Retouche de code ajoutée par Kaleido après le patch officiel : quelques octets d'un
/// overlay non compressé, vérifiés avant d'être remplacés.
#[derive(Debug, Clone, Copy)]
pub struct CodePatch {
    pub overlay: u32,
    /// Adresse en mémoire du premier octet modifié.
    pub address: u32,
    pub original: &'static [u8],
    pub patched: &'static [u8],
}

/// Ajustements Kaleido d'un hack (appliqués à l'installation, ou sur place à une ROM
/// déjà installée quand ils ont changé).
#[derive(Debug, Clone, Copy)]
pub struct Tweaks {
    /// Ce qui change pour le joueur.
    pub label: &'static str,
    pub patches: &'static [CodePatch],
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
    pub tweaks: Option<Tweaks>,
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
    // hg-engine remplace le test du jeu (ARM9 0x02070068) par le sien, dans l'overlay 129 :
    // `SHINY_VALUE <= SHINY_ODDS` avec SHINY_ODDS = 8 (9 / 65 536). On passe à 127 :
    // 128 / 65 536 = 1 / 512. La boucle qui empêche certains Pokémon d'être chromatiques
    // (même comparaison) suit le même seuil.
    tweaks: Some(Tweaks {
        label: "Chromatiques : 1 chance sur 512 (au lieu d'environ 1 sur 7 300)",
        patches: &[
            CodePatch { overlay: 129, address: 0x023D_B28E, original: &[0x08, 0x22], patched: &[0x7F, 0x22] }, // movs r2, #8 → #127
            CodePatch { overlay: 129, address: 0x023D_AACE, original: &[0x08, 0x2B], patched: &[0x7F, 0x2B] }, // cmp r3, #8 → #127
        ],
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

fn io_err(e: std::io::Error) -> RomhackError {
    RomhackError::Invalid(e.to_string())
}

fn read_u32<R: Read + Seek>(r: &mut R, at: u64) -> Result<u32> {
    let mut b = [0u8; 4];
    r.seek(SeekFrom::Start(at)).map_err(io_err)?;
    r.read_exact(&mut b).map_err(io_err)?;
    Ok(u32::from_le_bytes(b))
}

/// Position dans le fichier .nds du premier octet d'une retouche (lecture de l'en-tête,
/// de la table des overlays et de la FAT seulement).
fn patch_offset<R: Read + Seek>(r: &mut R, p: &CodePatch) -> Result<u64> {
    let (fat, ovt, ovt_size) = (read_u32(r, 0x48)? as u64, read_u32(r, 0x50)? as u64, read_u32(r, 0x54)? as u64);
    for entry in (ovt..ovt + ovt_size).step_by(32) {
        if read_u32(r, entry)? != p.overlay {
            continue;
        }
        let ram = read_u32(r, entry + 4)?;
        let file_id = read_u32(r, entry + 24)? as u64;
        if read_u32(r, entry + 28)? & 0x0100_0000 != 0 {
            return Err(RomhackError::Invalid(format!("overlay {} compressé", p.overlay)));
        }
        let (start, end) = (read_u32(r, fat + file_id * 8)?, read_u32(r, fat + file_id * 8 + 4)?);
        let rel = p.address.checked_sub(ram).filter(|&o| start + o + p.original.len() as u32 <= end);
        return rel.map(|o| (start + o) as u64).ok_or_else(|| RomhackError::Invalid(format!("adresse {:#x} hors de l'overlay {}", p.address, p.overlay)));
    }
    Err(RomhackError::Invalid(format!("overlay {} introuvable", p.overlay)))
}

/// Les ajustements Kaleido sont-ils déjà dans cette ROM ? Erreur si le code ne
/// ressemble ni à l'original ni à la version retouchée (autre version du hack).
pub fn tweaks_applied<R: Read + Seek>(r: &mut R, def: &HackDef) -> Result<bool> {
    let Some(t) = def.tweaks else { return Ok(true) };
    let mut all = true;
    for p in t.patches {
        let at = patch_offset(r, p)?;
        let mut cur = vec![0u8; p.original.len()];
        r.seek(SeekFrom::Start(at)).map_err(io_err)?;
        r.read_exact(&mut cur).map_err(io_err)?;
        if cur == p.original {
            all = false;
        } else if cur != p.patched {
            return Err(RomhackError::Invalid("code inattendu : cette ROM n'est pas la version du hack prévue".into()));
        }
    }
    Ok(all)
}

/// Applique les ajustements Kaleido (sur place). Renvoie `true` si la ROM a changé.
pub fn apply_tweaks<R: Read + Write + Seek>(r: &mut R, def: &HackDef) -> Result<bool> {
    if tweaks_applied(r, def)? {
        return Ok(false);
    }
    for p in def.tweaks.map(|t| t.patches).unwrap_or_default() {
        let at = patch_offset(r, p)?;
        r.seek(SeekFrom::Start(at)).map_err(io_err)?;
        r.write_all(p.patched).map_err(io_err)?;
    }
    Ok(true)
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
    fn ajustements_sur_place() {
        let def = &HACKS[0];
        let t = def.tweaks.unwrap();
        // En-tête + table des overlays (une entrée, overlay 129) + FAT + overlay.
        let (fat, ovt, code, ram) = (0x200u32, 0x300u32, 0x1000u32, 0x023D_8000u32);
        let mut rom = vec![0u8; 0x1000 + 0x6400];
        let put = |rom: &mut Vec<u8>, at: u32, v: u32| rom[at as usize..at as usize + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut rom, 0x48, fat);
        put(&mut rom, 0x50, ovt);
        put(&mut rom, 0x54, 32);
        put(&mut rom, ovt, 129);
        put(&mut rom, ovt + 4, ram);
        put(&mut rom, ovt + 24, 3);
        put(&mut rom, fat + 24, code);
        put(&mut rom, fat + 28, code + 0x6400);
        for p in t.patches {
            let at = (code + p.address - ram) as usize;
            rom[at..at + 2].copy_from_slice(p.original);
        }
        let mut cur = std::io::Cursor::new(rom);
        assert!(!tweaks_applied(&mut cur, def).unwrap());
        assert!(apply_tweaks(&mut cur, def).unwrap());
        assert!(tweaks_applied(&mut cur, def).unwrap());
        assert!(!apply_tweaks(&mut cur, def).unwrap());
        let rom = cur.into_inner();
        assert_eq!(&rom[(code + 0x023D_B28E - ram) as usize..][..2], &[0x7F, 0x22]);
        // Code inconnu : refus.
        let mut bad = rom.clone();
        bad[(code + 0x023D_B28E - ram) as usize] = 0x10;
        assert!(tweaks_applied(&mut std::io::Cursor::new(bad), def).is_err());
    }

    #[test]
    fn echappements() {
        assert_eq!(unescape("a\\nb\\rc\\fd"), "a\nb\rc\u{c}d");
        assert_eq!(unescape("{VAR:0101,0000,0000}!"), "{VAR:0101,0000,0000}!");
    }
}
