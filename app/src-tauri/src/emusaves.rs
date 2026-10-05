//! Détection automatique des sauvegardes rangées par les émulateurs :
//! carte SD émulée d'Azahar / Citra / Lime3DS (3DS), dossiers de melonDS et
//! `Battery/` de DeSmuME (DS), et dossiers des ROMs de la bibliothèque.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::AppHandle;

use crate::play::{self, EmulatorId, Env};

/// Sauvegarde trouvée chez un émulateur.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FoundSave {
    pub path: String,
    /// Émulateur qui l'utilise (« Azahar », « melonDS »…).
    pub emulator: String,
    /// Jeu déduit du dossier (3DS : title ID), sinon `None`.
    pub game: Option<String>,
}

/// Jeux 3DS pris en charge, d'après la partie basse du title ID (identique dans toutes les régions).
fn ctr_game(low: &str) -> Option<&'static str> {
    Some(match low.to_ascii_lowercase().as_str() {
        "00055d00" => "Pokémon X",
        "00055e00" => "Pokémon Y",
        "0011c400" => "Pokémon Rubis Oméga",
        "0011c500" => "Pokémon Saphir Alpha",
        "00164800" => "Pokémon Soleil",
        "00175e00" => "Pokémon Lune",
        "001b5000" => "Pokémon Ultra-Soleil",
        "001b5100" => "Pokémon Ultra-Lune",
        _ => return None,
    })
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// `<user>/sdmc/Nintendo 3DS/<id0>/<id1>/title/00040000/<jeu>/data/00000001/main`.
pub fn scan_ctr_user_dir(user: &Path, emulator: &str) -> Vec<FoundSave> {
    let mut out = Vec::new();
    for id0 in subdirs(&user.join("sdmc").join("Nintendo 3DS")) {
        for id1 in subdirs(&id0) {
            for title in subdirs(&id1.join("title").join("00040000")) {
                let low = title.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let Some(game) = ctr_game(&low) else { continue };
                let main = title.join("data").join("00000001").join("main");
                if main.is_file() {
                    out.push(FoundSave { path: main.display().to_string(), emulator: emulator.into(), game: Some(game.into()) });
                }
            }
        }
    }
    out
}

/// Fichiers `.sav` / `.dsv` d'un dossier (sans sous-dossiers).
pub fn scan_nds_dir(dir: &Path, emulator: &str) -> Vec<FoundSave> {
    let mut v: Vec<FoundSave> = fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("sav") || x.eq_ignore_ascii_case("dsv")))
                .map(|p| FoundSave { path: p.display().to_string(), emulator: emulator.into(), game: None })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

/// Toutes les sauvegardes trouvées chez les émulateurs (même ceux dont l'exécutable
/// est introuvable : leurs dossiers par défaut sont quand même explorés), plus les
/// `.sav` / `.dsv` des dossiers de ROMs donnés.
#[tauri::command]
pub async fn emulator_saves(rom_dirs: Vec<String>, app: AppHandle) -> Result<Vec<FoundSave>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let config = play::load_config(&app);
        let env = Env::system(&config.search_dirs);
        let mut out: Vec<FoundSave> = Vec::new();
        for id in EmulatorId::ALL {
            let r = play::resolve(id, &config, &env);
            if id.is_ctr() {
                if let Some(user) = r.ctr_user_dir() {
                    out.extend(scan_ctr_user_dir(&user, id.name()));
                }
            } else if let Some(dir) = r.nds_save_dir() {
                out.extend(scan_nds_dir(&dir, id.name()));
            }
        }
        for dir in rom_dirs {
            out.extend(scan_nds_dir(Path::new(&dir), "dossier des ROMs"));
        }
        // Un même fichier vu deux fois (Citra et Azahar partagent parfois un dossier) : une seule entrée.
        let mut seen = std::collections::HashSet::new();
        out.retain(|s| seen.insert(s.path.to_lowercase()));
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_azahar_saves_by_title_id() {
        let tmp = std::env::temp_dir().join(format!("kaleido-emusaves-{}", std::process::id()));
        let zeros = "0".repeat(32);
        let base = tmp.join("sdmc").join("Nintendo 3DS").join(&zeros).join(&zeros).join("title").join("00040000");
        let oras = base.join("0011c400").join("data").join("00000001");
        fs::create_dir_all(&oras).unwrap();
        fs::write(oras.join("main"), b"x").unwrap();
        // Jeu inconnu et dossier sans sauvegarde : ignorés.
        fs::create_dir_all(base.join("00030700").join("data").join("00000001")).unwrap();
        fs::write(base.join("00030700").join("data").join("00000001").join("main"), b"x").unwrap();
        fs::create_dir_all(base.join("00164800")).unwrap();
        let found = scan_ctr_user_dir(&tmp, "Azahar");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].game.as_deref(), Some("Pokémon Rubis Oméga"));
        assert!(found[0].path.ends_with("main"));
        fs::remove_dir_all(&tmp).ok();
    }
}
