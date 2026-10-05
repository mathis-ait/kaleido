//! Équipes d'exemple de Smogon, via l'API publique de crob.at (sans clé) :
//! `GET /api/samples/{format}` (liste) et `GET /api/team/{slug}` (texte Showdown).
//!
//! Les réponses sont gardées en cache dans le dossier de données (`teams/`) et
//! rafraîchies au plus une fois par semaine ; hors ligne, la copie en cache sert.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const API: &str = "https://crob.at/api";
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
const MAX_DOWNLOAD: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub slug: String,
    pub name: String,
    pub author: Option<String>,
    pub views: Option<u64>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub slug: String,
    pub name: String,
    pub author: Option<String>,
    pub format: String,
    /// Équipe au format d'export Showdown.
    pub paste: String,
    pub url: String,
    pub source_url: Option<String>,
}

#[derive(Deserialize)]
struct RawSample {
    slug: String,
    name: String,
    author: Option<String>,
    views: Option<u64>,
    source_url: Option<String>,
}

#[derive(Deserialize)]
struct RawTeam {
    slug: String,
    name: String,
    author: Option<String>,
    source_url: Option<String>,
    teams: Vec<RawPaste>,
}

#[derive(Deserialize)]
struct RawPaste {
    format: String,
    paste: String,
}

/// Identifiant sûr pour un nom de fichier et une URL (lettres, chiffres, « - », « _ »).
fn checked(id: &str) -> Result<&str, String> {
    let ok = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(id).ok_or_else(|| format!("identifiant invalide : {id}"))
}

fn cache_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("teams");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{name}.json")))
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url).timeout(Duration::from_secs(20)).call().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    response.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

fn is_fresh(path: &PathBuf) -> bool {
    fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|m| SystemTime::now().duration_since(m).ok()).is_some_and(|age| age < MAX_AGE)
}

/// Réponse JSON de l'API : cache récent, sinon réseau, sinon cache ancien.
fn fetch<T: for<'de> Deserialize<'de>>(app: &AppHandle, cache: &str, url: &str, refresh: bool) -> Result<T, String> {
    let file = cache_file(app, cache)?;
    if !refresh && is_fresh(&file) {
        if let Some(v) = fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            return Ok(v);
        }
    }
    match download(url).and_then(|b| serde_json::from_slice::<T>(&b).map(|v| (v, b)).map_err(|e| e.to_string())) {
        Ok((v, bytes)) => {
            let tmp = file.with_extension("json.part");
            if fs::write(&tmp, &bytes).and_then(|_| fs::rename(&tmp, &file)).is_err() {
                let _ = fs::remove_file(&tmp);
            }
            Ok(v)
        }
        Err(e) => {
            fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).ok_or_else(|| format!("équipes indisponibles (hors ligne ?) : {e}"))
        }
    }
}

/// Équipes d'exemple d'un format Showdown (« gen5ou », « gen4vgc2010 »…).
#[tauri::command]
pub async fn teams_list(format: String, refresh: Option<bool>, app: AppHandle) -> Result<Vec<TeamSummary>, String> {
    crate::blocking(move || {
        let format = checked(&format)?;
        let raw: Vec<RawSample> = fetch(&app, &format!("samples-{format}"), &format!("{API}/samples/{format}"), refresh.unwrap_or(false))?;
        Ok(raw.into_iter().map(|r| TeamSummary { slug: r.slug, name: r.name, author: r.author, views: r.views, source_url: r.source_url }).collect())
    })
    .await
}

/// Une équipe complète (première équipe d'un paste qui en contient plusieurs).
#[tauri::command]
pub async fn teams_get(slug: String, app: AppHandle) -> Result<Team, String> {
    crate::blocking(move || {
        let slug = checked(&slug)?;
        let raw: RawTeam = fetch(&app, &format!("team-{slug}"), &format!("{API}/team/{slug}"), false)?;
        let first = raw.teams.into_iter().next().ok_or("équipe vide")?;
        Ok(Team {
            url: format!("https://crob.at/{}", raw.slug),
            slug: raw.slug,
            name: raw.name,
            author: raw.author,
            format: first.format,
            paste: first.paste,
            source_url: raw.source_url,
        })
    })
    .await
}
