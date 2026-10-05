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
//! son pied. Comme PKHeX, la comparaison lit toujours les deux mots à `fin − 0x14`
//! (compteur principal) et `fin − 0x10` (secondaire), y compris en HGSS.
//!
//! | | Général | Boîtes (début, taille) | Dresseur | Équipe | Pokédex |
//! |---|---|---|---|---|---|
//! | DP | 0xC100 | 0xC100, 0x121E0 | 0x64 | 0x98 | 0x12DC |
//! | Pt | 0xCF2C | 0xCF2C, 0x121E4 | 0x68 | 0xA0 | 0x1328 |
//! | HGSS | 0xF628 | 0xF700, 0x12310 | 0x64 | 0x98 | 0x12B8 |
//!
//! **Non vérifié sur de vraies sauvegardes** : surtout l'ordre des compteurs dans le
//! pied et la comparaison HGSS (un seul compteur supposé), ainsi que les offsets du
//! dresseur (nom +0, TID +0x10, SID +0x12, argent +0x14, sexe +0x18, temps de jeu +0x22).
//!
//! Sac : bloc général + 0x624 (DP), 0x630 (Pt), 0x644 (HGSS), d'après
//! `PlayerBag4*.BaseOffset` de PKHeX (voir [`super::inventory`]).
//! Dresseur : nom +0, TID +0x10, SID +0x12, argent +0x14, sexe +0x18, langue +0x19,
//! temps de jeu +0x22.
//!
//! Vérifié sur le code de PKHeX (`SAV4.cs`, `SAV4Sinnoh.cs`, `SAV4DP.cs`, `SAV4Pt.cs`,
//! `SAV4HGSS.cs`, `SAV4BlockDetection.cs`) ; toujours pas testé sur une vraie
//! sauvegarde Gen 4 (PKHeX n'en fournit pas dans ses tests).

use super::checksum::crc16_ccitt;
use super::{rd_u32, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout};

pub(super) const PARTITION: usize = 0x40000;
const BOX_COUNT: usize = 18;
// Vérifié : PKHeX SAV4Sinnoh.cs / SAV4HGSS.cs (BOX_NAME_LEN = 40, 8 caractères saisissables).
const BOX_NAME_BYTES: usize = 40;
/// HGSS : drapeaux « boîte modifiée » (une boîte par bit) après la boîte courante.
// Vérifié : PKHeX SAV4HGSS.cs (FlagsBoxContentChanged à BOX_END + 4, 0x3FFFF = 18 boîtes).
const HGSS_BOX_FLAGS: usize = 0x12004;
const HGSS_BOX_FLAGS_ALL: u32 = 0x3_FFFF;

#[derive(Debug, Clone, Copy)]
pub(super) struct Consts {
    pub(super) general_size: usize,
    pub(super) storage_start: usize,
    pub(super) storage_size: usize,
    pub(super) footer: usize,
    trainer: usize,
    party: usize,
    /// Sac, relatif au bloc général (`PlayerBag4*.BaseOffset` de PKHeX).
    bag: usize,
    pub(super) dex: usize,
    hgss: bool,
}

// Vérifié : PKHeX SAV4DP.cs, SAV4Pt.cs, SAV4HGSS.cs (GeneralSize, StorageSize, début des
// boîtes = GeneralSize [+ GeneralGap 0xD8 en HGSS], Trainer1, Party, PokeDex, FooterSize).
pub(super) fn consts(version: SaveVersion) -> Consts {
    match version {
        SaveVersion::DiamondPearl => Consts {
            general_size: 0xC100,
            storage_start: 0xC100,
            storage_size: 0x121E0,
            footer: 0x14,
            trainer: 0x64,
            party: 0x98,
            bag: 0x624,
            dex: 0x12DC,
            hgss: false,
        },
        SaveVersion::Platinum => Consts {
            general_size: 0xCF2C,
            storage_start: 0xCF2C,
            storage_size: 0x121E4,
            footer: 0x14,
            trainer: 0x68,
            party: 0xA0,
            bag: 0x630,
            dex: 0x1328,
            hgss: false,
        },
        _ => Consts {
            general_size: 0xF628,
            storage_start: 0xF700,
            storage_size: 0x12310,
            footer: 0x10,
            trainer: 0x64,
            party: 0x98,
            bag: 0x644,
            dex: 0x12B8,
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

/// Sommes de contrôle Gen 4, plus les drapeaux HGSS à forcer avant l'écriture.
#[derive(Debug, Clone)]
pub(super) struct Gen4Checks {
    blocks: Vec<FooterBlock>,
    /// HGSS : offset absolu des drapeaux « boîte modifiée ».
    hgss_box_flags: Option<usize>,
}

/// Résultat de `SAV4BlockDetection.CompareCounters` de PKHeX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Newer {
    First,
    Second,
    Same,
}

// Vérifié : PKHeX SAV4BlockDetection.cs (CompareCounters) : 0xFFFFFFFF = jamais écrit,
// sauf débordement (l'autre vaut 0xFFFFFFFE).
fn compare_counters(c1: u32, c2: u32) -> Newer {
    if c1 == u32::MAX && c2 != u32::MAX - 1 {
        return Newer::Second;
    }
    if c2 == u32::MAX && c1 != u32::MAX - 1 {
        return Newer::First;
    }
    match c1.cmp(&c2) {
        std::cmp::Ordering::Greater => Newer::First,
        std::cmp::Ordering::Less => Newer::Second,
        std::cmp::Ordering::Equal => Newer::Same,
    }
}

/// `true` si la copie de la 2e partition est la plus récente.
// Vérifié : PKHeX SAV4.cs (GetActiveBlock : begin + length − 0x14, pour les trois jeux)
// et SAV4BlockDetection.cs (CompareFooters : compteur principal puis secondaire, égalité
// = 1re copie).
fn second_is_newer(data: &[u8], end: usize) -> bool {
    let (o1, o2) = (end - 0x14, PARTITION + end - 0x14);
    match compare_counters(rd_u32(data, o1), rd_u32(data, o2)) {
        Newer::First => false,
        Newer::Second => true,
        Newer::Same => compare_counters(rd_u32(data, o1 + 4), rd_u32(data, o2 + 4)) == Newer::Second,
    }
}

/// Décalage (0 ou 0x40000) de la copie la plus récente d'un bloc, et validité.
///
/// Ajout de Kaleido : si une seule copie a un pied valide (taille attendue à
/// `fin − 0xC`), elle est choisie sans comparer les compteurs. PKHeX compare
/// toujours les compteurs ; le résultat est le même tant que la copie invalide
/// n'a jamais été écrite (compteurs à 0xFFFFFFFF).
pub(super) fn active_partition(data: &[u8], start: usize, size: usize) -> (usize, bool) {
    let valid = |p: usize| rd_u32(data, p + start + size - 0xC) == size as u32;
    match (valid(0), valid(PARTITION)) {
        (true, false) => (0, true),
        (false, true) => (PARTITION, true),
        (false, false) => (0, false),
        (true, true) => (if second_is_newer(data, start + size) { PARTITION } else { 0 }, true),
    }
}

pub(super) fn layout(version: SaveVersion, data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    if data.len() < 2 * PARTITION {
        return Err(SaveError::Truncated("partition de secours"));
    }
    let c = consts(version);
    let mut warnings = Vec::new();
    let (general, general_ok) = active_partition(data, 0, c.general_size);
    let (storage_part, storage_ok) = active_partition(data, c.storage_start, c.storage_size);
    let storage = c.storage_start + storage_part;
    if !general_ok {
        warnings.push("Pied du bloc général invalide dans les deux partitions.".into());
    }
    if !storage_ok {
        warnings.push("Pied du bloc des boîtes invalide dans les deux partitions.".into());
    }

    // DP/Pt : u32 « boîte courante » puis 18 boîtes contiguës de 0xFF0, puis les noms.
    // HGSS : boîtes alignées sur 0x1000, boîte courante en 0x12000, noms en 0x12008.
    // Vérifié : PKHeX SAV4Sinnoh.cs (GetBoxOffset = 4 + box·0xFF0, BOX_NAME = 4 + 18·0xFF0)
    // et SAV4HGSS.cs (GetBoxOffset = box·0x1000, BOX_NAME = 0x12008).
    let (boxes, box_stride, box_names) = if c.hgss { (storage, 0x1000, storage + 0x12008) } else { (storage + 4, 0xFF0, storage + 0x11EE4) };

    let t = general + c.trainer;
    let layout = Layout {
        format: PkmFormat::Gen4,
        // Vérifié : PKHeX SAV4.cs (OT = Trainer1, 16 octets ; ID32 +0x10 ; Money +0x14 ;
        // Gender +0x18 ; Language +0x19 ; PlayedHours +0x22 (u16), minutes +0x24, secondes +0x25).
        trainer: TrainerLayout {
            name: t,
            name_max: 7,
            tid: t + 0x10,
            sid: t + 0x12,
            money: t + 0x14,
            gender: t + 0x18,
            language: t + 0x19,
            hours: t + 0x22,
            minutes: t + 0x24,
            seconds: t + 0x25,
        },
        // Vérifié : PKHeX SAV4.cs (PartyCount = General[Party − 4]).
        party: general + c.party,
        party_count: general + c.party - 4,
        boxes,
        box_stride,
        box_count: BOX_COUNT,
        box_names,
        box_name_stride: BOX_NAME_BYTES,
        box_name_max: BOX_NAME_BYTES / 2 - 1,
        items: general + c.bag,
        dex: general + c.dex,
        checks: Checks::Gen4(Gen4Checks {
            blocks: vec![
                FooterBlock { name: "bloc général", start: general, size: c.general_size, footer: c.footer },
                FooterBlock { name: "bloc des boîtes", start: storage, size: c.storage_size, footer: c.footer },
            ],
            hgss_box_flags: c.hgss.then_some(storage + HGSS_BOX_FLAGS),
        }),
    };
    Ok((layout, warnings))
}

// Vérifié : PKHeX SAV4.cs (CalcBlockChecksum = CRC16_CCITT(data[..^FooterSize]), stocké
// dans les deux derniers octets du bloc).
fn block_crc(data: &[u8], b: &FooterBlock) -> u16 {
    data.get(b.start..b.start + b.size - b.footer).map_or(0, crc16_ccitt)
}

pub(super) fn fix(data: &mut [u8], checks: &Gen4Checks) {
    // Correction (PKHeX SAV4HGSS.GetFinalData) : HGSS ne réécrit que les boîtes marquées
    // comme modifiées ; on les marque toutes, sinon le jeu peut corrompre les boîtes
    // éditées lors de la sauvegarde suivante.
    if let Some(at) = checks.hgss_box_flags {
        if let Some(b) = data.get_mut(at..at + 4) {
            b.copy_from_slice(&HGSS_BOX_FLAGS_ALL.to_le_bytes());
        }
    }
    for b in &checks.blocks {
        let crc = block_crc(data, b);
        wr_u16(data, b.start + b.size - 2, crc);
    }
}

pub(super) fn verify(data: &[u8], checks: &Gen4Checks) -> Vec<BlockCheck> {
    checks
        .blocks
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
/// Sert aux tests et à la sauvegarde de démonstration.
pub(super) fn blank(version: SaveVersion, newer_general: usize, newer_storage: usize) -> Vec<u8> {
    let c = consts(version);
    let mut d = vec![0u8; 2 * PARTITION];
    let mut footer = |start: usize, size: usize, newer: bool| {
        let end = start + size;
        let counter: u32 = if newer { 8 } else { 7 };
        d[end - c.footer..end - c.footer + 4].copy_from_slice(&counter.to_le_bytes());
        d[end - 0xC..end - 0x8].copy_from_slice(&(size as u32).to_le_bytes());
        d[end - 0x8..end - 0x4].copy_from_slice(&crate::saves::GEN4_MAGIC_INTL.to_le_bytes());
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
        assert_eq!(active_partition(&d, 0, c.general_size), (PARTITION, true));
        assert_eq!(active_partition(&d, c.storage_start, c.storage_size), (0, true));

        // Compteur principal égal : le second compteur départage (DP/Pt).
        let mut d = blank(v, 0, 0);
        let end2 = PARTITION + c.general_size;
        d[end2 - 0x14..end2 - 0x10].copy_from_slice(&8u32.to_le_bytes());
        d[end2 - 0x10..end2 - 0xC].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(active_partition(&d, 0, c.general_size).0, PARTITION);

        // Égalité parfaite : la 1re copie (comme PKHeX).
        let mut d = blank(v, 0, 0);
        d[end2 - 0x14..end2 - 0x10].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(active_partition(&d, 0, c.general_size).0, 0);

        // Une seule copie valide : elle est choisie quel que soit le compteur.
        let mut d = blank(v, 1, 1);
        d[end2 - 0xC..end2 - 0x8].fill(0xFF);
        assert_eq!(active_partition(&d, 0, c.general_size), (0, true));

        // Compteur jamais écrit (0xFFFFFFFF) : l'autre copie gagne.
        let mut d = blank(v, 0, 0);
        d[c.general_size - 0x14..c.general_size - 0x10].fill(0xFF);
        assert_eq!(active_partition(&d, 0, c.general_size).0, PARTITION);
    }

    #[test]
    fn counter_rollover() {
        assert_eq!(compare_counters(u32::MAX, 3), Newer::Second);
        assert_eq!(compare_counters(3, u32::MAX), Newer::First);
        // Débordement : 0xFFFFFFFF suit 0xFFFFFFFE.
        assert_eq!(compare_counters(u32::MAX, u32::MAX - 1), Newer::First);
        assert_eq!(compare_counters(5, 5), Newer::Same);
    }

    #[test]
    fn hgss_counters() {
        let v = SaveVersion::HeartGoldSoulSilver;
        let c = consts(v);
        let d = blank(v, 1, 1);
        assert_eq!(active_partition(&d, 0, c.general_size).0, PARTITION);
        assert_eq!(active_partition(&d, c.storage_start, c.storage_size).0, PARTITION);
    }

    #[test]
    fn hgss_marks_all_boxes_changed() {
        let v = SaveVersion::HeartGoldSoulSilver;
        let mut d = blank(v, 0, 0);
        let (layout, _) = layout(v, &d).unwrap();
        let Checks::Gen4(checks) = &layout.checks else { panic!() };
        fix(&mut d, checks);
        let at = consts(v).storage_start + HGSS_BOX_FLAGS;
        assert_eq!(rd_u32(&d, at), 0x3_FFFF);
        // DP/Pt : rien de tel.
        let (layout, _) = super::layout(SaveVersion::Platinum, &blank(SaveVersion::Platinum, 0, 0)).unwrap();
        let Checks::Gen4(checks) = &layout.checks else { panic!() };
        assert!(checks.hgss_box_flags.is_none());
    }
}
