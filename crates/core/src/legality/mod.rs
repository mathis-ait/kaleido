//! Légalité des Pokémon Gen 4 à 7, d'après PKHeX (kwsch/PKHeX, GPLv3).
//!
//! - [`encounters`] : base des rencontres (sauvages, fixes, dons, échanges, œufs…) ;
//! - [`evolution`] : lignées et niveaux d'évolution ;
//! - [`learn`] : attaques apprenables (niveau, CT/CS, donneurs, capacités Œuf) ;
//! - [`rng`] : corrélations PID / IV (méthode 1 des Gen 3/4, règles de la Gen 5) ;
//! - [`verify`] : vérifications et verdict (Légal / Douteux / Illégal) ;
//! - [`legalize`] : « Rendre légal » et « Générer un Pokémon légal ».
//!
//! Données : `data/pkhex/legality/` (voir `data/pkhex/README.md`).
//!
//! Limites connues (par rapport à PKHeX) : pas de base des distributions (Cadeaux
//! Mystère, signalés « Douteux »), pas de vérification des rencontres des Gen 1 à 3,
//! héritage des Balls et des attaques d'œuf simplifié, pas de vérification des
//! souvenirs, rubans, tailles, dates ni des créneaux RNG (méthodes J/K).

pub mod db;
pub mod encounters;
pub mod evolution;
pub mod learn;
pub mod legalize;
pub mod rng;
pub mod verify;

#[cfg(test)]
mod tests;

pub use db::{encounter_db, game_from_id, game_label, games_for, species_entries, species_index, EncounterEntry, SpeciesEncounters};
pub use encounters::{encounters, Encounter, EncounterKind};
pub use legalize::{generate_legal, legalize, GenerateRequest, LegalizeOutcome};
pub use verify::{analyze, Check, Report, Severity, Verdict};
