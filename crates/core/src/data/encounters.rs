//! Tables de rencontres sauvages.
//!
//! - Platine : 424 octets par zone. Herbe : taux u32 puis 12 × (niveau u32, espèce u32) ;
//!   puis remplacements (essaims, jour/nuit, Pokéradar, slots double-jeu GBA) en
//!   espèces u32 ; enfin Surf, Éclate-Roc, Canne, Super Canne, Méga Canne :
//!   taux u32 + 5 × (max u8, min u8, u16, espèce u32).
//! - Noire/Blanche : 232 octets par saison (1 ou 4 saisons par zone). 8 octets de
//!   taux, puis 56 emplacements (espèce u16 avec forme en bits 11-15, min u8, max u8).

use super::{put_u16, u16_at};

/// Un emplacement de rencontre modifiable, repéré par sa position dans le fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub offset: usize,
    pub species: u16,
    pub min_level: u8,
    pub max_level: u8,
    /// Encodage de l'espèce dans le fichier.
    pub(crate) kind: SlotKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlotKind {
    /// Espèce u32 (Platine).
    U32,
    /// Espèce u16 avec forme dans les bits hauts (Gen 5).
    U16Form,
}

/// Lit tous les emplacements non vides d'un fichier de zone (décompressé).
pub fn read(generation: u8, data: &[u8]) -> Vec<Slot> {
    match generation {
        ..=4 => read_platinum(data),
        5 => read_bw(data),
        _ => read_oras(data),
    }
}

/// Rubis Oméga / Saphir Alpha : fichier de zone « ZO » ; la section 4 (offset u32
/// en 0x10) commence par 14 octets de taux, puis 61 emplacements au format Gen 5
/// (herbe 12, hautes herbes 12, spéciaux 3, Surf 5, Éclate-Roc 5, cannes 3×3, hordes 3×5).
pub const ORAS_RATES: usize = 0x0E;
pub const ORAS_SLOTS: usize = 61;

pub fn oras_section(d: &[u8]) -> Option<std::ops::Range<usize>> {
    if d.len() < 0x14 || &d[..2] != b"ZO" {
        return None;
    }
    let start = u32::from_le_bytes(d[0x10..0x14].try_into().unwrap()) as usize;
    let end = start + ORAS_RATES + ORAS_SLOTS * 4;
    (end <= d.len()).then_some(start..end)
}

fn read_oras(d: &[u8]) -> Vec<Slot> {
    let Some(section) = oras_section(d) else { return Vec::new() };
    (0..ORAS_SLOTS)
        .filter_map(|i| {
            let at = section.start + ORAS_RATES + i * 4;
            let species = u16_at(d, at) & 0x07FF;
            (species != 0).then_some(Slot { offset: at, species, min_level: d[at + 2], max_level: d[at + 3], kind: SlotKind::U16Form })
        })
        .collect()
}

fn read_platinum(d: &[u8]) -> Vec<Slot> {
    if d.len() < 0x1A8 {
        return Vec::new();
    }
    let u32_at = |at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap());
    let mut slots = Vec::new();
    let mut push = |offset: usize, min: u8, max: u8| {
        let species = u32_at(offset) as u16;
        if species != 0 {
            slots.push(Slot { offset, species, min_level: min, max_level: max, kind: SlotKind::U32 });
        }
    };
    let grass_level = |i: usize| u32_at(0x04 + i * 8) as u8;
    for i in 0..12 {
        push(0x08 + i * 8, grass_level(i), grass_level(i));
    }
    // Remplacements des emplacements d'herbe (essaims, jour, nuit, Pokéradar, puis
    // double-jeu GBA). Les u32 0x8C-0xA3 sont des réglages de formes, pas des espèces.
    for i in (0..10).chain(16..26) {
        push(0x64 + i * 4, grass_level(0), grass_level(0));
    }
    for area in 0..5 {
        let base = 0xCC + area * 0x2C;
        for i in 0..5 {
            let at = base + 4 + i * 8;
            push(at + 4, d[at + 1], d[at]);
        }
    }
    slots
}

fn read_bw(d: &[u8]) -> Vec<Slot> {
    d.as_chunks::<232>()
        .0
        .iter()
        .enumerate()
        .flat_map(|(season, chunk)| {
            (0..56).filter_map(move |i| {
                let at = 8 + i * 4;
                let raw = u16_at(chunk, at);
                let species = raw & 0x07FF;
                (species != 0).then_some(Slot {
                    offset: season * 232 + at,
                    species,
                    min_level: chunk[at + 2],
                    max_level: chunk[at + 3],
                    kind: SlotKind::U16Form,
                })
            })
        })
        .collect()
}

/// Remplace l'espèce d'un emplacement (la forme est remise à zéro).
pub fn set_species(data: &mut [u8], slot: &Slot, species: u16) {
    match slot.kind {
        SlotKind::U32 => data[slot.offset..slot.offset + 4].copy_from_slice(&(species as u32).to_le_bytes()),
        SlotKind::U16Form => put_u16(data, slot.offset, species & 0x07FF),
    }
}

/// Modifie les niveaux d'un emplacement (Gen 5 et eaux de Platine uniquement ;
/// l'herbe de Platine stocke ses niveaux à part et n'est pas touchée).
pub fn scale_levels(data: &mut [u8], slot: &Slot, factor: f32) {
    let scale = |l: u8| ((l as f32 * factor).round() as u32).clamp(1, 100) as u8;
    match slot.kind {
        SlotKind::U16Form => {
            data[slot.offset + 2] = scale(slot.min_level);
            data[slot.offset + 3] = scale(slot.max_level);
        }
        SlotKind::U32 => {
            if slot.offset >= 0xCC {
                data[slot.offset - 4] = scale(slot.max_level);
                data[slot.offset - 3] = scale(slot.min_level);
            } else if slot.offset < 0x64 {
                let at = slot.offset - 4;
                data[at..at + 4].copy_from_slice(&(scale(slot.min_level) as u32).to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bw_slots_with_forms_and_seasons() {
        let mut d = vec![0u8; 232 * 2];
        d[8..12].copy_from_slice(&[0x50, 0x02 | 0x08, 10, 25]); // espèce 592, forme 1
        d[232 + 8..232 + 12].copy_from_slice(&[0x50, 0x02, 3, 4]);
        let slots = read(5, &d);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].species, 592);
        assert_eq!(slots[1].offset, 240);
        set_species(&mut d, &slots[0], 25);
        assert_eq!(read(5, &d)[0].species, 25);
    }

    #[test]
    fn platinum_surf_slot() {
        let mut d = vec![0u8; 424];
        d[0xCC] = 10;
        d[0xD0..0xD8].copy_from_slice(&[30, 20, 0, 0, 54, 0, 0, 0]);
        let slots = read(4, &d);
        assert_eq!(slots, vec![Slot { offset: 0xD4, species: 54, min_level: 20, max_level: 30, kind: SlotKind::U32 }]);
        scale_levels(&mut d, &slots[0], 1.5);
        assert_eq!((d[0xD0], d[0xD1]), (45, 30));
    }
}
