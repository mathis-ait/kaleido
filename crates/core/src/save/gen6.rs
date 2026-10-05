//! Sauvegardes Gen 6 : X/Y et Rubis Oméga/Saphir Alpha (`SAV6*` de PKHeX), et table
//! des blocs commune aux sauvegardes 3DS (Gen 6 et 7).
//!
//! Table des blocs : à `taille − 0x200`. La signature « BEEF » (0x42454546) est à
//! +0x10, suivie d'une entrée de 8 octets par bloc à partir de +0x14 :
//! `[taille u32][id u16][CRC u16]`. Les blocs se suivent depuis l'offset 0, chacun
//! commençant à la frontière de 0x200 suivant la fin du précédent. Le CRC couvre
//! tout le bloc : CRC16-CCITT en Gen 6, CRC16 inversé en Gen 7. Tous les blocs de
//! la table sont recalculés à l'écriture.
//!
//! Offsets Gen 6 (de mémoire de PKHeX, **non vérifiés**) :
//!
//! | | Dresseur | Équipe | Temps de jeu | Divers (argent +0x8) | Noms des boîtes | Boîtes |
//! |---|---|---|---|---|---|---|
//! | XY | 0x14000 | 0x14200 | 0x1800 | 0x4200 | 0x4400 | 0x22600 |
//! | ROSA | 0x14000 | 0x14200 | 0x1800 | 0x4200 | 0x4400 | 0x33000 |
//!
//! Dresseur Gen 6 : TID +0, SID +2, sexe +5, nom +0x48. Équipe : 6 × 260 octets puis
//! le compteur. Boîtes : 31 × 30 × 232 octets ; noms de 0x22 octets. Sac : bloc 1
//! (0x400, 0xB88 octets en XY, 0xB90 en ROSA, d'après `SaveBlockAccessor6XY/6AO`).
//! À l'ouverture, Kaleido vérifie que ces zones tombent dans un bloc de la table et
//! signale sinon une structure inattendue.

use super::checksum::{crc16_ccitt, crc16_invert};
use super::{rd_u16, rd_u32, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout, BOX_SLOTS, PARTY_SLOTS};

pub(super) const BEEF: u32 = 0x4245_4546;
const BLOCK_ALIGN: usize = 0x200;
const BOX_NAME_BYTES: usize = 0x22;

/// Position de la table des blocs.
pub(super) fn table_offset(len: usize) -> Option<usize> {
    len.checked_sub(0x200)
}

#[derive(Debug, Clone)]
pub(super) struct CtrBlock {
    index: usize,
    offset: usize,
    len: usize,
    chk_at: usize,
}

/// Lit la table des blocs. S'arrête à la première entrée vide, incohérente (id
/// différent de l'index) ou qui déborderait sur la table.
pub(super) fn parse_table(data: &[u8]) -> Result<Vec<CtrBlock>, SaveError> {
    let info = table_offset(data.len()).ok_or(SaveError::Truncated("table des blocs"))?;
    if rd_u32(data, info + 0x10) != BEEF {
        return Err(SaveError::MissingBeef);
    }
    let mut blocks = Vec::new();
    let mut offset = 0usize;
    for index in 0.. {
        let at = info + 0x14 + 8 * index;
        if at + 8 > data.len() {
            break;
        }
        let len = rd_u32(data, at) as usize;
        let id = rd_u16(data, at + 4) as usize;
        if len == 0 || id != index || offset + len > info {
            break;
        }
        blocks.push(CtrBlock { index, offset, len, chk_at: at + 6 });
        offset = (offset + len).div_ceil(BLOCK_ALIGN) * BLOCK_ALIGN;
    }
    if blocks.is_empty() {
        return Err(SaveError::EmptyBlockTable);
    }
    Ok(blocks)
}

fn crc(data: &[u8], b: &CtrBlock, invert: bool) -> u16 {
    data.get(b.offset..b.offset + b.len).map_or(0, |d| if invert { crc16_invert(d) } else { crc16_ccitt(d) })
}

pub(super) fn fix(data: &mut [u8], blocks: &[CtrBlock], invert: bool) {
    for b in blocks {
        let c = crc(data, b, invert);
        wr_u16(data, b.chk_at, c);
    }
}

pub(super) fn verify(data: &[u8], blocks: &[CtrBlock], invert: bool) -> Vec<BlockCheck> {
    blocks
        .iter()
        .map(|b| {
            let stored = rd_u16(data, b.chk_at);
            let computed = crc(data, b, invert);
            BlockCheck { name: format!("bloc {}", b.index), offset: b.offset, length: b.len, stored, computed, valid: stored == computed }
        })
        .collect()
}

/// Offsets absolus des zones utiles d'une sauvegarde 3DS.
pub(super) struct CtrOffsets {
    pub(super) status: usize,
    pub(super) ot_name: usize,
    pub(super) party: usize,
    pub(super) play_time: usize,
    pub(super) money: usize,
    pub(super) box_names: usize,
    pub(super) boxes: usize,
    pub(super) box_count: usize,
    /// Bloc « MyItem » (sac) : offset et taille (`SaveBlockAccessor6*/7*` de PKHeX).
    pub(super) items: usize,
    pub(super) items_len: usize,
}

/// Construit la disposition 3DS et vérifie que chaque zone tombe dans un bloc.
pub(super) fn ctr_layout(format: PkmFormat, data: &[u8], o: &CtrOffsets, invert: bool) -> Result<(Layout, Vec<String>), SaveError> {
    let blocks = parse_table(data)?;
    let party_size = format.party_size();
    let zones = [
        ("dresseur", o.status, o.ot_name + 0x1A),
        ("équipe", o.party, PARTY_SLOTS * party_size + 1),
        ("temps de jeu", o.play_time, 4),
        ("argent", o.money, 4),
        ("noms des boîtes", o.box_names, o.box_count * BOX_NAME_BYTES),
        ("boîtes", o.boxes, o.box_count * BOX_SLOTS * format.stored_size()),
        ("sac", o.items, o.items_len),
    ];
    let mut warnings = Vec::new();
    for (name, start, len) in zones {
        if start + len > data.len() {
            return Err(SaveError::Truncated(name));
        }
        if !blocks.iter().any(|b| start >= b.offset && start + len <= b.offset + b.len) {
            warnings.push(format!(
                "Zone « {name} » (0x{start:X}, 0x{len:X} octets) hors des blocs de la table : structure inattendue, ses sommes de contrôle peuvent être fausses."
            ));
        }
    }
    let layout = Layout {
        format,
        trainer: TrainerLayout {
            name: o.status + o.ot_name,
            name_max: 12,
            tid: o.status,
            sid: o.status + 2,
            gender: o.status + 5,
            money: o.money,
            hours: o.play_time,
            minutes: o.play_time + 2,
            seconds: o.play_time + 3,
        },
        party: o.party,
        party_count: o.party + PARTY_SLOTS * party_size,
        boxes: o.boxes,
        box_stride: BOX_SLOTS * format.stored_size(),
        box_count: o.box_count,
        box_names: o.box_names,
        box_name_stride: BOX_NAME_BYTES,
        box_name_max: BOX_NAME_BYTES / 2 - 1,
        items: o.items,
        checks: Checks::Ctr { blocks, invert },
    };
    Ok((layout, warnings))
}

pub(super) fn offsets(version: SaveVersion) -> CtrOffsets {
    CtrOffsets {
        status: 0x14000,
        ot_name: 0x48,
        party: 0x14200,
        play_time: 0x1800,
        money: 0x4200 + 0x8,
        box_names: 0x4400,
        boxes: if version == SaveVersion::XY { 0x22600 } else { 0x33000 },
        box_count: 31,
        // Bloc 1 « MyItem » : SaveBlockAccessor6XY (0x400, 0xB88) / 6AO (0x400, 0xB90).
        items: 0x400,
        items_len: if version == SaveVersion::XY { 0xB88 } else { 0xB90 },
    }
}

pub(super) fn layout(version: SaveVersion, data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    ctr_layout(PkmFormat::Gen6, data, &offsets(version), false)
}

/// Sauvegarde 3DS vierge synthétique : table des blocs aux tailles données.
/// Renvoie aussi l'offset calculé de chaque bloc.
#[cfg(test)]
pub(super) fn blank(size: usize, lengths: &[usize]) -> (Vec<u8>, Vec<usize>) {
    let mut d = vec![0u8; size];
    let info = table_offset(size).unwrap();
    d[info + 0x10..info + 0x14].copy_from_slice(&BEEF.to_le_bytes());
    let mut offsets = Vec::new();
    let mut offset = 0;
    for (i, &len) in lengths.iter().enumerate() {
        let at = info + 0x14 + 8 * i;
        d[at..at + 4].copy_from_slice(&(len as u32).to_le_bytes());
        d[at + 4..at + 6].copy_from_slice(&(i as u16).to_le_bytes());
        offsets.push(offset);
        offset = (offset + len).div_ceil(BLOCK_ALIGN) * BLOCK_ALIGN;
    }
    (d, offsets)
}

/// Tailles synthétiques plaçant le dresseur, l'équipe et les boîtes aux offsets Gen 6.
#[cfg(test)]
pub(super) fn synthetic_lengths(version: SaveVersion) -> Vec<usize> {
    let boxes = offsets(version).boxes;
    vec![0x13F00, 0x170, 0x61C, boxes - 0x14A00, 0x34AD0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_parsing() {
        let (d, offsets) = blank(0x65600, &synthetic_lengths(SaveVersion::XY));
        assert_eq!(offsets, [0, 0x14000, 0x14200, 0x14A00, 0x22600]);
        let blocks = parse_table(&d).unwrap();
        assert_eq!(blocks.len(), 5);
        assert_eq!(blocks[4].offset, 0x22600);
        assert_eq!(blocks[0].chk_at, 0x65400 + 0x14 + 6);

        // Alignement : un bloc qui finit pile sur 0x200 n'ajoute pas de trou.
        let (d, offsets) = blank(0x65600, &[0x200, 0x10, 0x1F0]);
        assert_eq!(offsets, [0, 0x200, 0x400]);
        assert_eq!(parse_table(&d).unwrap().len(), 3);

        assert!(matches!(parse_table(&vec![0; 0x65600]), Err(SaveError::MissingBeef)));
        assert!(matches!(parse_table(&[0; 8]), Err(SaveError::Truncated(_))));
    }

    #[test]
    fn table_stops_on_bad_entries() {
        let (mut d, _) = blank(0x65600, &[0x100, 0x100, 0x100]);
        let info = table_offset(d.len()).unwrap();
        // Id incohérent sur la 3e entrée.
        d[info + 0x14 + 16 + 4] = 9;
        assert_eq!(parse_table(&d).unwrap().len(), 2);
        // Bloc débordant sur la table.
        let (d, _) = blank(0x65600, &[0x70000]);
        assert!(matches!(parse_table(&d), Err(SaveError::EmptyBlockTable)));
    }
}
