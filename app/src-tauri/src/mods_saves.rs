//! Sauvegardes de partie autour des mods, et profils de mods.
//!
//! - **Copies de sauvegarde** : avant d'activer un mod qui touche à la partie (romhack,
//!   rééquilibrage, mod qui doit rester seul), Kaleido copie la sauvegarde du jeu dans
//!   `<données>/mods-saves/<TITLEID>/<date> …/`, avec un `kaleido-backup.json`. Les 10 plus
//!   récentes sont gardées. La remise en place copie d'abord la partie actuelle.
//! - **Profils** : une liste de mods actifs par jeu (`<données>/mods-profiles.json`). Choisir
//!   un profil active ses mods et met les autres de côté. Un profil qui contient un mod de
//!   partie garde sa propre sauvegarde (`mods-saves/<TITLEID>/profils/<nom>/`), échangée au
//!   changement de profil.
//!
//! Emplacements : Eden `<nand>/user/save/0000000000000000/<utilisateur>/<TITLEID>/` ;
//! Azahar `sdmc/Nintendo 3DS/<id0>/<id1>/title/00040000/<jeu>/data/`. Les romhacks DS
//! créent une nouvelle ROM, donc une sauvegarde à part : rien à faire.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::mods::{copy_dir, ctr_emulator, resolve, ModEntry, ModTarget};
use crate::play::EmulatorId;
use crate::tuning;

/// Copies gardées par jeu (hors emplacements des profils).
const KEEP: usize = 10;
const META: &str = "kaleido-backup.json";

/// Le mod change la partie : une sauvegarde faite avec lui ne sert plus au jeu d'origine.
pub fn affects_save(e: &ModEntry) -> bool {
    e.exclusive || matches!(e.category.as_str(), "romhack" | "gameplay")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackup {
    /// Nom du dossier de la copie.
    pub id: String,
    /// « avant Luminescent Platinum », « avant la remise en place »…
    pub reason: String,
    /// Mod qui a déclenché la copie.
    pub mod_id: Option<String>,
    pub created: u64,
    pub size: u64,
    /// Dossiers copiés, relatifs au dossier de l'émulateur.
    #[serde(default)]
    pub paths: Vec<String>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir).into_iter().flatten().flatten().map(|e| if e.path().is_dir() { dir_size(&e.path()) } else { e.metadata().map_or(0, |m| m.len()) }).sum()
}

fn root_dir(app: &AppHandle, tid: u64) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("mods-saves").join(format!("{tid:016X}")))
}

/// Dossier de l'émulateur et dossiers de sauvegarde du jeu (relatifs à lui).
fn save_dirs(app: &AppHandle, target: &ModTarget, tid: u64) -> Result<(PathBuf, Vec<PathBuf>), String> {
    match target.platform.as_str() {
        "switch" => {
            let r = resolve(EmulatorId::Eden, app);
            let user = r.ctr_user_dir().ok_or("dossier d'Eden introuvable")?;
            let configured = fs::read_to_string(user.join("config").join("qt-config.ini"))
                .ok()
                .and_then(|t| tuning::get_qt(&t, "Data%20Storage", "nand_directory"))
                .filter(|v| !v.is_empty())
                .map(PathBuf::from);
            let nand = configured.unwrap_or_else(|| user.join("nand"));
            let base = nand.join("user").join("save").join("0000000000000000");
            let tid_dir = format!("{tid:016X}");
            let mut dirs = Vec::new();
            for profile in fs::read_dir(&base).into_iter().flatten().flatten() {
                let d = profile.path().join(&tid_dir);
                if d.is_dir() {
                    dirs.push(d);
                }
            }
            Ok((nand, dirs))
        }
        "3ds" => {
            let user = ctr_emulator(app).ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
            let low = format!("{:08x}", tid & 0xFFFF_FFFF);
            let mut dirs = Vec::new();
            for id0 in fs::read_dir(user.join("sdmc").join("Nintendo 3DS")).into_iter().flatten().flatten() {
                for id1 in fs::read_dir(id0.path()).into_iter().flatten().flatten() {
                    let d = id1.path().join("title").join("00040000").join(&low).join("data");
                    if d.is_dir() {
                        dirs.push(d);
                    }
                }
            }
            Ok((user, dirs))
        }
        _ => Err("pas de sauvegarde gérée pour cette console".into()),
    }
}

/// Refuse de toucher aux sauvegardes pendant que l'émulateur tourne.
fn ensure_closed(target: &ModTarget) -> Result<(), String> {
    let name = if target.platform == "switch" { "Eden" } else { "Azahar" };
    let running = crate::library::running_emulators();
    if running.iter().any(|r| *r == name || (target.platform == "3ds" && matches!(*r, "Lime3DS" | "Citra"))) {
        return Err(format!("Ferme {name} d'abord : la sauvegarde du jeu est en cours d'utilisation."));
    }
    Ok(())
}

/// Copie la sauvegarde actuelle dans `dest` (remplacé). `None` : le jeu n'a pas de sauvegarde.
fn copy_current(app: &AppHandle, target: &ModTarget, tid: u64, dest: &Path) -> Result<Option<Vec<String>>, String> {
    let (root, dirs) = save_dirs(app, target, tid)?;
    let dirs: Vec<PathBuf> = dirs.into_iter().filter(|d| fs::read_dir(d).map(|mut r| r.next().is_some()).unwrap_or(false)).collect();
    if dirs.is_empty() {
        return Ok(None);
    }
    let _ = fs::remove_dir_all(dest);
    let mut paths = Vec::new();
    for d in &dirs {
        let rel = d.strip_prefix(&root).map_err(|_| "chemin de sauvegarde inattendu".to_string())?;
        copy_dir(d, &dest.join(rel)).map_err(|e| format!("copie de la sauvegarde impossible : {e}"))?;
        paths.push(rel.to_string_lossy().replace('\\', "/"));
    }
    Ok(Some(paths))
}

/// Copie la sauvegarde du jeu avant un changement. `None` : pas encore de sauvegarde.
pub fn backup(app: &AppHandle, target: &ModTarget, tid: u64, reason: &str, mod_id: Option<&str>) -> Result<Option<SaveBackup>, String> {
    ensure_closed(target)?;
    let created = now();
    let stamp = chrono_like(created);
    let id = folder(&format!("{stamp} {reason}"));
    let dest = root_dir(app, tid)?.join(&id);
    let Some(paths) = copy_current(app, target, tid, &dest)? else { return Ok(None) };
    let b = SaveBackup { id, reason: reason.to_string(), mod_id: mod_id.map(str::to_string), created, size: dir_size(&dest), paths };
    fs::write(dest.join(META), serde_json::to_vec_pretty(&b).unwrap_or_default()).map_err(|e| e.to_string())?;
    prune(app, tid);
    Ok(Some(b))
}

/// Date en `AAAA-MM-JJ HHhMM` (UTC), pour des noms de dossiers triés.
fn chrono_like(secs: u64) -> String {
    let days = secs / 86_400;
    let (h, m) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Conversion jours → date civile (algorithme de Howard Hinnant).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02} {h:02}h{m:02}")
}

fn folder(name: &str) -> String {
    name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) { '-' } else { c }).collect::<String>().trim_end_matches(['.', ' ']).chars().take(120).collect()
}

/// Copies d'un jeu, de la plus récente à la plus ancienne.
pub fn list(app: &AppHandle, tid: u64) -> Vec<SaveBackup> {
    let Ok(root) = root_dir(app, tid) else { return vec![] };
    let mut out: Vec<SaveBackup> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| fs::read(e.path().join(META)).ok().and_then(|d| serde_json::from_slice::<SaveBackup>(&d).ok()))
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn prune(app: &AppHandle, tid: u64) {
    let Ok(root) = root_dir(app, tid) else { return };
    for old in list(app, tid).into_iter().skip(KEEP) {
        let _ = fs::remove_dir_all(root.join(old.id));
    }
}

/// Remet en place une copie (dossier `src` d'une copie ou d'un profil) à la place de la
/// sauvegarde actuelle.
fn put_back(app: &AppHandle, target: &ModTarget, tid: u64, src: &Path, paths: &[String]) -> Result<(), String> {
    let (root, current) = save_dirs(app, target, tid)?;
    for d in current {
        fs::remove_dir_all(&d).map_err(|e| format!("impossible de retirer la sauvegarde actuelle : {e}"))?;
    }
    for rel in paths {
        if rel.split('/').any(|p| p == "..") {
            continue;
        }
        copy_dir(&src.join(rel), &root.join(rel)).map_err(|e| format!("remise en place impossible : {e}"))?;
    }
    Ok(())
}

/// Remet une copie : la partie actuelle est d'abord copiée à son tour.
pub fn restore(app: &AppHandle, target: &ModTarget, tid: u64, id: &str) -> Result<(), String> {
    if id.contains(['/', '\\']) || id == ".." {
        return Err("copie invalide".into());
    }
    let b = list(app, tid).into_iter().find(|b| b.id == id).ok_or("copie introuvable")?;
    backup(app, target, tid, "avant la remise en place", None)?;
    let src = root_dir(app, tid)?.join(&b.id);
    put_back(app, target, tid, &src, &b.paths)
}

/// Dernière copie faite avant d'activer ce mod (proposée quand on le désactive).
pub fn offer_for(app: &AppHandle, tid: u64, mod_id: &str) -> Option<SaveBackup> {
    list(app, tid).into_iter().find(|b| b.mod_id.as_deref() == Some(mod_id))
}

// ---------------------------------------------------------------------------
// Profils

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    /// Mods actifs : `id:<identifiant Kaleido>` ou `folder:<dossier installé à la main>`.
    pub mods: Vec<String>,
    /// Garde sa propre sauvegarde (contient un mod qui touche à la partie).
    pub own_save: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfiles {
    pub active: Option<String>,
    pub profiles: Vec<Profile>,
}

/// Nom du profil sans mod, toujours proposé.
pub const ORIGIN: &str = "Jeu d'origine";

fn profiles_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("mods-profiles.json"))
}

fn load_all(app: &AppHandle) -> BTreeMap<String, GameProfiles> {
    profiles_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

fn save_all(app: &AppHandle, all: &BTreeMap<String, GameProfiles>) -> Result<(), String> {
    let p = profiles_path(app)?;
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(p, serde_json::to_vec_pretty(all).unwrap_or_default()).map_err(|e| e.to_string())
}

pub fn game_profiles(app: &AppHandle, tid: u64) -> GameProfiles {
    load_all(app).remove(&format!("{tid:016X}")).unwrap_or_default()
}

pub fn set_game_profiles(app: &AppHandle, tid: u64, g: GameProfiles) -> Result<(), String> {
    let mut all = load_all(app);
    all.insert(format!("{tid:016X}"), g);
    save_all(app, &all)
}

/// Mods actifs d'une vue, au format des profils.
pub fn active_keys(view: &crate::mods::ModsView) -> Vec<String> {
    let mut keys: Vec<String> = view
        .mods
        .iter()
        // Les codes, les ROM patchées et les ajouts automatiques (sauf Fl4sh) ne se désactivent pas.
        .filter(|m| m.installed && m.enabled && !matches!(m.kind.as_deref(), Some("cheats") | Some("patch")) && (m.source != "auto" || m.id.starts_with("fl4sh:")))
        .map(|m| format!("id:{}", m.id))
        .chain(view.others.iter().filter(|o| o.enabled && o.can_toggle).map(|o| format!("folder:{}", o.name)))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Ce que l'application d'un profil doit changer : (à activer, à désactiver, absents).
pub fn plan(view: &crate::mods::ModsView, wanted: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let current = active_keys(view);
    let known: Vec<String> = view
        .mods
        .iter()
        .filter(|m| m.installed)
        .map(|m| format!("id:{}", m.id))
        .chain(view.others.iter().filter(|o| o.can_toggle).map(|o| format!("folder:{}", o.name)))
        .collect();
    let enable = wanted.iter().filter(|k| !current.contains(k) && known.contains(k)).cloned().collect();
    let disable = current.iter().filter(|k| !wanted.contains(k)).cloned().collect();
    let missing = wanted.iter().filter(|k| !known.contains(k)).cloned().collect();
    (enable, disable, missing)
}

fn slot_dir(app: &AppHandle, tid: u64, profile: &str) -> Result<PathBuf, String> {
    Ok(root_dir(app, tid)?.join("profils").join(folder(profile)))
}

/// Échange les sauvegardes au changement de profil : la partie actuelle va dans l'emplacement
/// du profil quitté, celle du profil choisi (s'il en a une) revient en place.
pub fn swap_saves(app: &AppHandle, target: &ModTarget, tid: u64, from: &str, to: &str) -> Result<bool, String> {
    ensure_closed(target)?;
    let out_slot = slot_dir(app, tid, from)?;
    if let Some(paths) = copy_current(app, target, tid, &out_slot)? {
        let meta = SaveBackup { id: format!("profil {from}"), reason: format!("partie du profil « {from} »"), mod_id: None, created: now(), size: dir_size(&out_slot), paths };
        fs::write(out_slot.join(META), serde_json::to_vec_pretty(&meta).unwrap_or_default()).map_err(|e| e.to_string())?;
    }
    let in_slot = slot_dir(app, tid, to)?;
    let Some(meta) = fs::read(in_slot.join(META)).ok().and_then(|d| serde_json::from_slice::<SaveBackup>(&d).ok()) else { return Ok(false) };
    put_back(app, target, tid, &in_slot, &meta.paths)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(chrono_like(0), "1970-01-01 00h00");
        assert_eq!(chrono_like(1_791_504_000 + 3600 * 13 + 60 * 7), "2026-10-09 13h07");
    }

    #[test]
    fn profile_plan() {
        let entry = |id: &str, on: bool| ModEntry { id: id.into(), installed: true, enabled: on, source: "gamebanana".into(), ..Default::default() };
        let view = crate::mods::ModsView {
            mods: vec![entry("gb:1", true), entry("gb:2", false), entry("nexus:lumi", true)],
            others: vec![crate::mods::OtherMod { name: "Mon mod".into(), enabled: true, category: "other".into(), overlaps: vec![], can_toggle: true, exefs: None, from_kaleido: false }],
            ..Default::default()
        };
        assert_eq!(active_keys(&view), vec!["folder:Mon mod", "id:gb:1", "id:nexus:lumi"]);
        let (on, off, missing) = plan(&view, &["id:gb:2".into(), "id:gb:9".into(), "folder:Mon mod".into()]);
        assert_eq!(on, vec!["id:gb:2"]);
        assert_eq!(off, vec!["id:gb:1", "id:nexus:lumi"]);
        assert_eq!(missing, vec!["id:gb:9"]);
    }
}
