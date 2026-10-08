//! Compagnon en direct : lit la mémoire de l'émulateur toutes les 200 ms, en lecture seule.
//!
//! La sauvegarde reste la vérité durable (journal, morts Nuzlocke) ; la mémoire ne fait
//! qu'anticiper : PV et statuts en combat, rencontres, K.O. instantanés, carte actuelle.
//! Un jeu sans carte mémoire, un émulateur inconnu ou une lecture refusée laissent le
//! compagnon sur la sauvegarde, sans erreur.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use kaleido_core::live::{maps, reader::LiveReader};
use kaleido_core::save::diff::{diff_memory, Brief, GameEvent};
use kaleido_core::save::session::{game_of, LiveFoe, LiveSnapshot, SaveSession};
use kaleido_core::save::{Pokemon, RamHints, SaveVersion};
use serde::Serialize;
use tauri::AppHandle;

use crate::play::EmulatorId;

/// Période de la boucle de lecture.
const TICK: Duration = Duration::from_millis(200);
/// Recherche d'un émulateur (liste des processus) quand on n'est pas attaché.
const PROBE: Duration = Duration::from_secs(2);
/// Combat gardé à l'écran si l'équipe adverse n'est plus relue pendant ce délai.
const BATTLE_HOLD: Duration = Duration::from_millis(1500);
/// Sans lecture valide pendant ce délai, la pastille repasse de « En direct » à « En jeu ».
const STALE: Duration = Duration::from_secs(2);

/// État de la lecture en direct, affiché par la pastille du compagnon.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveInfo {
    /// `live` (mémoire lue), `ingame` (émulateur lancé, mémoire non lue), `offline` (aucun émulateur).
    pub status: &'static str,
    /// Lecture de la mémoire activée dans les réglages.
    pub enabled: bool,
    pub emulator: Option<String>,
    /// Code jeu (DS) ou nom du jeu (3DS) reconnu en mémoire.
    pub game: Option<String>,
    /// Lecture vérifiée en jeu pour ce code (`memory_maps.json`).
    pub verified: bool,
    /// Pourquoi la mémoire n'est pas lue (jeu sans carte, partie différente…).
    pub detail: Option<String>,
}

impl LiveInfo {
    fn new(status: &'static str, enabled: bool) -> Self {
        LiveInfo { status, enabled, emulator: None, game: None, verified: false, detail: None }
    }
}

/// Combat vu en mémoire.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleView {
    pub wild: bool,
    pub foes: Vec<LiveFoe>,
}

/// Rencontre sauvage, de l'entrée en combat jusqu'à l'issue (capture auto, ou choix du joueur).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    /// PID du Pokémon sauvage.
    pub id: u32,
    pub species: u16,
    pub form: u8,
    pub species_name: String,
    pub level: u8,
    pub shiny: bool,
    pub place: Option<String>,
    /// Route Nuzlocke du lieu (clé), si la ROM est connue.
    pub route: Option<String>,
    /// `caught`, `missed`, `fled` ; `None` tant que rien n'est décidé.
    pub outcome: Option<String>,
    pub at: u64,
}

/// Dernière lecture valide de la mémoire.
#[derive(Clone)]
pub struct MemoryRead {
    pub party: Vec<Pokemon>,
    pub map: Option<u16>,
    pub badges: Option<u8>,
}

#[derive(Default)]
struct Shared {
    /// Sauvegarde suivie par la boucle en cours.
    save: Option<PathBuf>,
    info: Option<LiveInfo>,
    read: Option<MemoryRead>,
    battle: Option<BattleView>,
    encounter: Option<Encounter>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared { save: None, info: None, read: None, battle: None, encounter: None });

/// Ce que le compagnon ajoute à son état quand la mémoire est lue.
pub struct Overlay {
    pub info: Option<LiveInfo>,
    pub read: Option<MemoryRead>,
    pub battle: Option<BattleView>,
    pub encounter: Option<Encounter>,
}

/// Dernier état connu de la lecture en direct pour la sauvegarde `save`.
pub fn overlay(save: &Path) -> Overlay {
    let s = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    let same = s.save.as_deref() == Some(save);
    Overlay {
        info: if same { s.info.clone() } else { None },
        read: if same && s.info.as_ref().is_some_and(|i| i.status == "live") { s.read.clone() } else { None },
        battle: if same { s.battle.clone() } else { None },
        encounter: if same { s.encounter.clone() } else { None },
    }
}

/// Issue d'une rencontre choisie par le joueur. Renvoie la rencontre mise à jour.
pub fn set_outcome(id: u32, outcome: &str) -> Option<Encounter> {
    let mut s = SHARED.lock().ok()?;
    let e = s.encounter.as_mut().filter(|e| e.id == id)?;
    e.outcome = Some(outcome.to_string());
    Some(e.clone())
}

fn update(f: impl FnOnce(&mut Shared)) {
    let mut s = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut s);
}

/// Boucle de lecture d'une partie : arrêtée quand la poignée est lâchée (compagnon fermé ou
/// autre sauvegarde suivie).
pub struct LiveLoop {
    stop: Arc<AtomicBool>,
}

impl Drop for LiveLoop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn spawn(app: AppHandle, save: PathBuf) -> LiveLoop {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    update(|s| *s = Shared { save: Some(save.clone()), ..Shared::default() });
    let _ = std::thread::Builder::new().name("kaleido-live".into()).spawn(move || run(app, save, flag));
    LiveLoop { stop }
}

/// Sauvegarde lue sur le disque : instantané de base et repères pour la mémoire.
struct Base {
    modified: Option<SystemTime>,
    snapshot: LiveSnapshot,
    hints: RamHints,
    version: SaveVersion,
}

fn read_base(path: &Path) -> Option<Base> {
    let modified = std::fs::metadata(path).ok()?.modified().ok();
    let session = SaveSession::open(&std::fs::read(path).ok()?).ok()?;
    let snapshot = session.live().ok()?;
    Some(Base { modified, hints: session.save.ram_hints(), version: session.save.version(), snapshot })
}

#[cfg(windows)]
struct Attached {
    proc: kaleido_core::live::windows::Process,
    reader: LiveReader,
    info: LiveInfo,
}

#[cfg(windows)]
fn emulators(ctr: bool) -> Vec<(u32, &'static str)> {
    kaleido_core::live::windows::processes()
        .into_iter()
        .filter_map(|p| {
            let lower = p.exe.to_lowercase();
            EmulatorId::ALL.into_iter().find(|id| !id.is_switch() && id.is_ctr() == ctr && id.matches_exe(&lower)).map(|id| (p.pid, id.name()))
        })
        .collect()
}

/// Nom court du jeu de la sauvegarde, tel que dans `memory_maps.json` (`HGSS`, `XY`…).
fn game_name(v: SaveVersion) -> String {
    format!("{:?}", game_of(v))
}

/// Code jeu, adresse DS et premiers octets de l'overlay de combat.
type BattleCode = (String, u32, Vec<u8>);

/// Début de l'overlay de combat d'une ROM, lu une fois par ROM (la ROM fait plusieurs centaines de Mo).
fn battle_code(rom: &Path, overlay: u32) -> Option<BattleCode> {
    static CACHE: Mutex<Vec<(PathBuf, u32, Option<BattleCode>)>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().ok()?;
    if let Some((_, _, v)) = cache.iter().find(|(p, o, _)| p == rom && *o == overlay) {
        return v.clone();
    }
    let v = maps::battle_code(rom, overlay);
    cache.push((rom.to_path_buf(), overlay, v.clone()));
    v
}

/// En-tête du module de combat d'une ROM 3DS, lu une fois par ROM.
fn cro_head(rom: &Path, module: &str) -> Option<Vec<u8>> {
    type Entry = (PathBuf, String, Option<Vec<u8>>);
    static CACHE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().ok()?;
    if let Some((_, _, v)) = cache.iter().find(|(p, m, _)| p == rom && m == module) {
        return v.clone();
    }
    let v = maps::cro_head(rom, module);
    cache.push((rom.to_path_buf(), module.to_string(), v.clone()));
    v
}

/// Cherche l'émulateur qui fait tourner la partie suivie et s'y attache.
#[cfg(windows)]
fn attach(base: &Base, info: &mut LiveInfo, rom: Option<&Path>) -> Option<Attached> {
    use kaleido_core::live::reader::Console;
    use kaleido_core::live::{scan, windows::Process};
    let ctr = base.version.generation() >= 6;
    let found = emulators(ctr);
    info.status = if found.is_empty() { "offline" } else { "ingame" };
    info.emulator = found.first().map(|(_, n)| n.to_string());
    info.detail = None;
    let wanted = game_name(base.version);
    for (pid, name) in found {
        let proc = match Process::open(pid) {
            Ok(p) => p,
            Err(e) => {
                info.detail = Some(format!("{name} : {e}"));
                continue;
            }
        };
        let (console, map, id) = if ctr {
            let Some(map) = maps::for_game(&wanted) else {
                info.detail = Some("Jeu sans carte mémoire : le compagnon suit la sauvegarde.".into());
                continue;
            };
            (Console::Ctr, map, wanted.clone())
        } else {
            let Some(ram) = scan::find_ds_ram(&proc) else {
                info.detail = Some(format!("{name} : jeu Pokémon introuvable en mémoire (pas encore lancé ?)."));
                continue;
            };
            let Some(map) = maps::for_ds_code(&ram.game_code) else {
                info.detail = Some(format!("Jeu {} sans carte mémoire : le compagnon suit la sauvegarde.", ram.game_code));
                continue;
            };
            if map.game != wanted {
                info.detail = Some(format!("{name} fait tourner un autre jeu ({}) que la partie suivie.", ram.title));
                continue;
            }
            let code = ram.game_code.clone();
            (Console::Ds(ram), map, code)
        };
        let mut reader = LiveReader::new(console, base.hints.clone());
        reader.set_battle(map.battle_scan());
        if let Some((addr, value)) = map.battle_flag(&id) {
            reader.set_battle_flag(addr, value);
        }
        // 3DS : en-tête du module de combat (CRO) lu dans la ROM de la partie.
        if let (Some(module), Some(rom), true) = (map.battle_module.as_deref(), rom, ctr) {
            if let Some(head) = cro_head(rom, module) {
                reader.set_battle_module(head);
                if let Some(field) = map.field_module.as_deref().and_then(|f| cro_head(rom, f)) {
                    reader.set_field_module(field);
                }
            }
        }
        // Overlay de combat lu dans la ROM de la partie, si c'est bien le jeu qui tourne.
        if let (Some(ovl), Some(rom)) = (map.battle_overlay, rom) {
            if let Some((code, addr, bytes)) = battle_code(rom, ovl) {
                if code.eq_ignore_ascii_case(&id) {
                    reader.set_battle_code(addr, bytes);
                }
            }
        }
        let _ = reader.tick(&proc);
        reader.prime_battle(&proc);
        let info = LiveInfo {
            status: "ingame",
            enabled: true,
            emulator: Some(name.to_string()),
            verified: map.party_verified(&id),
            game: Some(id),
            detail: None,
        };
        return Some(Attached { proc, reader, info });
    }
    None
}

/// Empreinte de ce qui se voit à l'écran : on ne prévient la fenêtre que si elle change.
fn fingerprint(snap: &LiveSnapshot, battle: &Option<BattleView>, encounter: &Option<Encounter>) -> String {
    let mut s = format!("{}|", snap.map);
    for m in &snap.party {
        s += &format!("{}:{}:{}:{}:{:?}:{};", m.uid, m.level, m.hp, m.max_hp, m.status, m.move_names.join(","));
    }
    if let Some(b) = battle {
        for f in &b.foes {
            s += &format!("f{}:{}:{};", f.pid, f.hp, f.level);
        }
    }
    if let Some(e) = encounter {
        s += &format!("e{}:{:?}", e.id, e.outcome);
    }
    s
}

#[cfg(not(windows))]
fn run(_app: AppHandle, _save: PathBuf, _stop: Arc<AtomicBool>) {}

#[cfg(windows)]
fn run(app: AppHandle, save: PathBuf, stop: Arc<AtomicBool>) {
    let mut base: Option<Base> = None;
    let mut attached: Option<Attached> = None;
    let mut last_probe: Option<Instant> = None;
    let mut last_good: Option<Instant> = None;
    let mut battle_at: Option<Instant> = None;
    let mut prev: Option<Brief> = None;
    let mut last_fp = String::new();
    let mut last_info: Option<LiveInfo> = None;
    // Lieu de la dernière carte lue (la ROM n'est consultée qu'au changement de carte).
    let mut place_of: Option<(u16, (Option<String>, Option<String>))> = None;
    let mut enabled = crate::companion::memory_enabled(&app);
    let mut config_at = Instant::now();
    let mine = |s: &Shared| s.save.as_deref() == Some(save.as_path());

    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        if config_at.elapsed() >= PROBE {
            enabled = crate::companion::memory_enabled(&app);
            config_at = Instant::now();
        }
        // Sauvegarde relue seulement quand elle change (nouvelle sauvegarde en jeu).
        let modified = std::fs::metadata(&save).ok().and_then(|m| m.modified().ok());
        if base.as_ref().is_none_or(|b| b.modified != modified) {
            if let Some(b) = read_base(&save) {
                if let Some(a) = attached.as_mut() {
                    a.reader.set_hints(b.hints.clone());
                }
                base = Some(b);
            }
        }

        let mut events: Vec<String> = Vec::new();
        let mut info = last_info.clone().unwrap_or_else(|| LiveInfo::new("offline", enabled));
        info.enabled = enabled;
        if !enabled {
            attached = None;
            if last_probe.is_none_or(|t| t.elapsed() >= PROBE) {
                last_probe = Some(Instant::now());
                let ctr = base.as_ref().is_some_and(|b| b.version.generation() >= 6);
                let found = emulators(ctr);
                info = LiveInfo::new(if found.is_empty() { "offline" } else { "ingame" }, false);
                info.emulator = found.first().map(|(_, n)| n.to_string());
            }
        } else if attached.is_none() {
            if let Some(b) = &base {
                if last_probe.is_none_or(|t| t.elapsed() >= PROBE) {
                    last_probe = Some(Instant::now());
                    let mut probe = LiveInfo::new("offline", true);
                    let rom = crate::companion::target_rom(&app);
                    attached = attach(b, &mut probe, rom.as_deref());
                    info = attached.as_ref().map(|a| a.info.clone()).unwrap_or(probe);
                    prev = None;
                }
            }
        }

        if let (Some(a), Some(b)) = (attached.as_mut(), base.as_ref()) {
            match a.reader.tick(&a.proc) {
                Err(_) => {
                    // Émulateur fermé : on repart à zéro.
                    attached = None;
                    last_good = None;
                    info = LiveInfo::new("offline", true);
                    update(|s| {
                        if mine(s) {
                            s.read = None;
                            s.battle = None;
                        }
                    });
                }
                Ok(None) => {
                    if last_good.is_none_or(|t| t.elapsed() >= STALE) {
                        info = LiveInfo { status: "ingame", detail: Some("Équipe pas encore trouvée en mémoire.".into()), ..a.info.clone() };
                    }
                }
                Ok(Some(read)) => {
                    last_good = Some(Instant::now());
                    info = LiveInfo { status: "live", ..a.info.clone() };
                    let snap = b.snapshot.with_memory(&read.party, read.map, read.badges);
                    let brief = Brief::from(&snap);
                    if let Some(p) = &prev {
                        events.extend(diff_memory(p, &brief).iter().map(GameEvent::text));
                    }
                    prev = Some(brief);

                    if place_of.as_ref().is_none_or(|(m, _)| *m != snap.map) {
                        place_of = Some((snap.map, crate::companion::place_now(&app, b.version, snap.generation, snap.map)));
                    }
                    let place = place_of.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
                    let mut s = SHARED.lock().unwrap_or_else(|e| e.into_inner());
                    if !mine(&s) {
                        break;
                    }
                    match read.battle {
                        Some(battle) => {
                            battle_at = Some(Instant::now());
                            let foes: Vec<LiveFoe> = battle.enemies.iter().map(LiveFoe::of).collect();
                            // Adversaire mis K.O. depuis la lecture précédente.
                            if let Some(old) = &s.battle {
                                for f in &foes {
                                    if f.hp == 0 && old.foes.iter().any(|o| o.pid == f.pid && o.hp > 0) {
                                        events.push(format!("{} adverse est K.O.", f.species_name));
                                    }
                                }
                            }
                            if battle.new && battle.wild {
                                if let Some(f) = foes.first() {
                                    let (place_name, route) = place.clone();
                                    let at_place = place_name.as_deref().map(|p| format!(", {p}")).unwrap_or_default();
                                    events.push(format!("Rencontre : {} niveau {}{at_place}", f.species_name, f.level));
                                    s.encounter = Some(Encounter {
                                        id: f.pid,
                                        species: f.species,
                                        form: f.form,
                                        species_name: f.species_name.clone(),
                                        level: f.level,
                                        shiny: f.shiny,
                                        place: place_name,
                                        route,
                                        outcome: None,
                                        at: crate::companion::now_ms(),
                                    });
                                }
                            }
                            s.battle = Some(BattleView { wild: battle.wild, foes });
                        }
                        // Combat vu par le jeu sans équipe adverse lisible (3DS) : carte « Combat en cours ».
                        None if read.in_battle == Some(true) => {
                            battle_at = Some(Instant::now());
                            if s.battle.as_ref().is_none_or(|b| !b.foes.is_empty()) {
                                s.battle = Some(BattleView { wild: false, foes: Vec::new() });
                            }
                        }
                        None if battle_at.is_none_or(|t| t.elapsed() >= BATTLE_HOLD) => s.battle = None,
                        None => {}
                    }
                    // Capture vue en mémoire : le Pokémon sauvage est entré dans l'équipe.
                    if let Some(e) = s.encounter.as_mut().filter(|e| e.outcome.is_none()) {
                        if read.party.iter().any(|p| p.pid() == e.id) {
                            e.outcome = Some("caught".into());
                        }
                    }
                    s.read = Some(MemoryRead { party: read.party, map: read.map, badges: read.badges });
                    s.info = Some(info.clone());
                    let fp = fingerprint(&snap, &s.battle, &s.encounter);
                    drop(s);
                    if fp != last_fp || !events.is_empty() {
                        last_fp = fp;
                        crate::companion::push_memory(&app, events.clone());
                    }
                }
            }
        }

        if last_info.as_ref() != Some(&info) {
            last_info = Some(info.clone());
            update(|s| {
                if mine(s) {
                    s.info = Some(info.clone());
                    if info.status != "live" {
                        s.battle = None;
                    }
                }
            });
            if events.is_empty() {
                crate::companion::push_memory(&app, Vec::new());
            }
        }
        if !SHARED.lock().map(|s| mine(&s)).unwrap_or(false) {
            break;
        }
        if let Some(rest) = TICK.checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}
