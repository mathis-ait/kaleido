//! Champs secondaires des Pokémon Gen 4 à 7 : concours, rubans, caractéristique, terrain
//! de rencontre, champs propres à une génération (feuilles brillantes, humeur de promenade,
//! éclat de N, renommée Pokéstar), et, en Gen 6/7, soigneur, pays, souvenirs, Poké Récré,
//! Super Training et Hyper Training.
//!
//! Offsets vérifiés sur le code de PKHeX (`PK4.cs`, `PK5.cs`, `PK6.cs`, `PK7.cs`, `G6PKM.cs`,
//! `EntityCharacteristic.cs`) ; les tables de rubans en sont extraites telles quelles.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{strings, Gender, PkmError, PkmFormat, Pokemon};
use crate::dex;

// Vérifié : PKHeX PK4.cs / PK5.cs (Contest 0x1E, ShinyLeaf 0x41, NSparkle bit 1 de 0x42,
// GroundTile 0x85, WalkingMood / PokeStarFame 0x87) ; PK6.cs / PK7.cs (Contest 0x24,
// SuperTrainBitFlags 0x2C, DistTrainBitFlags 0x3A, FormArgument 0x3C, Secret / Supremely 0x72,
// HandlingTrainerTrash 0x78 sur 26 octets, HandlingTrainerGender 0x92, CurrentHandler 0x93,
// Geo1-5 0x94, HT Friendship 0xA2, HT Affection 0xA3, HT Memory 0xA4-0xA9, Fullness 0xAE,
// Enjoyment 0xAF, OT Affection 0xCB, OT Memory 0xCC-0xD0, GroundTile PK6 / HyperTrainFlags PK7 0xDE,
// Country 0xE0, Region 0xE1, ConsoleRegion 0xE2).
const G45_CONTEST: usize = 0x1E;
const G67_CONTEST: usize = 0x24;
const G4_SHINY_LEAF: usize = 0x41;
const G5_FLAGS: usize = 0x42;
const G45_GROUND_TILE: usize = 0x85;
/// Gen 4 : humeur de promenade (HGSS) ; Gen 5 : renommée Pokéstar (N2B2).
const G45_BYTE_87: usize = 0x87;
const G67_SUPER_TRAINING: usize = 0x2C;
const G67_DIST_TRAINING: usize = 0x3A;
const G67_FORM_ARGUMENT: usize = 0x3C;
const G67_TRAINING_FLAGS: usize = 0x72;
const G67_HT_NAME: usize = 0x78;
const G67_HT_GENDER: usize = 0x92;
const G67_CURRENT_HANDLER: usize = 0x93;
const G67_GEO: usize = 0x94;
const G67_HT_FRIENDSHIP: usize = 0xA2;
const G67_HT_AFFECTION: usize = 0xA3;
const G67_HT_MEMORY: usize = 0xA4;
const G67_FULLNESS: usize = 0xAE;
const G67_ENJOYMENT: usize = 0xAF;
const G67_OT_AFFECTION: usize = 0xCB;
const G67_OT_MEMORY: usize = 0xCC;
const G6_GROUND_TILE: usize = 0xDE;
const G7_HYPER_TRAINING: usize = 0xDE;
const G67_COUNTRY: usize = 0xE0;
const G67_REGION: usize = 0xE1;
const G67_CONSOLE_REGION: usize = 0xE2;
const HT_NAME_MAX: usize = 12;

/// Ordre Kaleido (PV, Att, Déf, Atq Spé, Déf Spé, Vit) → bit de l'Hyper Training (PK7.HT_*).
const HYPER_TRAINING_BITS: [u8; 6] = [0, 1, 2, 3, 4, 5];

/// Rubans : (clé PKHeX, octet, bit), extraits de PKHeX PK5.cs (identique à PK4.cs), PK6.cs et PK7.cs.
const RIBBONS_DS: [(&str, usize, u8); 80] = [
    ("RibbonChampionSinnoh", 0x24, 0),
    ("RibbonAbility", 0x24, 1),
    ("RibbonAbilityGreat", 0x24, 2),
    ("RibbonAbilityDouble", 0x24, 3),
    ("RibbonAbilityMulti", 0x24, 4),
    ("RibbonAbilityPair", 0x24, 5),
    ("RibbonAbilityWorld", 0x24, 6),
    ("RibbonAlert", 0x24, 7),
    ("RibbonShock", 0x25, 0),
    ("RibbonDowncast", 0x25, 1),
    ("RibbonCareless", 0x25, 2),
    ("RibbonRelax", 0x25, 3),
    ("RibbonSnooze", 0x25, 4),
    ("RibbonSmile", 0x25, 5),
    ("RibbonGorgeous", 0x25, 6),
    ("RibbonRoyal", 0x25, 7),
    ("RibbonGorgeousRoyal", 0x26, 0),
    ("RibbonFootprint", 0x26, 1),
    ("RibbonRecord", 0x26, 2),
    ("RibbonEvent", 0x26, 3),
    ("RibbonLegend", 0x26, 4),
    ("RibbonChampionWorld", 0x26, 5),
    ("RibbonBirthday", 0x26, 6),
    ("RibbonSpecial", 0x26, 7),
    ("RibbonSouvenir", 0x27, 0),
    ("RibbonWishing", 0x27, 1),
    ("RibbonClassic", 0x27, 2),
    ("RibbonPremier", 0x27, 3),
    ("RibbonG3Cool", 0x3C, 0),
    ("RibbonG3CoolSuper", 0x3C, 1),
    ("RibbonG3CoolHyper", 0x3C, 2),
    ("RibbonG3CoolMaster", 0x3C, 3),
    ("RibbonG3Beauty", 0x3C, 4),
    ("RibbonG3BeautySuper", 0x3C, 5),
    ("RibbonG3BeautyHyper", 0x3C, 6),
    ("RibbonG3BeautyMaster", 0x3C, 7),
    ("RibbonG3Cute", 0x3D, 0),
    ("RibbonG3CuteSuper", 0x3D, 1),
    ("RibbonG3CuteHyper", 0x3D, 2),
    ("RibbonG3CuteMaster", 0x3D, 3),
    ("RibbonG3Smart", 0x3D, 4),
    ("RibbonG3SmartSuper", 0x3D, 5),
    ("RibbonG3SmartHyper", 0x3D, 6),
    ("RibbonG3SmartMaster", 0x3D, 7),
    ("RibbonG3Tough", 0x3E, 0),
    ("RibbonG3ToughSuper", 0x3E, 1),
    ("RibbonG3ToughHyper", 0x3E, 2),
    ("RibbonG3ToughMaster", 0x3E, 3),
    ("RibbonChampionG3", 0x3E, 4),
    ("RibbonWinning", 0x3E, 5),
    ("RibbonVictory", 0x3E, 6),
    ("RibbonArtist", 0x3E, 7),
    ("RibbonEffort", 0x3F, 0),
    ("RibbonChampionBattle", 0x3F, 1),
    ("RibbonChampionRegional", 0x3F, 2),
    ("RibbonChampionNational", 0x3F, 3),
    ("RibbonCountry", 0x3F, 4),
    ("RibbonNational", 0x3F, 5),
    ("RibbonEarth", 0x3F, 6),
    ("RibbonWorld", 0x3F, 7),
    ("RibbonG4Cool", 0x60, 0),
    ("RibbonG4CoolGreat", 0x60, 1),
    ("RibbonG4CoolUltra", 0x60, 2),
    ("RibbonG4CoolMaster", 0x60, 3),
    ("RibbonG4Beauty", 0x60, 4),
    ("RibbonG4BeautyGreat", 0x60, 5),
    ("RibbonG4BeautyUltra", 0x60, 6),
    ("RibbonG4BeautyMaster", 0x60, 7),
    ("RibbonG4Cute", 0x61, 0),
    ("RibbonG4CuteGreat", 0x61, 1),
    ("RibbonG4CuteUltra", 0x61, 2),
    ("RibbonG4CuteMaster", 0x61, 3),
    ("RibbonG4Smart", 0x61, 4),
    ("RibbonG4SmartGreat", 0x61, 5),
    ("RibbonG4SmartUltra", 0x61, 6),
    ("RibbonG4SmartMaster", 0x61, 7),
    ("RibbonG4Tough", 0x62, 0),
    ("RibbonG4ToughGreat", 0x62, 1),
    ("RibbonG4ToughUltra", 0x62, 2),
    ("RibbonG4ToughMaster", 0x62, 3),
];

const RIBBONS_PK6: [(&str, usize, u8); 44] = [
    ("RibbonChampionKalos", 0x30, 0),
    ("RibbonChampionG3", 0x30, 1),
    ("RibbonChampionSinnoh", 0x30, 2),
    ("RibbonBestFriends", 0x30, 3),
    ("RibbonTraining", 0x30, 4),
    ("RibbonBattlerSkillful", 0x30, 5),
    ("RibbonBattlerExpert", 0x30, 6),
    ("RibbonEffort", 0x30, 7),
    ("RibbonAlert", 0x31, 0),
    ("RibbonShock", 0x31, 1),
    ("RibbonDowncast", 0x31, 2),
    ("RibbonCareless", 0x31, 3),
    ("RibbonRelax", 0x31, 4),
    ("RibbonSnooze", 0x31, 5),
    ("RibbonSmile", 0x31, 6),
    ("RibbonGorgeous", 0x31, 7),
    ("RibbonRoyal", 0x32, 0),
    ("RibbonGorgeousRoyal", 0x32, 1),
    ("RibbonArtist", 0x32, 2),
    ("RibbonFootprint", 0x32, 3),
    ("RibbonRecord", 0x32, 4),
    ("RibbonLegend", 0x32, 5),
    ("RibbonCountry", 0x32, 6),
    ("RibbonNational", 0x32, 7),
    ("RibbonEarth", 0x33, 0),
    ("RibbonWorld", 0x33, 1),
    ("RibbonClassic", 0x33, 2),
    ("RibbonPremier", 0x33, 3),
    ("RibbonEvent", 0x33, 4),
    ("RibbonBirthday", 0x33, 5),
    ("RibbonSpecial", 0x33, 6),
    ("RibbonSouvenir", 0x33, 7),
    ("RibbonWishing", 0x34, 0),
    ("RibbonChampionBattle", 0x34, 1),
    ("RibbonChampionRegional", 0x34, 2),
    ("RibbonChampionNational", 0x34, 3),
    ("RibbonChampionWorld", 0x34, 4),
    ("RibbonChampionG6Hoenn", 0x34, 7),
    ("RibbonContestStar", 0x35, 0),
    ("RibbonMasterCoolness", 0x35, 1),
    ("RibbonMasterBeauty", 0x35, 2),
    ("RibbonMasterCuteness", 0x35, 3),
    ("RibbonMasterCleverness", 0x35, 4),
    ("RibbonMasterToughness", 0x35, 5),
];

const RIBBONS_PK7: [(&str, usize, u8); 48] = [
    ("RibbonChampionKalos", 0x30, 0),
    ("RibbonChampionG3", 0x30, 1),
    ("RibbonChampionSinnoh", 0x30, 2),
    ("RibbonBestFriends", 0x30, 3),
    ("RibbonTraining", 0x30, 4),
    ("RibbonBattlerSkillful", 0x30, 5),
    ("RibbonBattlerExpert", 0x30, 6),
    ("RibbonEffort", 0x30, 7),
    ("RibbonAlert", 0x31, 0),
    ("RibbonShock", 0x31, 1),
    ("RibbonDowncast", 0x31, 2),
    ("RibbonCareless", 0x31, 3),
    ("RibbonRelax", 0x31, 4),
    ("RibbonSnooze", 0x31, 5),
    ("RibbonSmile", 0x31, 6),
    ("RibbonGorgeous", 0x31, 7),
    ("RibbonRoyal", 0x32, 0),
    ("RibbonGorgeousRoyal", 0x32, 1),
    ("RibbonArtist", 0x32, 2),
    ("RibbonFootprint", 0x32, 3),
    ("RibbonRecord", 0x32, 4),
    ("RibbonLegend", 0x32, 5),
    ("RibbonCountry", 0x32, 6),
    ("RibbonNational", 0x32, 7),
    ("RibbonEarth", 0x33, 0),
    ("RibbonWorld", 0x33, 1),
    ("RibbonClassic", 0x33, 2),
    ("RibbonPremier", 0x33, 3),
    ("RibbonEvent", 0x33, 4),
    ("RibbonBirthday", 0x33, 5),
    ("RibbonSpecial", 0x33, 6),
    ("RibbonSouvenir", 0x33, 7),
    ("RibbonWishing", 0x34, 0),
    ("RibbonChampionBattle", 0x34, 1),
    ("RibbonChampionRegional", 0x34, 2),
    ("RibbonChampionNational", 0x34, 3),
    ("RibbonChampionWorld", 0x34, 4),
    ("RibbonChampionG6Hoenn", 0x34, 7),
    ("RibbonContestStar", 0x35, 0),
    ("RibbonMasterCoolness", 0x35, 1),
    ("RibbonMasterBeauty", 0x35, 2),
    ("RibbonMasterCuteness", 0x35, 3),
    ("RibbonMasterCleverness", 0x35, 4),
    ("RibbonMasterToughness", 0x35, 5),
    ("RibbonChampionAlola", 0x35, 6),
    ("RibbonBattleRoyale", 0x35, 7),
    ("RibbonBattleTreeGreat", 0x36, 0),
    ("RibbonBattleTreeMaster", 0x36, 1),
];

/// Souvenir Gen 6/7 (texte, intensité, ressenti, variable : lieu, espèce, attaque ou objet).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Memory {
    pub id: u8,
    pub intensity: u8,
    pub feeling: u8,
    pub variable: u16,
}

/// Ruban et état.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RibbonState {
    pub key: &'static str,
    pub name: &'static str,
    pub on: bool,
}

/// Soigneur actuel, pays et souvenirs (Gen 6/7).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handler {
    /// Nom du dernier dresseur à l'avoir reçu par échange (vide = jamais échangé).
    pub name: String,
    pub gender: Gender,
    /// 0 : dresseur d'origine, 1 : soigneur (dernier dresseur l'ayant reçu).
    pub current: u8,
    pub friendship: u8,
    pub ot_affection: u8,
    pub ht_affection: u8,
    pub country: u8,
    pub region: u8,
    pub console_region: u8,
    /// Pays et régions visités, du plus récent au plus ancien : [région, pays].
    pub geo: [[u8; 2]; 5],
    pub fullness: u8,
    pub enjoyment: u8,
    pub ot_memory: Memory,
    pub ht_memory: Memory,
}

/// Médailles du Super Training (Gen 6/7).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuperTraining {
    /// 30 entraînements normaux puis 8 distribués (ordre de `dex::super_training_names`).
    pub medals: Vec<bool>,
    pub secret_unlocked: bool,
    pub supremely_trained: bool,
}

/// Champs secondaires de la fiche (absents quand ils n'existent pas dans le format).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PokemonExtras {
    /// Sang-froid, Beauté, Grâce, Intelligence, Robustesse, Lustre.
    pub contest: [u8; 6],
    pub ribbons: Vec<RibbonState>,
    /// Indice 0-29 dans `dex::characteristic_names`.
    pub characteristic: u8,
    pub ground_tile: Option<u8>,
    /// Gen 4 : feuilles brillantes (bits 0-4) et couronne (bit 5).
    pub shiny_leaf: Option<u8>,
    /// Gen 4 (HGSS) : humeur en promenade, de -127 à 127.
    pub walking_mood: Option<i8>,
    pub n_sparkle: Option<bool>,
    pub pokestar_fame: Option<u8>,
    /// Gen 6/7 : argument de forme (jours restants de Hoopa Déchaîné, de la coupe de Couafarel…).
    pub form_argument: Option<u32>,
    /// Gen 7 : statistiques entraînées par l'Hyper Training (ordre Kaleido).
    pub hyper_training: Option<[bool; 6]>,
    pub handler: Option<Handler>,
    pub super_training: Option<SuperTraining>,
}

/// Modifications des champs secondaires (absents = inchangés).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtrasPatch {
    pub contest: Option<[u8; 6]>,
    /// Clé PKHeX → posé ou non.
    pub ribbons: Option<BTreeMap<String, bool>>,
    pub ground_tile: Option<u8>,
    pub shiny_leaf: Option<u8>,
    pub walking_mood: Option<i8>,
    pub n_sparkle: Option<bool>,
    pub pokestar_fame: Option<u8>,
    pub form_argument: Option<u32>,
    pub hyper_training: Option<[bool; 6]>,
    pub ht_name: Option<String>,
    pub ht_gender: Option<Gender>,
    pub current_handler: Option<u8>,
    pub ht_friendship: Option<u8>,
    pub ot_affection: Option<u8>,
    pub ht_affection: Option<u8>,
    pub country: Option<u8>,
    pub region: Option<u8>,
    pub console_region: Option<u8>,
    pub geo: Option<[[u8; 2]; 5]>,
    pub fullness: Option<u8>,
    pub enjoyment: Option<u8>,
    pub ot_memory: Option<Memory>,
    pub ht_memory: Option<Memory>,
    pub medals: Option<Vec<bool>>,
    pub secret_unlocked: Option<bool>,
    pub supremely_trained: Option<bool>,
}

impl ExtrasPatch {
    /// Le patch touche-t-il un champ propre aux Gen 6/7 ?
    fn touches_gen67(&self) -> bool {
        self.form_argument.is_some()
            || self.ht_name.is_some()
            || self.ht_gender.is_some()
            || self.current_handler.is_some()
            || self.ht_friendship.is_some()
            || self.ot_affection.is_some()
            || self.ht_affection.is_some()
            || self.country.is_some()
            || self.region.is_some()
            || self.console_region.is_some()
            || self.geo.is_some()
            || self.fullness.is_some()
            || self.enjoyment.is_some()
            || self.ot_memory.is_some()
            || self.ht_memory.is_some()
            || self.medals.is_some()
            || self.secret_unlocked.is_some()
            || self.supremely_trained.is_some()
    }
}

const REGULAR_MEDALS: usize = 30;
const DIST_MEDALS: usize = 8;

/// Caractéristique (PKHeX `EntityCharacteristic.GetCharacteristic`) : la plus haute des IV,
/// en partant de la statistique `ec % 6` (ordre du jeu), puis IV % 5. Gen 5+ : « PV » si
/// toutes les IV valent 0 (`GetCharacteristicInit0`).
fn characteristic(ec: u32, iv32: u32, init0: bool) -> u8 {
    if init0 && iv32 & 0x3FFF_FFFF == 0 {
        return 0;
    }
    let mut index = (ec % 6) as usize;
    let mut max_index = index;
    let mut max_value = 0;
    loop {
        let v = (iv32 >> (index * 5)) & 0x1F;
        if v > max_value {
            max_index = index;
            max_value = v;
        }
        index = if index == 5 { 0 } else { index + 1 };
        if index == max_index {
            break;
        }
    }
    (max_index * 5 + (max_value as usize % 5)) as u8
}

impl Pokemon {
    fn ribbon_table(&self) -> &'static [(&'static str, usize, u8)] {
        match self.format {
            PkmFormat::Gen1 | PkmFormat::Gen2 | PkmFormat::Gen3 | PkmFormat::Gen4 | PkmFormat::Gen5 => &RIBBONS_DS,
            PkmFormat::Gen6 => &RIBBONS_PK6,
            PkmFormat::Gen7 => &RIBBONS_PK7,
        }
    }

    pub fn ribbons(&self) -> Vec<RibbonState> {
        self.ribbon_table()
            .iter()
            .filter(|&&(_, at, _)| self.ribbon_exists(at))
            .map(|&(key, at, bit)| RibbonState { key, name: dex::ribbon_name(key).unwrap_or(key), on: self.u8(at) >> bit & 1 != 0 })
            .collect()
    }

    /// Pose ou retire un ruban ; une clé inconnue dans ce format est refusée.
    pub fn set_ribbon(&mut self, key: &str, on: bool) -> Result<(), PkmError> {
        let &(_, at, bit) =
            self.ribbon_table().iter().filter(|&&(_, at, _)| self.ribbon_exists(at)).find(|(k, _, _)| *k == key).ok_or_else(|| PkmError::Invalid(format!("ruban inconnu dans ce format : {key}")))?;
        let v = self.u8(at) & !(1 << bit) | (on as u8) << bit;
        self.put_u8(at, v);
        Ok(())
    }

    /// Gen 3 : seuls les rubans de Hoenn (octets 0x3C à 0x3F du PK4) existent.
    fn ribbon_exists(&self, at: usize) -> bool {
        match self.format {
            // Pas de rubans en Gen 1 et 2.
            PkmFormat::Gen1 | PkmFormat::Gen2 => false,
            PkmFormat::Gen3 => (0x3C..=0x3F).contains(&at),
            _ => true,
        }
    }

    fn contest_offset(&self) -> usize {
        if self.format.is_ds() {
            G45_CONTEST
        } else {
            G67_CONTEST
        }
    }

    pub fn contest_stats(&self) -> [u8; 6] {
        let at = self.contest_offset();
        std::array::from_fn(|i| self.u8(at + i))
    }

    pub fn set_contest_stats(&mut self, v: [u8; 6]) {
        let at = self.contest_offset();
        for (i, x) in v.into_iter().enumerate() {
            self.put_u8(at + i, x);
        }
    }

    /// Indice de la caractéristique (Gen 4 : d'après le PID ; ensuite d'après l'EC).
    pub fn characteristic(&self) -> u8 {
        characteristic(self.encryption_constant(), self.u32(self.format.ofs().iv32), self.format != PkmFormat::Gen4)
    }

    fn ground_tile_offset(&self) -> Option<usize> {
        match self.format {
            PkmFormat::Gen1 | PkmFormat::Gen2 | PkmFormat::Gen3 => None,
            PkmFormat::Gen4 | PkmFormat::Gen5 => Some(G45_GROUND_TILE),
            PkmFormat::Gen6 => Some(G6_GROUND_TILE),
            PkmFormat::Gen7 => None,
        }
    }

    pub fn extras(&self) -> PokemonExtras {
        let gen67 = !self.format.is_ds();
        PokemonExtras {
            contest: self.contest_stats(),
            ribbons: self.ribbons(),
            characteristic: self.characteristic(),
            ground_tile: self.ground_tile_offset().map(|at| self.u8(at)),
            shiny_leaf: (self.format == PkmFormat::Gen4).then(|| self.u8(G4_SHINY_LEAF)),
            walking_mood: (self.format == PkmFormat::Gen4).then(|| self.u8(G45_BYTE_87) as i8),
            n_sparkle: (self.format == PkmFormat::Gen5).then(|| self.u8(G5_FLAGS) & 2 != 0),
            pokestar_fame: (self.format == PkmFormat::Gen5).then(|| self.u8(G45_BYTE_87)),
            form_argument: gen67.then(|| self.u32(G67_FORM_ARGUMENT)),
            hyper_training: (self.format == PkmFormat::Gen7).then(|| {
                let v = self.u8(G7_HYPER_TRAINING);
                HYPER_TRAINING_BITS.map(|b| v >> b & 1 != 0)
            }),
            handler: gen67.then(|| self.handler()),
            super_training: gen67.then(|| self.super_training()),
        }
    }

    /// Dresseur d'origine : intensité, souvenir, variable (u16), ressenti.
    /// Soigneur : intensité, souvenir, ressenti, (octet libre), variable (u16).
    fn memory(&self, at: usize, ot: bool) -> Memory {
        if ot {
            Memory { intensity: self.u8(at), id: self.u8(at + 1), variable: self.u16(at + 2), feeling: self.u8(at + 4) }
        } else {
            Memory { intensity: self.u8(at), id: self.u8(at + 1), feeling: self.u8(at + 2), variable: self.u16(at + 4) }
        }
    }

    fn set_memory(&mut self, at: usize, ot: bool, m: Memory) {
        self.put_u8(at, m.intensity);
        self.put_u8(at + 1, m.id);
        if ot {
            self.put_u16(at + 2, m.variable);
            self.put_u8(at + 4, m.feeling);
        } else {
            self.put_u8(at + 2, m.feeling);
            self.put_u16(at + 4, m.variable);
        }
    }

    pub fn ht_name(&self) -> String {
        strings::decode(self.format, &self.data[G67_HT_NAME..G67_HT_NAME + 2 * (HT_NAME_MAX + 1)])
    }

    fn handler(&self) -> Handler {
        Handler {
            name: self.ht_name(),
            gender: Gender::from_bits(self.u8(G67_HT_GENDER)),
            current: self.u8(G67_CURRENT_HANDLER),
            friendship: self.u8(G67_HT_FRIENDSHIP),
            ot_affection: self.u8(G67_OT_AFFECTION),
            ht_affection: self.u8(G67_HT_AFFECTION),
            country: self.u8(G67_COUNTRY),
            region: self.u8(G67_REGION),
            console_region: self.u8(G67_CONSOLE_REGION),
            geo: std::array::from_fn(|i| [self.u8(G67_GEO + 2 * i), self.u8(G67_GEO + 2 * i + 1)]),
            fullness: self.u8(G67_FULLNESS),
            enjoyment: self.u8(G67_ENJOYMENT),
            ot_memory: self.memory(G67_OT_MEMORY, true),
            ht_memory: self.memory(G67_HT_MEMORY, false),
        }
    }

    fn super_training(&self) -> SuperTraining {
        let regular = self.u32(G67_SUPER_TRAINING);
        let dist = self.u8(G67_DIST_TRAINING);
        let flags = self.u8(G67_TRAINING_FLAGS);
        SuperTraining {
            medals: (0..REGULAR_MEDALS).map(|i| regular >> (i + 2) & 1 != 0).chain((0..DIST_MEDALS).map(|i| dist >> i & 1 != 0)).collect(),
            secret_unlocked: flags & 1 != 0,
            supremely_trained: flags & 2 != 0,
        }
    }

    fn set_flag(&mut self, at: usize, mask: u8, on: bool) {
        let v = self.u8(at);
        self.put_u8(at, if on { v | mask } else { v & !mask });
    }

    /// Applique les champs fournis ; ceux qui n'existent pas dans le format sont refusés.
    pub fn apply_extras(&mut self, p: &ExtrasPatch) -> Result<(), PkmError> {
        let f = self.format;
        let missing = |what: &str| PkmError::Invalid(format!("{what} n'existe pas en Gen {}", f.generation()));
        let only = |ok: bool, what: &str| if ok { Ok(()) } else { Err(missing(what)) };
        if let Some(v) = p.contest {
            self.set_contest_stats(v);
        }
        if let Some(r) = &p.ribbons {
            for (key, &on) in r {
                self.set_ribbon(key, on)?;
            }
        }
        if let Some(v) = p.ground_tile {
            let at = self.ground_tile_offset().ok_or_else(|| missing("Le terrain de rencontre"))?;
            self.put_u8(at, v);
        }
        if let Some(v) = p.shiny_leaf {
            only(f == PkmFormat::Gen4, "Les feuilles brillantes")?;
            self.put_u8(G4_SHINY_LEAF, v & 0x3F);
        }
        if let Some(v) = p.walking_mood {
            only(f == PkmFormat::Gen4, "L'humeur de promenade")?;
            self.put_u8(G45_BYTE_87, v as u8);
        }
        if let Some(v) = p.n_sparkle {
            only(f == PkmFormat::Gen5, "L'éclat de N")?;
            self.set_flag(G5_FLAGS, 2, v);
        }
        if let Some(v) = p.pokestar_fame {
            only(f == PkmFormat::Gen5, "La renommée Pokéstar")?;
            self.put_u8(G45_BYTE_87, v);
        }
        if let Some(v) = p.hyper_training {
            only(f == PkmFormat::Gen7, "L'Hyper Training")?;
            let bits = v.iter().zip(HYPER_TRAINING_BITS).fold(self.u8(G7_HYPER_TRAINING) & !0x3F, |acc, (&on, b)| acc | (on as u8) << b);
            self.put_u8(G7_HYPER_TRAINING, bits);
        }
        if !p.touches_gen67() {
            return Ok(());
        }
        only(!f.is_ds(), "Ce champ (soigneur, pays, souvenirs, Super Training)")?;
        if let Some(v) = p.form_argument {
            self.put_u32(G67_FORM_ARGUMENT, v);
        }
        if let Some(name) = &p.ht_name {
            let mut bytes = if name.is_empty() { Vec::new() } else { strings::encode(f, name, HT_NAME_MAX)? };
            bytes.resize(2 * (HT_NAME_MAX + 1), 0);
            self.data[G67_HT_NAME..G67_HT_NAME + bytes.len()].copy_from_slice(&bytes);
        }
        if let Some(g) = p.ht_gender {
            self.put_u8(G67_HT_GENDER, (g == Gender::Female) as u8);
        }
        for (v, at) in [
            (p.current_handler.map(|v| v.min(1)), G67_CURRENT_HANDLER),
            (p.ht_friendship, G67_HT_FRIENDSHIP),
            (p.ot_affection, G67_OT_AFFECTION),
            (p.ht_affection, G67_HT_AFFECTION),
            (p.country, G67_COUNTRY),
            (p.region, G67_REGION),
            (p.console_region, G67_CONSOLE_REGION),
            (p.fullness, G67_FULLNESS),
            (p.enjoyment, G67_ENJOYMENT),
        ] {
            if let Some(v) = v {
                self.put_u8(at, v);
            }
        }
        if let Some(geo) = p.geo {
            for (i, [region, country]) in geo.into_iter().enumerate() {
                self.put_u8(G67_GEO + 2 * i, region);
                self.put_u8(G67_GEO + 2 * i + 1, country);
            }
        }
        if let Some(m) = p.ot_memory {
            self.set_memory(G67_OT_MEMORY, true, m);
        }
        if let Some(m) = p.ht_memory {
            self.set_memory(G67_HT_MEMORY, false, m);
        }
        if let Some(medals) = &p.medals {
            // Les bits 0-1 (inutilisés) et l'octet haut des médailles distribuées sont conservés.
            let mut regular = self.u32(G67_SUPER_TRAINING) & 0b11;
            let mut dist = 0u8;
            for (i, _) in medals.iter().enumerate().filter(|(_, on)| **on) {
                if i < REGULAR_MEDALS {
                    regular |= 1 << (i + 2);
                } else if i < REGULAR_MEDALS + DIST_MEDALS {
                    dist |= 1 << (i - REGULAR_MEDALS);
                }
            }
            self.put_u32(G67_SUPER_TRAINING, regular);
            self.put_u8(G67_DIST_TRAINING, dist);
        }
        if let Some(v) = p.secret_unlocked {
            self.set_flag(G67_TRAINING_FLAGS, 1, v);
        }
        if let Some(v) = p.supremely_trained {
            self.set_flag(G67_TRAINING_FLAGS, 2, v);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characteristic_matches_pkhex() {
        // IV toutes à 31 : on part de ec % 6 ; 31 % 5 = 1.
        let all31 = (0..6).fold(0u32, |acc, i| acc | 31 << (i * 5));
        assert_eq!(characteristic(0, all31, true), 1); // PV
        assert_eq!(characteristic(3, all31, true), 16); // Vitesse (ordre du jeu)
        // Seule l'Atq Spé (index 4 dans l'ordre du jeu) au maximum.
        assert_eq!(characteristic(0, 31 << 20, true), 21);
        assert_eq!(characteristic(5, 0, true), 0);
        assert_eq!(characteristic(5, 0, false), 25);
    }

    #[test]
    fn ribbon_tables_have_french_names() {
        for (key, _, _) in RIBBONS_DS.iter().chain(&RIBBONS_PK6).chain(&RIBBONS_PK7) {
            assert!(dex::ribbon_name(key).is_some(), "{key}");
        }
    }

    #[test]
    fn extras_round_trip_on_every_format() {
        for f in [PkmFormat::Gen4, PkmFormat::Gen5, PkmFormat::Gen6, PkmFormat::Gen7] {
            let mut p = Pokemon::blank(f);
            let key = p.ribbon_table()[3].0;
            let mut patch = ExtrasPatch { contest: Some([1, 2, 3, 4, 5, 6]), ribbons: Some([(key.to_string(), true)].into()), ..Default::default() };
            if !f.is_ds() {
                patch.ht_name = Some("Lilie".into());
                patch.country = Some(77);
                patch.region = Some(2);
                patch.ot_memory = Some(Memory { id: 4, intensity: 3, feeling: 5, variable: 300 });
                patch.ht_memory = Some(Memory { id: 6, intensity: 2, feeling: 1, variable: 1234 });
                patch.medals = Some((0..38).map(|i| i % 3 == 0).collect());
                patch.secret_unlocked = Some(true);
            }
            p.apply_extras(&patch).unwrap();
            let x = p.extras();
            assert_eq!(x.contest, [1, 2, 3, 4, 5, 6]);
            assert!(x.ribbons[3].on && x.ribbons.iter().filter(|r| r.on).count() == 1);
            if let Some(h) = x.handler {
                assert_eq!(h.name, "Lilie");
                assert_eq!((h.country, h.region), (77, 2));
                assert_eq!(h.ot_memory, Memory { id: 4, intensity: 3, feeling: 5, variable: 300 });
                assert_eq!(h.ht_memory, Memory { id: 6, intensity: 2, feeling: 1, variable: 1234 });
                let st = x.super_training.unwrap();
                assert_eq!(st.medals, (0..38).map(|i| i % 3 == 0).collect::<Vec<_>>());
                assert!(st.secret_unlocked && !st.supremely_trained);
            } else {
                assert!(p.apply_extras(&ExtrasPatch { country: Some(1), ..Default::default() }).is_err());
            }
        }
    }
}

