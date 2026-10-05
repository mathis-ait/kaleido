//! Commandes de légalité : vérifier, rendre légal, générer un Pokémon légal, base
//! « Rencontres » (moteur : `kaleido_core::legality`).

use kaleido_core::legality::{self, games_for, species_entries, species_index, EncounterEntry, GenerateRequest, Report, SpeciesEncounters, Verdict};
use kaleido_core::save::session::{SaveView, Slot, SlotView};
use kaleido_core::save::{self, BOX_SLOTS};
use serde::Serialize;
use tauri::State;

use crate::saves::OpenSave;

/// Rapport de légalité d'un emplacement.
#[tauri::command]
pub fn legality_check(slot: Slot, state: State<'_, OpenSave>) -> Result<Report, String> {
    state.with(|s| {
        let pk = s.get(slot)?.ok_or_else(|| save::SaveError::Invalid("emplacement vide".into()))?;
        Ok(legality::analyze(&pk, s.game()))
    })
}

/// Résultat de « Tout vérifier » pour un Pokémon.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotReport {
    slot: Slot,
    #[serde(flatten)]
    report: Report,
}

fn all_slots(s: &save::session::SaveSession) -> Vec<Slot> {
    let mut out: Vec<Slot> = (0..s.save.party_count()).map(|index| Slot::Party { index }).collect();
    for b in 0..s.save.box_count() {
        out.extend((0..BOX_SLOTS).map(|index| Slot::Box { r#box: b, index }));
    }
    out
}

/// Vérifie tous les Pokémon de la sauvegarde (équipe puis boîtes).
#[tauri::command]
pub fn legality_check_all(state: State<'_, OpenSave>) -> Result<Vec<SlotReport>, String> {
    state.with(|s| {
        let game = s.game();
        let mut out = Vec::new();
        for slot in all_slots(s) {
            if let Some(pk) = s.get(slot)? {
                out.push(SlotReport { slot, report: legality::analyze(&pk, game) });
            }
        }
        Ok(out)
    })
}

/// Résultat de « Rendre légal ».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeResult {
    view: SlotView,
    changes: Vec<String>,
    report: Report,
    success: bool,
}

/// « Rendre légal » un emplacement (une seule étape d'annulation).
#[tauri::command]
pub fn legality_legalize(slot: Slot, state: State<'_, OpenSave>) -> Result<LegalizeResult, String> {
    state.with(|s| {
        let pk = s.get(slot)?.ok_or_else(|| save::SaveError::Invalid("emplacement vide".into()))?;
        let trainer = s.save.trainer();
        let out = legality::legalize(&pk, s.game(), &trainer);
        if !out.changes.is_empty() {
            s.replace_pokemon(vec![(slot, out.pokemon.clone())])?;
        }
        Ok(LegalizeResult { view: s.view_slot(slot)?, changes: out.changes, report: legality::analyze(&s.get(slot)?.unwrap_or(out.pokemon), s.game()), success: out.success })
    })
}

/// Bilan de « Tout rendre légal ».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeAllResult {
    view: SaveView,
    fixed: usize,
    failed: usize,
    untouched: usize,
}

/// Rend légaux tous les Pokémon illégaux (ou ceux de `slots`), en une seule étape d'annulation.
#[tauri::command]
pub fn legality_legalize_all(slots: Option<Vec<Slot>>, state: State<'_, OpenSave>) -> Result<LegalizeAllResult, String> {
    state.with(|s| {
        let game = s.game();
        let trainer = s.save.trainer();
        let targets = slots.unwrap_or_else(|| all_slots(s));
        let (mut fixed, mut failed, mut untouched) = (0, 0, 0);
        let mut changes = Vec::new();
        for slot in targets {
            let Some(pk) = s.get(slot)? else { continue };
            if legality::analyze(&pk, game).verdict != Verdict::Illegal {
                untouched += 1;
                continue;
            }
            let out = legality::legalize(&pk, game, &trainer);
            if out.success {
                fixed += 1;
                changes.push((slot, out.pokemon));
            } else {
                failed += 1;
            }
        }
        if !changes.is_empty() {
            s.replace_pokemon(changes)?;
        }
        Ok(LegalizeAllResult { view: s.view()?, fixed, failed, untouched })
    })
}

/// Résultat de « Générer un Pokémon légal ».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    view: SlotView,
    changes: Vec<String>,
    report: Report,
    success: bool,
}

/// Crée un Pokémon légal (capture ou éclosion dans ce jeu) dans `slot`, ou dans le premier
/// emplacement libre des boîtes (à partir de `from_box`).
#[tauri::command]
pub fn legality_generate(slot: Option<Slot>, from_box: Option<usize>, request: GenerateRequest, state: State<'_, OpenSave>) -> Result<GenerateResult, String> {
    state.with(|s| {
        let slot = match slot {
            Some(sl) => sl,
            None => s.first_empty_box_slot(from_box.unwrap_or(0)).ok_or_else(|| save::SaveError::Invalid("aucun emplacement libre dans les boîtes".into()))?,
        };
        let trainer = s.save.trainer();
        let out = legality::generate_legal(s.game(), s.save.format(), &trainer, &request).map_err(save::SaveError::Invalid)?;
        let used = s.replace_pokemon(vec![(slot, out.pokemon)])?;
        let slot = used.first().copied().unwrap_or(slot);
        Ok(GenerateResult { view: s.view_slot(slot)?, changes: out.changes, report: out.report, success: out.success })
    })
}

/// Base « Rencontres » : résumé par espèce, pour le jeu de la sauvegarde (`all = false`)
/// ou pour tous les jeux dont les Pokémon peuvent y être transférés.
#[tauri::command]
pub async fn encounter_species(all: bool, state: State<'_, OpenSave>) -> Result<Vec<SpeciesEncounters>, String> {
    let game = state.with(|s| Ok(s.game()))?;
    tauri::async_runtime::spawn_blocking(move || species_index(&games_for(game, all))).await.map_err(|e| e.to_string())
}

/// Base « Rencontres » : toutes les rencontres d'une espèce.
#[tauri::command]
pub async fn encounter_details(species: u16, all: bool, state: State<'_, OpenSave>) -> Result<Vec<EncounterEntry>, String> {
    let game = state.with(|s| Ok(s.game()))?;
    tauri::async_runtime::spawn_blocking(move || species_entries(&games_for(game, all), species)).await.map_err(|e| e.to_string())
}
