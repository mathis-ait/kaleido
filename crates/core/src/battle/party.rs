//! Équipe du joueur : Pokémon de la sauvegarde → [`Combatant`] (valeurs exactes).

use super::trainers::RomTrainers;
use super::{types, Combatant};
use crate::dex;
use crate::save::session::SlotView;

/// Convertit un Pokémon de la sauvegarde. Avec une ROM liée, les types viennent
/// de la ROM (types randomisés) ; sinon des données du jeu d'origine.
pub fn from_slot(game: dex::Game, v: &SlotView, rom: Option<&RomTrainers>) -> Option<Combatant> {
    let s = &v.summary;
    if s.is_egg || s.species == 0 {
        return None;
    }
    let mut notes = Vec::new();
    let types = match rom.and_then(|r| r.species_types(s.species, s.form as u16)) {
        Some(t) => t,
        None => v.species_data.as_ref().map(|d| d.types.clone()).unwrap_or_else(|| vec![types::NORMAL]),
    };
    let stats = v.stats.unwrap_or_else(|| {
        notes.push("Statistiques inconnues (fiche de l'espèce introuvable).".into());
        [1; 6]
    });
    let weight = dex::personal(game, s.species, s.form)
        .map(|p| p.weight)
        .filter(|&w| w > 0)
        .or_else(|| dex::personal(dex::Game::B2W2, s.species, s.form).map(|p| p.weight))
        .unwrap_or(0);
    Some(Combatant {
        species: s.species,
        form: s.form,
        name: if s.nickname.is_empty() { v.species_name.clone() } else { s.nickname.clone() },
        level: s.level,
        types,
        stats,
        ability: s.ability,
        item: s.held_item,
        moves: s.moves,
        nature: s.nature,
        ivs: s.ivs,
        evs: s.evs,
        friendship: s.friendship,
        weight,
        notes,
    })
}
