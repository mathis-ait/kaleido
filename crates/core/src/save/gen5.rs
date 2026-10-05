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
//! Blocs communs : noms des boîtes 0x0 (0x3E0, entrée 0 : nom `i` à 4 + 0x28·i, 0x14
//! octets), boîte `i` en 0x400 + 0x1000·i (0xFF0, entrée 1 + i), équipe 0x18E00 (0x534,
//! entrée 26 : compteur à +4, Pokémon à +8), dresseur (entrée 27 : nom +0x4, TID +0x14,
//! SID +0x16, langue +0x1E, sexe +0x21, temps de jeu +0x24). L'argent est lu dans le
//! bloc « divers » (0x21200 en NB, 0x21100 en N2B2) mais n'est pas modifiable ici.
//! Pokédex : 0x21600 (0x4D4, entrée 55) en NB, 0x21400 (0x4DC, entrée 54) en N2B2.
//!
//! Seuls les blocs que Kaleido peut modifier sont recalculés : les autres gardent
//! leur CRC d'origine. Le jeu identifié est celui dont la table est valide.
//!
//! Vérifié sur le code de PKHeX (`SAV5.cs`, `SaveBlockAccessor5BW.cs`,
//! `SaveBlockAccessor5B2W2.cs`, `BlockInfoNDS.cs`, `PlayerData5.cs`, `Misc5.cs`,
//! `BoxLayout5.cs`, `SaveUtil.IsValidFooter5`) ; pas encore testé sur une vraie
//! sauvegarde. La copie de secours (seconde moitié du fichier) n'est pas modifiée,
//! comme dans PKHeX.

use super::checksum::crc16_ccitt;
use super::{rd_u16, wr_u16, BlockCheck, Checks, Layout, PkmFormat, SaveError, SaveVersion, TrainerLayout};

const BOX_COUNT: usize = 24;
const BOXES: usize = 0x400;
const BOX_STRIDE: usize = 0x1000;
const PARTY_BLOCK: usize = 0x18E00;
const TRAINER: usize = 0x19400;

#[derive(Debug, Clone, Copy)]
struct Table {
    offset: usize,
    len: usize,
    trainer_len: usize,
    money: usize,
    /// Bloc du Pokédex : début, taille, index dans la table.
    dex: (usize, usize, usize),
}

// Vérifié : PKHeX Saves/Access/SaveBlockAccessor5BW.cs et SaveBlockAccessor5B2W2.cs
// (bloc 27 « Trainer Data » 0x68 / 0xB0, bloc 52 « Misc » 0x21200 / 0x21100 avec l'argent
// à +0, Pokédex = bloc 55 en NB / 54 en N2B2, table finale 0x8C / 0x94).
fn table(version: SaveVersion) -> Table {
    if version == SaveVersion::Black2White2 {
        Table { offset: 0x25F00, len: 0x94, trainer_len: 0xB0, money: 0x21100, dex: (0x21400, 0x4DC, 54) }
    } else {
        Table { offset: 0x23F00, len: 0x8C, trainer_len: 0x68, money: 0x21200, dex: (0x21600, 0x4D4, 55) }
    }
}

// Vérifié : PKHeX SaveUtil.cs (IsValidFooter5 : CRC de la table à `taille − 0x100 + len + 0x10 − 2`)
// et SaveBlockAccessor5BW.cs (dernière entrée : 0x23F00, 0x8C, CRC en 0x23F9A).
fn table_chk_at(t: &Table) -> usize {
    t.offset + t.len + 0xE
}

fn table_valid(data: &[u8], t: &Table) -> bool {
    data.get(t.offset..t.offset + t.len).is_some_and(|d| crc16_ccitt(d) == rd_u16(data, table_chk_at(t)))
}

/// Noire/Blanche ou Noire 2/Blanche 2, d'après la table de sommes de contrôle valide.
// Vérifié : PKHeX SaveUtil.cs (IsG5BW essayé avant IsG5B2W2).
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
    // Vérifié : PKHeX SaveBlockAccessor5BW.cs (CRC de chaque bloc à début + taille + 2,
    // copie dans la table à 0x23F00 + 2·index) et BlockInfoNDS.cs.
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
    blocks.push(NdsBlock { name: "équipe".into(), offset: PARTY_BLOCK, len: 0x534, mirror: mirror(26) });
    blocks.push(NdsBlock { name: "dresseur".into(), offset: TRAINER, len: t.trainer_len, mirror: mirror(27) });
    let (dex, dex_len, dex_index) = t.dex;
    blocks.push(NdsBlock { name: "Pokédex".into(), offset: dex, len: dex_len, mirror: mirror(dex_index) });

    let layout = Layout {
        format: PkmFormat::Gen5,
        // Vérifié : PKHeX Saves/Substructures/Gen5/PlayerData5.cs (OT +0x4 sur 0x10 octets,
        // TID +0x14, SID +0x16, Language +0x1E, Gender +0x21, PlayedHours +0x24 (u16),
        // minutes +0x26, secondes +0x27) et Misc5.cs (Money à +0).
        trainer: TrainerLayout {
            name: TRAINER + 0x4,
            name_max: 7,
            tid: TRAINER + 0x14,
            sid: TRAINER + 0x16,
            gender: TRAINER + 0x21,
            language: TRAINER + 0x1E,
            money: t.money,
            hours: TRAINER + 0x24,
            minutes: TRAINER + 0x26,
            seconds: TRAINER + 0x27,
        },
        // Vérifié : PKHeX SAV5.cs (Party = 0x18E00, PartyCount = Data[Party + 4],
        // GetPartyOffset = Party + 8 + 220·slot, Box = 0x400, GetBoxOffset = Box + 0x1000·box).
        party: PARTY_BLOCK + 8,
        party_count: PARTY_BLOCK + 4,
        boxes: BOXES,
        box_stride: BOX_STRIDE,
        box_count: BOX_COUNT,
        // Correction (PKHeX BoxLayout5.cs) : les noms sont espacés de 0x28 octets à partir
        // de +4, mais chacun n'occupe que 0x14 octets (9 caractères + terminateur) ; on
        // lisait 0x28 octets, donc le début du nom suivant si le terminateur manquait.
        box_names: 4,
        box_name_stride: 0x28,
        box_name_max: 0x14 / 2 - 1,
        dex,
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
        // Pokédex : mêmes positions que SaveBlockAccessor5BW / 5B2W2 de PKHeX.
        for (v, chk, mirror) in [(SaveVersion::BlackWhite, 0x21AD6, 0x23F6E), (SaveVersion::Black2White2, 0x218DE, 0x25F6C)] {
            let t = table(v);
            let b = NdsBlock { name: String::new(), offset: t.dex.0, len: t.dex.1, mirror: t.offset + 2 * t.dex.2 };
            assert_eq!((b.chk_at(), b.mirror), (chk, mirror), "{v:?}");
        }
    }

    // L'équipe (compteur + 6 × 220 octets) tient dans son bloc de 0x534 octets.
    const _: () = assert!(8 + 6 * 220 <= 0x534);
}
