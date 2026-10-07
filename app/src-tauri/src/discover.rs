//! « Rechercher sur ce PC » : parcourt les disques pour trouver les dossiers de jeux
//! (DS, 3DS, Switch) et les émulateurs dézippés n'importe où.
//!
//! Le parcours ignore les dossiers système et de développement (Windows, Program
//! Files, AppData, node_modules…), s'arrête à une profondeur raisonnable et a une
//! durée maximale : il reste rapide même sur un gros disque. Les émulateurs déjà
//! lancés une fois sont aussi retrouvés grâce à l'historique de Windows (voir
//! `play::launched_programs`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::play::{self, EmulatorId, Env};

/// Dossiers jamais parcourus (comparaison sans casse).
const SKIP: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "appdata",
    "$recycle.bin",
    "system volume information",
    "recovery",
    "perflogs",
    "msocache",
    "node_modules",
    "target",
    ".git",
    "__pycache__",
    "site-packages",
    "steamapps",
    "windowsapps",
    "drivers",
];

const MAX_DEPTH: usize = 7;
const MAX_ENTRIES: usize = 400_000;
const MAX_TIME: Duration = Duration::from_secs(45);
/// Une ROM DS fait au moins quelques Mo (évite les petits fichiers `.nds` de démo).
const MIN_GAME_SIZE: u64 = 1 << 20;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundFolder {
    pub path: String,
    #[serde(default)]
    pub gba: usize,
    pub nds: usize,
    pub ctr: usize,
    pub switch: usize,
    /// Quelques noms de fichiers, pour reconnaître le dossier.
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundEmulator {
    pub id: EmulatorId,
    pub name: &'static str,
    pub exe: String,
    /// Exécutable déjà utilisé par Kaleido pour cet émulateur.
    pub current: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub folders: Vec<FoundFolder>,
    pub emulators: Vec<FoundEmulator>,
    /// Le parcours s'est arrêté avant la fin (limite de temps ou de taille).
    pub partial: bool,
    pub scanned: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    scanned: usize,
    current: String,
}

fn kind_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "gba" | "gb" | "gbc" => "gba",
        "nds" => "nds",
        "3ds" | "cci" | "cia" => "ctr",
        e if crate::switch::EXTENSIONS.contains(&e) => "switch",
        _ => return None,
    })
}

fn skipped(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with('.') || lower.starts_with('$') || SKIP.contains(&lower.as_str())
}

struct Walk<'a> {
    started: Instant,
    scanned: usize,
    partial: bool,
    folders: BTreeMap<PathBuf, FoundFolder>,
    exes: Vec<PathBuf>,
    progress: &'a dyn Fn(usize, &Path),
}

impl Walk<'_> {
    fn over(&mut self) -> bool {
        if self.scanned >= MAX_ENTRIES || self.started.elapsed() > MAX_TIME {
            self.partial = true;
        }
        self.partial
    }

    fn dir(&mut self, dir: &Path, depth: usize) {
        if self.over() {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            self.scanned += 1;
            if self.scanned % 2000 == 0 {
                (self.progress)(self.scanned, dir);
            }
            let Ok(ft) = entry.file_type() else { continue };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if ft.is_dir() {
                if !skipped(&name) {
                    subdirs.push(path);
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let lower = name.to_lowercase();
            if lower.ends_with(".exe") {
                if EmulatorId::ALL.iter().any(|id| id.matches_exe(&lower)) {
                    self.exes.push(path);
                }
                continue;
            }
            let Some(kind) = kind_of(&path) else { continue };
            if entry.metadata().map(|m| m.len()).unwrap_or(0) < MIN_GAME_SIZE {
                continue;
            }
            let f = self.folders.entry(dir.to_path_buf()).or_insert_with(|| FoundFolder { path: dir.display().to_string(), ..Default::default() });
            match kind {
                "gba" => f.gba += 1,
                "nds" => f.nds += 1,
                "ctr" => f.ctr += 1,
                _ => f.switch += 1,
            }
            if f.examples.len() < 3 {
                f.examples.push(name);
            }
        }
        if depth == 0 {
            return;
        }
        subdirs.sort();
        for d in subdirs {
            self.dir(&d, depth - 1);
        }
    }
}

/// Dossiers trouvés, les plus fournis d'abord.
fn sorted(folders: BTreeMap<PathBuf, FoundFolder>) -> Vec<FoundFolder> {
    let mut list: Vec<FoundFolder> = folders.into_values().collect();
    list.sort_by(|a, b| (b.gba + b.nds + b.ctr + b.switch).cmp(&(a.gba + a.nds + a.ctr + a.switch)).then(a.path.cmp(&b.path)));
    list
}

/// Parcourt le dossier personnel, puis chaque disque (sans repasser par le dossier personnel).
pub fn discover(progress: &dyn Fn(usize, &Path)) -> Discovery {
    let mut walk = Walk { started: Instant::now(), scanned: 0, partial: false, folders: BTreeMap::new(), exes: Vec::new(), progress };
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    if let Some(h) = &home {
        walk.dir(h, MAX_DEPTH);
    }
    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        if root.is_dir() {
            walk.drive(&root, home.as_deref());
        }
    }
    let mut exes = walk.exes.clone();
    exes.extend(play::launched_programs());
    Discovery { folders: sorted(walk.folders), emulators: emulators_from(&exes, None), partial: walk.partial, scanned: walk.scanned }
}

impl Walk<'_> {
    /// Racine d'un disque : on saute le dossier qui contient le dossier personnel (`C:\Users`).
    fn drive(&mut self, root: &Path, home: Option<&Path>) {
        let Ok(entries) = fs::read_dir(root) else { return };
        let mut dirs: Vec<PathBuf> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if skipped(&name) || home.is_some_and(|h| h.starts_with(&d)) {
                continue;
            }
            self.dir(&d, MAX_DEPTH - 1);
        }
    }
}

/// Un exécutable par émulateur (le mieux placé, voir `play::best_exe`).
pub fn emulators_from(exes: &[PathBuf], current: Option<&BTreeMap<EmulatorId, PathBuf>>) -> Vec<FoundEmulator> {
    EmulatorId::ALL
        .iter()
        .filter_map(|&id| {
            let exe = play::best_exe(id, play::known_exes_of(id, exes))?;
            let current = current.and_then(|c| c.get(&id)).is_some_and(|c| c == &exe);
            Some(FoundEmulator { id, name: id.name(), exe: exe.display().to_string(), current })
        })
        .collect()
}

/// Cherche les jeux et les émulateurs sur tout le PC (évènements `discover-progress`).
#[tauri::command]
pub async fn discover_pc(app: AppHandle) -> Result<Discovery, String> {
    crate::blocking(move || {
        let emit = |scanned: usize, dir: &Path| {
            let _ = app.emit("discover-progress", Progress { scanned, current: dir.display().to_string() });
        };
        let mut found = discover(&emit);
        let config = play::load_config(&app);
        let env = Env::system(&config.search_dirs);
        let current: BTreeMap<EmulatorId, PathBuf> = EmulatorId::ALL.iter().filter_map(|&id| play::resolve(id, &config, &env).exe.map(|e| (id, e))).collect();
        for e in &mut found.emulators {
            e.current = current.get(&e.id).is_some_and(|c| c.display().to_string() == e.exe);
        }
        Ok(found)
    })
    .await
}

/// « Localiser » : l'utilisateur montre lui-même l'exécutable d'un émulateur.
#[tauri::command]
pub async fn emulator_locate(id: EmulatorId, exe: PathBuf, app: AppHandle) -> Result<play::EmulatorsState, String> {
    crate::blocking(move || {
        if !exe.is_file() {
            return Err(format!("fichier introuvable : {}", exe.display()));
        }
        let mut config = play::load_config(&app);
        config.profiles.entry(id).or_default().exe = Some(exe);
        play::store_config(&app, &config)?;
        Ok(play::state_of(config))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_games_and_emulators() {
        let tmp = std::env::temp_dir().join(format!("kaleido-discover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let big = vec![0u8; MIN_GAME_SIZE as usize];
        for (dir, file) in [
            ("Jeux/DS", "Platine.nds"),
            ("Jeux/DS", "Noire.nds"),
            ("Jeux/Switch/Arceus", "Arceus.xci"),
            ("node_modules/x", "piege.nds"),
            ("AppData/Roaming", "piege.3ds"),
        ] {
            fs::create_dir_all(tmp.join(dir)).unwrap();
            fs::write(tmp.join(dir).join(file), &big).unwrap();
        }
        fs::write(tmp.join("Jeux/DS/petit.nds"), b"x").unwrap();
        fs::create_dir_all(tmp.join("Emus/Eden-Windows-v0.2.1")).unwrap();
        fs::write(tmp.join("Emus/Eden-Windows-v0.2.1/eden.exe"), b"MZ").unwrap();
        fs::write(tmp.join("Emus/Eden-Windows-v0.2.1/eden-cli.exe"), b"MZ").unwrap();

        let mut walk = Walk { started: Instant::now(), scanned: 0, partial: false, folders: BTreeMap::new(), exes: Vec::new(), progress: &|_, _| {} };
        walk.dir(&tmp, MAX_DEPTH);
        let folders = sorted(walk.folders);
        assert_eq!(folders.len(), 2);
        assert_eq!((folders[0].nds, folders[0].examples.len()), (2, 2));
        assert!(folders[1].path.ends_with("Arceus") && folders[1].switch == 1);
        let emus = emulators_from(&walk.exes, None);
        assert_eq!(emus.len(), 1);
        assert_eq!(emus[0].id, EmulatorId::Eden);
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn muicache() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\MuiCache\r\n    C:\\Users\\T\\Documents\\Switch\\Eden-Windows-x\\eden.exe.FriendlyAppName    REG_SZ    eden.exe\r\n    C:\\Users\\T\\melonDS\\melonDS.exe.ApplicationCompany    REG_SZ    Melon\r\n    C:\\Users\\T\\melonDS\\melonDS.exe.FriendlyAppName    REG_SZ    melonDS\r\n    LangID    REG_BINARY    0904\r\n";
        let paths = play::parse_muicache(out);
        assert_eq!(paths, vec![PathBuf::from("C:\\Users\\T\\Documents\\Switch\\Eden-Windows-x\\eden.exe"), PathBuf::from("C:\\Users\\T\\melonDS\\melonDS.exe")]);
    }

    #[test]
    fn prefers_named_folder() {
        let tmp = std::env::temp_dir().join(format!("kaleido-best-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let a = tmp.join("Super Mario Galaxy").join("eden.exe");
        let b = tmp.join("Eden-Windows-abc").join("eden.exe");
        for p in [&b, &a] {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"MZ").unwrap();
        }
        assert_eq!(play::best_exe(EmulatorId::Eden, vec![a.clone(), b.clone()]), Some(b));
        fs::remove_dir_all(&tmp).unwrap();
    }
}

/// Sur le vrai PC : `cargo test -p kaleido-app real_pc -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn real_pc() {
    let t = Instant::now();
    let d = discover(&|_, _| {});
    println!("{} entrées en {:?} (partiel : {})", d.scanned, t.elapsed(), d.partial);
    for f in &d.folders {
        println!("  {} : DS {} · 3DS {} · Switch {} · {:?}", f.path, f.nds, f.ctr, f.switch, f.examples);
    }
    for e in &d.emulators {
        println!("  {} → {}", e.name, e.exe);
    }
}
