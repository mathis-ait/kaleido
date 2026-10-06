//! Sprites des Pokémon, téléchargés à la demande puis gardés en cache dans le
//! dossier de données de l'application.
//!
//! - Icônes (pokesprite, MIT) : `sprite://localhost/<n° national>[-shiny].png`.
//! - Modèles (Pokémon Showdown) : `sprite://localhost/<style>/<n°>[-<forme>][-f][-shiny].<ext>`
//!   où `<style>` vaut `ani` (3D animés), `gen5ani` (pixel animés), `dex` (3D fixe),
//!   `gen5` (pixel fixe) ou `home` (rendus HOME). `-f` demande le sprite femelle
//!   quand il existe (Hippodocus, Pikachu…), sinon le sprite normal est servi.
//!
//! Le premier affichage télécharge l'image, les suivants la lisent sur le disque.
//! Un modèle absent du serveur (404) est mémorisé pour ne pas le redemander :
//! l'interface affiche alors l'icône à la place.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

const BASE_URL: &str = "https://cdn.jsdelivr.net/gh/msikma/pokesprite@master";
const SHOWDOWN_URL: &str = "https://play.pokemonshowdown.com/sprites";
const USER_AGENT: &str = concat!("Kaleido/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mathis-ait/kaleido)");
/// Taille maximale acceptée pour un fichier téléchargé.
const MAX_DOWNLOAD: u64 = 8 * 1024 * 1024;
/// Extensions des fichiers d'images gardés en cache (et marqueurs « absent »).
const CACHED_EXT: [&str; 3] = ["png", "gif", "missing"];

/// Table n° national → noms Showdown, une ligne par espèce : le nom de base puis,
/// s'il y a des formes, le suffixe de chaque forme dans l'ordre du jeu (`=` : forme de base).
/// Générée depuis `data/pokedex.json` de Pokémon Showdown (MIT), champ `formeOrder`.
const SHOWDOWN_IDS: &str = include_str!("showdown_ids.txt");

pub fn cache_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("sprites");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    fetch(url)?.ok_or_else(|| "introuvable (404)".to_string())
}

/// Télécharge `url` ; `Ok(None)` si le serveur répond 404 (image inexistante).
pub(crate) fn fetch(url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = match ureq::get(url).set("User-Agent", USER_AGENT).call() {
        Ok(r) => r,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut body = Vec::new();
    response.into_reader().take(MAX_DOWNLOAD).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(Some(body))
}

/// Type MIME d'après les premiers octets (PNG, GIF ou WebP).
fn content_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if data.starts_with(b"GIF8") {
        Some("image/gif")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Correspondance n° national → nom de fichier pokesprite (« mr-mime »…),
/// lue une seule fois depuis `data/pokemon.json` (mise en cache sur le disque).
fn slugs(cache: &Path) -> Result<&'static HashMap<u16, String>, String> {
    // Seul un succès est mémorisé : un échec (hors ligne) sera retenté à la demande suivante.
    static SLUGS: OnceLock<HashMap<u16, String>> = OnceLock::new();
    if let Some(map) = SLUGS.get() {
        return Ok(map);
    }
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
    let map = entries.into_iter().filter_map(|(idx, v)| Some((idx.parse().ok()?, v["slug"]["eng"].as_str()?.to_string()))).collect();
    Ok(SLUGS.get_or_init(|| map))
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

// --- Modèles (Pokémon Showdown)

/// Styles servis depuis Showdown, avec l'extension de leurs fichiers.
const MODEL_STYLES: [(&str, &str); 5] = [("ani", "gif"), ("gen5ani", "gif"), ("dex", "png"), ("gen5", "png"), ("home", "png")];

#[derive(Debug, PartialEq)]
struct ModelRequest {
    style: &'static str,
    ext: &'static str,
    id: u16,
    form: u16,
    female: bool,
    shiny: bool,
}

/// `"ani/479-2-shiny.gif"` → Motisma Lavage chromatique en 3D animé.
fn parse_model(path: &str) -> Option<ModelRequest> {
    let (style, name) = path.split_once('/')?;
    let &(style, ext) = MODEL_STYLES.iter().find(|(s, _)| *s == style)?;
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    let mut parts = stem.split('-');
    let id = parts.next()?.parse().ok().filter(|&n| n > 0)?;
    let mut req = ModelRequest { style, ext, id, form: 0, female: false, shiny: false };
    for part in parts {
        match part {
            "shiny" => req.shiny = true,
            "f" => req.female = true,
            n => req.form = n.parse().ok()?,
        }
    }
    Some(req)
}

/// Nom Showdown d'une espèce et d'une forme (`charizard-megax`, `rotom-wash`…).
/// Une forme inconnue retombe sur l'espèce de base.
fn showdown_slug(id: u16, form: u16) -> Option<String> {
    let line = SHOWDOWN_IDS.lines().nth(usize::from(id).checked_sub(1)?)?;
    let mut tokens = line.split(' ');
    let base = tokens.next().filter(|b| !b.is_empty())?;
    Some(match tokens.nth(usize::from(form)) {
        Some(suffix) if suffix != "=" => format!("{base}-{suffix}"),
        _ => base.to_string(),
    })
}

/// Lit (ou télécharge) `<dir>/<slug>.<ext>` ; `Ok(None)` si le serveur ne l'a pas.
fn cached_model(cache: &Path, dir: &str, slug: &str, ext: &str) -> Result<Option<Vec<u8>>, String> {
    let folder = cache.join(dir);
    let file = folder.join(format!("{slug}.{ext}"));
    if let Ok(data) = fs::read(&file) {
        return Ok(Some(data));
    }
    let missing = folder.join(format!("{slug}.missing"));
    if missing.exists() {
        return Ok(None);
    }
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    match fetch(&format!("{SHOWDOWN_URL}/{dir}/{slug}.{ext}"))? {
        Some(data) if content_type(&data).is_some() => {
            fs::write(&file, &data).map_err(|e| e.to_string())?;
            Ok(Some(data))
        }
        Some(_) => Err("réponse inattendue (pas une image)".into()),
        None => {
            // Marqueur vide : « Vider le cache » permet de redemander plus tard.
            let _ = fs::write(&missing, b"");
            Ok(None)
        }
    }
}

fn load_model(cache: &Path, path: &str) -> Result<Vec<u8>, String> {
    let req = parse_model(path).ok_or("nom de sprite invalide")?;
    let slug = showdown_slug(req.id, req.form).ok_or("Pokémon inconnu de Showdown")?;
    // Sans version animée (certains Pokémon Gen 6-7 en pixel animé), l'image fixe du même style.
    let still = match req.style {
        "ani" => Some(("dex", "png")),
        "gen5ani" => Some(("gen5", "png")),
        _ => None,
    };
    // Le sprite femelle n'existe que pour les espèces avec une différence visible ;
    // une forme sans modèle retombe sur la forme de base.
    let mut names = Vec::new();
    if req.female {
        names.push(format!("{slug}-f"));
    }
    names.push(slug.clone());
    if let Some(base) = showdown_slug(req.id, 0).filter(|b| *b != slug) {
        names.push(base);
    }
    for (style, ext) in std::iter::once((req.style, req.ext)).chain(still) {
        let dir = if req.shiny { format!("{style}-shiny") } else { style.to_string() };
        for name in &names {
            if let Some(data) = cached_model(cache, &dir, name, ext)? {
                return Ok(data);
            }
        }
    }
    Err("pas de modèle pour ce Pokémon".into())
}

/// Gestionnaire du protocole `sprite://`.
pub fn handle<R: Runtime>(app: &AppHandle<R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    // `convertFileSrc` encode le « / » des modèles (`ani%2F25.gif`).
    let name = request.uri().path().trim_start_matches('/').replace("%2F", "/").replace("%2f", "/");
    let result = cache_dir(app).and_then(|cache| if name.contains('/') { load_model(&cache, &name) } else { load(&cache, &name) });
    match result {
        Ok(data) => Response::builder()
            .header("Content-Type", content_type(&data).unwrap_or("application/octet-stream"))
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(data)
            .unwrap(),
        Err(e) => Response::builder().status(StatusCode::NOT_FOUND).header("Content-Type", "text/plain; charset=utf-8").body(e.into_bytes()).unwrap(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub files: usize,
    pub bytes: u64,
    pub path: String,
}

/// Images en cache : dossier racine (icônes) et sous-dossiers des modèles.
fn cached_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(cached_files(&path));
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| CACHED_EXT.contains(&e)) {
            out.push(path);
        }
    }
    out
}

pub fn cache_info<R: Runtime>(app: &AppHandle<R>) -> Result<CacheInfo, String> {
    let dir = cache_dir(app)?;
    let (mut files, mut bytes) = (0, 0);
    for path in cached_files(&dir) {
        if path.extension().is_some_and(|e| e != "missing") {
            files += 1;
            bytes += fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(CacheInfo { files, bytes, path: dir.display().to_string() })
}

pub fn clear_cache<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = cache_dir(app)?;
    for path in cached_files(&dir) {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
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

    #[test]
    fn model_names() {
        let req = parse_model("ani/479-2-shiny.gif").unwrap();
        assert_eq!((req.style, req.ext, req.id, req.form, req.female, req.shiny), ("ani", "gif", 479, 2, false, true));
        let req = parse_model("home/450-f.png").unwrap();
        assert_eq!((req.style, req.id, req.form, req.female), ("home", 450, 0, true));
        assert!(parse_model("ani/0.gif").is_none());
        assert!(parse_model("../25.gif").is_none());
        assert!(parse_model("ani/../25.gif").is_none());
        assert!(parse_model("autre/25.gif").is_none());
        assert!(parse_model("ani/25-x.gif").is_none());
    }

    #[test]
    fn showdown_table() {
        assert_eq!(SHOWDOWN_IDS.lines().count(), 1025);
        assert_eq!(showdown_slug(1, 0).as_deref(), Some("bulbasaur"));
        assert_eq!(showdown_slug(6, 1).as_deref(), Some("charizard-megax"));
        assert_eq!(showdown_slug(6, 9).as_deref(), Some("charizard"));
        assert_eq!(showdown_slug(29, 0).as_deref(), Some("nidoranf"));
        assert_eq!(showdown_slug(122, 0).as_deref(), Some("mrmime"));
        assert_eq!(showdown_slug(201, 1).as_deref(), Some("unown-b"));
        assert_eq!(showdown_slug(479, 2).as_deref(), Some("rotom-wash"));
        assert_eq!(showdown_slug(666, 6).as_deref(), Some("vivillon"));
        assert_eq!(showdown_slug(666, 0).as_deref(), Some("vivillon-icysnow"));
        assert_eq!(showdown_slug(678, 1).as_deref(), Some("meowstic-f"));
        assert_eq!(showdown_slug(718, 1).as_deref(), Some("zygarde-10"));
        assert_eq!(showdown_slug(772, 0).as_deref(), Some("typenull"));
        assert_eq!(showdown_slug(1025, 0).as_deref(), Some("pecharunt"));
        assert_eq!(showdown_slug(0, 0), None);
        assert_eq!(showdown_slug(1026, 0), None);
    }

    /// Téléchargements réels depuis Showdown : `cargo test -p kaleido-app -- --ignored`.
    #[test]
    #[ignore = "réseau"]
    fn showdown_downloads() {
        let cache = std::env::temp_dir().join(format!("kaleido-sprites-test-{}", std::process::id()));
        let gif = |path: &str| content_type(&load_model(&cache, path).unwrap());
        assert_eq!(gif("ani/6-1.gif"), Some("image/gif")); // Méga-Dracaufeu X
        assert_eq!(gif("ani/479-2-shiny.gif"), Some("image/gif")); // Motisma Lavage chromatique
        assert_eq!(gif("ani/450-f.gif"), Some("image/gif")); // Hippodocus femelle
        assert_eq!(gif("ani/6-f.gif"), Some("image/gif")); // pas de différence : sprite normal
        assert_eq!(gif("gen5ani/655.gif"), Some("image/png")); // pas de version animée : image fixe
        assert_eq!(gif("dex/201-1.png"), Some("image/png")); // Zarbi B
        assert_eq!(gif("home/1025.png"), Some("image/png"));
        assert!(cache.join("ani").join("charizard-f.missing").exists());
        // Le marqueur évite un nouvel aller-retour réseau.
        assert_eq!(gif("ani/6-f.gif"), Some("image/gif"));
        let _ = fs::remove_dir_all(&cache);
    }

    #[test]
    fn image_types() {
        assert_eq!(content_type(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(content_type(b"GIF89a"), Some("image/gif"));
        assert_eq!(content_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(content_type(b"<html>"), None);
    }
}
