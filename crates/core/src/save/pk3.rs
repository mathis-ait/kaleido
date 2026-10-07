//! Format PK3 (Rubis, Saphir, Émeraude, Rouge Feu, Vert Feuille), d'après PKHeX
//! (`PKM/PK3.cs`, `PokeCrypto.Decrypt3`, `SpeciesConverter`, `ItemConverter`).
//!
//! 80 octets en boîte, 100 dans l'équipe : en-tête de 32 octets (PID, ID du dresseur,
//! surnom sur 10 octets, langue, drapeaux, nom du dresseur sur 7 octets, marquages,
//! somme de contrôle), puis 4 sous-structures de 12 octets (Croissance, Attaques, EV et
//! concours, Divers) mélangées selon `PID % 24` et chiffrées par XOR avec `PID ^ OTID`.
//!
//! Dans Kaleido, un Pokémon Gen 3 est tenu en mémoire sous la forme d'un PK4 (mêmes
//! offsets que la Gen 4 : espèce en n° national, objet en identifiant Gen 4+, textes
//! encodés Gen 4) suivi d'une copie du PK3 d'origine déchiffré. Tous les accesseurs de
//! [`super::Pokemon`] fonctionnent donc tels quels ; seule la conversion à la lecture et
//! à l'écriture est propre à la Gen 3. La copie d'origine garde ce qui n'a pas
//! d'équivalent (rubans de concours en compteurs, obéissance, courrier, octets inutilisés)
//! pour qu'un Pokémon relu puis réécrit sans modification soit identique à l'octet près.

use super::conv_tables::{ITEM3_TO_4, TABLE3_INTERNAL_TO_NATIONAL, TABLE3_NATIONAL_TO_INTERNAL};
use super::pkm::{BLOCK_POSITION, PkmFormat};
use super::strings;
use crate::text::gen3 as text3;

pub const STORED_SIZE: usize = 80;
pub const PARTY_SIZE: usize = 100;
/// Taille de la partie « PK4 » du tampon interne.
pub(crate) const PK4_PART: usize = 236;
/// Début de la copie du PK3 d'origine dans le tampon interne.
pub(crate) const RAW_AT: usize = PK4_PART;
/// Taille du tampon interne d'un Pokémon Gen 3.
pub(crate) const BUFFER_SIZE: usize = PK4_PART + PARTY_SIZE;

const HEADER: usize = 32;
const BLOCK: usize = 12;
/// Objet sans équivalent (`ItemConverter.NaN`).
const ITEM_NAN: u16 = 128;

const FIRST_UNALIGNED_NATIONAL: u16 = 252;
const FIRST_UNALIGNED_INTERNAL: u16 = 277;

/// Espèce interne Gen 3 → n° national (0 si inconnue).
pub fn national_from_internal(raw: u16) -> u16 {
    if raw < FIRST_UNALIGNED_NATIONAL {
        return raw;
    }
    match TABLE3_INTERNAL_TO_NATIONAL.get((raw.wrapping_sub(FIRST_UNALIGNED_INTERNAL)) as usize) {
        Some(&d) if raw >= FIRST_UNALIGNED_INTERNAL => (raw as i32 + d as i32) as u16,
        _ => 0,
    }
}

/// N° national → espèce interne Gen 3.
pub fn internal_from_national(species: u16) -> u16 {
    match TABLE3_NATIONAL_TO_INTERNAL.get(species.wrapping_sub(FIRST_UNALIGNED_NATIONAL) as usize) {
        Some(&d) if species >= FIRST_UNALIGNED_NATIONAL => (species as i32 + d as i32) as u16,
        _ => species,
    }
}

/// Objet Gen 3 → identifiant Gen 4+ (0 si sans équivalent).
pub fn item_to_gen4(item: u16) -> u16 {
    match ITEM3_TO_4.get(item as usize) {
        Some(&ITEM_NAN) | None => 0,
        Some(&v) => v,
    }
}

/// Identifiant Gen 4+ → objet Gen 3 (0 si l'objet n'existe pas en Gen 3).
pub fn item_from_gen4(item: u16) -> u16 {
    if item == 0 || item == ITEM_NAN {
        return 0;
    }
    ITEM3_TO_4.iter().position(|&v| v == item).map_or(0, |i| i as u16)
}

/// Objet Gen 3 tel que Kaleido l'expose : identifiant Gen 4+ s'il existe, sinon
/// `GEN3_ITEM_FLAG | id Gen 3` (objets rares, objets disparus) pour ne rien perdre.
pub fn item_exposed(item: u16) -> u16 {
    match (item, item_to_gen4(item)) {
        (0, _) => 0,
        (raw, 0) => crate::dex::GEN3_ITEM_FLAG | raw,
        (_, v) => v,
    }
}

/// Inverse de [`item_exposed`].
pub fn item_raw(exposed: u16) -> u16 {
    if exposed & crate::dex::GEN3_ITEM_FLAG != 0 {
        exposed & !crate::dex::GEN3_ITEM_FLAG
    } else {
        item_from_gen4(exposed)
    }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn put_u16(d: &mut [u8], at: usize, v: u16) {
    d[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// Somme de contrôle : somme des u16 des 48 octets des sous-structures (déchiffrées).
pub fn checksum(raw: &[u8]) -> u16 {
    raw[HEADER..STORED_SIZE].chunks(2).fold(0u16, |acc, w| acc.wrapping_add(u16::from_le_bytes([w[0], w[1]])))
}

fn crypt(raw: &mut [u8]) {
    let key = u32_at(raw, 0) ^ u32_at(raw, 4);
    for w in raw[HEADER..STORED_SIZE].chunks_mut(4) {
        let v = u32::from_le_bytes([w[0], w[1], w[2], w[3]]) ^ key;
        w.copy_from_slice(&v.to_le_bytes());
    }
}

/// Déchiffre (80 ou 100 octets) : sous-structures remises dans l'ordre Croissance,
/// Attaques, EV, Divers. Le résultat fait toujours 100 octets.
pub fn decrypt(bytes: &[u8]) -> Vec<u8> {
    let mut raw = bytes.to_vec();
    raw.resize(PARTY_SIZE, 0);
    crypt(&mut raw);
    let sv = (u32_at(&raw, 0) % 24) as usize;
    let src = raw[HEADER..STORED_SIZE].to_vec();
    for (k, &p) in BLOCK_POSITION[sv].iter().enumerate() {
        let p = p as usize;
        raw[HEADER + k * BLOCK..HEADER + (k + 1) * BLOCK].copy_from_slice(&src[p * BLOCK..(p + 1) * BLOCK]);
    }
    raw
}

/// Chiffre un PK3 déchiffré (100 octets) ; la somme de contrôle doit déjà être à jour.
pub fn encrypt(raw: &[u8]) -> Vec<u8> {
    let mut out = raw.to_vec();
    out.resize(PARTY_SIZE, 0);
    let sv = (u32_at(&out, 0) % 24) as usize;
    let src = out[HEADER..STORED_SIZE].to_vec();
    for (k, &p) in BLOCK_POSITION[sv].iter().enumerate() {
        let p = p as usize;
        out[HEADER + p * BLOCK..HEADER + (p + 1) * BLOCK].copy_from_slice(&src[k * BLOCK..(k + 1) * BLOCK]);
    }
    crypt(&mut out);
    out
}

/// Données déjà déchiffrées ? (somme de contrôle cohérente et espèce plausible, ou emplacement vide).
pub fn looks_decrypted(bytes: &[u8]) -> bool {
    if bytes[..STORED_SIZE].iter().all(|&b| b == 0) {
        return true;
    }
    let species = u16_at(bytes, 0x20);
    checksum(bytes) == u16_at(bytes, 0x1C) && species <= 412
}

/// Échange des bits 1 et 2 (cercle, carré, triangle, cœur ↔ ordre Gen 4).
fn swap_marking_bits(v: u8) -> u8 {
    (v & !0b110) | ((v >> 1) & 0b10) | ((v << 1) & 0b100)
}

/// Lettre de Zarbi déduite du PID.
fn unown_form(pid: u32) -> u8 {
    let v = ((pid >> 24) & 3) << 6 | ((pid >> 16) & 3) << 4 | ((pid >> 8) & 3) << 2 | (pid & 3);
    (v % 28) as u8
}

/// Talents (1, 2) de l'espèce dans les jeux Gen 3.
fn abilities(species: u16) -> (u16, u16) {
    crate::dex::personal(crate::dex::Game::E, species, 0).map_or((0, 0), |p| (p.abilities[0], p.abilities[1]))
}

/// Sexe déduit du PID et du taux de femelles (0 = mâle, 1 = femelle, 2 = asexué).
fn gender_bits(species: u16, pid: u32) -> u8 {
    let ratio = crate::dex::personal(crate::dex::Game::E, species, 0).map_or(255, |p| p.gender_ratio);
    match ratio {
        255 => 2,
        254 => 1,
        0 => 0,
        r => ((pid & 0xFF) < r as u32) as u8,
    }
}

fn is_nicknamed(species: u16, name: &str, egg: bool) -> bool {
    if egg || species == 0 {
        return false;
    }
    let upper = name.to_uppercase();
    [crate::dex::Lang::Fr, crate::dex::Lang::En].iter().filter_map(|&l| crate::dex::species_name_lang(species, l)).all(|n| n.to_uppercase() != upper)
}

/// Rubans Gen 3 (compteurs de concours sur 3 bits, puis 12 rubans en bits 15 à 26) → rubans
/// de Hoenn du PK4 (octets 0x3C à 0x3F : 4 bits par concours, puis les 12 rubans), comme
/// `PK3.ConvertToPK4`.
fn ribbons_to_pk4(r: u32) -> u32 {
    let mut out = 0u32;
    for c in 0..5 {
        let count = ((r >> (3 * c)) & 7).min(4);
        out |= ((1u32 << count) - 1) << (4 * c);
    }
    out | ((r >> 15) & 0xFFF) << 20
}

fn ribbons_from_pk4(v: u32, old: u32) -> u32 {
    let mut out = old & !0x07FF_FFFF;
    for c in 0..5 {
        out |= ((v >> (4 * c)) & 0xF).count_ones() << (3 * c);
    }
    out | ((v >> 20) & 0xFFF) << 15
}

/// Texte Gen 3 tel que `to_internal` l'écrit dans le tampon (Gen 4, `2 * (max + 1)` octets).
fn gen4_text(raw: &[u8], max: usize) -> Vec<u8> {
    let mut out = vec![0u8; 2 * (max + 1)];
    if let Ok(b) = strings::encode(PkmFormat::Gen4, &text3::decode(raw), max) {
        out[..b.len()].copy_from_slice(&b);
    }
    out
}

/// PK3 déchiffré (100 octets) → tampon interne (PK4 + copie du PK3).
pub(crate) fn to_internal(raw: &[u8]) -> Vec<u8> {
    let mut raw = raw.to_vec();
    raw.resize(PARTY_SIZE, 0);
    let mut d = vec![0u8; BUFFER_SIZE];
    let pid = u32_at(&raw, 0);
    let species = national_from_internal(u16_at(&raw, 0x20));
    let iv32 = u32_at(&raw, 0x48);
    let egg = iv32 >> 30 & 1 != 0;
    let origins = u16_at(&raw, 0x46);
    let ribbons = u32_at(&raw, 0x4C);

    put_u32(&mut d, 0x00, pid);
    put_u16(&mut d, 0x08, species);
    put_u16(&mut d, 0x0A, item_exposed(u16_at(&raw, 0x22)));
    d[0x0C..0x10].copy_from_slice(&raw[0x04..0x08]);
    put_u32(&mut d, 0x10, u32_at(&raw, 0x24));
    d[0x14] = raw[0x29];
    let (a1, a2) = abilities(species);
    let bit = iv32 >> 31 != 0;
    d[0x15] = if bit && a2 != 0 { a2 } else { a1 } as u8;
    d[0x16] = swap_marking_bits(raw[0x1B]);
    d[0x17] = raw[0x12];
    d[0x18..0x24].copy_from_slice(&raw[0x38..0x44]);
    d[0x28..0x30].copy_from_slice(&raw[0x2C..0x34]);
    d[0x30..0x34].copy_from_slice(&raw[0x34..0x38]);
    for i in 0..4 {
        d[0x34 + i] = (raw[0x28] >> (2 * i)) & 3;
    }
    let nickname = text3::decode(&raw[0x08..0x12]);
    let nicknamed = is_nicknamed(species, &nickname, egg);
    put_u32(&mut d, 0x38, (iv32 & 0x3FFF_FFFF) | (egg as u32) << 30 | (nicknamed as u32) << 31);
    let form = if species == 201 { unown_form(pid) } else { 0 };
    d[0x40] = (ribbons >> 31) as u8 | gender_bits(species, pid) << 1 | form << 3;
    d[0x48..0x48 + 22].copy_from_slice(&gen4_text(&raw[0x08..0x12], 10));
    d[0x5F] = ((origins >> 7) & 0xF) as u8;
    d[0x68..0x68 + 16].copy_from_slice(&gen4_text(&raw[0x14..0x1B], 7));
    put_u32(&mut d, 0x3C, ribbons_to_pk4(ribbons));
    put_u16(&mut d, 0x80, raw[0x45] as u16);
    d[0x82] = raw[0x44];
    d[0x83] = ((origins >> 11) & 0xF) as u8;
    d[0x84] = (origins & 0x7F) as u8 | ((origins >> 15) as u8) << 7;
    // Section équipe.
    put_u32(&mut d, 0x88, u32_at(&raw, 0x50));
    d[0x8C] = raw[0x54];
    put_u16(&mut d, 0x8E, u16_at(&raw, 0x56));
    d[0x90..0x9C].copy_from_slice(&raw[0x58..0x64]);
    d[RAW_AT..].copy_from_slice(&raw);
    d
}

/// Tampon interne → PK3 déchiffré (100 octets), somme de contrôle à jour.
pub(crate) fn from_internal(d: &[u8]) -> Vec<u8> {
    let mut raw = d[RAW_AT..RAW_AT + PARTY_SIZE].to_vec();
    let pid = u32_at(d, 0);
    let species = u16_at(d, 0x08);
    let iv32 = u32_at(d, 0x38);
    let egg = iv32 >> 30 & 1 != 0;

    put_u32(&mut raw, 0x00, pid);
    raw[0x04..0x08].copy_from_slice(&d[0x0C..0x10]);
    // Textes : réécrits seulement s'ils ont changé (garde les octets de remplissage d'origine).
    let nickname = strings::decode(PkmFormat::Gen4, &d[0x48..0x48 + 22]);
    if gen4_text(&raw[0x08..0x12], 10) != d[0x48..0x48 + 22] {
        raw[0x08..0x12].copy_from_slice(&text3::encode(&nickname, 10, 0xFF));
    }
    raw[0x12] = d[0x17];
    let flags = raw[0x13] & !0b111;
    raw[0x13] = flags | ((species != 0) as u8) << 1 | (egg as u8) << 2;
    let ot = strings::decode(PkmFormat::Gen4, &d[0x68..0x68 + 16]);
    if gen4_text(&raw[0x14..0x1B], 7) != d[0x68..0x68 + 16] {
        raw[0x14..0x1B].copy_from_slice(&text3::encode(&ot, 7, 0xFF));
    }
    raw[0x1B] = swap_marking_bits(d[0x16]);
    // Croissance.
    put_u16(&mut raw, 0x20, if species == 0 { 0 } else { internal_from_national(species) });
    let item4 = u16_at(d, 0x0A);
    let old3 = u16_at(&raw, 0x22);
    if item_exposed(old3) != item4 {
        put_u16(&mut raw, 0x22, item_raw(item4));
    }
    put_u32(&mut raw, 0x24, u32_at(d, 0x10));
    raw[0x28] = (0..4).fold(0u8, |acc, i| acc | (d[0x34 + i] & 3) << (2 * i));
    raw[0x29] = d[0x14];
    // Attaques, EV et concours.
    raw[0x2C..0x34].copy_from_slice(&d[0x28..0x30]);
    raw[0x34..0x38].copy_from_slice(&d[0x30..0x34]);
    raw[0x38..0x44].copy_from_slice(&d[0x18..0x24]);
    // Divers.
    raw[0x44] = d[0x82];
    raw[0x45] = u16_at(d, 0x80).min(255) as u8;
    let origins = (d[0x84] & 0x7F) as u16 | ((d[0x5F] & 0xF) as u16) << 7 | ((d[0x83] & 0xF) as u16) << 11 | ((d[0x84] >> 7) as u16) << 15;
    put_u16(&mut raw, 0x46, origins);
    let ability = d[0x15] as u16;
    let (a1, a2) = abilities(species);
    let old_bit = u32_at(&raw, 0x48) >> 31;
    let bit = if a2 != 0 && a2 != a1 && ability == a2 {
        1
    } else if ability == a1 {
        0
    } else {
        old_bit
    };
    put_u32(&mut raw, 0x48, (iv32 & 0x3FFF_FFFF) | (egg as u32) << 30 | bit << 31);
    let old = u32_at(&raw, 0x4C);
    let ribbons = if ribbons_to_pk4(old) == u32_at(d, 0x3C) { old } else { ribbons_from_pk4(u32_at(d, 0x3C), old) };
    let ribbons = (ribbons & 0x7FFF_FFFF) | ((d[0x40] & 1) as u32) << 31;
    put_u32(&mut raw, 0x4C, ribbons);
    // Équipe.
    put_u32(&mut raw, 0x50, u32_at(d, 0x88));
    raw[0x54] = d[0x8C];
    put_u16(&mut raw, 0x56, u16_at(d, 0x8E));
    raw[0x58..0x64].copy_from_slice(&d[0x90..0x9C]);
    let chk = checksum(&raw);
    put_u16(&mut raw, 0x1C, chk);
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_and_items() {
        assert_eq!(national_from_internal(25), 25);
        assert_eq!(national_from_internal(277), 252); // Arcko
        assert_eq!(national_from_internal(411), 358); // Éoko (dernier index interne)
        assert_eq!(internal_from_national(252), 277);
        for n in 1..=386 {
            assert_eq!(national_from_internal(internal_from_national(n)), n, "{n}");
        }
        assert_eq!(national_from_internal(260), 0); // emplacement vide
        assert_eq!(item_to_gen4(13), 17); // Potion
        assert_eq!(item_from_gen4(17), 13);
        assert_eq!(item_to_gen4(52), 0); // sans équivalent
        assert_eq!(item_to_gen4(289), 328); // CT01
    }

    #[test]
    fn crypt_round_trip() {
        let mut raw = vec![0u8; PARTY_SIZE];
        put_u32(&mut raw, 0, 0x1234_5679);
        put_u32(&mut raw, 4, 0xABCD_0042);
        for (i, b) in raw[HEADER..STORED_SIZE].iter_mut().enumerate() {
            *b = i as u8;
        }
        let chk = checksum(&raw);
        put_u16(&mut raw, 0x1C, chk);
        let enc = encrypt(&raw);
        assert_ne!(enc[HEADER..STORED_SIZE], raw[HEADER..STORED_SIZE]);
        assert_eq!(decrypt(&enc), raw);
        assert!(looks_decrypted(&raw) || u16_at(&raw, 0x20) > 412);
    }
}
