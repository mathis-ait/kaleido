//! Pokémon de départ.
//!
//! - Platine : tableau de 3 espèces u32 dans l'overlay 78, à l'offset 0x1BC0.
//! - Diamant / Perle : même tableau dans l'overlay 64, à l'offset 0x1B88 (UPR-ZX
//!   `StarterPokemonOffset` ; vérifié sur Diamant ADAF : 387, 390, 393), même code
//!   d'affichage et mêmes scripts du rival que Platine (autres fichiers).
//! - HeartGold / SoulSilver (non vérifié, d'après UPR-ZX) : 3 espèces u32 dans l'ARM9,
//!   13 octets avant le motif `hgssStarterCodeSuffix` ; cris dans l'overlay 61.
//! - Noire/Blanche, trois endroits :
//!   - overlay 223 : tableau de 3 espèces u16 de l'écran de choix ;
//!   - script 304 : commandes `57 00 00 <espèce>` (sprite / cri sur l'écran de choix) ;
//!   - script 782 : le **don** réel, `28 00 21 80 <espèce>` (variable 0x8021 = espèce
//!     reçue) suivi de `57 00 01 <espèce>` (affichage du Pokémon reçu).
//!     Mêmes emplacements que l'Universal Pokémon Randomizer (782:639, 782:644…).

use crate::rom::{GameRom, RomError};
use kaleido_formats::narc::Narc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterLocation {
    Platinum,
    DiamondPearl,
    HeartGoldSoulSilver,
    BlackWhite,
}

const PT_OVERLAY: u32 = 78;
const PT_OFFSET: usize = 0x1BC0;
const PT_ORIGINAL: [u16; 3] = [387, 390, 393];
const DP_OVERLAY: u32 = 64;
const DP_OFFSET: usize = 0x1B88;

/// HeartGold / SoulSilver (UPR-ZX `Gen4Constants`).
const HGSS_CODE_SUFFIX: [u8; 8] = [0x03, 0x03, 0x1A, 0x12, 0x01, 0x23, 0x00, 0x00];
const HGSS_ORIGINAL: [u16; 3] = [152, 155, 158];
const HGSS_STARTER_OVERLAY: u32 = 61;
const HGSS_CRIES_PREFIX: [u8; 32] = [
    0x00, 0x04, 0x00, 0x0C, 0x10, 0xBD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
];

/// Position du tableau des starters de HGSS dans l'ARM9 décompressé.
fn hgss_table(arm9: &[u8]) -> Option<usize> {
    find_unique(arm9, &HGSS_CODE_SUFFIX).and_then(|p| p.checked_sub(13)).filter(|&p| p + 10 <= arm9.len())
}

const BW_OVERLAY: u32 = 223;
const BW_OFFSET: usize = 0x3170;
const BW_ORIGINAL: [u16; 3] = [495, 498, 501];
const BW_SCRIPTS: &str = "a/0/5/7";
const BW_SCRIPT: usize = 304;
const BW_GIFT_SCRIPT: usize = 782;
/// `setvar 0x8021, <espèce>` : l'espèce effectivement donnée au joueur.
const BW_GIFT_SETVAR: [u8; 4] = [0x28, 0x00, 0x21, 0x80];
const BW_GIFT_SHOW: [u8; 3] = [0x57, 0x00, 0x01];

/// Remplace `prefix + orig` par `prefix + new` dans `script` ; renvoie le nombre de remplacements.
fn replace_after(script: &mut [u8], prefix: &[u8], orig: u16, new: u16) -> usize {
    let mut pattern = prefix.to_vec();
    pattern.extend(orig.to_le_bytes());
    let mut count = 0;
    let mut at = 0;
    while let Some(pos) = script[at..].windows(pattern.len()).position(|w| w == pattern.as_slice()) {
        let p = at + pos + prefix.len();
        script[p..p + 2].copy_from_slice(&new.to_le_bytes());
        count += 1;
        at = p + 2;
    }
    count
}

/// Espèces réellement données par le script 782 (dans l'ordre Plante, Feu, Eau).
pub fn read_bw_gifts(game: &GameRom) -> Result<Vec<u16>, RomError> {
    let narc = Narc::parse(game.rom().file_by_path(BW_SCRIPTS)?)?;
    let script = narc.files.get(BW_GIFT_SCRIPT).ok_or_else(mismatch)?;
    Ok(script.windows(6).filter(|w| w[..4] == BW_GIFT_SETVAR).map(|w| u16::from_le_bytes([w[4], w[5]])).collect())
}

fn mismatch() -> RomError {
    RomError::Layout("emplacement des starters inattendu (ROM modifiée ?)".into())
}

pub fn read(game: &GameRom, location: StarterLocation) -> Result<[u16; 3], RomError> {
    let mut out = [0; 3];
    match location {
        StarterLocation::Platinum | StarterLocation::DiamondPearl => {
            let (overlay, offset) = if location == StarterLocation::Platinum { (PT_OVERLAY, PT_OFFSET) } else { (DP_OVERLAY, DP_OFFSET) };
            let ovl = game.rom().overlay(overlay)?;
            for (i, s) in out.iter_mut().enumerate() {
                let at = offset + i * 4;
                *s = u32::from_le_bytes(ovl.get(at..at + 4).ok_or_else(mismatch)?.try_into().unwrap()) as u16;
            }
        }
        StarterLocation::HeartGoldSoulSilver => {
            let arm9 = game.rom().arm9_decompressed()?;
            let at = hgss_table(&arm9).ok_or_else(mismatch)?;
            for (i, s) in out.iter_mut().enumerate() {
                *s = u16::from_le_bytes([arm9[at + i * 4], arm9[at + i * 4 + 1]]);
            }
        }
        StarterLocation::BlackWhite => {
            let ovl = game.rom().overlay(BW_OVERLAY)?;
            for (i, s) in out.iter_mut().enumerate() {
                let at = BW_OFFSET + i * 2;
                *s = u16::from_le_bytes(ovl.get(at..at + 2).ok_or_else(mismatch)?.try_into().unwrap());
            }
        }
    }
    if out.iter().any(|&s| s == 0 || s > 700) {
        return Err(mismatch());
    }
    Ok(out)
}

/// Platine, Diamant et Perle : portage de `Gen4RomHandler.setStarters` de l'Universal
/// Pokémon Randomizer (code ARM de l'écran de choix, scripts du rival et des combats
/// en duo, textes).
mod platinum {
    use super::{find_unique, mismatch, GameRom, Narc, RomError};

    /// Fichiers propres à chaque jeu (UPR-ZX `gen4_offsets.ini` et `Gen4Constants`).
    pub(super) struct Config {
        scripts: &'static str,
        text_archive: &'static str,
        rival_files: &'static [usize],
        /// Premières commandes possibles du script visé par le saut (combat du rival…).
        rival_targets: &'static [u16],
        tag_files: &'static [usize],
        starter_screen_text: usize,
        pokedex_category_text: usize,
    }

    pub(super) const PLATINUM: Config = Config {
        scripts: "fielddata/script/scr_seq.narc",
        text_archive: "msgdata/pl_msg.narc",
        rival_files: &[31, 36, 112, 123, 186, 427, 429, 1096],
        rival_targets: &[0xE5, 0x28F, 0x125],
        tag_files: &[2, 136, 201, 236],
        starter_screen_text: 360,
        pokedex_category_text: 711,
    };

    /// Diamant / Perle (`dpFilesWithRivalScript`, `dpFilesWithTagScript` ; textes 320 et
    /// 621 vérifiés sur Diamant ADAF).
    pub(super) const DIAMOND_PEARL: Config = Config {
        scripts: "fielddata/script/scr_seq_release.narc",
        text_archive: "msgdata/msg.narc",
        rival_files: &[34, 90, 118, 180, 195, 394],
        rival_targets: &[0xE5, 0x28F],
        tag_files: &[2, 131, 230],
        starter_screen_text: 320,
        pokedex_category_text: 621,
    };

    const GRAPHICS_PREFIX: [u8; 8] = [0x00, 0x02, 0x22, 0x40, 0x21, 0x04, 0x12, 0x0C];
    const GRAPHICS_PREFIX_INNER: [u8; 8] = [0x02, 0x90, 0x03, 0x90, 0x02, 0x20, 0x00, 0x02];
    const RIVAL_MAGIC: [u8; 13] = [0xDE, 0x00, 0x0C, 0x80, 0x11, 0x00, 0x0C, 0x80, 0x83, 0x01, 0x1C, 0x00, 0x01];
    const TAG_MAGIC1: [u8; 8] = [0xDE, 0x00, 0x0C, 0x80, 0x28, 0x00, 0x04, 0x80];
    const TAG_MAGIC2: [u8; 9] = [0x11, 0x00, 0x0C, 0x80, 0x86, 0x01, 0x1C, 0x00, 0x01];
    const TURTWIG: u16 = 387;

    fn rd(d: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([d[at], d[at + 1]])
    }

    fn wr(d: &mut [u8], at: usize, v: u16) {
        d[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn rd32(d: &[u8], at: usize) -> i32 {
        i32::from_le_bytes(d[at..at + 4].try_into().unwrap())
    }

    /// L'écran de choix calcule l'index du sprite par décalages : on insère des
    /// instructions Thumb `add`/`sub` pour viser n'importe quelle espèce (comme l'UPR).
    pub(super) fn patch_graphics_code(d: &mut [u8], starters: [u16; 3]) -> Result<(), RomError> {
        let Some(mut off) = find_unique(d, &GRAPHICS_PREFIX).filter(|&o| o > 0) else { return Err(mismatch()) };
        off += GRAPHICS_PREFIX.len();
        if off + 0x20 + 3 * 0xE + 1 > d.len() {
            return Err(mismatch());
        }
        let v = rd(d, off + 0xA);
        wr(d, off + 0xC, v);
        if off % 4 == 0 {
            d[off + 0xC] = d[off + 0xC].wrapping_sub(1);
        }
        let v = rd(d, off + 0x8);
        wr(d, off + 0xA, v);
        d[off + 0xA] = d[off + 0xA].wrapping_sub(1);
        for at in [0x8, 0x6, 0x4] {
            let v = rd(d, off + at - 2);
            wr(d, off + at, v);
        }
        wr(d, off + 0x2, 0x6828);
        wr(d, off, 0x182D);
        off += 0x16;
        wr(d, off, 0x6828);
        off += 0xA;
        for (i, &s) in starters.iter().enumerate() {
            let base = 4 * (i as i32 + 1);
            let mut diff = s as i32 - base;
            let (mut instr1, mut instr2) = (0x3200u16, 0x3200u16);
            if diff < 0 {
                instr1 |= 0x800;
                diff = -diff;
            } else if diff > 255 {
                instr2 |= 0xFF;
                diff -= 255;
            }
            instr1 |= (diff & 0xFF) as u16;
            d[off] = base as u8;
            let v = rd(d, off + 4);
            wr(d, off + 2, v);
            wr(d, off + 4, instr1);
            wr(d, off + 8, instr2);
            off += 0xE;
        }
        d[off] = 1;
        let Some(inner) = find_unique(d, &GRAPHICS_PREFIX_INNER).filter(|&o| o > 0) else { return Err(mismatch()) };
        d[inner + GRAPHICS_PREFIX_INNER.len() + 1] = 0x68;
        Ok(())
    }

    /// Scripts du rival (« si starter == Tortipouss… ») et combats en duo.
    pub(super) fn patch_scripts(game: &mut GameRom, cfg: &Config, starters: [u16; 3]) -> Result<(), RomError> {
        let mut narc = Narc::parse(game.rom().file_by_path(cfg.scripts)?)?;
        let positions = |f: &[u8], m: &[u8]| -> Vec<usize> { f.windows(m.len()).enumerate().filter(|(_, w)| *w == m).map(|(i, _)| i).collect() };
        let target = |f: &[u8], jump_loc: usize| -> Option<usize> {
            let jump_to = rd32(f, jump_loc) as i64 + jump_loc as i64 + 4;
            usize::try_from(jump_to).ok().filter(|&t| t + 2 <= f.len())
        };

        for &n in cfg.rival_files {
            let Some(file) = narc.files.get_mut(n) else { continue };
            for base in positions(file, &RIVAL_MAGIC) {
                let jump_loc = base + RIVAL_MAGIC.len();
                if jump_loc + 4 > file.len() || base + 0x17 > file.len() {
                    continue;
                }
                let Some(to) = target(file, jump_loc) else { continue };
                if !cfg.rival_targets.contains(&rd(file, to)) {
                    continue; // pas un script du rival
                }
                wr(file, base + 0x8, starters[0]);
                wr(file, base + 0x15, starters[1]);
            }
        }

        for &n in cfg.tag_files {
            let Some(file) = narc.files.get_mut(n) else { continue };
            for base in positions(file, &TAG_MAGIC1) {
                let second = base + TAG_MAGIC1.len() + 2;
                let jump_loc = second + TAG_MAGIC2.len();
                if jump_loc + 4 > file.len() || base + 0x23 > file.len() || file[second..jump_loc] != TAG_MAGIC2 {
                    continue;
                }
                let Some(to) = target(file, jump_loc) else { continue };
                if rd(file, to) != 0x1B {
                    continue; // pas un combat en duo
                }
                let first_or_third = if rd(file, base + 0x21) == TURTWIG { starters[0] } else { starters[2] };
                wr(file, base + 0x21, first_or_third);
                wr(file, base + 0xE, starters[1]);
            }
        }
        game.rom_mut().replace_file_by_path(cfg.scripts, narc.to_bytes())?;
        Ok(())
    }

    /// Écran de choix : « {catégorie} {nom} » ; la question qui suit reste celle du jeu,
    /// ce qui fonctionne dans toutes les langues.
    pub(super) fn patch_texts(game: &mut GameRom, cfg: &Config, starters: [u16; 3]) -> Result<(), RomError> {
        use crate::text::gen4::MsgFile;
        let names = game.text_file(game.layout.species_names)?;
        let categories = game.text_file(cfg.pokedex_category_text)?;
        let mut narc = Narc::parse(game.rom().file_by_path(cfg.text_archive)?)?;
        let Some(file) = narc.files.get_mut(cfg.starter_screen_text) else { return Ok(()) };
        let msg = MsgFile::parse(file)?;
        let mut lines = msg.strings();
        const RESET: &str = "{VAR:FF00,0000}";
        for (i, &s) in starters.iter().enumerate() {
            let (Some(line), Some(name), Some(category)) = (lines.get(i + 1), names.get(s as usize), categories.get(s as usize)) else { continue };
            let color = if i == 0 { 3 } else { i };
            let first_line = line.split('\n').next().unwrap_or("");
            let Some(tail) = first_line.rfind(RESET).map(|p| &line[p..]) else { continue };
            let new = format!("{{VAR:FF00,{color:04X}}}{category} {name}{tail}");
            lines[i + 1] = new;
        }
        *file = MsgFile::from_strings(msg.seed, &lines)?.to_bytes();
        game.rom_mut().replace_file_by_path(cfg.text_archive, narc.to_bytes())?;
        Ok(())
    }
}

/// HeartGold / SoulSilver : portage de la branche HGSS de `Gen4RomHandler.setStarters`
/// (UPR-ZX). Non vérifié sur une vraie ROM : chaque motif est contrôlé avant écriture.
mod hgss {
    use super::{find_unique, mismatch, GameRom, Narc, RomError, StarterLabels, HGSS_CRIES_PREFIX, HGSS_STARTER_OVERLAY};

    const SCRIPTS: &str = "a/0/1/2";
    const RIVAL_FILES: [usize; 7] = [7, 23, 96, 110, 819, 850, 866];
    /// `StoreStarter2 0x800C ; If 0x800C == 152 ; CheckLR B_!=` (`hgssRivalScriptMagic`).
    const RIVAL_MAGIC: [u8; 13] = [0xCE, 0x00, 0x0C, 0x80, 0x11, 0x00, 0x0C, 0x80, 0x98, 0x00, 0x1C, 0x00, 0x05];
    const CYNDAQUIL: u8 = 155;
    const STARTER_SCREEN_TEXT: usize = 190;

    /// Scripts du rival : « si starter == Germignon… sinon si starter == Héricendre… ».
    pub(super) fn patch_scripts(game: &mut GameRom, starters: [u16; 3]) -> Result<(), RomError> {
        let mut narc = Narc::parse(game.rom().file_by_path(SCRIPTS)?)?;
        for &n in &RIVAL_FILES {
            let Some(file) = narc.files.get_mut(n) else { continue };
            let Some(base) = find_unique(file, &RIVAL_MAGIC) else { continue };
            if base + 17 > file.len() {
                continue;
            }
            file[base + 8..base + 10].copy_from_slice(&starters[0].to_le_bytes());
            let jump = i32::from_le_bytes(file[base + 13..base + 17].try_into().unwrap()) as i64;
            let Some(second) = usize::try_from(jump + base as i64 + 17).ok().filter(|&s| s + 6 <= file.len()) else { continue };
            if file[second] == 0x11 && file[second + 4] == CYNDAQUIL {
                file[second + 4..second + 6].copy_from_slice(&starters[1].to_le_bytes());
            }
        }
        game.rom_mut().replace_file_by_path(SCRIPTS, narc.to_bytes())?;
        Ok(())
    }

    /// Cris de l'écran de choix (overlay 61, `starterCriesPrefix`).
    pub(super) fn patch_cries(game: &mut GameRom, starters: [u16; 3]) -> Result<(), RomError> {
        let mut ovl = game.rom().overlay(HGSS_STARTER_OVERLAY)?;
        let Some(at) = find_unique(&ovl, &HGSS_CRIES_PREFIX).map(|p| p + HGSS_CRIES_PREFIX.len()).filter(|&p| p + 12 <= ovl.len()) else {
            return Err(mismatch());
        };
        for (i, &s) in starters.iter().enumerate() {
            ovl[at + i * 4..at + i * 4 + 4].copy_from_slice(&(s as u32).to_le_bytes());
        }
        game.rom_mut().replace_overlay(HGSS_STARTER_OVERLAY, ovl)?;
        Ok(())
    }

    /// Textes du Prof. Orme sur l'écran de choix (lignes 1-3 et 4-6 du fichier 190).
    pub(super) fn patch_texts(game: &mut GameRom, labels: &StarterLabels) -> Result<(), RomError> {
        use crate::text::gen4::MsgFile;
        let archive = game.layout.text_archive;
        let mut narc = Narc::parse(game.rom().file_by_path(archive)?)?;
        let Some(file) = narc.files.get_mut(STARTER_SCREEN_TEXT) else { return Ok(()) };
        let msg = MsgFile::parse(file)?;
        let mut lines = msg.strings();
        if lines.len() < 7 {
            return Ok(());
        }
        for (i, (type_name, name)) in labels.iter().enumerate() {
            let color = if i == 0 { 3 } else { i };
            let shown = format!("{{VAR:FF00,{color:04X}}}{name}{{VAR:FF00,0000}}");
            lines[i + 1] = format!("Prof. Orme : tu choisis {shown},\nle Pokémon de type {type_name} ?");
            lines[i + 4] = format!("{shown}, le Pokémon de type\n{type_name}, est dans cette Poké Ball !");
        }
        *file = MsgFile::from_strings(msg.seed, &lines)?.to_bytes();
        game.rom_mut().replace_file_by_path(archive, narc.to_bytes())?;
        Ok(())
    }
}

/// Libellés des starters pour les textes du jeu : (type principal, nom), en français.
pub type StarterLabels = [(String, String); 3];

/// Script du Pokédex (Noire/Blanche) : correctif appliqué par l'Universal Pokémon
/// Randomizer dès que les starters changent (Gen5RomHandler.setStarters, « GIVE ME
/// BACK MY PURRLOIN ») — un saut vers une copie du script ajoutée en fin de fichier.
const BW_POKEDEX_SCRIPT: usize = 792;
const BW_POKEDEX_MAGIC: [u8; 4] = [0x24, 0x00, 0xA7, 0x02];
const BW1_NEW_STARTER_SCRIPT: [u8; 16] = [0x24, 0x00, 0xA7, 0x02, 0xE7, 0x00, 0x00, 0x00, 0xDE, 0x00, 0x00, 0x00, 0xF8, 0x01, 0x05, 0x00];
/// Images de l'écran de choix et sprites des Pokémon (20 fichiers par espèce).
const BW_STARTER_GRAPHICS: &str = "a/2/0/5";
const BW_POKEMON_GRAPHICS: &str = "a/0/0/4";
/// Table des cris de l'écran de choix (overlay 223), repérée par ce préfixe.
const BW_CRY_PREFIX: [u8; 7] = [0x08, 0x0A, 0x07, 0x00, 0x08, 0x00, 0x00];
/// Textes « Pokémon de type … » : fichier 430 des textes de l'histoire, lignes 18, 17, 16.
const BW_STORY_TEXT: &str = "a/0/0/3";
const BW_STARTER_TEXT_FILE: usize = 430;
const BW_STARTER_TEXT_LINE: usize = 18;

/// Position unique de `pattern`, sinon `None`.
fn find_unique(data: &[u8], pattern: &[u8]) -> Option<usize> {
    let mut hits = data.windows(pattern.len()).enumerate().filter(|(_, w)| *w == pattern).map(|(i, _)| i);
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

/// Écrit les nouveaux starters. Ne fonctionne que sur une ROM dont les starters
/// sont encore ceux d'origine (on part toujours de la ROM originale).
/// Pour Noire/Blanche, suit `Gen5RomHandler.setStarters` de l'Universal Pokémon Randomizer.
pub fn write(game: &mut GameRom, location: StarterLocation, starters: [u16; 3], labels: &StarterLabels) -> Result<(), RomError> {
    match location {
        StarterLocation::Platinum | StarterLocation::DiamondPearl => {
            if read(game, location)? != PT_ORIGINAL {
                return Err(mismatch());
            }
            let (overlay, offset, cfg) = if location == StarterLocation::Platinum {
                (PT_OVERLAY, PT_OFFSET, &platinum::PLATINUM)
            } else {
                (DP_OVERLAY, DP_OFFSET, &platinum::DIAMOND_PEARL)
            };
            let mut ovl = game.rom().overlay(overlay)?;
            for (i, s) in starters.iter().enumerate() {
                let at = offset + i * 4;
                ovl[at..at + 4].copy_from_slice(&(*s as u32).to_le_bytes());
            }
            platinum::patch_graphics_code(&mut ovl, starters)?;
            game.rom_mut().replace_overlay(overlay, ovl)?;
            platinum::patch_scripts(game, cfg, starters)?;
            platinum::patch_texts(game, cfg, starters)?;
        }
        StarterLocation::HeartGoldSoulSilver => {
            if read(game, location)? != HGSS_ORIGINAL {
                return Err(mismatch());
            }
            let mut arm9 = game.rom().arm9_decompressed()?;
            let at = hgss_table(&arm9).ok_or_else(mismatch)?;
            for (i, s) in starters.iter().enumerate() {
                arm9[at + i * 4..at + i * 4 + 2].copy_from_slice(&s.to_le_bytes());
            }
            game.rom_mut().replace_arm9(&arm9)?;
            hgss::patch_scripts(game, starters)?;
            hgss::patch_texts(game, labels)?;
            hgss::patch_cries(game, starters)?;
        }
        StarterLocation::BlackWhite => {
            if read(game, location)? != BW_ORIGINAL {
                return Err(mismatch());
            }
            // Cris de l'écran de choix (overlay 223).
            let mut ovl = game.rom().overlay(BW_OVERLAY)?;
            let table = find_unique(&ovl, &BW_CRY_PREFIX).map_or(BW_OFFSET, |p| p + BW_CRY_PREFIX.len());
            for (i, s) in starters.iter().enumerate() {
                let at = table + i * 2;
                ovl[at..at + 2].copy_from_slice(&s.to_le_bytes());
            }
            game.rom_mut().replace_overlay(BW_OVERLAY, ovl)?;

            // Images de l'écran de choix : palette et image du sprite de face de chaque espèce.
            let pokegra = Narc::parse(game.rom().file_by_path(BW_POKEMON_GRAPHICS)?)?;
            let mut gfx = Narc::parse(game.rom().file_by_path(BW_STARTER_GRAPHICS)?)?;
            for (i, &s) in starters.iter().enumerate() {
                let base = s as usize * 20;
                let (Some(palette), Some(picture)) = (pokegra.files.get(base + 18), pokegra.files.get(base)) else {
                    return Err(mismatch());
                };
                if gfx.files.len() < 15 {
                    return Err(mismatch());
                }
                gfx.files[i * 2] = palette.clone();
                gfx.files[12 + i] = kaleido_formats::lz::decompress(picture)?;
            }
            game.rom_mut().replace_file_by_path(BW_STARTER_GRAPHICS, gfx.to_bytes())?;

            // Textes « Pokémon de type … » présentant chaque starter.
            let mut story = Narc::parse(game.rom().file_by_path(BW_STORY_TEXT)?)?;
            if let Some(file) = story.files.get_mut(BW_STARTER_TEXT_FILE) {
                let mut msg = crate::text::gen5::MsgFile::parse(file)?;
                let mut lines = msg.strings();
                for (i, (type_name, name)) in labels.iter().enumerate() {
                    if let Some(line) = lines.get_mut(BW_STARTER_TEXT_LINE - i) {
                        *line = format!("{{VAR:BD02}} Pokémon de type {type_name}\n{{VAR:BD02}}{name}");
                    }
                }
                msg.set_strings(&lines)?;
                *file = msg.to_bytes();
                game.rom_mut().replace_file_by_path(BW_STORY_TEXT, story.to_bytes())?;
            }

            let mut narc = Narc::parse(game.rom().file_by_path(BW_SCRIPTS)?)?;

            // Écran de choix : sprites et cris.
            let script = narc.files.get_mut(BW_SCRIPT).ok_or_else(mismatch)?;
            let shown: usize = BW_ORIGINAL.iter().zip(starters).map(|(&o, n)| replace_after(script, &[0x57, 0x00, 0x00], o, n)).sum();
            if shown == 0 {
                return Err(mismatch());
            }

            // Don réel : chaque starter doit être donné exactement une fois, sinon on refuse
            // d'écrire une ROM où le Pokémon reçu ne correspondrait pas au choix. Les
            // affichages (`57 00 01`) apparaissent plusieurs fois dans les scènes qui
            // suivent le choix : on les remplace tous.
            let gift = narc.files.get_mut(BW_GIFT_SCRIPT).ok_or_else(mismatch)?;
            for (&orig, new) in BW_ORIGINAL.iter().zip(starters) {
                if replace_after(gift, &BW_GIFT_SETVAR, orig, new) != 1 || replace_after(gift, &BW_GIFT_SHOW, orig, new) == 0 {
                    return Err(RomError::Layout("script de don des starters (782) inattendu".into()));
                }
            }

            // Correctif du script du Pokédex (repris tel quel de l'Universal Pokémon Randomizer).
            let pokedex = narc.files.get_mut(BW_POKEDEX_SCRIPT).ok_or_else(mismatch)?;
            if let Some(pos) = find_unique(pokedex, &BW_POKEDEX_MAGIC).filter(|&p| p > 0) {
                let old_len = pokedex.len();
                pokedex.extend_from_slice(&BW1_NEW_STARTER_SCRIPT);
                pokedex[pos..pos + 2].copy_from_slice(&[0x04, 0x00]);
                let relative = old_len as i32 - (pos as i32 + 2 + 4);
                pokedex[pos + 2..pos + 6].copy_from_slice(&relative.to_le_bytes());
            }
            game.rom_mut().replace_file_by_path(BW_SCRIPTS, narc.to_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hgss_table_before_code_suffix() {
        // Germignon, Héricendre, Kaiminus (u32), un octet, puis le motif de l'UPR.
        let mut arm9 = vec![0xAAu8; 32];
        let table = arm9.len();
        for s in HGSS_ORIGINAL {
            arm9.extend((s as u32).to_le_bytes());
        }
        arm9.push(0);
        arm9.extend(HGSS_CODE_SUFFIX);
        arm9.extend([0; 8]);
        assert_eq!(hgss_table(&arm9), Some(table));
        // Motif en double : refus.
        let mut twice = arm9.clone();
        twice.extend(HGSS_CODE_SUFFIX);
        assert_eq!(hgss_table(&twice), None);
        assert_eq!(hgss_table(&HGSS_CODE_SUFFIX), None);
    }
}
