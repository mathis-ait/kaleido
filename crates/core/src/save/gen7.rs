//! Sauvegardes Gen 7 : Soleil/Lune et Ultra-Soleil/Ultra-Lune (`SAV7*` de PKHeX).
//!
//! Même table des blocs que la Gen 6 (voir [`super::gen6`]), mais CRC16 inversé.
//!
//! Offsets (vérifiés sur `SaveBlockAccessor7SM/USUM` de PKHeX, et sur la sauvegarde
//! SL `SM Project 802.main` de ses tests) :
//!
//! | | Dresseur (bloc 3) | Équipe (4) | Pokédex (6) | Divers (9, argent +0x4) | Noms des boîtes (13) | Boîtes (14) | Temps de jeu (16) |
//! |---|---|---|---|---|---|---|---|
//! | SL | 0x1200 | 0x1400 | 0x2A00 | 0x4000 | 0x4800 | 0x4E00 | 0x40C00 |
//! | USUL | 0x1400 | 0x1600 | 0x2C00 | 0x4400 | 0x4C00 | 0x5200 | 0x41000 |
//!
//! Dresseur Gen 7 : TID +0, SID +2, sexe +5, nom +0x38. 32 boîtes. Sac : bloc 0
//! (0x0, 0xDE0 octets en SL, 0xE28 en USUL, d'après `SaveBlockAccessor7SM/USUM`).
//! Dresseur Gen 7 : TID +0, SID +2, sexe +5, langue +0x35, nom +0x38. 32 boîtes.
//!
//! **Signature MemeCrypto** : le jeu vérifie une signature RSA du SHA-256 de la
//! table des blocs (bloc 36). Comme PKHeX, Kaleido la recalcule à l'écriture
//! (voir [`super::memecrypto`]).

use super::gen6::{ctr_layout, CtrOffsets};
use super::{Layout, PkmFormat, SaveError, SaveVersion};

// Vérifié : PKHeX SaveBlockAccessor7SM.cs / SaveBlockAccessor7USUM.cs (blocs 3, 4, 6, 9, 13,
// 14, 16), SAV7.cs (PartyCount = Data[Party + 6·260], BoxCount = 32), MyStatus7.cs (TID +0,
// SID +2, Gender +5, Language +0x35, OT +0x38 sur 0x1A octets), Misc7.cs (Money +0x4),
// BoxLayout7.cs (noms de 0x22 octets), PlayTime6 réutilisé pour le bloc 16.
pub(super) fn offsets(version: SaveVersion) -> CtrOffsets {
    if version == SaveVersion::UltraSunUltraMoon {
        CtrOffsets {
            status: 0x1400,
            ot_name: 0x38,
            party: 0x1600,
            play_time: 0x41000,
            money: 0x4400 + 0x4,
            box_names: 0x4C00,
            boxes: 0x5200,
            box_count: 32,
            // Bloc 0 « MyItem » (SaveBlockAccessor7USUM).
            items: 0,
            items_len: 0xE28,
            language: 0x1400 + 0x35,
            dex: (0x2C00, 0xF78),
            // Vérifié : PKHeX SaveBlockAccessor7USUM (bloc 1 « Situation » en 0x1000), Situation7.cs (`M` en +0).
            map: 0x1000,
        }
    } else {
        CtrOffsets {
            status: 0x1200,
            ot_name: 0x38,
            party: 0x1400,
            play_time: 0x40C00,
            money: 0x4000 + 0x4,
            box_names: 0x4800,
            boxes: 0x4E00,
            box_count: 32,
            // Bloc 0 « MyItem » (SaveBlockAccessor7SM).
            items: 0,
            items_len: 0xDE0,
            language: 0x1200 + 0x35,
            dex: (0x2A00, 0xF78),
            // Vérifié : PKHeX SaveBlockAccessor7SM (bloc 1 « Situation » en 0xE00).
            map: 0xE00,
        }
    }
}

pub(super) fn layout(version: SaveVersion, data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    ctr_layout(PkmFormat::Gen7, data, &offsets(version), true)
}

/// Tailles des blocs 0 à 16 (Soleil/Lune d'après PKHeX ; Ultra-Soleil/Ultra-Lune
/// synthétiques, choisies pour retomber sur les offsets ci-dessus).
#[cfg(test)]
pub(super) fn synthetic_lengths(version: SaveVersion) -> Vec<usize> {
    if version == SaveVersion::UltraSunUltraMoon {
        vec![0xE28, 0x7C, 0x14, 0xC0, 0x61C, 0xE00, 0xF78, 0x228, 0x30C, 0x1FC, 0x4C, 0x4, 0x58, 0x5E6, 0x36600, 0x572C, 0x8]
    } else {
        vec![0xDE0, 0x7C, 0x14, 0xC0, 0x61C, 0xE00, 0xF78, 0x228, 0x104, 0x200, 0x20, 0x4, 0x58, 0x5E6, 0x36600, 0x572C, 0x8]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_offsets_match_constants() {
        for (version, size) in [(SaveVersion::SunMoon, 0x6BE00), (SaveVersion::UltraSunUltraMoon, 0x6CC00)] {
            let (_, offsets) = super::super::gen6::blank(size, &synthetic_lengths(version));
            let o = super::offsets(version);
            assert_eq!(offsets[3], o.status, "{version:?}");
            assert_eq!(offsets[4], o.party, "{version:?}");
            assert_eq!(offsets[6], o.dex.0, "{version:?}");
            assert_eq!(offsets[9] + 4, o.money, "{version:?}");
            assert_eq!(offsets[13], o.box_names, "{version:?}");
            assert_eq!(offsets[14], o.boxes, "{version:?}");
            assert_eq!(offsets[16], o.play_time, "{version:?}");
            assert_eq!((offsets[0], synthetic_lengths(version)[0]), (o.items, o.items_len), "{version:?}");
        }
    }
}
