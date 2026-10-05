//! Taux de Pokémon chromatiques (DS, Gen 4 / 5).
//!
//! Un Pokémon est chromatique si `(ID ^ ID secret ^ PID haut ^ PID bas) < 8`, soit
//! 8 chances sur 65 536 (1 / 8 192). La fonction du jeu se termine par
//! `eor r0, r1 ; eor r0, r2 ; cmp r0, #8 ; bcs ; mov r0, #1` (Thumb), identique
//! dans Platine (ARM9 0x75E50) et Noire/Blanche (ARM9 décompressé 0x13F0C).
//!
//! - Seuil 1 à 255 : on remplace la constante 8 (255 ≈ 1 / 257).
//! - « Toujours » : on neutralise le `bcs`, la fonction répond toujours « chromatique ».
//!   Attention : les événements qui relancent un tirage tant que le Pokémon est
//!   chromatique (Pokémon qui ne doivent jamais l'être) peuvent alors boucler.

use kaleido_formats::nds::NdsRom;

use crate::rom::RomError;

/// eor r0, r1 ; eor r0, r2 ; cmp r0, #N ; bcs +1 ; mov r0, #1
const SHINY_CHECK: [u8; 10] = [0x48, 0x40, 0x50, 0x40, 0x08, 0x28, 0x01, 0xD2, 0x01, 0x20];
const THRESHOLD_OFFSET: usize = 4;
const BRANCH_OFFSET: usize = 6;
/// `nop` Thumb (mov r8, r8).
const THUMB_NOP: [u8; 2] = [0xC0, 0x46];
pub const DEFAULT_THRESHOLD: u16 = 8;
/// Seuil spécial : tous les Pokémon sont chromatiques.
pub const ALWAYS: u16 = 256;

/// Position (dans l'ARM9 décompressé) du début du test. La constante et le saut sont
/// ignorés : on retrouve aussi une ROM déjà modifiée.
pub fn locate(rom: &NdsRom) -> Result<usize, RomError> {
    let arm9 = rom.arm9_decompressed()?;
    let ignored = [THRESHOLD_OFFSET, BRANCH_OFFSET, BRANCH_OFFSET + 1];
    let matches = |w: &[u8]| w.iter().zip(SHINY_CHECK).enumerate().all(|(i, (a, b))| ignored.contains(&i) || *a == b);
    let mut hits = arm9.windows(SHINY_CHECK.len()).enumerate().filter(|(_, w)| matches(w)).map(|(i, _)| i);
    match (hits.next(), hits.next()) {
        (Some(p), None) => Ok(p),
        _ => Err(RomError::Layout("fonction de test chromatique introuvable".into())),
    }
}

/// Seuil correspondant à « 1 chance sur `odds` » (arrondi, borné à 1..=255 ; 1 = toujours).
pub fn threshold_for_odds(odds: u32) -> u16 {
    if odds <= 1 {
        return ALWAYS;
    }
    ((65536.0 / odds as f64).round() as u16).clamp(1, 255)
}

/// Applique un seuil (1..=255, ou [`ALWAYS`]). Renvoie la probabilité obtenue « 1 / n »
/// (1 pour « toujours »).
pub fn set_threshold(rom: &mut NdsRom, threshold: u16) -> Result<u32, RomError> {
    let at = locate(rom)?;
    if threshold >= ALWAYS {
        let mut arm9 = rom.arm9_decompressed()?;
        arm9[at + BRANCH_OFFSET..at + BRANCH_OFFSET + 2].copy_from_slice(&THUMB_NOP);
        rom.replace_arm9(&arm9)?;
        return Ok(1);
    }
    let t = threshold.clamp(1, 255) as u8;
    rom.patch_arm9(at + THRESHOLD_OFFSET, t)?;
    Ok(65536 / t as u32)
}

/// Seuil actuellement utilisé par la ROM (8 = taux normal, [`ALWAYS`] si neutralisé).
pub fn current_threshold(rom: &NdsRom) -> Result<u16, RomError> {
    let at = locate(rom)?;
    let arm9 = rom.arm9_decompressed()?;
    if arm9[at + BRANCH_OFFSET..at + BRANCH_OFFSET + 2] == THUMB_NOP {
        return Ok(ALWAYS);
    }
    Ok(arm9[at + THRESHOLD_OFFSET] as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odds_to_threshold() {
        assert_eq!(threshold_for_odds(8192), 8);
        assert_eq!(threshold_for_odds(4096), 16);
        assert_eq!(threshold_for_odds(100), 255);
        assert_eq!(threshold_for_odds(1), ALWAYS);
        assert_eq!(threshold_for_odds(1_000_000), 1);
    }
}
