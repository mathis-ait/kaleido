// Pas de console en plus de la fenêtre en mode release sur Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kaleido_core::detect::GameInfo;
use kaleido_core::pokemon::Species;
use kaleido_core::randomizer::ctr::LayeredFsTarget;
use kaleido_core::randomizer::{self, Outcome, PokemonRef, Preset, Settings};
use kaleido_core::{CtrGameRom, Detection, GameRom};
use serde::Serialize;
use tauri::{AppHandle, Manager};

mod saves;
mod sprites;

/// ROM ouverte : DS (chargée en mémoire) ou 3DS (lue à la demande).
enum Loaded {
    Nds(GameRom),
    Ctr(CtrGameRom),
}

impl Loaded {
    fn open(path: &Path) -> Result<Self, String> {
        let is_nds = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"));
        if is_nds {
            GameRom::open(path).map(Loaded::Nds).map_err(|e| e.to_string())
        } else {
            CtrGameRom::open(path).map(Loaded::Ctr).map_err(|e| e.to_string())
        }
    }
}

/// Dernière ROM ouverte (éditeur, aperçu du randomizer), gardée en mémoire.
#[derive(Default)]
struct OpenRom(Mutex<Option<(PathBuf, Loaded)>>);

impl OpenRom {
    /// Exécute `f` sur la ROM `path`, en la chargeant si besoin.
    fn with<T>(&self, path: &Path, f: impl FnOnce(&Loaded) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if slot.as_ref().is_none_or(|(p, _)| p != path) {
            *slot = Some((path.to_path_buf(), Loaded::open(path)?));
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
pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Identifie un fichier ou un dossier déposé par l'utilisateur.
#[tauri::command]
async fn detect_file(path: PathBuf) -> Result<Detection, String> {
    blocking(move || kaleido_core::detect_path(&path).map_err(|e| e.to_string())).await
}

/// Remplace chaque dossier ordinaire par les ROMs et sauvegardes qu'il contient.
#[tauri::command]
fn expand_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.iter().flat_map(|p| kaleido_core::detect::expand_path(p)).collect()
}

/// Charge une ROM (DS ou 3DS) et renvoie son Pokédex.
#[tauri::command]
async fn open_rom(path: PathBuf, app: AppHandle) -> Result<RomOverview, String> {
    blocking(move || {
        app.state::<OpenRom>().with(&path, |loaded| {
            let display = path.display().to_string();
            Ok(match loaded {
                Loaded::Nds(game) => {
                    let h = game.rom().header();
                    RomOverview {
                        path: display,
                        game: game.game.into(),
                        internal_title: h.title.clone(),
                        game_code: h.game_code.clone(),
                        file_count: game.rom().file_count(),
                        verified: game.layout.verified,
                        can_randomize: randomizer::supports(game),
                        species: game.species().map_err(|e| e.to_string())?,
                    }
                }
                Loaded::Ctr(game) => RomOverview {
                    path: display,
                    game: game.game.into(),
                    internal_title: game.game.name_fr().to_string(),
                    game_code: format!("{:016X}", game.title_id()),
                    file_count: game.romfs().files().len(),
                    verified: game.layout.verified,
                    can_randomize: randomizer::ctr::supports(game.game),
                    species: game.species().map_err(|e| e.to_string())?,
                },
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
        app.state::<OpenRom>().with(&path, |loaded| match loaded {
            Loaded::Nds(game) => randomizer::preview_starters(game, &settings, seed).map_err(|e| e.to_string()),
            Loaded::Ctr(game) => randomizer::ctr::preview_starters(game, &settings, seed).map_err(|e| e.to_string()),
        })
    })
    .await
}

/// Randomise une copie fraîche de la ROM DS et l'écrit dans `output` (+ journal `.txt`).
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CtrOutcome {
    #[serde(flatten)]
    outcome: Outcome,
    romfs: String,
}

/// Randomise un jeu 3DS vers un dossier LayeredFS (+ journal).
#[tauri::command]
async fn randomize_ctr(path: PathBuf, settings: Settings, seed: u64, output: PathBuf, target: LayeredFsTarget) -> Result<CtrOutcome, String> {
    blocking(move || {
        let game = CtrGameRom::open(&path).map_err(|e| e.to_string())?;
        let (outcome, romfs) = randomizer::ctr::randomize(&game, &settings, seed, &output, target).map_err(|e| e.to_string())?;
        std::fs::write(output.join(format!("Kaleido {seed} - journal.txt")), &outcome.log).map_err(|e| e.to_string())?;
        Ok(CtrOutcome { outcome, romfs: romfs.display().to_string() })
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
        .manage(saves::OpenSave::default())
        .register_asynchronous_uri_scheme_protocol("sprite", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(sprites::handle(&app, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            detect_file,
            expand_paths,
            open_rom,
            randomizer_presets,
            preview_starters,
            randomize_rom,
            randomize_ctr,
            parse_share_code,
            sprite_cache_info,
            clear_sprite_cache,
            saves::open_save,
            saves::save_view,
            saves::save_box,
            saves::save_move,
            saves::save_patch,
            saves::save_delete,
            saves::save_export_pokemon,
            saves::save_import_pokemon,
            saves::save_write,
            saves::save_history,
            saves::save_copy,
            saves::save_create,
            saves::save_all,
            saves::save_set_trainer,
            saves::save_set_box_name,
            saves::save_path,
            saves::name_lists
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer Kaleido");
}
