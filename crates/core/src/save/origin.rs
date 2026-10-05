//! Origine du dresseur : langue de la partie, version exacte (3DS) et réglages de
//! région de la console (3DS). Sert à recevoir un cadeau mystère comme le jeu.
//!
//! Vérifié : PKHeX `MyStatus6.cs` (Game +0x04, Region +0x26, Country +0x27,
//! ConsoleRegion +0x2C, Language +0x2D) et `MyStatus7.cs` (Game +0x04, Region +0x2E,
//! Country +0x2F, ConsoleRegion +0x34, Language +0x35).

use super::{rd_u8, SaveFile};

/// Langue, version et région du dresseur de la sauvegarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainerOrigin {
    /// Langue de la partie (`LanguageID` de PKHeX : 1 japonais, 2 anglais, 3 français…).
    pub language: u8,
    /// Version exacte (`GameVersion` de PKHeX), connue seulement sur 3DS.
    pub version: Option<u8>,
    /// Région matérielle, pays et région du pays (3DS ; 0 sur DS).
    pub console_region: u8,
    pub country: u8,
    pub region: u8,
}

impl SaveFile {
    pub fn trainer_origin(&self) -> TrainerOrigin {
        let d = &self.data;
        let t = &self.layout.trainer;
        let language = rd_u8(d, t.language);
        // Sur 3DS, le bloc MyStatus commence à l'ID du dresseur.
        let status = t.tid;
        match self.generation() {
            6 => TrainerOrigin {
                language,
                version: Some(rd_u8(d, status + 0x04)),
                console_region: rd_u8(d, status + 0x2C),
                country: rd_u8(d, status + 0x27),
                region: rd_u8(d, status + 0x26),
            },
            7 => TrainerOrigin {
                language,
                version: Some(rd_u8(d, status + 0x04)),
                console_region: rd_u8(d, status + 0x34),
                country: rd_u8(d, status + 0x2F),
                region: rd_u8(d, status + 0x2E),
            },
            _ => TrainerOrigin { language, version: None, console_region: 0, country: 0, region: 0 },
        }
    }
}
