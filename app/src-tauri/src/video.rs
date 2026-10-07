//! Vidéo de l'écran titre (Soleil / Lune, Ultra-Soleil / Ultra-Lune) : le fichier
//! moflex de la ROM (vidéo Mobiclip) est converti une fois en WebM (VP9, sans le son,
//! que le lanceur joue déjà) puis gardé en cache.
//!
//! Seul ffmpeg sait décoder le Mobiclip : il est téléchargé à la demande (version LGPL
//! de BtbN/FFmpeg-Builds, ~75 Mo) dans `<données locales>/tools/ffmpeg`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{download_to, extract_zip, music_key, USER_AGENT};

/// Durée gardée (la vidéo boucle dans le lanceur).
const VIDEO_SECONDS: u32 = 60;

fn tool_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("tools").join("ffmpeg"))
}

/// `ffmpeg.exe` installé (dans le dossier `bin` de l'archive).
pub fn ffmpeg_exe(app: &AppHandle) -> Option<PathBuf> {
    fn find(dir: &Path, depth: u8) -> Option<PathBuf> {
        let exe = dir.join("ffmpeg.exe");
        if exe.is_file() {
            return Some(exe);
        }
        if depth == 0 {
            return None;
        }
        fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find(&e.path(), depth - 1))
    }
    find(&tool_dir(app).ok()?, 3)
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Archive choisie : version stable la plus récente, LGPL, bibliothèques partagées.
pub fn pick_asset(names: &[&str]) -> Option<usize> {
    let mut best: Option<(usize, (u32, u32))> = None;
    for (i, n) in names.iter().enumerate() {
        let Some(rest) = n.strip_prefix("ffmpeg-n") else { continue };
        if !n.contains("-win64-lgpl-shared-") || !n.ends_with(".zip") {
            continue;
        }
        let version = rest.split('-').next().unwrap_or("");
        let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        let v = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        if best.is_none_or(|(_, b)| v > b) {
            best = Some((i, v));
        }
    }
    best.map(|(i, _)| i)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegDownload {
    file: String,
    size: u64,
}

fn latest() -> Result<(Asset, Vec<Asset>), String> {
    let response = ureq::get("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest")
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("impossible de joindre GitHub : {e}"))?;
    let mut release: Release = serde_json::from_reader(response.into_reader()).map_err(|e| e.to_string())?;
    let names: Vec<&str> = release.assets.iter().map(|a| a.name.as_str()).collect();
    let i = pick_asset(&names).ok_or("pas d'archive Windows de ffmpeg dans la dernière version")?;
    let asset = release.assets.remove(i);
    Ok((asset, release.assets))
}

/// Taille du téléchargement (pour le demander à l'utilisateur).
#[tauri::command]
pub async fn ffmpeg_download_info() -> Result<FfmpegDownload, String> {
    crate::blocking(|| {
        let (a, _) = latest()?;
        Ok(FfmpegDownload { file: a.name, size: a.size })
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgress {
    step: &'static str,
    done: u64,
    total: u64,
}

#[tauri::command]
pub async fn ffmpeg_install(app: AppHandle) -> Result<(), String> {
    crate::blocking(move || {
        let (asset, _) = latest()?;
        let dir = tool_dir(&app)?;
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("tool-install", ToolProgress { step, done, total });
        };
        let archive = dir.join("ffmpeg.zip");
        download_to(&asset.browser_download_url, &archive, asset.size, |d, t| emit("download", d, t))?;
        let extracted = extract_zip(&archive, &dir, |d, t| emit("extract", d, t));
        let _ = fs::remove_file(&archive);
        extracted?;
        ffmpeg_exe(&app).map(|_| ()).ok_or_else(|| "ffmpeg.exe introuvable dans l'archive".into())
    })
    .await
}

fn game_from_id(id: &str) -> Option<kaleido_core::games::Game> {
    kaleido_core::games::Game::ALL.into_iter().find(|g| serde_json::to_value(g).ok().and_then(|v| v.as_str().map(|s| s == id)).unwrap_or(false))
}

/// Vidéo de l'écran titre en WebM (erreur « FFMPEG_MISSING » si ffmpeg n'est pas installé).
#[tauri::command]
pub async fn title_video(path: PathBuf, game: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::blocking(move || {
        let game = game_from_id(&game).ok_or("jeu inconnu")?;
        let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("video");
        let key = music_key(&path).ok_or("ROM introuvable")?;
        let cached = dir.join(format!("{key}.webm"));
        if let Ok(data) = fs::read(&cached) {
            return Ok(tauri::ipc::Response::new(data));
        }
        let moflex = kaleido_core::music::ctr::title_video(&path, game).map_err(|e| e.to_string())?;
        let exe = ffmpeg_exe(&app).ok_or("FFMPEG_MISSING")?;
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let input = dir.join(format!("{key}.moflex"));
        let output = dir.join(format!("{key}.part.webm"));
        fs::write(&input, moflex).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["-y", "-loglevel", "error", "-i"])
            .arg(&input)
            .args(["-an", "-t", &VIDEO_SECONDS.to_string(), "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "30", "-deadline", "good", "-cpu-used", "5", "-row-mt", "1", "-f", "webm"])
            .arg(&output);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let result = cmd.output();
        let _ = fs::remove_file(&input);
        let out = result.map_err(|e| format!("ffmpeg n'a pas pu être lancé : {e}"))?;
        if !out.status.success() || !output.is_file() {
            let _ = fs::remove_file(&output);
            return Err(format!("conversion de la vidéo impossible : {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")));
        }
        fs::rename(&output, &cached).map_err(|e| e.to_string())?;
        Ok(tauri::ipc::Response::new(fs::read(&cached).map_err(|e| e.to_string())?))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_lgpl_shared() {
        let names = [
            "ffmpeg-master-latest-win64-lgpl-shared.zip",
            "ffmpeg-n8.1-latest-win64-lgpl-shared-8.1.zip",
            "ffmpeg-n9.0-latest-win64-gpl-shared-9.0.zip",
            "ffmpeg-n9.0-latest-win64-lgpl-shared-9.0.zip",
            "ffmpeg-n9.0-latest-win64-lgpl-9.0.zip",
            "ffmpeg-n9.0-latest-linux64-lgpl-shared-9.0.tar.xz",
        ];
        assert_eq!(pick_asset(&names), Some(3));
    }
}
