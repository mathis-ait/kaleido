// Pas de console en plus de la fenêtre en mode release sur Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use kaleido_core::Detection;

/// Identifie un fichier ou un dossier déposé par l'utilisateur.
#[tauri::command]
async fn detect_file(path: PathBuf) -> Result<Detection, String> {
    tauri::async_runtime::spawn_blocking(move || kaleido_core::detect_path(&path).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![detect_file])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer Kaleido");
}
