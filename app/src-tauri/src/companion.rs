//! Compagnon de partie : petite fenêtre à côté de l'émulateur qui relit la sauvegarde
//! à chaque sauvegarde en jeu et affiche l'équipe, les boîtes et la progression.
//!
//! Lecture seule : rien n'est jamais écrit dans la sauvegarde depuis le compagnon.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use std::sync::Arc;

use kaleido_core::battle::trainers::RomTrainers;
use kaleido_core::battle::{self, Combatant, Field, SideState, Verdict};
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
    /// Overlay de stream (source navigateur OBS).
    pub overlay: crate::overlay::OverlayConfig,
    /// Lecture de la mémoire de l'émulateur (compagnon en direct), activée par défaut.
    pub memory: bool,
}

impl Default for CompanionConfig {
    fn default() -> Self {
        CompanionConfig { width: 460.0, height: 780.0, x: None, y: None, on_top: true, auto_open: HashMap::new(), overlay: Default::default(), memory: true }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("companion.json"))
}

pub(crate) fn load_config(app: &AppHandle) -> CompanionConfig {
    config_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub(crate) fn save_config(app: &AppHandle, c: &CompanionConfig) -> Result<(), String> {
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
    /// Lecture de la mémoire de l'émulateur (arrêtée quand la session est lâchée).
    _live: crate::live::LiveLoop,
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
    /// Lieux de capture à rattacher à une route (dans la page Nuzlocke de l'éditeur).
    unassigned: usize,
    /// Toutes les routes : la route affichée suit la carte lue en mémoire, sans relire la sauvegarde.
    #[serde(skip)]
    all: Vec<RouteHere>,
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
    /// Prochain champion (d'après les badges) et meilleurs contres de l'équipe actuelle.
    next_battle: Option<NextBattle>,
    /// Origine de l'instantané : `file` (sauvegarde) ou `memory` (équipe lue dans l'émulateur).
    source: &'static str,
    /// Lecture de la mémoire de l'émulateur : pastille « En direct » / « En jeu » / « Hors ligne ».
    live: Option<crate::live::LiveInfo>,
    /// Combat en cours vu en mémoire.
    battle: Option<crate::live::BattleView>,
    /// Dernière rencontre sauvage vue en mémoire, avec son issue.
    encounter: Option<crate::live::Encounter>,
    /// Ce qui a déclenché cet envoi : `save` (sauvegarde relue) ou `memory` (lecture en direct).
    reason: &'static str,
}

fn modified_ms(path: &Path) -> Option<u64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

pub(crate) fn now_ms() -> u64 {
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
    let all: Vec<RouteHere> = report
        .routes
        .iter()
        .map(|v| RouteHere {
            key: v.key.clone(),
            name: v.name.clone(),
            status: v.status,
            capture: v.capture.as_ref().map(|m| if m.is_nicknamed { m.nickname.clone() } else { m.species_name.clone() }),
            marked_missed: v.marked_missed,
        })
        .collect();
    let here = here.and_then(|r| all.iter().find(|v| v.key == r.key)).cloned();
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
        unassigned: report.unassigned.len(),
        all,
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
        next_battle: None,
        source: "file",
        live: None,
        battle: None,
        encounter: None,
        reason: "save",
    };
    let _guard = INGEST.lock();
    let mut log = runlog::load(app, &t.path);
    let journal = |log: &runlog::RunLog| log.journal.iter().rev().take(100).cloned().collect();
    if !out.exists {
        out.journal = journal(&log);
        apply_live(app, &t.path, &mut out);
        return out;
    }
    let (session, snap) = match read_save(&t.path) {
        Ok(r) => r,
        Err(e) => {
            out.error = Some(e);
            out.journal = journal(&log);
            apply_live(app, &t.path, &mut out);
            return out;
        }
    };

    // ROM : celle liée au Nuzlocke, sinon celle lancée depuis la bibliothèque.
    let mut state = nuzlocke::load_state(&t.path);
    let version = session.save.version();
    // La ROM lancée fait foi : un suivi Nuzlocke peut être resté lié à une autre ROM.
    let rom_path = t.rom.clone().filter(|p| p.exists()).or_else(|| state.rom_path.clone().map(PathBuf::from));
    let info = rom_path.clone().and_then(|p| crate::nuzlocke::rom_info_sync(app, p).ok()).filter(|i| nuzlocke::compatible(i.game, version));
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
        out.can_track = !tracking;
        match nuzlocke::report(info, &session, &state) {
            Ok(r) => {
                if tracking {
                    out.nuzlocke = Some(summary(&r, route));
                }
                let leader = r.caps.iter().find(|c| c.current).map(|c| &c.leader);
                out.next_battle = rom_path.as_deref().zip(leader).and_then(|(rom, l)| next_battle(rom, &session, l));
            }
            Err(e) if tracking => out.error = Some(e.to_string()),
            Err(_) => {}
        }
    }
    out.snapshot = Some(snap);
    apply_live(app, &t.path, &mut out);
    out
}

fn current(app: &AppHandle) -> Option<Target> {
    app.state::<Companion>().0.lock().ok()?.as_ref().map(|s| s.target.clone())
}

/// Dernier état envoyé à la fenêtre : la lecture en mémoire le reprend sans relire la sauvegarde.
static LAST: Mutex<Option<CompanionState>> = Mutex::new(None);

fn remember(s: &CompanionState) {
    if let Ok(mut last) = LAST.lock() {
        *last = Some(CompanionState { events: Vec::new(), reason: "save", ..s.clone() });
    }
}

/// Envoie l'état à la fenêtre du compagnon (et à tout ce qui écoute `companion-update`) :
/// point de passage unique des mises à jour, sauvegarde comme mémoire.
fn emit_state(app: &AppHandle, s: CompanionState) {
    crate::overlay::publish(Some(&s));
    let _ = app.emit("companion-update", s);
}

/// Relit tout et prévient la fenêtre (après une action de l'utilisateur).
fn push(app: &AppHandle) {
    if let Some(t) = current(app) {
        let s = ingest(app, &t);
        remember(&s);
        emit_state(app, s);
    }
}

/// Ajoute la lecture en direct : équipe, carte et badges vus en mémoire, combat, rencontre.
/// L'instantané de la sauvegarde garde boîtes, dresseur et temps de jeu.
fn apply_live(app: &AppHandle, path: &Path, out: &mut CompanionState) {
    merge_live(app, path, out);
    // Taux de chromatiques modifié par le randomizer : le jeu compare à un autre seuil que 8.
    let rom = current(app).and_then(|t| rom_of(&t));
    if let Some(t) = shiny_threshold(rom.as_deref()).filter(|&t| t != 8) {
        if let Some(snap) = out.snapshot.as_mut() {
            snap.set_shiny_threshold(t);
        }
        if let Some(b) = out.battle.as_mut() {
            for f in &mut b.foes {
                f.set_shiny_threshold(t);
            }
        }
        if let Some(e) = out.encounter.as_mut() {
            if let Some(f) = out.battle.as_ref().and_then(|b| b.foes.iter().find(|f| f.pid == e.id)) {
                e.shiny = f.shiny;
            }
        }
    }
}

/// Seuil des chromatiques de la ROM DS de la partie (taux modifié par le randomizer), lu une fois.
pub(crate) fn shiny_threshold(rom: Option<&Path>) -> Option<u32> {
    static CACHE: Mutex<Option<HashMap<PathBuf, Option<u32>>>> = Mutex::new(None);
    let rom = rom.filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds")))?;
    let mut cache = CACHE.lock().ok()?;
    let map = cache.get_or_insert_with(HashMap::new);
    *map.entry(rom.to_path_buf()).or_insert_with(|| {
        // « Toujours chromatique » : toute valeur passe le test.
        kaleido_core::data::shiny::rom_threshold(rom).map(|t| if t >= kaleido_core::data::shiny::ALWAYS { 0x10000 } else { u32::from(t) })
    })
}

/// ROM de la partie suivie (lecture en direct : code de l'overlay de combat).
pub(crate) fn target_rom(app: &AppHandle) -> Option<PathBuf> {
    current(app).and_then(|t| rom_of(&t))
}

/// ROM de la partie : celle lancée depuis la bibliothèque, sinon celle liée au Nuzlocke.
fn rom_of(t: &Target) -> Option<PathBuf> {
    t.rom.clone().filter(|p| p.exists()).or_else(|| nuzlocke::load_state(&t.path).rom_path.map(PathBuf::from))
}

fn merge_live(app: &AppHandle, path: &Path, out: &mut CompanionState) {
    let o = crate::live::overlay(path);
    out.live = o.info;
    out.battle = o.battle;
    out.encounter = o.encounter;
    let (Some(read), Some(snap)) = (o.read, out.snapshot.as_ref()) else { return };
    let merged = snap.with_memory(&read.party, read.map, read.badges);
    if merged.map != snap.map {
        let (place, route) = place_now(app, merged.version, merged.generation, merged.map);
        if let Some(place) = place {
            out.place = Some(place);
        }
        if let Some(n) = out.nuzlocke.as_mut() {
            n.here = route.and_then(|k| n.all.iter().find(|r| r.key == k).cloned());
        }
    }
    out.snapshot = Some(merged);
    out.source = "memory";
}

/// Lieu (et route Nuzlocke) d'une carte de la partie suivie, d'après sa ROM.
pub(crate) fn place_now(app: &AppHandle, version: kaleido_core::save::SaveVersion, generation: u8, map: u16) -> (Option<String>, Option<String>) {
    let Some(t) = current(app) else { return (None, None) };
    let state = nuzlocke::load_state(&t.path);
    // La ROM lancée fait foi : un suivi Nuzlocke peut être resté lié à une autre ROM.
    let rom_path = t.rom.clone().filter(|p| p.exists()).or_else(|| state.rom_path.clone().map(PathBuf::from));
    let info = rom_path.and_then(|p| crate::nuzlocke::rom_info_sync(app, p).ok()).filter(|i| nuzlocke::compatible(i.game, version));
    let location = info.as_ref().and_then(|i| i.location_of_map(map));
    let route = info.as_ref().zip(location).and_then(|(i, l)| i.route_of_location(l));
    let place = route.map(|r| r.name.clone()).or_else(|| location.and_then(|l| place_name(generation, l)));
    (place, route.map(|r| r.key.clone()))
}

/// Lecture de la mémoire activée dans les réglages du compagnon.
pub(crate) fn memory_enabled(app: &AppHandle) -> bool {
    load_config(app).memory
}

/// Nouvelle lecture en mémoire : reprend le dernier état (sans relire la sauvegarde) et prévient
/// la fenêtre, avec les évènements vus en mémoire (rencontre, K.O.…).
pub(crate) fn push_memory(app: &AppHandle, events: Vec<String>) {
    let Some(t) = current(app) else { return };
    let path = t.path.display().to_string();
    let last = LAST.lock().ok().and_then(|l| l.clone()).filter(|s| s.path == path);
    let mut s = match last {
        Some(s) => s,
        None => {
            let s = ingest(app, &t);
            remember(&s);
            s
        }
    };
    apply_live(app, &t.path, &mut s);
    s.events = events;
    s.reason = "memory";
    emit_state(app, s);
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
            crate::overlay::publish::<CompanionState>(None);
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
            let live = crate::live::spawn(app.clone(), target.path.clone());
            *slot = Some(Session { target, _watch: watch, _live: live });
        }
    }
    // Nouvelle partie suivie : la fenêtre déjà ouverte et l'overlay de stream la voient tout de suite.
    let handle = app.clone();
    std::thread::spawn(move || push(&handle));
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
    crate::blocking(move || {
        let s = ingest(&app, &t);
        remember(&s);
        crate::overlay::publish(Some(&s));
        Ok(Some(s))
    })
    .await
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

/// Issue d'une rencontre sauvage vue en mémoire : `caught` (Capturé), `missed` (Raté : K.O. du
/// sauvage), `fled` (Fui). Raté et Fui marquent la route comme ratée si le Nuzlocke est suivi,
/// et l'issue entre au journal.
#[tauri::command]
pub async fn companion_encounter_outcome(id: u32, outcome: String, app: AppHandle) -> Result<(), String> {
    if !matches!(outcome.as_str(), "caught" | "missed" | "fled") {
        return Err(format!("issue inconnue : {outcome}"));
    }
    let t = current(&app).ok_or("aucune partie suivie")?;
    let e = crate::live::set_outcome(id, &outcome).ok_or("rencontre introuvable : elle a pu être remplacée par une autre")?;
    crate::blocking(move || {
        let mut state = nuzlocke::load_state(&t.path);
        let tracking = state.rom_path.is_some();
        if outcome != "caught" && tracking {
            if let Some(route) = &e.route {
                state.missed.insert(route.clone());
                nuzlocke::store_state(&t.path, &state).map_err(|e| format!("enregistrement impossible : {e}"))?;
            }
        }
        let verb = match outcome.as_str() {
            "caught" => "capturé",
            "missed" => "raté",
            _ => "fui",
        };
        let mut log = runlog::load(&app, &t.path);
        let play = log.last.as_ref().map_or(0, |b| b.play_seconds);
        log.note("encounter", format!("Rencontre : {} niveau {}, {verb}", e.species_name, e.level), now_ms(), play, e.place.as_deref(), Some(e.species));
        runlog::store(&app, &t.path, &log)?;
        push(&app);
        Ok(())
    })
    .await
}

/// Lecture de la mémoire de l'émulateur activée ?
#[tauri::command]
pub fn companion_memory(app: AppHandle) -> bool {
    memory_enabled(&app)
}

/// Active ou coupe la lecture de la mémoire de l'émulateur.
#[tauri::command]
pub fn companion_set_memory(on: bool, app: AppHandle) -> Result<(), String> {
    let mut c = load_config(&app);
    c.memory = on;
    save_config(&app, &c)
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

// ---------------------------------------------------------------------------
// Prochain combat : équipe du prochain champion et meilleur contre de l'équipe actuelle.

/// Données de combat de la ROM de la partie (lues une fois).
static TRAINERS: Mutex<Option<(PathBuf, Arc<RomTrainers>)>> = Mutex::new(None);

fn trainers_of(path: &Path) -> Option<Arc<RomTrainers>> {
    let mut slot = TRAINERS.lock().ok()?;
    if let Some((p, t)) = slot.as_ref() {
        if p == path {
            return Some(t.clone());
        }
    }
    let t = Arc::new(RomTrainers::open(path).ok()?);
    *slot = Some((path.to_path_buf(), t.clone()));
    Some(t)
}

/// Mon meilleur Pokémon contre un adversaire.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counter {
    name: String,
    species: u16,
    verdict: Verdict,
    move_name: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opponent {
    species: u16,
    form: u8,
    name: String,
    level: u8,
    counter: Option<Counter>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextBattle {
    label: String,
    name: String,
    town: String,
    trainer_id: u16,
    team: Vec<Opponent>,
}

fn rank(v: Verdict) -> u8 {
    match v {
        Verdict::Win => 0,
        Verdict::Uncertain => 1,
        Verdict::Lose => 2,
        Verdict::None => 3,
    }
}

/// Équipe du prochain champion et, pour chacun de ses Pokémon, ton meilleur contre
/// (calcul de dégâts sans modificateurs, comme la page Combat par défaut).
fn next_battle(rom: &Path, session: &SaveSession, leader: &nuzlocke::Leader) -> Option<NextBattle> {
    let trainers = trainers_of(rom)?;
    let id = *leader.trainer_ids.first()?;
    let theirs = trainers.team(id)?;
    let game = session.game();
    let party = session.view().ok()?.party;
    let mine: Vec<Combatant> = party.iter().filter_map(|v| battle::party::from_slot(game, v, Some(&trainers))).collect();
    let (side, field) = (SideState::default(), Field::default());
    let cells = battle::matrix(game, &mine, &side, &theirs, &side, &field);
    let team = theirs
        .iter()
        .enumerate()
        .map(|(j, them)| {
            let counter = (0..mine.len())
                .min_by_key(|&i| {
                    let c = &cells[i][j];
                    (rank(c.verdict), -(c.mine.as_ref().map_or(0.0, |m| m.min_percent) * 10.0) as i64)
                })
                .map(|i| Counter {
                    name: mine[i].name.clone(),
                    species: mine[i].species,
                    verdict: cells[i][j].verdict,
                    move_name: cells[i][j].mine.as_ref().map(|m| m.name.clone()),
                });
            Opponent { species: them.species, form: them.form, name: them.name.clone(), level: them.level, counter }
        })
        .collect();
    Some(NextBattle { label: leader.label.clone(), name: leader.name.to_string(), town: leader.town.to_string(), trainer_id: id, team })
}

/// Ouvre la sauvegarde dans la fenêtre principale (éditeur ou page Combat sur ce dresseur).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenInMain {
    path: String,
    rom: Option<String>,
    /// Dresseur à afficher dans la page Combat ; `None` = éditeur.
    trainer: Option<u16>,
    /// Page de l'éditeur à ouvrir (« nuzlocke »…).
    page: Option<String>,
}

#[tauri::command]
pub async fn companion_open_in_main(trainer: Option<u16>, page: Option<String>, app: AppHandle) -> Result<(), String> {
    let t = current(&app).ok_or("aucune partie suivie")?;
    let state = nuzlocke::load_state(&t.path);
    let rom = state.rom_path.clone().or_else(|| t.rom.as_ref().map(|r| r.display().to_string()));
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    app.emit("companion-open-in-main", OpenInMain { path: t.path.display().to_string(), rom, trainer, page }).map_err(|e| e.to_string())
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
