//! Caractéristiques des attaques par jeu.
//!
//! PP et types : tables `MoveInfo4..7` et `MoveInfo5/9.Type` de PKHeX.
//! Puissance, précision, priorité et catégorie : PokeAPI (`moves.csv`, valeurs
//! actuelles) corrigé par `move_changelog.csv` (valeurs d'avant chaque changement).

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Serialize;

use super::{max_move, Game};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

impl MoveCategory {
    pub fn name_fr(self) -> &'static str {
        match self {
            MoveCategory::Physical => "Physique",
            MoveCategory::Special => "Spéciale",
            MoveCategory::Status => "Statut",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveInfo {
    pub id: u16,
    /// Type (identifiant du jeu, voir [`super::type_name`]).
    pub type_id: u8,
    pub category: MoveCategory,
    /// PP de base, sans PP Plus.
    pub pp: u8,
    /// Puissance ; `None` pour les attaques de statut ou à puissance variable.
    pub power: Option<u8>,
    /// Précision en % ; `None` pour les attaques qui ne ratent jamais.
    pub accuracy: Option<u8>,
    pub priority: i8,
}

fn numbers(text: &'static str) -> Vec<u8> {
    text.split_ascii_whitespace().filter_map(|n| n.parse().ok()).collect()
}

static PP: LazyLock<[Vec<u8>; 4]> = LazyLock::new(|| {
    [
        numbers(include_str!("../../data/pkhex/moves/pp_g4.txt")),
        numbers(include_str!("../../data/pkhex/moves/pp_g5.txt")),
        numbers(include_str!("../../data/pkhex/moves/pp_g6.txt")),
        numbers(include_str!("../../data/pkhex/moves/pp_g7.txt")),
    ]
});

/// Types des attaques : Gen 2-5 (avant le type Fée) puis Gen 6+.
static TYPES_G5: LazyLock<Vec<u8>> = LazyLock::new(|| numbers(include_str!("../../data/pkhex/moves/type_g5.txt")));
static TYPES_G6: LazyLock<Vec<u8>> = LazyLock::new(|| numbers(include_str!("../../data/pkhex/moves/type_g9.txt")));

#[derive(Clone, Copy, Default)]
struct Stats {
    power: Option<u8>,
    accuracy: Option<u8>,
    priority: i8,
    category: Option<MoveCategory>,
}

/// Changement : valeurs en vigueur avant le groupe de versions `before` (ordre PokeAPI).
struct Change {
    before: u8,
    power: Option<u8>,
    accuracy: Option<u8>,
    priority: Option<i8>,
}

struct PokeApi {
    current: HashMap<u16, Stats>,
    changes: HashMap<u16, Vec<Change>>,
}

/// Ordre chronologique d'un groupe de versions PokeAPI (colonne `order` de `version_groups.csv`).
fn version_group_order(id: u8) -> u8 {
    match id {
        1..=6 => id + 2,
        7 => 11,
        8..=11 => id + 4,
        12 => 9,
        13 => 10,
        14..=27 => id + 2,
        _ => u8::MAX,
    }
}

/// Ordre du groupe de versions de chaque jeu.
fn game_order(game: Game) -> u8 {
    version_group_order(match game {
        Game::DP => 8,
        Game::Pt => 9,
        Game::HGSS => 10,
        Game::BW => 11,
        Game::B2W2 => 14,
        Game::XY => 15,
        Game::ORAS => 16,
        Game::SM => 17,
        Game::USUM => 18,
    })
}

fn csv_rows(text: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    text.lines().skip(1).filter(|l| !l.trim().is_empty()).map(|l| l.trim_end_matches('\r').split(',').collect())
}

static POKEAPI: LazyLock<PokeApi> = LazyLock::new(|| {
    let mut current = HashMap::new();
    for row in csv_rows(include_str!("../../data/pokeapi/moves.csv")) {
        let [id, power, accuracy, priority, class] = row[..] else { continue };
        let Ok(id) = id.parse::<u16>() else { continue };
        let category = match class {
            "1" => Some(MoveCategory::Status),
            "2" => Some(MoveCategory::Physical),
            "3" => Some(MoveCategory::Special),
            _ => None,
        };
        current.insert(id, Stats { power: power.parse().ok(), accuracy: accuracy.parse().ok(), priority: priority.parse().unwrap_or(0), category });
    }
    let mut changes: HashMap<u16, Vec<Change>> = HashMap::new();
    for row in csv_rows(include_str!("../../data/pokeapi/move_changelog.csv")) {
        let [id, group, power, accuracy, priority] = row[..] else { continue };
        let (Ok(id), Ok(group)) = (id.parse::<u16>(), group.parse::<u8>()) else { continue };
        changes.entry(id).or_default().push(Change {
            before: version_group_order(group),
            power: power.parse().ok(),
            accuracy: accuracy.parse().ok(),
            priority: priority.parse().ok(),
        });
    }
    for list in changes.values_mut() {
        list.sort_by_key(|c| c.before);
    }
    PokeApi { current, changes }
});

/// Valeurs en vigueur dans le groupe de versions d'ordre `order` : pour chaque champ,
/// le premier changement postérieur donne l'ancienne valeur, sinon la valeur actuelle.
fn stats_at(id: u16, order: u8) -> Stats {
    let data = &*POKEAPI;
    let mut stats = data.current.get(&id).copied().unwrap_or_default();
    let Some(changes) = data.changes.get(&id) else { return stats };
    let later = || changes.iter().filter(|c| c.before > order);
    if let Some(c) = later().find(|c| c.power.is_some()) {
        stats.power = c.power;
    }
    if let Some(c) = later().find(|c| c.accuracy.is_some()) {
        stats.accuracy = c.accuracy;
    }
    if let Some(p) = later().find_map(|c| c.priority) {
        stats.priority = p;
    }
    stats
}

/// Caractéristiques de l'attaque dans le jeu donné.
pub fn move_info_in(game: Game, id: u16) -> Option<MoveInfo> {
    if id == 0 || id > max_move(game) {
        return None;
    }
    let generation = game.generation();
    let pp = *PP[(generation - 4) as usize].get(id as usize)?;
    let types = if generation <= 5 { &TYPES_G5 } else { &TYPES_G6 };
    let type_id = *types.get(id as usize)?;
    let stats = stats_at(id, game_order(game));
    let category = stats.category.unwrap_or(MoveCategory::Status);
    Some(MoveInfo { id, type_id, category, pp, power: stats.power, accuracy: stats.accuracy, priority: stats.priority })
}

/// Caractéristiques de l'attaque dans Ultra-Soleil/Ultra-Lune (toutes les attaques Gen 4 à 7).
pub fn move_info(id: u16) -> Option<MoveInfo> {
    move_info_in(Game::USUM, id)
}
