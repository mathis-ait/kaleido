//! Mises à jour : dernière release publiée sur GitHub, installée depuis Kaleido.
//!
//! L'installation télécharge l'installeur NSIS de la release (`Kaleido_<version>_x64-setup.exe`)
//! puis le lance en mode passif (`/P` : barre de progression seulement), `/UPDATE` (mise à jour sur
//! place) et `/R` (relancer Kaleido à la fin), comme le module de mise à jour officiel de Tauri.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const LATEST_RELEASE: &str = "https://api.github.com/repos/mathis-ait/kaleido/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/mathis-ait/kaleido/releases";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: &'static str,
    /// `None` : aucune release publiée.
    latest: Option<String>,
    newer: bool,
    /// La release contient un installeur que Kaleido sait lancer lui-même.
    installable: bool,
    url: String,
    /// Titre et notes de la release.
    name: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: Option<String>,
    name: Option<String>,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn latest_release() -> Result<Option<Release>, String> {
    let response = ureq::get(LATEST_RELEASE)
        .set("User-Agent", concat!("Kaleido/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call();
    match response {
        Ok(r) => serde_json::from_reader(r.into_reader()).map(Some).map_err(|e| format!("réponse de GitHub illisible : {e}")),
        // Pas encore de release publiée.
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(format!("impossible de joindre GitHub : {e}")),
    }
}

/// Installeur Windows de la release.
fn setup_asset(release: &Release) -> Option<&Asset> {
    release.assets.iter().find(|a| a.name.to_ascii_lowercase().ends_with("-setup.exe"))
}

/// « v1.2.3-beta » → (1, 2, 3, false) ; une préversion passe avant la version finale.
fn parse_version(v: &str) -> Option<(u32, u32, u32, bool)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match v.split_once('-') {
        Some((c, _)) => (c, true),
        None => (v, false),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u32>().ok());
    let major = parts.next()??;
    let minor = parts.next().flatten().unwrap_or(0);
    let patch = parts.next().flatten().unwrap_or(0);
    Some((major, minor, patch, !pre))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[tauri::command]
pub fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION");
    let Some(release) = latest_release()? else {
        return Ok(UpdateInfo { current, latest: None, newer: false, installable: false, url: RELEASES_PAGE.into(), name: None, notes: None });
    };
    Ok(UpdateInfo {
        current,
        installable: cfg!(windows) && setup_asset(&release).is_some(),
        newer: is_newer(&release.tag_name, current),
        latest: Some(release.tag_name.trim_start_matches(['v', 'V']).to_string()),
        url: release.html_url.unwrap_or_else(|| RELEASES_PAGE.into()),
        name: release.name.filter(|n| !n.trim().is_empty()),
        notes: release.body.filter(|b| !b.trim().is_empty()),
    })
}

#[derive(Clone, Serialize)]
struct Progress {
    done: u64,
    total: u64,
}

/// Télécharge l'installeur de la dernière version, le lance puis ferme Kaleido (l'installeur
/// le relance une fois la mise à jour faite). Progression : évènement `update-progress`.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let installer = crate::blocking({
        let app = app.clone();
        move || {
            let release = latest_release()?.ok_or("aucune version publiée")?;
            if !is_newer(&release.tag_name, env!("CARGO_PKG_VERSION")) {
                return Err("Kaleido est déjà à jour.".into());
            }
            let asset = setup_asset(&release).ok_or("cette version n'a pas d'installeur Windows : télécharge-la depuis GitHub")?;
            let dir = std::env::temp_dir().join("kaleido-update");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Nom venu de GitHub : on n'en garde que le dernier composant.
            let name = std::path::Path::new(&asset.name).file_name().ok_or("nom d'installeur invalide")?;
            let dest = dir.join(name);
            let size = crate::library::download_to(&asset.browser_download_url, &dest, asset.size, |done, total| {
                let _ = app.emit("update-progress", Progress { done, total });
            })?;
            if asset.size > 0 && size != asset.size {
                let _ = std::fs::remove_file(&dest);
                return Err("téléchargement incomplet, réessaie".into());
            }
            Ok(dest)
        }
    })
    .await?;
    // L'installeur ne démarre qu'une fois Kaleido vraiment fermé : lancé tout de suite, il
    // tentait de le fermer pendant sa fermeture et s'arrêtait sur « Failed to kill Kaleido ».
    let quoted = installer.display().to_string().replace('\'', "''");
    let script = format!(
        "Wait-Process -Id {} -Timeout 60 -ErrorAction SilentlyContinue; Start-Process -FilePath '{quoted}' -ArgumentList '/P','/R','/UPDATE'",
        std::process::id()
    );
    let mut command = std::process::Command::new("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command.spawn().map_err(|e| format!("impossible de lancer l'installeur : {e}"))?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta", "0.1.0"));
        assert!(is_newer("0.1.0", "0.1.0-beta"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
