//! Sauvegardes Gen 3 : Rubis / Saphir, Émeraude, Rouge Feu / Vert Feuille (`SAV3*` de PKHeX).
//!
//! Fichier de 128 Kio (64 Kio pour certains émulateurs mal réglés) : deux emplacements
//! de 14 secteurs de 4 Kio, puis 4 secteurs « extra » (Temple de la Renommée…). Chaque
//! secteur porte 0xF80 octets de données et, à la fin, `[id u16 @0xFF4][somme u16 @0xFF6]
//! [signature 0x08012025 @0xFF8][compteur u32 @0xFFC]`. Le jeu écrit à tour de rôle dans
//! l'un ou l'autre emplacement, en faisant tourner les secteurs : le plus récent est
//! celui dont le secteur 0 a le plus grand compteur (`SAV3BlockDetection`).
//!
//! Kaleido reconstitue les données dans l'ordre des identifiants (14 × 0xF80 octets) :
//! « petit » bloc (id 0, dresseur, Pokédex), « grand » bloc (ids 1 à 4, équipe, sac,
//! argent, drapeaux) et boîtes (ids 5 à 13). À l'écriture, chaque secteur de
//! l'emplacement actif reçoit la tranche correspondante et sa somme est recalculée
//! (somme des u32 des 0xF80 octets, repliée sur 16 bits : `Checksums.CheckSum32`).
//!
//! Clé de sécurité (Émeraude : petit bloc + 0xAC ; Rouge Feu / Vert Feuille : + 0xF20) :
//! l'argent et les quantités du sac (hors PC) sont stockés XOR cette clé.
//!
//! | | Équipe | Argent | Sac | Drapeaux | Badges (drapeau) |
//! |---|---|---|---|---|---|
//! | RS | grand + 0x234 | + 0x490 | + 0x498 | + 0x1220 | 0x807 |
//! | E | grand + 0x234 | + 0x490 | + 0x498 | + 0x1270 | 0x867 |
//! | RFVF | grand + 0x034 | + 0x290 | + 0x298 | + 0xEE0 | 0x820 |
//!
//! Vérifié sur le code de PKHeX (`SAV3.cs`, `SAV3E.cs`, `SAV3FRLG.cs`, `SAV3RS.cs`,
//! `SaveBlock3Small*.cs`, `SaveBlock3Large*.cs`, `PlayerBag3*.cs`, `SaveUtil.GetVersionG3SAV`).
//! **Testé uniquement sur des sauvegardes synthétiques** (aucune vraie sauvegarde Gen 3).

use super::{rd_u16, rd_u32, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout};

pub const FULL_SIZE: usize = 0x20000;
pub const HALF_SIZE: usize = 0x10000;
/// Pied d'horloge (RTC) que mGBA ajoute après les 128 Kio de Rubis / Saphir / Émeraude.
pub const RTC_FOOTER: usize = 0x10;
const SECTOR: usize = 0x1000;
pub(super) const USED: usize = 0xF80;
const MAIN_SECTORS: usize = 14;
const SLOT_SIZE: usize = MAIN_SECTORS * SECTOR;
const SIGNATURE: u32 = 0x0801_2025;
/// Grand bloc et boîtes dans les données reconstituées.
pub(super) const LARGE: usize = USED;
pub(super) const STORAGE: usize = 5 * USED;
/// Taille des données reconstituées, plus un octet de langue (hors fichier, voir `layout`).
pub(super) const LOGICAL_SIZE: usize = MAIN_SECTORS * USED;
const LANGUAGE_AT: usize = LOGICAL_SIZE;

const BOX_COUNT: usize = 14;
const BOX_NAME_LEN: usize = 9;

/// Somme d'un secteur (`Checksums.CheckSum32` de PKHeX).
pub fn checksum32(data: &[u8]) -> u16 {
    let sum = data.chunks(4).fold(0u32, |acc, c| acc.wrapping_add(u32::from_le_bytes([c[0], c[1], c[2], c[3]])));
    (sum.wrapping_add(sum >> 16)) as u16
}

/// Taille utile d'un fichier (sans le pied RTC de mGBA).
pub fn body_size(len: usize) -> Option<usize> {
    match len {
        FULL_SIZE | HALF_SIZE => Some(len),
        l if l == FULL_SIZE + RTC_FOOTER => Some(FULL_SIZE),
        _ => None,
    }
}

/// Les 14 secteurs d'un emplacement sont-ils tous là ? Renvoie l'offset du secteur 0.
fn slot_sector0(data: &[u8], slot: usize) -> Option<usize> {
    let start = slot * SLOT_SIZE;
    if start + SLOT_SIZE > data.len() {
        return None;
    }
    let mut seen = 0u32;
    let mut sector0 = 0;
    for k in 0..MAIN_SECTORS {
        let at = start + k * SECTOR;
        let id = rd_u16(data, at + 0xFF4) as usize;
        if id >= MAIN_SECTORS || rd_u32(data, at + 0xFF8) != SIGNATURE {
            return None;
        }
        seen |= 1 << id;
        if id == 0 {
            sector0 = at;
        }
    }
    (seen == (1 << MAIN_SECTORS) - 1).then_some(sector0)
}

/// Emplacement le plus récent (0 ou 1), `None` si aucun n'est complet.
fn active_slot(data: &[u8]) -> Option<usize> {
    match (slot_sector0(data, 0), slot_sector0(data, 1)) {
        (None, None) => None,
        (Some(_), None) => Some(0),
        (None, Some(_)) => Some(1),
        (Some(a), Some(b)) => {
            let (c1, c2) = (rd_u32(data, a + 0xFFC), rd_u32(data, b + 0xFFC));
            // `SAV3BlockDetection.CompareCounters` : 0xFFFFFFFF = jamais écrit (sauf débordement).
            let second = if c1 == u32::MAX && c2 != u32::MAX - 1 {
                true
            } else if c2 == u32::MAX && c1 != u32::MAX - 1 {
                false
            } else {
                c2 > c1
            };
            Some(second as usize)
        }
    }
}

/// Jeu d'une sauvegarde Gen 3 (`SaveUtil.GetVersionG3SAV`), `None` si ce n'en est pas une.
pub fn identify(data: &[u8]) -> Option<SaveVersion> {
    let body = &data[..body_size(data.len())?];
    let slot = active_slot(body)?;
    let small = slot_sector0(body, slot)?;
    let small = &body[small..small + USED];
    Some(match rd_u32(small, 0xAC) {
        1 => SaveVersion::FireRedLeafGreen,
        0 => SaveVersion::RubySapphire,
        _ if small[0x890..0xF2C].iter().any(|&b| b != 0) => SaveVersion::Emerald,
        _ => SaveVersion::RubySapphire,
    })
}

/// Emplacement actif et fichier d'origine (pour réécrire les secteurs).
#[derive(Debug, Clone)]
pub(super) struct Gen3Checks {
    raw: Vec<u8>,
    slot: usize,
}

/// Offsets propres à chaque jeu (relatifs au début des données reconstituées).
struct Consts {
    party: usize,
    money: usize,
    bag: usize,
    event_flags: usize,
    badge_flag: usize,
    security_key: Option<usize>,
}

fn consts(version: SaveVersion) -> Consts {
    match version {
        SaveVersion::FireRedLeafGreen => {
            Consts { party: LARGE + 0x34, money: LARGE + 0x290, bag: LARGE + 0x298, event_flags: LARGE + 0xEE0, badge_flag: 0x820, security_key: Some(0xF20) }
        }
        SaveVersion::Emerald => {
            Consts { party: LARGE + 0x234, money: LARGE + 0x490, bag: LARGE + 0x498, event_flags: LARGE + 0x1270, badge_flag: 0x867, security_key: Some(0xAC) }
        }
        _ => Consts { party: LARGE + 0x234, money: LARGE + 0x490, bag: LARGE + 0x498, event_flags: LARGE + 0x1220, badge_flag: 0x807, security_key: None },
    }
}

/// Lit le fichier : données reconstituées (dans l'ordre des secteurs) et disposition.
pub(super) fn layout(version: SaveVersion, body: &[u8]) -> Result<(Layout, Vec<String>, Vec<u8>), SaveError> {
    let slot = active_slot(body).ok_or(SaveError::Unrecognized)?;
    let mut logical = vec![0u8; LOGICAL_SIZE + 1];
    for k in 0..MAIN_SECTORS {
        let at = slot * SLOT_SIZE + k * SECTOR;
        let id = rd_u16(body, at + 0xFF4) as usize;
        logical[id * USED..(id + 1) * USED].copy_from_slice(&body[at..at + USED]);
    }
    let c = consts(version);
    let mut warnings = Vec::new();
    if body.len() < FULL_SIZE {
        warnings.push("Sauvegarde de 64 Kio : seul le premier emplacement existe (réglage de l'émulateur ?).".into());
    }
    // La langue n'est pas enregistrée en Gen 3 : celle du premier Pokémon du joueur, sinon le français.
    logical[LANGUAGE_AT] = guess_language(&logical, &c).unwrap_or(3);
    let layout = Layout {
        format: PkmFormat::Gen3,
        trainer: TrainerLayout {
            name: 0,
            name_max: 7,
            tid: 0x0A,
            sid: 0x0C,
            gender: 0x08,
            language: LANGUAGE_AT,
            money: c.money,
            hours: 0x0E,
            minutes: 0x10,
            seconds: 0x11,
        },
        party: c.party + 4,
        party_count: c.party,
        boxes: STORAGE + 4,
        box_stride: 30 * 80,
        box_count: BOX_COUNT,
        box_names: STORAGE + 4 + BOX_COUNT * 30 * 80,
        box_name_stride: BOX_NAME_LEN,
        box_name_max: 8,
        items: c.bag,
        dex: 0x18,
        // Carte actuelle : SaveBlock1.location (banque puis numéro), juste après la position.
        map: LARGE + 4,
        checks: Checks::Gen3(Gen3Checks { raw: body.to_vec(), slot }),
    };
    Ok((layout, warnings, logical))
}

fn guess_language(logical: &[u8], c: &Consts) -> Option<u8> {
    let tid = rd_u32(logical, 0x0A);
    let count = logical[c.party].min(6) as usize;
    (0..count).find_map(|i| {
        let at = c.party + 4 + i * super::pk3::PARTY_SIZE;
        let raw = super::pk3::decrypt(&logical[at..at + super::pk3::PARTY_SIZE]);
        (rd_u32(&raw, 4) == tid && (1..=7).contains(&raw[0x12])).then_some(raw[0x12])
    })
}

/// Fichier complet : secteurs de l'emplacement actif remplis et sommes recalculées.
pub(super) fn assemble(checks: &Gen3Checks, logical: &[u8]) -> Vec<u8> {
    let mut out = checks.raw.clone();
    for k in 0..MAIN_SECTORS {
        let at = checks.slot * SLOT_SIZE + k * SECTOR;
        let id = rd_u16(&out, at + 0xFF4) as usize;
        out[at..at + USED].copy_from_slice(&logical[id * USED..(id + 1) * USED]);
        let sum = checksum32(&out[at..at + USED]);
        out[at + 0xFF6..at + 0xFF8].copy_from_slice(&sum.to_le_bytes());
    }
    out
}

/// État des sommes des 14 secteurs de l'emplacement actif.
pub(super) fn verify(checks: &Gen3Checks, logical: &[u8]) -> Vec<BlockCheck> {
    let names = ["Dresseur", "Partie 1", "Partie 2", "Partie 3", "Partie 4"];
    (0..MAIN_SECTORS)
        .map(|k| {
            let at = checks.slot * SLOT_SIZE + k * SECTOR;
            let id = rd_u16(&checks.raw, at + 0xFF4) as usize;
            let stored = rd_u16(&checks.raw, at + 0xFF6);
            let computed = checksum32(&logical[id * USED..(id + 1) * USED]);
            let name = names.get(id).map_or_else(|| format!("Boîtes {}", id - 4), |n| n.to_string());
            BlockCheck { name: format!("Secteur {id} ({name})"), offset: at, length: USED, stored, computed, valid: stored == computed }
        })
        .collect()
}

/// Clé de sécurité (0 en Rubis / Saphir).
pub(super) fn security_key(version: SaveVersion, logical: &[u8]) -> u32 {
    consts(version).security_key.map_or(0, |at| rd_u32(logical, at))
}

/// Badges : 8 drapeaux d'évènement consécutifs.
pub(super) fn badges(version: SaveVersion, logical: &[u8]) -> u8 {
    let c = consts(version);
    (0..8).fold(0u8, |acc, i| {
        let flag = c.badge_flag + i;
        let byte = logical.get(c.event_flags + flag / 8).copied().unwrap_or(0);
        acc | (((byte >> (flag % 8)) & 1) << i)
    })
}

pub(super) fn set_badges(version: SaveVersion, logical: &mut [u8], bits: u8) {
    let c = consts(version);
    for i in 0..8 {
        let flag = c.badge_flag + i;
        if let Some(b) = logical.get_mut(c.event_flags + flag / 8) {
            let mask = 1 << (flag % 8);
            *b = if bits & (1 << i) != 0 { *b | mask } else { *b & !mask };
        }
    }
}

/// Sauvegarde Gen 3 vide et valide (deux emplacements, compteurs 1 et 2), pour les tests
/// et la démonstration. `version` choisit la valeur témoin en 0xAC du petit bloc.
pub fn blank(version: SaveVersion) -> Vec<u8> {
    let mut out = vec![0u8; FULL_SIZE];
    for slot in 0..2 {
        for k in 0..MAIN_SECTORS {
            // Secteurs tournés d'un cran par emplacement, comme après une sauvegarde.
            let id = (k + slot) % MAIN_SECTORS;
            let at = slot * SLOT_SIZE + k * SECTOR;
            let data = &mut out[at..at + USED];
            if id == 0 {
                match version {
                    SaveVersion::FireRedLeafGreen => data[0xAC] = 1,
                    SaveVersion::Emerald => {
                        data[0xAC..0xB0].copy_from_slice(&0x1234_5678u32.to_le_bytes());
                        data[0xF00] = 1; // données après 0x890 : Émeraude
                    }
                    _ => {}
                }
                data[..8].copy_from_slice(&[0xCE, 0xC2, 0xC3, 0xCD, 0xC7, 0xBB, 0xFF, 0xFF]); // « THISMA »
            }
            let sum = checksum32(&out[at..at + USED]);
            out[at + 0xFF4..at + 0xFF6].copy_from_slice(&(id as u16).to_le_bytes());
            out[at + 0xFF6..at + 0xFF8].copy_from_slice(&sum.to_le_bytes());
            out[at + 0xFF8..at + 0xFFC].copy_from_slice(&SIGNATURE.to_le_bytes());
            out[at + 0xFFC..at + 0x1000].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
        }
    }
    out
}
