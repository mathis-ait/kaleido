//! Objets ramassables : Poké Balls posées au sol et objets cachés (Gen 4 / 5).
//!
//! Portage de `getFieldItems` / `setFieldItems` de l'Universal Pokémon Randomizer
//! (Gen4RomHandler, Gen5RomHandler) :
//!
//! - les objets visibles sont donnés par un fichier de scripts : une liste de
//!   pointeurs relatifs (`u32`, terminée par `0xFD13`) vers des scripts qui
//!   commencent par `setvar <variable>, <objet>` ;
//!   - Platine : script 404 de `fielddata/script/scr_seq.narc`,
//!     `28 00 08 80 <objet>` au début du script (scripts 25, 238, 321, 325 et 326 ignorés) ;
//!   - Noire/Blanche : script 864 de `a/0/5/7`, `28 00 0C 80 <objet>` 2 octets après le début ;
//! - les objets cachés :
//!   - Platine : table de 257 entrées de 8 octets dans l'ARM9 (`u16` objet en tête) ;
//!   - Noire/Blanche : script 865, `2A 00 00 80 <objet>` 2 octets après le début.
//!
//! Les emplacements de chaque objet sont mémorisés à la lecture : l'écriture remet
//! simplement un nouvel identifiant au même endroit (aucun fichier ne change de taille).

use kaleido_formats::narc::Narc;
use serde::Serialize;

use crate::games::Game;
use crate::rom::{GameRom, RomError};

const SCRIPT_LIST_END: u16 = 0xFD13;

/// Emplacement des objets ramassables et du texte des noms d'objets pour une ROM.
#[derive(Debug, Clone, Copy)]
pub struct ItemLayout {
    pub gen: u8,
    /// Fichier de texte des noms d'objets.
    pub item_names: usize,
    pub scripts: &'static str,
    pub ball_script: usize,
    /// Entrées de la liste de scripts à ignorer (ce ne sont pas des objets).
    pub ball_skip: &'static [usize],
    pub hidden: HiddenItems,
}

#[derive(Debug, Clone, Copy)]
pub enum HiddenItems {
    /// Table de l'ARM9 (Platine) : `count` entrées de 8 octets, objet en tête.
    Arm9Table { offset: usize, count: usize },
    /// Fichier de scripts (Noire/Blanche).
    Script(usize),
}

impl ItemLayout {
    /// Emplacements par code de jeu, repris de `gen4_offsets.ini` / `gen5_offsets.ini`
    /// de l'Universal Pokémon Randomizer. Vérifiés sur Platine (CPUE) et Blanche (IRAF).
    pub fn for_rom(game: &GameRom) -> Option<Self> {
        let code = game.rom().header().game_code.as_str();
        match game.game {
            Game::Platinum => {
                let (hidden, names) = match code {
                    "CPUE" => (0xEA378, 392),
                    "CPUD" => (0xEA3D0, 392),
                    "CPUF" => (0xEA400, 392),
                    "CPUS" => (0xEA40C, 392),
                    "CPUI" => (0xEA394, 392),
                    "CPUJ" => (0xE9A4C, 390),
                    "CPUK" => (0xEAE00, 390),
                    _ => return None,
                };
                Some(Self {
                    gen: 4,
                    item_names: names,
                    scripts: "fielddata/script/scr_seq.narc",
                    ball_script: 404,
                    ball_skip: &[25, 238, 321, 325, 326],
                    hidden: HiddenItems::Arm9Table { offset: hidden, count: 257 },
                })
            }
            Game::Black | Game::White => Some(Self {
                gen: 5,
                item_names: 54,
                scripts: "a/0/5/7",
                ball_script: 864,
                ball_skip: &[],
                hidden: HiddenItems::Script(865),
            }),
            _ => None,
        }
    }

    /// Position de la commande `setvar` dans chaque script, commande, variable.
    fn ball_command(&self) -> ScriptCommand {
        if self.gen == 4 {
            ScriptCommand { at: 0, command: 0x28, variable: 0x8008 }
        } else {
            ScriptCommand { at: 2, command: 0x28, variable: 0x800C }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldItemKind {
    /// Poké Ball posée au sol.
    Visible,
    /// Objet caché (Cherch'Objet).
    Hidden,
}

impl FieldItemKind {
    pub fn name_fr(self) -> &'static str {
        match self {
            FieldItemKind::Visible => "au sol",
            FieldItemKind::Hidden => "caché",
        }
    }
}

/// Endroit où est stocké l'identifiant d'un objet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPos {
    /// Fichier de l'archive de scripts, position de l'identifiant.
    Script { file: usize, offset: usize },
    /// Position dans l'ARM9 décompressé.
    Arm9(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldItem {
    pub kind: FieldItemKind,
    pub item: u16,
    pub pos: ItemPos,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScriptCommand {
    /// Décalage de la commande depuis le début du script.
    pub at: usize,
    pub command: u16,
    pub variable: u16,
}

fn rd16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn wr16(d: &mut [u8], at: usize, v: u16) -> Result<(), RomError> {
    d.get_mut(at..at + 2)
        .ok_or_else(|| RomError::Layout(format!("position {at:#X} hors du fichier")))?
        .copy_from_slice(&v.to_le_bytes());
    Ok(())
}

/// Positions des identifiants d'objets d'un fichier de scripts (liste de pointeurs
/// relatifs terminée par `0xFD13`), selon l'algorithme de l'Universal Pokémon Randomizer.
pub(crate) fn script_item_positions(script: &[u8], skip: &[usize], cmd: ScriptCommand) -> Vec<usize> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    loop {
        let Some(part1) = rd16(script, offset) else { break };
        if part1 == SCRIPT_LIST_END {
            break;
        }
        let Some(raw) = script.get(offset..offset + 4) else { break };
        let relative = i32::from_le_bytes(raw.try_into().unwrap_or_default()) as i64;
        let target = relative + offset as i64 + 4;
        offset += 4;
        if target < 0 || target as usize > script.len() {
            break;
        }
        if skip.contains(&(offset / 4 - 1)) {
            continue;
        }
        let start = target as usize + cmd.at;
        if rd16(script, start) == Some(cmd.command) && rd16(script, start + 2) == Some(cmd.variable) && rd16(script, start + 4).is_some() {
            out.push(start + 4);
        }
    }
    out
}

/// Lit tous les objets ramassables (visibles puis cachés), dans l'ordre de l'UPR.
pub fn read(game: &GameRom) -> Result<Vec<FieldItem>, RomError> {
    let layout = ItemLayout::for_rom(game).ok_or_else(unsupported)?;
    let narc = Narc::parse(game.rom().file_by_path(layout.scripts)?)?;
    let mut out = Vec::new();

    let mut from_script = |file: usize, skip: &[usize], cmd: ScriptCommand, kind: FieldItemKind| -> Result<(), RomError> {
        let script = narc.files.get(file).ok_or_else(|| RomError::Layout(format!("script n°{file} des objets absent")))?;
        for offset in script_item_positions(script, skip, cmd) {
            let item = rd16(script, offset).unwrap_or(0);
            out.push(FieldItem { kind, item, pos: ItemPos::Script { file, offset } });
        }
        Ok(())
    };
    from_script(layout.ball_script, layout.ball_skip, layout.ball_command(), FieldItemKind::Visible)?;

    match layout.hidden {
        HiddenItems::Script(file) => {
            let cmd = ScriptCommand { at: 2, command: 0x2A, variable: 0x8000 };
            from_script(file, &[], cmd, FieldItemKind::Hidden)?;
        }
        HiddenItems::Arm9Table { offset, count } => {
            let arm9 = game.rom().arm9_decompressed()?;
            for i in 0..count {
                let at = offset + i * 8;
                let item = rd16(&arm9, at).ok_or_else(|| RomError::Layout("table des objets cachés hors de l'ARM9".into()))?;
                out.push(FieldItem { kind: FieldItemKind::Hidden, item, pos: ItemPos::Arm9(at) });
            }
        }
    }

    // Garde-fou : une table mal placée donnerait des identifiants absurdes.
    if out.is_empty() || out.iter().any(|f| f.item > 1000) {
        return Err(RomError::Layout("objets ramassables introuvables (ROM modifiée ?)".into()));
    }
    Ok(out)
}

/// Réécrit les objets ramassables à leur place (lus avec [`read`], identifiants modifiés).
pub fn write(game: &mut GameRom, items: &[FieldItem]) -> Result<(), RomError> {
    let layout = ItemLayout::for_rom(game).ok_or_else(unsupported)?;
    let mut narc = Narc::parse(game.rom().file_by_path(layout.scripts)?)?;
    let mut arm9: Option<Vec<u8>> = None;
    let mut scripts_changed = false;
    for f in items {
        match f.pos {
            ItemPos::Script { file, offset } => {
                let script = narc.files.get_mut(file).ok_or_else(|| RomError::Layout(format!("script n°{file} absent")))?;
                if rd16(script, offset) != Some(f.item) {
                    wr16(script, offset, f.item)?;
                    scripts_changed = true;
                }
            }
            ItemPos::Arm9(at) => {
                if arm9.is_none() {
                    arm9 = Some(game.rom().arm9_decompressed()?);
                }
                if let Some(code) = arm9.as_mut() {
                    wr16(code, at, f.item)?;
                }
            }
        }
    }
    if scripts_changed {
        game.rom_mut().replace_file_by_path(layout.scripts, narc.to_bytes())?;
    }
    if let Some(code) = arm9 {
        if code != game.rom().arm9_decompressed()? {
            game.rom_mut().replace_arm9(&code)?;
        }
    }
    Ok(())
}

pub(crate) fn unsupported() -> RomError {
    RomError::Unsupported("objets et boutiques : Platine, Noire et Blanche seulement".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Liste de 3 pointeurs relatifs puis `FD13`, suivie de 3 scripts.
    fn synthetic(cmd_at: usize) -> Vec<u8> {
        let mut d = Vec::new();
        let header = 3 * 4 + 2;
        let script = |item: u16, var: u16| {
            let mut s = vec![0u8; cmd_at];
            s.extend(0x28u16.to_le_bytes());
            s.extend(var.to_le_bytes());
            s.extend(item.to_le_bytes());
            s.extend([0x02, 0x00]);
            s
        };
        let scripts = [script(17, 0x8008), script(50, 0x8008), script(4, 0x1234)];
        let mut at = header;
        for (i, s) in scripts.iter().enumerate() {
            let rel = at as i32 - (i as i32 * 4 + 4);
            d.extend(rel.to_le_bytes());
            at += s.len();
        }
        d.extend(SCRIPT_LIST_END.to_le_bytes());
        for s in &scripts {
            d.extend(s);
        }
        d
    }

    #[test]
    fn finds_setvar_items_and_skips() {
        let cmd = ScriptCommand { at: 0, command: 0x28, variable: 0x8008 };
        let d = synthetic(0);
        let pos = script_item_positions(&d, &[], cmd);
        let items: Vec<u16> = pos.iter().map(|&p| rd16(&d, p).unwrap()).collect();
        assert_eq!(items, vec![17, 50]); // le 3e script a une autre variable
        let skipped = script_item_positions(&d, &[0], cmd);
        assert_eq!(skipped.len(), 1);
        assert_eq!(rd16(&d, skipped[0]), Some(50));

        let cmd5 = ScriptCommand { at: 2, command: 0x28, variable: 0x8008 };
        let d5 = synthetic(2);
        assert_eq!(script_item_positions(&d5, &[], cmd5).len(), 2);
    }

    #[test]
    fn tolerates_truncated_data() {
        let cmd = ScriptCommand { at: 0, command: 0x28, variable: 0x8008 };
        let d = synthetic(0);
        for len in 0..d.len() {
            let _ = script_item_positions(&d[..len], &[], cmd);
        }
        // Pointeur vers l'arrière, hors du fichier : arrêt sans panique.
        let mut bad = (-100i32).to_le_bytes().to_vec();
        bad.extend([0x13, 0xFD]);
        assert!(script_item_positions(&bad, &[], cmd).is_empty());
    }
}
