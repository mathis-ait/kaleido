//! ScreenScraper (screenscraper.fr) : scans des vraies étiquettes de cartouches pour le
//! mode Cartouche du lanceur.
//!
//! L'API exige des identifiants développeur (demandés sur le forum de ScreenScraper) :
//! compilés dans Kaleido (`SCREENSCRAPER_DEV_ID` / `SCREENSCRAPER_DEV_PASSWORD` au moment
//! de la compilation), ou saisis dans les Réglages. Le compte ScreenScraper de
//! l'utilisateur est facultatif ; il donne un meilleur quota. Sans identifiants
//! développeur, rien n'est demandé et le lanceur garde ses étiquettes neutres.
//!
//! La ROM est reconnue par son empreinte MD5 et sa taille. Le résultat (image, ou
//! absence) est gardé dans `<données>/labels-ss` : une seule requête par ROM.

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::USER_AGENT;

const API: &str = "https://api.screenscraper.fr/api2";
const SOFTNAME: &str = concat!("Kaleido-", env!("CARGO_PKG_VERSION"));

/// Régions préférées : la française, l'américaine, puis l'européenne et la mondiale.
const REGIONS: [&str; 4] = ["fr", "us", "eu", "wor"];
/// Types de média : la texture de l'étiquette seule, sinon la photo du support.
const MEDIA: [(&str, u8); 2] = [("support-texture", KIND_TEXTURE), ("support-2D", KIND_PHOTO)];
const KIND_TEXTURE: u8 = 0;
const KIND_PHOTO: u8 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dev_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Identifiants développeur disponibles (compilés ou saisis).
    dev: bool,
    /// Identifiants développeur compilés dans cette version de Kaleido.
    built_in: bool,
    user: Option<String>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("screenscraper.json"))
}

fn load(app: &AppHandle) -> Config {
    config_path(app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

/// Identifiants développeur : ceux saisis dans les Réglages, sinon ceux compilés.
fn dev_credentials(config: &Config) -> Option<(String, String)> {
    let saved = config.dev_id.clone().zip(config.dev_password.clone()).filter(|(id, pw)| !id.is_empty() && !pw.is_empty());
    saved.or_else(|| Some((option_env!("SCREENSCRAPER_DEV_ID")?.to_string(), option_env!("SCREENSCRAPER_DEV_PASSWORD")?.to_string())))
}

fn status_of(config: &Config) -> Status {
    Status {
        dev: dev_credentials(config).is_some(),
        built_in: option_env!("SCREENSCRAPER_DEV_ID").is_some(),
        user: config.user.clone().filter(|u| !u.is_empty()),
    }
}

#[tauri::command]
pub fn screenscraper_status(app: AppHandle) -> Status {
    status_of(&load(&app))
}

/// Enregistre les identifiants (un champ vide efface la valeur enregistrée).
#[tauri::command]
pub fn screenscraper_save(dev_id: Option<String>, dev_password: Option<String>, user: Option<String>, password: Option<String>, app: AppHandle) -> Result<Status, String> {
    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut config = load(&app);
    if let Some(id) = dev_id {
        config.dev_id = clean(Some(id));
    }
    if let Some(pw) = dev_password {
        config.dev_password = clean(Some(pw));
    }
    if let Some(u) = user {
        config.user = clean(Some(u));
    }
    if let Some(pw) = password {
        config.password = clean(Some(pw));
    }
    let path = config_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // Nouveaux identifiants : les jeux introuvables jusque-là sont redemandés.
    if let Ok(dir) = cache_dir(&app) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.path().extension().is_some_and(|e| e == "missing") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(status_of(&config))
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("labels-ss");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Identifiant ScreenScraper du système.
fn system_id(platform: &str) -> Option<u32> {
    Some(match platform {
        "nds" => 15,
        "3ds" => 17,
        "gba" => 12,
        "gb" => 9,
        "gbc" => 10,
        "switch" => 225,
        _ => return None,
    })
}

/// Clé de cache d'un fichier : chemin, taille et date de modification (pas besoin de relire la ROM).
fn file_key(path: &Path) -> Result<(String, u64), String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    let mut h = Md5::new();
    h.update(path.to_string_lossy().to_lowercase().as_bytes());
    h.update(meta.len().to_le_bytes());
    h.update(mtime.to_le_bytes());
    Ok((hex(&h.finalize()), meta.len()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn md5_file(path: &Path) -> Result<String, String> {
    let mut reader = BufReader::with_capacity(1 << 20, File::open(path).map_err(|e| e.to_string())?);
    let mut h = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

#[derive(Deserialize)]
struct Media {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    url: String,
}

/// Meilleur scan d'étiquette parmi les médias du jeu : type puis région préférés.
fn pick_media(medias: &[Media]) -> Option<(&Media, u8)> {
    for (kind, code) in MEDIA {
        for region in REGIONS {
            if let Some(m) = medias.iter().find(|m| m.kind == kind && m.region.as_deref() == Some(region) && !m.url.is_empty()) {
                return Some((m, code));
            }
        }
    }
    None
}

enum Lookup {
    Found(u8, Vec<u8>),
    /// Jeu inconnu de ScreenScraper, ou sans scan d'étiquette.
    Missing,
}

fn lookup(config: &Config, (dev_id, dev_pw): &(String, String), rom: &Path, size: u64, system: u32) -> Result<Lookup, String> {
    let md5 = md5_file(rom)?;
    let name = rom.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut request = ureq::get(&format!("{API}/jeuInfos.php"))
        .set("User-Agent", USER_AGENT)
        .query("devid", dev_id)
        .query("devpassword", dev_pw)
        .query("softname", SOFTNAME)
        .query("output", "json")
        .query("systemeid", &system.to_string())
        .query("romtype", "rom")
        .query("md5", &md5)
        .query("romtaille", &size.to_string())
        .query("romnom", &name);
    if let (Some(user), Some(pw)) = (&config.user, &config.password) {
        request = request.query("ssid", user).query("sspassword", pw);
    }
    let body = match request.call() {
        Ok(r) => r.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Ok(Lookup::Missing),
        Err(ureq::Error::Status(code, r)) => {
            let text = r.into_string().unwrap_or_default();
            return Err(format!("ScreenScraper a refusé la demande ({code}) : {}", text.trim()));
        }
        Err(e) => return Err(e.to_string()),
    };
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|_| format!("réponse inattendue de ScreenScraper : {}", body.chars().take(120).collect::<String>()))?;
    let medias: Vec<Media> = serde_json::from_value(json["response"]["jeu"]["medias"].clone()).unwrap_or_default();
    let Some((media, kind)) = pick_media(&medias) else { return Ok(Lookup::Missing) };
    let image = crate::sprites::fetch(&media.url)?.ok_or("scan introuvable")?;
    let is_image = image.starts_with(b"\x89PNG") || image.starts_with(&[0xFF, 0xD8, 0xFF]);
    Ok(if is_image { Lookup::Found(kind, image) } else { Lookup::Missing })
}

/// Scan de la vraie étiquette d'une ROM : premier octet = 0 (texture de l'étiquette) ou
/// 1 (photo du support), puis l'image. Corps vide sans identifiants ou sans scan.
#[tauri::command]
pub async fn label_scan(path: PathBuf, platform: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::blocking(move || {
        let empty = || Ok(tauri::ipc::Response::new(Vec::new()));
        let Some(system) = system_id(&platform) else { return empty() };
        if !path.is_file() {
            return empty();
        }
        let dir = cache_dir(&app)?;
        let (key, size) = file_key(&path)?;
        let hit = dir.join(format!("{key}.img"));
        let missing = dir.join(format!("{key}.missing"));
        if let Ok(data) = fs::read(&hit) {
            return Ok(tauri::ipc::Response::new(data));
        }
        if missing.exists() {
            return empty();
        }
        let config = load(&app);
        let Some(dev) = dev_credentials(&config) else { return empty() };
        match lookup(&config, &dev, &path, size, system)? {
            Lookup::Found(kind, image) => {
                let mut data = Vec::with_capacity(image.len() + 1);
                data.push(kind);
                data.extend(image);
                let _ = fs::write(&hit, &data);
                Ok(tauri::ipc::Response::new(data))
            }
            Lookup::Missing => {
                let _ = fs::write(&missing, b"");
                empty()
            }
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(kind: &str, region: &str) -> Media {
        Media { kind: kind.into(), region: Some(region.into()), url: format!("https://x/{kind}/{region}") }
    }

    #[test]
    fn prefers_french_label_texture() {
        let all = [media("support-2D", "fr"), media("support-texture", "eu"), media("support-texture", "fr"), media("box-2D", "fr")];
        let (m, kind) = pick_media(&all).unwrap();
        assert_eq!((m.kind.as_str(), m.region.as_deref(), kind), ("support-texture", Some("fr"), KIND_TEXTURE));
    }

    #[test]
    fn falls_back_to_photo_then_nothing() {
        let photo = [media("support-2D", "us"), media("support-2D", "jp")];
        assert_eq!(pick_media(&photo).map(|(m, k)| (m.region.clone(), k)), Some((Some("us".into()), KIND_PHOTO)));
        // Une étiquette japonaise ou espagnole ne remplace pas l'étiquette neutre.
        assert!(pick_media(&[media("support-texture", "jp"), media("support-2D", "sp")]).is_none());
    }

    #[test]
    fn systems() {
        assert_eq!(system_id("nds"), Some(15));
        assert_eq!(system_id("3ds"), Some(17));
        assert_eq!(system_id("psx"), None);
    }
}
