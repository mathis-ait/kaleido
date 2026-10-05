//! Commandes de l'éditeur de sauvegardes.

use std::path::PathBuf;
use std::sync::Mutex;

use kaleido_core::save::session::{PokemonPatch, SaveSession, SaveView, Slot, SlotView};
use kaleido_core::{names, save};
use serde::Serialize;
use tauri::State;

/// Sauvegarde ouverte et son chemin d'origine.
#[derive(Default)]
pub struct OpenSave(Mutex<Option<(PathBuf, SaveSession)>>);

impl OpenSave {
    fn with<T>(&self, f: impl FnOnce(&mut SaveSession) -> Result<T, save::SaveError>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        let (_, session) = slot.as_mut().ok_or("aucune sauvegarde ouverte")?;
        f(session).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn open_save(path: PathBuf, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
    let session = SaveSession::open(&bytes).map_err(|e| e.to_string())?;
    let view = session.view().map_err(|e| e.to_string())?;
    *state.0.lock().map_err(|e| e.to_string())? = Some((path, session));
    Ok(view)
}

#[tauri::command]
pub fn save_view(state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| s.view())
}

#[tauri::command]
pub fn save_box(index: usize, state: State<'_, OpenSave>) -> Result<Vec<Option<SlotView>>, String> {
    state.with(|s| s.box_view(index))
}

/// Déplace (ou échange) un Pokémon, puis renvoie l'état de l'équipe.
#[tauri::command]
pub fn save_move(from: Slot, to: Slot, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.move_pokemon(from, to)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_patch(slot: Slot, patch: PokemonPatch, state: State<'_, OpenSave>) -> Result<SlotView, String> {
    state.with(|s| s.patch(slot, &patch))
}

#[tauri::command]
pub fn save_delete(slot: Slot, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.delete(slot)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_export_pokemon(slot: Slot, output: PathBuf, state: State<'_, OpenSave>) -> Result<(), String> {
    let (bytes, _) = state.with(|s| s.export(slot))?;
    std::fs::write(output, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_import_pokemon(slot: Slot, file: PathBuf, state: State<'_, OpenSave>) -> Result<SlotView, String> {
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    state.with(|s| s.import(slot, &bytes))
}

/// Écrit la sauvegarde. Avant d'écraser le fichier d'origine, une copie de
/// sécurité est faite à côté (`.kaleido.bak`). Renvoie le chemin de cette copie.
#[tauri::command]
pub fn save_write(output: Option<PathBuf>, state: State<'_, OpenSave>) -> Result<Option<String>, String> {
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    let (path, session) = slot.as_ref().ok_or("aucune sauvegarde ouverte")?;
    let target = output.unwrap_or_else(|| path.clone());
    let mut backup = None;
    if target.exists() {
        let mut name = target.file_name().unwrap_or_default().to_os_string();
        name.push(".kaleido.bak");
        let bak = target.with_file_name(name);
        if !bak.exists() {
            std::fs::copy(&target, &bak).map_err(|e| format!("copie de sécurité impossible : {e}"))?;
        }
        backup = Some(bak.display().to_string());
    }
    std::fs::write(&target, session.to_bytes()).map_err(|e| e.to_string())?;
    Ok(backup)
}

#[derive(Serialize)]
pub struct NameLists {
    species: &'static [String],
    moves: &'static [String],
    items: &'static [String],
}

/// Listes de noms pour les menus (espèces, attaques, objets).
#[tauri::command]
pub fn name_lists() -> NameLists {
    NameLists { species: names::all_species(), moves: names::all_moves(), items: names::all_items() }
}
