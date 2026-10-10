//! Commandes de la Living Dex : lecture des Pokémon possédés (sauvegardes suivies et banque)
//! et stockage de l'état propre à la Living Dex (`livedex.json` : saisies manuelles, règles,
//! sources désactivées, succès), avec une copie de sécurité par jour (les 14 plus récentes).

use std::path::{Path, PathBuf};

use kaleido_core::livedex::{self, Specimen};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::bank::OpenBank;

/// Une sauvegarde lue (ou illisible) lors du scan.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedSource {
    /// Chemin de la sauvegarde, ou `"bank"` pour la banque Kaleido.
    pub path: String,
    pub file_name: String,
    pub game: String,
    pub version: Option<kaleido_core::save::SaveVersion>,
    pub generation: u8,
    pub trainer: String,
    /// Dernière modification du fichier (secondes depuis 1970).
    pub modified: Option<u64>,
    pub specimens: Vec<Specimen>,
    pub error: Option<String>,
}

fn modified(path: &Path) -> Option<u64> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

fn scan_save(path: &Path) -> ScannedSource {
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut out = ScannedSource {
        path: path.display().to_string(),
        file_name,
        game: String::new(),
        version: None,
        generation: 0,
        trainer: String::new(),
        modified: modified(path),
        specimens: Vec::new(),
        error: None,
    };
    let result = std::fs::read(path).map_err(|e| format!("lecture impossible : {e}")).and_then(|bytes| {
        let session = crate::saves::open_session(path, &bytes)?;
        let view = session.view().map_err(|e| e.to_string())?;
        out.game = view.game.to_string();
        out.version = Some(view.version);
        out.generation = view.generation;
        out.trainer = view.trainer.name.clone();
        livedex::from_session(&session).map_err(|e| e.to_string())
    });
    match result {
        Ok(list) => out.specimens = list,
        Err(e) => out.error = Some(e),
    }
    out
}

fn scan_bank(app: &AppHandle) -> ScannedSource {
    let mut out = ScannedSource {
        path: "bank".into(),
        file_name: String::new(),
        game: "Banque Kaleido".into(),
        version: None,
        generation: 0,
        trainer: String::new(),
        modified: None,
        specimens: Vec::new(),
        error: None,
    };
    let state = app.state::<OpenBank>();
    let result = state.with(app, |bank| {
        let mut list = Vec::new();
        for (b, info) in bank.boxes().into_iter().enumerate() {
            if info.count == 0 {
                continue;
            }
            for item in bank.box_view(b).map_err(|e| e.to_string())?.into_iter().flatten() {
                let Ok(detail) = bank.detail(item.slot, None) else { continue };
                if let Some(mut s) = livedex::specimen(&detail.pokemon, item.entry.generation, info.name.clone(), None) {
                    s.slot = None;
                    list.push(s);
                }
            }
        }
        Ok(list)
    });
    match result {
        Ok(list) => out.specimens = list,
        Err(e) => out.error = Some(e),
    }
    out
}

/// Lit les Pokémon de chaque sauvegarde donnée, puis ceux de la banque si `bank` est vrai.
#[tauri::command]
pub async fn livedex_scan(paths: Vec<String>, bank: bool, app: AppHandle) -> Result<Vec<ScannedSource>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out: Vec<ScannedSource> = paths.iter().map(|p| scan_save(Path::new(p))).collect();
        if bank {
            out.push(scan_bank(&app));
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// État enregistré de la Living Dex (`null` au premier lancement).
#[tauri::command]
pub fn livedex_load(app: AppHandle) -> Result<serde_json::Value, String> {
    let file = data_dir(&app)?.join("livedex.json");
    match std::fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("livedex.json illisible : {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::Value::Null),
        Err(e) => Err(e.to_string()),
    }
}

/// Écrit l'état (fichier temporaire puis renommage, pour ne jamais laisser un fichier à moitié
/// écrit), et garde une copie du jour dans `livedex-backups/`.
#[tauri::command]
pub fn livedex_store(data: serde_json::Value, today: String, app: AppHandle) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let bytes = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    write_atomic(&dir.join("livedex.json"), &bytes)?;
    backup(&dir.join("livedex-backups"), &today, &bytes)
}

fn write_atomic(file: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Copie du jour (`livedex-AAAA-MM-JJ.json`), puis suppression des plus anciennes au-delà de 14.
fn backup(dir: &Path, today: &str, bytes: &[u8]) -> Result<(), String> {
    const KEEP: usize = 14;
    if today.len() != 10 || !today.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return Err("date invalide".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(format!("livedex-{today}.json")), bytes)?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("livedex-") && n.ends_with(".json")))
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(KEEP);
    for old in &files[..excess] {
        std::fs::remove_file(old).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_fourteen_newest_backups() {
        let dir = std::env::temp_dir().join(format!("kaleido-livedex-{}", std::process::id()));
        for day in 1..=20 {
            backup(&dir, &format!("2026-09-{day:02}"), b"{}").unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), 14);
        assert_eq!(names[0], "livedex-2026-09-07.json");
        assert!(backup(&dir, "../../evil", b"{}").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
