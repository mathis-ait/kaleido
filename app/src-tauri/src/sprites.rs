//! Sprites des Pokémon, téléchargés à la demande depuis pokesprite (MIT) puis
//! gardés en cache dans le dossier de données de l'application.
//!
//! L'interface demande `sprite://localhost/<n° national>[-shiny].png` ; le
//! premier affichage télécharge l'image, les suivants la lisent sur le disque.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

const BASE_URL: &str = "https://cdn.jsdelivr.net/gh/msikma/pokesprite@master";
/// Taille maximale acceptée pour un fichier téléchargé.
const MAX_DOWNLOAD: u64 = 4 * 1024 * 1024;

pub fn cache_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("sprites");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url).call().map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    response.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

/// Correspondance n° national → nom de fichier pokesprite (« mr-mime »…),
/// lue une seule fois depuis `data/pokemon.json` (mise en cache sur le disque).
fn slugs(cache: &Path) -> Result<&'static HashMap<u16, String>, String> {
    static SLUGS: OnceLock<Result<HashMap<u16, String>, String>> = OnceLock::new();
    SLUGS
        .get_or_init(|| {
            let path = cache.join("pokemon.json");
            let json = match fs::read(&path) {
                Ok(data) => data,
                Err(_) => {
                    let data = download(&format!("{BASE_URL}/data/pokemon.json"))?;
                    fs::write(&path, &data).map_err(|e| e.to_string())?;
                    data
                }
            };
            let entries: HashMap<String, serde_json::Value> = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
            Ok(entries
                .into_iter()
                .filter_map(|(idx, v)| Some((idx.parse().ok()?, v["slug"]["eng"].as_str()?.to_string())))
                .collect())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// `"025-shiny.png"` → (25, true).
fn parse_name(name: &str) -> Option<(u16, bool)> {
    let stem = name.strip_suffix(".png")?;
    let (id, shiny) = match stem.strip_suffix("-shiny") {
        Some(id) => (id, true),
        None => (stem, false),
    };
    Some((id.parse().ok().filter(|&n| n > 0)?, shiny))
}

fn load(cache: &Path, name: &str) -> Result<Vec<u8>, String> {
    let (id, shiny) = parse_name(name).ok_or("nom de sprite invalide")?;
    let file = cache.join(format!("{id:04}{}.png", if shiny { "-shiny" } else { "" }));
    if let Ok(data) = fs::read(&file) {
        return Ok(data);
    }
    let slug = slugs(cache)?.get(&id).ok_or("Pokémon inconnu de pokesprite")?;
    let variant = if shiny { "shiny" } else { "regular" };
    let data = download(&format!("{BASE_URL}/pokemon-gen8/{variant}/{slug}.png"))?;
    if !data.starts_with(b"\x89PNG") {
        return Err("réponse inattendue (pas une image PNG)".into());
    }
    fs::write(&file, &data).map_err(|e| e.to_string())?;
    Ok(data)
}

/// Gestionnaire du protocole `sprite://`.
pub fn handle<R: Runtime>(app: &AppHandle<R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let name = request.uri().path().trim_start_matches('/').to_string();
    let result = cache_dir(app).and_then(|cache| load(&cache, &name));
    match result {
        Ok(png) => Response::builder()
            .header("Content-Type", "image/png")
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(png)
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(e.into_bytes())
            .unwrap(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub files: usize,
    pub bytes: u64,
    pub path: String,
}

pub fn cache_info<R: Runtime>(app: &AppHandle<R>) -> Result<CacheInfo, String> {
    let dir = cache_dir(app)?;
    let (mut files, mut bytes) = (0, 0);
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if entry.path().extension().is_some_and(|e| e == "png") {
            files += 1;
            bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(CacheInfo { files, bytes, path: dir.display().to_string() })
}

pub fn clear_cache<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = cache_dir(app)?;
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if entry.path().extension().is_some_and(|e| e == "png") {
            fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_names() {
        assert_eq!(parse_name("025.png"), Some((25, false)));
        assert_eq!(parse_name("6-shiny.png"), Some((6, true)));
        assert_eq!(parse_name("0.png"), None);
        assert_eq!(parse_name("../x.png"), None);
    }
}
