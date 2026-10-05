//! Taux de Pokémon chromatiques (DS, Gen 4 / 5).
//!
//! Un Pokémon est chromatique si `(ID ^ ID secret ^ PID haut ^ PID bas) < 8`, soit
//! 8 chances sur 65 536 (1 / 8 192). La fonction du jeu se termine par
//! `eor r0, r1 ; eor r0, r2 ; cmp r0, #8 ; bcs ; mov r0, #1` (Thumb), identique
//! dans Platine (ARM9 0x75E50) et Noire/Blanche (ARM9 décompressé 0x13F0C).
//! Changer la constante 8 multiplie le taux. Plafond : 255 (≈ 1 / 257).

use kaleido_formats::nds::NdsRom;

use crate::rom::RomError;

/// eor r0, r1 ; eor r0, r2 ; cmp r0, #8 ; bcs +1 ; mov r0, #1
const SHINY_CHECK: [u8; 10] = [0x48, 0x40, 0x50, 0x40, 0x08, 0x28, 0x01, 0xD2, 0x01, 0x20];
const THRESHOLD_OFFSET: usize = 4;
pub const DEFAULT_THRESHOLD: u8 = 8;

/// Position (dans l'ARM9 décompressé) de la constante du test chromatique. La
/// constante elle-même est ignorée : on retrouve aussi une ROM déjà modifiée.
pub fn locate(rom: &NdsRom) -> Result<usize, RomError> {
    let arm9 = rom.arm9_decompressed()?;
    let matches = |w: &[u8]| w.iter().zip(SHINY_CHECK).enumerate().all(|(i, (a, b))| i == THRESHOLD_OFFSET || *a == b);
    let mut hits = arm9.windows(SHINY_CHECK.len()).enumerate().filter(|(_, w)| matches(w)).map(|(i, _)| i);
    match (hits.next(), hits.next()) {
        (Some(p), None) => Ok(p + THRESHOLD_OFFSET),
        _ => Err(RomError::Layout("fonction de test chromatique introuvable".into())),
    }
}

/// Multiplie le taux de chromatiques (1 = inchangé). Renvoie la nouvelle probabilité « 1 / n ».
pub fn set_multiplier(rom: &mut NdsRom, multiplier: u16) -> Result<u32, RomError> {
    let threshold = (DEFAULT_THRESHOLD as u32 * multiplier.max(1) as u32).min(255) as u8;
    let at = locate(rom)?;
    rom.patch_arm9(at, threshold)?;
    Ok(65536 / threshold as u32)
}

/// Seuil actuellement utilisé par la ROM (8 = taux normal).
pub fn current_threshold(rom: &NdsRom) -> Result<u8, RomError> {
    let at = locate(rom)?;
    Ok(rom.arm9_decompressed()?[at])
}
