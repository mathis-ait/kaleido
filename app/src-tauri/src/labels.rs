//! Étiquettes des cartouches du lanceur (mode Cartouche).
//!
//! L'étiquette est composée côté interface (canvas) ; ce module ne fait que la garder
//! en PNG dans `<données>/labels` pour ne pas la recalculer à chaque ouverture, et
//! copier l'image choisie par l'utilisateur, qui prime sur l'étiquette générée.

use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

fn labels_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("labels");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Nom de fichier sûr : empreinte, title ID ou clé calculée par l'interface.
fn file_name(key: &str) -> Result<String, String> {
    let ok = !key.is_empty() && key.len() <= 120 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') && !key.starts_with('.');
    if ok {
        Ok(format!("{key}.png"))
    } else {
        Err(format!("clé d'étiquette invalide : {key}"))
    }
}

/// Lit (corps vide) ou écrit (corps = PNG) l'étiquette `x-label-key` du cache.
/// En lecture, renvoie un corps vide si l'étiquette n'est pas en cache.
#[tauri::command]
pub fn label_cache(request: tauri::ipc::Request<'_>, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    let key = request.headers().get("x-label-key").and_then(|v| v.to_str().ok()).ok_or("clé d'étiquette manquante")?;
    let path = labels_dir(&app)?.join(file_name(key)?);
    match request.body() {
        tauri::ipc::InvokeBody::Raw(png) if !png.is_empty() => {
            fs::write(&path, png).map_err(|e| e.to_string())?;
            Ok(tauri::ipc::Response::new(Vec::new()))
        }
        _ => Ok(tauri::ipc::Response::new(fs::read(&path).unwrap_or_default())),
    }
}

/// Étiquette personnalisée d'un jeu : copie l'image choisie (PNG ou JPG) sous
/// `<empreinte>.png`, ou la supprime quand `source` est absent.
#[tauri::command]
pub fn label_custom(key: String, source: Option<PathBuf>, app: AppHandle) -> Result<(), String> {
    let path = labels_dir(&app)?.join(file_name(&key)?);
    match source {
        Some(source) => {
            let data = fs::read(&source).map_err(|e| e.to_string())?;
            let image = data.starts_with(b"\x89PNG") || data.starts_with(&[0xFF, 0xD8, 0xFF]);
            if !image {
                return Err("l'étiquette doit être une image PNG ou JPG".into());
            }
            fs::write(&path, data).map_err(|e| e.to_string())
        }
        None => match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn keys_stay_inside_the_folder() {
        assert_eq!(file_name("a1b2-c3_v1").unwrap(), "a1b2-c3_v1.png");
        assert!(file_name("../x").is_err());
        assert!(file_name(r"a\b").is_err());
        assert!(file_name("").is_err());
        assert!(file_name(".hidden").is_err());
    }
}
