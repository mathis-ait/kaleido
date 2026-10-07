//! Tables de données des ROM Gen 3 (lecture et écriture en place).
//!
//! Formats d'après `Gen3RomHandler.java` de l'Universal Pokémon Randomizer (UPR-ZX,
//! GPLv3) et les décompilations pokeruby / pokeemerald / pokefirered :
//!
//! - **Attaques par niveau** : pointeur par espèce interne vers une liste de u16
//!   `attaque (9 bits) | niveau << 9`, terminée par 0xFFFF (même codage qu'en Gen 4).
//!   Une liste plus longue est déplacée dans la zone libre (`FreeSpace`).
//! - **Évolutions** : 5 entrées de 8 octets par espèce interne (méthode u16, paramètre
//!   u16, espèce interne u16, 0). Les méthodes 1 à 15 sont celles de la Gen 4.
//! - **Rencontres** : en-têtes de 20 octets (banque, carte, 2 octets, puis 4 pointeurs :
//!   herbe, surf, Éclate-Roc, canne), terminés par 0xFF 0xFF. Chaque pointeur mène à
//!   (taux u8, 3 octets, pointeur des emplacements) ; un emplacement = niveau min,
//!   niveau max, espèce interne u16. 12 emplacements dans l'herbe, 5 en surf et
//!   Éclate-Roc, 10 à la canne (2 Canne, 3 Super Canne, 5 Méga Canne).
//! - **Dresseurs** : entrées de 40 octets ; format de l'équipe en +0 (bit 0 attaques,
//!   bit 1 objet), classe en +1, nom en +4, combat double en `taille - 16`, nombre de
//!   Pokémon en `taille - 8`, pointeur de l'équipe en `taille - 4`. Un Pokémon = IV
//!   u16 (0-255), niveau u16, espèce interne u16, puis objet et/ou 4 attaques.
//! - **Starters** : Rubis / Saphir / Émeraude : 3 u16 consécutifs ; Rouge Feu / Vert
//!   Feuille : 3 scripts (+0, +515, +461), chacun avec l'espèce du rival en +5.
//! - **CT** : 50 u16 (attaque de chaque CT), une seconde copie en Émeraude / RFVF ;
//!   compatibilité CT/CS : 8 octets (64 bits) par espèce interne.

use crate::gba_rom::{GbaGameRom, EVOLUTIONS_SIZE, HM_COUNT, SPECIES_COUNT, TM_COUNT};
use crate::rom::RomError;
use crate::text::gen3 as text;
use kaleido_formats::gba::FREE_BYTE;

fn bad(what: &str) -> RomError {
    RomError::Layout(format!("{what} : données Gen 3 inattendues"))
}

// ---------------------------------------------------------------------------
// Attaques par niveau

/// Liste brute (avec son terminateur 0xFFFF) de l'espèce, au format Gen 4.
pub fn learnset(g: &GbaGameRom, national: u16) -> Result<Vec<u8>, RomError> {
    let i = g.internal(national) as usize;
    let at = g.rom().pointer(g.layout.movesets + i * 4).ok_or_else(|| bad("pointeur des attaques"))?;
    let mut out = Vec::new();
    let mut p = at;
    loop {
        let v = g.rom().u16(p).ok_or_else(|| bad("attaques par niveau"))?;
        out.extend(v.to_le_bytes());
        if v == 0xFFFF {
            return Ok(out);
        }
        p += 2;
        if out.len() > 200 {
            return Err(bad("attaques par niveau (pas de terminateur)"));
        }
    }
}

/// Écrit une liste d'attaques (format Gen 4, terminée par 0xFFFF). Sur place si elle
/// tient, sinon dans la zone libre (avec 2 octets de marge, comme l'UPR).
pub fn set_learnset(g: &mut GbaGameRom, national: u16, data: &[u8]) -> Result<(), RomError> {
    let mut data = data.to_vec();
    if !data.ends_with(&[0xFF, 0xFF]) {
        data.extend([0xFF, 0xFF]);
    }
    let old = learnset(g, national)?;
    let i = g.internal(national) as usize;
    let slot = g.layout.movesets + i * 4;
    if data.len() <= old.len() {
        let at = g.rom().pointer(slot).ok_or_else(|| bad("pointeur des attaques"))?;
        g.rom_mut().write(at, &data)?;
        return Ok(());
    }
    let free = g.layout.free_space;
    let at = g.rom().find_free_space(data.len() + 2, free, 4).ok_or_else(|| RomError::Unsupported("ROM pleine : plus de place pour les attaques".into()))?;
    g.rom_mut().write(at, &data)?;
    // Marge après le terminateur : la zone ne passe plus pour libre.
    g.rom_mut().write(at + data.len(), &[0, 0])?;
    g.rom_mut().write_pointer(slot, at)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Évolutions

/// Évolutions de l'espèce, converties au format Gen 4 (entrées de 6 octets, cibles en n° national).
pub fn evolutions(g: &GbaGameRom, national: u16) -> Result<Vec<u8>, RomError> {
    let i = g.internal(national) as usize;
    let at = g.layout.evolutions + i * EVOLUTIONS_SIZE;
    let raw = g.rom().bytes(at, EVOLUTIONS_SIZE).ok_or_else(|| bad("évolutions"))?;
    let mut out = Vec::with_capacity(5 * 6);
    for e in raw.chunks(8) {
        let (method, param, target) = (u16::from_le_bytes([e[0], e[1]]), u16::from_le_bytes([e[2], e[3]]), u16::from_le_bytes([e[4], e[5]]));
        let target = if (1..=15).contains(&method) { g.national(target) } else { 0 };
        let method = if target == 0 { 0 } else { method };
        out.extend(method.to_le_bytes());
        out.extend(if target == 0 { 0 } else { param }.to_le_bytes());
        out.extend(target.to_le_bytes());
    }
    Ok(out)
}

/// Écrit des évolutions au format Gen 4 (5 entrées au plus).
pub fn set_evolutions(g: &mut GbaGameRom, national: u16, data: &[u8]) -> Result<(), RomError> {
    let i = g.internal(national) as usize;
    let at = g.layout.evolutions + i * EVOLUTIONS_SIZE;
    let mut raw = vec![0u8; EVOLUTIONS_SIZE];
    for (k, e) in data.chunks(6).take(5).enumerate() {
        if e.len() < 6 {
            break;
        }
        let (method, param, target) = (u16::from_le_bytes([e[0], e[1]]), u16::from_le_bytes([e[2], e[3]]), u16::from_le_bytes([e[4], e[5]]));
        if method == 0 || target == 0 {
            continue;
        }
        let o = k * 8;
        raw[o..o + 2].copy_from_slice(&method.to_le_bytes());
        raw[o + 2..o + 4].copy_from_slice(&param.to_le_bytes());
        raw[o + 4..o + 6].copy_from_slice(&g.internal(target).to_le_bytes());
    }
    g.rom_mut().write(at, &raw)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Starters

const RSE_STARTER_STEP: usize = 2;
const FRLG_STARTER_2: usize = 515;
const FRLG_STARTER_3: usize = 461;
const FRLG_REPEAT: usize = 5;

pub fn starters(g: &GbaGameRom) -> Result<[u16; 3], RomError> {
    let base = g.layout.starters;
    let offsets = if g.kind.is_frlg() { [0, FRLG_STARTER_2, FRLG_STARTER_3] } else { [0, RSE_STARTER_STEP, 2 * RSE_STARTER_STEP] };
    let mut out = [0u16; 3];
    for (slot, off) in out.iter_mut().zip(offsets) {
        *slot = g.national(g.rom().u16(base + off).ok_or_else(|| bad("starters"))?);
        if *slot == 0 {
            return Err(bad("starters"));
        }
    }
    Ok(out)
}

/// Écrit les trois starters (n° nationaux). Rouge Feu / Vert Feuille : le rival prend
/// le starter qui a l'avantage du type, comme dans le jeu (`setStarters` de l'UPR).
pub fn set_starters(g: &mut GbaGameRom, starters: [u16; 3]) -> Result<(), RomError> {
    let base = g.layout.starters;
    let [a, b, c] = starters.map(|s| g.internal(s));
    let writes: Vec<(usize, u16)> = if g.kind.is_frlg() {
        vec![(0, a), (FRLG_REPEAT, b), (FRLG_STARTER_2, b), (FRLG_STARTER_2 + FRLG_REPEAT, c), (FRLG_STARTER_3, c), (FRLG_STARTER_3 + FRLG_REPEAT, a)]
    } else {
        vec![(0, a), (RSE_STARTER_STEP, b), (2 * RSE_STARTER_STEP, c)]
    };
    for (off, s) in writes {
        g.rom_mut().write_u16(base + off, s)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rencontres sauvages

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WildKind {
    Grass,
    Surf,
    RockSmash,
    Fishing,
}

impl WildKind {
    pub fn slots(self) -> usize {
        match self {
            WildKind::Grass => 12,
            WildKind::Surf | WildKind::RockSmash => 5,
            WildKind::Fishing => 10,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WildKind::Grass => "Herbe",
            WildKind::Surf => "Surf",
            WildKind::RockSmash => "Éclate-Roc",
            WildKind::Fishing => "Canne",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WildSlot {
    pub min_level: u8,
    pub max_level: u8,
    /// N° national.
    pub species: u16,
}

#[derive(Debug, Clone)]
pub struct WildArea {
    pub bank: u8,
    pub map: u8,
    pub kind: WildKind,
    pub rate: u8,
    /// Emplacement des créneaux dans la ROM.
    pub slots_at: usize,
    pub slots: Vec<WildSlot>,
}

/// Toutes les zones de rencontres, sans doublon (zones partagées par plusieurs cartes).
pub fn wild_areas(g: &GbaGameRom) -> Result<Vec<WildArea>, RomError> {
    let rom = g.rom();
    let mut out: Vec<WildArea> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut at = g.layout.wild;
    for _ in 0..1024 {
        let (Some(bank), Some(map)) = (rom.u8(at), rom.u8(at + 1)) else { return Err(bad("rencontres")) };
        if bank == 0xFF && map == 0xFF {
            return Ok(out);
        }
        for (k, kind) in [WildKind::Grass, WildKind::Surf, WildKind::RockSmash, WildKind::Fishing].into_iter().enumerate() {
            let Some(info) = rom.pointer(at + 4 + k * 4) else { continue };
            let Some(rate) = rom.u8(info).filter(|&r| r != 0) else { continue };
            let Some(slots_at) = rom.pointer(info + 4) else { continue };
            if !seen.insert(slots_at) {
                continue;
            }
            let slots = (0..kind.slots())
                .map(|s| {
                    let p = slots_at + s * 4;
                    Some(WildSlot { min_level: rom.u8(p)?, max_level: rom.u8(p + 1)?, species: g.national(rom.u16(p + 2)?) })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| bad("emplacements de rencontre"))?;
            out.push(WildArea { bank, map, kind, rate, slots_at, slots });
        }
        at += 20;
    }
    Err(bad("rencontres (liste sans fin)"))
}

/// Réécrit les créneaux d'une zone (mêmes nombres d'emplacements).
pub fn set_wild_area(g: &mut GbaGameRom, area: &WildArea) -> Result<(), RomError> {
    for (s, slot) in area.slots.iter().enumerate() {
        let p = area.slots_at + s * 4;
        let internal = g.internal(slot.species);
        let rom = g.rom_mut();
        rom.write(p, &[slot.min_level, slot.max_level.max(slot.min_level)])?;
        rom.write_u16(p + 2, internal)?;
    }
    Ok(())
}

/// Section de carte (`mapsec`, = lieu de rencontre des Pokémon) d'une carte.
pub fn map_section(g: &GbaGameRom, bank: u8, map: u8) -> Option<u8> {
    let rom = g.rom();
    let banks = g.layout.map_banks?;
    let bank_at = rom.pointer(banks + bank as usize * 4)?;
    let header = rom.pointer(bank_at + map as usize * 4)?;
    rom.u8(header + 0x14)
}

// ---------------------------------------------------------------------------
// Dresseurs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerMon {
    /// « IV » du dresseur (0 à 255 pour 0 à 31).
    pub iv: u16,
    pub level: u16,
    /// N° national.
    pub species: u16,
    pub item: u16,
    pub moves: [u16; 4],
}

#[derive(Debug, Clone)]
pub struct Trainer {
    pub index: usize,
    /// Bit 0 : attaques choisies ; bit 1 : objets tenus.
    pub party_kind: u8,
    pub class: u8,
    pub name: String,
    pub double: bool,
    pub party_at: usize,
    pub party: Vec<TrainerMon>,
}

impl Trainer {
    pub fn has_moves(&self) -> bool {
        self.party_kind & 1 != 0
    }

    pub fn has_items(&self) -> bool {
        self.party_kind & 2 != 0
    }

    fn mon_size(kind: u8) -> usize {
        if kind & 1 != 0 {
            16
        } else {
            8
        }
    }
}

/// Dresseur `index` (1 à `TrainerCount - 1`).
pub fn trainer(g: &GbaGameRom, index: usize) -> Result<Trainer, RomError> {
    let size = g.layout.trainer_entry_size;
    let at = g.layout.trainers + index * size;
    let rom = g.rom();
    let entry = rom.bytes(at, size).ok_or_else(|| bad("dresseurs"))?;
    let party_kind = entry[0];
    let count = entry[size - 8] as usize;
    let party_at = rom.pointer(at + size - 4).unwrap_or(0);
    let mut party = Vec::new();
    if party_at != 0 && count <= 6 && party_kind <= 3 {
        let step = Trainer::mon_size(party_kind);
        for k in 0..count {
            let p = party_at + k * step;
            let w = |o: usize| rom.u16(p + o).unwrap_or(0);
            let (item, moves_at) = match party_kind {
                2 => (w(6), None),
                1 => (0, Some(6)),
                3 => (w(6), Some(8)),
                _ => (0, None),
            };
            let moves = moves_at.map_or([0; 4], |m| [w(m), w(m + 2), w(m + 4), w(m + 6)]);
            party.push(TrainerMon { iv: w(0), level: w(2), species: g.national(w(4)), item, moves });
        }
    }
    Ok(Trainer { index, party_kind, class: entry[1], name: text::decode(&entry[4..16]), double: entry[size - 16] == 1, party_at, party })
}

/// Tous les dresseurs de la table (l'entrée 0 est vide dans les jeux).
pub fn trainers(g: &GbaGameRom) -> Result<Vec<Trainer>, RomError> {
    (1..g.layout.trainer_count).map(|i| trainer(g, i)).collect()
}

/// Réécrit l'équipe (même nombre de Pokémon et même format : rien n'est déplacé).
pub fn set_party(g: &mut GbaGameRom, t: &Trainer) -> Result<(), RomError> {
    if t.party_at == 0 {
        return Ok(());
    }
    let step = Trainer::mon_size(t.party_kind);
    for (k, m) in t.party.iter().enumerate() {
        let p = t.party_at + k * step;
        let species = g.internal(m.species);
        let rom = g.rom_mut();
        rom.write_u16(p, m.iv)?;
        rom.write_u16(p + 2, m.level)?;
        rom.write_u16(p + 4, species)?;
        match t.party_kind {
            2 => rom.write_u16(p + 6, m.item)?,
            1 | 3 => {
                let start = if t.party_kind == 3 {
                    rom.write_u16(p + 6, m.item)?;
                    8
                } else {
                    6
                };
                for (j, mv) in m.moves.iter().enumerate() {
                    rom.write_u16(p + start + j * 2, *mv)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Nom de la classe de dresseur (« CHAMPION », « MEILLEURE DRESSEUSE »…).
pub fn trainer_class_name(g: &GbaGameRom, class: u8) -> Option<String> {
    let base = g.layout.trainer_class_names?;
    let len = g.layout.trainer_class_name_length;
    if g.layout.trainer_class_count > 0 && class as usize >= g.layout.trainer_class_count {
        return None;
    }
    g.rom().bytes(base + class as usize * len, len).map(text::decode)
}

// ---------------------------------------------------------------------------
// Rencontres fixes

/// Espèce (n° national) et niveaux de chaque rencontre fixe du fichier d'offsets.
pub fn statics(g: &GbaGameRom) -> Vec<(u16, Vec<u8>)> {
    g.entry
        .statics
        .iter()
        .map(|s| {
            let species = s.species.first().and_then(|&at| g.rom().u16(at)).map_or(0, |v| g.national(v));
            let levels = s.levels.iter().filter_map(|&at| g.rom().u8(at)).collect();
            (species, levels)
        })
        .collect()
}

/// Remplace l'espèce (et les niveaux) de la rencontre fixe `index`.
pub fn set_static(g: &mut GbaGameRom, index: usize, species: u16, level: Option<u8>) -> Result<(), RomError> {
    let def = g.entry.statics.get(index).ok_or_else(|| bad("rencontre fixe"))?.clone();
    let internal = g.internal(species);
    for at in def.species {
        g.rom_mut().write_u16(at, internal)?;
    }
    if let Some(level) = level {
        for at in def.levels {
            g.rom_mut().write_u8(at, level)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CT / CS

pub fn tm_moves(g: &GbaGameRom) -> Result<Vec<u16>, RomError> {
    (0..TM_COUNT).map(|k| g.rom().u16(g.layout.tm_moves + k * 2).ok_or_else(|| bad("CT"))).collect()
}

pub fn set_tm_moves(g: &mut GbaGameRom, moves: &[u16]) -> Result<(), RomError> {
    let (base, dup) = (g.layout.tm_moves, g.layout.tm_moves_duplicate);
    for (k, &m) in moves.iter().take(TM_COUNT).enumerate() {
        g.rom_mut().write_u16(base + k * 2, m)?;
        if let Some(dup) = dup {
            g.rom_mut().write_u16(dup + k * 2, m)?;
        }
    }
    Ok(())
}

/// Compatibilité CT puis CS (50 + 8 booléens) d'une espèce.
pub fn tmhm_compat(g: &GbaGameRom, national: u16) -> Result<Vec<bool>, RomError> {
    let i = g.internal(national) as usize;
    let raw = g.rom().bytes(g.layout.tmhm_compat + i * 8, 8).ok_or_else(|| bad("compatibilité CT"))?;
    Ok((0..TM_COUNT + HM_COUNT).map(|b| raw[b / 8] & (1 << (b % 8)) != 0).collect())
}

pub fn set_tmhm_compat(g: &mut GbaGameRom, national: u16, bits: &[bool]) -> Result<(), RomError> {
    let i = g.internal(national) as usize;
    let mut raw = [0u8; 8];
    for (b, &on) in bits.iter().take(TM_COUNT + HM_COUNT).enumerate() {
        if on {
            raw[b / 8] |= 1 << (b % 8);
        }
    }
    let at = g.layout.tmhm_compat + i * 8;
    g.rom_mut().write(at, &raw)?;
    Ok(())
}

/// Vérifie qu'une zone de la ROM est libre (utile aux tests de relecture).
pub fn is_free(g: &GbaGameRom, at: usize, len: usize) -> bool {
    g.rom().bytes(at, len).is_some_and(|b| b.iter().all(|&x| x == FREE_BYTE))
}

/// Nombre d'espèces de la Gen 3.
pub const fn species_count() -> u16 {
    SPECIES_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gba_rom::synthetic;

    fn open(code: &str) -> GbaGameRom {
        GbaGameRom::from_rom(synthetic::build(code, 0)).unwrap()
    }

    #[test]
    fn learnsets_round_trip_and_relocate() {
        for code in ["BPEF", "BPRF"] {
            let mut g = open(code);
            let before = g.rom().data().to_vec();
            let ls = learnset(&g, 25).unwrap();
            assert_eq!(crate::data::learnsets::read(4, &ls), vec![(33, 1), (45, 5)]);
            // Relecture sans changement : ROM identique.
            set_learnset(&mut g, 25, &ls).unwrap();
            assert_eq!(g.rom().data(), &before[..]);
            // Liste plus longue : déplacée dans la zone libre, les autres espèces intactes.
            let long: Vec<u8> = [(33u16, 1u16), (45, 5), (84, 9), (85, 20)].iter().flat_map(|&(m, l)| (m | l << 9).to_le_bytes()).collect();
            set_learnset(&mut g, 25, &long).unwrap();
            assert_eq!(crate::data::learnsets::read(4, &learnset(&g, 25).unwrap()), vec![(33, 1), (45, 5), (84, 9), (85, 20)]);
            assert!(g.rom().pointer(g.layout.movesets + 25 * 4).unwrap() >= g.layout.free_space);
            assert_eq!(learnset(&g, 26).unwrap(), ls);
        }
    }

    #[test]
    fn evolutions_starters_tms() {
        let mut g = open("BPRF");
        let evo = crate::data::evolutions::read(&evolutions(&g, 1).unwrap());
        assert_eq!(evo, vec![crate::data::evolutions::Evolution { method: 4, param: 16, target: 2 }]);
        let raw = evolutions(&g, 1).unwrap();
        set_evolutions(&mut g, 1, &raw).unwrap();
        assert_eq!(evolutions(&g, 1).unwrap(), raw);

        assert_eq!(starters(&g).unwrap(), [1, 4, 7]);
        set_starters(&mut g, [252, 255, 258]).unwrap();
        assert_eq!(starters(&g).unwrap(), [252, 255, 258]);
        // Rival : le starter qui bat celui du joueur.
        assert_eq!(g.national(g.rom().u16(g.layout.starters + FRLG_REPEAT).unwrap()), 255);

        let mut e = open("BPEF");
        assert_eq!(starters(&e).unwrap(), [252, 255, 258]);
        set_starters(&mut e, [1, 4, 7]).unwrap();
        assert_eq!(starters(&e).unwrap(), [1, 4, 7]);

        let tms = tm_moves(&g).unwrap();
        assert_eq!(tms[0], 1);
        let mut new = tms.clone();
        new[3] = 89;
        set_tm_moves(&mut g, &new).unwrap();
        assert_eq!(tm_moves(&g).unwrap()[3], 89);
        assert_eq!(g.rom().u16(g.layout.tm_moves_duplicate.unwrap() + 6), Some(89));

        let mut bits = vec![false; 58];
        bits[0] = true;
        bits[57] = true;
        set_tmhm_compat(&mut g, 386, &bits).unwrap();
        assert_eq!(tmhm_compat(&g, 386).unwrap(), bits);
        assert!(tmhm_compat(&g, 385).unwrap().iter().all(|b| !b));
    }

    #[test]
    fn wild_and_trainers() {
        let mut rom = synthetic::build("BPEF", 0);
        synthetic::add_grass(&mut rom, 0, 16, &[16, 16, 19, 19, 16, 19, 16, 19, 10, 10, 13, 13]);
        let mut g = GbaGameRom::from_rom(rom).unwrap();
        let areas = wild_areas(&g).unwrap();
        assert_eq!(areas.len(), 1);
        assert_eq!((areas[0].bank, areas[0].map, areas[0].kind), (0, 16, WildKind::Grass));
        assert_eq!(areas[0].slots[0], WildSlot { min_level: 2, max_level: 4, species: 16 });
        let mut area = areas[0].clone();
        area.slots[0].species = 300;
        set_wild_area(&mut g, &area).unwrap();
        assert_eq!(wild_areas(&g).unwrap()[0].slots[0].species, 300);

        let t = trainer(&g, 1).unwrap();
        assert_eq!(t.name, "TEST");
        assert_eq!(t.party.len(), 2);
        assert_eq!((t.party[0].level, t.party[0].species), (5, 16));
        let mut t2 = t.clone();
        t2.party[1].species = 384;
        t2.party[1].level = 70;
        set_party(&mut g, &t2).unwrap();
        let back = trainer(&g, 1).unwrap();
        assert_eq!((back.party[1].species, back.party[1].level), (384, 70));
        assert_eq!(back.party[0], t.party[0]);
        assert_eq!(trainers(&g).unwrap().len(), g.layout.trainer_count - 1);

        let st = statics(&g);
        assert!(!st.is_empty());
        set_static(&mut g, 0, 150, Some(70)).unwrap();
        assert_eq!(statics(&g)[0], (150, vec![70; g.entry.statics[0].levels.len()]));
    }
}
