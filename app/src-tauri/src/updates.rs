//! Vérification des mises à jour : dernière release publiée sur GitHub.

use std::time::Duration;

use serde::{Deserialize, Serialize};

const LATEST_RELEASE: &str = "https://api.github.com/repos/mathis-ait/kaleido/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/mathis-ait/kaleido/releases";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: &'static str,
    /// `None` : aucune release publiée.
    latest: Option<String>,
    newer: bool,
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
    let response = ureq::get(LATEST_RELEASE)
        .set("User-Agent", concat!("Kaleido/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(10))
        .call();
    let release: Release = match response {
        Ok(r) => serde_json::from_reader(r.into_reader()).map_err(|e| format!("réponse de GitHub illisible : {e}"))?,
        // Pas encore de release publiée.
        Err(ureq::Error::Status(404, _)) => {
            return Ok(UpdateInfo { current, latest: None, newer: false, url: RELEASES_PAGE.into(), name: None, notes: None })
        }
        Err(e) => return Err(format!("impossible de joindre GitHub : {e}")),
    };
    Ok(UpdateInfo {
        current,
        newer: is_newer(&release.tag_name, current),
        latest: Some(release.tag_name.trim_start_matches(['v', 'V']).to_string()),
        url: release.html_url.unwrap_or_else(|| RELEASES_PAGE.into()),
        name: release.name.filter(|n| !n.trim().is_empty()),
        notes: release.body.filter(|b| !b.trim().is_empty()),
    })
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
