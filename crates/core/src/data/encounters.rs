//! Tables de rencontres sauvages.
//!
//! - Platine : 424 octets par zone. Herbe : taux u32 puis 12 × (niveau u32, espèce u32) ;
//!   puis remplacements (essaims, jour/nuit, Pokéradar, slots double-jeu GBA) en
//!   espèces u32 ; enfin Surf, Éclate-Roc, Canne, Super Canne, Méga Canne :
//!   taux u32 + 5 × (max u8, min u8, u16, espèce u32).
//!   Diamant / Perle : même format (vérifié sur Diamant ADAF, `d_enc_data.narc`).
//! - HeartGold / SoulSilver (d'après UPR-ZX `getEncountersHGSS`, non vérifié sur une
//!   ROM) : 196 octets par zone. Taux u8 (herbe, Surf, Éclate-Roc, Canne, Super Canne,
//!   Méga Canne), 2 octets, 12 niveaux d'herbe u8, puis 3 × 12 espèces u16 (matin,
//!   jour, nuit) ; radio Hoenn / Sinnoh (4 × u16) ; Surf 5, Éclate-Roc 2, cannes 3 × 5
//!   emplacements (min u8, max u8, espèce u16) ; essaims (4 × u16).
//! - Noire/Blanche : 232 octets par saison (1 ou 4 saisons par zone). 8 octets de
//!   taux, puis 56 emplacements (espèce u16 avec forme en bits 11-15, min u8, max u8).

use super::{put_u16, u16_at};
use crate::games::Game;

/// Format des fichiers de rencontres d'un jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Diamant, Perle, Platine.
    Sinnoh,
    /// HeartGold, SoulSilver.
    Johto,
    Gen5,
    Oras,
}

impl Format {
    /// Format par défaut d'une génération (Gen 4 : Diamant / Perle / Platine).
    pub fn for_generation(generation: u8) -> Self {
        match generation {
            ..=4 => Format::Sinnoh,
            5 => Format::Gen5,
            _ => Format::Oras,
        }
    }

    pub fn for_game(game: Game) -> Self {
        match game {
            Game::HeartGold | Game::SoulSilver => Format::Johto,
            _ => Self::for_generation(game.generation()),
        }
    }
}

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
    /// Espèce u16 de HeartGold / SoulSilver (niveaux selon la position).
    U16Johto,
}

/// Lit tous les emplacements non vides d'un fichier de zone (décompressé),
/// au format par défaut de la génération (Gen 4 : Diamant / Perle / Platine).
pub fn read(generation: u8, data: &[u8]) -> Vec<Slot> {
    read_format(Format::for_generation(generation), data)
}

pub fn read_format(format: Format, data: &[u8]) -> Vec<Slot> {
    match format {
        Format::Sinnoh => read_platinum(data),
        Format::Johto => read_hgss(data),
        Format::Gen5 => read_bw(data),
        Format::Oras => read_oras(data),
    }
}

/// HeartGold / SoulSilver : positions des blocs (UPR-ZX `getEncountersHGSS`).
pub const HGSS_ZONE_SIZE: usize = 196;
pub const HGSS_GRASS: usize = 20;
pub const HGSS_RADIO: usize = 92;
pub const HGSS_WATER: usize = 100;
pub const HGSS_SWARMS: usize = 188;
/// Nombre d'emplacements de Surf, Éclate-Roc, Canne, Super Canne, Méga Canne.
pub const HGSS_WATER_SLOTS: [usize; 5] = [5, 2, 5, 5, 5];

fn read_hgss(d: &[u8]) -> Vec<Slot> {
    if d.len() < HGSS_ZONE_SIZE {
        return Vec::new();
    }
    let mut slots = Vec::new();
    let mut push = |offset: usize, min: u8, max: u8| {
        let species = u16_at(d, offset);
        if species != 0 {
            slots.push(Slot { offset, species, min_level: min, max_level: max, kind: SlotKind::U16Johto });
        }
    };
    let levels = &d[8..20];
    let (low, high) = (levels.iter().copied().filter(|&l| l > 0).min().unwrap_or(1), levels.iter().copied().max().unwrap_or(1));
    if d[0] != 0 {
        for time in 0..3 {
            for (i, &level) in levels.iter().enumerate() {
                push(HGSS_GRASS + time * 24 + i * 2, level, level);
            }
        }
    }
    // Radio Hoenn / Sinnoh : remplacent des emplacements d'herbe.
    for i in 0..4 {
        push(HGSS_RADIO + i * 2, low, high);
    }
    let mut at = HGSS_WATER;
    for (area, &count) in HGSS_WATER_SLOTS.iter().enumerate() {
        for i in 0..count {
            let e = at + i * 4;
            if d[1 + area] != 0 {
                push(e + 2, d[e], d[e + 1]);
            }
        }
        at += count * 4;
    }
    // Essaims (herbe, Surf), pêche de nuit, essaim de pêche : niveaux d'herbe à titre indicatif.
    for i in 0..4 {
        push(HGSS_SWARMS + i * 2, low, high);
    }
    slots
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
        SlotKind::U16Johto => put_u16(data, slot.offset, species),
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
        SlotKind::U16Johto => {
            if (HGSS_GRASS..HGSS_RADIO).contains(&slot.offset) {
                // Niveaux d'herbe communs aux trois moments de la journée : calculés
                // depuis le niveau d'origine, l'écriture est la même pour les trois.
                data[8 + (slot.offset - HGSS_GRASS) / 2 % 12] = scale(slot.min_level);
            } else if (HGSS_WATER..HGSS_SWARMS).contains(&slot.offset) {
                data[slot.offset - 2] = scale(slot.min_level);
                data[slot.offset - 1] = scale(slot.max_level);
            }
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
    fn hgss_zone() {
        let mut d = vec![0u8; HGSS_ZONE_SIZE];
        d[0] = 30; // herbe
        d[1] = 10; // Surf
        d[8] = 3; // niveau du 1er emplacement d'herbe
        d[20..22].copy_from_slice(&16u16.to_le_bytes()); // matin
        d[44..46].copy_from_slice(&19u16.to_le_bytes()); // jour
        d[68..70].copy_from_slice(&163u16.to_le_bytes()); // nuit
        d[100..104].copy_from_slice(&[20, 30, 54, 0]); // Surf : Psykokwak 20-30
        d[120..124].copy_from_slice(&[5, 6, 74, 0]); // Éclate-Roc sans taux : ignoré
        let slots = read_format(Format::Johto, &d);
        let species: Vec<u16> = slots.iter().map(|s| s.species).collect();
        assert_eq!(species, vec![16, 19, 163, 54]);
        assert_eq!((slots[3].min_level, slots[3].max_level), (20, 30));
        for s in &slots {
            scale_levels(&mut d, s, 2.0);
        }
        assert_eq!(d[8], 6, "niveau d'herbe partagé, mis à l'échelle une seule fois");
        assert_eq!((d[100], d[101]), (40, 60));
        set_species(&mut d, &slots[2], 25);
        assert_eq!(u16_at(&d, 68), 25);
        assert!(read(4, &d).iter().all(|s| s.kind == SlotKind::U32), "Gen 4 par défaut : Sinnoh");
        assert_eq!(Format::for_game(Game::SoulSilver), Format::Johto);
        assert_eq!(Format::for_game(Game::Diamond), Format::Sinnoh);
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
