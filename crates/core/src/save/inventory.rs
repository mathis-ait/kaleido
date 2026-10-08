//! Sac du joueur (objets) des Gen 4 à 7.
//!
//! Portage de PKHeX (kwsch/PKHeX, GPLv3, comme Kaleido), fichiers de `PKHeX.Core` :
//!
//! - `Items/Bags/PlayerBag4DP.cs`, `PlayerBag4Pt.cs`, `PlayerBag4HGSS.cs`,
//!   `PlayerBag5BW.cs`, `PlayerBag5B2W2.cs`, `PlayerBag6XY.cs`, `PlayerBag6AO.cs`,
//!   `PlayerBag7SM.cs`, `PlayerBag7USUM.cs` : poches, offsets, quantités maximales ;
//! - `Items/ItemStorage4*.cs`, `ItemStorage5*.cs`, `ItemStorage6XY.cs`,
//!   `ItemStorage6AO.cs`, `ItemStorage7SM.cs`, `ItemStorage7USUM.cs` : objets admis
//!   dans chaque poche (et, hors Gen 7, nombre d'emplacements de la poche) ;
//! - `Saves/Substructures/Inventory/Pouch/InventoryPouch4.cs` et `InventoryPouch7.cs`,
//!   `Inventory/Item/InventoryItem7.cs` : encodage des emplacements ;
//! - `Saves/Access/SaveBlockAccessor5BW.cs`, `5B2W2.cs`, `6XY.cs`, `6AO.cs`, `7SM.cs`,
//!   `7USUM.cs` : bloc « MyItem / Inventory » (offset et taille) ;
//! - `Saves/SAV4.cs` : le sac Gen 4 est relatif au bloc « général » de la partition
//!   active (`General[BaseOffset..]`).
//!
//! Encodage d'un emplacement (4 octets) :
//!
//! - **Gen 4 à 6** : `[id u16][quantité u16]` ; aucun drapeau ;
//! - **Gen 7** : un `u32` : id sur 10 bits, quantité sur 10 bits (`>> 10`), indice
//!   « espace libre » sur 10 bits (`>> 20`), drapeau « nouveau » au bit 30, bit 31
//!   réservé. L'indice d'espace libre et le bit réservé sont conservés à l'écriture
//!   pour les objets déjà présents dans la poche.
//!
//! Base du sac (voir [`super::Layout`]) :
//!
//! | | Base | Taille du bloc | Source |
//! |---|---|---|---|
//! | DP | général + 0x624 | — | `PlayerBag4DP.BaseOffset` |
//! | Pt | général + 0x630 | — | `PlayerBag4Pt.BaseOffset` |
//! | HGSS | général + 0x644 | — | `PlayerBag4HGSS.BaseOffset` |
//! | NB | 0x18400 (bloc 25) | 0x9C0 | `SaveBlockAccessor5BW` |
//! | N2B2 | 0x18400 (bloc 25) | 0x9EC | `SaveBlockAccessor5B2W2` |
//! | XY | 0x400 (bloc 1) | 0xB88 | `SaveBlockAccessor6XY` |
//! | ROSA | 0x400 (bloc 1) | 0xB90 | `SaveBlockAccessor6AO` |
//! | SL | 0x0 (bloc 0) | 0xDE0 | `SaveBlockAccessor7SM` |
//! | USUL | 0x0 (bloc 0) | 0xE28 | `SaveBlockAccessor7USUM` |
//!
//! Comme dans PKHeX, une poche Gen 4 à 6 compte autant d'emplacements que d'objets
//! admis (la zone réservée par le jeu peut être un peu plus grande) ; en Gen 7, la
//! taille est donnée par `PlayerBag7*`.
//!
//! **Non vérifié sur de vraies sauvegardes** (aucune disponible) : testé sur des
//! sauvegardes synthétiques seulement.

use serde::{Deserialize, Serialize};

use super::{rd_u16, rd_u32, SaveError, SaveFile, SaveVersion};

/// Poche du sac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PouchKind {
    Items,
    KeyItems,
    TmHm,
    Medicine,
    Berries,
    Balls,
    BattleItems,
    Mail,
    ZCrystals,
    /// Rotom-Pouvoirs d'Ultra-Soleil/Ultra-Lune (`InventoryType.BattleItems` dans
    /// `PlayerBag7USUM`, distingué ici pour l'interface).
    RotoPowers,
}

impl PouchKind {
    /// Nom de la poche dans l'interface (français).
    pub fn label_fr(self) -> &'static str {
        match self {
            PouchKind::Items => "Objets",
            PouchKind::KeyItems => "Objets rares",
            PouchKind::TmHm => "CT & CS",
            PouchKind::Medicine => "Soins",
            PouchKind::Berries => "Baies",
            PouchKind::Balls => "Poké Balls",
            PouchKind::BattleItems => "Objets de combat",
            PouchKind::Mail => "Lettres",
            PouchKind::ZCrystals => "Cristaux Z",
            PouchKind::RotoPowers => "Rotom-Pouvoirs",
        }
    }
}

/// Objet d'une poche. Les drapeaux n'existent qu'en Gen 7 (« nouveau ») ; aucun jeu
/// Gen 4 à 7 ne stocke de favori : `is_favorite` vaut toujours `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    pub id: u16,
    pub count: u16,
    #[serde(default)]
    pub is_new: bool,
    #[serde(default)]
    pub is_favorite: bool,
}

/// Poche lue dans la sauvegarde, avec ses règles pour ce jeu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pouch {
    pub kind: PouchKind,
    /// Nombre d'emplacements.
    pub capacity: usize,
    /// Quantité maximale par objet (quelques exceptions : voir [`SaveFile::set_inventory`]).
    pub max_count: u16,
    /// Objets admis dans cette poche pour ce jeu.
    pub allowed: Vec<u16>,
    /// Objets présents, dans l'ordre du sac (sans emplacement vide).
    pub items: Vec<InventoryItem>,
}

/// Encodage des emplacements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    /// `[id u16][quantité u16]` (Gen 4 à 6, `InventoryPouch4`).
    Pair,
    /// Champ de bits `u32` (Gen 7, `InventoryPouch7` / `InventoryItem7`).
    Packed7,
    /// Gen 3 (`InventoryPouch3`) : `[id Gen 3 u16][quantité u16 XOR clé de sécurité]` ;
    /// l'id est exposé comme sur les Pokémon (voir `pk3::item_exposed`).
    Gen3,
}

/// Poche d'un jeu : offset relatif à la base du sac.
#[derive(Debug, Clone, Copy)]
struct PouchSpec {
    kind: PouchKind,
    offset: usize,
    capacity: usize,
    max_count: u16,
    allowed: &'static [u16],
}

/// Poche dont la taille est celle de la liste d'objets admis (Gen 4 à 6).
const fn sized(kind: PouchKind, offset: usize, max_count: u16, allowed: &'static [u16]) -> PouchSpec {
    PouchSpec { kind, offset, capacity: allowed.len(), max_count, allowed }
}

const fn fixed(kind: PouchKind, offset: usize, capacity: usize, max_count: u16, allowed: &'static [u16]) -> PouchSpec {
    PouchSpec { kind, offset, capacity, max_count, allowed }
}

use PouchKind as K;

/// `PlayerBag4DP.GetPouches`.
const DP: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, G4_GENERAL_DP),
    sized(K::KeyItems, 0x294, 1, G4_KEY),
    sized(K::TmHm, 0x35C, 99, G4_MACHINE),
    sized(K::Mail, 0x4EC, 999, G4_MAIL),
    sized(K::Medicine, 0x51C, 999, G4_MEDICINE),
    sized(K::Berries, 0x5BC, 999, G4_BERRY),
    sized(K::Balls, 0x6BC, 999, G4_BALLS_DPPT),
    sized(K::BattleItems, 0x6F8, 999, G4_BATTLE),
];

/// `PlayerBag4Pt.GetPouches`.
const PT: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, G4_GENERAL_PT),
    sized(K::KeyItems, 0x294, 1, G4_KEY_PT),
    sized(K::TmHm, 0x35C, 99, G4_MACHINE),
    sized(K::Mail, 0x4EC, 999, G4_MAIL),
    sized(K::Medicine, 0x51C, 999, G4_MEDICINE),
    sized(K::Berries, 0x5BC, 999, G4_BERRY),
    sized(K::Balls, 0x6BC, 999, G4_BALLS_DPPT),
    sized(K::BattleItems, 0x6F8, 999, G4_BATTLE),
];

/// `PlayerBag4HGSS.GetPouches` (objets : `ItemStorage4HGSS` reprend `GeneralPt`).
const HGSS: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, G4_GENERAL_PT),
    sized(K::KeyItems, 0x294, 1, G4_KEY_HGSS),
    sized(K::TmHm, 0x35C, 99, G4_MACHINE),
    sized(K::Mail, 0x4F0, 999, G4_MAIL),
    sized(K::Medicine, 0x520, 999, G4_MEDICINE),
    sized(K::Berries, 0x5C0, 999, G4_BERRY),
    sized(K::Balls, 0x6C0, 999, G4_BALLS_HGSS),
    sized(K::BattleItems, 0x720, 999, G4_BATTLE),
];

/// `PlayerBag5BW.GetPouches`.
const BW: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, G5_GENERAL),
    sized(K::KeyItems, 0x4D8, 1, BW_KEY),
    sized(K::TmHm, 0x624, 1, G5_MACHINE),
    sized(K::Medicine, 0x7D8, 999, G5_MEDICINE),
    sized(K::Berries, 0x898, 999, G5_BERRY),
];

/// `PlayerBag5B2W2.GetPouches`.
const B2W2: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, G5_GENERAL),
    sized(K::KeyItems, 0x4D8, 1, B2W2_KEY),
    sized(K::TmHm, 0x624, 1, G5_MACHINE),
    sized(K::Medicine, 0x7D8, 999, G5_MEDICINE),
    sized(K::Berries, 0x898, 999, G5_BERRY),
];

/// `PlayerBag6XY.GetPouches`.
const XY: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, XY_GENERAL),
    sized(K::KeyItems, 0x640, 1, XY_KEY),
    sized(K::TmHm, 0x7C0, 1, XY_MACHINE),
    sized(K::Medicine, 0x968, 999, XY_MEDICINE),
    sized(K::Berries, 0xA68, 999, XY_BERRY),
];

/// `PlayerBag6AO.GetPouches` (baies : `ItemStorage6AO` reprend celles de X/Y).
const AO: &[PouchSpec] = &[
    sized(K::Items, 0x000, 999, AO_GENERAL),
    sized(K::KeyItems, 0x640, 1, AO_KEY),
    sized(K::TmHm, 0x7C0, 1, AO_MACHINE),
    sized(K::Medicine, 0x970, 999, AO_MEDICINE),
    sized(K::Berries, 0xA70, 999, XY_BERRY),
];

/// `PlayerBag7SM.GetPouches` (ordre des poches dans le bloc, pas celui du menu).
const SM: &[PouchSpec] = &[
    fixed(K::Items, 0x000, 430, 999, SM_GENERAL),
    fixed(K::Medicine, 0xB48, 64, 999, SM_MEDICINE),
    fixed(K::TmHm, 0x998, 108, 1, SM_MACHINE),
    fixed(K::Berries, 0xC48, 72, 999, SM_BERRY),
    fixed(K::KeyItems, 0x6B8, 184, 1, SM_KEY),
    fixed(K::ZCrystals, 0xD68, 30, 1, SM_ZCRYSTAL_KEY),
];

/// `PlayerBag7USUM.GetPouches` (objets, CT, soins et baies : listes de Soleil/Lune).
const USUM: &[PouchSpec] = &[
    fixed(K::Items, 0x000, 427, 999, SM_GENERAL),
    fixed(K::Medicine, 0xB74, 60, 999, SM_MEDICINE),
    fixed(K::TmHm, 0x9C4, 108, 1, SM_MACHINE),
    fixed(K::Berries, 0xC64, 67, 999, SM_BERRY),
    fixed(K::KeyItems, 0x6AC, 198, 1, USUM_KEY),
    fixed(K::ZCrystals, 0xD70, 35, 1, USUM_ZCRYSTAL_KEY),
    fixed(K::RotoPowers, 0xDFC, 11, 999, USUM_ROTO),
];

/// `PlayerBag3RS.GetPouches` (sans les objets du PC).
const RS3: &[PouchSpec] = &[
    fixed(K::Items, 0x0C8, 20, 99, G3_GENERAL),
    fixed(K::KeyItems, 0x118, 20, 1, G3_KEY_RS),
    fixed(K::Balls, 0x168, 16, 99, G3_BALLS),
    fixed(K::TmHm, 0x1A8, 64, 99, G3_MACHINE),
    fixed(K::Berries, 0x2A8, 46, 999, G3_BERRY),
];

/// `PlayerBag3E.GetPouches`.
const E3: &[PouchSpec] = &[
    fixed(K::Items, 0x0C8, 30, 99, G3_GENERAL),
    fixed(K::KeyItems, 0x140, 30, 1, G3_KEY_E),
    fixed(K::Balls, 0x1B8, 16, 99, G3_BALLS),
    fixed(K::TmHm, 0x1F8, 64, 99, G3_MACHINE),
    fixed(K::Berries, 0x2F8, 46, 999, G3_BERRY),
];

/// `PlayerBag3FRLG.GetPouches`.
const FRLG3: &[PouchSpec] = &[
    fixed(K::Items, 0x078, 42, 999, G3_GENERAL),
    fixed(K::KeyItems, 0x120, 30, 1, G3_KEY_FRLG),
    fixed(K::Balls, 0x198, 13, 999, G3_BALLS),
    fixed(K::TmHm, 0x1CC, 58, 999, G3_MACHINE),
    fixed(K::Berries, 0x2B4, 43, 999, G3_BERRY),
];

fn bag(version: SaveVersion) -> (Encoding, &'static [PouchSpec]) {
    match version {
        // Sac Gen 1 / 2 non géré : aucune poche.
        SaveVersion::RedBlue | SaveVersion::Yellow | SaveVersion::GoldSilver | SaveVersion::Crystal => (Encoding::Pair, &[]),
        SaveVersion::RubySapphire => (Encoding::Gen3, RS3),
        SaveVersion::Emerald => (Encoding::Gen3, E3),
        SaveVersion::FireRedLeafGreen => (Encoding::Gen3, FRLG3),
        SaveVersion::DiamondPearl => (Encoding::Pair, DP),
        SaveVersion::Platinum => (Encoding::Pair, PT),
        SaveVersion::HeartGoldSoulSilver => (Encoding::Pair, HGSS),
        SaveVersion::BlackWhite => (Encoding::Pair, BW),
        SaveVersion::Black2White2 => (Encoding::Pair, B2W2),
        SaveVersion::XY => (Encoding::Pair, XY),
        SaveVersion::OmegaRubyAlphaSapphire => (Encoding::Pair, AO),
        SaveVersion::SunMoon => (Encoding::Packed7, SM),
        SaveVersion::UltraSunUltraMoon => (Encoding::Packed7, USUM),
    }
}

/// Anneau Z : 2 exemplaires possibles (prêté puis rendu par Pectorius),
/// `PlayerBag7SM/USUM.ItemIndexZRing`.
const Z_RING: u16 = 797;

/// Quantité maximale d'un objet précis (`PlayerBag*.GetMaxCount`) : les CS Gen 4
/// (`ItemConverter.IsItemHM4`) sont limitées à 1, l'Anneau Z Gen 7 à 2.
fn max_count_for(encoding: Encoding, spec: &PouchSpec, id: u16) -> u16 {
    match (encoding, spec.kind) {
        (Encoding::Pair, PouchKind::TmHm) if spec.max_count > 1 && ((420..=427).contains(&id) || id == 737) => 1,
        (Encoding::Packed7, PouchKind::KeyItems) if id == Z_RING => 2,
        // CS de la Gen 3 (`ItemConverter.IsItemHM3`).
        (Encoding::Gen3, PouchKind::TmHm) if (339..=346).contains(&super::pk3::item_raw(id)) => 1,
        _ => spec.max_count,
    }
}

const ID_MASK: u32 = 0x3FF;
const NEW_FLAG: u32 = 0x4000_0000;
/// Bits conservés tels quels en Gen 7 : indice d'espace libre (20 à 29) et bit 31.
const KEEP_MASK: u32 = (0x3FF << 20) | 0x8000_0000;

/// Lit un emplacement : `(objet, bits Gen 7 à conserver)`. `key` : clé de sécurité Gen 3.
fn read_slot(encoding: Encoding, key: u16, d: &[u8], at: usize) -> (InventoryItem, u32) {
    match encoding {
        Encoding::Gen3 => {
            let item = InventoryItem { id: super::pk3::item_exposed(rd_u16(d, at)), count: rd_u16(d, at + 2) ^ key, is_new: false, is_favorite: false };
            (item, 0)
        }
        Encoding::Pair => {
            let item = InventoryItem { id: rd_u16(d, at), count: rd_u16(d, at + 2), is_new: false, is_favorite: false };
            (item, 0)
        }
        Encoding::Packed7 => {
            let v = rd_u32(d, at);
            let item = InventoryItem { id: (v & ID_MASK) as u16, count: ((v >> 10) & ID_MASK) as u16, is_new: v & NEW_FLAG != 0, is_favorite: false };
            (item, v & KEEP_MASK)
        }
    }
}

fn encode_slot(encoding: Encoding, key: u16, item: &InventoryItem, keep: u32) -> [u8; 4] {
    match encoding {
        Encoding::Gen3 => {
            let mut b = [0u8; 4];
            b[..2].copy_from_slice(&super::pk3::item_raw(item.id).to_le_bytes());
            b[2..].copy_from_slice(&(item.count ^ key).to_le_bytes());
            b
        }
        Encoding::Pair => {
            let mut b = [0u8; 4];
            b[..2].copy_from_slice(&item.id.to_le_bytes());
            b[2..].copy_from_slice(&item.count.to_le_bytes());
            b
        }
        Encoding::Packed7 => {
            let mut v = (item.id as u32 & ID_MASK) | ((item.count as u32 & ID_MASK) << 10) | keep;
            if item.is_new {
                v |= NEW_FLAG;
            }
            v.to_le_bytes()
        }
    }
}

/// Emplacement occupé ? Hors Gen 7, une quantité nulle vaut un emplacement vide ; en
/// Gen 7, le jeu peut garder un objet à 0 (PKHeX l'admet s'il gère « nouveau »).
fn occupied(encoding: Encoding, item: &InventoryItem) -> bool {
    item.id != 0 && (encoding == Encoding::Packed7 || item.count != 0)
}

impl SaveFile {
    fn pouch_range(&self, spec: &PouchSpec) -> Result<usize, SaveError> {
        let start = self.layout.items + spec.offset;
        if start + 4 * spec.capacity > self.data.len() {
            return Err(SaveError::Truncated("sac"));
        }
        Ok(start)
    }

    /// Emplacements occupés d'une poche, avec les bits Gen 7 à conserver.
    fn read_pouch(&self, encoding: Encoding, spec: &PouchSpec) -> Result<Vec<(InventoryItem, u32)>, SaveError> {
        let start = self.pouch_range(spec)?;
        let key = self.bag_key(encoding);
        Ok((0..spec.capacity).map(|i| read_slot(encoding, key, &self.data, start + 4 * i)).filter(|(item, _)| occupied(encoding, item)).collect())
    }

    /// Clé de sécurité des quantités (Gen 3 : 16 bits de poids faible de la clé ; 0 sinon).
    fn bag_key(&self, encoding: Encoding) -> u16 {
        if encoding == Encoding::Gen3 {
            super::gen3::security_key(self.version, &self.data) as u16
        } else {
            0
        }
    }

    /// Poches du sac, dans l'ordre où le jeu les stocke.
    pub fn inventory(&self) -> Result<Vec<Pouch>, SaveError> {
        let (encoding, specs) = bag(self.version);
        specs
            .iter()
            .map(|spec| {
                let items = self.read_pouch(encoding, spec)?.into_iter().map(|(item, _)| item).collect();
                let allowed = if encoding == Encoding::Gen3 { spec.allowed.iter().map(|&i| super::pk3::item_exposed(i)).collect() } else { spec.allowed.to_vec() };
                Ok(Pouch { kind: spec.kind, capacity: spec.capacity, max_count: spec.max_count, allowed, items })
            })
            .collect()
    }

    /// Réécrit les poches données (identifiées par `kind` ; les autres ne changent
    /// pas). Seuls `kind` et `items` sont lus : capacité, maximum et objets admis sont
    /// ceux du jeu. Les objets d'id 0 sont ignorés, ainsi que (hors Gen 7) ceux de
    /// quantité nulle.
    ///
    /// Refusé si : poche absente du jeu ou donnée deux fois, objet non admis dans la
    /// poche (sauf s'il y est déjà dans la sauvegarde), objet en double, quantité
    /// au-delà du maximum (1 pour les CS Gen 4, 2 pour l'Anneau Z Gen 7), plus
    /// d'objets que d'emplacements. Rien n'est écrit en cas d'erreur.
    ///
    /// L'écriture est compactée (pas de trou, reste de la poche remis à zéro). En
    /// Gen 7, le drapeau « nouveau » vient de `is_new` ; l'indice d'espace libre des
    /// objets déjà présents est conservé.
    pub fn set_inventory(&mut self, pouches: &[Pouch]) -> Result<(), SaveError> {
        let (encoding, specs) = bag(self.version);
        let mut writes: Vec<(usize, Vec<u8>)> = Vec::with_capacity(pouches.len());
        for (n, pouch) in pouches.iter().enumerate() {
            let label = pouch.kind.label_fr();
            let spec = specs
                .iter()
                .find(|s| s.kind == pouch.kind)
                .ok_or_else(|| SaveError::Invalid(format!("La poche « {label} » n'existe pas dans {}.", self.version.label())))?;
            if pouches[..n].iter().any(|p| p.kind == pouch.kind) {
                return Err(SaveError::Invalid(format!("Poche « {label} » donnée deux fois.")));
            }
            let start = self.pouch_range(spec)?;
            let current = self.read_pouch(encoding, spec)?;

            let items: Vec<&InventoryItem> = pouch.items.iter().filter(|item| occupied(encoding, item)).collect();
            if items.len() > spec.capacity {
                return Err(SaveError::Invalid(format!("Poche « {label} » : {} objets pour {} emplacements.", items.len(), spec.capacity)));
            }
            let mut bytes = vec![0u8; 4 * spec.capacity];
            for (i, item) in items.iter().enumerate() {
                let previous = current.iter().find(|(c, _)| c.id == item.id);
                let raw_id = if encoding == Encoding::Gen3 { super::pk3::item_raw(item.id) } else { item.id };
                if !spec.allowed.contains(&raw_id) && previous.is_none() {
                    return Err(SaveError::Invalid(format!("Poche « {label} » : l'objet n°{} n'y est pas admis.", item.id)));
                }
                if items[..i].iter().any(|other| other.id == item.id) {
                    return Err(SaveError::Invalid(format!("Poche « {label} » : l'objet n°{} est en double.", item.id)));
                }
                let max = max_count_for(encoding, spec, item.id);
                if item.count > max {
                    return Err(SaveError::Invalid(format!(
                        "Poche « {label} » : {} exemplaires de l'objet n°{} (maximum {max}).",
                        item.count, item.id
                    )));
                }
                let keep = previous.map_or(0, |(_, keep)| *keep);
                bytes[4 * i..4 * i + 4].copy_from_slice(&encode_slot(encoding, self.bag_key(encoding), item, keep));
            }
            writes.push((start, bytes));
        }
        for (start, bytes) in writes {
            self.data[start..start + bytes.len()].copy_from_slice(&bytes);
        }
        Ok(())
    }
}

// --- Objets admis par poche : listes `ReadOnlySpan<ushort>` des `ItemStorage*` de
// PKHeX, recopiées telles quelles (même ordre).

/// `ItemStorage3RS.General` (identifiants Gen 3).
const G3_GENERAL: &[u16] = &[
    13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    49, 50, 51, 63, 64, 65, 66, 67, 68, 69, 70, 71, 73, 74, 75, 76, 77, 78, 79, 80, 81, 83, 84, 85, 86, 93, 94, 95, 96, 97, 98, 103, 104, 106, 107,
    108, 109, 110, 111, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190,
    191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218,
    219, 220, 221, 222, 223, 224, 225, 254, 255, 256, 257, 258,
];
/// `ItemStorage3RS.Key`.
const G3_KEY_RS: &[u16] = &[259, 260, 261, 262, 263, 264, 265, 266, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288];
/// `ItemStorage3E.Key`.
const G3_KEY_E: &[u16] = &[259, 260, 261, 262, 263, 264, 265, 266, 268, 269, 270, 271, 272, 273, 274, 275, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 370, 371, 372, 375, 376];
/// `ItemStorage3FRLG.Key`.
const G3_KEY_FRLG: &[u16] = &[260, 261, 262, 263, 264, 265, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374];
/// `ItemStorage3RS.Machine` (CT01 à CT50 puis CS01 à CS08).
const G3_MACHINE: &[u16] = &[
    289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317,
    318, 319, 320, 321, 322, 323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346,
];
/// `ItemStorage3RS.Berry`.
const G3_BERRY: &[u16] = &[
    133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161,
    162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175,
];
/// `ItemStorage3RS.Balls`.
const G3_BALLS: &[u16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// `ItemStorage4.GeneralDP` (161 objets).
const G4_GENERAL_DP: &[u16] = &[
    68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102,
    103, 104, 105, 106, 107, 108, 109, 110, 111, 135, 136, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230,
    231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259,
    260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288,
    289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317,
    318, 319, 320, 321, 322, 323, 324, 325, 326, 327,
];

/// `ItemStorage4.GeneralPt` (162 objets).
const G4_GENERAL_PT: &[u16] = &[
    68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102,
    103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 135, 136, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229,
    230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258,
    259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287,
    288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316,
    317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 327,
];

/// `ItemStorage4.Key` (37 objets).
const G4_KEY: &[u16] = &[
    428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 456,
    457, 458, 459, 460, 461, 462, 463, 464,
];

/// `ItemStorage4.Machine` (100 objets).
const G4_MACHINE: &[u16] = &[
    328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385,
    386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414,
    415, 416, 417, 418, 419, 420, 421, 422, 423, 424, 425, 426, 427,
];

/// `ItemStorage4.Mail` (12 objets).
const G4_MAIL: &[u16] = &[137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148];

/// `ItemStorage4.Medicine` (38 objets).
const G4_MEDICINE: &[u16] = &[
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54,
];

/// `ItemStorage4.Berry` (64 objets).
const G4_BERRY: &[u16] = &[
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177,
    178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
    207, 208, 209, 210, 211, 212,
];

/// `ItemStorage4.BallsDPPt` (15 objets).
const G4_BALLS_DPPT: &[u16] = &[1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

/// `ItemStorage4.Battle` (13 objets).
const G4_BATTLE: &[u16] = &[55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67];

/// `ItemStorage4Pt.KeyPt` (40 objets).
const G4_KEY_PT: &[u16] = &[
    428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447, 448, 449, 450, 451, 452, 453, 454, 455, 456,
    457, 458, 459, 460, 461, 462, 463, 464, 465, 466, 467,
];

/// `ItemStorage4HGSS.KeyHGSS` (38 objets).
const G4_KEY_HGSS: &[u16] = &[
    434, 435, 437, 444, 445, 446, 447, 450, 456, 464, 465, 466, 468, 469, 470, 471, 472, 473, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484,
    501, 502, 503, 504, 532, 533, 534, 535, 536,
];

/// `ItemStorage4HGSS.BallsHGSS` (24 objets).
const G4_BALLS_HGSS: &[u16] = &[1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 492, 493, 494, 495, 496, 497, 498, 499, 500];

/// `ItemStorage5.General` (261 objets).
const G5_GENERAL: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76,
    77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109,
    110, 111, 112, 116, 117, 118, 119, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 213, 214, 215, 216, 217, 218, 219, 220,
    221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249,
    250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278,
    279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307,
    308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 327, 492, 493, 494, 495, 496, 497, 498, 499, 500,
    537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 571,
    572, 573, 575, 576, 577, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590,
];

/// `ItemStorage5.Machine` (101 objets).
const G5_MACHINE: &[u16] = &[
    328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385,
    386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414,
    415, 416, 417, 418, 419, 618, 619, 620, 420, 421, 422, 423, 424, 425,
];

/// `ItemStorage5.Medicine` (47 objets).
const G5_MEDICINE: &[u16] = &[
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 134, 504, 565, 566, 567, 568, 569, 570, 591,
];

/// `ItemStorage5.Berry` (64 objets).
const G5_BERRY: &[u16] = &[
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177,
    178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
    207, 208, 209, 210, 211, 212,
];

/// `ItemStorage5BW.Key` (19 objets).
const BW_KEY: &[u16] = &[437, 442, 447, 450, 465, 466, 471, 504, 533, 574, 578, 579, 616, 617, 621, 623, 624, 625, 626];

/// `ItemStorage5B2W2.Key` (27 objets).
const B2W2_KEY: &[u16] =
    &[437, 442, 447, 450, 453, 458, 465, 466, 471, 504, 578, 616, 617, 621, 626, 627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 638];

/// `ItemStorage6XY.General` (289 objets).
const XY_GENERAL: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76,
    77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 112, 116, 117,
    118, 119, 135, 136, 213, 214, 215, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238,
    239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267,
    268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296,
    297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322, 323, 324, 325,
    326, 327, 492, 493, 494, 495, 496, 497, 498, 499, 500, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554,
    555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 571, 572, 573, 576, 577, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 639, 640, 644,
    646, 647, 648, 649, 650, 652, 653, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674, 675,
    676, 677, 678, 679, 680, 681, 682, 683, 684, 685, 699, 704, 710, 711, 715,
];

/// `ItemStorage6XY.Key` (32 objets).
const XY_KEY: &[u16] = &[
    216, 431, 442, 445, 446, 447, 450, 465, 466, 471, 628, 629, 631, 632, 638, 641, 642, 643, 651, 689, 695, 696, 697, 698, 700, 701, 702, 703, 705,
    712, 713, 714,
];

/// `ItemStorage6XY.Machine` (105 objets).
const XY_MACHINE: &[u16] = &[
    328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385,
    386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414,
    415, 416, 417, 418, 419, 618, 619, 620, 690, 691, 692, 693, 694, 420, 421, 422, 423, 424,
];

/// `ItemStorage6XY.Medicine` (51 objets).
const XY_MEDICINE: &[u16] = &[
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 134, 504, 565, 566, 567, 568, 569, 570, 571, 591, 645, 708, 709,
];

/// `ItemStorage6XY.Berry` (67 objets).
const XY_BERRY: &[u16] = &[
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177,
    178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
    207, 208, 209, 210, 211, 212, 686, 687, 688,
];

/// `ItemStorage6AO.Machine` (107 objets).
const AO_MACHINE: &[u16] = &[
    328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385,
    386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414,
    415, 416, 417, 418, 419, 618, 619, 620, 690, 691, 692, 693, 694, 420, 421, 422, 423, 424, 425, 737,
];

/// `ItemStorage6AO.General` (305 objets).
const AO_GENERAL: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
    80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 112, 116, 117, 118, 119,
    135, 136, 213, 214, 215, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240,
    241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267, 268, 269,
    270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298,
    299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322, 323, 324, 325, 326, 327,
    492, 493, 494, 495, 496, 497, 498, 499, 500, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556,
    557, 558, 559, 560, 561, 562, 563, 564, 571, 572, 573, 576, 577, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 639, 640, 644, 646, 647,
    648, 649, 650, 652, 653, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674, 675, 676, 677,
    678, 679, 680, 681, 682, 683, 684, 685, 699, 704, 710, 711, 715, 534, 535, 752, 753, 754, 755, 756, 757, 758, 759, 760, 761, 762, 763, 764, 767,
    768, 769, 770,
];

/// `ItemStorage6AO.Key` (47 objets).
const AO_KEY: &[u16] = &[
    216, 445, 446, 447, 465, 466, 471, 628, 629, 631, 632, 638, 697, 457, 474, 503, 718, 719, 720, 721, 722, 724, 725, 726, 727, 728, 729, 730, 731,
    732, 733, 734, 735, 736, 738, 739, 740, 741, 742, 743, 744, 751, 765, 771, 772, 774, 775,
];

/// `ItemStorage6AO.Medicine` (54 objets).
const AO_MEDICINE: &[u16] = &[
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 134, 504, 565, 566, 567, 568, 569, 570, 571, 591, 645, 708, 709, 65, 66, 67,
];

/// `ItemStorage7SM.General` (335 objets).
const SM_GENERAL: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
    80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 116, 117, 118,
    119, 135, 136, 137, 213, 214, 215, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238,
    239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259, 260, 261, 262, 263, 264, 265, 266, 267,
    268, 269, 270, 271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 293, 294, 295, 296,
    297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321, 322, 323, 324, 325,
    326, 327, 492, 493, 494, 495, 496, 497, 498, 499, 534, 535, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547, 548, 549, 550, 551, 552, 553,
    554, 555, 556, 557, 558, 559, 560, 561, 562, 563, 564, 571, 572, 573, 576, 577, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 639, 640,
    644, 646, 647, 648, 649, 650, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674, 675, 676, 677, 678,
    679, 680, 681, 682, 683, 684, 685, 699, 704, 710, 711, 715, 752, 753, 754, 755, 756, 757, 758, 759, 760, 761, 762, 763, 764, 767, 768, 769, 770,
    795, 796, 844, 846, 849, 851, 853, 854, 855, 856, 879, 880, 881, 882, 883, 884, 904, 905, 906, 907, 908, 909, 910, 911, 912, 913, 914, 915, 916,
    917, 918, 919, 920,
];

/// `ItemStorage7SM.Key` (22 objets).
const SM_KEY: &[u16] = &[216, 465, 466, 628, 629, 631, 632, 638, 705, 706, 765, 773, 797, 841, 842, 843, 845, 847, 850, 857, 858, 860];

/// `ItemStorage7SM.Machine` (100 objets).
const SM_MACHINE: &[u16] = &[
    328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 378, 379, 380, 381, 382, 383, 384, 385,
    386, 387, 388, 389, 390, 391, 392, 393, 394, 395, 396, 397, 398, 399, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414,
    415, 416, 417, 418, 419, 618, 619, 620, 690, 691, 692, 693, 694,
];

/// `ItemStorage7SM.Medicine` (54 objets).
const SM_MEDICINE: &[u16] = &[
    17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
    53, 54, 65, 66, 67, 134, 504, 565, 566, 567, 568, 569, 570, 591, 645, 708, 709, 852,
];

/// `ItemStorage7SM.Berry` (67 objets).
const SM_BERRY: &[u16] = &[
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177,
    178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
    207, 208, 209, 210, 211, 212, 686, 687, 688,
];

/// `ItemStorage7SM.ZCrystalKey` (29 objets).
const SM_ZCRYSTAL_KEY: &[u16] = &[
    807, 808, 809, 810, 811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 822, 823, 824, 825, 826, 827, 828, 829, 830, 831, 832, 833, 834, 835,
];

/// `ItemStorage7USUM.Key` (39 objets).
const USUM_KEY: &[u16] = &[
    216, 440, 465, 466, 628, 629, 631, 632, 638, 705, 706, 765, 773, 797, 841, 842, 843, 845, 847, 850, 857, 858, 860, 933, 934, 935, 936, 937, 938,
    939, 940, 941, 942, 943, 944, 945, 946, 947, 948,
];

/// `ItemStorage7USUM.Roto` (11 objets).
const USUM_ROTO: &[u16] = &[949, 950, 951, 952, 953, 954, 955, 956, 957, 958, 959];

/// `ItemStorage7USUM.ZCrystalKey` (35 objets).
const USUM_ZCRYSTAL_KEY: &[u16] = &[
    807, 808, 809, 810, 811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 822, 823, 824, 825, 826, 827, 828, 829, 830, 831, 832, 833, 834, 835,
    927, 928, 929, 930, 931, 932,
];

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
