//! Sauvegardes Gen 4 : Diamant/Perle, Platine, HeartGold/SoulSilver (`SAV4*` de PKHeX).
//!
//! Le fichier de 512 Kio contient deux partitions de 0x40000. Chacune contient un
//! bloc « général » (dresseur, équipe…) puis un bloc « boîtes ». Le jeu écrit
//! alternativement dans l'une ou l'autre, et les deux blocs sont indépendants : le
//! plus récent est choisi bloc par bloc d'après les compteurs de son pied.
//!
//! Pied de bloc (fin du bloc) : DP/Pt = 0x14 octets `[compteur u32][compteur u32]
//! [taille u32][date SDK u32][id u16][CRC u16]` ; HGSS = 0x10 octets `[compteur u32]
//! [taille u32][date SDK u32][id u16][CRC u16]`. Le CRC16-CCITT couvre le bloc sans
//! son pied.
//!
//! | | Général | Boîtes (début, taille) | Dresseur | Équipe |
//! |---|---|---|---|---|
//! | DP | 0xC100 | 0xC100, 0x121E0 | 0x64 | 0x98 |
//! | Pt | 0xCF2C | 0xCF2C, 0x121E4 | 0x68 | 0xA0 |
//! | HGSS | 0xF628 | 0xF700, 0x12310 | 0x64 | 0x98 |
//!
//! **Non vérifié sur de vraies sauvegardes** : surtout l'ordre des compteurs dans le
//! pied et la comparaison HGSS (un seul compteur supposé), ainsi que les offsets du
//! dresseur (nom +0, TID +0x10, SID +0x12, argent +0x14, sexe +0x18, temps de jeu +0x22).

use super::checksum::crc16_ccitt;
use super::{rd_u32, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout};

pub(super) const PARTITION: usize = 0x40000;
const BOX_COUNT: usize = 18;
const BOX_NAME_BYTES: usize = 40;

#[derive(Debug, Clone, Copy)]
pub(super) struct Consts {
    pub(super) general_size: usize,
    pub(super) storage_start: usize,
    pub(super) storage_size: usize,
    pub(super) footer: usize,
    trainer: usize,
    party: usize,
    hgss: bool,
}

pub(super) fn consts(version: SaveVersion) -> Consts {
    match version {
        SaveVersion::DiamondPearl => Consts {
            general_size: 0xC100,
            storage_start: 0xC100,
            storage_size: 0x121E0,
            footer: 0x14,
            trainer: 0x64,
            party: 0x98,
            hgss: false,
        },
        SaveVersion::Platinum => Consts {
            general_size: 0xCF2C,
            storage_start: 0xCF2C,
            storage_size: 0x121E4,
            footer: 0x14,
            trainer: 0x68,
            party: 0xA0,
            hgss: false,
        },
        _ => Consts {
            general_size: 0xF628,
            storage_start: 0xF700,
            storage_size: 0x12310,
            footer: 0x10,
            trainer: 0x64,
            party: 0x98,
            hgss: true,
        },
    }
}

/// Bloc protégé par un CRC stocké dans ses deux derniers octets.
#[derive(Debug, Clone)]
pub(super) struct FooterBlock {
    name: &'static str,
    start: usize,
    size: usize,
    footer: usize,
}

/// Compteurs de sauvegarde du pied (`None` = jamais écrit, 0xFFFFFFFF).
fn counters(data: &[u8], end: usize, footer: usize) -> [Option<u32>; 2] {
    let read = |at: usize| Some(rd_u32(data, at)).filter(|&v| v != u32::MAX);
    if footer == 0x14 {
        [read(end - 0x14), read(end - 0x10)]
    } else {
        [read(end - 0x10), Some(0)]
    }
}

/// Décalage (0 ou 0x40000) de la copie la plus récente d'un bloc, et validité.
pub(super) fn active_partition(data: &[u8], start: usize, size: usize, footer: usize) -> (usize, bool) {
    let valid = |p: usize| rd_u32(data, p + start + size - 0xC) == size as u32;
    match (valid(0), valid(PARTITION)) {
        (true, false) => (0, true),
        (false, true) => (PARTITION, true),
        (false, false) => (0, false),
        (true, true) => {
            let first = counters(data, start + size, footer);
            let second = counters(data, PARTITION + start + size, footer);
            (if second > first { PARTITION } else { 0 }, true)
        }
    }
}

pub(super) fn layout(version: SaveVersion, data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    if data.len() < 2 * PARTITION {
        return Err(SaveError::Truncated("partition de secours"));
    }
    let c = consts(version);
    let mut warnings = Vec::new();
    let (general, general_ok) = active_partition(data, 0, c.general_size, c.footer);
    let (storage_part, storage_ok) = active_partition(data, c.storage_start, c.storage_size, c.footer);
    let storage = c.storage_start + storage_part;
    if !general_ok {
        warnings.push("Pied du bloc général invalide dans les deux partitions.".into());
    }
    if !storage_ok {
        warnings.push("Pied du bloc des boîtes invalide dans les deux partitions.".into());
    }

    // DP/Pt : u32 « boîte courante » puis 18 boîtes contiguës de 0xFF0, puis les noms.
    // HGSS : boîtes alignées sur 0x1000, boîte courante en 0x12000, noms en 0x12008.
    let (boxes, box_stride, box_names) =
        if c.hgss { (storage, 0x1000, storage + 0x12008) } else { (storage + 4, 0xFF0, storage + 0x11EE4) };

    let t = general + c.trainer;
    let layout = Layout {
        format: PkmFormat::Gen4,
        trainer: TrainerLayout {
            name: t,
            name_max: 7,
            tid: t + 0x10,
            sid: t + 0x12,
            money: t + 0x14,
            gender: t + 0x18,
            hours: t + 0x22,
            minutes: t + 0x24,
            seconds: t + 0x25,
        },
        party: general + c.party,
        party_count: general + c.party - 4,
        boxes,
        box_stride,
        box_count: BOX_COUNT,
        box_names,
        box_name_stride: BOX_NAME_BYTES,
        box_name_max: BOX_NAME_BYTES / 2 - 1,
        checks: Checks::Gen4(vec![
            FooterBlock { name: "bloc général", start: general, size: c.general_size, footer: c.footer },
            FooterBlock { name: "bloc des boîtes", start: storage, size: c.storage_size, footer: c.footer },
        ]),
    };
    Ok((layout, warnings))
}

fn block_crc(data: &[u8], b: &FooterBlock) -> u16 {
    data.get(b.start..b.start + b.size - b.footer).map_or(0, crc16_ccitt)
}

pub(super) fn fix(data: &mut [u8], blocks: &[FooterBlock]) {
    for b in blocks {
        let crc = block_crc(data, b);
        wr_u16(data, b.start + b.size - 2, crc);
    }
}

pub(super) fn verify(data: &[u8], blocks: &[FooterBlock]) -> Vec<BlockCheck> {
    blocks
        .iter()
        .map(|b| {
            let stored = super::rd_u16(data, b.start + b.size - 2);
            let computed = block_crc(data, b);
            BlockCheck { name: b.name.into(), offset: b.start, length: b.size, stored, computed, valid: stored == computed }
        })
        .collect()
}

/// Sauvegarde vierge synthétique : pieds valides dans les deux partitions, la
/// partition `newer_general` / `newer_storage` (0 ou 1) ayant le compteur le plus haut.
#[cfg(test)]
pub(super) fn blank(version: SaveVersion, newer_general: usize, newer_storage: usize) -> Vec<u8> {
    let c = consts(version);
    let mut d = vec![0u8; 2 * PARTITION];
    let mut footer = |start: usize, size: usize, newer: bool| {
        let end = start + size;
        let counter: u32 = if newer { 8 } else { 7 };
        d[end - c.footer..end - c.footer + 4].copy_from_slice(&counter.to_le_bytes());
        d[end - 0xC..end - 0x8].copy_from_slice(&(size as u32).to_le_bytes());
        d[end - 0x8..end - 0x4].copy_from_slice(&0x2006_0623u32.to_le_bytes());
    };
    for p in 0..2 {
        footer(p * PARTITION, c.general_size, p == newer_general);
        footer(p * PARTITION + c.storage_start, c.storage_size, p == newer_storage);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_choice() {
        let v = SaveVersion::Platinum;
        let c = consts(v);
        let d = blank(v, 1, 0);
        assert_eq!(active_partition(&d, 0, c.general_size, c.footer), (PARTITION, true));
        assert_eq!(active_partition(&d, c.storage_start, c.storage_size, c.footer), (0, true));

        // Compteur principal égal : le second compteur départage (DP/Pt).
        let mut d = blank(v, 0, 0);
        let end2 = PARTITION + c.general_size;
        d[end2 - 0x14..end2 - 0x10].copy_from_slice(&8u32.to_le_bytes());
        d[end2 - 0x10..end2 - 0xC].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(active_partition(&d, 0, c.general_size, c.footer).0, PARTITION);

        // Une seule copie valide : elle est choisie quel que soit le compteur.
        let mut d = blank(v, 1, 1);
        d[end2 - 0xC..end2 - 0x8].fill(0xFF);
        assert_eq!(active_partition(&d, 0, c.general_size, c.footer), (0, true));

        // Compteur jamais écrit (0xFFFFFFFF) : l'autre copie gagne.
        let mut d = blank(v, 0, 0);
        d[c.general_size - 0x14..c.general_size - 0x10].fill(0xFF);
        assert_eq!(active_partition(&d, 0, c.general_size, c.footer).0, PARTITION);
    }

    #[test]
    fn hgss_single_counter() {
        let v = SaveVersion::HeartGoldSoulSilver;
        let c = consts(v);
        let d = blank(v, 1, 1);
        assert_eq!(active_partition(&d, 0, c.general_size, c.footer).0, PARTITION);
        assert_eq!(active_partition(&d, c.storage_start, c.storage_size, c.footer).0, PARTITION);
    }
}
