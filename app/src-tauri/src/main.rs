// Pas de console en plus de la fenêtre en mode release sur Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kaleido_core::detect::GameInfo;
use kaleido_core::pokemon::Species;
use kaleido_core::randomizer::ctr::{CtrOutput, LayeredFsTarget};
use kaleido_core::randomizer::{self, Outcome, PokemonRef, Preset, Settings};
use kaleido_core::romedit::{self, EditorData, SpeciesData};
use kaleido_core::romedit_ctr;
use kaleido_core::{CtrGameRom, Detection, GameRom};
use serde::Serialize;
use tauri::{AppHandle, Manager};

mod adventure;
mod bank;
mod companion;
mod runlog;
mod discover;
mod battle;
mod emusaves;
mod gifts;
mod legality;
mod library;
mod mods;
mod nuzlocke;
mod nxmusic;
mod play;
mod saves;
mod showdown;
mod sprites;
mod switch;
mod teams;
mod tuning;
mod updates;
mod video;

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
    /// Fiches et attaques apprises modifiables par l'éditeur.
    can_edit: bool,
    species: Vec<Species>,
}

/// Vrai si les deux chemins désignent le même fichier (casse, `..`, liens : Windows compris).
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
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
                        can_edit: romedit::supports(game),
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
                    can_edit: romedit_ctr::supports(game),
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

/// Réglages du moteur quand rien n'est demandé (tout inchangé) : base des presets « Nouvelle aventure ».
#[tauri::command]
fn randomizer_defaults() -> Settings {
    Settings::default()
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
        if same_file(&output, &path) {
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
    /// Dossier `romfs` du LayeredFS (si demandé).
    romfs: Option<String>,
    /// ROM `.3ds` / `.cxi` reconstruite (si demandée).
    image: Option<String>,
}

/// Randomise un jeu 3DS vers un dossier LayeredFS et/ou une ROM complète (+ journal),
/// selon `settings.ctr_output`.
#[tauri::command]
async fn randomize_ctr(path: PathBuf, settings: Settings, seed: u64, output: PathBuf, target: LayeredFsTarget) -> Result<CtrOutcome, String> {
    blocking(move || {
        let game = CtrGameRom::open(&path).map_err(|e| e.to_string())?;
        let (outcome, written) = randomizer::ctr::randomize(&game, &settings, seed, &output, target).map_err(|e| e.to_string())?;
        std::fs::write(output.join(format!("Kaleido {seed} - journal.txt")), &outcome.log).map_err(|e| e.to_string())?;
        // Le mod garde l'adresse du jeu d'origine : Nuzlocke, Combat et compagnon le relisent par-dessus.
        if let Some(title_dir) = written.romfs.as_ref().and_then(|r| r.parent()) {
            let _ = std::fs::write(title_dir.join(kaleido_core::formats::romfs::MOD_BASE_FILE), path.display().to_string());
        }
        let show = |p: Option<PathBuf>| p.map(|p| p.display().to_string());
        Ok(CtrOutcome { outcome, romfs: show(written.romfs), image: show(written.image) })
    })
    .await
}

/// Données modifiables de la ROM ouverte (DS ou 3DS).
#[tauri::command]
async fn rom_editor_data(path: PathBuf, app: AppHandle) -> Result<EditorData, String> {
    blocking(move || {
        app.state::<OpenRom>().with(&path, |loaded| match loaded {
            Loaded::Nds(game) => romedit::read(game).map_err(|e| e.to_string()),
            Loaded::Ctr(game) => romedit_ctr::read(game).map_err(|e| e.to_string()),
        })
    })
    .await
}

/// Écrit une copie de la ROM avec les espèces modifiées. Renvoie le nombre d'espèces changées.
#[tauri::command]
async fn rom_editor_save(path: PathBuf, edits: Vec<SpeciesData>, output: PathBuf) -> Result<usize, String> {
    blocking(move || {
        if same_file(&output, &path) {
            return Err("choisis un autre fichier : la ROM d'origine ne doit pas être écrasée".into());
        }
        let mut game = GameRom::open(&path).map_err(|e| e.to_string())?;
        let written = romedit::apply(&mut game, &edits).map_err(|e| e.to_string())?;
        game.save(&output).map_err(|e| e.to_string())?;
        Ok(written)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CtrEditSaved {
    /// Nombre d'espèces modifiées.
    count: usize,
    /// Dossier `romfs` du LayeredFS (si demandé).
    romfs: Option<String>,
    /// ROM `.3ds` / `.cxi` reconstruite (si demandée).
    image: Option<String>,
}

/// Jeu 3DS : écrit les espèces modifiées dans un dossier LayeredFS et/ou une ROM
/// reconstruite sous `output` (dossier), sans toucher à la ROM d'origine.
#[tauri::command]
async fn rom_editor_save_ctr(path: PathBuf, edits: Vec<SpeciesData>, output: PathBuf, format: CtrOutput, target: LayeredFsTarget) -> Result<CtrEditSaved, String> {
    blocking(move || {
        let game = CtrGameRom::open(&path).map_err(|e| e.to_string())?;
        let (count, written) = romedit_ctr::save(&game, &edits, &output, format, target).map_err(|e| e.to_string())?;
        let show = |p: Option<PathBuf>| p.map(|p| p.display().to_string());
        Ok(CtrEditSaved { count, romfs: show(written.romfs), image: show(written.image) })
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
        .manage(play::SaveWatch::default())
        .manage(companion::Companion::default())
        .manage(nuzlocke::RomCache::default())
        .manage(bank::OpenBank::default())
        .manage(gifts::GiftFiles::default())
        .manage(battle::LinkedRom::default())
        .register_asynchronous_uri_scheme_protocol("sprite", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(sprites::handle(&app, &request)));
        })
        .setup(|app| {
            library::watch_emulators(app.handle().clone());
            Ok(())
        })
        // Fermer la fenêtre principale quitte Kaleido, compagnon de partie compris.
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                if let Some(c) = window.app_handle().get_webview_window("companion") {
                    let _ = c.close();
                }
            }
        })
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(library::handle_cover(&app, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            detect_file,
            expand_paths,
            open_rom,
            randomizer_presets,
            preview_starters,
            randomize_rom,
            randomize_ctr,
            rom_editor_data,
            rom_editor_save,
            rom_editor_save_ctr,
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
            saves::save_inventory,
            saves::save_set_inventory,
            saves::ability_names,
            saves::save_lists,
            saves::save_suggest_moves,
            saves::save_learnset,
            saves::save_dex,
            saves::save_set_dex,
            saves::save_dex_all,
            saves::peek_save,
            saves::name_lists,
            play::emulators_list,
            play::emulators_save,
            play::emulator_test,
            play::play_plan,
            play::play_rom,
            play::play_backup,
            play::play_temp_save,
            play::play_install_save,
            play::play_find_rom,
            play::watch_save,
            companion::companion_open,
            adventure::adventure_preview,
            randomizer_defaults,
            adventure::user_presets,
            adventure::save_user_presets,
            adventure::nuzlocke_track_save,
            companion::companion_launch,
            companion::companion_state,
            companion::companion_set_on_top,
            companion::companion_set_auto_open,
            companion::companion_set_compact,
            companion::companion_track_nuzlocke,
            companion::companion_mark_missed,
            companion::companion_open_in_main,
            play::unwatch_save,
            play::watch_save_resync,
            library::library_config,
            library::library_set_config,
            library::library_scan,
            library::emulator_download_info,
            library::emulator_install,
            library::music_title_theme,
            nxmusic::music_switch_theme,
            nxmusic::music_switch_tracks,
            nxmusic::music_switch_preview,
            nxmusic::music_switch_choose,
            nxmusic::vgmstream_install,
            video::title_video,
            video::ffmpeg_install,
            video::ffmpeg_download_info,
            library::game_status,
            library::emulators_running,
            library::library_scan_switch,
            discover::discover_pc,
            discover::emulator_locate,
            mods::mods_list,
            mods::mods_install,
            mods::mods_uninstall,
            mods::mods_toggle_other,
            mods::cheats_list,
            mods::cheats_set,
            tuning::tune_plan,
            tuning::tune_apply,
            tuning::tune_restore,
            nuzlocke::nuzlocke_view,
            nuzlocke::nuzlocke_set_state,
            nuzlocke::nuzlocke_link_rom,
            bank::bank_info,
            bank::bank_set_path,
            bank::bank_box,
            bank::bank_box_compat,
            bank::bank_detail,
            bank::bank_search,
            bank::bank_move,
            bank::bank_delete,
            bank::bank_add_box,
            bank::bank_rename_box,
            bank::bank_delete_box,
            bank::bank_import,
            bank::bank_export,
            bank::bank_deposit,
            bank::bank_withdraw,
            showdown::showdown_preview,
            showdown::showdown_import,
            showdown::showdown_export,
            showdown::showdown_apply,
            showdown::showdown_add_set,
            showdown::smogon_sets,
            teams::teams_list,
            teams::teams_get,
            updates::check_update,
            emusaves::emulator_saves,
            gifts::gifts_search,
            gifts::gifts_overview,
            gifts::gifts_details,
            gifts::gifts_add,
            gifts::gifts_export,
            gifts::gifts_import,
            battle::battle_link_rom,
            battle::battle_linked,
            battle::battle_unlink,
            battle::battle_matrix,
            battle::battle_duel,
            legality::legality_check,
            legality::legality_check_all,
            legality::legality_legalize,
            legality::legality_legalize_preview,
            legality::legality_legalize_apply,
            legality::legality_legalize_all,
            legality::legality_generate,
            legality::encounter_species,
            legality::encounter_details
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer Kaleido");
}
