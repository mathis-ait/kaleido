//! Formats PK1 et PK2 (Rouge, Bleu, Jaune, Or, Argent, Cristal), d'après PKHeX (`PK1.cs`,
//! `PK2.cs`, `GBPKM.cs`, `GBPKML.cs`, `PokeList1/2.cs`).
//!
//! Un Pokémon GB n'a ni PID ni chiffrement : 44 octets (PK1) ou 48 octets (PK2) en équipe
//! (33 / 32 en boîte), entiers gros-boutistes, valeurs génétiques (DV) sur 4 bits, « EV »
//! sur 16 bits (expérience de statistique). Surnom et nom du dresseur sont dans des tables
//! séparées de la liste (11 octets chacun). Le sexe, le chromatique et la forme de Zarbi
//! se déduisent des DV ; la nature affichée est celle de la Console virtuelle (`EXP % 25`).
//!
//! Fichiers `.pk1` / `.pk2` : liste d'un seul Pokémon, comme PKHeX : `[1][espèce][0xFF]`,
//! données d'équipe, nom du dresseur, surnom (69 octets en PK1, 73 en PK2).
//!
//! Comme pour le PK3 (`super::pk3`), le Pokémon est tenu en mémoire au format PK4 suivi de
//! la copie de ses octets d'origine : relu puis réécrit sans changement, il est identique.

use super::conv_tables::{ITEM2_TO_4, TABLE1_INTERNAL_TO_NATIONAL, TABLE1_NATIONAL_TO_INTERNAL};
use super::pkm::PkmFormat;
use super::strings;
use crate::text::gen12 as text;

pub const PK1_PARTY: usize = 44;
pub const PK2_PARTY: usize = 48;
pub const PK1_STORED: usize = 33;
pub const PK2_STORED: usize = 32;
/// Longueur d'un nom (dresseur ou surnom) dans les jeux occidentaux.
pub const NAME: usize = 11;
/// Fichier `.pk1` / `.pk2` (liste d'un Pokémon).
pub const PK1_FILE: usize = 3 + PK1_PARTY + 2 * NAME;
pub const PK2_FILE: usize = 3 + PK2_PARTY + 2 * NAME;
pub(crate) const PK4_PART: usize = 236;
pub(crate) const RAW_AT: usize = PK4_PART;
/// Tampon interne : PK4 + données d'équipe (48 au plus) + dresseur + surnom.
pub(crate) const BUFFER_SIZE: usize = PK4_PART + PK2_PARTY + 2 * NAME;
/// Objet Gen 2 sans équivalent Gen 4+ : `GEN2_ITEM_FLAG | id Gen 2`.
pub const GEN2_ITEM_FLAG: u16 = 0x4000;
/// Valeur « œuf » de la liste des espèces (Gen 2).
pub const EGG: u8 = 0xFD;

fn party_size(gen: u8) -> usize {
    if gen == 1 {
        PK1_PARTY
    } else {
        PK2_PARTY
    }
}

pub fn national_gen1(internal: u8) -> u16 {
    TABLE1_INTERNAL_TO_NATIONAL[internal as usize] as u16
}

pub fn internal_gen1(national: u16) -> u8 {
    TABLE1_NATIONAL_TO_INTERNAL.get(national as usize).copied().unwrap_or(0)
}

pub fn item2_exposed(item: u8) -> u16 {
    match (item, ITEM2_TO_4[item as usize]) {
        (0, _) => 0,
        (raw, 128) => GEN2_ITEM_FLAG | raw as u16,
        (_, v) => v,
    }
}

pub fn item2_raw(exposed: u16) -> u8 {
    if exposed & GEN2_ITEM_FLAG != 0 {
        return (exposed & 0xFF) as u8;
    }
    if exposed == 0 {
        return 0;
    }
    ITEM2_TO_4.iter().position(|&v| v == exposed).map_or(0, |i| i as u8)
}

fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([d[at], d[at + 1]])
}

fn put_be16(d: &mut [u8], at: usize, v: u16) {
    d[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn put_le32(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Offsets des champs communs dans les données d'équipe.
struct Ofs {
    moves: usize,
    tid: usize,
    exp: usize,
    evs: usize,
    dvs: usize,
    pp: usize,
    level: usize,
    stats: usize,
    hp: usize,
    status: usize,
}

const OFS1: Ofs = Ofs { moves: 0x08, tid: 0x0C, exp: 0x0E, evs: 0x11, dvs: 0x1B, pp: 0x1D, level: 0x21, stats: 0x22, hp: 0x01, status: 0x04 };
const OFS2: Ofs = Ofs { moves: 0x02, tid: 0x06, exp: 0x08, evs: 0x0B, dvs: 0x15, pp: 0x17, level: 0x1F, stats: 0x24, hp: 0x22, status: 0x20 };

fn ofs(gen: u8) -> &'static Ofs {
    if gen == 1 {
        &OFS1
    } else {
        &OFS2
    }
}

/// DV (Att, Déf, Vit, Spé) → IV au format Kaleido (PV, Att, Déf, Atq Spé, Déf Spé, Vit).
pub fn dvs_to_ivs(dv: u16) -> [u8; 6] {
    let (a, d, s, c) = (((dv >> 12) & 0xF) as u8, ((dv >> 8) & 0xF) as u8, ((dv >> 4) & 0xF) as u8, (dv & 0xF) as u8);
    let hp = ((a & 1) << 3) | ((d & 1) << 2) | ((s & 1) << 1) | (c & 1);
    [hp, a, d, c, c, s]
}

pub fn ivs_to_dvs(iv: [u8; 6]) -> u16 {
    let v = |x: u8| x.min(15) as u16;
    v(iv[1]) << 12 | v(iv[2]) << 8 | v(iv[5]) << 4 | v(iv[3])
}

/// Chromatique (Gen 2, `ShinyUtil.GetIsShinyGB`) : Déf, Vit et Spé à 10, Att en {2, 3, 6, 7, 10, 11, 14, 15}.
pub fn is_shiny(dv: u16) -> bool {
    let a = (dv >> 12) & 0xF;
    (dv & 0x0FFF) == 0x0AAA && a & 2 != 0
}

/// Sexe d'après le DV d'Attaque et le taux de femelles (0 mâle, 1 femelle, 2 asexué).
pub fn gender_bits(species: u16, dv: u16) -> u8 {
    let ratio = crate::dex::personal(crate::dex::Game::C, species, 0).map_or(255, |p| p.gender_ratio);
    match ratio {
        255 => 2,
        254 => 1,
        0 => 0,
        r => (((dv >> 12) & 0xF) as u8 <= r >> 4) as u8,
    }
}

/// Lettre de Zarbi d'après les DV (`GetUnownFormValue`).
pub fn unown_form(dv: u16) -> u8 {
    let (a, d, s, c) = ((dv >> 12) & 0xF, (dv >> 8) & 0xF, (dv >> 4) & 0xF, dv & 0xF);
    let v = ((a & 6) << 5) | ((d & 6) << 3) | ((s & 6) << 1) | ((c & 6) >> 1);
    (v / 10) as u8
}

fn gen4_text(raw: &[u8], max: usize) -> Vec<u8> {
    let mut out = vec![0u8; 2 * (max + 1)];
    if let Ok(b) = strings::encode(PkmFormat::Gen4, &text::decode(raw), max) {
        out[..b.len()].copy_from_slice(&b);
    }
    out
}

fn is_nicknamed(species: u16, name: &str) -> bool {
    let upper = name.to_uppercase();
    [crate::dex::Lang::Fr, crate::dex::Lang::En].iter().filter_map(|&l| crate::dex::species_name_lang(species, l)).all(|n| n.to_uppercase() != upper)
}

/// Langue probable : celle dont le nom d'espèce correspond au surnom (français par défaut).
fn guess_language(species: u16, name: &str) -> u8 {
    let upper = name.to_uppercase();
    if crate::dex::species_name_lang(species, crate::dex::Lang::En).is_some_and(|n| n.to_uppercase() == upper)
        && crate::dex::species_name_lang(species, crate::dex::Lang::Fr).is_none_or(|n| n.to_uppercase() != upper)
    {
        2
    } else {
        3
    }
}

/// Entrée GB (données d'équipe, dresseur, surnom) → tampon interne. `egg` : marqué œuf
/// dans la liste (Gen 2) ; `version` : jeu d'origine supposé (35 Rouge, 39 Or, 41 Cristal).
pub(crate) fn to_internal(gen: u8, data: &[u8], ot: &[u8], nick: &[u8], egg: bool, version: u8) -> Vec<u8> {
    let o = ofs(gen);
    let size = party_size(gen);
    let mut raw = vec![0u8; PK2_PARTY + 2 * NAME];
    raw[..data.len().min(size)].copy_from_slice(&data[..data.len().min(size)]);
    raw[PK2_PARTY..PK2_PARTY + ot.len().min(NAME)].copy_from_slice(&ot[..ot.len().min(NAME)]);
    raw[PK2_PARTY + NAME..PK2_PARTY + NAME + nick.len().min(NAME)].copy_from_slice(&nick[..nick.len().min(NAME)]);
    let r = &raw;

    let mut d = vec![0u8; BUFFER_SIZE];
    let species = if gen == 1 { national_gen1(r[0]) } else { r[0] as u16 };
    let dv = be16(r, o.dvs);
    let exp = (r[o.exp] as u32) << 16 | (r[o.exp + 1] as u32) << 8 | r[o.exp + 2] as u32;
    d[0x08..0x0A].copy_from_slice(&species.to_le_bytes());
    if gen == 2 {
        d[0x0A..0x0C].copy_from_slice(&item2_exposed(r[1]).to_le_bytes());
    }
    d[0x0C..0x0E].copy_from_slice(&be16(r, o.tid).to_le_bytes());
    put_le32(&mut d, 0x10, exp);
    d[0x14] = if gen == 2 { r[0x1B] } else { 70 };
    let nickname = text::decode(&r[PK2_PARTY + NAME..PK2_PARTY + 2 * NAME]);
    d[0x17] = guess_language(species, &nickname);
    // Expérience de statistique (0 à 65 535) ramenée à 0-255 ; Spécial = Atq Spé et Déf Spé.
    let ev = |k: usize| (be16(r, o.evs + 2 * k) >> 8) as u8;
    d[0x18..0x1E].copy_from_slice(&[ev(0), ev(1), ev(2), ev(3), ev(4), ev(4)]);
    for i in 0..4 {
        d[0x28 + 2 * i..0x2A + 2 * i].copy_from_slice(&(r[o.moves + i] as u16).to_le_bytes());
        d[0x30 + i] = r[o.pp + i] & 0x3F;
        d[0x34 + i] = r[o.pp + i] >> 6;
    }
    let iv = dvs_to_ivs(dv);
    // IV dans l'ordre du jeu : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
    let game_order = [iv[0], iv[1], iv[2], iv[5], iv[3], iv[4]];
    let packed = game_order.iter().enumerate().fold(0u32, |acc, (i, &v)| acc | (v as u32) << (5 * i));
    put_le32(&mut d, 0x38, packed | (egg as u32) << 30 | (is_nicknamed(species, &nickname) as u32 & !egg as u32) << 31);
    let form = if species == 201 { unown_form(dv) } else { 0 };
    d[0x40] = gender_bits(species, dv) << 1 | form << 3;
    d[0x48..0x48 + 22].copy_from_slice(&gen4_text(&r[PK2_PARTY + NAME..PK2_PARTY + 2 * NAME], 10));
    d[0x5F] = version;
    d[0x68..0x68 + 16].copy_from_slice(&gen4_text(&r[PK2_PARTY..PK2_PARTY + NAME], 7));
    d[0x83] = 4;
    if gen == 2 {
        d[0x82] = r[0x1C];
        // Données de capture (Cristal) : heure et niveau, puis sexe du dresseur et lieu.
        let caught = be16(r, 0x1D);
        let met_level = ((caught >> 8) & 0x3F) as u8;
        let location = (caught & 0x7F) as u16;
        let ot_female = caught & 0x80 != 0;
        d[0x80..0x82].copy_from_slice(&location.to_le_bytes());
        d[0x84] = met_level | (ot_female as u8) << 7;
    }
    // Section équipe.
    d[0x88] = r[o.status];
    d[0x8C] = r[o.level];
    d[0x8E..0x90].copy_from_slice(&be16(r, o.hp).to_le_bytes());
    let stat = |k: usize| be16(r, o.stats + 2 * k);
    let stats: [u16; 6] = if gen == 1 { [stat(0), stat(1), stat(2), stat(3), stat(4), stat(4)] } else { [stat(0), stat(1), stat(2), stat(3), stat(4), stat(5)] };
    for (k, s) in stats.iter().enumerate() {
        d[0x90 + 2 * k..0x92 + 2 * k].copy_from_slice(&s.to_le_bytes());
    }
    d[RAW_AT..].copy_from_slice(&raw);
    d
}

/// Tampon interne → (données d'équipe, dresseur, surnom).
pub(crate) fn from_internal(gen: u8, d: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let o = ofs(gen);
    let size = party_size(gen);
    let mut raw = d[RAW_AT..RAW_AT + PK2_PARTY + 2 * NAME].to_vec();
    let species = u16::from_le_bytes([d[0x08], d[0x09]]);
    raw[0] = if gen == 1 { internal_gen1(species) } else { species as u8 };
    if gen == 1 {
        // Types et taux de capture de la fiche (le jeu les relit à l'envoi au combat).
        if let Some(p) = crate::dex::personal(crate::dex::Game::RB, species, 0) {
            if p.types != crate::dex::personal(crate::dex::Game::RB, national_gen1(d[RAW_AT]), 0).map_or([0, 0], |q| q.types) {
                raw[5] = crate::pokemon::gen4_type_to_gb(p.types[0]).unwrap_or(0);
                raw[6] = crate::pokemon::gen4_type_to_gb(p.types[1]).unwrap_or(0);
            }
        }
    } else {
        let item = u16::from_le_bytes([d[0x0A], d[0x0B]]);
        if item2_exposed(raw[1]) != item {
            raw[1] = item2_raw(item);
        }
        raw[0x1B] = d[0x14];
        raw[0x1C] = d[0x82];
        let location = u16::from_le_bytes([d[0x80], d[0x81]]).min(0x7F);
        let old = be16(&raw, 0x1D);
        let caught = (old & 0xC000) | ((d[0x84] & 0x3F) as u16) << 8 | ((d[0x84] >> 7) as u16) << 7 | location;
        put_be16(&mut raw, 0x1D, caught);
    }
    put_be16(&mut raw, o.tid, u16::from_le_bytes([d[0x0C], d[0x0D]]));
    let exp = le32(d, 0x10).min(0xFF_FFFF);
    raw[o.exp..o.exp + 3].copy_from_slice(&exp.to_be_bytes()[1..]);
    // EV : réécrits seulement s'ils ont changé (garde la valeur exacte sur 16 bits).
    for k in 0..5 {
        let new = d[0x18 + k];
        if (be16(&raw, o.evs + 2 * k) >> 8) as u8 != new {
            put_be16(&mut raw, o.evs + 2 * k, (new as u16) << 8 | if new == 255 { 0xFF } else { 0 });
        }
    }
    for i in 0..4 {
        raw[o.moves + i] = u16::from_le_bytes([d[0x28 + 2 * i], d[0x29 + 2 * i]]).min(255) as u8;
        raw[o.pp + i] = (d[0x30 + i] & 0x3F) | (d[0x34 + i] & 3) << 6;
    }
    let packed = le32(d, 0x38);
    let game: [u8; 6] = std::array::from_fn(|i| ((packed >> (5 * i)) & 31) as u8);
    // Ordre Kaleido (PV, Att, Déf, Atq Spé, Déf Spé, Vit) depuis l'ordre du jeu.
    put_be16(&mut raw, o.dvs, ivs_to_dvs([game[0], game[1], game[2], game[4], game[5], game[3]]));
    raw[o.status] = d[0x88];
    // Niveau « boîte » (PK1, octet 3) : suivi seulement si le niveau change (garde l'octet d'origine).
    if gen == 1 && raw[o.level] != d[0x8C] {
        raw[0x03] = d[0x8C];
    }
    raw[o.level] = d[0x8C];
    put_be16(&mut raw, o.hp, u16::from_le_bytes([d[0x8E], d[0x8F]]));
    let stat = |k: usize| u16::from_le_bytes([d[0x90 + 2 * k], d[0x91 + 2 * k]]);
    let n = if gen == 1 { 5 } else { 6 };
    for k in 0..n {
        put_be16(&mut raw, o.stats + 2 * k, stat(k));
    }
    // Textes : réécrits seulement s'ils ont changé.
    let ot_at = PK2_PARTY;
    let nick_at = PK2_PARTY + NAME;
    if gen4_text(&raw[ot_at..ot_at + NAME], 7) != d[0x68..0x68 + 16] {
        let ot = strings::decode(PkmFormat::Gen4, &d[0x68..0x68 + 16]);
        raw[ot_at..ot_at + NAME].copy_from_slice(&text::encode(&ot, NAME, text::TERMINATOR));
    }
    if gen4_text(&raw[nick_at..nick_at + NAME], 10) != d[0x48..0x48 + 22] {
        let nick = strings::decode(PkmFormat::Gen4, &d[0x48..0x48 + 22]);
        raw[nick_at..nick_at + NAME].copy_from_slice(&text::encode(&nick.to_uppercase(), NAME, text::TERMINATOR));
    }
    (raw[..size].to_vec(), raw[ot_at..ot_at + NAME].to_vec(), raw[nick_at..nick_at + NAME].to_vec())
}

/// Fichier `.pk1` / `.pk2` → (données, dresseur, surnom, œuf).
pub fn parse_file(gen: u8, bytes: &[u8]) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>, bool)> {
    let size = party_size(gen);
    if bytes.len() < 3 + size + 2 * NAME || bytes[0] != 1 || bytes[2] != 0xFF {
        return None;
    }
    let data = bytes[3..3 + size].to_vec();
    let ot = bytes[3 + size..3 + size + NAME].to_vec();
    let nick = bytes[3 + size + NAME..3 + size + 2 * NAME].to_vec();
    Some((data, ot, nick, gen == 2 && bytes[1] == EGG))
}

/// (données, dresseur, surnom) → fichier `.pk1` / `.pk2`.
pub fn to_file(gen: u8, data: &[u8], ot: &[u8], nick: &[u8], egg: bool) -> Vec<u8> {
    let mut out = vec![1, if egg { EGG } else { data[0] }, 0xFF];
    out.extend_from_slice(data);
    out.extend_from_slice(ot);
    out.extend_from_slice(nick);
    let _ = gen;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dv_helpers() {
        // DV 10 / 10 / 10 / 10, Attaque 10 : chromatique.
        assert!(is_shiny(0xAAAA));
        assert!(!is_shiny(0x9AAA));
        assert_eq!(dvs_to_ivs(0xFFFF), [15; 6]);
        assert_eq!(ivs_to_dvs([0, 1, 2, 3, 3, 4]), 0x1243);
        assert_eq!(national_gen1(0x99), 1); // Bulbizarre
        assert_eq!(internal_gen1(1), 0x99);
        assert_eq!(item2_exposed(item2_raw(17)), 17);
    }
}
