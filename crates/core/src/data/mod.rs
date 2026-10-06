//! Données de jeu modifiables (Gen 4 / 5) : rencontres, dresseurs, attaques
//! apprises, évolutions, starters. Chaque format a été vérifié octet par octet
//! sur de vraies ROMs (Platine, Blanche) avant d'être codé.

pub mod encounters;
pub mod evolutions;
pub mod field_items;
pub mod learnsets;
pub mod machines;
pub mod shiny;
pub mod shops;
pub mod starters;
pub mod trainers;

#[cfg(test)]
mod gen4_tests;

use crate::games::Game;

/// Emplacement des données pour le randomizer.
#[derive(Debug, Clone, Copy)]
pub struct DataPaths {
    pub encounters: &'static str,
    pub trainer_data: &'static str,
    pub trainer_pokemon: &'static str,
    pub learnsets: &'static str,
    pub evolutions: &'static str,
    /// Fichier de texte des noms d'attaques.
    pub move_names: usize,
    /// Plus grand identifiant de talent utilisable.
    pub max_ability: u16,
    pub starters: starters::StarterLocation,
}

impl DataPaths {
    /// Jeux pris en charge par le randomizer (formats vérifiés sur une vraie ROM).
    pub fn for_game(game: Game) -> Option<Self> {
        match game {
            Game::Platinum => Some(Self {
                encounters: "fielddata/encountdata/pl_enc_data.narc",
                trainer_data: "poketool/trainer/trdata.narc",
                trainer_pokemon: "poketool/trainer/trpoke.narc",
                learnsets: "poketool/personal/wotbl.narc",
                evolutions: "poketool/personal/evo.narc",
                move_names: 647,
                max_ability: 123,
                starters: starters::StarterLocation::Platinum,
            }),
            // Vérifié sur Diamant (ADAF) ; chemins de Perle d'après UPR-ZX (gen4_offsets.ini).
            Game::Diamond | Game::Pearl => Some(Self {
                encounters: if game == Game::Diamond { "fielddata/encountdata/d_enc_data.narc" } else { "fielddata/encountdata/p_enc_data.narc" },
                trainer_data: "poketool/trainer/trdata.narc",
                trainer_pokemon: "poketool/trainer/trpoke.narc",
                learnsets: "poketool/personal/wotbl.narc",
                evolutions: "poketool/personal/evo.narc",
                move_names: 588,
                max_ability: 123,
                starters: starters::StarterLocation::DiamondPearl,
            }),
            // D'après UPR-ZX, `[HeartGold (U)]` / `[SoulSilver (U)]` ; vérifié sur SoulSilver (IPGF).
            Game::HeartGold | Game::SoulSilver => Some(Self {
                encounters: if game == Game::HeartGold { "a/0/3/7" } else { "a/1/3/6" },
                trainer_data: "a/0/5/5",
                trainer_pokemon: "a/0/5/6",
                learnsets: "a/0/3/3",
                evolutions: "a/0/3/4",
                move_names: 750,
                max_ability: 123,
                starters: starters::StarterLocation::HeartGoldSoulSilver,
            }),
            Game::Black | Game::White => Some(Self {
                encounters: "a/1/2/6",
                trainer_data: "a/0/9/2",
                trainer_pokemon: "a/0/9/3",
                learnsets: "a/0/1/8",
                evolutions: "a/0/1/9",
                move_names: 203,
                max_ability: 164,
                starters: starters::StarterLocation::BlackWhite,
            }),
            // Vérifié sur Noire 2 (IREF) ; NARC renumérotés d'après `gen5_offsets.ini` de l'UPR.
            Game::Black2 | Game::White2 => Some(Self {
                encounters: "a/1/2/7",
                trainer_data: "a/0/9/1",
                trainer_pokemon: "a/0/9/2",
                learnsets: "a/0/1/8",
                evolutions: "a/0/1/9",
                move_names: 403,
                max_ability: 164,
                starters: starters::StarterLocation::Black2White2,
            }),
            _ => None,
        }
    }
}

pub(crate) fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

pub(crate) fn put_u16(d: &mut [u8], at: usize, v: u16) {
    d[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
