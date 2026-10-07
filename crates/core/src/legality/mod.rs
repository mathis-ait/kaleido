//! Légalité des Pokémon Gen 4 à 7, d'après PKHeX (kwsch/PKHeX, GPLv3).
//!
//! - [`encounters`] : base des rencontres (sauvages, fixes, dons, échanges, œufs…) ;
//! - [`evolution`] : lignées et niveaux d'évolution ;
//! - [`learn`] : attaques apprenables (niveau, CT/CS, donneurs, capacités Œuf) ;
//! - [`rng`] : corrélations PID / IV (méthode 1 des Gen 3/4, tirages des méthodes J et K
//!   avec slot et niveau, Poké Radar, règles de la Gen 5) ;
//! - [`verify`] : vérifications et verdict (Légal / Douteux / Illégal) ;
//! - [`legalize`] : « Rendre légal » et « Générer un Pokémon légal ».
//!
//! Données : `data/pkhex/legality/` (voir `data/pkhex/README.md`).
//!
//! - [`events`] : distributions (Cadeaux Mystère) de la base de PKHeX ;
//! - [`db`] : base « Rencontres » pour l'interface.
//!
//! Limites connues (par rapport à PKHeX) : rencontres et attaques des Gen 1 à 3 non
//! vérifiées (« Douteux »), héritage des Balls simplifié, pas de vérification des tailles
//! ni des surnoms d'échange ; méthodes J/K : talents de tête Statik / Magnépiège, Pression
//! (niveau maximal), activation des cannes et rencontres spéciales (Safari, Concours de
//! capture, Coup d'Boule, Éclate-Roc, arbres à Miel) non vérifiés ; distributions
//! comparées sans les restrictions de langue des cartes.

pub mod db;
pub mod encounters;
pub mod events;
pub mod evolution;
pub mod learn;
pub mod legalize;
pub mod rng;
pub mod verify;

#[cfg(test)]
mod tests;

pub use db::{encounter_db, game_from_id, game_label, games_for, species_entries, species_index, EncounterEntry, SpeciesEncounters};
pub use encounters::{encounters, Encounter, EncounterKind};
pub use legalize::{generate_legal, legalize, legalize_with, Change, EncounterOption, GenerateRequest, LegalizeOutcome};
pub use verify::{analyze, Check, Report, Severity, Verdict};
