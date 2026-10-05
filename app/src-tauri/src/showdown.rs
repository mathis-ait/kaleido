//! Commandes Showdown (import / export de texte) et sets compétitifs Smogon.
//!
//! Les sets viennent de `https://data.pkmn.cc/sets/gen{N}.json` (projet pkmn/smogon,
//! MIT, d'après les analyses de Smogon University). Ils sont gardés en cache dans le
//! dossier de données de l'application (`smogon/gen{N}.json`) et rafraîchis au plus
//! une fois par semaine ; hors ligne, la copie en cache sert, même ancienne.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use kaleido_core::dex::Lang;
use kaleido_core::save::session::{Slot, SlotView};
use kaleido_core::save::showdown_apply::{ImportReport, ImportTarget, ShowdownPreview};
use kaleido_core::showdown::smogon::{SmogonData, SmogonSet};
use kaleido_core::showdown::ShowdownSet;
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::saves::OpenSave;

const BASE_URL: &str = "https://data.pkmn.cc/sets";
/// Âge au-delà duquel on retélécharge les sets (s'il y a du réseau).
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
const MAX_DOWNLOAD: u64 = 32 * 1024 * 1024;

#[tauri::command]
pub fn showdown_preview(text: String, state: State<'_, OpenSave>) -> Result<ShowdownPreview, String> {
    state.with(|s| Ok(s.preview_showdown(&text)))
}

#[tauri::command]
pub fn showdown_import(text: String, target: ImportTarget, state: State<'_, OpenSave>) -> Result<ImportReport, String> {
    state.with(|s| s.import_showdown(&text, target))
}

#[tauri::command]
pub fn showdown_export(slots: Vec<Slot>, lang: Lang, state: State<'_, OpenSave>) -> Result<String, String> {
    state.with(|s| s.export_showdown(&slots, lang))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedSet {
    pub view: SlotView,
    pub warnings: Vec<String>,
}

/// Applique un set (Smogon ou Showdown, noms anglais) à un Pokémon existant.
#[tauri::command]
pub fn showdown_apply(slot: Slot, set: ShowdownSet, state: State<'_, OpenSave>) -> Result<AppliedSet, String> {
    state.with(|s| s.apply_showdown_set(slot, &set, Lang::En)).map(|(view, warnings)| AppliedSet { view, warnings })
}

/// Ajoute un set (noms anglais) comme nouveau Pokémon.
#[tauri::command]
pub fn showdown_add_set(set: ShowdownSet, target: ImportTarget, state: State<'_, OpenSave>) -> Result<ImportReport, String> {
    state.with(|s| s.import_sets(std::slice::from_ref(&set), Lang::En, target))
}

// --- Sets Smogon ----------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmogonSets {
    pub generation: u8,
    pub sets: Vec<SmogonSet>,
    /// Date de la copie utilisée (secondes depuis 1970).
    pub fetched_at: Option<u64>,
    /// Copie en cache trop ancienne : le téléchargement a échoué (hors ligne).
    pub stale: bool,
}

struct Loaded {
    data: Arc<SmogonData>,
    fetched_at: Option<u64>,
    stale: bool,
}

fn memory() -> &'static Mutex<HashMap<u8, Arc<Loaded>>> {
    static CACHE: OnceLock<Mutex<HashMap<u8, Arc<Loaded>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn cache_file<R: Runtime>(app: &AppHandle<R>, generation: u8) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("smogon");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("gen{generation}.json")))
}

fn download(generation: u8) -> Result<Vec<u8>, String> {
    let response = ureq::get(&format!("{BASE_URL}/gen{generation}.json")).timeout(Duration::from_secs(20)).call().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    response.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

fn modified_secs(path: &PathBuf) -> Option<(u64, bool)> {
    let modified = fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let age = SystemTime::now().duration_since(modified).unwrap_or_default();
    let secs = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    Some((secs, age > MAX_AGE))
}

/// Charge les sets de la génération : mémoire, puis disque, puis réseau.
fn load(file: PathBuf, generation: u8, refresh: bool) -> Result<Arc<Loaded>, String> {
    if !refresh {
        if let Some(l) = memory().lock().map_err(|e| e.to_string())?.get(&generation) {
            return Ok(l.clone());
        }
    }
    let cached = modified_secs(&file);
    let fresh_enough = matches!(cached, Some((_, false))) && !refresh;
    let (bytes, fetched_at, stale) = if fresh_enough {
        (fs::read(&file).map_err(|e| e.to_string())?, cached.map(|c| c.0), false)
    } else {
        match download(generation).and_then(|b| SmogonData::parse(&b).map(|_| b)) {
            Ok(bytes) => {
                // Écriture via un fichier temporaire : pas de cache à moitié écrit.
                let tmp = file.with_extension("json.part");
                if fs::write(&tmp, &bytes).and_then(|_| fs::rename(&tmp, &file)).is_err() {
                    let _ = fs::remove_file(&tmp);
                }
                let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                (bytes, Some(now), false)
            }
            Err(e) => match fs::read(&file) {
                Ok(bytes) => (bytes, cached.map(|c| c.0), true),
                Err(_) => return Err(format!("Sets Smogon indisponibles hors ligne (aucune copie en cache) : {e}")),
            },
        }
    };
    let data = Arc::new(SmogonData::parse(&bytes)?);
    let loaded = Arc::new(Loaded { data, fetched_at, stale });
    memory().lock().map_err(|e| e.to_string())?.insert(generation, loaded.clone());
    Ok(loaded)
}

/// Sets Smogon de l'espèce pour la génération de la sauvegarde ouverte.
/// `refresh` force un nouveau téléchargement.
#[tauri::command]
pub async fn smogon_sets(app: AppHandle, species: u16, form: u8, refresh: Option<bool>, state: State<'_, OpenSave>) -> Result<SmogonSets, String> {
    let game = state.with(|s| Ok(s.game()))?;
    let generation = game.generation();
    let file = cache_file(&app, generation)?;
    let refresh = refresh.unwrap_or(false);
    let loaded = tauri::async_runtime::spawn_blocking(move || load(file, generation, refresh)).await.map_err(|e| e.to_string())??;
    Ok(SmogonSets { generation, sets: loaded.data.sets_for(game, species, form), fetched_at: loaded.fetched_at, stale: loaded.stale })
}
