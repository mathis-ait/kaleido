//! Catalogue des mods choisis à la main pour chaque jeu (`data/mods-catalog/*.json`).
//!
//! Le catalogue est intégré au programme et rafraîchi une fois par jour depuis le dépôt
//! GitHub de Kaleido : de nouveaux mods peuvent ainsi apparaître sans nouvelle version.

use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;
use tauri::AppHandle;

use crate::mods::cached;

const EMBEDDED: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/mods_catalog.rs"));

const REMOTE: &str = "https://raw.githubusercontent.com/mathis-ait/kaleido/main/app/src-tauri/data/mods-catalog";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFile {
    /// Title ID (Switch, 3DS) ou code de jeu DS à 3 lettres.
    pub titles: Vec<String>,
    /// Jeu GameBanana et notes de recherche : informatifs (l'explorateur prend `gamebanana::GAMES`).
    #[serde(default)]
    #[allow(dead_code)]
    pub gamebanana: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub mods: Vec<CatalogMod>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMod {
    pub id: String,
    /// « gamebanana » (téléchargé par Kaleido) ou « manual » (l'utilisateur télécharge).
    pub source: String,
    #[serde(default)]
    pub gb: Option<u32>,
    /// Début du nom du fichier GameBanana à prendre.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "other")]
    pub category: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default)]
    pub author: Option<String>,
    /// Restreint le mod à certains titres du fichier.
    #[serde(default)]
    pub titles: Option<Vec<String>>,
    #[serde(default)]
    pub popularity: Option<u64>,
    /// 3DS : « layeredfs » (défaut) ou « textures » ; DS : « patch ».
    #[serde(default)]
    pub kind: Option<String>,
    /// Variante à prendre quand l'archive en contient plusieurs (début du chemin).
    #[serde(default)]
    pub variant: Option<String>,
}

fn other() -> String {
    "other".into()
}

pub const CATEGORIES: &[&str] = &["fps", "graphics", "textures", "style", "qol", "gameplay", "romhack", "audio", "ui", "cheats", "resolution", "display", "other"];

fn parse(text: &str) -> Option<CatalogFile> {
    let mut file: CatalogFile = serde_json::from_str(text).ok()?;
    for m in &mut file.mods {
        if !CATEGORIES.contains(&m.category.as_str()) {
            m.category = "other".into();
        }
    }
    Some(file)
}

fn embedded() -> &'static Vec<(String, CatalogFile)> {
    static CELL: OnceLock<Vec<(String, CatalogFile)>> = OnceLock::new();
    CELL.get_or_init(|| EMBEDDED.iter().filter_map(|(name, text)| parse(text).map(|f| (name.to_string(), f))).collect())
}

fn matches(file: &CatalogFile, key: &str) -> bool {
    file.titles.iter().any(|t| t.eq_ignore_ascii_case(key))
}

/// Catalogue d'un jeu (`key` = title ID en hexadécimal sur 16 chiffres, ou code DS à 3 lettres).
pub fn for_game(app: &AppHandle, key: &str) -> Option<CatalogFile> {
    let (name, local) = embedded().iter().find(|(_, f)| matches(f, key))?;
    let url = format!("{REMOTE}/{name}.json");
    let remote = cached(app, &format!("catalog-{name}.json"), &url, Duration::from_secs(24 * 3600), false)
        .ok()
        .and_then(|d| parse(&String::from_utf8_lossy(&d)))
        .filter(|f| matches(f, key) && !f.mods.is_empty());
    // Le catalogue distant l'emporte pour les mods qu'il connaît ; ceux qui ne sont que dans la
    // version intégrée (cache téléchargé avant leur ajout) restent proposés.
    let mut file = match remote {
        Some(mut r) => {
            for m in &local.mods {
                if !r.mods.iter().any(|x| x.id == m.id) {
                    r.mods.push(m.clone());
                }
            }
            r
        }
        None => local.clone(),
    };
    file.mods.retain(|m| m.titles.as_ref().is_none_or(|t| t.iter().any(|t| t.eq_ignore_ascii_case(key))));
    Some(file)
}

/// Mod du catalogue d'un jeu, par identifiant.
pub fn find(app: &AppHandle, key: &str, id: &str) -> Option<CatalogMod> {
    for_game(app, key)?.mods.into_iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid() {
        for (name, text) in EMBEDDED {
            let file: Result<CatalogFile, _> = serde_json::from_str(text);
            let file = file.unwrap_or_else(|e| panic!("{name}.json : {e}"));
            assert!(!file.titles.is_empty(), "{name}");
            let mut ids = std::collections::HashSet::new();
            for m in &file.mods {
                assert!(ids.insert(m.id.clone()), "{name} : {} en double", m.id);
                assert!(CATEGORIES.contains(&m.category.as_str()), "{name} : catégorie {}", m.category);
                match m.source.as_str() {
                    "gamebanana" => assert!(m.gb.is_some(), "{name} : {} sans id GameBanana", m.id),
                    "manual" => assert!(m.page.is_some(), "{name} : {} sans page", m.id),
                    s => panic!("{name} : source {s}"),
                }
                for r in &m.requires {
                    assert!(file.mods.iter().any(|o| &o.id == r), "{name} : {} exige {r} absent du catalogue", m.id);
                }
            }
        }
    }
}
