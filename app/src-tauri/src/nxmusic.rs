//! Musique des jeux Switch dans le lanceur : morceau choisi automatiquement (voir
//! `kaleido_core::music::nx::auto_track`) ou par l'utilisateur, lu dans son jeu avec
//! ses propres clés.
//!
//! Kaleido décode lui-même l'Opus des jeux Wwise et les flux AST ; les autres formats
//! (BFSTM, BWAV, CRI HCA/ADX, FMOD…) passent par **vgmstream** (licence ISC), téléchargé
//! à la demande depuis sa page GitHub officielle dans `<données locales>/tools/vgmstream`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kaleido_core::music::nx::{self as nxm, Track, TrackData};
use kaleido_core::music::Pcm;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{download_to, extract_zip, music_key, MUSIC_SECONDS, USER_AGENT};

// ---------------------------------------------------------------------------
// Choix de l'utilisateur

/// Morceau choisi par jeu (title ID → identifiant de morceau).
type Choices = BTreeMap<String, String>;

fn choices_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("switch-music.json"))
}

fn load_choices(app: &AppHandle) -> Choices {
    choices_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

fn store_choices(app: &AppHandle, c: &Choices) -> Result<(), String> {
    let path = choices_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn keys(app: &AppHandle) -> Result<kaleido_core::nx::Keys, String> {
    let file = crate::switch::prod_keys(app).ok_or("clés de la console introuvables (prod.keys d'Eden)")?;
    kaleido_core::nx::Keys::load(&file).map_err(|e| e.to_string())
}

fn parse_tid(s: &str) -> Result<u64, String> {
    u64::from_str_radix(s, 16).map_err(|_| "title ID invalide".to_string())
}

// ---------------------------------------------------------------------------
// vgmstream

fn tool_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("tools").join("vgmstream"))
}

fn vgmstream_exe(app: &AppHandle) -> Option<PathBuf> {
    let dir = tool_dir(app).ok()?;
    let exe = dir.join("vgmstream-cli.exe");
    exe.is_file().then_some(exe)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgress {
    step: &'static str,
    done: u64,
    total: u64,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Télécharge la dernière version Windows 64 bits de vgmstream.
#[tauri::command]
pub async fn vgmstream_install(app: AppHandle) -> Result<String, String> {
    crate::blocking(move || {
        let response = ureq::get("https://api.github.com/repos/vgmstream/vgmstream/releases/latest")
            .set("User-Agent", USER_AGENT)
            .set("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(15))
            .call()
            .map_err(|e| format!("impossible de joindre GitHub : {e}"))?;
        let release: Release = serde_json::from_reader(response.into_reader()).map_err(|e| e.to_string())?;
        let asset = release.assets.iter().find(|a| a.name == "vgmstream-win64.zip").ok_or("pas d'archive Windows dans la dernière version de vgmstream")?;
        let dir = tool_dir(&app)?;
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("tool-install", ToolProgress { step, done, total });
        };
        let archive = dir.join("vgmstream.zip");
        download_to(&asset.browser_download_url, &archive, asset.size, |d, t| emit("download", d, t))?;
        let extracted = extract_zip(&archive, &dir, |d, t| emit("extract", d, t));
        let _ = fs::remove_file(&archive);
        extracted?;
        vgmstream_exe(&app).ok_or("vgmstream-cli.exe introuvable dans l'archive")?;
        Ok(release.tag_name)
    })
    .await
}

/// Lit un WAV PCM 16 bits (sortie de vgmstream) en stéréo.
pub fn read_wav(data: &[u8]) -> Result<Pcm, String> {
    if data.get(..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WAVE") {
        return Err("sortie de vgmstream illisible".into());
    }
    let (mut channels, mut rate, mut bits, mut body) = (0usize, 0u32, 0u16, None);
    let mut o = 12;
    while o + 8 <= data.len() {
        let size = u32::from_le_bytes(data[o + 4..o + 8].try_into().unwrap()) as usize;
        let chunk = &data[o + 8..(o + 8 + size).min(data.len())];
        match &data[o..o + 4] {
            b"fmt " if chunk.len() >= 16 => {
                channels = u16::from_le_bytes([chunk[2], chunk[3]]) as usize;
                rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
                bits = u16::from_le_bytes([chunk[14], chunk[15]]);
            }
            b"data" => body = Some(chunk),
            _ => {}
        }
        o += 8 + size + (size & 1);
    }
    let body = body.ok_or("sortie de vgmstream sans données")?;
    if bits != 16 || channels == 0 || rate == 0 {
        return Err("format de sortie de vgmstream inattendu".into());
    }
    let frames = body.len() / (2 * channels);
    let mut samples = Vec::with_capacity(frames * 2);
    for f in 0..frames {
        let s = |c: usize| i16::from_le_bytes([body[(f * channels + c) * 2], body[(f * channels + c) * 2 + 1]]);
        samples.push(s(0));
        samples.push(s(1.min(channels - 1)));
    }
    Ok(Pcm { sample_rate: rate, samples })
}

/// Décode avec vgmstream (fichiers du morceau écrits dans un dossier temporaire).
fn decode_vgmstream(app: &AppHandle, track: &TrackData, max_seconds: f32) -> Result<Vec<u8>, String> {
    let exe = vgmstream_exe(app).ok_or("VGMSTREAM_MISSING")?;
    let work = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("vgmstream-work").join(format!("{}", std::process::id()));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = (|| {
        for (name, data) in &track.files {
            fs::write(work.join(name), data).map_err(|e| e.to_string())?;
        }
        let out = work.join("out.wav");
        let mut cmd = std::process::Command::new(&exe);
        // -i : sans boucle ; le lanceur boucle lui-même l'extrait.
        cmd.args(["-i", "-o"]).arg(&out).arg(work.join(&track.main)).current_dir(exe.parent().unwrap_or(Path::new(".")));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let output = cmd.output().map_err(|e| format!("vgmstream n'a pas pu être lancé : {e}"))?;
        if !output.status.success() || !out.is_file() {
            let msg = String::from_utf8_lossy(&output.stderr).lines().chain(String::from_utf8_lossy(&output.stdout).lines()).find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
            return Err(format!("format audio non reconnu par vgmstream {msg}").trim().to_string());
        }
        let mut pcm = read_wav(&fs::read(&out).map_err(|e| e.to_string())?)?;
        nxm::clip(&mut pcm, max_seconds);
        Ok(pcm.to_wav())
    })();
    let _ = fs::remove_dir_all(&work);
    result
}

fn decode(app: &AppHandle, track: &TrackData, max_seconds: f32) -> Result<Vec<u8>, String> {
    match nxm::decode_native(track, max_seconds) {
        Some(r) => r,
        None => decode_vgmstream(app, track, max_seconds),
    }
}

/// Morceau décodé, gardé en cache par jeu et par morceau.
fn audio_for(app: &AppHandle, game: &Path, track_id: &str) -> Result<Vec<u8>, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("music");
    let key = music_key(game).ok_or("jeu introuvable")?;
    let hash = track_id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    let cached = dir.join(format!("{key}-{hash:016x}.nxaudio"));
    if let Ok(data) = fs::read(&cached) {
        return Ok(data);
    }
    let track = nxm::read_track(game, &keys(app)?, track_id)?;
    let audio = decode(app, &track, MUSIC_SECONDS)?;
    if fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(&cached, &audio);
    }
    Ok(audio)
}

// ---------------------------------------------------------------------------
// Commandes

/// Musique du lanceur pour un jeu Switch : morceau choisi, sinon deviné.
#[tauri::command]
pub async fn music_switch_theme(path: PathBuf, title_id: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::blocking(move || {
        let tid = parse_tid(&title_id)?;
        let id = match load_choices(&app).get(&format!("{tid:016X}")) {
            Some(id) => id.clone(),
            None => nxm::auto_track(&path, &keys(&app)?, tid)?,
        };
        Ok(tauri::ipc::Response::new(audio_for(&app, &path, &id)?))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackList {
    tracks: Vec<Track>,
    /// Morceau choisi par l'utilisateur.
    chosen: Option<String>,
    /// Morceau deviné par Kaleido.
    auto: Option<String>,
    /// Pourquoi rien n'a été deviné.
    auto_error: Option<String>,
    vgmstream: bool,
}

/// Morceaux d'un jeu Switch, pour le choix de la musique du lanceur.
#[tauri::command]
pub async fn music_switch_tracks(path: PathBuf, title_id: String, app: AppHandle) -> Result<TrackList, String> {
    crate::blocking(move || {
        let tid = parse_tid(&title_id)?;
        let keys = keys(&app)?;
        let tracks = nxm::list_tracks(&path, &keys)?;
        let auto = nxm::auto_track(&path, &keys, tid);
        Ok(TrackList {
            tracks,
            chosen: load_choices(&app).get(&format!("{tid:016X}")).cloned(),
            auto_error: auto.as_ref().err().cloned(),
            auto: auto.ok(),
            vgmstream: vgmstream_exe(&app).is_some(),
        })
    })
    .await
}

/// Extrait écoutable d'un morceau (erreur « VGMSTREAM_MISSING » si le décodeur manque).
#[tauri::command]
pub async fn music_switch_preview(path: PathBuf, track_id: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::blocking(move || Ok(tauri::ipc::Response::new(audio_for(&app, &path, &track_id)?))).await
}

/// Retient le morceau d'un jeu (`None` : revenir au choix automatique).
#[tauri::command]
pub fn music_switch_choose(title_id: String, track_id: Option<String>, app: AppHandle) -> Result<(), String> {
    let tid = parse_tid(&title_id)?;
    let mut c = load_choices(&app);
    match track_id {
        Some(id) => c.insert(format!("{tid:016X}"), id),
        None => c.remove(&format!("{tid:016X}")),
    };
    store_choices(&app, &c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_to_stereo() {
        let mut wav = Pcm { sample_rate: 44100, samples: vec![1, 2, 3, 4] }.to_wav();
        let pcm = read_wav(&wav).unwrap();
        assert_eq!((pcm.sample_rate, pcm.samples), (44100, vec![1, 2, 3, 4]));
        // Mono : le canal est dupliqué.
        wav[22] = 1;
        let mono = read_wav(&wav).unwrap();
        assert_eq!(mono.samples, vec![1, 1, 2, 2, 3, 3, 4, 4]);
    }
}
