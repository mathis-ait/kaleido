// Pas de console en plus de la fenêtre en mode release sur Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use kaleido_core::detect::GameInfo;
use kaleido_core::pokemon::Species;
use kaleido_core::{Detection, GameRom};
use serde::Serialize;
use tauri::State;

/// ROM actuellement ouverte dans l'éditeur.
#[derive(Default)]
struct OpenRom(Mutex<Option<GameRom>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RomOverview {
    path: String,
    game: GameInfo,
    internal_title: String,
    game_code: String,
    file_count: usize,
    verified: bool,
    species: Vec<Species>,
}

/// Lance un travail bloquant hors du fil de l'interface.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Identifie un fichier ou un dossier déposé par l'utilisateur.
#[tauri::command]
async fn detect_file(path: PathBuf) -> Result<Detection, String> {
    blocking(move || kaleido_core::detect_path(&path).map_err(|e| e.to_string())).await
}

/// Charge une ROM DS et renvoie son Pokédex.
#[tauri::command]
async fn open_rom(path: PathBuf, state: State<'_, OpenRom>) -> Result<RomOverview, String> {
    let display = path.display().to_string();
    let (game, overview) = blocking(move || {
        let game = GameRom::open(&path).map_err(|e| e.to_string())?;
        let species = game.species().map_err(|e| e.to_string())?;
        let h = game.rom().header();
        let overview = RomOverview {
            path: display,
            game: game.game.into(),
            internal_title: h.title.clone(),
            game_code: h.game_code.clone(),
            file_count: game.rom().file_count(),
            verified: game.layout.verified,
            species,
        };
        Ok((game, overview))
    })
    .await?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(game);
    Ok(overview)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(OpenRom::default())
        .invoke_handler(tauri::generate_handler![detect_file, open_rom])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer Kaleido");
}
