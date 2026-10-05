//! Sauvegardes Gen 5 : Noire/Blanche et Noire 2/Blanche 2 (`SAV5*` de PKHeX).
//!
//! Chaque bloc est suivi de son CRC16-CCITT (à `début + taille + 2`), recopié dans
//! une table de sommes de contrôle, elle-même protégée par un CRC :
//!
//! | | Table | Taille | CRC de la table | Bloc dresseur |
//! |---|---|---|---|---|
//! | NB | 0x23F00 | 0x8C | 0x23F9A | 0x19400, 0x68 |
//! | N2B2 | 0x25F00 | 0x94 | 0x25FA2 | 0x19400, 0xB0 |
//!
//! Blocs communs : noms des boîtes 0x0 (0x3E0, entrée 0), boîte `i` en
//! 0x400 + 0x1000·i (0xFF0, entrée 1 + i), équipe 0x18E00 (0x534, entrée 26 :
//! compteur à +4, Pokémon à +8), dresseur (entrée 27 : nom +0x4, TID +0x14, SID +0x16,
//! sexe +0x21, temps de jeu +0x24), sac 0x18400 (entrée 25 : 0x9C0 octets en NB,
//! 0x9EC en N2B2, d'après `SaveBlockAccessor5BW/B2W2` de PKHeX). L'argent est lu dans le bloc « divers »
//! (0x21200 en NB, 0x21100 en N2B2) mais n'est pas modifiable ici.
//!
//! Seuls les blocs que Kaleido peut modifier sont recalculés : les autres gardent
//! leur CRC d'origine. Le jeu identifié est celui dont la table est valide.
//!
//! **Non vérifié sur de vraies sauvegardes** : en particulier le +4 des noms de
//! boîtes, la longueur du bloc dresseur et l'offset de l'argent. La copie de
//! secours (seconde moitié du fichier) n'est pas modifiée, comme dans PKHeX.

use super::checksum::crc16_ccitt;
use super::{rd_u16, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout};

const BOX_COUNT: usize = 24;
const BOXES: usize = 0x400;
const BOX_STRIDE: usize = 0x1000;
const ITEMS: usize = 0x18400;
const PARTY_BLOCK: usize = 0x18E00;
const TRAINER: usize = 0x19400;

#[derive(Debug, Clone, Copy)]
struct Table {
    offset: usize,
    len: usize,
    trainer_len: usize,
    money: usize,
    /// Taille du bloc 25 « Inventory » (`SaveBlockAccessor5BW/B2W2` de PKHeX).
    items_len: usize,
}

fn table(version: SaveVersion) -> Table {
    if version == SaveVersion::Black2White2 {
        Table { offset: 0x25F00, len: 0x94, trainer_len: 0xB0, money: 0x21100, items_len: 0x9EC }
    } else {
        Table { offset: 0x23F00, len: 0x8C, trainer_len: 0x68, money: 0x21200, items_len: 0x9C0 }
    }
}

fn table_chk_at(t: &Table) -> usize {
    t.offset + t.len + 0xE
}

fn table_valid(data: &[u8], t: &Table) -> bool {
    data.get(t.offset..t.offset + t.len).is_some_and(|d| crc16_ccitt(d) == rd_u16(data, table_chk_at(t)))
}

/// Noire/Blanche ou Noire 2/Blanche 2, d'après la table de sommes de contrôle valide.
pub(super) fn detect(data: &[u8]) -> Option<SaveVersion> {
    [SaveVersion::BlackWhite, SaveVersion::Black2White2].into_iter().find(|&v| table_valid(data, &table(v)))
}

#[derive(Debug, Clone)]
pub(super) struct NdsBlock {
    name: String,
    offset: usize,
    len: usize,
    /// Copie du CRC dans la table.
    mirror: usize,
}

impl NdsBlock {
    fn chk_at(&self) -> usize {
        self.offset + self.len + 2
    }
}

#[derive(Debug, Clone)]
pub(super) struct NdsChecks {
    blocks: Vec<NdsBlock>,
    table: usize,
    table_len: usize,
    table_chk: usize,
}

pub(super) fn layout(version: SaveVersion, data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    let t = table(version);
    if data.len() < table_chk_at(&t) + 2 {
        return Err(SaveError::Truncated("table des sommes de contrôle"));
    }
    let mirror = |index: usize| t.offset + 2 * index;
    let mut blocks = vec![NdsBlock { name: "noms des boîtes".into(), offset: 0, len: 0x3E0, mirror: mirror(0) }];
    for i in 0..BOX_COUNT {
        blocks.push(NdsBlock { name: format!("boîte {}", i + 1), offset: BOXES + i * BOX_STRIDE, len: 0xFF0, mirror: mirror(1 + i) });
    }
    blocks.push(NdsBlock { name: "sac".into(), offset: ITEMS, len: t.items_len, mirror: mirror(25) });
    blocks.push(NdsBlock { name: "équipe".into(), offset: PARTY_BLOCK, len: 0x534, mirror: mirror(26) });
    blocks.push(NdsBlock { name: "dresseur".into(), offset: TRAINER, len: t.trainer_len, mirror: mirror(27) });

    let layout = Layout {
        format: PkmFormat::Gen5,
        trainer: TrainerLayout {
            name: TRAINER + 0x4,
            name_max: 7,
            tid: TRAINER + 0x14,
            sid: TRAINER + 0x16,
            gender: TRAINER + 0x21,
            money: t.money,
            hours: TRAINER + 0x24,
            minutes: TRAINER + 0x26,
            seconds: TRAINER + 0x27,
        },
        party: PARTY_BLOCK + 8,
        party_count: PARTY_BLOCK + 4,
        boxes: BOXES,
        box_stride: BOX_STRIDE,
        box_count: BOX_COUNT,
        box_names: 4,
        box_name_stride: 0x28,
        box_name_max: 0x28 / 2 - 1,
        items: ITEMS,
        checks: Checks::Gen5(NdsChecks { blocks, table: t.offset, table_len: t.len, table_chk: table_chk_at(&t) }),
    };
    Ok((layout, Vec::new()))
}

fn crc_of(data: &[u8], offset: usize, len: usize) -> u16 {
    data.get(offset..offset + len).map_or(0, crc16_ccitt)
}

pub(super) fn fix(data: &mut [u8], checks: &NdsChecks) {
    for b in &checks.blocks {
        let crc = crc_of(data, b.offset, b.len);
        wr_u16(data, b.chk_at(), crc);
        wr_u16(data, b.mirror, crc);
    }
    let crc = crc_of(data, checks.table, checks.table_len);
    wr_u16(data, checks.table_chk, crc);
}

pub(super) fn verify(data: &[u8], checks: &NdsChecks) -> Vec<BlockCheck> {
    let mut out: Vec<BlockCheck> = checks
        .blocks
        .iter()
        .map(|b| {
            let stored = rd_u16(data, b.chk_at());
            let computed = crc_of(data, b.offset, b.len);
            let valid = stored == computed && rd_u16(data, b.mirror) == computed;
            BlockCheck { name: b.name.clone(), offset: b.offset, length: b.len, stored, computed, valid }
        })
        .collect();
    let stored = rd_u16(data, checks.table_chk);
    let computed = crc_of(data, checks.table, checks.table_len);
    out.push(BlockCheck {
        name: "table des sommes de contrôle".into(),
        offset: checks.table,
        length: checks.table_len,
        stored,
        computed,
        valid: stored == computed,
    });
    out
}

/// Sauvegarde vierge synthétique dont seule la table est valide.
#[cfg(test)]
pub(super) fn blank(version: SaveVersion) -> Vec<u8> {
    let t = table(version);
    let mut d = vec![0u8; crate::saves::NDS_SAVE_SIZE];
    let crc = crc16_ccitt(&d[t.offset..t.offset + t.len]);
    wr_u16(&mut d, table_chk_at(&t), crc);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_variant_by_table() {
        assert_eq!(detect(&blank(SaveVersion::BlackWhite)), Some(SaveVersion::BlackWhite));
        assert_eq!(detect(&blank(SaveVersion::Black2White2)), Some(SaveVersion::Black2White2));
        assert_eq!(detect(&vec![0; crate::saves::NDS_SAVE_SIZE]), None);
        assert_eq!(detect(&[0; 16]), None);
    }

    #[test]
    fn block_positions() {
        // Les CRC des boîtes suivent immédiatement les 30 Pokémon (0xFF0 octets).
        let b = NdsBlock { name: String::new(), offset: BOXES, len: 0xFF0, mirror: 0 };
        assert_eq!(b.chk_at(), 0x13F2);
        assert_eq!(table_chk_at(&table(SaveVersion::BlackWhite)), 0x23F9A);
        assert_eq!(table_chk_at(&table(SaveVersion::Black2White2)), 0x25FA2);
    }

    // L'équipe (compteur + 6 × 220 octets) tient dans son bloc de 0x534 octets.
    const _: () = assert!(8 + 6 * 220 <= 0x534);
}
