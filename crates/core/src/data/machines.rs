//! CT / CS (capsules techniques et secrètes) et donneurs de capacités (DS).
//!
//! Formats repris de l'Universal Pokémon Randomizer (UPR-FVX) et vérifiés sur de
//! vraies ROMs (Platine, Blanche) :
//!
//! - Liste des CT/CS : tableau de u16 dans l'ARM9 (décompressé), repéré par le
//!   motif qui le précède. Platine : CT01-92 puis CS01-08. Noire/Blanche :
//!   CT01-92, CS01-06, puis CT93-95 (ordre des objets).
//! - Compatibilité : un bit par CT puis par CS (CT01-95 puis CS01-06 en Gen 5),
//!   dans la fiche « personal » (0x1C en Gen 4, 0x28 en Gen 5).
//! - Donneurs de capacités de Platine : overlay 5, 38 entrées de 12 octets
//!   (attaque u16 + prix en tessons), suivies de la compatibilité (5 octets
//!   par espèce, un bit par attaque).
//! - Données d'attaques : Gen 4 `type` en 4, puissance en 3, précision en 5 ;
//!   Gen 5 `type` en 0, puissance en 3, précision en 4.

use kaleido_formats::nds::NdsRom;

use super::{put_u16, u16_at};
use crate::games::Game;
use crate::pokemon::PokeType;
use crate::rom::RomError;

/// Organisation des CT/CS d'une génération.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineSpec {
    pub generation: u8,
    pub tm_count: usize,
    pub hm_count: usize,
    /// CT stockées avant les CS (les suivantes sont après).
    block_one: usize,
    /// Octets précédant immédiatement le tableau dans l'ARM9.
    prefix: &'static [u8],
    /// Objet CT01, et objet de la première CT du second bloc.
    pub first_tm_item: u16,
    pub second_block_item: u16,
    /// Position de la compatibilité dans la fiche « personal ».
    pub compat_offset: usize,
}

impl MachineSpec {
    pub fn for_generation(generation: u8) -> Option<Self> {
        match generation {
            4 => Some(Self {
                generation,
                tm_count: 92,
                hm_count: 8,
                block_one: 92,
                prefix: &[0xD1, 0x00, 0xD2, 0x00, 0xD3, 0x00, 0xD4, 0x00],
                first_tm_item: 328,
                second_block_item: 328 + 92,
                compat_offset: 0x1C,
            }),
            5 => Some(Self {
                generation,
                tm_count: 95,
                hm_count: 6,
                block_one: 92,
                prefix: &[0x87, 0x03, 0x88, 0x03],
                first_tm_item: 328,
                second_block_item: 618,
                compat_offset: 0x28,
            }),
            _ => None,
        }
    }

    /// Nombre total de bits de compatibilité utilisés (CT + CS).
    pub fn total(&self) -> usize {
        self.tm_count + self.hm_count
    }

    /// Objet correspondant à la CT n° `i` (0 = CT01).
    pub fn tm_item(&self, i: usize) -> u16 {
        if i < self.block_one {
            self.first_tm_item + i as u16
        } else {
            self.second_block_item + (i - self.block_one) as u16
        }
    }

    /// Position (relative au tableau) de la CT n° `i`.
    fn tm_slot(&self, i: usize) -> usize {
        if i < self.block_one {
            i * 2
        } else {
            (i + self.hm_count) * 2
        }
    }

    fn hm_slot(&self, i: usize) -> usize {
        (self.block_one + i) * 2
    }

    fn table_len(&self) -> usize {
        self.total() * 2
    }
}

/// Attaques des CT et des CS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machines {
    pub tms: Vec<u16>,
    pub hms: Vec<u16>,
}

fn find_unique(hay: &[u8], needle: &[u8]) -> Option<usize> {
    let mut hits = hay.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle).map(|(i, _)| i);
    match (hits.next(), hits.next()) {
        (Some(p), None) => Some(p),
        _ => None,
    }
}

/// Début du tableau des CT/CS dans l'ARM9 décompressé.
pub fn table_offset(arm9: &[u8], spec: &MachineSpec) -> Result<usize, RomError> {
    let at = find_unique(arm9, spec.prefix).ok_or_else(|| RomError::Layout("tableau des CT introuvable dans l'ARM9".into()))? + spec.prefix.len();
    if at + spec.table_len() > arm9.len() {
        return Err(RomError::Layout("tableau des CT tronqué".into()));
    }
    Ok(at)
}

/// Lit les CT/CS dans un ARM9 décompressé.
pub fn read_from(arm9: &[u8], spec: &MachineSpec) -> Result<Machines, RomError> {
    let at = table_offset(arm9, spec)?;
    Ok(Machines {
        tms: (0..spec.tm_count).map(|i| u16_at(arm9, at + spec.tm_slot(i))).collect(),
        hms: (0..spec.hm_count).map(|i| u16_at(arm9, at + spec.hm_slot(i))).collect(),
    })
}

/// Remplace les attaques des CT (les CS ne sont jamais modifiées).
pub fn write_into(arm9: &mut [u8], spec: &MachineSpec, tms: &[u16]) -> Result<(), RomError> {
    if tms.len() != spec.tm_count {
        return Err(RomError::Layout(format!("{} CT attendues, {} fournies", spec.tm_count, tms.len())));
    }
    let at = table_offset(arm9, spec)?;
    for (i, &mv) in tms.iter().enumerate() {
        put_u16(arm9, at + spec.tm_slot(i), mv);
    }
    Ok(())
}

pub fn read(rom: &NdsRom, generation: u8) -> Result<Machines, RomError> {
    let spec = MachineSpec::for_generation(generation).ok_or_else(unsupported)?;
    read_from(&rom.arm9_decompressed()?, &spec)
}

fn unsupported() -> RomError {
    RomError::Unsupported("CT/CS : seules les générations 4 et 5 sont prises en charge".into())
}

// ---------------------------------------------------------------------------
// Couleur des CT dans le sac (palette de l'icône selon le type de l'attaque).

/// Entrées CT01 et CT02 (UPR `pthgssItemPalettesPrefix`). Les palettes (octets 2-3
/// et 10-11) sont ignorées à la recherche : une ROM déjà modifiée est aussi reconnue.
const PALETTE_PREFIX_GEN4: [u8; 16] = [0x8D, 0x01, 0x8E, 0x01, 0x21, 0x01, 0x33, 0x01, 0x8D, 0x01, 0x8F, 0x01, 0x22, 0x01, 0x34, 0x01];
const PALETTE_WILDCARDS_GEN4: [usize; 4] = [2, 3, 10, 11];
const PALETTE_PREFIX_BW: [u8; 16] = [0xE9, 0x03, 0xEA, 0x03, 0x02, 0x00, 0x03, 0x00, 0x04, 0x00, 0x05, 0x00, 0x06, 0x00, 0x07, 0x00];

/// Palette d'icône de CT pour un type (identique en Gen 4 et 5, d'après UPR).
pub fn tm_palette(t: PokeType) -> u16 {
    match t {
        PokeType::Fighting => 398,
        PokeType::Dragon => 399,
        PokeType::Water => 400,
        PokeType::Psychic => 401,
        PokeType::Normal => 402,
        PokeType::Poison => 403,
        PokeType::Ice => 404,
        PokeType::Grass => 405,
        PokeType::Fire => 406,
        PokeType::Dark => 407,
        PokeType::Steel => 408,
        PokeType::Electric => 409,
        PokeType::Ground => 410,
        PokeType::Rock => 412,
        PokeType::Flying => 413,
        PokeType::Bug => 610,
        PokeType::Ghost | PokeType::Mystery | PokeType::Fairy => 411,
    }
}

/// Position du champ « palette » de chaque CT dans l'ARM9 (ou `None` si la table
/// des icônes d'objets n'est pas reconnue : la couleur n'est alors pas modifiée).
pub fn palette_slots(arm9: &[u8], spec: &MachineSpec) -> Option<Vec<usize>> {
    let slots: Vec<usize> = if spec.generation <= 4 {
        // La table commence à l'entrée de la CT01 (8 octets par objet).
        let matches = |w: &[u8]| w.iter().zip(PALETTE_PREFIX_GEN4).enumerate().all(|(i, (a, b))| PALETTE_WILDCARDS_GEN4.contains(&i) || *a == b);
        let mut hits = arm9.windows(PALETTE_PREFIX_GEN4.len()).enumerate().filter(|(_, w)| matches(w)).map(|(i, _)| i);
        let base = match (hits.next(), hits.next()) {
            (Some(p), None) => p,
            _ => return None,
        };
        (0..spec.tm_count).map(|i| base + i * 8 + 2).collect()
    } else {
        // La table commence à l'objet 0 (4 octets par objet).
        let base = find_unique(arm9, &PALETTE_PREFIX_BW)?;
        (0..spec.tm_count).map(|i| base + spec.tm_item(i) as usize * 4 + 2).collect()
    };
    slots.iter().all(|&s| s + 2 <= arm9.len()).then_some(slots)
}

// ---------------------------------------------------------------------------
// Compatibilité (fiche « personal »).

/// L'espèce peut-elle apprendre la machine n° `index` (CT puis CS) ?
pub fn compatible(personal: &[u8], spec: &MachineSpec, index: usize) -> bool {
    personal.get(spec.compat_offset + index / 8).is_some_and(|b| b & (1 << (index % 8)) != 0)
}

pub fn set_compatible(personal: &mut [u8], spec: &MachineSpec, index: usize, value: bool) {
    if let Some(b) = personal.get_mut(spec.compat_offset + index / 8) {
        if value {
            *b |= 1 << (index % 8);
        } else {
            *b &= !(1 << (index % 8));
        }
    }
}

// ---------------------------------------------------------------------------
// Données d'attaques.

/// NARC des données d'attaques.
pub fn move_data_path(game: Game) -> Option<&'static str> {
    match game {
        Game::Platinum => Some("poketool/waza/pl_waza_tbl.narc"),
        Game::Black | Game::White => Some("a/0/2/1"),
        _ => None,
    }
}

/// Champs utiles d'une attaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveInfo {
    pub kind: Option<PokeType>,
    pub power: u8,
    /// Précision (0 ou 101 : touche toujours).
    pub accuracy: u8,
}

pub fn move_info(generation: u8, d: &[u8]) -> Option<MoveInfo> {
    let (t, p, a) = if generation <= 4 { (4, 3, 5) } else { (0, 3, 4) };
    Some(MoveInfo { kind: PokeType::from_index(generation, *d.get(t)?), power: *d.get(p)?, accuracy: *d.get(a)? })
}

// ---------------------------------------------------------------------------
// Donneurs de capacités (Platine).

pub const PT_TUTOR_OVERLAY: u32 = 5;
pub const PT_TUTOR_COUNT: usize = 38;
const PT_TUTOR_ENTRY: usize = 12;
const PT_TUTOR_COMPAT_BYTES: usize = 5;
/// Début du tableau selon la langue (UPR : US/EU, japonais, allemand,
/// français/espagnol, italien, coréen).
const PT_TUTOR_OFFSETS: [usize; 6] = [0x2FF64, 0x2FD54, 0x2FF80, 0x2FF6C, 0x2FF74, 0x2FF5C];

/// Tableau des donneurs de capacités dans l'overlay 5 de Platine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TutorTable {
    pub offset: usize,
    /// Nombre d'entrées de compatibilité (espèces puis formes).
    pub compat_entries: usize,
}

impl TutorTable {
    /// Cherche le tableau aux emplacements connus et vérifie sa structure :
    /// 38 attaques distinctes et valides, compatibilité de `entries` espèces à la suite.
    pub fn locate(overlay: &[u8], max_move: u16, entries: usize) -> Result<Self, RomError> {
        let table_end = PT_TUTOR_COUNT * PT_TUTOR_ENTRY;
        for &offset in &PT_TUTOR_OFFSETS {
            if offset + table_end + entries * PT_TUTOR_COMPAT_BYTES > overlay.len() {
                continue;
            }
            let moves: Vec<u16> = (0..PT_TUTOR_COUNT).map(|i| u16_at(overlay, offset + i * PT_TUTOR_ENTRY)).collect();
            let valid = moves.iter().all(|&m| (1..=max_move).contains(&m));
            let distinct = moves.iter().enumerate().all(|(i, m)| !moves[..i].contains(m));
            if valid && distinct {
                return Ok(Self { offset, compat_entries: entries });
            }
        }
        Err(RomError::Layout("tableau des donneurs de capacités introuvable (overlay 5)".into()))
    }

    pub fn moves(&self, overlay: &[u8]) -> Vec<u16> {
        (0..PT_TUTOR_COUNT).map(|i| u16_at(overlay, self.offset + i * PT_TUTOR_ENTRY)).collect()
    }

    pub fn set_moves(&self, overlay: &mut [u8], moves: &[u16]) {
        for (i, &m) in moves.iter().take(PT_TUTOR_COUNT).enumerate() {
            put_u16(overlay, self.offset + i * PT_TUTOR_ENTRY, m);
        }
    }

    /// Position de la compatibilité de l'entrée `entry` (0 = Bulbizarre).
    fn compat_at(&self, entry: usize) -> Option<usize> {
        (entry < self.compat_entries).then_some(self.offset + PT_TUTOR_COUNT * PT_TUTOR_ENTRY + entry * PT_TUTOR_COMPAT_BYTES)
    }

    pub fn compatible(&self, overlay: &[u8], entry: usize, tutor: usize) -> bool {
        self.compat_at(entry).and_then(|at| overlay.get(at + tutor / 8)).is_some_and(|b| b & (1 << (tutor % 8)) != 0)
    }

    /// Modifie un bit ; les bits inutilisés du dernier octet sont conservés.
    pub fn set_compatible(&self, overlay: &mut [u8], entry: usize, tutor: usize, value: bool) {
        if tutor >= PT_TUTOR_COUNT {
            return;
        }
        if let Some(b) = self.compat_at(entry).and_then(|at| overlay.get_mut(at + tutor / 8)) {
            if value {
                *b |= 1 << (tutor % 8);
            } else {
                *b &= !(1 << (tutor % 8));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_arm9(spec: &MachineSpec, moves: &[u16]) -> Vec<u8> {
        let mut d = vec![0xAAu8; 32];
        d.extend_from_slice(spec.prefix);
        for m in moves {
            d.extend_from_slice(&m.to_le_bytes());
        }
        d.extend_from_slice(&[0x55; 16]);
        d
    }

    #[test]
    fn gen5_table_order() {
        // CT01-92, CS01-06, CT93-95.
        let spec = MachineSpec::for_generation(5).unwrap();
        let raw: Vec<u16> = (1..=92).chain(1001..=1006).chain(93..=95).collect();
        let mut arm9 = fake_arm9(&spec, &raw);
        let m = read_from(&arm9, &spec).unwrap();
        assert_eq!(m.tms, (1..=95).collect::<Vec<u16>>());
        assert_eq!(m.hms, (1001..=1006).collect::<Vec<u16>>());
        let new: Vec<u16> = (201..=295).collect();
        write_into(&mut arm9, &spec, &new).unwrap();
        let m = read_from(&arm9, &spec).unwrap();
        assert_eq!(m.tms, new);
        assert_eq!(m.hms, (1001..=1006).collect::<Vec<u16>>(), "les CS ne bougent pas");
        assert_eq!(spec.tm_item(92), 618);
        assert_eq!(spec.tm_item(0), 328);
    }

    #[test]
    fn gen4_table_and_ambiguity() {
        let spec = MachineSpec::for_generation(4).unwrap();
        let raw: Vec<u16> = (1..=100).collect();
        let arm9 = fake_arm9(&spec, &raw);
        let m = read_from(&arm9, &spec).unwrap();
        assert_eq!(m.tms.len(), 92);
        assert_eq!(m.hms, (93..=100).collect::<Vec<u16>>());
        // Motif présent deux fois : refus.
        let mut twice = arm9.clone();
        twice.extend_from_slice(spec.prefix);
        twice.extend_from_slice(&[0; 200]);
        assert!(read_from(&twice, &spec).is_err());
        assert!(write_into(&mut twice.clone(), &spec, &[1; 91]).is_err());
    }

    #[test]
    fn gen4_palettes_found_after_change() {
        let spec = MachineSpec::for_generation(4).unwrap();
        let mut arm9 = vec![0u8; 16];
        arm9.extend_from_slice(&PALETTE_PREFIX_GEN4);
        arm9.extend_from_slice(&[0; 92 * 8]);
        let slots = palette_slots(&arm9, &spec).unwrap();
        assert_eq!((slots[0], slots[1]), (18, 26));
        // CT01 devenue Plante (405) : toujours reconnue.
        arm9[18..20].copy_from_slice(&405u16.to_le_bytes());
        assert_eq!(palette_slots(&arm9, &spec), Some(slots));
    }

    #[test]
    fn compat_bits() {
        let spec = MachineSpec::for_generation(4).unwrap();
        let mut p = vec![0u8; 0x2C];
        set_compatible(&mut p, &spec, 0, true);
        set_compatible(&mut p, &spec, 99, true);
        assert_eq!(p[0x1C], 1);
        assert_eq!(p[0x1C + 12], 1 << 3);
        assert!(compatible(&p, &spec, 99) && !compatible(&p, &spec, 98));
        set_compatible(&mut p, &spec, 99, false);
        assert!(!compatible(&p, &spec, 99));
        // Hors de la fiche : ignoré, sans panique.
        set_compatible(&mut p, &spec, 500, true);
        assert!(!compatible(&p, &spec, 500));
    }

    #[test]
    fn tutor_table() {
        let mut ovl = vec![0u8; 0x31000];
        let at = 0x2FF6C;
        for i in 0..PT_TUTOR_COUNT {
            put_u16(&mut ovl, at + i * 12, 100 + i as u16);
        }
        let t = TutorTable::locate(&ovl, 467, 505).unwrap();
        assert_eq!(t.offset, at);
        assert_eq!(t.moves(&ovl)[37], 137);
        // Dernier octet : 6 bits utiles, les 2 autres sont conservés.
        let last = at + 38 * 12 + 4;
        ovl[last] = 0xC0;
        t.set_compatible(&mut ovl, 0, 37, true);
        assert_eq!(ovl[last], 0xC0 | 0x20);
        t.set_compatible(&mut ovl, 0, 38, true);
        assert_eq!(ovl[last], 0xE0);
        assert!(t.compatible(&ovl, 0, 37));
        assert!(TutorTable::locate(&ovl, 120, 505).is_err(), "attaques hors limites");
    }

    #[test]
    fn real_move_data() {
        // Charge : Platine (puissance 35, précision 95) et Blanche (50, 100).
        let pt = [0x00, 0x00, 0x00, 0x23, 0x00, 0x5F, 0x23, 0x00];
        assert_eq!(move_info(4, &pt), Some(MoveInfo { kind: Some(PokeType::Normal), power: 35, accuracy: 95 }));
        let bw = [0x00, 0x00, 0x01, 0x32, 0x64, 0x23];
        assert_eq!(move_info(5, &bw), Some(MoveInfo { kind: Some(PokeType::Normal), power: 50, accuracy: 100 }));
        assert_eq!(move_info(5, &[0]), None);
    }
}
