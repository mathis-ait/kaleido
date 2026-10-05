//! Base « Rencontres » pour l'interface : où trouver chaque Pokémon dans un jeu.

use std::collections::HashMap;

use serde::Serialize;

use super::encounters::{encounters, AbilityRule, EncounterKind, ShinyRule};
use crate::dex::{self, Game};

/// Une ligne de la base (rencontres regroupées par espèce, lieu et type).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterEntry {
    /// Indice d'une rencontre représentative dans [`encounters`] (pour « Créer ce Pokémon »).
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
}

fn location_label(game: Game, loc: u16) -> String {
    match loc {
        0 => "—".into(),
        l => dex::location_name(game.generation(), l).map_or_else(|| format!("Lieu n°{l}"), str::to_string),
    }
}

/// Toutes les rencontres du jeu, regroupées (niveaux fusionnés) et triées par n° de Pokédex.
pub fn encounter_db(game: Game) -> Vec<EncounterEntry> {
    let list = encounters(game);
    let mut out: Vec<EncounterEntry> = Vec::new();
    let mut groups: HashMap<(u16, u8, EncounterKind, u16, Vec<u8>, bool), usize> = HashMap::new();
    for (index, e) in list.iter().enumerate() {
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
        let form_name = if e.form >= 30 { Some("Forme variable".to_string()) } else if e.form != 0 { dex::form_name(game, e.species, e.form) } else { None };
        out.push(EncounterEntry {
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
            location_name: if e.kind == EncounterKind::EggGift { location_label(game, e.egg_location) } else { location_label(game, e.location) },
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
        });
    }
    out.sort_by_key(|e| (e.species, e.form, e.level_min));
    out
}
