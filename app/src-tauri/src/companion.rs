//! Compagnon de partie : petite fenêtre à côté de l'émulateur qui relit la sauvegarde
//! à chaque sauvegarde en jeu et affiche l'équipe, les boîtes et la progression.
//!
//! Lecture seule : rien n'est jamais écrit dans la sauvegarde depuis le compagnon.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use kaleido_core::dex;
use kaleido_core::nuzlocke::{self, RouteStatus, Severity};
use kaleido_core::save::diff::{diff, Brief, DeathKind, GameEvent};
use kaleido_core::save::session::{LiveSnapshot, SaveSession};
use kaleido_core::save::SaveError;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::play::FileWatch;
use crate::runlog;

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

/// Partie suivie : sauvegarde, nom du jeu, clé de la bibliothèque, ROM lancée.
#[derive(Clone)]
struct Target {
    path: PathBuf,
    title: String,
    key: Option<String>,
    /// ROM lancée depuis la bibliothèque : sert à nommer le lieu et à proposer le suivi Nuzlocke.
    rom: Option<PathBuf>,
}

struct Session {
    target: Target,
    _watch: FileWatch,
}

#[derive(Default)]
pub struct Companion(Mutex<Option<Session>>);

/// Une lecture à la fois : la surveillance et l'interface peuvent demander l'état en même temps,
/// et chaque lecture compare à la précédente puis l'enregistre.
static INGEST: Mutex<()> = Mutex::new(());

/// Prochain champion (ou Conseil 4) et son Pokémon le plus fort.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextLeader {
    label: String,
    name: String,
    town: String,
    ace_species: u16,
    ace_level: u8,
}

/// Route Nuzlocke du lieu de la sauvegarde.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteHere {
    key: String,
    name: String,
    status: RouteStatus,
    capture: Option<String>,
    marked_missed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    error: bool,
    title: String,
    detail: String,
}

/// Résumé du suivi Nuzlocke pour le compagnon.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuzlockeSummary {
    captures: usize,
    routes: usize,
    routes_caught: usize,
    alive: usize,
    dead: usize,
    level_cap: Option<u8>,
    next: Option<NextLeader>,
    here: Option<RouteHere>,
    warnings: Vec<Warning>,
}

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
    /// Lieu de la dernière sauvegarde (avec la ROM).
    place: Option<String>,
    /// Nouveautés depuis la lecture précédente, à annoncer.
    events: Vec<String>,
    /// Journal de partie, du plus récent au plus ancien (100 dernières entrées).
    journal: Vec<runlog::JournalEntry>,
    nuzlocke: Option<NuzlockeSummary>,
    /// ROM connue et compatible, mais suivi Nuzlocke pas encore activé.
    can_track: bool,
}

fn modified_ms(path: &Path) -> Option<u64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Message clair pour les sauvegardes qu'on ne peut pas lire.
fn friendly(e: SaveError) -> String {
    match e {
        SaveError::MissingBeef => "aucune partie enregistrée dans ce fichier pour l'instant".into(),
        e => e.to_string(),
    }
}

/// Lit la sauvegarde ; si elle est illisible (émulateur en pleine écriture), relit un peu plus tard.
fn read_save(path: &Path) -> Result<(SaveSession, LiveSnapshot), String> {
    let mut last = String::new();
    for attempt in 0..=RETRIES {
        if attempt > 0 {
            std::thread::sleep(RETRY);
        }
        let read = fs::read(path).map_err(|e| e.to_string()).and_then(|b| {
            let session = SaveSession::open(&b).map_err(friendly)?;
            let live = session.live().map_err(friendly)?;
            Ok((session, live))
        });
        match read {
            Ok(r) => return Ok(r),
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
fn read_snapshot(path: &Path) -> Result<LiveSnapshot, String> {
    read_save(path).map(|(_, s)| s)
}

/// Nom d'un lieu sans la précision entre parenthèses de la liste de PKHeX.
fn place_name(generation: u8, location: u16) -> Option<String> {
    let n = dex::location_name(generation, location)?;
    Some(match n.rsplit_once(" (") {
        Some((base, _)) if n.ends_with(')') => base.to_string(),
        _ => n.to_string(),
    })
}

fn summary(report: &nuzlocke::Report, here: Option<&nuzlocke::Route>) -> NuzlockeSummary {
    let next = report.caps.iter().find(|c| c.current).map(|c| NextLeader {
        label: c.leader.label.clone(),
        name: c.leader.name.to_string(),
        town: c.leader.town.to_string(),
        ace_species: c.leader.ace_species,
        ace_level: c.leader.ace_level,
    });
    let here = here.and_then(|r| report.routes.iter().find(|v| v.key == r.key)).map(|v| RouteHere {
        key: v.key.clone(),
        name: v.name.clone(),
        status: v.status,
        capture: v.capture.as_ref().map(|m| if m.is_nicknamed { m.nickname.clone() } else { m.species_name.clone() }),
        marked_missed: v.marked_missed,
    });
    let warnings = report
        .violations
        .iter()
        .filter(|v| v.severity != Severity::Info)
        .take(6)
        .map(|v| Warning { error: v.severity == Severity::Error, title: v.title.clone(), detail: v.detail.clone() })
        .collect();
    let s = &report.stats;
    NuzlockeSummary {
        captures: s.captures,
        routes: s.routes,
        routes_caught: s.routes_caught,
        alive: s.alive,
        dead: s.dead,
        level_cap: s.level_cap,
        next,
        here,
        warnings,
    }
}

/// Lit la sauvegarde, la compare à la précédente, tient le journal et le suivi Nuzlocke à jour.
fn ingest(app: &AppHandle, t: &Target) -> CompanionState {
    let config = load_config(app);
    let mut out = CompanionState {
        path: t.path.display().to_string(),
        title: t.title.clone(),
        key: t.key.clone(),
        exists: t.path.is_file(),
        modified: modified_ms(&t.path),
        snapshot: None,
        error: None,
        on_top: config.on_top,
        auto_open: t.key.as_ref().map(|k| config.auto_open.get(k).copied().unwrap_or(true)).unwrap_or(true),
        place: None,
        events: Vec::new(),
        journal: Vec::new(),
        nuzlocke: None,
        can_track: false,
    };
    let _guard = INGEST.lock();
    let mut log = runlog::load(app, &t.path);
    let journal = |log: &runlog::RunLog| log.journal.iter().rev().take(100).cloned().collect();
    if !out.exists {
        out.journal = journal(&log);
        return out;
    }
    let (session, snap) = match read_save(&t.path) {
        Ok(r) => r,
        Err(e) => {
            out.error = Some(e);
            out.journal = journal(&log);
            return out;
        }
    };

    // ROM : celle liée au Nuzlocke, sinon celle lancée depuis la bibliothèque.
    let mut state = nuzlocke::load_state(&t.path);
    let version = session.save.version();
    let rom_path = state.rom_path.clone().map(PathBuf::from).or_else(|| t.rom.clone());
    let info = rom_path.and_then(|p| crate::nuzlocke::rom_info_sync(app, p).ok()).filter(|i| nuzlocke::compatible(i.game, version));
    let location = info.as_ref().and_then(|i| i.location_of_map(snap.map));
    let route = info.as_ref().zip(location).and_then(|(i, l)| i.route_of_location(l));
    out.place = route.map(|r| r.name.clone()).or_else(|| location.and_then(|l| place_name(snap.generation, l)));

    // Ce qui s'est passé depuis la dernière lecture.
    let brief = Brief::from(&snap);
    let events = log.last.as_ref().map(|last| diff(last, &brief)).unwrap_or_default();
    let tracking = state.rom_path.is_some();
    let mut deaths = false;
    for e in &events {
        if let (true, GameEvent::Died { mon, how }) = (tracking, e) {
            let cause = match how {
                DeathKind::Deposited => "K.O. puis déposé en boîte",
                DeathKind::Released => "K.O. puis relâché",
            };
            deaths |= state.auto_dead.insert(mon.key.clone(), cause.into()).is_none();
        }
    }
    if deaths {
        let _ = nuzlocke::store_state(&t.path, &state);
    }
    if log.last.as_ref() != Some(&brief) {
        log.record(&events, now_ms(), brief.play_seconds, out.place.as_deref());
        log.last = Some(brief);
        let _ = runlog::store(app, &t.path, &log);
    }
    out.events = events.iter().map(GameEvent::text).collect();
    out.journal = journal(&log);

    if let Some(info) = &info {
        if tracking {
            match nuzlocke::report(info, &session, &state) {
                Ok(r) => out.nuzlocke = Some(summary(&r, route)),
                Err(e) => out.error = Some(e.to_string()),
            }
        } else {
            out.can_track = true;
        }
    }
    out.snapshot = Some(snap);
    out
}

fn current(app: &AppHandle) -> Option<Target> {
    app.state::<Companion>().0.lock().ok()?.as_ref().map(|s| s.target.clone())
}

/// Relit tout et prévient la fenêtre (après une action de l'utilisateur).
fn push(app: &AppHandle) {
    if let Some(t) = current(app) {
        let s = ingest(app, &t);
        let _ = app.emit("companion-update", s);
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
fn open(app: &AppHandle, target: Target) -> Result<(), String> {
    {
        let state = app.state::<Companion>();
        let mut slot = state.0.lock().map_err(|e| e.to_string())?;
        let same = slot.as_ref().is_some_and(|s| s.target.path == target.path);
        if same {
            // Même sauvegarde : on garde la surveillance, en complétant ce qu'on apprend (ROM…).
            if let Some(s) = slot.as_mut() {
                s.target.rom = target.rom.or(s.target.rom.take());
                s.target.key = target.key.or(s.target.key.take());
                if !target.title.is_empty() {
                    s.target.title = target.title;
                }
            }
        } else {
            let handle = app.clone();
            let watch = FileWatch::spawn(target.path.clone(), POLL, DEBOUNCE, move |_| {
                // Diffusé à toutes les fenêtres : seule celle du compagnon l'écoute.
                push(&handle);
            });
            *slot = Some(Session { target, _watch: watch });
        }
    }
    open_window(app)
}

// Commandes asynchrones : sous Windows, créer une fenêtre depuis une commande synchrone bloque l'app.

/// Ouvre le compagnon sur la sauvegarde `path`.
#[tauri::command]
pub async fn companion_open(path: PathBuf, title: String, key: Option<String>, rom: Option<PathBuf>, app: AppHandle) -> Result<(), String> {
    open(&app, Target { path, title, key, rom })
}

/// Ouverture automatique au lancement d'un jeu, si l'utilisateur ne l'a pas désactivée pour ce jeu.
#[tauri::command]
pub async fn companion_launch(path: PathBuf, title: String, key: Option<String>, rom: Option<PathBuf>, app: AppHandle) -> Result<bool, String> {
    let enabled = key.as_ref().map(|k| load_config(&app).auto_open.get(k).copied().unwrap_or(true)).unwrap_or(true);
    if enabled {
        open(&app, Target { path, title, key, rom })?;
    }
    Ok(enabled)
}

/// État actuel (lu sur le disque) de la partie suivie.
#[tauri::command]
pub async fn companion_state(app: AppHandle) -> Result<Option<CompanionState>, String> {
    let Some(t) = current(&app) else { return Ok(None) };
    crate::blocking(move || Ok(Some(ingest(&app, &t)))).await
}

/// Active le suivi Nuzlocke de la partie avec la ROM lancée.
#[tauri::command]
pub async fn companion_track_nuzlocke(app: AppHandle) -> Result<(), String> {
    let t = current(&app).ok_or("aucune partie suivie")?;
    let rom = t.rom.clone().ok_or("ROM de la partie inconnue : lance le jeu depuis la bibliothèque")?;
    crate::blocking(move || {
        let mut state = nuzlocke::load_state(&t.path);
        state.rom_path = Some(rom.display().to_string());
        nuzlocke::store_state(&t.path, &state).map_err(|e| format!("enregistrement impossible : {e}"))?;
        push(&app);
        Ok(())
    })
    .await
}

/// « Rencontre ratée ici » (fuite, K.O. du sauvage) : la sauvegarde n'en garde aucune trace.
#[tauri::command]
pub async fn companion_mark_missed(route: String, missed: bool, app: AppHandle) -> Result<(), String> {
    let t = current(&app).ok_or("aucune partie suivie")?;
    crate::blocking(move || {
        let mut state = nuzlocke::load_state(&t.path);
        if missed {
            state.missed.insert(route);
        } else {
            state.missed.remove(&route);
        }
        nuzlocke::store_state(&t.path, &state).map_err(|e| format!("enregistrement impossible : {e}"))?;
        push(&app);
        Ok(())
    })
    .await
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
