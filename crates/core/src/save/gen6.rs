//! Sauvegardes Gen 6 : X/Y et Rubis Oméga/Saphir Alpha (`SAV6*` de PKHeX), et table
//! des blocs commune aux sauvegardes 3DS (Gen 6 et 7).
//!
//! Table des blocs : à `taille − 0x200`. La signature « BEEF » (0x42454546) est à
//! +0x10, suivie d'une entrée de 8 octets par bloc à partir de +0x14 :
//! `[taille u32][id u16][CRC u16]`. Les blocs se suivent depuis l'offset 0, chacun
//! commençant à la frontière de 0x200 suivant la fin du précédent. Le CRC couvre
//! tout le bloc : CRC16-CCITT en Gen 6, CRC16 inversé en Gen 7 (bloc 36 : signature
//! MemeCrypto remise à zéro). Tous les blocs de la table sont recalculés à l'écriture.
//!
//! Offsets Gen 6 (vérifiés sur `SaveBlockAccessor6XY/AO` de PKHeX) :
//!
//! | | Dresseur | Équipe | Temps de jeu | Divers (argent +0x8) | Noms des boîtes | Pokédex | Boîtes |
//! |---|---|---|---|---|---|---|---|
//! | XY | 0x14000 | 0x14200 | 0x1800 | 0x4200 | 0x4400 | 0x15000 | 0x22600 |
//! | ROSA | 0x14000 | 0x14200 | 0x1800 | 0x4200 | 0x4400 | 0x15000 | 0x33000 |
//!
//! Dresseur Gen 6 : TID +0, SID +2, sexe +5, nom +0x48. Équipe : 6 × 260 octets puis
//! le compteur. Boîtes : 31 × 30 × 232 octets ; noms de 0x22 octets. Sac : bloc 1
//! (0x400, 0xB88 octets en XY, 0xB90 en ROSA, d'après `SaveBlockAccessor6XY/6AO`).
//! Dresseur Gen 6 : TID +0, SID +2, sexe +5, langue +0x2D, nom +0x48. Équipe : 6 × 260
//! octets puis le compteur. Boîtes : 31 × 30 × 232 octets ; noms de 0x22 octets.
//! À l'ouverture, Kaleido vérifie que ces zones tombent dans un bloc de la table et
//! signale sinon une structure inattendue. Pas encore testé sur une vraie sauvegarde
//! Gen 6 (la table des blocs et le CRC Gen 7 l'ont été sur la sauvegarde SL de PKHeX).

use super::checksum::{crc16_ccitt, crc16_invert};
use super::{rd_u16, rd_u32, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout, BOX_SLOTS, PARTY_SLOTS};

pub(super) const BEEF: u32 = 0x4245_4546;
const BLOCK_ALIGN: usize = 0x200;
const BOX_NAME_BYTES: usize = 0x22;

/// Position de la table des blocs.
// Vérifié : PKHeX SaveBlockAccessor6XY.cs etc. (BlockMetadataOffset = taille − 0x200) et
// SaveUtil.cs (HasSaveFooterBEEF : « BEEF » à taille − 0x1F0).
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
// Vérifié : PKHeX BlockInfo3DS.cs (entrées `[taille u32][id u16][CRC u16]` à +0x14, blocs
// alignés sur 0x200) ; les offsets ainsi calculés retombent sur ceux, codés en dur, de
// SaveBlockAccessor7SM.cs (contrôlé sur la sauvegarde SL des tests de PKHeX).
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

/// Gen 7 : zone de la signature MemeCrypto (`[début, fin)`) si `b` est le bloc qui la
/// contient (bloc 36, signature à +0x100 sur 0x80 octets).
// Vérifié : PKHeX SAV7.cs (MemeCryptoBlock = 36, ClearMemeCrypto) et MemeCrypto.cs
// (SaveFileSignatureOffset = 0x100, SaveFileSignatureLength = 0x80).
fn meme_region(b: &CtrBlock, gen7: bool) -> Option<(usize, usize)> {
    (gen7 && b.index == super::memecrypto::SAVE_BLOCK_INDEX && b.len >= 0x180).then(|| {
        let start = b.offset + super::memecrypto::SIGNATURE_OFFSET;
        (start, start + super::memecrypto::SIGNATURE_LENGTH)
    })
}

// Vérifié : PKHeX BlockInfo3DS.cs (BlockInfo6 = CRC16_CCITT, BlockInfo7 = CRC16Invert, sur
// tout le bloc ; CRC rangé à table + 0x14 + 8·id + 6).
fn crc(data: &[u8], b: &CtrBlock, gen7: bool) -> u16 {
    let Some(block) = data.get(b.offset..b.offset + b.len) else { return 0 };
    if !gen7 {
        return crc16_ccitt(block);
    }
    match meme_region(b, gen7) {
        // Correction (vérifiée sur la sauvegarde SL des tests de PKHeX) : le jeu calcule le
        // CRC du bloc 36 signature remise à zéro, puis signe ; le CRC stocké ne couvre donc
        // pas la signature.
        Some((start, end)) => {
            let mut copy = block.to_vec();
            copy[start - b.offset..end - b.offset].fill(0);
            crc16_invert(&copy)
        }
        None => crc16_invert(block),
    }
}

/// Recalcule tous les CRC ; en Gen 7, signe ensuite la table des blocs (MemeCrypto),
/// comme `SAV7.GetFinalData` de PKHeX.
pub(super) fn fix(data: &mut [u8], blocks: &[CtrBlock], gen7: bool) {
    for b in blocks {
        let c = crc(data, b, gen7);
        wr_u16(data, b.chk_at, c);
    }
    if gen7 {
        super::memecrypto::sign_save(data);
    }
}

pub(super) fn verify(data: &[u8], blocks: &[CtrBlock], gen7: bool) -> Vec<BlockCheck> {
    let mut out: Vec<BlockCheck> = blocks
        .iter()
        .map(|b| {
            let stored = rd_u16(data, b.chk_at);
            let computed = crc(data, b, gen7);
            BlockCheck { name: format!("bloc {}", b.index), offset: b.offset, length: b.len, stored, computed, valid: stored == computed }
        })
        .collect();
    if gen7 {
        if let Some(check) = super::memecrypto::verify_save(data) {
            out.push(check);
        }
    }
    out
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
    /// Langue de la partie, dans le bloc du dresseur.
    pub(super) language: usize,
    /// Bloc du Pokédex (début, taille).
    pub(super) dex: (usize, usize),
    /// Zone actuelle (`u16`), dans le bloc « Situation ».
    pub(super) map: usize,
}

/// Construit la disposition 3DS et vérifie que chaque zone tombe dans un bloc.
pub(super) fn ctr_layout(format: PkmFormat, data: &[u8], o: &CtrOffsets, gen7: bool) -> Result<(Layout, Vec<String>), SaveError> {
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
        ("Pokédex", o.dex.0, o.dex.1),
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
            language: o.language,
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
        dex: o.dex.0,
        map: o.map,
        checks: Checks::Ctr { blocks, gen7 },
    };
    Ok((layout, warnings))
}

// Vérifié : PKHeX Saves/Access/SaveBlockAccessor6XY.cs et SaveBlockAccessor6AO.cs (blocs 6
// PlayTime 0x1800, 11 Misc 0x4200, 12 BOX 0x4400, 17 MyStatus 0x14000, 18 PokePartySave
// 0x14200, 20 ZukanData 0x15000 (0x6A0 / 0x11CC), 53 / 56 Box 0x22600 / 0x33000),
// SAV6XY.cs / SAV6AO.cs (Party, Box), SAV6.cs (PartyCount = Data[Party + 6·260], BoxCount 31),
// MyStatus6.cs (TID +0, SID +2, Gender +5, Language +0x2D, OT +0x48 sur 0x1A octets),
// Misc6XY.cs / Misc6AO.cs (Money +0x8), BoxLayout6.cs (noms de 0x22 octets).
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
        language: 0x14000 + 0x2D,
        dex: (0x15000, if version == SaveVersion::XY { 0x6A0 } else { 0x11CC }),
        // Vérifié : PKHeX SaveBlockAccessor6XY/AO (bloc 4 « Situation » en 0x1400), Situation6.cs (`M` en +2).
        map: 0x1402,
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
