//! Client de l'API publique de GameBanana (apiv11) : fiches de mods, fichiers, liste des
//! mods d'un jeu (tri, recherche, catégories). Réponses gardées en cache quelques heures.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::mods::cached;

/// Jeux GameBanana des Pokémon pris en charge, par title ID de base (Switch, 3DS).
pub const GAMES: &[(u64, u32)] = &[
    (0x010003F003A34000, 6773),  // Let's Go Pikachu
    (0x0100187003A36000, 6773),  // Let's Go Évoli
    (0x0100ABF008968000, 7676),  // Épée
    (0x01008DB008C2C000, 7676),  // Bouclier
    (0x0100000011D90000, 14783), // Diamant Étincelant
    (0x010018E011D92000, 14783), // Perle Scintillante
    (0x01001F5010DFA000, 15053), // Légendes Arceus
    (0x0100A3D008C5C000, 17220), // Écarlate
    (0x01008F6008C5E000, 17220), // Violet
    (0x0100F43008C44000, 23582), // Légendes Z-A
    (0x0004000000055D00, 7496),  // X
    (0x0004000000055E00, 7496),  // Y
    (0x000400000011C400, 5877),  // Rubis Oméga
    (0x000400000011C500, 5877),  // Saphir Alpha
    (0x0004000000164800, 5897),  // Soleil
    (0x0004000000175E00, 5897),  // Lune
    (0x00040000001B5000, 6192),  // Ultra-Soleil
    (0x00040000001B5100, 6192),  // Ultra-Lune
];

pub fn game_of(tid: u64) -> Option<u32> {
    GAMES.iter().find(|(t, _)| *t == tid).map(|(_, g)| *g)
}

const API: &str = "https://gamebanana.com/apiv11";

fn api<T: for<'de> Deserialize<'de>>(app: &AppHandle, key: &str, url: &str, max_age: Duration) -> Result<T, String> {
    let data = cached(app, &format!("gb-{key}.json"), url, max_age, true)?;
    serde_json::from_slice(&data).map_err(|e| format!("réponse de GameBanana illisible : {e}"))
}

#[derive(Deserialize, Default, Clone)]
#[allow(non_snake_case)]
struct RawImage {
    #[serde(default)]
    _sBaseUrl: String,
    #[serde(default)]
    _sFile: String,
    #[serde(default)]
    _sFile530: Option<String>,
    #[serde(default)]
    _sFile220: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[allow(non_snake_case)]
struct RawMedia {
    #[serde(default)]
    _aImages: Vec<RawImage>,
}

#[derive(Deserialize, Default, Clone)]
#[allow(non_snake_case)]
struct RawNamed {
    #[serde(default)]
    _sName: String,
}

#[derive(Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct RawFile {
    pub _idRow: u64,
    pub _sFile: String,
    #[serde(default)]
    pub _nFilesize: u64,
    #[serde(default)]
    pub _tsDateAdded: i64,
    pub _sDownloadUrl: String,
    #[serde(default)]
    pub _sMd5Checksum: String,
    #[serde(default)]
    pub _sDescription: String,
    #[serde(default)]
    pub _nDownloadCount: u64,
    #[serde(default)]
    pub _sAvResult: String,
}

#[derive(Deserialize, Clone)]
#[allow(non_snake_case)]
struct RawProfile {
    _idRow: u32,
    _sName: String,
    #[serde(default)]
    _sText: String,
    #[serde(default)]
    _aFiles: Vec<RawFile>,
    #[serde(default)]
    _aPreviewMedia: RawMedia,
    #[serde(default)]
    _sVersion: String,
    #[serde(default)]
    _bIsObsolete: bool,
    #[serde(default)]
    _nLikeCount: u64,
    #[serde(default)]
    _nViewCount: u64,
    #[serde(default)]
    _nDownloadCount: u64,
    #[serde(default)]
    _tsDateModified: i64,
    #[serde(default)]
    _aSubmitter: RawNamed,
    #[serde(default)]
    _aCategory: RawNamed,
}

/// Image d'aperçu : (petite, grande).
fn images(media: &RawMedia) -> Vec<(String, String)> {
    media
        ._aImages
        .iter()
        .filter(|i| !i._sFile.is_empty())
        .map(|i| {
            let small = i._sFile220.clone().or(i._sFile530.clone()).unwrap_or(i._sFile.clone());
            let large = i._sFile530.clone().unwrap_or(i._sFile.clone());
            (format!("{}/{small}", i._sBaseUrl), format!("{}/{large}", i._sBaseUrl))
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GbFileInfo {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub date: i64,
    pub description: String,
    pub downloads: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GbProfile {
    pub id: u32,
    pub name: String,
    /// Description en texte brut (balises retirées), tronquée.
    pub text: String,
    pub files: Vec<GbFileInfo>,
    pub images: Vec<String>,
    pub thumbnail: Option<String>,
    pub version: String,
    pub obsolete: bool,
    pub likes: u64,
    pub views: u64,
    pub downloads: u64,
    pub updated: i64,
    pub author: String,
    pub category: String,
}

/// Fiche d'un mod, avec ses fichiers bruts (pour le téléchargement).
pub fn profile_raw(app: &AppHandle, id: u32) -> Result<(GbProfile, Vec<RawFile>), String> {
    let p: RawProfile = api(app, &format!("mod-{id}"), &format!("{API}/Mod/{id}/ProfilePage"), Duration::from_secs(6 * 3600))?;
    let imgs = images(&p._aPreviewMedia);
    let files = p._aFiles.iter().filter(|f| f._sAvResult != "infected").cloned().collect::<Vec<_>>();
    let profile = GbProfile {
        id: p._idRow,
        name: p._sName.clone(),
        text: html_to_text(&p._sText, 1200),
        files: files
            .iter()
            .map(|f| GbFileInfo { id: f._idRow, name: f._sFile.clone(), size: f._nFilesize, date: f._tsDateAdded, description: f._sDescription.clone(), downloads: f._nDownloadCount })
            .collect(),
        thumbnail: imgs.first().map(|i| i.0.clone()),
        images: imgs.into_iter().map(|i| i.1).take(8).collect(),
        version: p._sVersion,
        obsolete: p._bIsObsolete,
        likes: p._nLikeCount,
        views: p._nViewCount,
        downloads: p._nDownloadCount,
        updated: p._tsDateModified,
        author: p._aSubmitter._sName,
        category: p._aCategory._sName,
    };
    Ok((profile, files))
}

pub fn profile(app: &AppHandle, id: u32) -> Result<GbProfile, String> {
    profile_raw(app, id).map(|p| p.0)
}

/// Texte brut d'une description HTML de GameBanana.
pub fn html_to_text(html: &str, max: usize) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if t == "br" || (matches!(t.as_str(), "p" | "li" | "div" | "h1" | "h2" | "h3" | "h4" | "tr") && !out.ends_with('\n')) {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    let decoded = out.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&#039;", "'");
    let mut lines: Vec<&str> = decoded.lines().map(str::trim).collect();
    lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
    let text = lines.join("\n").trim().to_string();
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

// ---------------------------------------------------------------------------
// Liste des mods d'un jeu

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawItem {
    _idRow: u32,
    _sName: String,
    #[serde(default)]
    _sModelName: String,
    #[serde(default)]
    _aPreviewMedia: RawMedia,
    #[serde(default)]
    _aSubmitter: RawNamed,
    #[serde(default)]
    _aRootCategory: RawNamed,
    #[serde(default)]
    _nLikeCount: u64,
    #[serde(default)]
    _nViewCount: u64,
    #[serde(default)]
    _tsDateAdded: i64,
    #[serde(default)]
    _tsDateModified: i64,
    #[serde(default)]
    _bHasFiles: bool,
    #[serde(default)]
    _bHasContentRatings: bool,
    #[serde(default)]
    _bIsObsolete: bool,
    #[serde(default)]
    _sVersion: String,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawMeta {
    #[serde(default)]
    _nRecordCount: u64,
    #[serde(default)]
    _bIsComplete: bool,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawPage {
    _aMetadata: RawMeta,
    #[serde(default)]
    _aRecords: Vec<RawItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GbItem {
    pub id: u32,
    pub name: String,
    pub thumbnail: Option<String>,
    pub author: String,
    pub category: String,
    pub likes: u64,
    pub views: u64,
    pub added: i64,
    pub updated: i64,
    pub has_files: bool,
    pub obsolete: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GbPage {
    pub total: u64,
    pub complete: bool,
    pub items: Vec<GbItem>,
    /// Mods écartés car marqués « contenu sensible » par GameBanana.
    pub hidden: u32,
}

pub const PER_PAGE: u32 = 30;

/// Mods d'un jeu GameBanana. `sort` : « popular », « liked », « new », « updated ».
pub fn browse(app: &AppHandle, game: u32, query: &str, sort: &str, category: Option<u32>, page: u32) -> Result<GbPage, String> {
    let page = page.max(1);
    let q = query.trim();
    let (key, url) = if q.is_empty() {
        let s = match sort {
            "liked" => "Generic_MostLiked",
            "new" => "Generic_Newest",
            "updated" => "Generic_LatestUpdated",
            _ => "Generic_MostDownloaded",
        };
        let cat = category.map(|c| format!("&_aFilters%5BGeneric_Category%5D={c}")).unwrap_or_default();
        (
            format!("list-{game}-{s}-{}-{page}", category.unwrap_or(0)),
            format!("{API}/Mod/Index?_nPage={page}&_nPerpage={PER_PAGE}&_aFilters%5BGeneric_Game%5D={game}{cat}&_sSort={s}"),
        )
    } else {
        let enc = crate::mods::encode(q);
        (
            format!("search-{game}-{:016x}-{page}", crate::mods::fxhash(&q.to_lowercase())),
            format!("{API}/Util/Search/Results?_sModelName=Mod&_sOrder=best_match&_idGameRow={game}&_sSearchString={enc}&_nPage={page}&_nPerpage={PER_PAGE}"),
        )
    };
    let raw: RawPage = api(app, &key, &url, Duration::from_secs(3600))?;
    let mut hidden = 0;
    let items = raw
        ._aRecords
        .into_iter()
        .filter(|r| r._sModelName.is_empty() || r._sModelName == "Mod")
        .filter(|r| {
            let keep = !r._bHasContentRatings;
            if !keep {
                hidden += 1;
            }
            keep
        })
        .map(|r| GbItem {
            id: r._idRow,
            name: r._sName,
            thumbnail: images(&r._aPreviewMedia).first().map(|i| i.0.clone()),
            author: r._aSubmitter._sName,
            category: r._aRootCategory._sName,
            likes: r._nLikeCount,
            views: r._nViewCount,
            added: r._tsDateAdded,
            updated: r._tsDateModified,
            has_files: r._bHasFiles,
            obsolete: r._bIsObsolete,
            version: r._sVersion,
        })
        .collect();
    Ok(GbPage { total: raw._aMetadata._nRecordCount, complete: raw._aMetadata._bIsComplete, items, hidden })
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct RawCategory {
    _idRow: u32,
    _sName: String,
    #[serde(default)]
    _nItemCount: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GbCategory {
    pub id: u32,
    pub name: String,
    pub count: u64,
}

pub fn categories(app: &AppHandle, game: u32) -> Result<Vec<GbCategory>, String> {
    let raw: Vec<RawCategory> = api(app, &format!("cats-{game}"), &format!("{API}/Mod/Categories?_idGameRow={game}&_sSort=a_to_z&_bShowEmpty=false"), Duration::from_secs(24 * 3600))?;
    Ok(raw.into_iter().filter(|c| c._nItemCount > 0).map(|c| GbCategory { id: c._idRow, name: c._sName, count: c._nItemCount }).collect())
}

/// Nom français d'une catégorie GameBanana.
pub fn category_fr(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "skins" => "Apparences".into(),
        "game settings" | "game files" | "game mechanics" => "Jeu et réglages".into(),
        "guis" => "Interface".into(),
        "sounds" => "Sons et musiques".into(),
        "other/misc" | "miscellaneous" => "Divers".into(),
        "maps" => "Décors".into(),
        "textures" => "Textures".into(),
        "reshade" | "reshades" => "ReShade".into(),
        "rom hacks" => "Romhacks".into(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_cleanup() {
        let t = html_to_text("Scales every pokemon.<br><br>To install place the &quot;romfs&quot; folder<ul><li>Diamond</li><li>Pearl&nbsp;!</li></ul>", 500);
        assert_eq!(t, "Scales every pokemon.\n\nTo install place the \"romfs\" folder\nDiamond\nPearl !");
        assert!(html_to_text(&"a".repeat(50), 10).ends_with('…'));
    }

    #[test]
    fn games_cover_switch_titles() {
        assert_eq!(game_of(0x0100000011D90000), Some(14783));
        assert_eq!(game_of(0x00040000001B5100), Some(6192));
        assert_eq!(game_of(0x1), None);
    }
}
