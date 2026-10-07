//! Compagnon de partie : petite fenêtre à côté de l'émulateur qui relit la sauvegarde
//! à chaque sauvegarde en jeu et affiche l'équipe, les boîtes et la progression.
//!
//! Lecture seule : rien n'est jamais écrit dans la sauvegarde depuis le compagnon.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use kaleido_core::save::session::{LiveSnapshot, SaveSession};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::play::FileWatch;

const LABEL: &str = "companion";
/// Relevé plus serré que pour l'éditeur : la sauvegarde doit apparaître en moins de 2 s.
const POLL: Duration = Duration::from_millis(250);
const DEBOUNCE: Duration = Duration::from_millis(500);
/// Fichier illisible (écriture en cours) : nouvel essai après ce délai, quelques fois.
const RETRY: Duration = Duration::from_millis(500);
const RETRIES: usize = 4;

/// Réglages mémorisés du compagnon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanionConfig {
    pub width: f64,
    pub height: f64,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub on_top: bool,
    /// Ouverture automatique au lancement d'un jeu, par jeu de la bibliothèque (absent = oui).
    pub auto_open: HashMap<String, bool>,
}

impl Default for CompanionConfig {
    fn default() -> Self {
        CompanionConfig { width: 460.0, height: 780.0, x: None, y: None, on_top: true, auto_open: HashMap::new() }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("companion.json"))
}

fn load_config(app: &AppHandle) -> CompanionConfig {
    config_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_config(app: &AppHandle, c: &CompanionConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Partie suivie par le compagnon.
struct Session {
    path: PathBuf,
    title: String,
    key: Option<String>,
    _watch: FileWatch,
}

#[derive(Default)]
pub struct Companion(Mutex<Option<Session>>);

/// Ce que reçoit la fenêtre du compagnon.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionState {
    path: String,
    title: String,
    key: Option<String>,
    exists: bool,
    /// Dernière écriture du fichier (ms depuis 1970).
    modified: Option<u64>,
    snapshot: Option<LiveSnapshot>,
    /// Lecture impossible (fichier en cours d'écriture, format inconnu…). L'interface garde
    /// alors l'instantané précédent : jamais d'état à moitié lu.
    error: Option<String>,
    on_top: bool,
    auto_open: bool,
}

fn modified_ms(path: &Path) -> Option<u64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// Lit la sauvegarde ; si elle est illisible (émulateur en pleine écriture), relit un peu plus tard.
fn read_snapshot(path: &Path) -> Result<LiveSnapshot, String> {
    let mut last = String::new();
    for attempt in 0..=RETRIES {
        if attempt > 0 {
            std::thread::sleep(RETRY);
        }
        match fs::read(path).map_err(|e| e.to_string()).and_then(|b| SaveSession::open(&b).and_then(|s| s.live()).map_err(|e| e.to_string())) {
            Ok(s) => return Ok(s),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn state_of(app: &AppHandle, path: &Path, title: &str, key: Option<&String>) -> CompanionState {
    let exists = path.is_file();
    let (snapshot, error) = if exists {
        match read_snapshot(path) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    let config = load_config(app);
    CompanionState {
        path: path.display().to_string(),
        title: title.to_string(),
        key: key.cloned(),
        exists,
        modified: modified_ms(path),
        snapshot,
        error,
        on_top: config.on_top,
        auto_open: key.map(|k| config.auto_open.get(k).copied().unwrap_or(true)).unwrap_or(true),
    }
}

/// Mémorise taille et position quand on ferme le compagnon, et arrête la surveillance.
fn on_window_event(app: &AppHandle, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { .. } = event {
        if let Some(w) = app.get_webview_window(LABEL) {
            let scale = w.scale_factor().unwrap_or(1.0);
            let mut c = load_config(app);
            if let Ok(size) = w.inner_size() {
                let s = size.to_logical::<f64>(scale);
                c.width = s.width;
                // Fermé en mode barre : on garde la hauteur de la vue complète.
                if s.height > 200.0 {
                    c.height = s.height;
                }
            }
            if let Ok(pos) = w.outer_position() {
                let p = pos.to_logical::<f64>(scale);
                c.x = Some(p.x);
                c.y = Some(p.y);
            }
            let _ = save_config(app, &c);
        }
        if let Some(state) = app.try_state::<Companion>() {
            if let Ok(mut s) = state.0.lock() {
                *s = None;
            }
        }
    }
}

fn open_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let c = load_config(app);
    let mut b = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?companion".into()))
        .title("Kaleido · Compagnon")
        .inner_size(c.width, c.height)
        .min_inner_size(300.0, 96.0)
        .always_on_top(c.on_top);
    if let (Some(x), Some(y)) = (c.x, c.y) {
        b = b.position(x, y);
    }
    let w = b.build().map_err(|e| e.to_string())?;
    let handle = app.clone();
    w.on_window_event(move |e| on_window_event(&handle, e));
    Ok(())
}

/// Suit la sauvegarde `path` (qui peut ne pas encore exister : nouvelle partie) et affiche la fenêtre.
fn open(app: &AppHandle, path: PathBuf, title: String, key: Option<String>) -> Result<(), String> {
    {
        let state = app.state::<Companion>();
        let mut slot = state.0.lock().map_err(|e| e.to_string())?;
        let same = slot.as_ref().is_some_and(|s| s.path == path);
        if !same {
            let handle = app.clone();
            let (t, k) = (title.clone(), key.clone());
            let watch = FileWatch::spawn(path.clone(), POLL, DEBOUNCE, move |p| {
                let s = state_of(&handle, p, &t, k.as_ref());
                // Diffusé à toutes les fenêtres : seule celle du compagnon l'écoute.
                let _ = handle.emit("companion-update", s);
            });
            *slot = Some(Session { path, title, key, _watch: watch });
        }
    }
    open_window(app)
}

// Commandes asynchrones : sous Windows, créer une fenêtre depuis une commande synchrone bloque l'app.

/// Ouvre le compagnon sur la sauvegarde `path`.
#[tauri::command]
pub async fn companion_open(path: PathBuf, title: String, key: Option<String>, app: AppHandle) -> Result<(), String> {
    open(&app, path, title, key)
}

/// Ouverture automatique au lancement d'un jeu, si l'utilisateur ne l'a pas désactivée pour ce jeu.
#[tauri::command]
pub async fn companion_launch(path: PathBuf, title: String, key: Option<String>, app: AppHandle) -> Result<bool, String> {
    let enabled = key.as_ref().map(|k| load_config(&app).auto_open.get(k).copied().unwrap_or(true)).unwrap_or(true);
    if enabled {
        open(&app, path, title, key)?;
    }
    Ok(enabled)
}

/// État actuel (lu sur le disque) de la partie suivie.
#[tauri::command]
pub async fn companion_state(app: AppHandle) -> Result<Option<CompanionState>, String> {
    let current = {
        let state = app.state::<Companion>();
        let slot = state.0.lock().map_err(|e| e.to_string())?;
        slot.as_ref().map(|s| (s.path.clone(), s.title.clone(), s.key.clone()))
    };
    let Some((path, title, key)) = current else { return Ok(None) };
    crate::blocking(move || Ok(Some(state_of(&app, &path, &title, key.as_ref())))).await
}

#[tauri::command]
pub async fn companion_set_on_top(on: bool, app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.set_always_on_top(on).map_err(|e| e.to_string())?;
    }
    let mut c = load_config(&app);
    c.on_top = on;
    save_config(&app, &c)
}

#[tauri::command]
pub fn companion_set_auto_open(key: String, on: bool, app: AppHandle) -> Result<(), String> {
    let mut c = load_config(&app);
    c.auto_open.insert(key, on);
    save_config(&app, &c)
}

/// Taille de la fenêtre : barre d'une ligne (`compact`) ou vue complète.
#[tauri::command]
pub async fn companion_set_compact(compact: bool, app: AppHandle) -> Result<(), String> {
    let Some(w) = app.get_webview_window(LABEL) else { return Ok(()) };
    let scale = w.scale_factor().unwrap_or(1.0);
    let size = w.inner_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale);
    let height = if compact { 96.0 } else { load_config(&app).height.max(360.0) };
    w.set_size(tauri::LogicalSize::new(size.width, height)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaleido_core::save::session::Slot;
    use std::sync::mpsc;
    use std::time::Instant;

    /// Sauvegarde de démo modifiée (premier Pokémon au niveau `level`).
    fn demo_with_level(level: u8) -> Vec<u8> {
        let mut s = SaveSession::open(&kaleido_core::save::demo_save().unwrap()).unwrap();
        let patch = serde_json::from_value(serde_json::json!({ "level": level })).unwrap();
        s.patch(Slot::Party { index: 0 }, &patch).unwrap();
        s.to_bytes()
    }

    fn temp_save(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kaleido-companion-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("partie.sav");
        fs::write(&path, bytes).unwrap();
        path
    }

    /// Critère du PRD : moins de 2 s entre l'écriture de la sauvegarde et l'instantané à jour.
    #[test]
    fn sauvegarde_en_jeu_vue_en_moins_de_2_s() {
        let path = temp_save("latence", &demo_with_level(62));
        let (tx, rx) = mpsc::channel();
        let _watch = FileWatch::spawn(path.clone(), POLL, DEBOUNCE, move |p| {
            let _ = tx.send(read_snapshot(p).map(|s| s.party[0].level));
        });
        std::thread::sleep(Duration::from_millis(300));
        let written = Instant::now();
        fs::write(&path, demo_with_level(63)).unwrap();
        let level = rx.recv_timeout(Duration::from_secs(5)).expect("aucune mise à jour").unwrap();
        let elapsed = written.elapsed();
        assert_eq!(level, 63);
        assert!(elapsed < Duration::from_secs(2), "mise à jour en {elapsed:?}");
    }

    /// Critère du PRD : l'émulateur écrit la sauvegarde en deux moitiés à 200 ms d'écart ;
    /// le compagnon ne doit jamais lire l'état intermédiaire.
    #[test]
    fn ecriture_en_deux_fois_jamais_lue_a_moitie() {
        let path = temp_save("moities", &demo_with_level(62));
        let (tx, rx) = mpsc::channel();
        let _watch = FileWatch::spawn(path.clone(), POLL, DEBOUNCE, move |p| {
            let _ = tx.send(read_snapshot(p).map(|s| s.party[0].level));
        });
        std::thread::sleep(Duration::from_millis(300));
        let next = demo_with_level(64);
        let half = next.len() / 2;
        let mut first = fs::read(&path).unwrap();
        first[..half].copy_from_slice(&next[..half]);
        fs::write(&path, &first).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        fs::write(&path, &next).unwrap();
        let level = rx.recv_timeout(Duration::from_secs(5)).expect("aucune mise à jour").unwrap();
        assert_eq!(level, 64, "seul l'état final doit être lu");
        assert!(rx.recv_timeout(Duration::from_millis(1500)).is_err(), "une seule mise à jour attendue");
    }
}
