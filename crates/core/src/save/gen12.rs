//! Sauvegardes Gen 1 et 2 (32 Kio) : Rouge / Bleu / Jaune, Or / Argent, Cristal (`SAV1.cs`,
//! `SAV2.cs`, `SAV1Offsets.cs`, `SAV2Offsets.cs`, `PokeList1/2.cs`, `SaveUtil.IsG1/IsG2` de
//! PKHeX ; versions occidentales seulement).
//!
//! Les Pokémon sont rangés en **listes** : `[nombre][espèces… 0xFF][données…][dresseurs…]
//! [surnoms…]`. L'équipe en compte 6 (44 / 48 octets par Pokémon), une boîte 20 (33 / 32
//! octets). Les boîtes sont dans les banques 2 et 3 de la cartouche (0x4000 et 0x6000) ;
//! la boîte active a en plus une copie dans la banque 1, que le jeu relit en priorité
//! (Gen 1) : Kaleido écrit les deux.
//!
//! Sommes de contrôle : Gen 1, un octet `~somme(0x2598..0x3523)` en 0x3523 ; Gen 2, la somme
//! (u16) de 0x2009 à la fin du bloc principal, écrite à deux endroits (copie de secours).
//! **Testé uniquement sur des sauvegardes synthétiques** (aucune vraie sauvegarde).

use super::pk12::{self, EGG, NAME};
use super::{rd_u16, BlockCheck, Checks, Layout, PkmFormat, Pokemon, SaveError, SaveVersion, TrainerLayout};
use crate::text::gen12 as text;

pub const SIZE: usize = 0x8000;
const BOX_SLOTS: usize = 20;

#[derive(Debug, Clone)]
pub(super) struct Gen12Checks {
    pub(super) gen: u8,
    /// Somme : début, fin (exclue), emplacement(s).
    sum_start: usize,
    sum_end: usize,
    sum_at: usize,
    sum_at2: Option<usize>,
    pub(super) current_box_index: usize,
    pub(super) current_box: usize,
    pub(super) box_names: Option<usize>,
    pub(super) badges: usize,
}

fn list_ok(d: &[u8], at: usize, max: u8) -> bool {
    d.get(at).is_some_and(|&n| n <= max && d.get(at + 1 + n as usize) == Some(&0xFF))
}

/// Jeu d'une sauvegarde Gen 1 / 2 (32 Kio, avec ou sans le pied d'horloge de mGBA).
pub fn identify(data: &[u8]) -> Option<SaveVersion> {
    if data.len() < SIZE || data.len() > SIZE + 0x40 {
        return None;
    }
    let d = &data[..SIZE];
    if list_ok(d, 0x2F2C, 6) && list_ok(d, 0x30C0, 20) {
        // `SAV1.IsYellowINT` : starter Pikachu (0x54) ou amitié de Pikachu.
        let yellow = if d[0x29C3] != 0 { d[0x29C3] == 0x54 } else { d[0x271C] != 0 };
        return Some(if yellow { SaveVersion::Yellow } else { SaveVersion::RedBlue });
    }
    if list_ok(d, 0x288A, 6) && list_ok(d, 0x2D6C, 20) {
        return Some(SaveVersion::GoldSilver);
    }
    if list_ok(d, 0x2865, 6) && list_ok(d, 0x2D10, 20) {
        return Some(SaveVersion::Crystal);
    }
    None
}

fn gen_of(version: SaveVersion) -> u8 {
    if matches!(version, SaveVersion::RedBlue | SaveVersion::Yellow) {
        1
    } else {
        2
    }
}

fn stored_size(gen: u8) -> usize {
    if gen == 1 {
        pk12::PK1_STORED
    } else {
        pk12::PK2_STORED
    }
}

fn party_size(gen: u8) -> usize {
    if gen == 1 {
        pk12::PK1_PARTY
    } else {
        pk12::PK2_PARTY
    }
}

fn list_len(gen: u8, capacity: usize, party: bool) -> usize {
    let size = if party { party_size(gen) } else { stored_size(gen) };
    (2 * NAME + size + 1) * capacity + 2
}

pub(super) fn layout(version: SaveVersion, _data: &[u8]) -> Result<(Layout, Vec<String>), SaveError> {
    let gen = gen_of(version);
    let (party, current_box, current_box_index, money, badges, play_time, dex_caught, box_names, sum) = match version {
        SaveVersion::GoldSilver => (0x288A, 0x2D6C, 0x2724, 0x23DB, 0x23E4, 0x2053, 0x2A4C, Some(0x2727), (0x2009, 0x2D69, 0x2D69, Some(0x7E6D))),
        SaveVersion::Crystal => (0x2865, 0x2D10, 0x2700, 0x23DC, 0x23E5, 0x2052, 0x2A27, Some(0x2703), (0x2009, 0x2B83, 0x2D0D, Some(0x1F0D))),
        _ => (0x2F2C, 0x30C0, 0x284C, 0x25F3, 0x2602, 0x2CED, 0x25A3, None, (0x2598, 0x3523, 0x3523, None)),
    };
    let (ot, tid) = if gen == 1 { (0x2598, 0x2605) } else { (0x200B, 0x2009) };
    let layout = Layout {
        format: if gen == 1 { PkmFormat::Gen1 } else { PkmFormat::Gen2 },
        trainer: TrainerLayout {
            name: ot,
            name_max: 7,
            tid,
            sid: tid,
            gender: if version == SaveVersion::Crystal { 0x3E3D } else { tid + 2 },
            // Pas de langue enregistrée : un octet hors du bloc vérifié, jamais lu par le jeu.
            language: SIZE - 1,
            money,
            hours: play_time,
            minutes: play_time + 2,
            seconds: play_time + 3,
        },
        party,
        party_count: party,
        boxes: 0x4000,
        box_stride: list_len(gen, BOX_SLOTS, false),
        box_count: if gen == 1 { 12 } else { 14 },
        box_names: box_names.unwrap_or(0),
        box_name_stride: 9,
        box_name_max: 8,
        items: 0,
        dex: dex_caught,
        map: 0,
        checks: Checks::Gen12(Gen12Checks {
            gen,
            sum_start: sum.0,
            sum_end: sum.1,
            sum_at: sum.2,
            sum_at2: sum.3,
            current_box_index,
            current_box,
            box_names,
            badges,
        }),
    };
    Ok((layout, Vec::new()))
}

fn checksum(c: &Gen12Checks, d: &[u8]) -> u16 {
    let sum = d[c.sum_start..c.sum_end].iter().fold(0u16, |s, &b| s.wrapping_add(b as u16));
    if c.gen == 1 {
        (!(sum as u8)) as u16
    } else {
        sum
    }
}

pub(super) fn fix(d: &mut [u8], c: &Gen12Checks) {
    let sum = checksum(c, d);
    if c.gen == 1 {
        d[c.sum_at] = sum as u8;
        // Banques 2 et 3 (`SAV1.SetChecksums`) : somme des 6 boîtes, puis une par boîte.
        let inv = |s: &[u8]| !s.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        let len = list_len(1, BOX_SLOTS, false);
        for bank in [0x4000, 0x6000] {
            let all = bank + 6 * len;
            d[all] = inv(&d[bank..all]);
            for i in 0..6 {
                d[all + 1 + i] = inv(&d[bank + i * len..bank + (i + 1) * len]);
            }
        }
    } else {
        d[c.sum_at..c.sum_at + 2].copy_from_slice(&sum.to_le_bytes());
        if let Some(at) = c.sum_at2 {
            d[at..at + 2].copy_from_slice(&sum.to_le_bytes());
        }
    }
}

pub(super) fn verify(d: &[u8], c: &Gen12Checks) -> Vec<BlockCheck> {
    let computed = checksum(c, d);
    let stored = if c.gen == 1 { d[c.sum_at] as u16 } else { rd_u16(d, c.sum_at) };
    vec![BlockCheck { name: "Bloc principal".into(), offset: c.sum_start, length: c.sum_end - c.sum_start, stored, computed, valid: stored == computed }]
}

/// Emplacement de la liste d'une boîte dans les banques 2 et 3.
pub(super) fn box_offset(c: &Gen12Checks, index: usize) -> usize {
    let len = list_len(c.gen, BOX_SLOTS, false);
    let (split, gap) = if c.gen == 1 { (6, 0) } else { (7, 2) };
    if index < split {
        0x4000 + index * (len + gap)
    } else {
        0x6000 + (index - split) * (len + gap)
    }
}

/// Boîte active (copie lue par le jeu en Gen 1) et drapeau « boîtes initialisées ».
fn current_box(c: &Gen12Checks, d: &[u8]) -> (usize, bool) {
    let v = d[c.current_box_index];
    ((v & 0x7F) as usize, c.gen == 2 || v & 0x80 != 0)
}

/// Entrées d'une liste : (données, dresseur, surnom, œuf).
pub(super) fn read_list(gen: u8, d: &[u8], at: usize, capacity: usize, party: bool) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>, bool)> {
    let count = (d[at] as usize).min(capacity);
    let size = if party { party_size(gen) } else { stored_size(gen) };
    let data_at = at + 1 + capacity + 1;
    let ot_at = data_at + capacity * size;
    let nick_at = ot_at + capacity * NAME;
    (0..count)
        .map(|i| {
            let mut data = d[data_at + i * size..data_at + (i + 1) * size].to_vec();
            data.resize(party_size(gen), 0);
            let ot = d[ot_at + i * NAME..ot_at + (i + 1) * NAME].to_vec();
            let nick = d[nick_at + i * NAME..nick_at + (i + 1) * NAME].to_vec();
            (data, ot, nick, gen == 2 && d[at + 1 + i] == EGG)
        })
        .collect()
}

pub(super) fn write_list(gen: u8, d: &mut [u8], at: usize, capacity: usize, party: bool, entries: &[(Vec<u8>, Vec<u8>, Vec<u8>, bool)]) {
    let size = if party { party_size(gen) } else { stored_size(gen) };
    let data_at = at + 1 + capacity + 1;
    let ot_at = data_at + capacity * size;
    let nick_at = ot_at + capacity * NAME;
    d[at..at + list_len(gen, capacity, party)].fill(0);
    d[at] = entries.len() as u8;
    for (i, (data, ot, nick, egg)) in entries.iter().enumerate() {
        d[at + 1 + i] = if *egg { EGG } else { data[0] };
        d[data_at + i * size..data_at + (i + 1) * size].copy_from_slice(&data[..size]);
        d[ot_at + i * NAME..ot_at + (i + 1) * NAME].copy_from_slice(&ot[..NAME]);
        d[nick_at + i * NAME..nick_at + (i + 1) * NAME].copy_from_slice(&nick[..NAME]);
    }
    d[at + 1 + entries.len()] = 0xFF;
    // Les emplacements libres des noms sont remplis de terminateurs, comme le jeu.
    for i in entries.len()..capacity {
        d[ot_at + i * NAME..ot_at + (i + 1) * NAME].fill(text::TERMINATOR);
        d[nick_at + i * NAME..nick_at + (i + 1) * NAME].fill(text::TERMINATOR);
    }
}

fn version_hint(gen: u8, data: &[u8]) -> u8 {
    if gen == 1 {
        35
    } else if data[0x1D] != 0 || data[0x1E] != 0 {
        41
    } else {
        39
    }
}

pub(super) fn to_pokemon(gen: u8, e: &(Vec<u8>, Vec<u8>, Vec<u8>, bool)) -> Pokemon {
    let format = if gen == 1 { PkmFormat::Gen1 } else { PkmFormat::Gen2 };
    let buf = pk12::to_internal(gen, &e.0, &e.1, &e.2, e.3, version_hint(gen, &e.0));
    Pokemon::from_decrypted(format, &buf).expect("taille du tampon GB")
}

pub(super) fn to_entry(p: &Pokemon) -> (Vec<u8>, Vec<u8>, Vec<u8>, bool) {
    let (mut data, ot, nick) = p.gb_parts();
    // Gen 1 : le jeu lit le niveau des Pokémon en boîte dans l'octet 3.
    if p.format() == PkmFormat::Gen1 && data.len() > 0x21 {
        data[3] = data[0x21];
    }
    (data, ot, nick, p.is_egg())
}

/// Liste de la boîte `index` : la copie active si c'est la boîte courante (Gen 1).
pub(super) fn box_entries(c: &Gen12Checks, d: &[u8], index: usize) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>, bool)> {
    let (current, initialized) = current_box(c, d);
    if index == current || (!initialized && c.gen == 1) {
        if index != current {
            return Vec::new();
        }
        return read_list(c.gen, d, c.current_box, BOX_SLOTS, false);
    }
    read_list(c.gen, d, box_offset(c, index), BOX_SLOTS, false)
}

pub(super) fn set_box_entries(c: &Gen12Checks, d: &mut [u8], index: usize, entries: &[(Vec<u8>, Vec<u8>, Vec<u8>, bool)]) {
    let (current, _) = current_box(c, d);
    write_list(c.gen, d, box_offset(c, index), BOX_SLOTS, false, entries);
    if index == current {
        write_list(c.gen, d, c.current_box, BOX_SLOTS, false, entries);
    }
    if c.gen == 1 {
        // Boîtes désormais écrites dans les banques : drapeau « initialisées ».
        d[c.current_box_index] |= 0x80;
    }
}

pub(super) fn party_entries(gen: u8, d: &[u8], at: usize) -> Vec<(Vec<u8>, Vec<u8>, Vec<u8>, bool)> {
    read_list(gen, d, at, 6, true)
}

pub(super) fn set_party_entries(gen: u8, d: &mut [u8], at: usize, entries: &[(Vec<u8>, Vec<u8>, Vec<u8>, bool)]) {
    write_list(gen, d, at, 6, true, entries);
}

/// Argent : Gen 1 en BCD sur 3 octets, Gen 2 en binaire gros-boutiste sur 3 octets.
pub(super) fn money(gen: u8, d: &[u8], at: usize) -> u32 {
    let b = &d[at..at + 3];
    if gen == 1 {
        b.iter().fold(0u32, |acc, &x| acc * 100 + ((x >> 4) as u32) * 10 + (x & 0xF) as u32)
    } else {
        (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32
    }
}

pub(super) fn set_money(gen: u8, d: &mut [u8], at: usize, value: u32) {
    let v = value.min(999_999);
    if gen == 1 {
        let digits = [v / 100_000 % 10, v / 10_000 % 10, v / 1000 % 10, v / 100 % 10, v / 10 % 10, v % 10];
        for k in 0..3 {
            d[at + k] = (digits[2 * k] << 4 | digits[2 * k + 1]) as u8;
        }
    } else {
        d[at..at + 3].copy_from_slice(&v.to_be_bytes()[1..]);
    }
}

/// Sauvegarde vide et valide (tests) : équipe et boîtes vides, dresseur « THISMA ».
pub fn blank(version: SaveVersion) -> Vec<u8> {
    let mut d = vec![0u8; SIZE];
    let (layout, _) = layout(version, &d).unwrap();
    let Checks::Gen12(c) = &layout.checks else { unreachable!() };
    let gen = c.gen;
    d[layout.trainer.name..layout.trainer.name + NAME].copy_from_slice(&text::encode("THISMA", NAME, text::TERMINATOR));
    write_list(gen, &mut d, layout.party, 6, true, &[]);
    write_list(gen, &mut d, c.current_box, BOX_SLOTS, false, &[]);
    for b in 0..layout.box_count {
        write_list(gen, &mut d, box_offset(c, b), BOX_SLOTS, false, &[]);
    }
    if version == SaveVersion::Yellow {
        d[0x29C3] = 0x54;
    }
    if let Some(names) = c.box_names {
        for b in 0..layout.box_count {
            d[names + b * 9..names + b * 9 + 9].copy_from_slice(&text::encode(&format!("BOITE{}", b + 1), 9, text::TERMINATOR));
        }
    }
    fix(&mut d, c);
    d
}
