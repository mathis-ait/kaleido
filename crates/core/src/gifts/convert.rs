//! Réception d'un cadeau : carte → Pokémon au format de la sauvegarde, ou objets dans le sac.
//!
//! Portage des `ConvertToPKM` de PKHeX (`PGT.cs`, `PGF.cs`, `WC6.cs`, `WC7.cs`, et
//! `MysteryGift.ApplyTemplateIVs`) avec les infos du dresseur de la sauvegarde ; sans
//! « critères » de l'utilisateur : nature, sexe, talent et IV libres sont tirés au hasard,
//! en respectant les règles de la carte (PID fixe, verrou chromatique, IV garantis…).
//! La date de rencontre est celle de la carte, sinon aujourd'hui.
//!
//! Écarts assumés :
//! - Gen 5 : sans restriction de version sur la carte, le jeu de la sauvegarde est gardé (PKHeX
//!   tire au hasard entre Noire et Blanche) ;
//! - Gen 6 : une carte au dresseur « joueur » (sexe du DO = 3) prend aussi l'ID et l'ID secret
//!   de la sauvegarde, comme en Gen 7 (PKHeX garde ceux de la carte, nuls) ;
//! - surnom par défaut : nom français de l'espèce, quelle que soit la langue du Pokémon.

use rand::Rng;
use serde::Serialize;

use super::card::{self, u16le, u32le, RIBBON_BITS_PGF, RIBBON_BITS_PK45, RIBBON_BITS_PK67, RIBBON_BITS_WC};
use super::{compatibility, versions_of, Gift, GiftError, GiftKind};
use crate::dex::{self, Game};
use crate::names;
use crate::save::session::{default_version_id, game_of, gender_from_pid, suggested_moves, today, SaveSession, Slot, SlotView};
use crate::save::{exp_for_level, Gender, GrowthRate, PkmDate, PkmFormat, Pokemon, SaveFile, SaveVersion, BOX_SLOTS};

const LCRNG_MUL: u32 = 0x41C6_4E6D;
const LCRNG_ADD: u32 = 0x6073;
// Vérifié : PKHeX Legality/RNG/Algorithms/ARNG.cs (Mult 0x6C078965, Add 1).
const ARNG_MUL: u32 = 0x6C07_8965;

fn lcrng(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(LCRNG_MUL).wrapping_add(LCRNG_ADD);
    *seed
}

/// Dresseur qui reçoit le cadeau (`ITrainerInfo` de PKHeX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftTrainer {
    pub name: String,
    pub tid: u16,
    pub sid: u16,
    pub gender: Gender,
    pub language: u8,
    /// Version exacte (`GameVersion`).
    pub version: u8,
    pub save_version: SaveVersion,
    pub console_region: u8,
    pub country: u8,
    pub region: u8,
}

impl GiftTrainer {
    pub fn from_save(save: &SaveFile) -> Self {
        let t = save.trainer();
        let o = save.trainer_origin();
        let v = save.version();
        let version = o.version.filter(|x| versions_of(v).contains(x)).unwrap_or_else(|| default_version_id(v));
        GiftTrainer {
            name: t.name,
            tid: t.tid,
            sid: t.sid,
            gender: t.gender,
            language: o.language,
            version,
            save_version: v,
            console_region: o.console_region,
            country: o.country,
            region: o.region,
        }
    }

    fn generation(&self) -> u8 {
        self.save_version.generation()
    }

    fn game(&self) -> Game {
        game_of(self.save_version)
    }
}

/// Langue admise par les Gen 4 à 6 (`GetSafeLanguage456` : sinon anglais).
fn safe_language(lang: u8, generation: u8) -> u8 {
    let ok = matches!(lang, 1..=5 | 7 | 8) || (generation >= 7 && matches!(lang, 9 | 10));
    if ok {
        lang
    } else {
        2
    }
}

fn err(e: impl std::fmt::Display) -> GiftError {
    GiftError::Invalid(e.to_string())
}

/// Modifie directement les octets déchiffrés d'un Pokémon (champs sans accesseur).
fn raw(p: &mut Pokemon, f: impl FnOnce(&mut [u8])) -> Result<(), GiftError> {
    let mut d = p.data().to_vec();
    f(&mut d);
    *p = Pokemon::from_decrypted(p.format(), &d)?;
    Ok(())
}

fn growth(game: Game, species: u16, form: u8) -> GrowthRate {
    dex::personal(game, species, form).map(|p| p.growth_rate).or_else(|| names::growth_rate(species)).unwrap_or(GrowthRate::MediumFast)
}

fn base_pp(game: Game, moves: [u16; 4]) -> [u8; 4] {
    moves.map(|m| if m == 0 { 0 } else { dex::move_info_in(game, m).map_or(5, |i| i.pp) })
}

fn set_species_name(p: &mut Pokemon, species: u16) {
    let name = dex::species_name(species).unwrap_or("?");
    if p.set_nickname(name).is_err() {
        let _ = p.set_nickname("?");
    }
    p.set_is_nicknamed(false);
}

fn set_egg_name(p: &mut Pokemon) {
    if p.set_nickname("Œuf").is_err() {
        let _ = p.set_nickname("Oeuf");
    }
}

fn card_date(gift: &Gift) -> PkmDate {
    gift.date.unwrap_or_else(today)
}

/// IV de la carte (ordre de la carte) → valeurs finales (ordre Kaleido).
/// `ApplyTemplateIVs` de PKHeX : ≤ 31 = fixe ; 0xFC à 0xFE = 1 à 3 IV parfaits garantis
/// parmi les aléatoires ; 0xFF = aléatoire.
fn template_ivs(card_ivs: [u8; 6], rng: &mut impl Rng) -> [u8; 6] {
    let mut out = card_ivs;
    let random: [bool; 6] = card_ivs.map(|v| v > 31);
    let flawless = card_ivs.iter().filter(|&&v| (0xFC..=0xFE).contains(&v)).map(|&v| v - 0xFB).max().unwrap_or(0);
    let mut current = card_ivs.iter().filter(|&&v| v == 31).count() as u8;
    let mut candidates: Vec<usize> = (0..6).filter(|&i| random[i]).collect();
    while current < flawless && !candidates.is_empty() {
        let pick = rng.gen_range(0..candidates.len());
        out[candidates.swap_remove(pick)] = 31;
        current += 1;
    }
    for i in 0..6 {
        if random[i] && out[i] != 31 {
            out[i] = rng.gen_range(0..32);
        }
    }
    card::ivs_to_kaleido(out)
}

/// Sexe d'un Pokémon 3DS (indépendant du PID) : imposé par la carte, sinon au hasard.
fn pick_gender(card_gender: u8, ratio: u8, rng: &mut impl Rng) -> Gender {
    match ratio {
        255 => Gender::Genderless,
        254 => Gender::Female,
        0 => Gender::Male,
        _ => match card_gender {
            0 => Gender::Male,
            1 => Gender::Female,
            _ if rng.gen_range(0..252u16) + 1 < ratio as u16 => Gender::Female,
            _ => Gender::Male,
        },
    }
}

/// Emplacement de talent (0, 1, 2 = caché) selon `AbilityType`.
fn pick_ability_slot(ability_type: u8, rng: &mut impl Rng) -> usize {
    match ability_type {
        0..=2 => ability_type as usize,
        3 => rng.gen_range(0..2),
        _ => rng.gen_range(0..3),
    }
}

fn ability_of(game: Game, species: u16, form: u8, slot: usize) -> u16 {
    match dex::personal(game, species, form) {
        Some(info) if info.abilities[slot] != 0 => info.abilities[slot],
        Some(info) => info.abilities[0],
        None => names::first_ability(species).unwrap_or(0),
    }
}

fn set_ribbons(d: &mut [u8], card_bytes: &[u8], from: &[(usize, u8); 15], to: &[(usize, u8); 15]) {
    for (&(ca, cb), &(pa, pb)) in from.iter().zip(to) {
        if card_bytes[ca] >> cb & 1 != 0 {
            d[pa] |= 1 << pb;
        }
    }
}

/// Version d'origine : celle de la carte, sinon celle du dresseur si la carte l'admet,
/// sinon une des versions admises (`GetCompatibleVersion` de PKHeX).
fn pick_version(origin: u8, games: &[u8], trainer_version: u8, rng: &mut impl Rng) -> u8 {
    if origin != 0 {
        origin
    } else if games.is_empty() || games.contains(&trainer_version) {
        trainer_version
    } else {
        games[rng.gen_range(0..games.len())]
    }
}

/// Convertit le cadeau en Pokémon pour la sauvegarde du dresseur `tr`.
pub fn convert(gift: &Gift, tr: &GiftTrainer, rng: &mut impl Rng) -> Result<Pokemon, GiftError> {
    if !gift.is_pokemon() {
        return Err(err("cette carte ne contient pas de Pokémon"));
    }
    if gift.generation() != tr.generation() {
        let reason = compatibility(gift, tr.save_version).err();
        return Err(GiftError::Invalid(reason.unwrap_or_else(|| "génération différente".into())));
    }
    let mut p = match gift.generation() {
        4 => convert4(gift, tr, rng)?,
        5 => convert5(gift, tr, rng)?,
        g => convert3ds(gift, tr, g == 7, rng)?,
    };
    p.refresh_checksum();
    Ok(p)
}

// --- Gen 4 (PGT.ConvertToPKM) -------------------------------------------------------------

fn convert4(gift: &Gift, tr: &GiftTrainer, rng: &mut impl Rng) -> Result<Pokemon, GiftError> {
    let game = tr.game();
    let Some(template) = &gift.template4 else {
        return manaphy_egg(tr, rng);
    };
    let mut p = template.clone();
    let info = dex::personal(game, p.species(), p.form());
    let egg = gift.kind == GiftKind::Egg;
    // « Sanity » remise à 0 (Lucario WORLD08 coréen mal formé, d'après PKHeX).
    raw(&mut p, |d| d[4..6].fill(0))?;
    // Modèle sans dresseur : le jeu y met celui qui reçoit.
    if egg || p.ot_name().is_empty() {
        p.set_ot_name(&tr.name)?;
        p.set_tid(tr.tid);
        p.set_sid(tr.sid);
        p.set_ot_gender(tr.gender);
    }
    if p.version() == 0 {
        p.set_version(tr.version);
    }
    if p.language() == 0 {
        p.set_language(safe_language(tr.language, 4));
    }
    let ratio = info.as_ref().map_or(255, |i| i.gender_ratio);
    // PID : fixe s'il vaut plus de 1 ; sinon tiré (LCRNG), jamais chromatique (ARNG en cas de
    // collision), avec le sexe du modèle.
    if p.pid() <= 1 {
        let gender = p.gender();
        let tsv = (p.tid() ^ p.sid()) as u32 >> 3;
        let mut seed: u32 = rng.gen();
        let mut pid = 0;
        for _ in 0..100_000 {
            pid = lcrng(&mut seed);
            while ((pid >> 16) ^ (pid & 0xFFFF)) >> 3 == tsv {
                pid = pid.wrapping_mul(ARNG_MUL).wrapping_add(1);
            }
            if gender_from_pid(ratio, pid) == gender {
                break;
            }
        }
        p.set_pid(pid);
    }
    // Talent du modèle, sinon celui de l'emplacement donné par le bit 0 du PID.
    if p.ability() == 0 {
        p.set_ability(ability_of(game, p.species(), p.form(), (p.pid() & 1) as usize))?;
    }
    // IV : ceux du modèle, sinon deux tirages de 15 bits (méthode « séquentielle »).
    if p.ivs().iter().all(|&v| v == 0) {
        let mut seed: u32 = rng.gen();
        let iv1 = lcrng(&mut seed) >> 16 & 0x7FFF;
        let iv2 = lcrng(&mut seed) >> 16 & 0x7FFF;
        let iv32 = iv2 << 15 | iv1;
        let ivs = card::ivs_to_kaleido(std::array::from_fn(|i| (iv32 >> (5 * i) & 31) as u8));
        p.set_ivs(ivs)?;
    }
    let friendship = info.as_ref().map_or(70, |i| if egg { i.hatch_cycles } else { i.base_friendship });
    p.set_friendship(friendship);
    let now = today();
    if egg {
        // Aucun œuf PGT ordinaire n'a été distribué : traité comme un œuf reçu.
        p.set_egg_location(p.egg_location() + 3000);
        p.set_is_egg(true);
        p.set_is_nicknamed(false);
        set_egg_name(&mut p);
        p.set_egg_date(Some(now));
    } else {
        // Le modèle range le lieu de rencontre dans « lieu de l'œuf » moins 3000.
        let met = p.egg_location() + 3000;
        p.set_egg_location(0);
        p.set_met_location(met);
        p.set_met_date(Some(now));
        p.set_is_egg(false);
    }
    Ok(p)
}

/// Œuf de Manaphy (Pokémon Ranger) : `SetDefaultManaphyEggDetails` + PID/IV « méthode 1 ».
fn manaphy_egg(tr: &GiftTrainer, rng: &mut impl Rng) -> Result<Pokemon, GiftError> {
    const MANAPHY: u16 = 490;
    const HYDRATION: u16 = 93;
    const RANGER: u16 = 3001;
    let game = tr.game();
    let mut p = Pokemon::blank(PkmFormat::Gen4);
    p.set_species(MANAPHY);
    p.set_gender(Gender::Genderless);
    let moves = [294, 145, 346, 0];
    p.set_moves(moves);
    p.set_pp([20, 30, 15, 0]);
    p.set_ability(HYDRATION)?;
    p.set_fateful_encounter(true);
    p.set_version(if [7, 8, 10, 11, 12].contains(&tr.version) { tr.version } else { 10 });
    // Ni coréen, ni langue inconnue (PKHeX).
    let lang = if matches!(tr.language, 1..=5 | 7) { tr.language } else { 2 };
    p.set_language(lang);
    p.set_ot_name(&tr.name)?;
    p.set_tid(tr.tid);
    p.set_sid(tr.sid);
    p.set_ot_gender(tr.gender);
    p.set_ball(4);
    p.set_egg_location(RANGER);
    let now = today();
    p.set_egg_date(Some(now));
    p.set_met_date(Some(now));
    p.set_exp(exp_for_level(growth(game, MANAPHY, 0), 1));
    p.set_friendship(dex::personal(game, MANAPHY, 0).map_or(10, |i| i.hatch_cycles));
    // Méthode 1 : PID puis IV ; on garde un PID non chromatique pour ce dresseur.
    let tsv = (tr.tid ^ tr.sid) as u32 >> 3;
    let mut seed: u32 = rng.gen();
    loop {
        let a = lcrng(&mut seed) >> 16;
        let b = lcrng(&mut seed) >> 16;
        let c = lcrng(&mut seed) >> 16 & 0x7FFF;
        let d = lcrng(&mut seed) >> 16 & 0x7FFF;
        if (a ^ b) >> 3 == tsv {
            continue;
        }
        p.set_pid(b << 16 | a);
        let iv32 = d << 15 | c;
        p.set_ivs(card::ivs_to_kaleido(std::array::from_fn(|i| (iv32 >> (5 * i) & 31) as u8)))?;
        break;
    }
    p.set_is_egg(true);
    set_egg_name(&mut p);
    p.set_is_nicknamed(false);
    Ok(p)
}

// --- Gen 5 (PGF.ConvertToPKM) -------------------------------------------------------------

fn convert5(gift: &Gift, tr: &GiftTrainer, rng: &mut impl Rng) -> Result<Pokemon, GiftError> {
    let c = gift.card();
    let info = gift.pokemon.as_ref().ok_or_else(|| err("carte sans Pokémon"))?;
    let game = tr.game();
    let (species, form) = (info.species, info.form);
    let personal = dex::personal(game, species, form);
    let language = match (c[0x1D], gift.restrict_language) {
        (0, 0) => safe_language(tr.language, 5),
        (0, r) => r,
        (l, _) => l,
    };
    let level = if c[0x5B] > 0 { c[0x5B] } else { rng.gen_range(1..=100) };
    let mut p = Pokemon::blank(PkmFormat::Gen5);
    p.set_species(species);
    p.set_held_item(u16le(c, 0x10));
    p.set_met_level(level.min(100))?;
    p.set_form(form)?;
    p.set_version(pick_version(c[0x04], &gift.games, tr.version, rng));
    p.set_language(language);
    p.set_ball(c[0x0E]);
    let moves = if info.moves[0] == 0 { suggested_moves(game, species, form, level) } else { info.moves };
    p.set_moves(moves);
    p.set_pp(base_pp(game, moves));
    p.set_met_location(u16le(c, 0x3A));
    p.set_egg_location(u16le(c, 0x38));
    let date = card_date(gift);
    p.set_met_date(Some(date));
    p.set_exp(exp_for_level(growth(game, species, form), level));
    p.set_fateful_encounter(true);
    let egg = info.egg;
    // Dresseur : le joueur pour un œuf, sinon celui de la carte (copié octet pour octet).
    if egg {
        p.set_tid(tr.tid);
        p.set_sid(tr.sid);
        p.set_ot_name(&tr.name)?;
        p.set_ot_gender(tr.gender);
    } else {
        p.set_tid(u16le(c, 0x00));
        p.set_sid(u16le(c, 0x02));
        raw(&mut p, |d| d[0x68..0x78].copy_from_slice(&c[0x4A..0x5A]))?;
        let g = if c[0x5A] == 3 {
            tr.gender
        } else if c[0x5A] & 1 == 1 {
            Gender::Female
        } else {
            Gender::Male
        };
        p.set_ot_gender(g);
    }
    if info.nickname.is_some() {
        raw(&mut p, |d| d[0x48..0x5E].copy_from_slice(&c[0x1E..0x34]))?;
        p.set_is_nicknamed(true);
    } else {
        set_species_name(&mut p, species);
    }
    raw(&mut p, |d| {
        // Concours (0x1E à 0x23) et rubans.
        d[0x1E..0x24].copy_from_slice(&c[0x3D..0x43]);
        set_ribbons(d, c, &RIBBON_BITS_PGF, &RIBBON_BITS_PK45);
    })?;

    // Nature, sexe, talent, PID, IV (SetPINGA).
    let nature = if c[0x34] < 25 { c[0x34] } else { rng.gen_range(0..25) };
    p.set_nature(nature)?;
    let ratio = personal.as_ref().map_or(255, |i| i.gender_ratio);
    let gender = match (ratio, c[0x35]) {
        (255, _) => Gender::Genderless,
        (_, 0) => Gender::Male,
        (_, 1) => Gender::Female,
        _ => pick_gender(2, ratio, rng),
    };
    p.set_gender(gender);
    let av = pick_ability_slot(c[0x36], rng);
    let card_pid = u32le(c, 0x08);
    let pid = if card_pid != 0 {
        card_pid
    } else {
        let mut pid: u32 = rng.gen();
        // Octet bas tiré jusqu'à obtenir le sexe voulu.
        for _ in 0..10_000 {
            pid = (pid & 0xFFFF_FF00) | rng.gen_range(0..256u32);
            if gender_from_pid(ratio, pid) == gender {
                break;
            }
        }
        match c[0x37] {
            // Toujours chromatique : MonochromeRNG.GetShinyPID.
            2 => {
                let low = pid & 0xFF;
                ((low ^ p.tid() as u32 ^ p.sid() as u32) << 16) | low
            }
            // Au hasard.
            1 => pid,
            // Jamais chromatique.
            _ => {
                let xor = (p.tid() ^ p.sid()) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF);
                if xor < 8 {
                    pid ^ 0x1000_0000
                } else {
                    pid
                }
            }
        }
    };
    let pid = if card_pid != 0 {
        pid
    } else if av == 1 {
        pid | 0x1_0000
    } else {
        pid & !0x1_0000
    };
    p.set_pid(pid);
    p.set_ability(ability_of(game, species, form, av))?;
    raw(&mut p, |d| d[0x42] = (d[0x42] & !1) | (av == 2) as u8)?;
    let ivs_card: [u8; 6] = std::array::from_fn(|i| c[0x43 + i]);
    p.set_ivs(template_ivs(ivs_card, rng))?;

    if egg {
        p.set_is_egg(true);
        p.set_egg_date(Some(date));
        set_egg_name(&mut p);
        p.set_is_nicknamed(true);
    }
    p.set_friendship(personal.as_ref().map_or(70, |i| if egg { i.hatch_cycles } else { i.base_friendship }));
    Ok(p)
}

// --- Gen 6 / 7 (WC6.ConvertToPKM, WC7.ConvertToPKM) ---------------------------------------

// Offsets PK6 / PK7 sans accesseur dans `Pokemon`.
// Vérifié : PKHeX PK6.cs / PK7.cs (Contest 0x24-0x29, RelearnMove1 0x6A, HandlingTrainerTrash 0x78
// sur 0x1A octets, HandlingTrainerGender 0x92, CurrentHandler 0x93, HandlingTrainerFriendship
// 0xA2, HT mémoire : intensité 0xA4, mémoire 0xA5, ressenti 0xA6, variable 0xA8 ;
// OriginalTrainerFriendship 0xCA, OT mémoire : intensité 0xCC, mémoire 0xCD, variable 0xCE,
// ressenti 0xD0 ; Country 0xE0, Region 0xE1, ConsoleRegion 0xE2).
mod pk6 {
    pub const CONTEST: usize = 0x24;
    pub const RELEARN: usize = 0x6A;
    pub const HT_NAME: usize = 0x78;
    pub const HT_GENDER: usize = 0x92;
    pub const CURRENT_HANDLER: usize = 0x93;
    pub const HT_FRIENDSHIP: usize = 0xA2;
    pub const HT_INTENSITY: usize = 0xA4;
    pub const HT_MEMORY: usize = 0xA5;
    pub const HT_FEELING: usize = 0xA6;
    pub const HT_VARIABLE: usize = 0xA8;
    pub const OT_NAME: usize = 0xB0;
    pub const OT_FRIENDSHIP: usize = 0xCA;
    pub const OT_INTENSITY: usize = 0xCC;
    pub const OT_MEMORY: usize = 0xCD;
    pub const OT_VARIABLE: usize = 0xCE;
    pub const OT_FEELING: usize = 0xD0;
    pub const COUNTRY: usize = 0xE0;
    pub const REGION: usize = 0xE1;
    pub const CONSOLE_REGION: usize = 0xE2;
    pub const NICKNAME: usize = 0x40;
}

fn convert3ds(gift: &Gift, tr: &GiftTrainer, gen7: bool, rng: &mut impl Rng) -> Result<Pokemon, GiftError> {
    let c = gift.card();
    let info = gift.pokemon.as_ref().ok_or_else(|| err("carte sans Pokémon"))?;
    let game = tr.game();
    let (species, form) = (info.species, info.form);
    let personal = dex::personal(game, species, form);
    let format = if gen7 { PkmFormat::Gen7 } else { PkmFormat::Gen6 };
    let language = if c[0x85] != 0 { c[0x85] } else { safe_language(tr.language, tr.generation()) };
    let level = if c[0xD0] > 0 { c[0xD0] } else { rng.gen_range(1..=100) };
    let met_level = if gen7 && c[0xA8] > 0 { c[0xA8] } else { level };
    let ec = match u32le(c, 0x70) {
        0 => rng.gen(),
        ec => ec,
    };
    let ot_gender_code = c[0xB5];
    let ot_set = c[0xB6] != 0 || c[0xB7] != 0;
    let id32 = u32le(c, 0x68);
    // Pikachu à casquette de Sacha (carte simulée du jeu) : pas de dresseur actuel.
    let ash_pikachu = gen7 && id32 == 0x798B_469B;

    let mut p = Pokemon::blank(format);
    p.set_encryption_constant(ec);
    p.set_species(species);
    p.set_held_item(u16le(c, 0x78));
    if ot_gender_code == 3 {
        p.set_tid(tr.tid);
        p.set_sid(tr.sid);
    } else {
        p.set_tid(u16le(c, 0x68));
        p.set_sid(u16le(c, 0x6A));
    }
    p.set_met_level(met_level.min(100))?;
    p.set_form(form)?;
    p.set_version(pick_version(c[0x6C], &gift.games, tr.version, rng));
    p.set_language(language);
    p.set_ball(c[0x76]);
    p.set_moves(info.moves);
    p.set_pp(base_pp(game, info.moves));
    p.set_met_location(u16le(c, 0xA6));
    p.set_egg_location(u16le(c, 0xA4));
    if ot_set {
        raw(&mut p, |d| d[pk6::OT_NAME..pk6::OT_NAME + 0x1A].copy_from_slice(&c[0xB6..0xB6 + 0x1A]))?;
    } else {
        p.set_ot_name(&tr.name)?;
    }
    p.set_ot_gender(if ot_gender_code == 3 {
        tr.gender
    } else if ot_gender_code & 1 == 1 {
        Gender::Female
    } else {
        Gender::Male
    });
    p.set_exp(exp_for_level(growth(game, species, form), level));
    p.set_fateful_encounter(info.fateful);
    p.set_evs(info.evs);
    let base_friendship = personal.as_ref().map_or(70, |i| i.base_friendship);
    // Nom du dresseur actuel : on l'encode via le champ du DO d'un Pokémon temporaire.
    let mut tmp = Pokemon::blank(format);
    tmp.set_ot_name(&tr.name)?;
    let ht_name = tmp.data()[pk6::OT_NAME..pk6::OT_NAME + 0x1A].to_vec();
    let handled = ot_set && !ash_pikachu;
    let memory_feeling: u8 = rng.gen_range(0..10);
    raw(&mut p, |d| {
        d[pk6::CONTEST..pk6::CONTEST + 6].copy_from_slice(&c[0xA9..0xAF]);
        d[pk6::RELEARN..pk6::RELEARN + 8].copy_from_slice(&c[0xD8..0xE0]);
        set_ribbons(d, c, &RIBBON_BITS_WC, &RIBBON_BITS_PK67);
        d[pk6::OT_FRIENDSHIP] = base_friendship;
        d[pk6::OT_INTENSITY] = c[0xE0];
        d[pk6::OT_MEMORY] = c[0xE1];
        d[pk6::OT_VARIABLE..pk6::OT_VARIABLE + 2].copy_from_slice(&c[0xE2..0xE4]);
        d[pk6::OT_FEELING] = c[0xE4];
        d[pk6::COUNTRY] = tr.country;
        d[pk6::REGION] = tr.region;
        d[pk6::CONSOLE_REGION] = tr.console_region;
        if handled {
            d[pk6::HT_NAME..pk6::HT_NAME + 0x1A].copy_from_slice(&ht_name);
            d[pk6::HT_GENDER] = (tr.gender == Gender::Female) as u8;
            d[pk6::HT_FRIENDSHIP] = base_friendship;
            d[pk6::CURRENT_HANDLER] = 1;
        }
        // Gen 6 : souvenir « rencontré à … » (mémoire 3, variable 9, intensité 1).
        if !gen7 && !info.egg {
            let (intensity, memory, feeling, variable) = if handled {
                (pk6::HT_INTENSITY, pk6::HT_MEMORY, pk6::HT_FEELING, pk6::HT_VARIABLE)
            } else {
                (pk6::OT_INTENSITY, pk6::OT_MEMORY, pk6::OT_FEELING, pk6::OT_VARIABLE)
            };
            d[memory] = 3;
            d[variable..variable + 2].copy_from_slice(&9u16.to_le_bytes());
            d[intensity] = 1;
            d[feeling] = memory_feeling;
        }
    })?;
    let date = card_date(gift);
    p.set_met_date(Some(date));
    let nicknamed = info.nickname.is_some() || (gen7 && info.egg);
    if info.nickname.is_some() {
        raw(&mut p, |d| d[pk6::NICKNAME..pk6::NICKNAME + 0x1A].copy_from_slice(&c[0x86..0x86 + 0x1A]))?;
        p.set_is_nicknamed(true);
    } else {
        set_species_name(&mut p, species);
        p.set_is_nicknamed(nicknamed);
    }

    // Nature, sexe, talent, PID, IV (SetPINGA).
    let nature = if c[0xA0] < 25 { c[0xA0] } else { rng.gen_range(0..25) };
    p.set_nature(nature)?;
    let ratio = personal.as_ref().map_or(255, |i| i.gender_ratio);
    p.set_gender(if c[0xA1] == 2 { Gender::Genderless } else { pick_gender(c[0xA1], ratio, rng) });
    let av = pick_ability_slot(c[0xA2], rng);
    p.set_ability(ability_of(game, species, form, av))?;
    p.set_ability_number(1 << av)?;
    let (tid, sid) = (p.tid() as u32, p.sid() as u32);
    let pid = match c[0xA3] {
        0 => u32le(c, 0xD4),
        1 => rng.gen(),
        2 => {
            let low = rng.gen::<u32>() & 0xFFFF;
            ((low ^ tid ^ sid) << 16) | low
        }
        _ => {
            let pid: u32 = rng.gen();
            if (tid ^ sid ^ (pid >> 16) ^ (pid & 0xFFFF)) < 16 {
                pid ^ 0x1000_0000
            } else {
                pid
            }
        }
    };
    p.set_pid(pid);
    let ivs_card: [u8; 6] = std::array::from_fn(|i| c[0xAF + i]);
    p.set_ivs(template_ivs(ivs_card, rng))?;

    if info.egg {
        p.set_is_egg(true);
        p.set_egg_date(Some(date));
        set_egg_name(&mut p);
        p.set_is_nicknamed(true);
    }
    p.set_friendship(personal.as_ref().map_or(70, |i| if info.egg { i.hatch_cycles } else { i.base_friendship }));
    Ok(p)
}

// --- Ajout à la sauvegarde ------------------------------------------------------------------

/// Résultat d'un ajout : message pour l'interface et emplacement du Pokémon reçu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOutcome {
    pub message: String,
    pub warning: Option<String>,
    pub slot: Option<Slot>,
    pub pokemon: Option<SlotView>,
}

fn first_empty_box_slot(session: &SaveSession) -> Result<Option<Slot>, GiftError> {
    for b in 0..session.save.box_count() {
        for index in 0..BOX_SLOTS {
            let slot = Slot::Box { r#box: b, index };
            if session.get(slot)?.is_none() {
                return Ok(Some(slot));
            }
        }
    }
    Ok(None)
}

/// Ajoute le cadeau à la sauvegarde, en une seule étape d'historique : Pokémon dans
/// `slot` s'il est libre (sinon le premier emplacement libre des boîtes), objets dans le sac.
pub fn add_to_save(session: &mut SaveSession, gift: &Gift, slot: Option<Slot>, rng: &mut impl Rng) -> Result<AddOutcome, GiftError> {
    let version = session.save.version();
    compatibility(gift, version).map_err(GiftError::Invalid)?;
    let warning = super::version_warning(gift, version);
    if gift.is_pokemon() {
        let tr = GiftTrainer::from_save(&session.save);
        let p = convert(gift, &tr, rng)?;
        let target = match slot {
            Some(s) if session.get(s)?.is_none() => Some(s),
            _ => first_empty_box_slot(session)?,
        };
        let target = target.ok_or_else(|| err("aucun emplacement libre dans les boîtes : libère une place puis recommence"))?;
        let view = session.import(target, p.stored_data())?;
        let place = match view.slot {
            Slot::Box { r#box, index } => {
                let name = session.save.box_name(r#box).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Boîte {}", r#box + 1));
                format!("{name}, emplacement {}", index + 1)
            }
            Slot::Party { index } => format!("l'équipe (position {})", index + 1),
        };
        let what = if p.is_egg() { format!("Œuf de {}", view.species_name) } else { view.species_name.clone() };
        return Ok(AddOutcome { message: format!("{what} reçu : {place}"), warning, slot: Some(view.slot), pokemon: Some(view) });
    }
    if gift.kind == GiftKind::Item {
        let mut pouches = session.inventory()?;
        let mut added = Vec::new();
        for item in &gift.items {
            let name = dex::item_name(item.id).map_or_else(|| format!("objet n°{}", item.id), str::to_string);
            let pouch = pouches
                .iter_mut()
                .find(|p| p.allowed.contains(&item.id))
                .ok_or_else(|| err(format!("{name} n'a pas de poche dans le sac de ce jeu")))?;
            let max = pouch.max_count.max(1);
            match pouch.items.iter().position(|i| i.id == item.id) {
                Some(at) => pouch.items[at].count = pouch.items[at].count.saturating_add(item.count).min(max),
                None if pouch.items.len() < pouch.capacity => {
                    pouch.items.push(crate::save::InventoryItem { id: item.id, count: item.count.min(max), is_new: true, is_favorite: false })
                }
                None => return Err(err(format!("la poche « {} » est pleine : impossible d'ajouter {name}", pouch.kind.label_fr()))),
            }
            added.push(if item.count > 1 { format!("{name} × {}", item.count) } else { name });
        }
        session.set_inventory(&pouches)?;
        return Ok(AddOutcome { message: format!("Ajouté au sac : {}", added.join(", ")), warning, slot: None, pokemon: None });
    }
    Err(err("ce type de cadeau n'est pas pris en charge"))
}

/// [`add_to_save`] avec le générateur aléatoire du système.
pub fn receive(session: &mut SaveSession, gift: &Gift, slot: Option<Slot>) -> Result<AddOutcome, GiftError> {
    add_to_save(session, gift, slot, &mut rand::thread_rng())
}
