//! Cadeaux mystère des Gen 4 à 7 : base des distributions officielles, fichiers de
//! cartes (`.pcd`, `.pgt`, `.pgf`, `.wc6`, `.wc6full`, `.wc7`, `.wc7full`) et réception
//! d'un cadeau dans une sauvegarde.
//!
//! La base vient de PKHeX (kwsch/PKHeX, GPLv3, comme Kaleido) :
//! `PKHeX.Core/Resources/legality/mgdb/{wc4,pgf,wc6,wc6full,wc7,wc7full}.pkl`, chargés comme
//! `EncounterEvent.cs` (`MGDB_G4` … `MGDB_G7`) :
//!
//! - `wc4.pkl` : cartes PCD (0x358 octets) bout à bout ;
//! - `pgf.pkl` : cartes PGF (0xCC octets), puis un octet par carte en fin de fichier
//!   (version admise sur 4 bits, langue admise sur 4 bits) — `PGF.GetArray` ;
//! - `wc6.pkl` / `wc7.pkl` : cartes de 0x108 octets ; `wc6full.pkl` / `wc7full.pkl` : fichiers
//!   « full » de 0x310 octets (version admise en 0x00, langue en 0x1FF) — `WC6Full.GetArray`.
//!
//! Voir [`card`] pour la lecture des cartes et [`convert`] pour leur réception.

mod card;
pub mod convert;
#[cfg(test)]
mod tests;

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::dex;
use crate::save::{Gender, PkmDate, PkmError, Pokemon, SaveError, SaveVersion};

pub use convert::{add_to_save, convert, receive, AddOutcome, GiftTrainer};

static WC4: &[u8] = include_bytes!("../../data/pkhex/mgdb/wc4.pkl");
static PGF: &[u8] = include_bytes!("../../data/pkhex/mgdb/pgf.pkl");
static WC6: &[u8] = include_bytes!("../../data/pkhex/mgdb/wc6.pkl");
static WC6_FULL: &[u8] = include_bytes!("../../data/pkhex/mgdb/wc6full.pkl");
static WC7: &[u8] = include_bytes!("../../data/pkhex/mgdb/wc7.pkl");
static WC7_FULL: &[u8] = include_bytes!("../../data/pkhex/mgdb/wc7full.pkl");

#[derive(Debug, thiserror::Error)]
pub enum GiftError {
    #[error("fichier de cadeau non reconnu ({0} octets) : formats acceptés .pcd, .pgt, .pgf, .wc6, .wc6full, .wc7 et .wc7full")]
    UnknownFormat(usize),
    #[error("taille invalide pour un fichier {format} : {got} octets (attendu {expected})")]
    BadSize { format: &'static str, got: usize, expected: usize },
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Pkm(#[from] PkmError),
    #[error(transparent)]
    Save(#[from] SaveError),
}

/// Format d'un fichier de cadeau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GiftFormat {
    /// Carte Gen 4 complète (cadeau + carte affichée en jeu).
    Pcd,
    /// Cadeau Gen 4 seul (sans la carte).
    Pgt,
    Pgf,
    Wc6,
    Wc6Full,
    Wc7,
    Wc7Full,
}

impl GiftFormat {
    pub const ALL: [GiftFormat; 7] =
        [GiftFormat::Pcd, GiftFormat::Pgt, GiftFormat::Pgf, GiftFormat::Wc6, GiftFormat::Wc6Full, GiftFormat::Wc7, GiftFormat::Wc7Full];

    pub fn generation(self) -> u8 {
        match self {
            GiftFormat::Pcd | GiftFormat::Pgt => 4,
            GiftFormat::Pgf => 5,
            GiftFormat::Wc6 | GiftFormat::Wc6Full => 6,
            GiftFormat::Wc7 | GiftFormat::Wc7Full => 7,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            GiftFormat::Pcd => "pcd",
            GiftFormat::Pgt => "pgt",
            GiftFormat::Pgf => "pgf",
            GiftFormat::Wc6 => "wc6",
            GiftFormat::Wc6Full => "wc6full",
            GiftFormat::Wc7 => "wc7",
            GiftFormat::Wc7Full => "wc7full",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GiftFormat::Pcd => "PCD",
            GiftFormat::Pgt => "PGT",
            GiftFormat::Pgf => "PGF",
            GiftFormat::Wc6 | GiftFormat::Wc6Full => "WC6",
            GiftFormat::Wc7 | GiftFormat::Wc7Full => "WC7",
        }
    }

    pub fn size(self) -> usize {
        match self {
            GiftFormat::Pcd => card::PCD_SIZE,
            GiftFormat::Pgt => card::PGT_SIZE,
            GiftFormat::Pgf => card::PGF_SIZE,
            GiftFormat::Wc6 | GiftFormat::Wc7 => card::WC_SIZE,
            GiftFormat::Wc6Full | GiftFormat::Wc7Full => card::FULL_SIZE,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        GiftFormat::ALL.into_iter().find(|f| f.extension() == ext)
    }
}

/// Nature du cadeau.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiftKind {
    Pokemon,
    Egg,
    Item,
    /// Autre contenu (Pouvoir Pass, O-Aura, Points Combat, décoration…) : non reçu par Kaleido.
    Other,
}

impl GiftKind {
    pub fn label(self) -> &'static str {
        match self {
            GiftKind::Pokemon => "Pokémon",
            GiftKind::Egg => "Œuf",
            GiftKind::Item => "Objet",
            GiftKind::Other => "Autre",
        }
    }
}

/// Règle de chromatique de la carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShinyRule {
    /// Jamais chromatique (« verrou chromatique »).
    Never,
    /// Au hasard, comme un Pokémon sauvage.
    Random,
    /// Toujours chromatique (PID tiré au hasard puis rendu chromatique).
    Always,
    /// PID fixe chromatique « étoile ».
    AlwaysStar,
    /// PID fixe chromatique « carré » (XOR nul).
    AlwaysSquare,
}

impl ShinyRule {
    pub fn is_shiny(self) -> bool {
        matches!(self, ShinyRule::Always | ShinyRule::AlwaysStar | ShinyRule::AlwaysSquare)
    }

    pub fn label(self) -> &'static str {
        match self {
            ShinyRule::Never => "Jamais chromatique",
            ShinyRule::Random => "Chromatique possible",
            ShinyRule::Always => "Toujours chromatique",
            ShinyRule::AlwaysStar => "Chromatique (étoile)",
            ShinyRule::AlwaysSquare => "Chromatique (carré)",
        }
    }
}

/// Talent de la carte : fixe (0, 1 ou 2 = caché), 1 ou 2 au hasard, ou n'importe lequel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "slot", rename_all = "snake_case")]
pub enum AbilityRule {
    Fixed(u8),
    OneOrTwo,
    Any,
}

impl AbilityRule {
    /// `AbilityType` des cartes PGF / WC6 / WC7 : 0, 1, 2 fixes ; 3 = 1 ou 2 ; 4 = tous.
    fn from_type(t: u8) -> Self {
        match t {
            0..=2 => AbilityRule::Fixed(t),
            3 => AbilityRule::OneOrTwo,
            _ => AbilityRule::Any,
        }
    }
}

/// Objet donné par une carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiftItem {
    pub id: u16,
    pub count: u16,
}

/// Pokémon décrit par une carte (champs « joueur » à `None` : remplis à la réception).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftPokemon {
    pub species: u16,
    pub form: u8,
    /// Niveau (0 = au hasard, de 1 à 100).
    pub level: u8,
    pub met_level: u8,
    pub egg: bool,
    pub moves: [u16; 4],
    /// Attaques « de base » réapprenables (Gen 6/7).
    pub relearn: [u16; 4],
    pub held_item: u16,
    pub ball: u8,
    /// Dresseur d'origine imposé ; `None` = celui de la sauvegarde.
    pub ot_name: Option<String>,
    pub ot_gender: Option<Gender>,
    pub tid: Option<u16>,
    pub sid: Option<u16>,
    pub shiny: ShinyRule,
    /// PID fixe.
    pub pid: Option<u32>,
    pub nature: Option<u8>,
    pub gender: Option<Gender>,
    pub ability: AbilityRule,
    /// IV fixés (PV, Att, Déf, Atq Spé, Déf Spé, Vit) ; `None` = aléatoire.
    pub ivs: [Option<u8>; 6],
    /// Nombre d'IV aléatoires garantis à 31.
    pub perfect_ivs: u8,
    pub met_location: u16,
    pub egg_location: u16,
    /// Langue imposée (0 = celle de la sauvegarde).
    pub language: u8,
    pub nickname: Option<String>,
    /// Jeu d'origine imposé (0 = celui de la sauvegarde).
    pub origin_game: u8,
    pub fateful: bool,
    /// Rubans (clés `Ribbon…` de PKHeX).
    pub ribbons: Vec<&'static str>,
    pub evs: [u8; 6],
}

/// Rubans d'événement présents sur les cartes, dans l'ordre des tables de [`card`].
pub const RIBBONS: [&str; 15] = [
    "RibbonCountry",
    "RibbonNational",
    "RibbonEarth",
    "RibbonWorld",
    "RibbonClassic",
    "RibbonPremier",
    "RibbonEvent",
    "RibbonBirthday",
    "RibbonSpecial",
    "RibbonSouvenir",
    "RibbonWishing",
    "RibbonChampionBattle",
    "RibbonChampionRegional",
    "RibbonChampionNational",
    "RibbonChampionWorld",
];

/// Un cadeau mystère lu dans la base ou dans un fichier.
#[derive(Debug, Clone)]
pub struct Gift {
    pub format: GiftFormat,
    /// Fichier complet, tel qu'exporté.
    file: Vec<u8>,
    /// Versions admises (bits, 0 = toutes) et langue admise (0 = toutes), pour la base.
    pub restrict_version: u8,
    pub restrict_language: u8,
    pub card_id: u16,
    pub title: String,
    pub kind: GiftKind,
    /// Précision pour les cadeaux « autres » (Pouvoir Pass, O-Aura…).
    pub kind_detail: Option<String>,
    pub date: Option<PkmDate>,
    /// Versions (`GameVersion` de PKHeX) qui peuvent recevoir la carte.
    pub games: Vec<u8>,
    pub pokemon: Option<GiftPokemon>,
    pub items: Vec<GiftItem>,
    /// Gen 4 : Pokémon modèle déchiffré.
    template4: Option<Pokemon>,
}

impl Gift {
    /// Lit un fichier dont le format est connu.
    pub fn parse(format: GiftFormat, file: &[u8]) -> Result<Self, GiftError> {
        Self::parse_with(format, file, 0, 0)
    }

    fn parse_with(format: GiftFormat, file: &[u8], restrict_version: u8, restrict_language: u8) -> Result<Self, GiftError> {
        if file.len() != format.size() {
            return Err(GiftError::BadSize { format: format.label(), got: file.len(), expected: format.size() });
        }
        let (rv, rl) = match format {
            GiftFormat::Wc6Full | GiftFormat::Wc7Full => (file[0], file[0x1FF]),
            _ => (restrict_version, restrict_language),
        };
        let p = card::parse(format, file, rv)?;
        Ok(Gift {
            format,
            file: file.to_vec(),
            restrict_version: rv,
            restrict_language: rl,
            card_id: p.card_id,
            title: p.title,
            kind: p.kind,
            kind_detail: p.kind_detail,
            date: p.date,
            games: p.games,
            pokemon: p.pokemon,
            items: p.items,
            template4: p.template4,
        })
    }

    /// Lit un fichier de cadeau : format d'après l'extension si elle est donnée, sinon
    /// d'après la taille (WC6 et WC7 ont la même taille : date et espèce départagent).
    pub fn from_file(bytes: &[u8], extension: Option<&str>) -> Result<Self, GiftError> {
        if let Some(format) = extension.and_then(GiftFormat::from_extension) {
            return Self::parse(format, bytes);
        }
        let format = match bytes.len() {
            card::PCD_SIZE => GiftFormat::Pcd,
            card::PGT_SIZE => GiftFormat::Pgt,
            card::PGF_SIZE => GiftFormat::Pgf,
            card::WC_SIZE => guess_3ds(bytes, false),
            card::FULL_SIZE => guess_3ds(bytes, true),
            n => return Err(GiftError::UnknownFormat(n)),
        };
        Self::parse(format, bytes)
    }

    pub fn generation(&self) -> u8 {
        self.format.generation()
    }

    /// Octets du fichier (pour l'enregistrer tel quel).
    pub fn file(&self) -> &[u8] {
        &self.file
    }

    /// Carte elle-même (sans l'en-tête des fichiers « full »).
    pub(crate) fn card(&self) -> &[u8] {
        match self.format {
            GiftFormat::Wc6Full | GiftFormat::Wc7Full => &self.file[card::FULL_START..],
            _ => &self.file,
        }
    }

    pub fn is_pokemon(&self) -> bool {
        matches!(self.kind, GiftKind::Pokemon | GiftKind::Egg)
    }

    pub fn species(&self) -> u16 {
        self.pokemon.as_ref().map_or(0, |p| p.species)
    }

    /// Titre affiché : celui de la carte, sinon une description.
    pub fn display_title(&self) -> String {
        if !self.title.is_empty() {
            return self.title.clone();
        }
        match (&self.pokemon, self.items.first()) {
            (Some(p), _) => format!("{} (cadeau sans carte)", dex::species_name(p.species).unwrap_or("Pokémon")),
            (None, Some(i)) => dex::item_name(i.id).map_or_else(|| format!("Objet n°{}", i.id), str::to_string),
            _ => self.kind_detail.clone().unwrap_or_else(|| "Cadeau".into()),
        }
    }

    /// Peut être reçu par une sauvegarde de ce jeu ?
    pub fn receivable_by(&self, version: SaveVersion) -> bool {
        versions_of(version).iter().any(|v| self.games.contains(v))
    }
}

/// WC6 ou WC7 (même taille) ? Fichier « full » : l'en-tête des WC7 a `0x000A` en 0x04 (relevé
/// sur toute la base), celui des WC6 y commence le titre. Carte seule : jeu d'origine, date
/// (année sur 2 chiffres en WC7), contenu inexistant en Gen 6 (espèce, attaque, objet, Ball,
/// langue chinoise). Sans indice : WC6.
fn guess_3ds(file: &[u8], full: bool) -> GiftFormat {
    if full {
        return if card::u16le(file, 4) == 0x000A { GiftFormat::Wc7Full } else { GiftFormat::Wc6Full };
    }
    let card = file;
    let u16at = |at| card::u16le(card, at);
    let raw_date = card::u32le(card, 0x4C);
    let pokemon = card[0x51] == 0;
    let gen7 = match card[0x6C] {
        24..=29 if pokemon => false,
        30..=33 if pokemon => true,
        _ if raw_date != 0 => raw_date / 10000 < 100,
        _ if pokemon => {
            u16at(0x82) > 721
                || (0..4).any(|i| u16at(0x7A + 2 * i) > 621 || u16at(0xD8 + 2 * i) > 621)
                || u16at(0x78) > 717
                || card[0x76] > 25
                || matches!(card[0x85], 9 | 10)
        }
        // Objets : jusqu'à 6 objets en WC7 ; identifiants propres à Soleil/Lune.
        _ if card[0x51] == 1 => (0..6).any(|i| u16at(0x68 + 4 * i) > 717) || u16at(0x6C) != 0,
        _ => card[0x51] >= 2,
    };
    if gen7 {
        GiftFormat::Wc7
    } else {
        GiftFormat::Wc6
    }
}

/// Versions (`GameVersion` de PKHeX) d'une sauvegarde.
pub fn versions_of(version: SaveVersion) -> &'static [u8] {
    match version {
        SaveVersion::RubySapphire => &[1, 2],
        SaveVersion::Emerald => &[3],
        SaveVersion::FireRedLeafGreen => &[4, 5],
        SaveVersion::DiamondPearl => &[10, 11],
        SaveVersion::Platinum => &[12],
        SaveVersion::HeartGoldSoulSilver => &[7, 8],
        SaveVersion::BlackWhite => &[20, 21],
        SaveVersion::Black2White2 => &[22, 23],
        SaveVersion::XY => &[24, 25],
        SaveVersion::OmegaRubyAlphaSapphire => &[26, 27],
        SaveVersion::SunMoon => &[30, 31],
        SaveVersion::UltraSunUltraMoon => &[32, 33],
    }
}

// --- Base de données --------------------------------------------------------------------------

/// Toutes les cartes de la base, Gen 4 puis 5, 6 et 7 (ordre de PKHeX).
pub fn database() -> &'static [Gift] {
    static DB: LazyLock<Vec<Gift>> = LazyLock::new(load_database);
    &DB
}

fn load_database() -> Vec<Gift> {
    let mut out = Vec::new();
    let mut push = |format: GiftFormat, chunk: &[u8], rv: u8, rl: u8| {
        // Les cartes de la base sont valides ; une carte illisible serait simplement ignorée.
        if let Ok(g) = Gift::parse_with(format, chunk, rv, rl) {
            out.push(g);
        }
    };
    for c in WC4.as_chunks::<{ card::PCD_SIZE }>().0 {
        push(GiftFormat::Pcd, c, 0, 0);
    }
    // PGF.GetArray : n cartes de 0xCC octets puis n octets de restrictions.
    let n = PGF.len() / (card::PGF_SIZE + 1);
    for i in 0..n {
        let r = PGF[n * card::PGF_SIZE + i];
        push(GiftFormat::Pgf, &PGF[i * card::PGF_SIZE..(i + 1) * card::PGF_SIZE], r & 0x0F, r >> 4);
    }
    // WC6Full.GetArray / WC7Full.GetArray : les « full » d'abord, puis les cartes seules.
    for c in WC6_FULL.as_chunks::<{ card::FULL_SIZE }>().0 {
        push(GiftFormat::Wc6Full, c, 0, 0);
    }
    for c in WC6.as_chunks::<{ card::WC_SIZE }>().0 {
        push(GiftFormat::Wc6, c, 0, 0);
    }
    for c in WC7_FULL.as_chunks::<{ card::FULL_SIZE }>().0 {
        push(GiftFormat::Wc7Full, c, 0, 0);
    }
    for c in WC7.as_chunks::<{ card::WC_SIZE }>().0 {
        push(GiftFormat::Wc7, c, 0, 0);
    }
    out
}

// --- Recherche ------------------------------------------------------------------------------

/// Tri de la liste.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GiftSort {
    #[default]
    Default,
    Newest,
    Species,
    Card,
    Title,
}

/// Filtres de la page « Cadeaux mystère » (champs absents = pas de filtre).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GiftQuery {
    pub species: Option<u16>,
    pub text: String,
    pub shiny: Option<bool>,
    pub egg: Option<bool>,
    /// Générations retenues (vide = toutes).
    pub generations: Vec<u8>,
    /// Natures de cadeau retenues (vide = toutes).
    pub kinds: Vec<GiftKind>,
    /// Seulement les cartes que ces versions peuvent recevoir (vide = pas de filtre).
    pub versions: Vec<u8>,
    pub sort: GiftSort,
}

/// Ignore accents et casse.
pub fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'À' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'í' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'ó' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'œ' | 'Œ' => 'o',
            c => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

impl GiftQuery {
    pub fn matches(&self, g: &Gift) -> bool {
        if !self.generations.is_empty() && !self.generations.contains(&g.generation()) {
            return false;
        }
        if !self.kinds.is_empty() && !self.kinds.contains(&g.kind) {
            return false;
        }
        if let Some(s) = self.species {
            if g.species() != s {
                return false;
            }
        }
        let pk = g.pokemon.as_ref();
        if let Some(shiny) = self.shiny {
            if pk.is_none_or(|p| p.shiny.is_shiny() != shiny) {
                return false;
            }
        }
        if let Some(egg) = self.egg {
            if pk.is_none_or(|p| p.egg != egg) {
                return false;
            }
        }
        if !self.versions.is_empty() && !self.versions.iter().any(|v| g.games.contains(v)) {
            return false;
        }
        let q = fold(self.text.trim());
        if !q.is_empty() {
            let mut hay = fold(&g.display_title());
            if let Some(p) = pk {
                hay.push(' ');
                hay.push_str(&fold(dex::species_name(p.species).unwrap_or("")));
                if let Some(ot) = &p.ot_name {
                    hay.push(' ');
                    hay.push_str(&fold(ot));
                }
            }
            for i in &g.items {
                hay.push(' ');
                hay.push_str(&fold(dex::item_name(i.id).unwrap_or("")));
            }
            let num = q.trim_start_matches('#').trim_start_matches("n°");
            let card_match = num.parse::<u16>().is_ok_and(|n| n == g.card_id);
            if !card_match && !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Indices (dans `gifts`) des cartes retenues, triés.
pub fn search(gifts: &[&Gift], query: &GiftQuery) -> Vec<usize> {
    let mut hits: Vec<usize> = (0..gifts.len()).filter(|&i| query.matches(gifts[i])).collect();
    match query.sort {
        GiftSort::Default => {}
        GiftSort::Newest => hits.sort_by_key(|&i| {
            let g = gifts[i];
            let d = g.date.map_or(0, |d| d.year as u32 * 10000 + d.month as u32 * 100 + d.day as u32);
            std::cmp::Reverse((g.generation(), d, g.card_id))
        }),
        GiftSort::Species => hits.sort_by_key(|&i| (gifts[i].species() == 0, gifts[i].species(), gifts[i].generation())),
        GiftSort::Card => hits.sort_by_key(|&i| (gifts[i].generation(), gifts[i].card_id)),
        GiftSort::Title => {
            let titles: Vec<String> = gifts.iter().map(|g| fold(&g.display_title())).collect();
            hits.sort_by(|&a, &b| titles[a].cmp(&titles[b]));
        }
    }
    hits
}

// --- Vues pour l'interface ------------------------------------------------------------------

/// Jeu d'une puce (« X », « Rubis Oméga »…).
#[derive(Debug, Clone, Serialize)]
pub struct GameChip {
    pub id: u8,
    pub name: &'static str,
}

fn chips(games: &[u8]) -> Vec<GameChip> {
    games.iter().map(|&id| GameChip { id, name: dex::game_name(id).unwrap_or("?") }).collect()
}

/// Ligne de la grille.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftSummary {
    pub id: usize,
    pub format: GiftFormat,
    pub format_label: &'static str,
    pub generation: u8,
    pub card_id: u16,
    pub title: String,
    pub kind: GiftKind,
    pub kind_label: String,
    pub species: u16,
    pub form: u8,
    pub species_name: Option<&'static str>,
    pub level: u8,
    pub shiny: Option<ShinyRule>,
    pub egg: bool,
    pub games: Vec<GameChip>,
    pub date: Option<PkmDate>,
    pub item_names: Vec<String>,
}

impl Gift {
    pub fn summary(&self, id: usize) -> GiftSummary {
        let pk = self.pokemon.as_ref();
        GiftSummary {
            id,
            format: self.format,
            format_label: self.format.label(),
            generation: self.generation(),
            card_id: self.card_id,
            title: self.display_title(),
            kind: self.kind,
            kind_label: self.kind_detail.clone().unwrap_or_else(|| self.kind.label().to_string()),
            species: self.species(),
            form: pk.map_or(0, |p| p.form),
            species_name: pk.and_then(|p| dex::species_name(p.species)),
            level: pk.map_or(0, |p| p.level),
            shiny: pk.map(|p| p.shiny),
            egg: pk.is_some_and(|p| p.egg),
            games: chips(&self.games),
            date: self.date,
            item_names: self.items.iter().map(item_label).collect(),
        }
    }
}

fn item_label(i: &GiftItem) -> String {
    let name = dex::item_name(i.id).map_or_else(|| format!("Objet n°{}", i.id), str::to_string);
    if i.count > 1 {
        format!("{name} × {}", i.count)
    } else {
        name
    }
}

/// Fiche détaillée (panneau de droite).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftDetails {
    #[serde(flatten)]
    pub summary: GiftSummary,
    pub pokemon: Option<GiftPokemon>,
    pub form_name: Option<String>,
    pub ot: String,
    pub trainer_id: String,
    pub ot_gender: Option<Gender>,
    pub ball_name: Option<&'static str>,
    pub held_item_name: Option<&'static str>,
    pub move_names: Vec<String>,
    pub relearn_names: Vec<String>,
    pub nature_name: String,
    pub gender_label: String,
    pub ability_label: String,
    pub ivs_label: String,
    pub met_location_name: Option<String>,
    pub egg_location_name: Option<String>,
    pub language_name: String,
    pub ribbon_names: Vec<&'static str>,
    pub shiny_label: Option<&'static str>,
    /// Remarques (restrictions, contenu non reçu par Kaleido…).
    pub notes: Vec<String>,
    /// Compatibilité avec la sauvegarde ouverte : `None` si aucune.
    pub compatible: Option<bool>,
    pub incompatible_reason: Option<String>,
    pub extension: &'static str,
}

/// Langues (`LanguageID` de PKHeX).
pub fn language_name(id: u8) -> &'static str {
    match id {
        0 => "Celle du jeu",
        1 => "Japonais",
        2 => "Anglais",
        3 => "Français",
        4 => "Italien",
        5 => "Allemand",
        7 => "Espagnol",
        8 => "Coréen",
        9 => "Chinois simplifié",
        10 => "Chinois traditionnel",
        _ => "Inconnue",
    }
}

impl Gift {
    pub fn details(&self, id: usize, save: Option<SaveVersion>) -> GiftDetails {
        let summary = self.summary(id);
        let pk = self.pokemon.as_ref();
        let generation = self.generation();
        let game = match generation {
            4 => dex::Game::HGSS,
            5 => dex::Game::B2W2,
            6 => dex::Game::ORAS,
            _ => dex::Game::USUM,
        };
        let names = |moves: &[u16; 4]| -> Vec<String> {
            moves.iter().filter(|&&m| m != 0).map(|&m| dex::move_name(m).map_or_else(|| format!("n°{m}"), str::to_string)).collect()
        };
        let mut notes = Vec::new();
        if self.kind == GiftKind::Other {
            notes.push(
                "Ce contenu (hors Pokémon et objets) n'est pas encore ajouté par Kaleido : le fichier peut être enregistré pour un autre outil."
                    .into(),
            );
        }
        if self.restrict_language != 0 {
            notes.push(format!("Distribution réservée aux jeux en {}.", language_name(self.restrict_language).to_lowercase()));
        }
        if let Some(w) = save.and_then(|v| version_warning(self, v)) {
            notes.push(w);
        }
        let (compatible, incompatible_reason) = match save {
            Some(v) => match compatibility(self, v) {
                Ok(()) => (Some(true), None),
                Err(e) => (Some(false), Some(e)),
            },
            None => (None, None),
        };
        let ot = match pk.and_then(|p| p.ot_name.clone()) {
            Some(n) => n,
            None if pk.is_some() => "Le tien (dresseur de la sauvegarde)".into(),
            None => String::new(),
        };
        let trainer_id = match pk.and_then(|p| p.tid.zip(p.sid)) {
            Some((tid, sid)) if generation >= 7 => format!("{:06}", ((sid as u32) << 16 | tid as u32) % 1_000_000),
            Some((tid, _)) => format!("{tid:05}"),
            None if pk.is_some() => "Le tien".into(),
            None => String::new(),
        };
        GiftDetails {
            pokemon: pk.cloned(),
            form_name: pk.and_then(|p| (p.form != 0).then(|| dex::form_name(game, p.species, p.form)).flatten()),
            ot,
            trainer_id,
            ot_gender: pk.and_then(|p| p.ot_gender),
            ball_name: pk.and_then(|p| dex::ball_name(p.ball)),
            held_item_name: pk.and_then(|p| (p.held_item != 0).then(|| dex::item_name(p.held_item)).flatten()),
            move_names: pk.map(|p| names(&p.moves)).unwrap_or_default(),
            relearn_names: pk.map(|p| names(&p.relearn)).unwrap_or_default(),
            nature_name: pk.map_or_else(String::new, |p| match p.nature {
                Some(n) => dex::nature_name(n).unwrap_or("?").to_string(),
                None => "Au hasard".into(),
            }),
            gender_label: pk.map_or_else(String::new, |p| {
                match p.gender {
                    Some(Gender::Male) => "Mâle",
                    Some(Gender::Female) => "Femelle",
                    Some(Gender::Genderless) => "Asexué",
                    None => "Au hasard",
                }
                .to_string()
            }),
            ability_label: pk.map_or_else(String::new, |p| ability_label(game, p)),
            ivs_label: pk.map_or_else(String::new, ivs_label),
            met_location_name: pk.and_then(|p| dex::location_name(generation, p.met_location).map(str::to_string)),
            egg_location_name: pk
                .and_then(|p| (p.egg_location != 0).then(|| dex::location_name(generation, p.egg_location)).flatten().map(str::to_string)),
            language_name: pk.map_or_else(String::new, |p| language_name(p.language).to_string()),
            ribbon_names: pk.map(|p| p.ribbons.iter().filter_map(|k| dex::ribbon_name(k)).collect()).unwrap_or_default(),
            shiny_label: pk.map(|p| p.shiny.label()),
            notes,
            compatible,
            incompatible_reason,
            extension: self.format.extension(),
            summary,
        }
    }
}

fn ability_label(game: dex::Game, p: &GiftPokemon) -> String {
    let info = dex::personal(game, p.species, p.form);
    let name = |slot: usize| -> String {
        info.as_ref()
            .map(|i| i.abilities[slot])
            .filter(|&a| a != 0)
            .and_then(dex::ability_name)
            .map_or_else(|| ["Talent 1", "Talent 2", "Talent caché"][slot].to_string(), str::to_string)
    };
    match p.ability {
        AbilityRule::Fixed(2) => format!("{} (caché)", name(2)),
        AbilityRule::Fixed(s) => name(s as usize),
        AbilityRule::OneOrTwo => format!("{} ou {}", name(0), name(1)),
        AbilityRule::Any => format!("{}, {} ou {} (caché)", name(0), name(1), name(2)),
    }
}

fn ivs_label(p: &GiftPokemon) -> String {
    if p.ivs.iter().all(Option::is_none) {
        return match p.perfect_ivs {
            0 => "Aléatoires".into(),
            n => format!("Aléatoires, dont {n} à 31"),
        };
    }
    let parts: Vec<String> = p.ivs.iter().map(|v| v.map_or_else(|| "?".into(), |v| v.to_string())).collect();
    let mut s = parts.join(" / ");
    if p.perfect_ivs > 0 {
        s.push_str(&format!(" (dont {} à 31 parmi les « ? »)", p.perfect_ivs));
    }
    s
}

/// Avertissement (sans blocage) : carte prévue pour d'autres versions de la même génération.
pub fn version_warning(gift: &Gift, version: SaveVersion) -> Option<String> {
    if gift.generation() != version.generation() || gift.receivable_by(version) {
        return None;
    }
    let names: Vec<&str> = gift.games.iter().filter_map(|&v| dex::game_name(v)).collect();
    Some(format!(
        "Distribuée seulement pour {} : Kaleido peut l'ajouter quand même, mais le Pokémon aurait pour jeu d'origine l'un de ceux-là.",
        names.join(", ")
    ))
}

/// Le cadeau peut-il être ajouté à une sauvegarde de ce jeu ? `Err` explique pourquoi.
pub fn compatibility(gift: &Gift, version: SaveVersion) -> Result<(), String> {
    let (g, s) = (gift.generation(), version.generation());
    if g != s {
        return Err(if g > s {
            format!(
                "Cadeau de la Gen {g} : une sauvegarde de la Gen {s} ne peut pas le recevoir (le format des Pokémon et des objets a changé depuis)."
            )
        } else {
            format!(
                "Cadeau de la Gen {g} : Kaleido ne l'ajoute qu'à une sauvegarde de la même génération. En jeu, il faudrait le recevoir en Gen {g} puis le transférer (Poké Transfert, Pokémon Bank…)."
            )
        });
    }
    match gift.kind {
        GiftKind::Other => Err("Ce type de cadeau n'est pas encore pris en charge : seuls les Pokémon, les œufs et les objets sont ajoutés.".into()),
        _ => Ok(()),
    }
}
