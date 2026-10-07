//! Tables des ROM Gen 1 et 2 (lecture et écriture en place), d'après `Gen1RomHandler.java` et
//! `Gen2RomHandler.java` de l'Universal Pokémon Randomizer (UPR-ZX, GPLv3) et les
//! décompilations pokered / pokecrystal.
//!
//! - **Fiches** : Gen 1 0x1C octets (n°, PV, Att, Déf, Vit, Spécial, types, capture,
//!   expérience, …, 4 attaques de départ en 0x0F, courbe en 0x13) ; Gen 2 0x20 octets (n°,
//!   6 statistiques, types, capture, expérience, objets, sexe, …, courbe en 0x16). Types
//!   en codes GB. Converties en fiches au format Gen 3/4 de Kaleido (0x2C) pour le
//!   randomizer et l'éditeur.
//! - **Évolutions et attaques** : table de pointeurs 16 bits (par espèce interne en Gen 1),
//!   dans la même banque : évolutions terminées par 0, puis paires (niveau, attaque)
//!   terminées par 0. Évolutions Gen 1 : niveau `[1, niv, esp]`, objet `[2, objet, 1, esp]`,
//!   échange `[3, 1, esp]` ; Gen 2 : niveau `[1, niv, esp]`, objet `[2, objet, esp]`,
//!   échange `[3, objet|0xFF, esp]`, bonheur `[4, cond, esp]`, statistiques
//!   `[5, niv, cond, esp]`. Réécrites **sur place** (même longueur) : les données
//!   suivantes ne bougent pas.
//! - **Rencontres** : chaque emplacement est repéré par l'offset de son niveau et de son
//!   espèce, ce qui couvre les formats variés (herbe et eau Gen 1, cannes, matin / jour /
//!   nuit Gen 2).
//! - **Dresseurs** : listes par classe (nombre par classe dans le fichier d'offsets). Gen 1 :
//!   `[niv, esp…, 0]` (niveau commun) ou `[0xFF, (niv, esp)…, 0]` ; Gen 2 : nom terminé par
//!   0x50, type (bit 0 attaques, bit 1 objet), puis `(niv, esp, [objet], [4 attaques])`
//!   jusqu'à 0xFF.

use kaleido_formats::gb::bank_offset;

use crate::gb_rom::GbGameRom;
use crate::pokemon::{gb_type_to_gen4, gen4_type_to_gb};
use crate::rom::RomError;

fn bad(what: &str) -> RomError {
    RomError::Layout(format!("{what} : données Gen 1 / 2 inattendues"))
}

// ---------------------------------------------------------------------------
// Fiches

/// Fiche GB → fiche au format Gen 3/4 (0x2C : statistiques PV, Att, Déf, Vit, Atq Spé,
/// Déf Spé, types Gen 4, capture, expérience ; sexe, éclosion et courbe en Gen 2).
pub fn to_gen4_record(generation: u8, d: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; 0x2C];
    let t = |b: u8| gb_type_to_gen4(b).unwrap_or(0);
    if generation == 1 {
        r[..6].copy_from_slice(&[d[1], d[2], d[3], d[4], d[5], d[5]]);
        r[6] = t(d[6]);
        r[7] = t(d[7]);
        r[8] = d[8];
        r[9] = d[9];
        r[0x10] = 127;
        r[0x13] = d[0x13];
    } else {
        r[..6].copy_from_slice(&d[1..7]);
        r[6] = t(d[7]);
        r[7] = t(d[8]);
        r[8] = d[9];
        r[9] = d[10];
        r[0x0C] = d[0x0B];
        r[0x0E] = d[0x0C];
        r[0x10] = d[0x0D];
        r[0x11] = d[0x0F];
        r[0x13] = d[0x16];
    }
    r[0x12] = 70;
    r
}

/// Réécrit statistiques, types et taux de capture d'une fiche GB depuis une fiche Gen 3/4.
/// Gen 1 : le Spécial prend l'Attaque Spéciale.
pub fn from_gen4_record(generation: u8, original: &[u8], r: &[u8]) -> Vec<u8> {
    let mut d = original.to_vec();
    let gb = |t: u8| gen4_type_to_gb(t).unwrap_or(0);
    if generation == 1 {
        d[1..6].copy_from_slice(&[r[0], r[1], r[2], r[3], r[4]]);
        d[6] = gb(r[6]);
        d[7] = gb(r[7]);
        d[8] = r[8];
    } else {
        d[1..7].copy_from_slice(&r[..6]);
        d[7] = gb(r[6]);
        d[8] = gb(r[7]);
        d[9] = r[8];
    }
    d
}

// ---------------------------------------------------------------------------
// Évolutions et attaques

/// Une évolution GB : position dans la ROM et taille, pour la réécrire sur place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbEvo {
    pub at: usize,
    pub method: u8,
    /// Niveau, objet ou condition (premier paramètre).
    pub param: u8,
    /// N° national de l'espèce obtenue.
    pub target: u16,
    pub len: usize,
}

/// Évolutions et attaques par niveau d'une espèce (pointeur de la table, dans sa banque).
#[derive(Debug, Clone)]
pub struct EvoMoves {
    pub evolutions: Vec<GbEvo>,
    /// (niveau, attaque) dans l'ordre de la ROM.
    pub moves: Vec<(u8, u8)>,
    /// Début de la liste d'attaques.
    pub moves_at: usize,
}

fn table_pointer(g: &GbGameRom, national: u16) -> Result<usize, RomError> {
    let table = g.entry.value("PokemonMovesetsTableOffset").ok_or_else(|| bad("table des attaques"))?;
    let index = if g.generation == 1 { g.internal(national) as usize } else { national as usize };
    if index == 0 {
        return Err(bad("espèce interne"));
    }
    g.rom().pointer_in(table + (index - 1) * 2, table / 0x4000).ok_or_else(|| bad("pointeur des attaques"))
}

pub fn evo_moves(g: &GbGameRom, national: u16) -> Result<EvoMoves, RomError> {
    let rom = g.rom();
    let mut at = table_pointer(g, national)?;
    let byte = |p: usize| rom.u8(p).ok_or_else(|| bad("évolutions"));
    let mut evolutions = Vec::new();
    for _ in 0..8 {
        let method = byte(at)?;
        if method == 0 {
            break;
        }
        let (len, target_at) = match (g.generation, method) {
            (1, 2) => (4, at + 3),
            (2, 5) => (4, at + 3),
            _ => (3, at + 2),
        };
        let target = byte(target_at)?;
        let target = if g.generation == 1 { g.national(target) } else { target as u16 };
        evolutions.push(GbEvo { at, method, param: byte(at + 1)?, target, len });
        at += len;
    }
    at += 1;
    let moves_at = at;
    let mut moves = Vec::new();
    for _ in 0..40 {
        let level = byte(at)?;
        if level == 0 {
            break;
        }
        moves.push((level, byte(at + 1)?));
        at += 2;
    }
    Ok(EvoMoves { evolutions, moves, moves_at })
}

/// Attaques de départ de la fiche Gen 1 (4 octets en 0x0F), niveau 1.
pub fn starting_moves(g: &GbGameRom, national: u16) -> Vec<u8> {
    if g.generation != 1 {
        return Vec::new();
    }
    g.personal(national).map(|p| p[0x0F..0x13].iter().copied().filter(|&m| m != 0).collect()).unwrap_or_default()
}

/// Liste d'attaques au format Gen 4 de Kaleido (`attaque | niveau << 9`, terminée par 0xFFFF),
/// attaques de départ de la Gen 1 comprises (niveau 1).
pub fn learnset_gen4(g: &GbGameRom, national: u16) -> Result<Vec<u8>, RomError> {
    let em = evo_moves(g, national)?;
    let mut out = Vec::new();
    for m in starting_moves(g, national) {
        out.extend((m as u16 | 1 << 9).to_le_bytes());
    }
    for (level, mv) in em.moves {
        out.extend((mv as u16 | (level as u16) << 9).to_le_bytes());
    }
    out.extend([0xFF, 0xFF]);
    Ok(out)
}

/// Réécrit les attaques (format Gen 4) sur place : les attaques de départ de la Gen 1 dans
/// la fiche, les autres dans la liste ; au plus autant qu'à l'origine.
pub fn set_learnset_gen4(g: &mut GbGameRom, national: u16, data: &[u8]) -> Result<(), RomError> {
    let moves = crate::data::learnsets::read(4, data);
    let em = evo_moves(g, national)?;
    let start = starting_moves(g, national);
    let (first, rest) = moves.split_at(start.len().min(moves.len()));
    if rest.len() > em.moves.len() {
        return Err(RomError::Unsupported(format!("Pokémon n°{national} : {} attaques par niveau au plus sur Game Boy", em.moves.len() + start.len())));
    }
    if g.generation == 1 && !start.is_empty() {
        let mut p = g.personal(national).ok_or_else(|| bad("fiche"))?.to_vec();
        for (i, slot) in p[0x0F..0x13].iter_mut().enumerate() {
            *slot = first.get(i).map_or(0, |&(m, _)| m as u8);
        }
        g.set_personal(national, &p)?;
    }
    let mut at = em.moves_at;
    for &(mv, level) in rest {
        g.rom_mut().write(at, &[level.max(1), mv as u8])?;
        at += 2;
    }
    if rest.len() < em.moves.len() {
        g.rom_mut().write_u8(at, 0)?;
    }
    Ok(())
}

/// Évolutions au format Gen 4 de Kaleido (6 octets : méthode, paramètre, cible) :
/// niveau → 4, objet → 7, échange → 5 (6 avec objet), bonheur → 1, statistiques → 8.
pub fn evolutions_gen4(g: &GbGameRom, national: u16) -> Result<Vec<u8>, RomError> {
    let mut out = Vec::new();
    for e in evo_moves(g, national)?.evolutions {
        let (method, param) = match e.method {
            1 => (4u16, e.param as u16),
            2 => (7, e.param as u16),
            3 if g.generation == 2 && e.param != 0xFF => (6, e.param as u16),
            3 => (5, 0),
            4 => (1, 0),
            _ => (8, e.param as u16),
        };
        out.extend(method.to_le_bytes());
        out.extend(param.to_le_bytes());
        out.extend(e.target.to_le_bytes());
    }
    Ok(out)
}

/// Évolutions sans échange (UPR `removeImpossibleEvolutions`) : échange → niveau 37 ;
/// Gen 2, échange avec objet → utilisation de l'objet. Sur place. Renvoie les espèces obtenues.
pub fn remove_trade_evolutions(g: &mut GbGameRom, national: u16) -> Result<Vec<u16>, RomError> {
    let mut changed = Vec::new();
    for e in evo_moves(g, national)?.evolutions {
        if e.method != 3 {
            continue;
        }
        let bytes = if g.generation == 2 && e.param != 0xFF { [2, e.param] } else { [1, 37] };
        g.rom_mut().write(e.at, &bytes)?;
        changed.push(e.target);
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------
// Rencontres

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbSlot {
    pub level_at: usize,
    pub species_at: usize,
    pub level: u8,
    /// N° national.
    pub species: u16,
}

#[derive(Debug, Clone)]
pub struct GbArea {
    pub label: String,
    pub slots: Vec<GbSlot>,
}

fn slot(g: &GbGameRom, level_at: usize, species_at: usize) -> Option<GbSlot> {
    let raw = g.rom().u8(species_at)?;
    let species = if g.generation == 1 { g.national(raw) } else { raw as u16 };
    Some(GbSlot { level_at, species_at, level: g.rom().u8(level_at)?, species })
}

/// Toutes les zones de rencontres (sans doublon d'emplacement).
pub fn wild_areas(g: &GbGameRom) -> Result<Vec<GbArea>, RomError> {
    if g.generation == 1 {
        gen1_wild(g)
    } else {
        gen2_wild(g)
    }
}

fn gen1_wild(g: &GbGameRom) -> Result<Vec<GbArea>, RomError> {
    let rom = g.rom();
    let e = g.entry;
    let mut out = Vec::new();
    let mut table = e.value("WildPokemonTableOffset").ok_or_else(|| bad("rencontres"))?;
    let bank = table / 0x4000;
    let mut seen = Vec::new();
    let mut map = 0;
    while rom.u16(table).is_some_and(|w| w != 0xFFFF) && map < 256 {
        let mut at = rom.pointer_in(table, bank).ok_or_else(|| bad("rencontres"))?;
        if !seen.contains(&at) {
            seen.push(at);
            for kind in ["Herbe", "Surf"] {
                let rate = rom.u8(at).ok_or_else(|| bad("rencontres"))?;
                at += 1;
                if rate == 0 {
                    continue;
                }
                let slots = (0..10).filter_map(|k| slot(g, at + 2 * k, at + 2 * k + 1)).collect();
                out.push(GbArea { label: format!("Carte {map} ({kind})"), slots });
                at += 20;
            }
        }
        table += 2;
        map += 1;
    }
    if let Some(old) = e.value("OldRodOffset") {
        out.extend(slot(g, old + 2, old + 1).map(|s| GbArea { label: "Canne".into(), slots: vec![s] }));
    }
    if let Some(good) = e.value("GoodRodOffset") {
        let slots = (0..2).filter_map(|k| slot(g, good + 2 * k, good + 2 * k + 1)).collect();
        out.push(GbArea { label: "Super Canne".into(), slots });
    }
    if let Some(mut sup) = e.value("SuperRodTableOffset") {
        let sbank = sup / 0x4000;
        let mut seen = Vec::new();
        for _ in 0..128 {
            let Some(map) = rom.u8(sup).filter(|&m| m != 0xFF) else { break };
            sup += 1;
            if g.is_yellow() {
                // Jaune : 4 emplacements (espèce, niveau) directement après le n° de carte.
                let slots = (0..4).filter_map(|k| slot(g, sup + 2 * k + 1, sup + 2 * k)).collect();
                out.push(GbArea { label: format!("Méga Canne, carte {map}"), slots });
                sup += 8;
            } else {
                let set = rom.pointer_in(sup, sbank).ok_or_else(|| bad("Méga Canne"))?;
                sup += 2;
                if seen.contains(&set) {
                    continue;
                }
                seen.push(set);
                let n = rom.u8(set).unwrap_or(0).min(10) as usize;
                let slots = (0..n).filter_map(|k| slot(g, set + 1 + 2 * k, set + 2 + 2 * k)).collect();
                out.push(GbArea { label: format!("Méga Canne, carte {map}"), slots });
            }
        }
    }
    Ok(out)
}

fn gen2_wild(g: &GbGameRom) -> Result<Vec<GbArea>, RomError> {
    let rom = g.rom();
    let mut at = g.entry.value("WildPokemonOffset").ok_or_else(|| bad("rencontres"))?;
    let mut out = Vec::new();
    // Johto herbe, Johto eau, Kanto herbe, Kanto eau, spéciaux herbe, spéciaux eau.
    for section in 0..6 {
        let land = section % 2 == 0;
        for _ in 0..256 {
            let (Some(group), Some(map)) = (rom.u8(at), rom.u8(at + 1)) else { return Err(bad("rencontres")) };
            if group == 0xFF {
                at += 1;
                break;
            }
            if land {
                for (t, tod) in ["matin", "jour", "nuit"].iter().enumerate() {
                    let base = at + 5 + t * 14;
                    let slots = (0..7).filter_map(|k| slot(g, base + 2 * k, base + 2 * k + 1)).collect();
                    out.push(GbArea { label: format!("Carte {group}.{map} (herbe, {tod})"), slots });
                }
                at += 5 + 42;
            } else {
                let slots = (0..3).filter_map(|k| slot(g, at + 3 + 2 * k, at + 4 + 2 * k)).collect();
                out.push(GbArea { label: format!("Carte {group}.{map} (surf)"), slots });
                at += 3 + 6;
            }
        }
    }
    Ok(out)
}

/// Réécrit espèce et niveau des emplacements.
pub fn set_wild_area(g: &mut GbGameRom, area: &GbArea) -> Result<(), RomError> {
    for s in &area.slots {
        let raw = if g.generation == 1 { g.internal(s.species) } else { s.species as u8 };
        g.rom_mut().write_u8(s.species_at, raw)?;
        g.rom_mut().write_u8(s.level_at, s.level.max(1))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Dresseurs

#[derive(Debug, Clone)]
pub struct GbTrainerMon {
    pub level_at: usize,
    pub species_at: usize,
    pub level: u8,
    pub species: u16,
    /// Gen 2 : emplacement des 4 attaques (dresseurs « avec attaques »).
    pub moves_at: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct GbTrainer {
    pub class: usize,
    /// Rang global (1, 2…) dans l'ordre des classes.
    pub index: usize,
    pub name: String,
    pub party: Vec<GbTrainerMon>,
}

pub fn trainers(g: &GbGameRom) -> Result<Vec<GbTrainer>, RomError> {
    let rom = g.rom();
    let e = g.entry;
    let table = e.value("TrainerDataTableOffset").ok_or_else(|| bad("dresseurs"))?;
    let counts = e.array("TrainerDataClassCounts");
    let classes = if g.generation == 1 { 47 } else { e.value("TrainerClassAmount").ok_or_else(|| bad("classes de dresseurs"))? };
    let bank = table / 0x4000;
    let mut out = Vec::new();
    for c in 0..classes {
        let class_index = if g.generation == 1 { c + 1 } else { c };
        let mut at = rom.pointer_in(table + c * 2, bank).ok_or_else(|| bad("dresseurs"))?;
        for _ in 0..counts.get(class_index).copied().unwrap_or(0) {
            let mut t = GbTrainer { class: class_index, index: out.len() + 1, name: String::new(), party: Vec::new() };
            let byte = |p: usize| rom.u8(p).ok_or_else(|| bad("dresseurs"));
            if g.generation == 1 {
                let first = byte(at)?;
                at += 1;
                if first == 0xFF {
                    while byte(at)? != 0 && t.party.len() < 6 {
                        let raw = byte(at + 1)?;
                        t.party.push(GbTrainerMon { level_at: at, species_at: at + 1, level: byte(at)?, species: g.national(raw), moves_at: None });
                        at += 2;
                    }
                } else {
                    let level_at = at - 1;
                    while byte(at)? != 0 && t.party.len() < 6 {
                        t.party.push(GbTrainerMon { level_at, species_at: at, level: first, species: g.national(byte(at)?), moves_at: None });
                        at += 1;
                    }
                }
                at += 1;
            } else {
                let start = at;
                while byte(at)? != crate::text::gen12::TERMINATOR && at - start < 16 {
                    at += 1;
                }
                t.name = crate::text::gen12::decode(rom.bytes(start, at - start + 1).unwrap_or(&[]));
                at += 1;
                let kind = byte(at)?;
                at += 1;
                while byte(at)? != 0xFF && t.party.len() < 6 {
                    let mut m = GbTrainerMon { level_at: at, species_at: at + 1, level: byte(at)?, species: byte(at + 1)? as u16, moves_at: None };
                    at += 2;
                    if kind & 2 != 0 {
                        at += 1;
                    }
                    if kind & 1 != 0 {
                        m.moves_at = Some(at);
                        at += 4;
                    }
                    t.party.push(m);
                }
                at += 1;
            }
            out.push(t);
        }
    }
    Ok(out)
}

/// Réécrit espèces, niveaux et attaques d'une équipe (même nombre de Pokémon).
pub fn set_party(g: &mut GbGameRom, t: &GbTrainer, moves: &[[u8; 4]]) -> Result<(), RomError> {
    for (k, m) in t.party.iter().enumerate() {
        let raw = if g.generation == 1 { g.internal(m.species) } else { m.species as u8 };
        g.rom_mut().write_u8(m.species_at, raw)?;
        g.rom_mut().write_u8(m.level_at, m.level.max(1))?;
        if let (Some(at), Some(mv)) = (m.moves_at, moves.get(k)) {
            g.rom_mut().write(at, mv)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Starters et Pokémon fixes

/// Starters (n° nationaux). Jaune : Pikachu et l'Évoli du rival seulement.
pub fn starters(g: &GbGameRom) -> Result<Vec<u16>, RomError> {
    let count = if g.is_yellow() { 2 } else { 3 };
    (1..=count)
        .map(|k| {
            let at = *g.entry.array(&format!("StarterOffsets{k}")).first().ok_or_else(|| bad("starters"))?;
            let raw = g.rom().u8(at).ok_or_else(|| bad("starters"))?;
            Ok(if g.generation == 1 { g.national(raw) } else { raw as u16 })
        })
        .collect()
}

pub fn set_starters(g: &mut GbGameRom, starters: &[u16]) -> Result<(), RomError> {
    for (k, &s) in starters.iter().enumerate() {
        let raw = if g.generation == 1 { g.internal(s) } else { s as u8 };
        for at in g.entry.array(&format!("StarterOffsets{}", k + 1)) {
            g.rom_mut().write_u8(at, raw)?;
        }
    }
    Ok(())
}

/// Rencontres fixes : (espèce, niveaux).
pub fn statics(g: &GbGameRom) -> Vec<(u16, Vec<u8>)> {
    g.entry
        .statics
        .iter()
        .map(|s| {
            let raw = s.species.first().and_then(|&at| g.rom().u8(at)).unwrap_or(0);
            let species = if g.generation == 1 { g.national(raw) } else { raw as u16 };
            (species, s.levels.iter().filter_map(|&at| g.rom().u8(at)).collect())
        })
        .collect()
}

pub fn set_static(g: &mut GbGameRom, index: usize, species: u16, level: Option<u8>) -> Result<(), RomError> {
    let def = g.entry.statics.get(index).ok_or_else(|| bad("rencontre fixe"))?.clone();
    let raw = if g.generation == 1 { g.internal(species) } else { species as u8 };
    for at in def.species {
        g.rom_mut().write_u8(at, raw)?;
    }
    if let Some(level) = level {
        for at in def.levels {
            g.rom_mut().write_u8(at, level)?;
        }
    }
    Ok(())
}

/// Offset d'un pointeur de banque (réexport pratique pour les tests).
pub fn resolve(bank: usize, pointer: u16) -> usize {
    bank_offset(bank, pointer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_rom::synthetic;

    fn open(section: &str) -> GbGameRom {
        GbGameRom::from_rom(synthetic::build(section)).unwrap()
    }

    #[test]
    fn records_round_trip() {
        for section in ["Red (F)", "Crystal (F)"] {
            let mut g = open(section);
            let raw = g.personal(1).unwrap().to_vec();
            let rec = to_gen4_record(g.generation, &raw);
            assert_eq!(&rec[..3], &[45, 49, 49]);
            assert_eq!(crate::pokemon::PokeType::from_index(3, rec[6]), Some(crate::pokemon::PokeType::Grass));
            assert_eq!(from_gen4_record(g.generation, &raw, &rec), raw, "{section}");
            let mut changed = rec.clone();
            changed[6] = 10; // Feu
            g.set_personal(1, &from_gen4_record(g.generation, &raw, &changed)).unwrap();
            assert_eq!(to_gen4_record(g.generation, g.personal(1).unwrap())[6], 10);
        }
    }

    #[test]
    fn evolutions_and_moves() {
        for section in ["Blue (U)", "Gold (F)"] {
            let mut g = open(section);
            let em = evo_moves(&g, 1).unwrap();
            assert_eq!(em.evolutions.len(), 1);
            assert_eq!((em.evolutions[0].method, em.evolutions[0].param, em.evolutions[0].target), (1, 16, 2));
            assert_eq!(em.moves, vec![(7, 22), (13, 73)]);
            let evo = crate::data::evolutions::read(&evolutions_gen4(&g, 1).unwrap());
            assert_eq!(evo[0].target, 2);
            let ls = learnset_gen4(&g, 2).unwrap();
            let moves = crate::data::learnsets::read(4, &ls);
            assert!(moves.contains(&(33, 5)));
            // Réécriture d'une attaque, même longueur.
            let mut new = moves.clone();
            let last = new.len() - 1;
            new[last].0 = 89;
            let enc: Vec<u8> = new.iter().flat_map(|&(m, l)| (m | (l as u16) << 9).to_le_bytes()).chain([0xFF, 0xFF]).collect();
            set_learnset_gen4(&mut g, 2, &enc).unwrap();
            assert_eq!(crate::data::learnsets::read(4, &learnset_gen4(&g, 2).unwrap()), new);
            // Une attaque de trop : refusée.
            let mut too_many = enc.clone();
            too_many.splice(0..0, (1u16 | 1 << 9).to_le_bytes());
            too_many.splice(0..0, (2u16 | 1 << 9).to_le_bytes());
            too_many.splice(0..0, (3u16 | 1 << 9).to_le_bytes());
            too_many.splice(0..0, (4u16 | 1 << 9).to_le_bytes());
            too_many.splice(0..0, (5u16 | 1 << 9).to_le_bytes());
            assert!(set_learnset_gen4(&mut g, 2, &too_many).is_err());
        }
    }

    #[test]
    fn wild_trainers_starters() {
        for section in ["Red (F)", "Crystal (F)"] {
            let mut g = open(section);
            let areas = wild_areas(&g).unwrap();
            assert!(!areas.is_empty(), "{section}");
            assert!(areas[0].slots.iter().any(|s| s.species == 16));
            let mut a = areas[0].clone();
            a.slots[0].species = 150;
            a.slots[0].level = 70;
            set_wild_area(&mut g, &a).unwrap();
            let back = wild_areas(&g).unwrap();
            assert_eq!((back[0].slots[0].species, back[0].slots[0].level), (150, 70));

            let ts = trainers(&g).unwrap();
            assert!(!ts.is_empty());
            assert_eq!(ts[0].party.len(), 2);
            assert_eq!((ts[0].party[0].level, ts[0].party[0].species), (5, 16));
            let mut t = ts[0].clone();
            t.party[1].species = 151;
            set_party(&mut g, &t, &[]).unwrap();
            assert_eq!(trainers(&g).unwrap()[0].party[1].species, 151);

            assert_eq!(starters(&g).unwrap(), vec![1, 4, 7]);
            let n = g.species_count();
            set_starters(&mut g, &[152.min(n), 155.min(n), 150]).unwrap();
            assert_eq!(starters(&g).unwrap()[0], 152.min(n));
        }
    }
}
