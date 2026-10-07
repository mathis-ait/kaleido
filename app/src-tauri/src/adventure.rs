//! « Nouvelle aventure » depuis le lanceur : aperçu de la partie randomisée avant
//! d'écrire la ROM, presets enregistrés par l'utilisateur, suivi Nuzlocke activé d'office.

use std::fs;
use std::path::PathBuf;

use kaleido_core::nuzlocke;
use kaleido_core::randomizer::preview::{self, Preview};
use kaleido_core::randomizer::Settings;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::blocking;

/// Aperçu : starters, première route et premier champion de la partie tirée avec `seed`.
#[tauri::command]
pub async fn adventure_preview(path: PathBuf, settings: Settings, seed: u64, app: AppHandle) -> Result<Preview, String> {
    let scratch = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("adventure-preview");
    blocking(move || {
        let nds = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"));
        let result = if crate::is_gba(&path) {
            preview::preview_gba(&path, &settings, seed)
        } else if nds {
            preview::preview_nds(&path, &settings, seed)
        } else {
            preview::preview_ctr(&path, &settings, seed, &scratch)
        };
        result.map_err(|e| e.to_string())
    })
    .await
}

fn presets_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("presets.json"))
}

/// Presets enregistrés depuis le formulaire détaillé du randomizer (même format que
/// les fichiers de `app/src/presets`).
#[tauri::command]
pub fn user_presets(app: AppHandle) -> Vec<Value> {
    presets_file(&app).ok().and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn save_user_presets(presets: Vec<Value>, app: AppHandle) -> Result<(), String> {
    let path = presets_file(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(&presets).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Preset « Nuzlocke prêt » : la sauvegarde de la nouvelle partie est suivie avec sa ROM
/// (règles par défaut), avant même la première sauvegarde en jeu.
#[tauri::command]
pub fn nuzlocke_track_save(save: PathBuf, rom: String) -> Result<(), String> {
    let mut state = nuzlocke::load_state(&save);
    state.rom_path = Some(rom);
    nuzlocke::store_state(&save, &state).map_err(|e| format!("enregistrement impossible : {e}"))
}
