//! Cartes mémoire par jeu (`data/memory_maps.json`) : quelles lectures en direct sont activées
//! pour un code jeu DS ou un Title ID 3DS, et ce qui a été vérifié dans un émulateur.
//!
//! Un jeu absent du fichier n'est pas lu en mémoire : le compagnon reste sur la sauvegarde.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Method {
    pub method: String,
    #[serde(default)]
    pub verified: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBlock {
    #[serde(default)]
    pub verified: Vec<String>,
}

/// Mot de la RAM qui vaut `value` pendant un combat (adresse DS, 0x02xxxxxx), pour ces codes jeu.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BattleFlag {
    pub codes: Vec<String>,
    pub address: String,
    pub value: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMap {
    pub game: String,
    pub console: String,
    #[serde(default)]
    pub codes: Vec<String>,
    #[serde(default)]
    pub title_ids: Vec<String>,
    pub party: Method,
    #[serde(default)]
    pub save_block: SaveBlock,
    pub battle: Option<Method>,
    #[serde(default)]
    pub battle_flags: Vec<BattleFlag>,
    /// Overlay ARM9 du moteur de combat : présent en RAM seulement pendant un combat.
    #[serde(default)]
    pub battle_overlay: Option<u32>,
    /// Module CRO du moteur de combat (3DS) : chargé seulement pendant un combat.
    #[serde(default)]
    pub battle_module: Option<String>,
    /// Module CRO de la carte (3DS), chargé au même emplacement que celui du combat.
    #[serde(default)]
    pub field_module: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
struct File {
    games: Vec<GameMap>,
}

fn all() -> &'static [GameMap] {
    static MAPS: OnceLock<Vec<GameMap>> = OnceLock::new();
    MAPS.get_or_init(|| serde_json::from_str::<File>(include_str!("../../data/memory_maps.json")).map(|f| f.games).unwrap_or_default())
}

impl GameMap {
    /// Lecture vérifiée en jeu pour ce code jeu / Title ID (sinon : « non vérifiée »).
    pub fn party_verified(&self, id: &str) -> bool {
        self.party.verified.iter().any(|v| v.eq_ignore_ascii_case(id))
    }

    /// Indicateur « en combat » pour ce code jeu : (adresse DS, valeur en combat).
    pub fn battle_flag(&self, code: &str) -> Option<(u32, u32)> {
        let f = self.battle_flags.iter().find(|f| f.codes.iter().any(|c| c.eq_ignore_ascii_case(code)))?;
        let addr = u32::from_str_radix(f.address.trim_start_matches("0x"), 16).ok()?;
        Some((addr, f.value))
    }

    pub fn battle_scan(&self) -> bool {
        self.battle.as_ref().is_some_and(|b| b.method == "partyHeader")
    }
}

/// Début du code de l'overlay de combat lu dans la ROM : (adresse DS, octets). En RAM à cette
/// adresse pendant un combat seulement ; ailleurs, la carte ou un menu l'ont remplacé.
pub fn battle_code(rom: &std::path::Path, overlay: u32) -> Option<(String, u32, Vec<u8>)> {
    let rom = kaleido_formats::nds::NdsRom::open(rom).ok()?;
    let o = rom.overlays().iter().find(|o| o.id == overlay)?.clone();
    let code = rom.overlay(overlay).ok()?;
    let n = code.len().min(256);
    (n >= 64).then(|| (rom.header().game_code.clone(), o.ram_address, code[..n].to_vec()))
}

/// Début (empreintes SHA-256 de l'en-tête) d'un module CRO de la ROM 3DS : unique au fichier,
/// présent en mémoire tant que le module est chargé.
pub fn cro_head(rom: &std::path::Path, module: &str) -> Option<Vec<u8>> {
    let fs = kaleido_formats::romfs::RomFsSource::open(rom).ok()?;
    let want = format!("/{module}.cro");
    let path = fs.files().iter().map(|f| f.path.clone()).find(|p| p.eq_ignore_ascii_case(&want) || p.eq_ignore_ascii_case(&want[1..]))?;
    fs.read_range(&path, 0, 32).ok().filter(|b| b.len() == 32 && b.iter().any(|&x| x != 0))
}

/// Carte d'un jeu DS d'après son code complet (`IPGF`).
pub fn for_ds_code(code: &str) -> Option<&'static GameMap> {
    all().iter().find(|g| g.console == "ds" && g.codes.iter().any(|c| c.eq_ignore_ascii_case(code)))
}

/// Carte d'un jeu 3DS d'après son Title ID.
pub fn for_title_id(id: u64) -> Option<&'static GameMap> {
    let hex = format!("{id:016X}");
    all().iter().find(|g| g.console == "3ds" && g.title_ids.iter().any(|t| t.eq_ignore_ascii_case(&hex)))
}

/// Carte d'un jeu 3DS d'après le nom court du jeu (`XY`, `ORAS`, `SM`, `USUM`), quand le Title ID
/// n'est pas lisible dans la mémoire de l'émulateur.
pub fn for_game(game: &str) -> Option<&'static GameMap> {
    all().iter().find(|g| g.game == game)
}

pub fn games() -> &'static [GameMap] {
    all()
}
