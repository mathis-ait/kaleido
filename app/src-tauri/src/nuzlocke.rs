//! Commandes du mode Nuzlocke : lier une ROM (randomisée) à la sauvegarde ouverte,
//! calculer le bilan, enregistrer règles et marques manuelles.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kaleido_core::nuzlocke::{self, Report, RomInfo, RunState};
use kaleido_core::GameRom;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::saves::OpenSave;

/// ROMs déjà lues (la lecture prend une à deux secondes), par chemin.
#[derive(Default)]
pub struct RomCache(Mutex<HashMap<PathBuf, Arc<RomInfo>>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuzlockeView {
    pub state: RunState,
    /// Fichier où l'état est enregistré.
    pub state_file: String,
    /// La sauvegarde ouverte est-elle d'un jeu pris en charge (Platine, Noire, Blanche) ?
    pub supported: bool,
    pub save_game: String,
    pub report: Option<Report>,
    /// Problème avec la ROM liée (introuvable, autre jeu…).
    pub error: Option<String>,
}

fn supported(version: kaleido_core::save::SaveVersion) -> bool {
    use kaleido_core::save::SaveVersion::*;
    matches!(version, Platinum | BlackWhite | Black2White2)
}

/// Lit la ROM (ou la reprend du cache) hors du fil de l'interface.
async fn rom_info(app: &AppHandle, path: PathBuf) -> Result<Arc<RomInfo>, String> {
    if let Some(info) = app.state::<RomCache>().0.lock().map_err(|e| e.to_string())?.get(&path) {
        return Ok(info.clone());
    }
    let key = path.clone();
    let info = crate::blocking(move || {
        if !path.exists() {
            return Err(format!("ROM introuvable : {}", path.display()));
        }
        let game = GameRom::open(&path).map_err(|e| e.to_string())?;
        let mut info = nuzlocke::rom::read(&game).map_err(|e| e.to_string())?;
        info.seed = nuzlocke::rom::kaleido_seed(&path);
        Ok(Arc::new(info))
    })
    .await?;
    app.state::<RomCache>().0.lock().map_err(|e| e.to_string())?.insert(key, info.clone());
    Ok(info)
}

fn open_save(app: &AppHandle) -> Result<(PathBuf, kaleido_core::save::SaveVersion), String> {
    app.state::<OpenSave>().read(|p, s| Ok((p.to_path_buf(), s.save.version())))
}

async fn build(app: &AppHandle, save_path: &Path, state: RunState) -> Result<NuzlockeView, String> {
    let (_, version) = open_save(app)?;
    let mut view = NuzlockeView {
        state_file: nuzlocke::state_path(save_path).display().to_string(),
        supported: supported(version),
        save_game: version.label().to_string(),
        report: None,
        error: None,
        state,
    };
    if let (true, Some(rom)) = (view.supported, view.state.rom_path.clone()) {
        match rom_info(app, PathBuf::from(rom)).await {
            Ok(info) => match app.state::<OpenSave>().read(|_, s| nuzlocke::report(&info, s, &view.state).map_err(|e| e.to_string())) {
                Ok(r) => view.report = Some(r),
                Err(e) => view.error = Some(e),
            },
            Err(e) => view.error = Some(e),
        }
    }
    Ok(view)
}

/// État et bilan Nuzlocke de la sauvegarde ouverte.
#[tauri::command]
pub async fn nuzlocke_view(app: AppHandle) -> Result<NuzlockeView, String> {
    let (path, _) = open_save(&app)?;
    let state = nuzlocke::load_state(&path);
    build(&app, &path, state).await
}

/// Enregistre les règles et marques manuelles, puis recalcule le bilan.
#[tauri::command]
pub async fn nuzlocke_set_state(state: RunState, app: AppHandle) -> Result<NuzlockeView, String> {
    let (path, _) = open_save(&app)?;
    nuzlocke::store_state(&path, &state).map_err(|e| format!("enregistrement impossible : {e}"))?;
    build(&app, &path, state).await
}

/// Lie une ROM à la sauvegarde ouverte (`None` pour délier) après avoir vérifié le jeu.
#[tauri::command]
pub async fn nuzlocke_link_rom(rom: Option<String>, app: AppHandle) -> Result<NuzlockeView, String> {
    let (path, version) = open_save(&app)?;
    if let Some(rom) = &rom {
        let info = rom_info(&app, PathBuf::from(rom)).await?;
        if !nuzlocke::compatible(info.game, version) {
            return Err(format!("cette ROM ({}) ne correspond pas à la sauvegarde ({})", info.game.name_fr(), version.label()));
        }
    }
    let mut state = nuzlocke::load_state(&path);
    state.rom_path = rom;
    nuzlocke::store_state(&path, &state).map_err(|e| format!("enregistrement impossible : {e}"))?;
    build(&app, &path, state).await
}
