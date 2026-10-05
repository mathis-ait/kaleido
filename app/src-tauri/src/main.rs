// Pas de console en plus de la fenêtre en mode release sur Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use kaleido_core::detect::GameInfo;
use kaleido_core::pokemon::Species;
use kaleido_core::randomizer::{self, Outcome, PokemonRef, Preset, Settings};
use kaleido_core::{Detection, GameRom};
use serde::Serialize;
use tauri::{AppHandle, Manager};

mod sprites;

/// Dernière ROM ouverte (éditeur, aperçu du randomizer), gardée en mémoire.
#[derive(Default)]
struct OpenRom(Mutex<Option<(PathBuf, GameRom)>>);

impl OpenRom {
    /// Exécute `f` sur la ROM `path`, en la chargeant si besoin.
    fn with<T>(&self, path: &PathBuf, f: impl FnOnce(&GameRom) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if slot.as_ref().is_none_or(|(p, _)| p != path) {
            *slot = Some((path.clone(), GameRom::open(path).map_err(|e| e.to_string())?));
        }
        f(&slot.as_ref().unwrap().1)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RomOverview {
    path: String,
    game: GameInfo,
    internal_title: String,
    game_code: String,
    file_count: usize,
    verified: bool,
    can_randomize: bool,
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
async fn open_rom(path: PathBuf, app: AppHandle) -> Result<RomOverview, String> {
    blocking(move || {
        let state = app.state::<OpenRom>();
        state.with(&path, |game| {
            let h = game.rom().header();
            Ok(RomOverview {
                path: path.display().to_string(),
                game: game.game.into(),
                internal_title: h.title.clone(),
                game_code: h.game_code.clone(),
                file_count: game.rom().file_count(),
                verified: game.layout.verified,
                can_randomize: randomizer::supports(game),
                species: game.species().map_err(|e| e.to_string())?,
            })
        })
    })
    .await
}

#[tauri::command]
fn randomizer_presets() -> Vec<Preset> {
    randomizer::presets()
}

#[tauri::command]
async fn preview_starters(path: PathBuf, settings: Settings, seed: u64, app: AppHandle) -> Result<Vec<PokemonRef>, String> {
    blocking(move || {
        let state = app.state::<OpenRom>();
        state.with(&path, |game| randomizer::preview_starters(game, &settings, seed).map_err(|e| e.to_string()))
    })
    .await
}

/// Randomise une copie fraîche de la ROM et l'écrit dans `output` (+ journal `.txt`).
#[tauri::command]
async fn randomize_rom(path: PathBuf, settings: Settings, seed: u64, output: PathBuf) -> Result<Outcome, String> {
    blocking(move || {
        if output == path {
            return Err("choisis un autre fichier : la ROM d'origine ne doit pas être écrasée".into());
        }
        let mut game = GameRom::open(&path).map_err(|e| e.to_string())?;
        let outcome = randomizer::randomize(&mut game, &settings, seed).map_err(|e| e.to_string())?;
        game.save(&output).map_err(|e| e.to_string())?;
        let log_path = output.with_extension("journal.txt");
        std::fs::write(&log_path, &outcome.log).map_err(|e| e.to_string())?;
        Ok(outcome)
    })
    .await
}

#[tauri::command]
fn parse_share_code(code: String) -> Option<(u64, Settings)> {
    randomizer::parse_share_code(&code)
}

#[tauri::command]
fn sprite_cache_info(app: AppHandle) -> Result<sprites::CacheInfo, String> {
    sprites::cache_info(&app)
}

#[tauri::command]
fn clear_sprite_cache(app: AppHandle) -> Result<(), String> {
    sprites::clear_cache(&app)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(OpenRom::default())
        .register_asynchronous_uri_scheme_protocol("sprite", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(sprites::handle(&app, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            detect_file,
            open_rom,
            randomizer_presets,
            preview_starters,
            randomize_rom,
            parse_share_code,
            sprite_cache_info,
            clear_sprite_cache
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer Kaleido");
}
