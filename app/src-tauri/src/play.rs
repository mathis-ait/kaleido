//! Bouton « Jouer » : profils d'émulateurs, lancement d'une ROM (ou d'un mod
//! LayeredFS 3DS) et synchronisation en direct de la sauvegarde.
//!
//! Conventions des émulateurs (vérifiées dans leurs sources) :
//!
//! - **melonDS** : la sauvegarde est `<dossier>/<nom de la ROM sans extension>.sav`,
//!   où `<dossier>` est le réglage `SaveFilePath` s'il est rempli, sinon le dossier
//!   de la ROM (`EmuInstance::getAssetPath(false, SaveFilePath, ".sav")`,
//!   src/frontend/qt_sdl/EmuInstance.cpp). Le réglage est dans `melonDS.toml`, table
//!   `[Instance0]` (src/frontend/qt_sdl/Config.cpp, `kConfigFile`), à côté de
//!   l'exécutable, dans `portable/`, ou dans le dossier de configuration de
//!   l'utilisateur (src/frontend/qt_sdl/main.cpp). Les versions 0.9 utilisaient
//!   `melonDS.ini` avec la même clé.
//! - **DeSmuME** : `<Battery>/<nom de la ROM sans extension>.dsv` ; `Battery` vaut
//!   `.\Battery\` par défaut, relatif au dossier de l'exécutable, et se règle dans
//!   `desmume.ini`, section `[PathSettings]`, clé `Battery` (src/path.h, src/path.cpp).
//!   Un `.dsv` sans pied DeSmuME est lu comme une carte vide (src/mc.cpp,
//!   `readFooter`) : on ajoute donc le pied quand on y copie un `.sav` brut.
//! - **Azahar / Citra / Lime3DS** : dossier utilisateur `user/` à côté de
//!   l'exécutable (mode portable) sinon `%APPDATA%\Azahar` (`Citra`, `Lime3DS`)
//!   (src/common/common_paths.h : `USERDATA_DIR`, `EMU_DATA_DIR`,
//!   `LEGACY_CITRA_DATA_DIR`, `LEGACY_LIME3DS_DATA_DIR`). Les mods sont lus dans
//!   `<user>/load/mods/<title ID en 16 chiffres hexa majuscules>/romfs/`
//!   (src/core/file_sys/ncch_container.cpp : `"{}mods/{:016X}/"` + `"romfs/"`).
//!   Les sauvegardes sont dans `<user>/sdmc/Nintendo 3DS/<id0>/<id1>/title/
//!   <8 hexa hauts>/<8 hexa bas>/data/00000001/` (src/core/file_sys/
//!   archive_source_sd_savedata.cpp), fichier `main` pour les jeux Pokémon.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------------------
// Émulateurs connus

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmulatorId {
    Melonds,
    Desmume,
    Azahar,
    Citra,
    Lime3ds,
}

impl EmulatorId {
    pub const ALL: [EmulatorId; 5] = [EmulatorId::Melonds, EmulatorId::Desmume, EmulatorId::Azahar, EmulatorId::Citra, EmulatorId::Lime3ds];

    pub fn name(self) -> &'static str {
        match self {
            EmulatorId::Melonds => "melonDS",
            EmulatorId::Desmume => "DeSmuME",
            EmulatorId::Azahar => "Azahar",
            EmulatorId::Citra => "Citra",
            EmulatorId::Lime3ds => "Lime3DS",
        }
    }

    pub fn is_ctr(self) -> bool {
        matches!(self, EmulatorId::Azahar | EmulatorId::Citra | EmulatorId::Lime3ds)
    }

    fn platform(self) -> &'static str {
        if self.is_ctr() {
            "3ds"
        } else {
            "nds"
        }
    }

    /// Nom du fichier exécutable (en minuscules) reconnu pour cet émulateur.
    fn matches_exe(self, lower: &str) -> bool {
        match self {
            EmulatorId::Melonds => lower == "melonds.exe",
            // Les archives officielles s'appellent par exemple `DeSmuME_0.9.13_x64.exe`.
            EmulatorId::Desmume => lower.starts_with("desmume") && lower.ends_with(".exe") && !lower.contains("cli"),
            EmulatorId::Azahar => matches!(lower, "azahar.exe" | "azahar-qt.exe"),
            EmulatorId::Citra => matches!(lower, "citra-qt.exe" | "citra.exe"),
            EmulatorId::Lime3ds => matches!(lower, "lime3ds.exe" | "lime3ds-gui.exe" | "lime3ds-qt.exe"),
        }
    }

    /// Noms des dossiers d'installation habituels.
    fn folder_names(self) -> &'static [&'static str] {
        match self {
            EmulatorId::Melonds => &["melonDS"],
            EmulatorId::Desmume => &["DeSmuME"],
            EmulatorId::Azahar => &["Azahar"],
            EmulatorId::Citra => &["Citra"],
            EmulatorId::Lime3ds => &["Lime3DS"],
        }
    }

    /// Dossier utilisateur dans `%APPDATA%` (émulateurs 3DS).
    fn data_dir_name(self) -> Option<&'static str> {
        match self {
            EmulatorId::Azahar => Some("Azahar"),
            EmulatorId::Citra => Some("Citra"),
            EmulatorId::Lime3ds => Some("Lime3DS"),
            _ => None,
        }
    }
}

/// Dossiers système où chercher (injectables pour les tests).
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub program_files: Vec<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub local_appdata: Option<PathBuf>,
    /// Dossiers choisis par l'utilisateur (émulateurs « portables »).
    pub extra: Vec<PathBuf>,
}

impl Env {
    pub fn system(extra: &[String]) -> Env {
        let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let mut program_files: Vec<PathBuf> = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(|k| var(k)).collect();
        program_files.dedup();
        Env {
            program_files,
            appdata: var("APPDATA"),
            local_appdata: var("LOCALAPPDATA"),
            extra: extra.iter().map(PathBuf::from).collect(),
        }
    }
}

/// Nombre maximal d'entrées lues par dossier pendant la recherche.
const SCAN_LIMIT: usize = 4000;

/// Cherche l'exécutable de `id` dans `dir`, jusqu'à `depth` niveaux de sous-dossiers.
pub fn scan_for_exe(dir: &Path, id: EmulatorId, depth: usize) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten().take(SCAN_LIMIT) {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_file() {
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if id.matches_exe(&lower) {
                return Some(path);
            }
        } else if kind.is_dir() {
            subdirs.push(path);
        }
    }
    if depth == 0 {
        return None;
    }
    subdirs.sort();
    subdirs.iter().find_map(|d| scan_for_exe(d, id, depth - 1))
}

/// Détection automatique : dossiers d'installation habituels puis dossiers de l'utilisateur.
pub fn detect_exe(id: EmulatorId, env: &Env) -> Option<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    for name in id.folder_names() {
        for pf in &env.program_files {
            places.push(pf.join(name));
        }
        if let Some(local) = &env.local_appdata {
            places.push(local.join("Programs").join(name));
            places.push(local.join(name));
        }
    }
    places.iter().find_map(|p| scan_for_exe(p, id, 2)).or_else(|| env.extra.iter().find_map(|p| scan_for_exe(p, id, 3)))
}

// ---------------------------------------------------------------------------
// Version d'un exécutable

/// Lit `ProductVersion` (ou `FileVersion`) dans la ressource de version d'un exécutable Windows.
pub fn pe_version(data: &[u8]) -> Option<String> {
    for key in ["ProductVersion", "FileVersion"] {
        let pat: Vec<u8> = key.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        let Some(pos) = data.windows(pat.len()).rposition(|w| w == pat.as_slice()) else { continue };
        let mut i = pos + pat.len();
        // Remplissage d'alignement avant la valeur.
        while i + 1 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            i += 2;
        }
        let mut units = Vec::new();
        while i + 1 < data.len() && units.len() < 64 {
            let u = u16::from_le_bytes([data[i], data[i + 1]]);
            if u == 0 {
                break;
            }
            units.push(u);
            i += 2;
        }
        let s = String::from_utf16_lossy(&units).trim().replace(", ", ".");
        if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(tidy_version(&s));
        }
    }
    None
}

/// « 1.0.0.0 » → « 1.0 ».
fn tidy_version(v: &str) -> String {
    let mut parts: Vec<&str> = v.split('.').collect();
    while parts.len() > 2 && parts.last() == Some(&"0") {
        parts.pop();
    }
    parts.join(".")
}

/// Version lisible dans un nom (« DeSmuME_0.9.13_x64 » → « 0.9.13 »).
pub fn version_from_name(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !(bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.')) {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let v = name[start..i].trim_end_matches('.');
            if v.contains('.') {
                return Some(v.to_string());
            }
        }
        i += 1;
    }
    None
}

fn exe_version(exe: &Path) -> Option<String> {
    type Versions = HashMap<(PathBuf, u64), Option<String>>;
    static CACHE: OnceLock<Mutex<Versions>> = OnceLock::new();
    let meta = fs::metadata(exe).ok()?;
    let key = (exe.to_path_buf(), meta.len());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().ok()?.get(&key) {
        return v.clone();
    }
    let v = fs::read(exe)
        .ok()
        .and_then(|d| pe_version(&d))
        .or_else(|| exe.file_stem().and_then(|s| version_from_name(&s.to_string_lossy())))
        .or_else(|| exe.parent().and_then(Path::file_name).and_then(|s| version_from_name(&s.to_string_lossy())));
    cache.lock().ok()?.insert(key, v.clone());
    v
}

// ---------------------------------------------------------------------------
// Réglages persistants

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    /// Exécutable choisi à la main (sinon : détection automatique).
    pub exe: Option<PathBuf>,
    /// Dossier des sauvegardes DS imposé (remplace celui de l'émulateur).
    pub save_dir: Option<PathBuf>,
    /// Dossier utilisateur 3DS imposé (celui qui contient `sdmc` et `load`).
    pub user_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayConfig {
    pub profiles: BTreeMap<EmulatorId, Profile>,
    pub search_dirs: Vec<String>,
    pub preferred_nds: Option<EmulatorId>,
    pub preferred_ctr: Option<EmulatorId>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("emulators.json"))
}

fn load_config(app: &AppHandle) -> PlayConfig {
    config_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

fn store_config(app: &AppHandle, config: &PlayConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

/// Émulateur prêt à l'emploi : exécutable et dossiers résolus.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub id: EmulatorId,
    pub exe: Option<PathBuf>,
    pub detected: bool,
    pub profile: Profile,
    pub env: Env,
}

pub fn resolve(id: EmulatorId, config: &PlayConfig, env: &Env) -> Resolved {
    let profile = config.profiles.get(&id).cloned().unwrap_or_default();
    let chosen = profile.exe.clone().filter(|p| p.is_file());
    let detected = chosen.is_none();
    let exe = chosen.or_else(|| detect_exe(id, env));
    Resolved { id, detected: detected && exe.is_some(), exe, profile, env: env.clone() }
}

impl Resolved {
    fn exe_dir(&self) -> Option<&Path> {
        self.exe.as_deref().and_then(Path::parent)
    }

    /// Dossier où l'émulateur DS range ses sauvegardes (`None` : à côté de la ROM).
    pub fn nds_save_dir(&self) -> Option<PathBuf> {
        if let Some(dir) = &self.profile.save_dir {
            return Some(dir.clone());
        }
        match self.id {
            EmulatorId::Melonds => melonds_config_save_dir(self.exe_dir(), &self.env),
            EmulatorId::Desmume => self.exe_dir().map(desmume_battery_dir),
            _ => None,
        }
    }

    /// Chemin de la sauvegarde que l'émulateur DS lira pour cette ROM.
    pub fn nds_save_path(&self, rom: &Path) -> PathBuf {
        let stem = rom.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let ext = if self.id == EmulatorId::Desmume { "dsv" } else { "sav" };
        let dir = self.nds_save_dir().unwrap_or_else(|| rom.parent().map(Path::to_path_buf).unwrap_or_default());
        dir.join(format!("{stem}.{ext}"))
    }

    /// Dossier utilisateur 3DS (contient `sdmc/` et `load/`).
    pub fn ctr_user_dir(&self) -> Option<PathBuf> {
        if let Some(dir) = &self.profile.user_dir {
            return Some(dir.clone());
        }
        if let Some(portable) = self.exe_dir().map(|d| d.join("user")).filter(|d| d.is_dir()) {
            return Some(portable);
        }
        Some(self.env.appdata.as_ref()?.join(self.id.data_dir_name()?))
    }
}

/// Valeur d'une clé dans un fichier INI ou TOML simple (`section = None` : n'importe où).
pub fn config_value(text: &str, section: Option<&str>, key: &str) -> Option<String> {
    let mut current = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.trim().to_string();
            continue;
        }
        if section.is_some_and(|s| !s.eq_ignore_ascii_case(&current)) {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        if !k.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        let v = v.trim();
        let value = if let Some(inner) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            inner.replace("\\\\", "\u{0}").replace("\\\"", "\"").replace('\u{0}', "\\")
        } else if let Some(inner) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            inner.to_string()
        } else {
            v.to_string()
        };
        return Some(value);
    }
    None
}

/// Dossier de sauvegarde imposé dans la configuration de melonDS, s'il y en a un.
pub fn melonds_config_save_dir(exe_dir: Option<&Path>, env: &Env) -> Option<PathBuf> {
    let mut candidates: Vec<(PathBuf, Option<&str>)> = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push((dir.join("portable").join("melonDS.toml"), Some("Instance0")));
        candidates.push((dir.join("melonDS.toml"), Some("Instance0")));
        candidates.push((dir.join("melonDS.ini"), None));
    }
    for base in [&env.local_appdata, &env.appdata].into_iter().flatten() {
        candidates.push((base.join("melonDS").join("melonDS.toml"), Some("Instance0")));
        candidates.push((base.join("melonDS").join("melonDS.ini"), None));
    }
    let (file, section) = candidates.into_iter().find(|(p, _)| p.is_file())?;
    let text = fs::read_to_string(&file).ok()?;
    let value = config_value(&text, section, "SaveFilePath").filter(|v| !v.trim().is_empty())?;
    let path = PathBuf::from(value);
    Some(if path.is_relative() { exe_dir.map(|d| d.join(&path)).unwrap_or(path) } else { path })
}

/// Dossier `Battery` de DeSmuME (relatif à l'exécutable par défaut).
pub fn desmume_battery_dir(exe_dir: &Path) -> PathBuf {
    let configured = fs::read_to_string(exe_dir.join("desmume.ini"))
        .ok()
        .and_then(|t| config_value(&t, Some("PathSettings"), "Battery"))
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| ".\\Battery\\".into());
    let cleaned = configured.trim().trim_start_matches(".\\").trim_start_matches("./").trim_end_matches(['\\', '/']).to_string();
    let path = PathBuf::from(&cleaned);
    if path.is_absolute() {
        path
    } else if cleaned.is_empty() || cleaned == "." {
        exe_dir.to_path_buf()
    } else {
        exe_dir.join(path)
    }
}

// ---------------------------------------------------------------------------
// Chemins 3DS

/// Title ID à partir d'un nom de dossier (« 000400000011C400 »).
pub fn parse_title_id(s: &str) -> Option<u64> {
    let s = s.trim();
    (s.len() == 16).then(|| u64::from_str_radix(s, 16).ok()).flatten()
}

/// Title ID d'un dossier `…/<title ID>/romfs` produit par le randomizer.
pub fn title_id_of_romfs(romfs: &Path) -> Option<u64> {
    parse_title_id(&romfs.parent()?.file_name()?.to_string_lossy())
}

/// `<user>/load/mods/<TITLE ID>` (le mod lui-même est dans `romfs/`).
pub fn mod_dir(user: &Path, title_id: u64) -> PathBuf {
    user.join("load").join("mods").join(format!("{title_id:016X}"))
}

fn is_hex_id(name: &str) -> bool {
    name.len() == 32 && name.chars().all(|c| c.is_ascii_hexdigit())
}

fn hex_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|r| r.flatten().filter(|e| e.path().is_dir() && is_hex_id(&e.file_name().to_string_lossy())).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Dossier de l'archive de sauvegarde d'un jeu 3DS dans la carte SD émulée.
pub fn ctr_save_dir(user: &Path, title_id: u64) -> PathBuf {
    let high = format!("{:08x}", title_id >> 32);
    let low = format!("{:08x}", title_id & 0xFFFF_FFFF);
    let tail = |base: &Path| base.join("title").join(&high).join(&low).join("data").join("00000001");
    let root = user.join("sdmc").join("Nintendo 3DS");
    // Identifiants de console et de carte SD : on garde ceux que l'émulateur a créés.
    let mut first = None;
    for id0 in hex_subdirs(&root) {
        for id1 in hex_subdirs(&id0) {
            if tail(&id1).is_dir() {
                return tail(&id1);
            }
            first.get_or_insert(id1);
        }
    }
    let zeros = "0".repeat(32);
    tail(&first.unwrap_or_else(|| root.join(&zeros).join(&zeros)))
}

// ---------------------------------------------------------------------------
// Copies de sécurité et copie de sauvegardes

/// Date UTC compacte pour les noms de copies : « 20261005-142233 ».
pub fn timestamp(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Algorithme « civil_from_days » (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `main` → `main.kaleido-<date>.bak`, avec un suffixe si ce nom est déjà pris.
pub fn backup_path(path: &Path, stamp: &str) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut n = 1;
    loop {
        let suffix = if n == 1 { String::new() } else { format!("-{n}") };
        let candidate = path.with_file_name(format!("{name}.kaleido-{stamp}{suffix}.bak"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Copie de sécurité d'un fichier existant (jamais d'écrasement d'une copie précédente).
pub fn backup_file(path: &Path, stamp: &str) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bak = backup_path(path, stamp);
    fs::copy(path, &bak).map_err(|e| format!("copie de sécurité impossible ({}) : {e}", path.display()))?;
    Ok(Some(bak))
}

const DSV_SNIP: &str = "|<--Snip above here to create a raw sav by excluding this DeSmuME savedata footer:";
const DSV_COOKIE: &str = "|-DESMUME SAVE-|";
pub const DSV_FOOTER: usize = 0x7A;

/// Taille et nombre d'octets d'adresse des types de mémoire de DeSmuME
/// (src/mc.cpp, `save_types[]`, sans « Autodetect »).
const DSV_TYPES: [(u32, u32); 13] = [
    (512, 1),
    (8 * 1024, 2),
    (64 * 1024, 2),
    (32 * 1024, 2),
    (256 * 1024, 3),
    (512 * 1024, 3),
    (1024 * 1024, 3),
    (2 * 1024 * 1024, 3),
    (4 * 1024 * 1024, 3),
    (8 * 1024 * 1024, 3),
    (16 * 1024 * 1024, 3),
    (32 * 1024 * 1024, 3),
    (64 * 1024 * 1024, 3),
];

fn has_dsv_footer(data: &[u8]) -> bool {
    data.len() >= DSV_FOOTER && data.ends_with(DSV_COOKIE.as_bytes())
}

/// Pied DeSmuME pour une sauvegarde brute (même ordre que `BackupDevice::flush`).
pub fn dsv_footer(size: u32) -> Vec<u8> {
    let (index, addr_size) = DSV_TYPES.iter().position(|(s, _)| *s == size).map(|i| (i as u32 + 1, DSV_TYPES[i].1)).unwrap_or((0, 3));
    let mut out = DSV_SNIP.as_bytes().to_vec();
    for v in [size, size, index, addr_size, size, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(DSV_COOKIE.as_bytes());
    out
}

/// Adapte une sauvegarde DS au format attendu par la cible (`.dsv` ↔ `.sav`).
pub fn convert_for(data: Vec<u8>, target: &Path) -> Vec<u8> {
    let to_dsv = target.extension().is_some_and(|e| e.eq_ignore_ascii_case("dsv"));
    let to_sav = target.extension().is_some_and(|e| e.eq_ignore_ascii_case("sav"));
    if to_dsv && !has_dsv_footer(&data) && DSV_TYPES.iter().any(|(s, _)| *s as usize == data.len()) {
        let mut out = data;
        let footer = dsv_footer(out.len() as u32);
        out.extend_from_slice(&footer);
        out
    } else if to_sav && has_dsv_footer(&data) {
        let mut out = data;
        out.truncate(out.len() - DSV_FOOTER);
        out
    } else {
        data
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Copie une sauvegarde vers l'emplacement de l'émulateur. Le fichier déjà
/// présent est toujours mis de côté avant d'être remplacé. Renvoie la copie de sécurité.
pub fn install_save(src: &Path, dst: &Path, stamp: &str) -> Result<Option<PathBuf>, String> {
    if same_file(src, dst) {
        return Ok(None);
    }
    let data = fs::read(src).map_err(|e| format!("lecture impossible ({}) : {e}", src.display()))?;
    let data = convert_for(data, dst);
    let backup = backup_file(dst, stamp)?;
    if let Some(dir) = dst.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(dst, data).map_err(|e| format!("écriture impossible ({}) : {e}", dst.display()))?;
    Ok(backup)
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// Dossier où sont rangés les anciens mods remplacés par Kaleido.
pub fn mod_backup_root(user: &Path) -> PathBuf {
    user.join("load").join("kaleido-backups")
}

/// Installe le dossier `romfs` d'un mod dans `<user>/load/mods/<title ID>/romfs`.
/// Un mod déjà présent n'est remplacé qu'avec `replace` ; il est alors déplacé
/// (pas supprimé) dans `load/kaleido-backups/`. Renvoie (dossier du mod, copie de sécurité).
pub fn install_mod(romfs: &Path, user: &Path, title_id: u64, replace: bool, stamp: &str) -> Result<(PathBuf, Option<PathBuf>), String> {
    if !romfs.is_dir() {
        return Err(format!("dossier du mod introuvable : {}", romfs.display()));
    }
    let target = mod_dir(user, title_id);
    if same_file(romfs, &target.join("romfs")) {
        return Ok((target, None));
    }
    let mut backup = None;
    if target.exists() {
        if !replace {
            return Err(format!("un mod est déjà installé pour ce jeu ({})", target.display()));
        }
        let root = mod_backup_root(user);
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut n = 1;
        let bak = loop {
            let suffix = if n == 1 { String::new() } else { format!("-{n}") };
            let candidate = root.join(format!("{title_id:016X}-{stamp}{suffix}"));
            if !candidate.exists() {
                break candidate;
            }
            n += 1;
        };
        fs::rename(&target, &bak).map_err(|e| format!("impossible de mettre l'ancien mod de côté : {e}"))?;
        backup = Some(bak);
    }
    copy_dir(romfs, &target.join("romfs")).map_err(|e| format!("copie du mod impossible : {e}"))?;
    // Petit mot pour reconnaître le mod (ignoré par l'émulateur).
    let _ = fs::write(target.join("kaleido.txt"), format!("Mod généré par Kaleido, copié depuis :\n{}\n", romfs.display()));
    Ok((target, backup))
}

// ---------------------------------------------------------------------------
// Surveillance de la sauvegarde (anti-rebond)

/// Empreinte d'un fichier : taille et date de modification.
pub type Stamp = (u64, u128);

pub fn stamp_of(path: &Path) -> Option<Stamp> {
    let m = fs::metadata(path).ok()?;
    let t = m.modified().ok()?.duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    Some((m.len(), t))
}

/// Signale un changement seulement quand le fichier est resté stable pendant `delay`
/// (les émulateurs écrivent souvent la sauvegarde en plusieurs fois).
#[derive(Debug)]
pub struct Debouncer {
    baseline: Option<Stamp>,
    pending: Option<(Stamp, Instant)>,
    delay: Duration,
}

impl Debouncer {
    pub fn new(baseline: Option<Stamp>, delay: Duration) -> Self {
        Debouncer { baseline, pending: None, delay }
    }

    /// Le fichier est maintenant dans l'état `current` : faut-il prévenir ?
    pub fn observe(&mut self, current: Option<Stamp>, now: Instant) -> bool {
        // Fichier absent (en cours de remplacement) : on attend qu'il revienne.
        let Some(cur) = current else {
            self.pending = None;
            return false;
        };
        if Some(cur) == self.baseline {
            self.pending = None;
            return false;
        }
        match self.pending {
            Some((s, since)) if s == cur => {
                if now.duration_since(since) >= self.delay {
                    self.baseline = Some(cur);
                    self.pending = None;
                    return true;
                }
            }
            _ => self.pending = Some((cur, now)),
        }
        false
    }

    /// Kaleido vient d'écrire le fichier lui-même : ce n'est pas le jeu.
    pub fn resync(&mut self, current: Option<Stamp>) {
        self.baseline = current;
        self.pending = None;
    }
}

struct Watching {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    debouncer: Arc<Mutex<Debouncer>>,
}

/// Sauvegarde surveillée (une seule à la fois).
#[derive(Default)]
pub struct SaveWatch(Mutex<Option<Watching>>);

#[derive(Clone, Serialize)]
struct SaveChanged {
    path: String,
}

const POLL: Duration = Duration::from_millis(500);
const DEBOUNCE: Duration = Duration::from_millis(1200);

/// Surveille `path` et émet l'évènement `save-changed` quand le jeu l'a réécrit.
#[tauri::command]
pub fn watch_save(path: PathBuf, app: AppHandle, state: State<'_, SaveWatch>) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.as_ref().is_some_and(|w| w.path == path) {
        return Ok(());
    }
    if let Some(old) = slot.take() {
        old.stop.store(true, Ordering::Relaxed);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let debouncer = Arc::new(Mutex::new(Debouncer::new(stamp_of(&path), DEBOUNCE)));
    let (s, d, p) = (stop.clone(), debouncer.clone(), path.clone());
    std::thread::spawn(move || {
        while !s.load(Ordering::Relaxed) {
            std::thread::sleep(POLL);
            let fire = d.lock().map(|mut d| d.observe(stamp_of(&p), Instant::now())).unwrap_or(false);
            if fire && !s.load(Ordering::Relaxed) {
                let _ = app.emit("save-changed", SaveChanged { path: p.display().to_string() });
            }
        }
    });
    *slot = Some(Watching { path, stop, debouncer });
    Ok(())
}

#[tauri::command]
pub fn unwatch_save(state: State<'_, SaveWatch>) -> Result<(), String> {
    if let Some(old) = state.0.lock().map_err(|e| e.to_string())?.take() {
        old.stop.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// À appeler après une écriture faite par Kaleido sur le fichier surveillé.
#[tauri::command]
pub fn watch_save_resync(state: State<'_, SaveWatch>) -> Result<(), String> {
    if let Some(w) = state.0.lock().map_err(|e| e.to_string())?.as_ref() {
        w.debouncer.lock().map_err(|e| e.to_string())?.resync(stamp_of(&w.path));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Commandes : profils

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorInfo {
    id: EmulatorId,
    name: &'static str,
    platform: &'static str,
    exe: Option<String>,
    /// Exécutable trouvé automatiquement (et non choisi à la main).
    detected: bool,
    /// Un exécutable a été choisi mais n'existe plus.
    missing: bool,
    version: Option<String>,
    /// Dossier des sauvegardes DS (`None` : à côté de la ROM) ou dossier utilisateur 3DS.
    data_dir: Option<String>,
    data_dir_exists: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorsState {
    emulators: Vec<EmulatorInfo>,
    config: PlayConfig,
}

fn info(r: &Resolved) -> EmulatorInfo {
    let data_dir = if r.id.is_ctr() { r.ctr_user_dir() } else { r.nds_save_dir() };
    EmulatorInfo {
        id: r.id,
        name: r.id.name(),
        platform: r.id.platform(),
        exe: r.exe.as_ref().map(|p| p.display().to_string()),
        detected: r.detected,
        missing: r.profile.exe.as_ref().is_some_and(|p| !p.is_file()),
        version: r.exe.as_deref().and_then(exe_version),
        data_dir_exists: data_dir.as_ref().is_some_and(|d| d.is_dir()),
        data_dir: data_dir.map(|d| d.display().to_string()),
    }
}

fn state_of(config: PlayConfig) -> EmulatorsState {
    let env = Env::system(&config.search_dirs);
    let emulators = EmulatorId::ALL.iter().map(|&id| info(&resolve(id, &config, &env))).collect();
    EmulatorsState { emulators, config }
}

/// Émulateurs connus, détectés ou configurés.
#[tauri::command]
pub async fn emulators_list(app: AppHandle) -> Result<EmulatorsState, String> {
    crate::blocking(move || Ok(state_of(load_config(&app)))).await
}

/// Enregistre les réglages des émulateurs et renvoie l'état à jour.
#[tauri::command]
pub async fn emulators_save(config: PlayConfig, app: AppHandle) -> Result<EmulatorsState, String> {
    crate::blocking(move || {
        store_config(&app, &config)?;
        Ok(state_of(config))
    })
    .await
}

/// Vérifie un exécutable : « Trouvé : melonDS 1.0 ».
#[tauri::command]
pub async fn emulator_test(id: EmulatorId, exe: Option<PathBuf>, app: AppHandle) -> Result<String, String> {
    crate::blocking(move || {
        let exe = match exe {
            Some(p) => p,
            None => {
                let config = load_config(&app);
                resolve(id, &config, &Env::system(&config.search_dirs)).exe.ok_or(format!("{} est introuvable", id.name()))?
            }
        };
        if !exe.is_file() {
            return Err(format!("fichier introuvable : {}", exe.display()));
        }
        let lower = exe.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let version = exe_version(&exe).map(|v| format!(" {v}")).unwrap_or_default();
        if id.matches_exe(&lower) {
            Ok(format!("Trouvé : {}{version}", id.name()))
        } else {
            Ok(format!("Fichier trouvé ({lower}), mais ce n'est pas le nom habituel de {} : vérifie qu'il s'agit du bon programme", id.name()))
        }
    })
    .await
}

// ---------------------------------------------------------------------------
// Commandes : jouer

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    pub emulator: EmulatorId,
    /// ROM DS (`.nds`) ou jeu 3DS de base à lancer.
    pub rom: Option<PathBuf>,
    /// Mod 3DS : dossier `romfs` produit par le randomizer.
    #[serde(default)]
    pub mod_romfs: Option<PathBuf>,
    /// Sauvegarde à installer avant de lancer.
    #[serde(default)]
    pub save: Option<PathBuf>,
    /// Remplacer (après copie de sécurité) un mod déjà installé.
    #[serde(default)]
    pub replace_mod: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayPlan {
    emulator: &'static str,
    exe: Option<String>,
    /// Sauvegarde que l'émulateur lira et écrira.
    save_path: Option<String>,
    save_exists: bool,
    /// La sauvegarde choisie remplacera un fichier existant (qui sera mis de côté).
    save_conflict: bool,
    title_id: Option<String>,
    mod_path: Option<String>,
    mod_exists: bool,
    /// 3DS : le jeu de base est nécessaire et n'a pas été fourni.
    needs_game: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayResult {
    emulator: &'static str,
    save_path: Option<String>,
    backups: Vec<String>,
    mod_path: Option<String>,
    warnings: Vec<String>,
}

/// Title ID d'un jeu 3DS de base (via la détection de Kaleido).
fn title_id_of_game(game: &Path) -> Option<u64> {
    let d = kaleido_core::detect_path(game).ok()?;
    d.details.iter().find(|x| x.label == "Title ID").and_then(|x| parse_title_id(&x.value))
}

fn request_title_id(req: &PlayRequest) -> Option<u64> {
    req.mod_romfs.as_deref().and_then(title_id_of_romfs).or_else(|| req.rom.as_deref().and_then(title_id_of_game))
}

/// Ce que `play_rom` fera, sans rien toucher (pour demander confirmation).
pub fn plan(req: &PlayRequest, r: &Resolved, title_id: Option<u64>) -> PlayPlan {
    let mut p = PlayPlan { emulator: r.id.name(), exe: r.exe.as_ref().map(|e| e.display().to_string()), ..Default::default() };
    if r.exe.is_none() {
        p.warnings.push(format!("{} est introuvable : choisis son exécutable dans Paramètres → Émulateurs.", r.id.name()));
    }
    if r.id.is_ctr() {
        p.needs_game = req.rom.as_ref().is_none_or(|g| !g.is_file());
        p.title_id = title_id.map(|t| format!("{t:016X}"));
        let Some(user) = r.ctr_user_dir() else {
            p.warnings.push("dossier utilisateur de l'émulateur introuvable".into());
            return p;
        };
        if let Some(tid) = title_id {
            if req.mod_romfs.is_some() {
                let dir = mod_dir(&user, tid);
                p.mod_exists = dir.exists() && !req.mod_romfs.as_deref().is_some_and(|m| same_file(m, &dir.join("romfs")));
                p.mod_path = Some(dir.display().to_string());
            }
            let save_dir = ctr_save_dir(&user, tid);
            let save = save_dir.join("main");
            p.save_exists = save.is_file();
            if req.save.is_some() && !save_dir.is_dir() {
                p.warnings.push(
                    "Le jeu n'a encore jamais été lancé dans cet émulateur : lance-le une fois, crée une partie, puis réessaie pour y mettre ta sauvegarde."
                        .into(),
                );
            }
            p.save_conflict = p.save_exists && req.save.as_deref().is_some_and(|s| !same_file(s, &save));
            p.save_path = Some(save.display().to_string());
        } else {
            p.warnings.push("title ID du jeu inconnu : impossible de placer le mod et la sauvegarde".into());
        }
    } else if let Some(rom) = &req.rom {
        let save = r.nds_save_path(rom);
        p.save_exists = save.is_file();
        p.save_conflict = p.save_exists && req.save.as_deref().is_some_and(|s| !same_file(s, &save));
        p.save_path = Some(save.display().to_string());
    }
    p
}

/// Prépare les fichiers (sauvegarde, mod) sans lancer l'émulateur.
pub fn prepare_files(req: &PlayRequest, r: &Resolved, title_id: Option<u64>, stamp: &str) -> Result<PlayResult, String> {
    let mut out = PlayResult { emulator: r.id.name(), ..Default::default() };
    if r.id.is_ctr() {
        let user = r.ctr_user_dir().ok_or("dossier utilisateur de l'émulateur introuvable")?;
        let tid = title_id.ok_or("title ID du jeu inconnu")?;
        if let Some(romfs) = &req.mod_romfs {
            let (dir, backup) = install_mod(romfs, &user, tid, req.replace_mod, stamp)?;
            out.mod_path = Some(dir.display().to_string());
            out.backups.extend(backup.map(|b| b.display().to_string()));
        }
        let save_dir = ctr_save_dir(&user, tid);
        let target = save_dir.join("main");
        if let Some(src) = &req.save {
            // Sans archive préparée par l'émulateur, le jeu formaterait la sauvegarde.
            if save_dir.is_dir() {
                out.backups.extend(install_save(src, &target, stamp)?.map(|b| b.display().to_string()));
            } else {
                out.warnings.push("Sauvegarde non copiée : lance le jeu une première fois dans l'émulateur et crée une partie, puis réessaie.".into());
            }
        }
        out.save_path = Some(target.display().to_string());
    } else {
        let rom = req.rom.as_ref().ok_or("aucune ROM à lancer")?;
        let target = r.nds_save_path(rom);
        if let Some(src) = &req.save {
            out.backups.extend(install_save(src, &target, stamp)?.map(|b| b.display().to_string()));
        }
        out.save_path = Some(target.display().to_string());
    }
    Ok(out)
}

fn launch(exe: &Path, game: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg(game);
    // DeSmuME résout ses dossiers relatifs (Battery…) depuis son propre dossier.
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("impossible de lancer {} : {e}", exe.display()))
}

#[tauri::command]
pub async fn play_plan(request: PlayRequest, app: AppHandle) -> Result<PlayPlan, String> {
    crate::blocking(move || {
        let config = load_config(&app);
        let r = resolve(request.emulator, &config, &Env::system(&config.search_dirs));
        let tid = if r.id.is_ctr() { request_title_id(&request) } else { None };
        Ok(plan(&request, &r, tid))
    })
    .await
}

/// Installe la sauvegarde (et le mod 3DS) puis lance l'émulateur.
#[tauri::command]
pub async fn play_rom(request: PlayRequest, app: AppHandle) -> Result<PlayResult, String> {
    crate::blocking(move || {
        let config = load_config(&app);
        let r = resolve(request.emulator, &config, &Env::system(&config.search_dirs));
        let exe = r.exe.clone().ok_or(format!("{} est introuvable : choisis son exécutable dans Paramètres → Émulateurs", r.id.name()))?;
        let game = request.rom.clone().filter(|g| g.is_file()).ok_or("choisis le fichier du jeu à lancer")?;
        let tid = if r.id.is_ctr() { request_title_id(&request) } else { None };
        let result = prepare_files(&request, &r, tid, &timestamp(SystemTime::now()))?;
        launch(&exe, &game)?;
        Ok(result)
    })
    .await
}

/// Copie de sécurité datée d'un fichier (avant « Envoyer au jeu »).
#[tauri::command]
pub fn play_backup(path: PathBuf) -> Result<Option<String>, String> {
    Ok(backup_file(&path, &timestamp(SystemTime::now()))?.map(|p| p.display().to_string()))
}

/// Fichier temporaire où l'éditeur écrit la sauvegarde avant de l'envoyer au jeu.
#[tauri::command]
pub fn play_temp_save(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("envoi-au-jeu.tmp").display().to_string())
}

/// « Envoyer au jeu » : copie `src` vers la sauvegarde de l'émulateur (copie de sécurité d'abord,
/// conversion `.sav` ↔ `.dsv` si besoin). Renvoie la copie de sécurité.
#[tauri::command]
pub fn play_install_save(src: PathBuf, dst: PathBuf) -> Result<Option<String>, String> {
    Ok(install_save(&src, &dst, &timestamp(SystemTime::now()))?.map(|p| p.display().to_string()))
}

/// ROM DS correspondant à une sauvegarde (`Platine.sav` → `Platine.nds` dans le même dossier).
#[tauri::command]
pub fn play_find_rom(save: PathBuf) -> Option<String> {
    let stem = save.file_stem()?.to_string_lossy().into_owned();
    let dir = save.parent()?;
    let rom = dir.join(format!("{stem}.nds"));
    rom.is_file().then(|| rom.display().to_string())
}

#[cfg(test)]
mod tests;
