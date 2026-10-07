//! Base « Rencontres » pour l'interface : où trouver chaque Pokémon, jeu par jeu.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use serde::Serialize;

use super::encounters::{encounters, AbilityRule, EncounterKind, ShinyRule};
use crate::dex::{self, Game};

/// Une ligne de la base (rencontres regroupées par espèce, lieu et type).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterEntry {
    /// Groupe de jeux et indice d'une rencontre représentative dans [`encounters`] (pour « Créer ce Pokémon »).
    pub game: Game,
    pub game_name: &'static str,
    pub index: usize,
    pub species: u16,
    pub form: u8,
    pub species_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_name: Option<String>,
    pub kind: EncounterKind,
    pub kind_label: &'static str,
    /// Famille pour les filtres : herbes, surf, peche, special, fixe, don, oeuf, echange, evenement.
    pub family: &'static str,
    pub level_min: u8,
    pub level_max: u8,
    pub location: u16,
    pub location_name: String,
    pub versions: Vec<u8>,
    pub version_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ball: Option<String>,
    pub ability: &'static str,
    pub hidden_ability: bool,
    pub shiny_lock: bool,
    pub shiny_always: bool,
    pub flawless_ivs: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_ivs: Option<[i8; 6]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nature: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held_item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trainer: Option<String>,
    pub fateful: bool,
    pub egg: bool,
    /// Titre de la carte (distributions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Résumé par espèce (liste de gauche de la page « Rencontres »).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeciesEncounters {
    pub species: u16,
    pub name: String,
    /// Familles de rencontres disponibles (herbes, surf, don…).
    pub families: Vec<&'static str>,
    pub count: usize,
    pub level_min: u8,
    pub games: Vec<&'static str>,
}

/// Nom français d'un groupe de jeux.
pub fn game_label(game: Game) -> &'static str {
    match game {
        Game::RB => "Rouge / Bleu",
        Game::Y => "Jaune",
        Game::GS => "Or / Argent",
        Game::C => "Cristal",
        Game::RS => "Rubis / Saphir",
        Game::E => "Émeraude",
        Game::FRLG => "Rouge Feu / Vert Feuille",
        Game::DP => "Diamant / Perle",
        Game::Pt => "Platine",
        Game::HGSS => "Or HeartGold / Argent SoulSilver",
        Game::BW => "Noir / Blanc",
        Game::B2W2 => "Noir 2 / Blanc 2",
        Game::XY => "X / Y",
        Game::ORAS => "Rubis Oméga / Saphir Alpha",
        Game::SM => "Soleil / Lune",
        Game::USUM => "Ultra-Soleil / Ultra-Lune",
    }
}

/// Groupe de jeux d'après son identifiant (`dp`, `pt`, `hgss`, `bw`, `b2w2`, `xy`, `oras`, `sm`, `usum`).
pub fn game_from_id(id: &str) -> Option<Game> {
    Game::ALL.iter().copied().find(|g| serde_json::to_value(g).ok().and_then(|v| v.as_str().map(|s| s == id)).unwrap_or(false))
}

fn location_label(game: Game, loc: u16) -> String {
    match loc {
        0 => "—".into(),
        l => dex::location_name(game.generation(), l).map_or_else(|| format!("Lieu n°{l}"), str::to_string),
    }
}

type GroupKey = (u16, u8, EncounterKind, u16, Vec<u8>, bool);

/// Indices des distributions dans la base (après les rencontres du jeu).
pub const EVENT_BASE: usize = 1_000_000;

/// Rencontre d'après son indice dans la base d'un jeu (rencontres puis distributions).
pub fn encounter_by_index(game: Game, index: usize) -> Option<super::encounters::Encounter> {
    if index >= EVENT_BASE {
        super::events::events(game.generation()).get(index - EVENT_BASE).cloned()
    } else {
        encounters(game).get(index).cloned()
    }
}

fn build(game: Game) -> Vec<EncounterEntry> {
    let list = encounters(game);
    let mut out: Vec<EncounterEntry> = Vec::new();
    let mut groups: HashMap<GroupKey, usize> = HashMap::new();
    let versions_of_game = super::encounters::game_versions(game);
    let event_list = super::events::events(game.generation())
        .iter()
        .enumerate()
        .filter(|(_, e)| e.versions.iter().any(|v| versions_of_game.contains(v)))
        .map(|(i, e)| (EVENT_BASE + i, e));
    for (index, e) in list.iter().enumerate().chain(event_list) {
        let mut versions = e.versions.clone();
        versions.sort();
        let hidden = matches!(e.ability, AbilityRule::OnlyHidden | AbilityRule::Any12H);
        if e.kind.is_wild() {
            let key = (e.species, e.form, e.kind, e.location, versions.clone(), hidden);
            if let Some(&i) = groups.get(&key) {
                let g = &mut out[i];
                g.level_min = g.level_min.min(e.level_min);
                g.level_max = g.level_max.max(e.level_max);
                continue;
            }
            groups.insert(key, out.len());
        }
        let form_name = if e.form >= 30 {
            Some("Forme variable".to_string())
        } else if e.form != 0 {
            dex::form_name(game, e.species, e.form)
        } else {
            None
        };
        out.push(EncounterEntry {
            game,
            game_name: game_label(game),
            index,
            species: e.species,
            form: e.form,
            species_name: dex::species_name(e.species).unwrap_or("?").to_string(),
            form_name,
            kind: e.kind,
            kind_label: e.kind.label(),
            family: e.kind.family(),
            level_min: e.level_min,
            level_max: e.level_max,
            location: e.location,
            location_name: if e.is_egg() && e.location == 0 { location_label(game, e.egg_location) } else { location_label(game, e.location) },
            version_names: versions.iter().map(|&v| dex::game_name(v).unwrap_or("?").to_string()).collect(),
            versions,
            ball: e.ball.and_then(dex::ball_name).map(str::to_string),
            ability: e.ability.label(),
            hidden_ability: hidden,
            shiny_lock: e.shiny == ShinyRule::Never,
            shiny_always: e.shiny == ShinyRule::Always,
            flawless_ivs: e.flawless_ivs,
            fixed_ivs: e.ivs,
            gender: e.gender.filter(|&g| g < 3),
            nature: e.nature.and_then(dex::nature_name).map(str::to_string),
            moves: e.moves.iter().filter_map(|&m| dex::move_name(m)).map(str::to_string).collect(),
            held_item: (e.held_item != 0).then(|| dex::item_name_in(game, e.held_item).unwrap_or("?").to_string()),
            trainer: e.trainer.as_ref().map(|t| {
                let name = t.names.iter().find(|(l, _)| *l == 3).or(t.names.first()).map(|(_, n)| n.clone()).unwrap_or_default();
                format!("{name} (ID {})", t.tid)
            }),
            fateful: e.fateful,
            egg: e.is_egg(),
            title: e.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string),
        });
    }
    out.sort_by_key(|e| (e.species, e.form, e.level_min));
    out
}

static TABLES: [LazyLock<Vec<EncounterEntry>>; 16] = [
    LazyLock::new(|| build(Game::DP)),
    LazyLock::new(|| build(Game::Pt)),
    LazyLock::new(|| build(Game::HGSS)),
    LazyLock::new(|| build(Game::BW)),
    LazyLock::new(|| build(Game::B2W2)),
    LazyLock::new(|| build(Game::XY)),
    LazyLock::new(|| build(Game::ORAS)),
    LazyLock::new(|| build(Game::SM)),
    LazyLock::new(|| build(Game::USUM)),
    LazyLock::new(|| build(Game::RS)),
    LazyLock::new(|| build(Game::E)),
    LazyLock::new(|| build(Game::FRLG)),
    LazyLock::new(|| build(Game::RB)),
    LazyLock::new(|| build(Game::Y)),
    LazyLock::new(|| build(Game::GS)),
    LazyLock::new(|| build(Game::C)),
];

/// Toutes les rencontres du jeu, regroupées (niveaux fusionnés) et triées par n° de Pokédex.
pub fn encounter_db(game: Game) -> &'static [EncounterEntry] {
    let i = Game::ALL.iter().position(|&g| g == game).unwrap_or(0);
    &TABLES[i]
}

/// Jeux consultés : celui de la sauvegarde, ou tous ceux dont les Pokémon peuvent y arriver.
pub fn games_for(save: Game, all: bool) -> Vec<Game> {
    if all {
        Game::ALL.iter().copied().filter(|g| g.generation() <= save.generation()).collect()
    } else {
        vec![save]
    }
}

/// Résumé par espèce pour une liste de jeux.
pub fn species_index(games: &[Game]) -> Vec<SpeciesEncounters> {
    let mut map: BTreeMap<u16, SpeciesEncounters> = BTreeMap::new();
    for &g in games {
        for e in encounter_db(g) {
            let s = map.entry(e.species).or_insert_with(|| SpeciesEncounters {
                species: e.species,
                name: e.species_name.clone(),
                families: Vec::new(),
                count: 0,
                level_min: e.level_min,
                games: Vec::new(),
            });
            s.count += 1;
            s.level_min = s.level_min.min(e.level_min);
            if !s.families.contains(&e.family) {
                s.families.push(e.family);
            }
            if !s.games.contains(&e.game_name) {
                s.games.push(e.game_name);
            }
        }
    }
    map.into_values().collect()
}

/// Rencontres d'une espèce dans une liste de jeux.
pub fn species_entries(games: &[Game], species: u16) -> Vec<EncounterEntry> {
    games.iter().flat_map(|&g| encounter_db(g).iter().filter(move |e| e.species == species).cloned()).collect()
}
