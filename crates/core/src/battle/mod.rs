//! « Préparer un combat » : calculateur de dégâts Gen 4 à 7 entre l'équipe de la
//! sauvegarde et les dresseurs d'une ROM (d'origine ou randomisée).
//!
//! Le calcul suit la structure du calculateur de Pokémon Showdown
//! (smogon/damage-calc, licence MIT, <https://github.com/smogon/damage-calc>) :
//! `mechanics/gen4.ts` (Diamant/Perle/Platine), `gen56.ts` (Noir/Blanc, X/Y,
//! ROSA) et `gen789.ts` (Soleil/Lune). Les arrondis (`pokeRound`, `chainMods`,
//! multiplicateurs sur 4096) sont repris tels quels.
//!
//! Hypothèses (comme Showdown par défaut) : combat simple, pas d'écran, pas de
//! terrain ; les attaques à 2-5 coups frappent 3 fois (5 avec Multi-Coups) ;
//! les probabilités de K.O. ignorent les coups critiques et les soins (Restes).

mod calc;
pub mod ids;
mod ko;
mod matchup;
pub mod party;
pub mod trainers;
pub mod types;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use calc::{calculate, speed, Damage, MoveCalc};
pub use ko::{ko_info, KoInfo};
pub use matchup::{duel, matrix, Cell, Duel, MoveLine, Verdict};

/// Un Pokémon prêt pour le calcul (statistiques finales, sans modificateurs de combat).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Combatant {
    pub species: u16,
    pub form: u8,
    /// Surnom ou nom de l'espèce.
    pub name: String,
    pub level: u8,
    /// Types canoniques (voir [`types`]), un ou deux.
    pub types: Vec<u8>,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit.
    pub stats: [u16; 6],
    pub ability: u16,
    pub item: u16,
    pub moves: [u16; 4],
    pub nature: u8,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit.
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
    /// Bonheur (Retour, Frustration).
    pub friendship: u8,
    /// Poids en hectogrammes (Balayage, Nœud Herbe…) ; 0 = inconnu.
    pub weight: u16,
    /// Approximations faites pour construire ce Pokémon (nature supposée…).
    pub notes: Vec<String>,
}

/// Problème de statut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    #[default]
    None,
    Burn,
    Paralysis,
    Poison,
    Sleep,
}

/// État d'un camp réglé dans l'interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SideState {
    /// Niveaux de statistiques (−6 à +6) : indices 1 Att, 2 Déf, 3 Atq Spé, 4 Déf Spé, 5 Vit (0 inutilisé).
    pub boosts: [i8; 6],
    pub status: Status,
    /// PV restants en % (100 = pleine forme), pour Brasier, Multiécaille, Gyroballe…
    pub hp_percent: u8,
}

impl Default for SideState {
    fn default() -> Self {
        Self { boosts: [0; 6], status: Status::None, hp_percent: 100 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Weather {
    #[default]
    None,
    Sun,
    Rain,
    Sand,
    Hail,
}

/// Réglages communs aux deux camps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Field {
    pub weather: Weather,
    /// Intimidation des deux camps appliquée (baisse l'Attaque adverse d'un niveau).
    pub intimidate: bool,
}

impl Default for Field {
    fn default() -> Self {
        Self { weather: Weather::None, intimidate: true }
    }
}
