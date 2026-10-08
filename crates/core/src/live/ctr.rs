//! Combat 3DS (6e et 7e générations) : Pokémon en combat lus dans la FCRAM.
//!
//! Observé dans Azahar sur Rubis Oméga : pendant un combat, chaque Pokémon (le nôtre comme
//! l'adversaire) a dans le tas du jeu
//! - ses données « boîte » au format PK6, chiffrées comme dans la sauvegarde (232 octets), tenues
//!   par un petit objet `[table virtuelle][adresse des données]…` placé juste avant ;
//! - un bloc de combat dont le premier mot est l'adresse (virtuelle, côté jeu) de cet objet, avec
//!   l'espèce en +0x0C, les PV max en +0x0E, les PV actuels en +0x10, le niveau en +0x18, puis
//!   de nouveau l'espèce en +0xF4 suivie des cinq autres statistiques (Att, Déf, Att Spé,
//!   Déf Spé, Vit). Les PV de ce bloc changent en direct pendant le combat.
//!
//! Le lien entre les deux ne dépend d'aucune adresse fixe : l'objet est à `k` octets avant les
//! données (k ≤ 0x100) et son second mot vaut `adresse de l'objet + k` dans l'espace du jeu. On
//! cherche donc les deux familles par leur propre cohérence (somme de contrôle du PK6, espèce
//! répétée du bloc de combat), puis on les relie.

use std::collections::HashSet;

use super::{MemorySource, Region};
use crate::save::{PkmFormat, Pokemon};

/// Bloc de combat : offsets observés (Rubis Oméga).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamLayout {
    pub species: usize,
    pub max_hp: usize,
    pub hp: usize,
    pub level: usize,
    /// Espèce répétée, suivie des cinq statistiques hors PV (ordre Kaleido).
    pub stats: usize,
}

pub const GEN6_PARAM: ParamLayout = ParamLayout { species: 0x0C, max_hp: 0x0E, hp: 0x10, level: 0x18, stats: 0xF4 };

impl ParamLayout {
    fn len(&self) -> usize {
        self.stats + 12
    }
}

/// Bloc de combat lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Premier mot : adresse (côté jeu) de l'objet qui tient les données PK6.
    pub object: u32,
    pub species: u16,
    pub max_hp: u16,
    pub hp: u16,
    pub level: u8,
    /// Att, Déf, Att Spé, Déf Spé, Vit.
    pub stats: [u16; 5],
}

const MAX_SPECIES: u16 = 807;

fn rd16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn rd32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Lit un bloc de combat au début de `b` (au moins `layout.len()` octets), s'il est cohérent.
pub fn param_at(layout: &ParamLayout, b: &[u8]) -> Option<Param> {
    if b.len() < layout.len() {
        return None;
    }
    let species = rd16(b, layout.species);
    if !(1..=MAX_SPECIES).contains(&species) || rd16(b, layout.stats) != species {
        return None;
    }
    let (max_hp, hp, level) = (rd16(b, layout.max_hp), rd16(b, layout.hp), b[layout.level]);
    if max_hp == 0 || max_hp > 999 || hp > max_hp || !(1..=100).contains(&level) {
        return None;
    }
    let stats: [u16; 5] = std::array::from_fn(|i| rd16(b, layout.stats + 2 + 2 * i));
    if stats.iter().any(|&s| s == 0 || s > 999) {
        return None;
    }
    let object = rd32(b, 0);
    // Adresse côté jeu : alignée, hors de la page nulle.
    if object & 3 != 0 || object < 0x0010_0000 {
        return None;
    }
    Some(Param { object, species, max_hp, hp, level, stats })
}

/// Filtre rapide d'un PK6/PK7 chiffré (232 octets) : constante non nulle, octets 4-5 nuls, somme
/// de contrôle juste. Le mélange des blocs ne change pas la somme : pas besoin de le défaire.
pub fn encrypted_box_mon(b: &[u8]) -> bool {
    if b.len() < 232 || b[4] != 0 || b[5] != 0 {
        return false;
    }
    let ec = rd32(b, 0);
    let chk = rd16(b, 6);
    if ec == 0 || chk == 0 {
        return false;
    }
    let mut seed = ec;
    let mut sum: u16 = 0;
    for w in 0..112 {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        sum = sum.wrapping_add(rd16(b, 8 + 2 * w) ^ (seed >> 16) as u16);
    }
    sum == chk
}

/// Ce qu'un balayage de la FCRAM a trouvé.
#[derive(Debug, Clone, Default)]
pub struct Found {
    /// Données PK6/PK7 valides : (adresse hôte, Pokémon déchiffré).
    pub mons: Vec<(u64, Pokemon)>,
    /// Blocs de combat cohérents : (adresse hôte, bloc).
    pub params: Vec<(u64, Param)>,
}

/// Lecture par morceaux, avec un recouvrement pour les structures à cheval.
const CHUNK: usize = 4 << 20;
const OVERLAP: usize = 0x200;

/// Balaye les zones : Pokémon chiffrés et blocs de combat.
pub fn scan(src: &dyn MemorySource, regions: &[Region], format: PkmFormat, layout: &ParamLayout) -> Found {
    let mut found = Found::default();
    let mut buf = vec![0u8; CHUNK + OVERLAP];
    for r in regions {
        let mut at = r.base & !3;
        while at < r.end() {
            let len = ((r.end() - at) as usize).min(CHUNK + OVERLAP) & !3;
            if len < 4 {
                break;
            }
            let chunk = &mut buf[..len];
            if src.read(at, chunk).is_ok() {
                let stop = len.min(CHUNK);
                let mut i = 0;
                while i < stop {
                    let rest = &chunk[i..];
                    if rest.len() >= 232 && encrypted_box_mon(rest) {
                        if let Ok(p) = Pokemon::from_encrypted(format, &rest[..232]) {
                            if (1..=MAX_SPECIES).contains(&p.species()) && p.checksum_valid() {
                                found.mons.push((at + i as u64, p));
                            }
                        }
                    } else if let Some(p) = param_at(layout, rest) {
                        found.params.push((at + i as u64, p));
                    }
                    i += 4;
                }
            }
            at += CHUNK as u64;
        }
    }
    found
}

/// Relie un bloc de combat aux données PK6 de son Pokémon : l'objet qui les tient est à `k`
/// octets avant elles et pointe sur elles (`objet + k`, adresses côté jeu).
pub fn link(src: &dyn MemorySource, param: &Param, mons: &[(u64, Pokemon)]) -> Option<usize> {
    mons.iter().position(|(at, p)| {
        if p.species() != param.species {
            return false;
        }
        let Some(start) = at.checked_sub(0x100) else { return false };
        let Ok(before) = src.read_vec(start, 0x100) else { return false };
        (2..=0x40).any(|n| {
            let k = 4 * n as u32;
            let o = 0x100 - k as usize;
            rd32(&before, o + 4) == param.object.wrapping_add(k)
        })
    })
}

/// Pokémon en combat : données PK6 complétées par le bloc de combat (niveau, statistiques, PV).
#[derive(Debug, Clone)]
pub struct Fighter {
    pub param_at: u64,
    pub param: Param,
    pub mon: Pokemon,
}

impl Fighter {
    pub fn key(&self) -> u32 {
        let d = self.mon.data();
        u32::from_le_bytes([d[0], d[1], d[2], d[3]])
    }

    /// Le Pokémon avec la section équipe remplie d'après le combat.
    pub fn pokemon(&self) -> Pokemon {
        let mut p = self.mon.clone();
        let s = self.param.stats;
        p.set_party_stats(self.param.level, [self.param.max_hp, s[0], s[1], s[2], s[3], s[4]]);
        p.set_current_hp(self.param.hp);
        p
    }
}

/// Tous les Pokémon en combat trouvés en mémoire (blocs reliés à des données PK6), sans doublon
/// de Pokémon : le jeu garde deux blocs par Pokémon (mêmes PV), le premier est gardé.
pub fn fighters(src: &dyn MemorySource, found: &Found) -> Vec<Fighter> {
    let mut out: Vec<Fighter> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for (at, param) in &found.params {
        let Some(i) = link(src, param, &found.mons) else { continue };
        let mon = found.mons[i].1.clone();
        let f = Fighter { param_at: *at, param: param.clone(), mon };
        if seen.insert(f.key()) {
            out.push(f);
        }
    }
    out
}

/// Relit un bloc de combat connu : `None` s'il ne vise plus le même Pokémon.
pub fn reread(src: &dyn MemorySource, layout: &ParamLayout, f: &Fighter) -> Option<Fighter> {
    let b = src.read_vec(f.param_at, layout.len()).ok()?;
    let p = param_at(layout, &b)?;
    (p.object == f.param.object && p.species == f.param.species).then(|| Fighter { param: p, ..f.clone() })
}

// --- Équipe en direct (hors combat).
//
// La copie de l'équipe au format « équipe » (260 octets) que l'on trouve en FCRAM est l'image du
// bloc de sauvegarde : le jeu ne la réécrit qu'en sauvegardant. L'équipe vivante est faite de six
// objets `[table virtuelle][adresse des données PK6][adresse de la section équipe][…]`, alloués
// d'avance, et d'un tableau `[6 adresses d'objets][nombre de Pokémon]`. La section équipe
// (28 octets : statut, niveau, PV, statistiques) est chiffrée à part, avec la même constante.
// Observé sur Rubis Oméga (objets espacés de 0x1E4, tableau 0x4C avant le premier objet).

/// Taille de la section équipe des formats 3DS.
pub const PARTY_EXT: usize = 28;

/// Déchiffre la section équipe (28 octets) avec la constante de chiffrement.
pub fn decrypt_ext(ec: u32, b: &[u8]) -> [u8; PARTY_EXT] {
    let mut out = [0u8; PARTY_EXT];
    let mut seed = ec;
    for w in 0..PARTY_EXT / 2 {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        let v = rd16(b, 2 * w) ^ (seed >> 16) as u16;
        out[2 * w..2 * w + 2].copy_from_slice(&v.to_le_bytes());
    }
    out
}

/// Section équipe déchiffrée plausible : niveau 1-100, PV ≤ PV max, statistiques non nulles.
pub fn plausible_ext(d: &[u8; PARTY_EXT]) -> bool {
    let level = d[4];
    let (hp, max) = (rd16(d, 8), rd16(d, 10));
    (1..=100).contains(&level) && max > 0 && max <= 999 && hp <= max && (0..5).all(|i| (1..=999).contains(&rd16(d, 12 + 2 * i)))
}

/// Objet Pokémon : sa table virtuelle et où sont ses données (adresses hôtes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonObject {
    pub vtable: u32,
    pub data: u64,
    pub ext: u64,
}

/// Lit l'objet à l'adresse hôte `o`, sachant la translation `host = jeu + off`.
fn object_at(src: &dyn MemorySource, o: u64, off: u64) -> Option<MonObject> {
    let b = src.read_vec(o, 12).ok()?;
    let (vtable, data, ext) = (rd32(&b, 0), rd32(&b, 4), rd32(&b, 8));
    Some(MonObject { vtable, data: (data as u64).wrapping_add(off), ext: (ext as u64).wrapping_add(off) })
}

/// Lit un Pokémon vivant (données + section équipe) : `None` si incohérent (écriture en cours).
pub fn read_live_mon(src: &dyn MemorySource, format: PkmFormat, obj: &MonObject) -> Option<Pokemon> {
    let data = src.read_vec(obj.data, 232).ok()?;
    if !encrypted_box_mon(&data) {
        return None;
    }
    let ext = src.read_vec(obj.ext, PARTY_EXT).ok()?;
    let mut raw = data;
    raw.extend_from_slice(&ext);
    let p = Pokemon::from_encrypted(format, &raw).ok()?;
    (p.checksum_valid() && (1..=MAX_SPECIES).contains(&p.species()) && p.party_level().is_some()).then_some(p)
}

/// Tableau de l'équipe vivante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveParty {
    /// Adresse hôte du tableau `[6 adresses][nombre]`.
    pub at: u64,
    /// Translation adresse du jeu → adresse hôte.
    pub off: u64,
    pub vtable: u32,
}

impl LiveParty {
    /// Équipe actuelle. `None` : tableau incohérent (partie relancée) ou lu pendant une écriture.
    pub fn read(&self, src: &dyn MemorySource, format: PkmFormat) -> Option<Vec<Pokemon>> {
        let b = src.read_vec(self.at, 28).ok()?;
        let n = rd32(&b, 24) as usize;
        if n > 6 {
            return None;
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let ptr = rd32(&b, 4 * i);
            let obj = object_at(src, (ptr as u64).wrapping_add(self.off), self.off)?;
            if obj.vtable != self.vtable {
                return None;
            }
            out.push(read_live_mon(src, format, &obj)?);
        }
        Some(out)
    }
}

/// Objet qui tient les données PK6 trouvées à `data` (adresse hôte) : (objet, adresse côté jeu
/// de l'objet, translation). L'objet est jusqu'à 0x100 octets avant, sa section équipe se
/// déchiffre avec la constante `ec`.
pub fn owner_of(src: &dyn MemorySource, data: u64, ec: u32) -> Option<(MonObject, u32, u64)> {
    let start = data.checked_sub(0x100)?;
    let before = src.read_vec(start, 0x100).ok()?;
    for n in 3..=0x40usize {
        let k = 4 * n;
        let o = 0x100 - k;
        let (vtable, d, e) = (rd32(&before, o), rd32(&before, o + 4), rd32(&before, o + 8));
        let delta = e.wrapping_sub(d) as i32;
        if vtable < 0x0010_0000 || vtable & 3 != 0 || !(-0x10000..0x10000).contains(&delta) || delta == 0 {
            continue;
        }
        let ext_at = data.wrapping_add(delta as i64 as u64);
        let Ok(ext) = src.read_vec(ext_at, PARTY_EXT) else { continue };
        if !plausible_ext(&decrypt_ext(ec, &ext)) {
            continue;
        }
        // Adresse côté jeu de l'objet : celle des données moins k.
        let vaddr = d.wrapping_sub(k as u32);
        let off = (start + o as u64).wrapping_sub(vaddr as u64);
        return Some((MonObject { vtable, data, ext: ext_at }, vaddr, off));
    }
    None
}

/// Cherche le tableau de l'équipe vivante à partir d'un Pokémon de l'équipe (adresse côté jeu de
/// son objet) : six adresses d'objets de même table virtuelle, puis le nombre de Pokémon.
pub fn find_live_party(src: &dyn MemorySource, regions: &[Region], vaddr: u32, off: u64, vtable: u32) -> Option<LiveParty> {
    for (hit, _) in super::scan::find_u32s(src, regions, &[vaddr], 64) {
        for j in 0..6u64 {
            let Some(at) = hit.checked_sub(4 * j) else { continue };
            let Ok(b) = src.read_vec(at, 28) else { continue };
            let n = rd32(&b, 24) as usize;
            if !(1..=6).contains(&n) || rd32(&b, 4 * j as usize) != vaddr {
                continue;
            }
            let ok = (0..6).all(|i| {
                let p = rd32(&b, 4 * i);
                p != 0 && object_at(src, (p as u64).wrapping_add(off), off).is_some_and(|o| o.vtable == vtable)
            });
            if ok {
                return Some(LiveParty { at, off, vtable });
            }
        }
    }
    None
}
