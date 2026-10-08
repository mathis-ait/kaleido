//! Base des rencontres Gen 4 à 7 : Pokémon sauvages (`encounter_*.pkl`), fixes, dons,
//! échanges, Rêve Radar, Pokéwalker, Monde des Rêves (tables codées en dur de PKHeX,
//! converties en `encounters.json`).
//!
//! Formats binaires d'après PKHeX : `EncounterArea4.cs` (lieu sur 1 octet, type, taux,
//! terrain, emplacements de 10 octets), `EncounterArea5/6XY/6AO/7.cs` (lieu sur 2 octets,
//! type, emplacements de 4 octets : espèce sur 11 bits + forme sur 5 bits, niveaux),
//! `BinLinkerAccessor.cs` (table de décalages 32 bits) et `EncounterStatic4Pokewalker.cs`.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::dex::Game;

// --- Versions (`GameVersion` de PKHeX).

/// Console virtuelle : Rouge, Vert, Bleu, Jaune, Or, Argent, Cristal.
pub const RD: u8 = 35;
pub const BU: u8 = 37;
pub const YW: u8 = 38;
pub const GD: u8 = 39;
pub const SI: u8 = 40;
pub const CR: u8 = 41;
pub const SA: u8 = 1;
pub const RU: u8 = 2;
pub const EM: u8 = 3;
pub const FR: u8 = 4;
pub const LG: u8 = 5;
pub const HG: u8 = 7;
pub const SS: u8 = 8;
pub const D: u8 = 10;
pub const P: u8 = 11;
pub const PT: u8 = 12;
pub const W: u8 = 20;
pub const B: u8 = 21;
pub const W2: u8 = 22;
pub const B2: u8 = 23;
pub const X: u8 = 24;
pub const Y: u8 = 25;
pub const AS: u8 = 26;
pub const OR: u8 = 27;
pub const SN: u8 = 30;
pub const MN: u8 = 31;
pub const US: u8 = 32;
pub const UM: u8 = 33;

/// Versions d'un groupe de jeux.
pub fn game_versions(game: Game) -> &'static [u8] {
    match game {
        Game::RB => &[RD, BU],
        Game::Y => &[YW],
        Game::GS => &[GD, SI],
        Game::C => &[CR],
        Game::RS => &[RU, SA],
        Game::E => &[EM],
        Game::FRLG => &[FR, LG],
        Game::DP => &[D, P],
        Game::Pt => &[PT],
        Game::HGSS => &[HG, SS],
        Game::BW => &[B, W],
        Game::B2W2 => &[B2, W2],
        Game::XY => &[X, Y],
        Game::ORAS => &[AS, OR],
        Game::SM => &[SN, MN],
        Game::USUM => &[US, UM],
    }
}

/// Groupe de jeux d'une version Gen 4 à 7.
pub fn version_game(version: u8) -> Option<Game> {
    Some(match version {
        RD | 36 | BU => Game::RB,
        YW => Game::Y,
        GD | SI => Game::GS,
        CR => Game::C,
        RU | SA => Game::RS,
        EM => Game::E,
        FR | LG => Game::FRLG,
        D | P => Game::DP,
        PT => Game::Pt,
        HG | SS => Game::HGSS,
        W | B => Game::BW,
        W2 | B2 => Game::B2W2,
        X | Y => Game::XY,
        AS | OR => Game::ORAS,
        SN | MN => Game::SM,
        US | UM => Game::USUM,
        _ => return None,
    })
}

/// Génération d'origine d'une version (1/2 = Console virtuelle, 3 = GBA / GameCube).
pub fn version_generation(version: u8) -> Option<u8> {
    Some(match version {
        1..=5 | 15 => 3,
        7 | 8 | 10..=12 => 4,
        20..=23 => 5,
        24..=27 => 6,
        30..=33 => 7,
        35..=38 => 1,
        39..=41 => 2,
        _ => return None,
    })
}

// --- Types de rencontre.

/// Façon d'obtenir le Pokémon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EncounterKind {
    Grass,
    Surf,
    OldRod,
    GoodRod,
    SuperRod,
    RockSmash,
    Headbutt,
    HeadbuttSpecial,
    BugContest,
    HoneyTree,
    SafariGrass,
    SafariSurf,
    SafariOldRod,
    SafariGoodRod,
    SafariSuperRod,
    /// Gen 5 à 7 : herbes, grottes, ponts… (type « Standard » de PKHeX).
    Standard,
    /// Gen 5 : herbes qui bougent, nuages de poussière, ombres.
    Shaking,
    Swarm,
    HiddenGrotto,
    Horde,
    FriendSafari,
    Sos,
    /// Pokémon fixe (légendaires, Électrode…).
    Static,
    /// Pokémon qui se déplace sur la carte.
    Roaming,
    /// Pokémon offert par un personnage.
    Gift,
    /// Œuf offert par un personnage.
    EggGift,
    /// Échange avec un personnage du jeu.
    Trade,
    /// Monde des Rêves (Pokémon Global Link).
    DreamWorld,
    /// Rêve Radar (3DS).
    DreamRadar,
    /// Pokéwalker (HGSS).
    Pokewalker,
    /// Œuf pondu à la Pension.
    Egg,
    /// Distribution (Cadeau Mystère).
    Event,
}

impl EncounterKind {
    /// Rencontre aléatoire dans les hautes herbes, l'eau…
    pub fn is_wild(self) -> bool {
        !matches!(
            self,
            Self::Static
                | Self::Roaming
                | Self::Gift
                | Self::EggGift
                | Self::Trade
                | Self::DreamWorld
                | Self::DreamRadar
                | Self::Pokewalker
                | Self::Egg
                | Self::Event
        )
    }

    pub fn is_safari(self) -> bool {
        matches!(self, Self::SafariGrass | Self::SafariSurf | Self::SafariOldRod | Self::SafariGoodRod | Self::SafariSuperRod)
    }

    /// Nom français affiché dans la base « Rencontres ».
    pub fn label(self) -> &'static str {
        match self {
            Self::Grass => "Hautes herbes",
            Self::Surf => "Surf",
            Self::OldRod => "Canne",
            Self::GoodRod => "Super Canne",
            Self::SuperRod => "Méga Canne",
            Self::RockSmash => "Éclate-Roc",
            Self::Headbutt => "Coup d'Boule",
            Self::HeadbuttSpecial => "Coup d'Boule (arbre spécial)",
            Self::BugContest => "Concours de Capture",
            Self::HoneyTree => "Arbre à Miel",
            Self::SafariGrass => "Safari : herbes",
            Self::SafariSurf => "Safari : surf",
            Self::SafariOldRod => "Safari : Canne",
            Self::SafariGoodRod => "Safari : Super Canne",
            Self::SafariSuperRod => "Safari : Méga Canne",
            Self::Standard => "Sauvage",
            Self::Shaking => "Herbes qui bougent",
            Self::Swarm => "Essaim",
            Self::HiddenGrotto => "Trouée Cachée",
            Self::Horde => "Horde",
            Self::FriendSafari => "Safari des Amis",
            Self::Sos => "Appel à l'aide (SOS)",
            Self::Static => "Fixe",
            Self::Roaming => "Vagabond",
            Self::Gift => "Don",
            Self::EggGift => "Œuf offert",
            Self::Trade => "Échange",
            Self::DreamWorld => "Monde des Rêves",
            Self::DreamRadar => "Rêve Radar",
            Self::Pokewalker => "Pokéwalker",
            Self::Egg => "Œuf (Pension)",
            Self::Event => "Distribution",
        }
    }

    /// Famille, pour les filtres de l'interface.
    pub fn family(self) -> &'static str {
        match self {
            Self::Grass | Self::Standard | Self::Shaking | Self::Swarm | Self::Horde | Self::Sos | Self::SafariGrass | Self::BugContest => "herbes",
            Self::Surf | Self::SafariSurf => "surf",
            Self::OldRod | Self::GoodRod | Self::SuperRod | Self::SafariOldRod | Self::SafariGoodRod | Self::SafariSuperRod => "peche",
            Self::RockSmash | Self::Headbutt | Self::HeadbuttSpecial | Self::HoneyTree | Self::HiddenGrotto | Self::FriendSafari => "special",
            Self::Static | Self::Roaming => "fixe",
            Self::Gift => "don",
            Self::EggGift | Self::Egg => "oeuf",
            Self::Trade => "echange",
            Self::DreamWorld | Self::DreamRadar | Self::Pokewalker | Self::Event => "evenement",
        }
    }
}

/// Talents permis par la rencontre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AbilityRule {
    /// Talent 1 ou 2.
    Any12,
    /// Talent 1, 2 ou caché.
    Any12H,
    OnlyFirst,
    OnlySecond,
    OnlyHidden,
}

impl AbilityRule {
    fn parse(s: &str) -> Self {
        match s {
            "1" => Self::OnlyFirst,
            "2" => Self::OnlySecond,
            "H" => Self::OnlyHidden,
            "12H" => Self::Any12H,
            _ => Self::Any12,
        }
    }

    pub fn allows_hidden(self) -> bool {
        matches!(self, Self::Any12H | Self::OnlyHidden)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Any12 => "Talent 1 ou 2",
            Self::Any12H => "Talent 1, 2 ou caché",
            Self::OnlyFirst => "Talent 1 imposé",
            Self::OnlySecond => "Talent 2 imposé",
            Self::OnlyHidden => "Talent caché imposé",
        }
    }
}

/// Chromatique permis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShinyRule {
    Random,
    /// Verrou chromatique : jamais chromatique.
    Never,
    Always,
    /// PID imposé (échanges, Pokémon de N…).
    FixedPid,
}

/// Dresseur imposé (échanges, Pokémon de N, Ranch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedTrainer {
    pub tid: u16,
    pub sid: u16,
    pub ot_gender: Option<u8>,
    /// Noms possibles selon la langue (identifiant de langue, nom).
    pub names: Vec<(u8, String)>,
}

/// Une rencontre possible.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    pub kind: EncounterKind,
    pub generation: u8,
    /// Versions où la rencontre existe (`GameVersion` de PKHeX).
    pub versions: Vec<u8>,
    pub species: u16,
    /// Forme ; 30 = motif de Prismillon de la console, 31 = forme aléatoire.
    pub form: u8,
    pub level_min: u8,
    pub level_max: u8,
    pub location: u16,
    pub egg_location: u16,
    /// Ball imposée.
    pub ball: Option<u8>,
    pub ability: AbilityRule,
    pub shiny: ShinyRule,
    /// Sexe imposé (0 mâle, 1 femelle, 2 asexué).
    pub gender: Option<u8>,
    pub nature: Option<u8>,
    /// IV imposés (ordre Kaleido : PV, Att, Déf, Atq Spé, Déf Spé, Vit ; -1 = libre).
    pub ivs: Option<[i8; 6]>,
    /// Nombre minimal d'IV à 31.
    pub flawless_ivs: u8,
    pub moves: Vec<u16>,
    pub relearn: Vec<u16>,
    pub fateful: bool,
    pub held_item: u16,
    pub trainer: Option<FixedTrainer>,
    pub pid: Option<u32>,
    /// Niveau de rencontre différent du niveau reçu (Ranch).
    pub met_level: Option<u8>,
    /// Pokémon de N (étincelles).
    pub n_sparkle: bool,
    /// Le nom est imposé (échanges).
    pub fixed_nickname: bool,
    /// Surnoms possibles (langue, surnom) pour un échange.
    pub nicknames: Vec<(u8, String)>,
    /// Peut évoluer dès l'échange.
    pub evolve_on_trade: bool,
    /// Distribution : langue et constante de chiffrement imposées, titre de la carte.
    pub language: Option<u8>,
    pub ec: Option<u32>,
    pub title: Option<String>,
    /// Gen 4 sauvage : numéros d'emplacement (slot) de la zone, pour les tirages des méthodes J et K.
    #[serde(skip)]
    pub slots: Vec<u8>,
}

impl Encounter {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn base(kind: EncounterKind, generation: u8, versions: Vec<u8>, species: u16, form: u8, min: u8, max: u8, location: u16) -> Self {
        Encounter {
            kind,
            generation,
            versions,
            species,
            form,
            level_min: min,
            level_max: max,
            location,
            egg_location: 0,
            ball: None,
            ability: AbilityRule::Any12,
            shiny: ShinyRule::Random,
            gender: None,
            nature: None,
            ivs: None,
            flawless_ivs: 0,
            moves: Vec::new(),
            relearn: Vec::new(),
            fateful: false,
            held_item: 0,
            trainer: None,
            pid: None,
            met_level: None,
            n_sparkle: false,
            fixed_nickname: false,
            nicknames: Vec::new(),
            evolve_on_trade: false,
            language: None,
            ec: None,
            title: None,
            slots: Vec::new(),
        }
    }

    /// Œuf de Pension de l'espèce dans un jeu.
    pub fn egg(game: Game, species: u16, form: u8) -> Self {
        let generation = game.generation();
        let mut e = Encounter::base(EncounterKind::Egg, generation, game_versions(game).to_vec(), species, form, 1, 1, 0);
        e.egg_location = if generation == 4 { 2000 } else { 60002 };
        if generation <= 5 {
            e.ball = Some(4);
        }
        e.ability = if generation >= 5 { AbilityRule::Any12H } else { AbilityRule::Any12 };
        e
    }

    pub fn is_egg(&self) -> bool {
        self.kind == EncounterKind::Egg
            || self.kind == EncounterKind::EggGift
            || (self.kind == EncounterKind::Event && self.egg_location != 0 && self.location == 0)
    }

    /// Lieu de rencontre attendu une fois le Pokémon reçu (échange sans lieu : lieu d'échange).
    pub fn met_location(&self) -> u16 {
        self.location
    }
}

// --- Lecture des fichiers de PKHeX.

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// « BinLinker » de PKHeX : identifiant (2 octets), nombre d'entrées (2 octets), puis
/// les décalages 32 bits de début de chaque entrée, le suivant marquant la fin.
pub(crate) fn binlinker32(data: &[u8]) -> Vec<&[u8]> {
    let count = u16_at(data, 2) as usize;
    (0..count)
        .map(|i| {
            let start = u32_at(data, 4 + 4 * i) as usize;
            let end = u32_at(data, 8 + 4 * i) as usize;
            data.get(start..end).unwrap_or(&[])
        })
        .collect()
}

fn gen4_kind(t: u8) -> EncounterKind {
    use EncounterKind::*;
    match t {
        0 => Grass,
        1 => Surf,
        2 => OldRod,
        3 => GoodRod,
        4 => SuperRod,
        5 => RockSmash,
        6 => Headbutt,
        7 => HeadbuttSpecial,
        8 => BugContest,
        9 => HoneyTree,
        10 => SafariGrass,
        11 => SafariSurf,
        12 => SafariOldRod,
        13 => SafariGoodRod,
        _ => SafariSuperRod,
    }
}

fn gen5_kind(t: u8) -> EncounterKind {
    use EncounterKind::*;
    match t {
        1 => Shaking,
        2 => Surf,
        3 => SuperRod,
        4 => Swarm,
        5 => HiddenGrotto,
        _ => Standard,
    }
}

fn gen6_kind(t: u8) -> EncounterKind {
    use EncounterKind::*;
    match t {
        1 => Grass,
        2 => Surf,
        3 => OldRod,
        4 => GoodRod,
        5 => SuperRod,
        6 => RockSmash,
        7 => Horde,
        8 => FriendSafari,
        _ => Standard,
    }
}

/// Espèces du Safari des Amis (XY), niveau 30 (`EncounterArea6XY.AllFriendSafariSpecies`).
const FRIEND_SAFARI: &[u16] = &[
    2, 5, 8, 12, 14, 16, 21, 25, 27, 35, 38, 39, 43, 44, 46, 49, 49, 51, 56, 58, 61, 63, 67, 77, 82, 83, 84, 87, 89, 91, 95, 96, 98, 101, 105, 112,
    113, 114, 125, 126, 127, 130, 131, 132, 133, 148, 163, 165, 168, 175, 178, 184, 190, 191, 194, 195, 202, 203, 205, 206, 209, 213, 214, 215, 215,
    216, 218, 219, 221, 222, 224, 225, 227, 231, 235, 236, 247, 262, 267, 268, 274, 281, 284, 286, 290, 294, 297, 299, 302, 303, 303, 307, 310, 313,
    314, 317, 323, 326, 328, 332, 336, 342, 352, 353, 356, 357, 359, 361, 363, 372, 375, 400, 404, 415, 417, 419, 423, 426, 437, 442, 444, 447, 452,
    454, 459, 506, 510, 511, 513, 515, 517, 520, 523, 525, 527, 530, 531, 536, 538, 539, 541, 544, 548, 551, 556, 557, 561, 569, 572, 575, 578, 581,
    586, 587, 596, 597, 600, 608, 611, 614, 618, 619, 621, 623, 624, 627, 629, 636, 651, 654, 657, 660, 662, 662, 668, 673, 674, 677, 682, 684, 686,
    689, 694, 701, 702, 702, 705, 707, 708, 710, 712, 714,
];

fn read_areas(data: &[u8], version: u8, generation: u8, out: &mut Vec<Encounter>) {
    for area in binlinker32(data) {
        if generation == 3 {
            // `EncounterArea3` : lieu u8, (inutilisé), type, taux, emplacements de 10 octets
            // (espèce u16, forme, n° d'emplacement, niveaux min et max, 4 octets de Méthode 1).
            if area.len() < 4 {
                continue;
            }
            let location = area[0] as u16;
            let kind = if area[2] >= 6 { EncounterKind::Swarm } else { gen4_kind(area[2]) };
            for s in area[4..].as_chunks::<10>().0 {
                let species = u16_at(s, 0);
                if species == 0 {
                    continue;
                }
                let mut e = Encounter::base(kind, 3, vec![version], species, s[2], s[4], s[5], location);
                // Parc Safari : Safari Ball.
                if matches!(location, 57 | 136) {
                    e.ball = Some(5);
                }
                out.push(e);
            }
        } else if generation == 4 {
            if area.len() < 6 {
                continue;
            }
            let location = area[0] as u16;
            let kind = gen4_kind(area[2]);
            for s in area[6..].as_chunks::<10>().0 {
                let species = u16_at(s, 0);
                if species == 0 {
                    continue;
                }
                let mut e = Encounter::base(kind, 4, vec![version], species, s[2], s[4], s[5], location);
                e.slots = vec![s[3]];
                if kind == EncounterKind::BugContest {
                    e.ball = Some(24);
                } else if kind.is_safari() || location == 52 {
                    e.ball = Some(5);
                }
                out.push(e);
            }
        } else {
            if area.len() < 4 {
                continue;
            }
            let location = u16_at(area, 0);
            let kind = match generation {
                5 => gen5_kind(area[2]),
                6 => gen6_kind(area[2]),
                _ if area[2] == 1 => EncounterKind::Sos,
                _ => EncounterKind::Standard,
            };
            for s in area[4..].as_chunks::<4>().0 {
                let raw = u16_at(s, 0);
                let species = raw & 0x3FF;
                if species == 0 {
                    continue;
                }
                let form = (raw >> 11) as u8;
                let mut e = Encounter::base(kind, generation, vec![version], species, form, s[2], s[3], location);
                match kind {
                    EncounterKind::HiddenGrotto => {
                        e.ability = AbilityRule::OnlyHidden;
                        e.shiny = ShinyRule::Never;
                    }
                    EncounterKind::Horde | EncounterKind::Sos => e.ability = AbilityRule::Any12H,
                    // DexNav (ROSA) : talent caché possible sauf Éclate-Roc.
                    _ if generation == 6 && (version == AS || version == OR) && kind != EncounterKind::RockSmash => e.ability = AbilityRule::Any12H,
                    _ => {}
                }
                // Île Paradis (Poké Loisir) : Poké Ball seulement.
                if generation == 7 && location == 30016 {
                    e.ball = Some(4);
                }
                out.push(e);
            }
        }
    }
}

fn friend_safari(out: &mut Vec<Encounter>) {
    let mut push = |species: u16, form: u8| {
        let mut e = Encounter::base(EncounterKind::FriendSafari, 6, vec![X, Y], species, form, 30, 30, 148);
        e.ability = AbilityRule::Any12H;
        e.flawless_ivs = 2;
        out.push(e);
    };
    let mut seen = std::collections::HashSet::new();
    for &s in FRIEND_SAFARI.iter() {
        if seen.insert(s) {
            push(s, 0);
        }
    }
    for f in [0, 1, 3] {
        push(670, f); // Floette
    }
    push(666, 30); // Prismillon (motif de la console)
}

fn pokewalker(data: &[u8], out: &mut Vec<Encounter>) {
    for s in data.as_chunks::<12>().0 {
        let species = u16_at(s, 0);
        if species == 0 {
            continue;
        }
        let mut e = Encounter::base(EncounterKind::Pokewalker, 4, vec![HG, SS], species, 0, s[2], s[2], 233);
        e.gender = Some(s[3]);
        e.ball = Some(4);
        e.shiny = ShinyRule::Never;
        e.moves = (0..4).map(|i| u16_at(s, 4 + 2 * i)).filter(|&m| m != 0).collect();
        out.push(e);
    }
}

/// Enregistrement de `encounters.json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    gen: u8,
    versions: Vec<u8>,
    kind: String,
    species: u16,
    #[serde(default)]
    form: u8,
    level: u8,
    #[serde(default)]
    level_max: Option<u8>,
    #[serde(default)]
    location: Option<u16>,
    #[serde(default)]
    egg_location: u16,
    #[serde(default)]
    ball: Option<u8>,
    #[serde(default)]
    ability: Option<String>,
    #[serde(default)]
    shiny: Option<String>,
    #[serde(default)]
    gender: Option<u8>,
    #[serde(default)]
    nature: Option<u8>,
    #[serde(default)]
    ivs: Option<Vec<i8>>,
    #[serde(default)]
    flawless: u8,
    #[serde(default)]
    moves: Vec<u16>,
    #[serde(default)]
    relearn: Vec<u16>,
    #[serde(default)]
    fateful: bool,
    #[serde(default)]
    held_item: u16,
    #[serde(default)]
    tid: Option<u16>,
    #[serde(default)]
    sid: Option<u16>,
    #[serde(default)]
    ot_gender: Option<u8>,
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default)]
    met_level: Option<u8>,
    #[serde(default)]
    n_sparkle: bool,
    #[serde(default)]
    trade_file: Option<String>,
    #[serde(default)]
    trade_index: Option<usize>,
    #[serde(default)]
    trade_ot: Option<String>,
    #[serde(default)]
    fixed_nickname: bool,
    #[serde(default)]
    ranch: bool,
    #[serde(default)]
    evolve_on_trade: bool,
}

/// Noms des échanges : fichier → langue → lignes (surnoms puis noms des dresseurs).
type TradeNames = HashMap<String, HashMap<u8, Vec<String>>>;

static TRADE_NAMES: LazyLock<TradeNames> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../data/pkhex/legality/trade_names.json")).unwrap_or_default());

/// Noms des dresseurs d'échanges de N2/B2 (`TradeOT_B2W2_F` / `_M`).
const B2W2_OT_F: [(u8, &str); 7] = [(1, "ルリ"), (2, "Yancy"), (3, "Brenda"), (4, "Lilì"), (5, "Sabine"), (7, "Belinda"), (8, "루리")];
const B2W2_OT_M: [(u8, &str); 7] = [(1, "テツ"), (2, "Curtis"), (3, "Julien"), (4, "Dadi"), (5, "Markus"), (7, "Julián"), (8, "철권")];
const RANCH_OT: [(u8, &str); 6] = [(1, "ユカリ"), (2, "Hayley"), (3, "EULALIE"), (4, "GIULIA"), (5, "EUKALIA"), (7, "Eulalia")];

/// Noms par langue (identifiant de langue, texte).
type Names = Vec<(u8, String)>;

fn trade_lines(file: &str, index: usize) -> (Names, Names) {
    let mut nicks = Vec::new();
    let mut ots = Vec::new();
    if let Some(langs) = TRADE_NAMES.get(file) {
        let mut ids: Vec<_> = langs.keys().copied().collect();
        ids.sort();
        for lang in ids {
            let lines = &langs[&lang];
            let half = lines.len() / 2;
            if let Some(n) = lines.get(index) {
                nicks.push((lang, n.clone()));
            }
            if let Some(o) = lines.get(index + half) {
                ots.push((lang, o.clone()));
            }
        }
    }
    (nicks, ots)
}

fn from_raw(r: Raw) -> Encounter {
    let kind = match r.kind.as_str() {
        "gift" => EncounterKind::Gift,
        "eggGift" => EncounterKind::EggGift,
        "roaming" => EncounterKind::Roaming,
        "trade" => EncounterKind::Trade,
        "dreamWorld" => EncounterKind::DreamWorld,
        "dreamRadar" => EncounterKind::DreamRadar,
        _ => EncounterKind::Static,
    };
    let level_max = r.level_max.unwrap_or(r.level);
    let location = r.location.unwrap_or(0);
    let mut e = Encounter::base(kind, r.gen, r.versions, r.species, r.form, r.level, level_max, location);
    e.egg_location = r.egg_location;
    e.ball = r.ball;
    e.ability = match r.ability.as_deref() {
        // Monde des Rêves : talent caché si l'espèce en a un (B2N2), sinon talent 1.
        Some("H?") => {
            let has_hidden = crate::dex::personal(Game::B2W2, r.species, r.form).is_some_and(|p| p.abilities[2] != 0);
            if has_hidden {
                AbilityRule::OnlyHidden
            } else {
                AbilityRule::OnlyFirst
            }
        }
        Some(a) => AbilityRule::parse(a),
        None => AbilityRule::Any12,
    };
    e.shiny = match r.shiny.as_deref() {
        Some("never") => ShinyRule::Never,
        Some("always") => ShinyRule::Always,
        Some("fixed") => ShinyRule::FixedPid,
        _ => ShinyRule::Random,
    };
    e.gender = r.gender;
    e.nature = r.nature;
    e.ivs = r.ivs.filter(|v| v.len() == 6).map(|v| [v[0], v[1], v[2], v[4], v[5], v[3]]);
    e.flawless_ivs = r.flawless;
    e.moves = r.moves.into_iter().filter(|&m| m != 0).collect();
    e.relearn = r.relearn.into_iter().filter(|&m| m != 0).collect();
    e.fateful = r.fateful;
    e.held_item = r.held_item;
    e.pid = r.pid;
    e.met_level = r.met_level;
    e.n_sparkle = r.n_sparkle;
    e.evolve_on_trade = r.evolve_on_trade;
    e.fixed_nickname = r.fixed_nickname;
    if let Some(tid) = r.tid {
        let mut names: Vec<(u8, String)> = Vec::new();
        if let (Some(file), Some(index)) = (&r.trade_file, r.trade_index) {
            let (nicks, ots) = trade_lines(file, index);
            e.nicknames = nicks;
            names = ots;
        } else if let Some(list) = &r.trade_ot {
            let src: &[(u8, &str)] = if list.ends_with("_M") { &B2W2_OT_M } else { &B2W2_OT_F };
            names = src.iter().map(|&(l, n)| (l, n.to_string())).collect();
        } else if r.ranch {
            names = RANCH_OT.iter().map(|&(l, n)| (l, n.to_string())).collect();
        } else if r.n_sparkle {
            names = vec![
                (1, "Ｎ".to_string()),
                (2, "N".to_string()),
                (3, "N".to_string()),
                (4, "N".to_string()),
                (5, "N".to_string()),
                (7, "N".to_string()),
                (8, "N".to_string()),
            ];
        }
        e.trainer = Some(FixedTrainer { tid, sid: r.sid.unwrap_or(0), ot_gender: r.ot_gender, names });
    }
    // Rêve Radar : niveau 5 à 40 par paliers de 5 (badges).
    if kind == EncounterKind::DreamRadar {
        e.level_max = 40;
    }
    e
}

static STATIC: LazyLock<Vec<Encounter>> = LazyLock::new(|| {
    let raw: Vec<Raw> = serde_json::from_str(include_str!("../../data/pkhex/legality/encounters.json")).unwrap_or_default();
    raw.into_iter().map(from_raw).collect()
});

macro_rules! pkl {
    ($name:literal) => {
        include_bytes!(concat!("../../data/pkhex/legality/encounter_", $name, ".pkl"))
    };
}

fn build(game: Game) -> Vec<Encounter> {
    let mut out = Vec::new();
    let generation = game.generation();
    let files: &[(&[u8], u8)] = match game {
        Game::DP => &[(pkl!("d"), D), (pkl!("p"), P)],
        Game::Pt => &[(pkl!("pt"), PT)],
        Game::HGSS => &[(pkl!("hg"), HG), (pkl!("ss"), SS)],
        Game::BW => &[(pkl!("b"), B), (pkl!("w"), W)],
        Game::B2W2 => &[(pkl!("b2"), B2), (pkl!("w2"), W2)],
        Game::XY => &[(pkl!("x"), X), (pkl!("y"), Y)],
        Game::ORAS => &[(pkl!("or"), OR), (pkl!("as"), AS)],
        Game::SM => &[(pkl!("sn"), SN), (pkl!("mn"), MN)],
        Game::USUM => &[(pkl!("us"), US), (pkl!("um"), UM)],
        // Gen 3 : herbes, surf, cannes, Éclate-Roc (format `EncounterArea3`).
        Game::RS => &[(pkl!("r"), RU), (pkl!("s"), SA)],
        Game::E => &[(pkl!("e"), EM)],
        Game::FRLG => &[(pkl!("fr"), FR), (pkl!("lg"), LG)],
        // Gen 1 et 2 : pas de table de rencontres embarquée.
        Game::RB | Game::Y | Game::GS | Game::C => &[],
    };
    for &(data, version) in files {
        read_areas(data, version, generation, &mut out);
    }
    if game == Game::XY {
        friend_safari(&mut out);
    }
    if game == Game::HGSS {
        pokewalker(pkl!("walker4"), &mut out);
    }
    let versions = game_versions(game);
    out.extend(STATIC.iter().filter(|e| e.versions.iter().any(|v| versions.contains(v))).cloned());
    merge_versions(out)
}

/// Fusionne les emplacements identiques des deux versions d'une paire (`versions` réunies).
fn merge_versions(list: Vec<Encounter>) -> Vec<Encounter> {
    let mut out: Vec<Encounter> = Vec::with_capacity(list.len());
    type Key = (EncounterKind, u16, u8, u8, u8, u16, bool);
    let mut index: HashMap<Key, usize> = HashMap::new();
    for e in list {
        if e.kind.is_wild() {
            let key = (e.kind, e.species, e.form, e.level_min, e.level_max, e.location, e.ability == AbilityRule::Any12H);
            if let Some(&i) = index.get(&key) {
                for v in &e.versions {
                    if !out[i].versions.contains(v) {
                        out[i].versions.push(*v);
                    }
                }
                for s in &e.slots {
                    if !out[i].slots.contains(s) {
                        out[i].slots.push(*s);
                    }
                }
                continue;
            }
            index.insert(key, out.len());
        }
        out.push(e);
    }
    out
}

static TABLES: [LazyLock<Vec<Encounter>>; 16] = [
    LazyLock::new(|| build(Game::DP)),
    LazyLock::new(|| build(Game::Pt)),
    LazyLock::new(|| build(Game::HGSS)),
    LazyLock::new(|| build(Game::BW)),
    LazyLock::new(|| build(Game::B2W2)),
    LazyLock::new(|| build(Game::XY)),
    LazyLock::new(|| build(Game::ORAS)),
    LazyLock::new(|| build(Game::SM)),
    LazyLock::new(|| build(Game::USUM)),
    LazyLock::new(|| build(Game::RS)),
    LazyLock::new(|| build(Game::E)),
    LazyLock::new(|| build(Game::FRLG)),
    LazyLock::new(|| build(Game::RB)),
    LazyLock::new(|| build(Game::Y)),
    LazyLock::new(|| build(Game::GS)),
    LazyLock::new(|| build(Game::C)),
];

/// Toutes les rencontres d'un groupe de jeux (sauvages puis fixes, dons, échanges…).
pub fn encounters(game: Game) -> &'static [Encounter] {
    let i = Game::ALL.iter().position(|&g| g == game).unwrap_or(0);
    &TABLES[i]
}
