//! Bibliothèque de jeux : dossiers mémorisés, jaquettes et installation des
//! émulateurs en un clic.
//!
//! - Jaquettes : `cover://localhost/<jeu>[-en].png`, téléchargées une fois depuis
//!   libretro-thumbnails (dossier `Named_Boxarts`, noms No-Intro) puis gardées dans
//!   `<données>/covers`. `-en` demande la boîte européenne au lieu de la française
//!   (jeux DS seulement : les boîtes 3DS européennes sont multilingues).
//! - Émulateurs : dernière release GitHub officielle (archive Windows .zip),
//!   décompressée dans `<données locales>/emulators/<nom>`, puis l'exécutable est
//!   enregistré dans le profil de l'émulateur (voir `play.rs`).

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::play::{self, EmulatorId, EmulatorsState};

const USER_AGENT: &str = concat!("Kaleido/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mathis-ait/kaleido)");

// ---------------------------------------------------------------------------
// Dossiers de la bibliothèque

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryConfig {
    /// Dossiers parcourus à chaque ouverture.
    pub folders: Vec<String>,
    /// Fichiers ajoutés un par un.
    pub files: Vec<String>,
    /// Fichiers retirés de la bibliothèque (même s'ils sont dans un dossier suivi).
    pub hidden: Vec<String>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("library.json"))
}

#[tauri::command]
pub fn library_config(app: AppHandle) -> LibraryConfig {
    config_path(&app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn library_set_config(config: LibraryConfig, app: AppHandle) -> Result<(), String> {
    let path = config_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Jeux (ROMs et dossiers 3DS identifiés) contenus dans les dossiers et fichiers suivis.
#[tauri::command]
pub async fn library_scan(config: LibraryConfig) -> Result<Vec<kaleido_core::detect::Detection>, String> {
    crate::blocking(move || {
        let mut paths: Vec<PathBuf> = config.folders.iter().flat_map(|f| kaleido_core::detect::expand_path(Path::new(f))).collect();
        paths.extend(config.files.iter().map(PathBuf::from).filter(|p| p.exists()));
        paths.sort();
        paths.dedup();
        let games = paths
            .iter()
            .filter(|p| !config.hidden.iter().any(|h| Path::new(h) == p.as_path()))
            .filter_map(|p| kaleido_core::detect_path(p).ok())
            .filter(|d| d.game.is_some() && !matches!(d.kind, kaleido_core::detect::FileKind::Save | kaleido_core::detect::FileKind::Unknown))
            .collect();
        Ok(games)
    })
    .await
}

// ---------------------------------------------------------------------------
// Jaquettes

const COVERS_URL: &str = "https://raw.githubusercontent.com/libretro-thumbnails";

/// Nom No-Intro de la boîte : (dépôt, version française, version européenne).
fn cover_names(game: &str) -> Option<(&'static str, &'static str, &'static str)> {
    const DS: &str = "Nintendo_-_Nintendo_DS";
    const CTR: &str = "Nintendo_-_Nintendo_3DS";
    Some(match game {
        "diamond" => (DS, "Pokemon - Version Diamant (France) (Rev 5)", "Pokemon - Diamond Version (Europe) (Rev 5)"),
        "pearl" => (DS, "Pokemon - Version Perle (France) (Rev 5)", "Pokemon - Pearl Version (Europe) (Rev 5)"),
        "platinum" => (DS, "Pokemon - Version Platine (France)", "Pokemon - Platinum Version (Europe)"),
        "heart_gold" => (DS, "Pokemon - Version Or HeartGold (France)", "Pokemon - HeartGold Version (Europe)"),
        "soul_silver" => (DS, "Pokemon - Version Argent SoulSilver (France)", "Pokemon - SoulSilver Version (Europe)"),
        "black" => (DS, "Pokemon - Version Noire (France) (NDSi Enhanced)", "Pokemon - Black Version (USA, Europe) (NDSi Enhanced)"),
        "white" => (DS, "Pokemon - Version Blanche (France) (NDSi Enhanced)", "Pokemon - White Version (USA, Europe) (NDSi Enhanced)"),
        "black2" => (DS, "Pokemon - Version Noire 2 (France) (NDSi Enhanced)", "Pokemon - Black Version 2 (USA, Europe) (NDSi Enhanced)"),
        "white2" => (DS, "Pokemon - Version Blanche 2 (France) (NDSi Enhanced)", "Pokemon - White Version 2 (USA, Europe) (NDSi Enhanced)"),
        "x" => (CTR, "Pokemon X (Europe) (En,Ja,Fr,De,Es,It,Ko)", ""),
        "y" => (CTR, "Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko)", ""),
        "omega_ruby" => (CTR, "Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2)", ""),
        "alpha_sapphire" => (CTR, "Pokemon Alpha Sapphire (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2)", ""),
        "sun" => (CTR, "Pokemon Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)", ""),
        "moon" => (CTR, "Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)", ""),
        "ultra_sun" => (CTR, "Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)", ""),
        "ultra_moon" => (CTR, "Pokemon Ultra Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)", ""),
        _ => return None,
    })
}

/// Adresse de la jaquette pour `<jeu>` ou `<jeu>-en`.
pub fn cover_url(key: &str) -> Option<String> {
    let (game, english) = match key.strip_suffix("-en") {
        Some(g) => (g, true),
        None => (key, false),
    };
    let (repo, fr, en) = cover_names(game)?;
    let name = if english && !en.is_empty() { en } else { fr };
    // Les noms No-Intro ne contiennent que des lettres, chiffres, espaces, virgules et parenthèses.
    let encoded = name.replace(' ', "%20").replace(',', "%2C").replace('(', "%28").replace(')', "%29");
    Some(format!("{COVERS_URL}/{repo}/master/Named_Boxarts/{encoded}.png"))
}

fn covers_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("covers");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn load_cover(dir: &Path, key: &str) -> Result<Vec<u8>, String> {
    let url = cover_url(key).ok_or("jeu inconnu")?;
    let file = dir.join(format!("{key}.png"));
    if let Ok(data) = fs::read(&file) {
        return Ok(data);
    }
    let missing = dir.join(format!("{key}.missing"));
    if missing.exists() {
        return Err("pas de jaquette".into());
    }
    match crate::sprites::fetch(&url)? {
        Some(data) if data.starts_with(b"\x89PNG") => {
            let _ = fs::write(&file, &data);
            Ok(data)
        }
        _ => {
            let _ = fs::write(&missing, b"");
            Err("pas de jaquette".into())
        }
    }
}

/// Protocole `cover://` (voir l'en-tête du module).
pub fn handle_cover<R: Runtime>(app: &AppHandle<R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let key = request.uri().path().trim_start_matches('/').trim_end_matches(".png").to_string();
    let valid = !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    let result = if valid { covers_dir(app).and_then(|dir| load_cover(&dir, &key)) } else { Err("nom invalide".into()) };
    match result {
        Ok(data) => Response::builder().header("Content-Type", "image/png").header("Cache-Control", "max-age=31536000, immutable").body(data).unwrap(),
        Err(e) => Response::builder().status(StatusCode::NOT_FOUND).header("Content-Type", "text/plain; charset=utf-8").body(e.into_bytes()).unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Installation des émulateurs

/// Dépôt GitHub officiel et archive Windows 64 bits de chaque émulateur installable.
fn source(id: EmulatorId) -> Option<(&'static str, fn(&str) -> bool)> {
    Some(match id {
        EmulatorId::Melonds => ("melonDS-emu/melonDS", |n| n.contains("windows-x86_64") && n.ends_with(".zip")),
        EmulatorId::Azahar => ("azahar-emu/azahar", |n| n.starts_with("azahar-windows-msvc-") && n.ends_with(".zip") && !n.contains("installer")),
        EmulatorId::Desmume => ("TASEmulators/desmume", |n| n.ends_with("-win64.zip")),
        _ => return None,
    })
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDownload {
    id: EmulatorId,
    name: &'static str,
    version: String,
    file: String,
    size: u64,
    page: String,
}

fn latest_release(repo: &str) -> Result<Release, String> {
    let response = ureq::get(&format!("https://api.github.com/repos/{repo}/releases/latest"))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("impossible de joindre GitHub : {e}"))?;
    serde_json::from_reader(response.into_reader()).map_err(|e| format!("réponse de GitHub illisible : {e}"))
}

fn find_download(id: EmulatorId) -> Result<(Release, usize), String> {
    let (repo, wanted) = source(id).ok_or_else(|| format!("{} ne peut pas être installé automatiquement", id.name()))?;
    let release = latest_release(repo)?;
    let index = release.assets.iter().position(|a| wanted(&a.name)).ok_or_else(|| format!("pas d'archive Windows dans la dernière version de {}", id.name()))?;
    Ok((release, index))
}

/// Version et taille de ce qui serait téléchargé.
#[tauri::command]
pub async fn emulator_download_info(id: EmulatorId) -> Result<EmulatorDownload, String> {
    crate::blocking(move || {
        let (release, i) = find_download(id)?;
        let asset = &release.assets[i];
        Ok(EmulatorDownload {
            id,
            name: id.name(),
            version: release.tag_name.trim_start_matches("release_").replace('_', ".").trim_start_matches(['v', 'V']).to_string(),
            file: asset.name.clone(),
            size: asset.size,
            page: release.html_url.clone(),
        })
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    id: EmulatorId,
    /// « download » puis « extract ».
    step: &'static str,
    done: u64,
    total: u64,
}

/// Chemin relatif sûr d'une entrée d'archive (`\` accepté, `..` et chemins absolus refusés).
pub fn safe_entry_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(p) if !p.to_string_lossy().contains(':') => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn extract_zip(archive: &Path, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("archive illisible : {e}"))?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("archive illisible : {e}"))?;
        let Some(rel) = safe_entry_path(entry.name()) else { continue };
        let target = dest.join(rel);
        if entry.is_dir() || entry.name().ends_with(['/', '\\']) {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&target).map_err(|e| format!("{} : {e}", target.display()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
        progress(i as u64 + 1, total);
    }
    Ok(())
}

/// Télécharge la dernière version de l'émulateur, la décompresse et l'enregistre dans son profil.
#[tauri::command]
pub async fn emulator_install(id: EmulatorId, app: AppHandle) -> Result<EmulatorsState, String> {
    crate::blocking(move || {
        let (release, i) = find_download(id)?;
        let asset = &release.assets[i];
        let root = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("emulators");
        let dest = root.join(id.name());
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("emulator-install", InstallProgress { id, step, done, total });
        };

        // Téléchargement dans un fichier temporaire, avec progression.
        let archive = root.join(format!("{}.download", asset.name));
        let response = ureq::get(&asset.browser_download_url)
            .set("User-Agent", USER_AGENT)
            .call()
            .map_err(|e| format!("téléchargement impossible : {e}"))?;
        let total = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(asset.size);
        let mut reader = response.into_reader();
        let mut out = fs::File::create(&archive).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 256 * 1024];
        let (mut done, mut last) = (0u64, 0u64);
        emit("download", 0, total);
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("téléchargement interrompu : {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            if done - last >= 512 * 1024 {
                last = done;
                emit("download", done, total);
            }
        }
        drop(out);
        emit("download", done, total);

        let extracted = extract_zip(&archive, &dest, |d, t| emit("extract", d, t));
        let _ = fs::remove_file(&archive);
        extracted?;

        let exe = play::scan_for_exe(&dest, id, 3).ok_or_else(|| format!("{} introuvable dans l'archive téléchargée", id.name()))?;
        let mut config = play::load_config(&app);
        config.profiles.entry(id).or_default().exe = Some(exe);
        let preferred = if id.is_ctr() { &mut config.preferred_ctr } else { &mut config.preferred_nds };
        if preferred.is_none() {
            *preferred = Some(id);
        }
        play::store_config(&app, &config)?;
        Ok(play::state_of(config))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_urls() {
        assert_eq!(
            cover_url("platinum").unwrap(),
            "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Nintendo_DS/master/Named_Boxarts/Pokemon%20-%20Version%20Platine%20%28France%29.png"
        );
        assert!(cover_url("black2-en").unwrap().contains("Black%20Version%202%20%28USA%2C%20Europe%29"));
        // Pas de boîte « anglaise » séparée en 3DS.
        assert_eq!(cover_url("x-en"), cover_url("x"));
        assert!(cover_url("emerald").is_none());
    }

    #[test]
    fn archive_paths() {
        assert_eq!(safe_entry_path("azahar-windows-msvc-1\\plugins\\a.dll"), Some(PathBuf::from("azahar-windows-msvc-1/plugins/a.dll")));
        assert_eq!(safe_entry_path("melonDS.exe"), Some(PathBuf::from("melonDS.exe")));
        assert_eq!(safe_entry_path("../evil.exe"), None);
        assert_eq!(safe_entry_path("C:\\Windows\\evil.exe"), None);
        assert_eq!(safe_entry_path("/abs"), None);
    }

    #[test]
    fn extract_then_find_exe() {
        let tmp = std::env::temp_dir().join(format!("kaleido-zip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let archive = tmp.join("emu.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            w.add_directory("azahar-windows-msvc-1\\plugins\\", opts).unwrap();
            w.start_file("azahar-windows-msvc-1\\azahar.exe", opts).unwrap();
            w.write_all(b"MZ").unwrap();
            w.start_file("../evil.txt", opts).unwrap();
            w.write_all(b"no").unwrap();
            w.finish().unwrap();
        }
        let dest = tmp.join("out");
        let mut steps = 0;
        extract_zip(&archive, &dest, |_, _| steps += 1).unwrap();
        assert_eq!(steps, 2);
        assert!(!tmp.join("evil.txt").exists());
        assert_eq!(play::scan_for_exe(&dest, EmulatorId::Azahar, 3), Some(dest.join("azahar-windows-msvc-1").join("azahar.exe")));
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn asset_choice() {
        let (_, melon) = source(EmulatorId::Melonds).unwrap();
        assert!(melon("melonDS-1.1-windows-x86_64.zip") && !melon("melonDS-1.1-windows-aarch64.zip"));
        let (_, az) = source(EmulatorId::Azahar).unwrap();
        assert!(az("azahar-windows-msvc-2126.1.2.zip"));
        assert!(!az("azahar-windows-msvc-2126.1.2-installer.exe") && !az("azahar-libretro-windows-x86_64-2126.1.2.zip"));
        let (_, des) = source(EmulatorId::Desmume).unwrap();
        assert!(des("desmume-0.9.13-win64.zip") && !des("desmume-0.9.9a-win64.7z"));
        assert!(source(EmulatorId::Citra).is_none());
    }
}
