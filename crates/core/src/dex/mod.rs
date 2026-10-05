//! Données de jeu hors ligne pour l'éditeur de sauvegarde (Gen 4 à 7).
//!
//! Noms français (espèces, attaques, talents, objets, natures, types, versions,
//! formes, Balls, rubans, lieux de rencontre), fiches des espèces par jeu, attaques
//! apprises par niveau et par reproduction, et caractéristiques des attaques.
//!
//! Les ressources viennent de PKHeX (kwsch/PKHeX, GPLv3,
//! <https://github.com/kwsch/PKHeX>) : textes `Resources/text`, tables binaires
//! `Resources/byte` et tables de PP/types de `Moves/MoveInfo*.cs`. La puissance, la
//! précision, la priorité et la catégorie des attaques viennent de PokeAPI
//! (BSD-3, <https://github.com/PokeAPI/pokeapi>). Voir `data/pkhex/README.md`.
//!
//! Tout est embarqué dans le binaire et décodé à la demande (`LazyLock`).

mod forms;
mod moves;
mod personal;
mod text;

#[cfg(test)]
mod tests;

use serde::Serialize;

pub use forms::{form_name, form_names};
pub use moves::{move_info, move_info_in, MoveCategory, MoveInfo};
pub use personal::{egg_moves, levelup, personal, PersonalInfo};
pub(crate) use personal::personal_raw;
pub use text::{
    ability_name, ability_names, ball_name, ball_names, game_name, game_names, item_name, item_name_in, item_names,
    location_name, locations, move_name, move_names, nature_name, nature_names, ribbon_name, ribbon_names,
    species_name, species_names, type_name, type_names,
};

/// Groupe de versions dont les données diffèrent (une sauvegarde = un groupe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    DP,
    Pt,
    HGSS,
    BW,
    B2W2,
    XY,
    ORAS,
    SM,
    USUM,
}

impl Game {
    pub const ALL: [Game; 9] = [Game::DP, Game::Pt, Game::HGSS, Game::BW, Game::B2W2, Game::XY, Game::ORAS, Game::SM, Game::USUM];

    pub fn generation(self) -> u8 {
        match self {
            Game::DP | Game::Pt | Game::HGSS => 4,
            Game::BW | Game::B2W2 => 5,
            Game::XY | Game::ORAS => 6,
            Game::SM | Game::USUM => 7,
        }
    }

    /// Rang du jeu parmi [`Game::ALL`], pour indexer les tables par jeu.
    fn index(self) -> usize {
        self as usize
    }
}

impl From<crate::games::Game> for Game {
    fn from(game: crate::games::Game) -> Self {
        use crate::games::Game as G;
        match game {
            G::Diamond | G::Pearl => Game::DP,
            G::Platinum => Game::Pt,
            G::HeartGold | G::SoulSilver => Game::HGSS,
            G::Black | G::White => Game::BW,
            G::Black2 | G::White2 => Game::B2W2,
            G::X | G::Y => Game::XY,
            G::OmegaRuby | G::AlphaSapphire => Game::ORAS,
            G::Sun | G::Moon => Game::SM,
            G::UltraSun | G::UltraMoon => Game::USUM,
        }
    }
}

/// Plus grand numéro d'espèce présent dans le jeu.
pub fn max_species(game: Game) -> u16 {
    match game.generation() {
        4 => 493,
        5 => 649,
        6 => 721,
        _ if game == Game::SM => 802,
        _ => 807,
    }
}

/// Plus grand identifiant d'attaque.
pub fn max_move(game: Game) -> u16 {
    match game {
        Game::DP | Game::Pt | Game::HGSS => 467,
        Game::BW | Game::B2W2 => 559,
        Game::XY => 617,
        Game::ORAS => 621,
        Game::SM => 719,
        Game::USUM => 728,
    }
}

/// Plus grand identifiant d'objet.
pub fn max_item(game: Game) -> u16 {
    match game {
        Game::DP => 464,
        Game::Pt => 467,
        Game::HGSS => 536,
        Game::BW => 632,
        Game::B2W2 => 638,
        Game::XY => 717,
        Game::ORAS => 775,
        Game::SM => 920,
        Game::USUM => 959,
    }
}

/// Plus grand identifiant de talent.
pub fn max_ability(game: Game) -> u16 {
    match game {
        Game::DP | Game::Pt | Game::HGSS => 123,
        Game::BW | Game::B2W2 => 164,
        Game::XY => 188,
        Game::ORAS => 191,
        Game::SM => 232,
        Game::USUM => 233,
    }
}

/// Plus grand identifiant de Ball (Compét’Ball en Gen 4, Rêve Ball en Gen 5-6, Ultra Ball en Gen 7).
pub fn max_ball(game: Game) -> u8 {
    match game.generation() {
        4 => 0x18,
        5 | 6 => 0x19,
        _ => 0x1A,
    }
}
