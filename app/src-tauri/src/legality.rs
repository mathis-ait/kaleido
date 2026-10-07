//! Commandes de légalité : vérifier, rendre légal, générer un Pokémon légal, base
//! « Rencontres » (moteur : `kaleido_core::legality`).

use kaleido_core::legality::{self, games_for, species_entries, species_index, Change, EncounterEntry, EncounterOption, GenerateRequest, Report, SpeciesEncounters, Verdict};
use kaleido_core::save::session::{view_of, SaveView, Slot, SlotView};
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

/// Résultat de « Rendre légal » (appliqué).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeResult {
    view: SlotView,
    changes: Vec<Change>,
    report: Report,
    success: bool,
    encounter: Option<EncounterOption>,
}

/// Aperçu de « Rendre légal » : le Pokémon avant et après, sans rien écrire.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizePreview {
    before: SlotView,
    after: SlotView,
    changes: Vec<Change>,
    report: Report,
    success: bool,
    /// Rencontre retenue (`None` : rencontre actuelle gardée).
    encounter: Option<EncounterOption>,
    /// Rencontres valides, la plus naturelle d'abord.
    options: Vec<EncounterOption>,
}

fn occupied(s: &save::session::SaveSession, slot: Slot) -> Result<save::Pokemon, save::SaveError> {
    s.get(slot)?.ok_or_else(|| save::SaveError::Invalid("emplacement vide".into()))
}

/// Aperçu de « Rendre légal » pour un emplacement, avec la rencontre `choice` (numéro
/// d'une des `options`) ou celle choisie par le moteur.
#[tauri::command]
pub fn legality_legalize_preview(slot: Slot, choice: Option<usize>, state: State<'_, OpenSave>) -> Result<LegalizePreview, String> {
    state.with(|s| {
        let pk = occupied(s, slot)?;
        let game = s.game();
        let out = legality::legalize_with(&pk, game, &s.save.trainer(), choice, true);
        Ok(LegalizePreview {
            before: view_of(game, slot, &pk),
            after: view_of(game, slot, &out.pokemon),
            changes: out.changes,
            report: out.report,
            success: out.success,
            encounter: out.encounter,
            options: out.options,
        })
    })
}

/// Applique « Rendre légal » (même résultat que l'aperçu, une seule étape d'annulation).
#[tauri::command]
pub fn legality_legalize_apply(slot: Slot, choice: Option<usize>, state: State<'_, OpenSave>) -> Result<LegalizeResult, String> {
    state.with(|s| {
        let pk = occupied(s, slot)?;
        let game = s.game();
        let out = legality::legalize_with(&pk, game, &s.save.trainer(), choice, false);
        if !out.changes.is_empty() {
            s.replace_pokemon(vec![(slot, out.pokemon.clone())])?;
        }
        Ok(LegalizeResult {
            view: s.view_slot(slot)?,
            changes: out.changes,
            report: legality::analyze(&s.get(slot)?.unwrap_or(out.pokemon), game),
            success: out.success,
            encounter: out.encounter,
        })
    })
}

/// « Rendre légal » sans aperçu (ancienne commande, gardée pour les raccourcis).
#[tauri::command]
pub fn legality_legalize(slot: Slot, state: State<'_, OpenSave>) -> Result<LegalizeResult, String> {
    legality_legalize_apply(slot, None, state)
}

/// Une ligne de l'aperçu groupé.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupItem {
    slot: Slot,
    before: SlotView,
    after: SlotView,
    changes: Vec<Change>,
    success: bool,
    encounter: Option<EncounterOption>,
}

/// Bilan de « Tout rendre légal ».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeAllResult {
    /// État de la sauvegarde après application (`None` en aperçu).
    view: Option<SaveView>,
    fixed: usize,
    failed: usize,
    untouched: usize,
    /// Détail par Pokémon illégal traité.
    items: Vec<GroupItem>,
}

/// Rend légaux tous les Pokémon illégaux (ou ceux de `slots`), en une seule étape
/// d'annulation ; `preview` : calcule seulement le résultat, sans rien écrire.
#[tauri::command]
pub async fn legality_legalize_all(slots: Option<Vec<Slot>>, preview: Option<bool>, state: State<'_, OpenSave>) -> Result<LegalizeAllResult, String> {
    let preview = preview.unwrap_or(false);
    state.with(|s| {
        let game = s.game();
        let trainer = s.save.trainer();
        let targets = slots.unwrap_or_else(|| all_slots(s));
        let (mut fixed, mut failed, mut untouched) = (0, 0, 0);
        let mut changes = Vec::new();
        let mut items = Vec::new();
        for slot in targets {
            let Some(pk) = s.get(slot)? else { continue };
            if legality::analyze(&pk, game).verdict != Verdict::Illegal {
                untouched += 1;
                continue;
            }
            let out = legality::legalize(&pk, game, &trainer);
            items.push(GroupItem {
                slot,
                before: view_of(game, slot, &pk),
                after: view_of(game, slot, &out.pokemon),
                changes: out.changes,
                success: out.success,
                encounter: out.encounter,
            });
            if out.success {
                fixed += 1;
                changes.push((slot, out.pokemon));
            } else {
                failed += 1;
            }
        }
        if !preview && !changes.is_empty() {
            s.replace_pokemon(changes)?;
        }
        Ok(LegalizeAllResult { view: if preview { None } else { Some(s.view()?) }, fixed, failed, untouched, items })
    })
}

/// Résultat de « Générer un Pokémon légal ».
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    view: SlotView,
    changes: Vec<Change>,
    report: Report,
    success: bool,
    encounter: Option<EncounterOption>,
}

/// Crée un Pokémon légal (capture ou éclosion dans ce jeu) dans `slot`, ou dans le premier
/// emplacement libre des boîtes (à partir de `from_box`).
#[tauri::command]
pub fn legality_generate(
    slot: Option<Slot>,
    from_box: Option<usize>,
    request: GenerateRequest,
    state: State<'_, OpenSave>,
) -> Result<GenerateResult, String> {
    state.with(|s| {
        let slot = match slot {
            Some(sl) => sl,
            None => s
                .first_empty_box_slot(from_box.unwrap_or(0))
                .ok_or_else(|| save::SaveError::Invalid("aucun emplacement libre dans les boîtes".into()))?,
        };
        let trainer = s.save.trainer();
        let out = legality::generate_legal(s.game(), s.save.format(), &trainer, &request).map_err(save::SaveError::Invalid)?;
        let used = s.replace_pokemon(vec![(slot, out.pokemon)])?;
        let slot = used.first().copied().unwrap_or(slot);
        Ok(GenerateResult { view: s.view_slot(slot)?, changes: out.changes, report: out.report, success: out.success, encounter: out.encounter })
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
