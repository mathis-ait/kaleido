//! Réglages du randomizer, préréglages et code de partage.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StarterMode {
    #[default]
    Unchanged,
    /// Trois Pokémon au hasard.
    Random,
    /// Trois Pokémon de base ayant deux évolutions.
    ThreeStage,
    /// Comme `ThreeStage`, en gardant le trio Plante / Feu / Eau.
    Triangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WildMode {
    #[default]
    Unchanged,
    /// Chaque emplacement indépendamment.
    Random,
    /// Dans chaque zone, une espèce d'origine → une même nouvelle espèce.
    Area,
    /// Une correspondance unique pour tout le jeu (Roucool devient partout X).
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerMode {
    #[default]
    Unchanged,
    Random,
    /// Chaque dresseur garde un type dominant (les champions restent thématiques).
    TypeThemed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatsMode {
    #[default]
    Unchanged,
    /// Les six statistiques sont mélangées entre elles (total identique).
    Shuffle,
    /// Nouvelle répartition aléatoire du même total.
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub starters: StarterMode,
    pub wild: WildMode,
    pub wild_similar_strength: bool,
    pub wild_level_percent: u16,
    pub trainers: TrainerMode,
    pub trainers_similar_strength: bool,
    pub trainer_level_percent: u16,
    pub stats: StatsMode,
    pub random_types: bool,
    pub random_abilities: bool,
    pub no_legendaries: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            starters: StarterMode::Unchanged,
            wild: WildMode::Unchanged,
            wild_similar_strength: true,
            wild_level_percent: 100,
            trainers: TrainerMode::Unchanged,
            trainers_similar_strength: true,
            trainer_level_percent: 100,
            stats: StatsMode::Unchanged,
            random_types: false,
            random_abilities: false,
            no_legendaries: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub settings: Settings,
}

/// Préréglages proposés en un clic.
pub fn presets() -> Vec<Preset> {
    let base = Settings::default();
    vec![
        Preset {
            id: "equilibre",
            name: "Équilibré",
            description: "Starters, sauvages et dresseurs aléatoires, à puissance comparable.",
            settings: Settings { starters: StarterMode::Triangle, wild: WildMode::Area, trainers: TrainerMode::Random, ..base.clone() },
        },
        Preset {
            id: "nuzlocke",
            name: "Nuzlocke",
            description: "Une espèce par zone, dresseurs thématiques et un peu plus forts.",
            settings: Settings {
                starters: StarterMode::ThreeStage,
                wild: WildMode::Area,
                trainers: TrainerMode::TypeThemed,
                trainer_level_percent: 110,
                ..base.clone()
            },
        },
        Preset {
            id: "chaos",
            name: "Chaos total",
            description: "Tout est aléatoire : types, talents, statistiques, légendaires compris.",
            settings: Settings {
                starters: StarterMode::Random,
                wild: WildMode::Random,
                wild_similar_strength: false,
                trainers: TrainerMode::Random,
                trainers_similar_strength: false,
                stats: StatsMode::Random,
                random_types: true,
                random_abilities: true,
                no_legendaries: false,
                ..base.clone()
            },
        },
        Preset {
            id: "defi",
            name: "Défi",
            description: "Dresseurs 25 % plus forts et thématiques, sauvages inchangés.",
            settings: Settings { trainers: TrainerMode::TypeThemed, trainer_level_percent: 125, ..base },
        },
    ]
}

#[derive(Serialize, Deserialize)]
struct Shared {
    seed: u64,
    settings: Settings,
}

const SHARE_PREFIX: &str = "KLD1-";

/// Code à partager : redonne exactement la même randomisation sur la même ROM.
pub fn share_code(seed: u64, settings: &Settings) -> String {
    let json = serde_json::to_vec(&Shared { seed, settings: settings.clone() }).unwrap_or_default();
    format!("{SHARE_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
}

pub fn parse_share_code(code: &str) -> Option<(u64, Settings)> {
    let data = URL_SAFE_NO_PAD.decode(code.trim().strip_prefix(SHARE_PREFIX)?).ok()?;
    let shared: Shared = serde_json::from_slice(&data).ok()?;
    Some((shared.seed, shared.settings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_code_roundtrip() {
        let settings = presets()[2].settings.clone();
        let code = share_code(42, &settings);
        assert_eq!(parse_share_code(&code), Some((42, settings)));
        assert_eq!(parse_share_code("n'importe quoi"), None);
    }
}
