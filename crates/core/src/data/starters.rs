//! Pokémon de départ.
//!
//! - Platine : tableau de 3 espèces u32 dans l'overlay 78, à l'offset 0x1BC0.
//! - Noire/Blanche : tableau de 3 espèces u16 dans l'overlay 223 (écran de choix),
//!   plus les commandes du script 304 (`57 00 00 <espèce u16>`) qui affichent le
//!   sprite et jouent le cri de chaque starter.

use crate::rom::{GameRom, RomError};
use kaleido_formats::narc::Narc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterLocation {
    Platinum,
    BlackWhite,
}

const PT_OVERLAY: u32 = 78;
const PT_OFFSET: usize = 0x1BC0;
const PT_ORIGINAL: [u16; 3] = [387, 390, 393];

const BW_OVERLAY: u32 = 223;
const BW_OFFSET: usize = 0x3170;
const BW_ORIGINAL: [u16; 3] = [495, 498, 501];
const BW_SCRIPTS: &str = "a/0/5/7";
const BW_SCRIPT: usize = 304;

fn mismatch() -> RomError {
    RomError::Layout("emplacement des starters inattendu (ROM modifiée ?)".into())
}

pub fn read(game: &GameRom, location: StarterLocation) -> Result<[u16; 3], RomError> {
    let mut out = [0; 3];
    match location {
        StarterLocation::Platinum => {
            let ovl = game.rom().overlay(PT_OVERLAY)?;
            for (i, s) in out.iter_mut().enumerate() {
                let at = PT_OFFSET + i * 4;
                *s = u32::from_le_bytes(ovl.get(at..at + 4).ok_or_else(mismatch)?.try_into().unwrap()) as u16;
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

/// Écrit les nouveaux starters. Ne fonctionne que sur une ROM dont les starters
/// sont encore ceux d'origine (on part toujours de la ROM originale).
pub fn write(game: &mut GameRom, location: StarterLocation, starters: [u16; 3]) -> Result<(), RomError> {
    match location {
        StarterLocation::Platinum => {
            if read(game, location)? != PT_ORIGINAL {
                return Err(mismatch());
            }
            let mut ovl = game.rom().overlay(PT_OVERLAY)?;
            for (i, s) in starters.iter().enumerate() {
                let at = PT_OFFSET + i * 4;
                ovl[at..at + 4].copy_from_slice(&(*s as u32).to_le_bytes());
            }
            game.rom_mut().replace_overlay(PT_OVERLAY, ovl)?;
        }
        StarterLocation::BlackWhite => {
            if read(game, location)? != BW_ORIGINAL {
                return Err(mismatch());
            }
            let mut ovl = game.rom().overlay(BW_OVERLAY)?;
            for (i, s) in starters.iter().enumerate() {
                let at = BW_OFFSET + i * 2;
                ovl[at..at + 2].copy_from_slice(&s.to_le_bytes());
            }
            game.rom_mut().replace_overlay(BW_OVERLAY, ovl)?;

            let mut narc = Narc::parse(game.rom().file_by_path(BW_SCRIPTS)?)?;
            let script = narc.files.get_mut(BW_SCRIPT).ok_or_else(mismatch)?;
            let mut replaced = 0;
            for (orig, new) in BW_ORIGINAL.iter().zip(starters) {
                let pattern = [0x57, 0x00, 0x00, *orig as u8, (*orig >> 8) as u8];
                let mut at = 0;
                while let Some(pos) = script[at..].windows(5).position(|w| w == pattern) {
                    let p = at + pos + 3;
                    script[p..p + 2].copy_from_slice(&new.to_le_bytes());
                    replaced += 1;
                    at = p + 2;
                }
            }
            if replaced == 0 {
                return Err(mismatch());
            }
            game.rom_mut().replace_file_by_path(BW_SCRIPTS, narc.to_bytes())?;
        }
    }
    Ok(())
}
