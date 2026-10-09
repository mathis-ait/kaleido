//! Fiches publiques de Nexus Mods (API GraphQL v2, sans clé) : image d'aperçu et nombre de
//! téléchargements des mods du catalogue à télécharger soi-même. Cache d'un jour.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::library::USER_AGENT;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusMod {
    pub mod_id: u32,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Deserialize)]
struct Nodes {
    nodes: Vec<NexusMod>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    legacy_mods_by_domain: Nodes,
}

#[derive(Deserialize)]
struct Reply {
    data: Data,
}

/// « nexus:pokemonbdsp:38:lumi » → (« pokemonbdsp », 38).
pub fn parse_id(id: &str) -> Option<(String, u32)> {
    let mut parts = id.strip_prefix("nexus:")?.split(':');
    let domain = parts.next()?.to_string();
    let n = parts.next()?.parse().ok()?;
    Some((domain, n))
}

/// Fiches de plusieurs mods Nexus, par (domaine, numéro).
pub fn mods(app: &AppHandle, ids: &[(String, u32)]) -> BTreeMap<(String, u32), NexusMod> {
    let mut wanted: Vec<(String, u32)> = ids.to_vec();
    wanted.sort();
    wanted.dedup();
    if wanted.is_empty() {
        return BTreeMap::new();
    }
    let key = crate::mods::fxhash(&format!("v2{wanted:?}"));
    let cache = app.path().app_cache_dir().ok().map(|d| d.join("mods").join(format!("nexus-{key:016x}.json")));
    let fresh = cache.as_ref().and_then(|c| fs::metadata(c).ok()).and_then(|m| m.modified().ok()).and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|a| a < Duration::from_secs(24 * 3600));
    let body = if fresh { cache.as_ref().and_then(|c| fs::read(c).ok()) } else { None }.or_else(|| {
        let list = wanted.iter().map(|(d, n)| format!("{{gameDomain: \\\"{d}\\\", modId: {n}}}")).collect::<Vec<_>>().join(", ");
        let query = format!("{{\"query\":\"{{ legacyModsByDomain(ids: [{list}]) {{ nodes {{ modId thumbnailUrl downloads version }} }} }}\"}}");
        let fetched = ureq::post("https://api.nexusmods.com/v2/graphql")
            .set("User-Agent", USER_AGENT)
            .set("Content-Type", "application/json")
            .timeout(Duration::from_secs(15))
            .send_string(&query)
            .ok()
            .and_then(|r| {
                let mut v = Vec::new();
                r.into_reader().take(4 << 20).read_to_end(&mut v).ok().map(|_| v)
            });
        if let (Some(data), Some(c)) = (&fetched, &cache) {
            let _ = fs::create_dir_all(c.parent().unwrap());
            let _ = fs::write(c, data);
        }
        fetched.or_else(|| cache.as_ref().and_then(|c| fs::read(c).ok()))
    });
    let Some(reply) = body.and_then(|b| serde_json::from_slice::<Reply>(&b).ok()) else { return BTreeMap::new() };
    // Les numéros de mods sont propres à chaque jeu : on les rattache aux domaines demandés.
    let mut out = BTreeMap::new();
    for m in reply.data.legacy_mods_by_domain.nodes {
        for (d, n) in wanted.iter().filter(|(_, n)| *n == m.mod_id) {
            out.insert((d.clone(), *n), m.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids() {
        assert_eq!(super::parse_id("nexus:pokemonbdsp:38:lumi"), Some(("pokemonbdsp".into(), 38)));
        assert_eq!(super::parse_id("gb:12"), None);
    }
}
