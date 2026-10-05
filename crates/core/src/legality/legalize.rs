//! « Rendre légal » et « Générer un Pokémon légal » (au mieux, comme l'Auto-Legality
//! Mod de PKHeX) : on choisit une rencontre possible puis on réécrit le Pokémon
//! (lieu, niveau et date de rencontre, version, Ball, talent, PID / IV selon la méthode
//! du jeu, attaques, PP, rencontre fatidique, dresseur) en gardant au maximum ce que
//! l'utilisateur avait choisi (chromatique, nature, sexe, IV, EV, attaques légales).

use serde::{Deserialize, Serialize};

use super::encounters::{self, game_versions, AbilityRule, Encounter, EncounterKind, ShinyRule};
use super::evolution;
use super::learn;
use super::rng::{self, PidType, PidWish, Rand};
use super::verify::{self, analyze, Report, Verdict};
use crate::dex::{self, Game};
use crate::save::session::{max_pp, today, LANGUAGE_FR};
use crate::save::{exp_for_level, Gender, PkmFormat, Pokemon, Trainer};

/// Résultat de « Rendre légal ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeOutcome {
    #[serde(skip)]
    pub pokemon: Pokemon,
    /// Modifications faites, en français.
    pub changes: Vec<String>,
    pub report: Report,
    /// Le Pokémon est désormais légal (ou seulement douteux).
    pub success: bool,
}

/// Demande de « Générer un Pokémon légal ».
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GenerateRequest {
    pub species: u16,
    pub form: u8,
    pub level: u8,
    pub shiny: Option<bool>,
    pub nature: Option<u8>,
    pub gender: Option<Gender>,
    /// 1, 2 ou 4 (caché).
    pub ability_number: Option<u8>,
    pub ball: Option<u8>,
    pub moves: Option<[u16; 4]>,
    pub ivs: Option<[u8; 6]>,
    pub evs: Option<[u8; 6]>,
    pub held_item: Option<u16>,
    pub nickname: Option<String>,
    /// Rencontre précise (indice dans [`encounters::encounters`] du jeu) : « Créer ce Pokémon ».
    pub encounter_index: Option<usize>,
    /// Jeu de cette rencontre (`dp`, `pt`… ; par défaut celui de la sauvegarde).
    pub encounter_game: Option<String>,
}

/// Une façon d'obtenir le Pokémon : rencontre + version.
#[derive(Clone)]
struct Plan {
    enc: Encounter,
    version: u8,
}

fn raw(p: &Pokemon, f: impl FnOnce(&mut [u8])) -> Pokemon {
    let mut d = p.data().to_vec();
    f(&mut d);
    Pokemon::from_decrypted(p.format(), &d).unwrap_or_else(|_| p.clone())
}

fn set_relearn(p: &Pokemon, moves: [u16; 4]) -> Pokemon {
    if p.format().generation() < 6 {
        return p.clone();
    }
    raw(p, |d| {
        for (i, m) in moves.iter().enumerate() {
            d[0x6A + 2 * i..0x6C + 2 * i].copy_from_slice(&m.to_le_bytes());
        }
    })
}

fn set_gen5_hidden(p: &Pokemon, hidden: bool, n_sparkle: bool) -> Pokemon {
    if p.format() != PkmFormat::Gen5 {
        return p.clone();
    }
    raw(p, |d| d[0x42] = (d[0x42] & !3) | hidden as u8 | (n_sparkle as u8) << 1)
}

/// IV du jeu (32 bits) → ordre Kaleido.
fn ivs_from_iv32(iv32: u32) -> [u8; 6] {
    let g: [u8; 6] = std::array::from_fn(|i| ((iv32 >> (5 * i)) & 31) as u8);
    [g[0], g[1], g[2], g[4], g[5], g[3]]
}

fn growth(game: Game, species: u16, form: u8) -> Option<crate::save::GrowthRate> {
    dex::personal(game, species, form).map(|p| p.growth_rate).or_else(|| crate::names::growth_rate(species))
}

/// Lieu d'éclosion par défaut (`Locations.HatchLocation*`).
fn hatch_location(version: u8) -> u16 {
    match version {
        encounters::HG | encounters::SS => 182,
        10..=12 => 4,
        20..=23 => 64,
        24 | 25 => 38,
        26 | 27 => 318,
        _ => 78,
    }
}

/// Priorité d'une rencontre pour la création (plus petit = préféré).
fn kind_rank(e: &Encounter, species: u16) -> u32 {
    let base = match e.kind {
        k if k.is_wild() && !matches!(k, EncounterKind::HiddenGrotto | EncounterKind::FriendSafari | EncounterKind::Swarm) => 0,
        EncounterKind::Static | EncounterKind::Gift | EncounterKind::Roaming => 10,
        EncounterKind::HiddenGrotto | EncounterKind::FriendSafari | EncounterKind::Swarm => 15,
        EncounterKind::Egg => 20,
        EncounterKind::EggGift => 25,
        EncounterKind::Trade => 30,
        _ => 40,
    };
    base + if e.species == species { 0 } else { 5 }
}

/// Jeux d'origine possibles pour un Pokémon de la sauvegarde (même génération d'abord).
fn origin_games(game: Game) -> Vec<Game> {
    let f = game.generation();
    let mut out = vec![game];
    for g in Game::ALL {
        if g != game && g.generation() == f {
            out.push(g);
        }
    }
    for gen in (4..f).rev() {
        for g in Game::ALL {
            if g.generation() == gen {
                out.push(g);
            }
        }
    }
    out
}

/// Rencontres possibles, de la plus naturelle à la plus exotique.
fn plans(game: Game, species: u16, form: u8, level: u8, prefer_version: u8) -> Vec<Plan> {
    let mut out: Vec<(u32, Plan)> = Vec::new();
    let format = game.generation();
    for (gi, og) in origin_games(game).into_iter().enumerate() {
        let chain = evolution::chain(format, species, form, level);
        let list = encounters::encounters(og);
        for st in &chain {
            if st.species > dex::max_species(og) {
                continue;
            }
            for e in list {
                if e.species != st.species || !(e.form == st.form || e.form >= 30 || e.form == 0 && st.form == 0) {
                    continue;
                }
                if matches!(e.kind, EncounterKind::DreamRadar | EncounterKind::DreamWorld) && gi > 0 {
                    continue;
                }
                if e.level_min > level {
                    continue;
                }
                let need = chain.iter().take_while(|s| s.species != st.species).map(|s| s.level_min).max().unwrap_or(1);
                if need > level {
                    continue;
                }
                let version = if e.versions.contains(&prefer_version) { prefer_version } else { e.versions[0] };
                out.push(((gi as u32) * 100 + kind_rank(e, species), Plan { enc: e.clone(), version }));
            }
        }
        // Œuf de Pension (si l'espèce de base peut se reproduire).
        let (bs, bf) = chain.last().map_or((species, form), |s| (s.species, s.form));
        if bs <= dex::max_species(og) {
            let info = dex::personal(og, bs, bf);
            let breedable = info.as_ref().is_some_and(|i| i.egg_groups[0] != 15) || is_baby(bs);
            if breedable && !is_unbreedable(bs) {
                let e = Encounter::egg(og, bs, bf);
                let versions = game_versions(og);
                let version = if versions.contains(&prefer_version) { prefer_version } else { versions[0] };
                out.push(((gi as u32) * 100 + kind_rank(&e, species), Plan { enc: e, version }));
            }
        }
    }
    out.sort_by_key(|(r, _)| *r);
    out.into_iter().map(|(_, p)| p).collect()
}

/// Bébés (groupe Œuf « Inconnu » mais obtenus par reproduction des parents).
fn is_baby(s: u16) -> bool {
    matches!(s, 172 | 173 | 174 | 175 | 236 | 238 | 239 | 240 | 298 | 360 | 406 | 433 | 438 | 439 | 440 | 446 | 447 | 458)
}

/// Pokémon qui ne peuvent pas se reproduire (Métamorph compris, comme parent seulement).
fn is_unbreedable(s: u16) -> bool {
    matches!(s, 132 | 490)
}

/// Souhaits de l'utilisateur, déduits du Pokémon actuel.
#[derive(Clone, Copy)]
struct Wishes {
    shiny: bool,
    nature: u8,
    gender: Gender,
    /// 1, 2 ou 4.
    ability_number: u8,
    ball: u8,
}

fn choose_ability(game: Game, e: &Encounter, species: u16, form: u8, wished: u8) -> u8 {
    let info = dex::personal(game, species, form);
    let has_hidden = info.as_ref().is_some_and(|i| i.abilities[2] != 0);
    let allows_hidden = match e.kind {
        EncounterKind::Egg => e.generation >= 5,
        _ => e.ability.allows_hidden(),
    };
    match e.ability {
        AbilityRule::OnlyHidden if has_hidden => 4,
        AbilityRule::OnlyFirst if e.generation <= 5 => 1,
        AbilityRule::OnlySecond if e.generation <= 5 => 2,
        _ if wished == 4 && allows_hidden && has_hidden => 4,
        _ if wished == 2 => 2,
        _ => 1,
    }
}

fn ball_allowed(e: &Encounter, version: u8, ball: u8) -> bool {
    if let Some(f) = e.ball {
        return ball == f;
    }
    match e.kind {
        EncounterKind::Egg => ball != 1 && ball != 16 && ball != 0 && ball <= if e.generation >= 7 { 26 } else { 25 },
        EncounterKind::DreamWorld => ball == 25 || verify::wild_balls(5, version).contains(&ball),
        _ => verify::wild_balls(e.generation, version).contains(&ball),
    }
}

/// Réécrit le Pokémon pour la rencontre `plan`.
fn apply(pk: &Pokemon, game: Game, trainer: &Trainer, plan: &Plan, wishes: Wishes, rand: &mut Rand, changes: &mut Vec<String>) -> Pokemon {
    let e = &plan.enc;
    let format = pk.format();
    let fgen = format.generation();
    let origin_gen = e.generation;
    let met_replaced = origin_gen <= 4 && fgen >= 5;
    let mut p = pk.clone();
    let species = p.species();
    let form = p.form();

    // Forme de combat → forme normale.
    let out_form = match species {
        555 => form & 2,
        774 if form < 7 => form + 7,
        778 => form & 2,
        658 if form == 2 => 1,
        800 if form == 3 => 0,
        718 if form == 4 => 0,
        351 | 421 | 648 | 681 | 716 | 746 | 382 | 383 => 0,
        s if form != 0 && [3, 6, 9, 65, 94, 115, 127, 130, 142, 150, 181, 212, 214, 229, 248, 257, 282, 303, 306, 308, 310, 354, 359, 380, 381, 445, 448, 460, 15, 18, 80, 208, 254, 260, 302, 319, 323, 334, 362, 373, 376, 384, 428, 475, 531, 719].contains(&s) => 0,
        _ => form,
    };
    if out_form != form {
        let _ = p.set_form(out_form);
        changes.push("Forme de combat remplacée par la forme normale".into());
    }

    // Niveau : au moins celui de la rencontre.
    let mut level = verify::growth_level(&p, game);
    if level < e.level_min && !e.is_egg() {
        level = e.level_min;
        if let Some(g) = growth(game, species, p.form()) {
            p.set_exp(exp_for_level(g, level));
        }
        changes.push(format!("Niveau relevé à {level} (niveau minimal de la rencontre)"));
    }

    // Version, langue.
    if p.version() != plan.version {
        changes.push(format!("Jeu d'origine : {}", dex::game_name(plan.version).unwrap_or("?")));
    }
    p.set_version(plan.version);
    let max_lang = if fgen >= 7 { 10 } else { 8 };
    let lang = p.language();
    if lang == 0 || lang == 6 || lang > max_lang {
        p.set_language(LANGUAGE_FR);
        changes.push("Langue : français".into());
    }

    // Dresseur.
    if let Some(t) = &e.trainer {
        p.set_tid(t.tid);
        p.set_sid(t.sid);
        let lang = p.language();
        if let Some((_, name)) = t.names.iter().find(|(l, _)| *l == lang).or_else(|| t.names.iter().find(|(l, _)| *l == 3)).or(t.names.first()) {
            let _ = p.set_ot_name(name);
        }
        if let Some(g) = t.ot_gender {
            p.set_ot_gender(if g == 1 { Gender::Female } else { Gender::Male });
        }
        changes.push("Dresseur d'origine : celui de l'échange".into());
        if e.fixed_nickname {
            if let Some((_, nick)) = e.nicknames.iter().find(|(l, _)| *l == lang).or_else(|| e.nicknames.iter().find(|(l, _)| *l == 3)) {
                let _ = p.set_nickname(nick);
            }
        }
    } else if p.ot_name() != trainer.name || p.tid() != trainer.tid || p.sid() != trainer.sid {
        let _ = p.set_ot_name(&trainer.name);
        p.set_tid(trainer.tid);
        p.set_sid(trainer.sid);
        p.set_ot_gender(trainer.gender);
        changes.push(format!("Dresseur d'origine : {} (ID {})", trainer.name, trainer.tid));
    }

    // Lieu, niveau et date de rencontre.
    let now = today();
    if e.is_egg() {
        p.set_egg_location(e.egg_location);
        if p.egg_date().is_none() {
            p.set_egg_date(Some(now));
        }
        if p.is_egg() {
            p.set_met_location(0);
            let _ = p.set_met_level(0);
            p.set_met_date(None);
        } else if met_replaced {
            p.set_met_location(30001);
            let _ = p.set_met_level(level);
        } else {
            let loc = if e.kind == EncounterKind::EggGift && e.location != 0 { e.location } else { hatch_location(plan.version) };
            p.set_met_location(loc);
            let _ = p.set_met_level(if origin_gen == 4 { 0 } else { 1 });
        }
        changes.push("Rencontre : œuf".into());
    } else {
        let met_level = if met_replaced {
            level
        } else if e.kind.is_wild() {
            level.clamp(e.level_min, e.level_max)
        } else if e.kind == EncounterKind::DreamRadar {
            ((level / 5) * 5).clamp(5, 40)
        } else {
            e.met_level.unwrap_or(e.level_min)
        };
        p.set_met_location(if met_replaced { 30001 } else { e.location });
        let _ = p.set_met_level(met_level);
        p.set_egg_location(e.egg_location);
        if e.egg_location == 0 {
            p.set_egg_date(None);
        }
        changes.push(format!(
            "Rencontre : {} à « {} », niveau {met_level}",
            e.kind.label().to_lowercase(),
            dex::location_name(origin_gen, e.location).unwrap_or("?")
        ));
    }
    if p.met_date().is_none() && !p.is_egg() {
        p.set_met_date(Some(now));
    }
    if p.fateful_encounter() != e.fateful {
        p.set_fateful_encounter(e.fateful);
        changes.push(if e.fateful { "Rencontre fatidique activée".into() } else { "Rencontre fatidique retirée".to_string() });
    }

    // Ball.
    let ball = if ball_allowed(e, plan.version, wishes.ball) {
        wishes.ball
    } else {
        e.ball.unwrap_or(4)
    };
    if ball != p.ball() {
        changes.push(format!("Ball : {}", dex::ball_name(ball).unwrap_or("?")));
    }
    p.set_ball(ball);

    // PID, IV, nature, sexe, talent, chromatique.
    let info = dex::personal(game, species, p.form());
    let ratio = info.as_ref().map_or(127, |i| i.gender_ratio);
    let ability_number = choose_ability(game, e, species, p.form(), wishes.ability_number);
    let shiny = match e.shiny {
        ShinyRule::Never => false,
        ShinyRule::Always => true,
        _ => wishes.shiny,
    };
    if shiny != wishes.shiny {
        changes.push(if shiny { "Chromatique imposé par la rencontre".into() } else { "Chromatique retiré (verrou chromatique)".to_string() });
    }
    let nature = e.nature.unwrap_or(wishes.nature);
    let gender_wish = match e.gender {
        Some(g) if g < 2 => g,
        _ => match wishes.gender {
            Gender::Male => 0,
            Gender::Female => 1,
            Gender::Genderless => 2,
        },
    };
    let (tid, sid) = (p.tid(), p.sid());
    let old_pid = p.pid();
    let old_ivs = p.ivs();
    let mut ivs = old_ivs;
    let ability_bit = if ability_number == 2 { 1 } else { 0 };
    // PID « d'origine » (Gen 3 à 5), avant un éventuel transfert vers la Gen 6+.
    let mut origin_pid: Option<u32> = None;
    if let Some(pid) = e.pid {
        origin_pid = Some(pid);
    } else if origin_gen == 4 && !e.is_egg() {
        if e.kind == EncounterKind::Pokewalker {
            origin_pid = Some(rng::pokewalker_pid(tid, sid, nature.min(23) as u32, gender_wish, ratio));
        } else {
            let mut wish = PidWish { nature: Some(nature), shiny: Some(shiny), ability_bit: Some(ability_bit), gender: Some((gender_wish, ratio)) };
            let mut found = rng::generate_method1(rand, tid, sid, wish);
            if found.is_none() {
                wish.gender = None;
                found = rng::generate_method1(rand, tid, sid, wish);
            }
            if found.is_none() {
                wish.ability_bit = None;
                found = rng::generate_method1(rand, tid, sid, wish);
            }
            if let Some((pid, iv32)) = found {
                origin_pid = Some(pid);
                ivs = ivs_from_iv32(iv32);
                if ivs != old_ivs {
                    changes.push("IV recalculés (méthode 1 : les IV découlent du PID)".into());
                }
            }
        }
    } else if origin_gen <= 5 {
        let wish = PidWish { nature: (origin_gen <= 4).then_some(nature), shiny: Some(shiny), ability_bit: (ability_number != 4).then_some(ability_bit), gender: Some((gender_wish, ratio)) };
        let wild_xor = origin_gen == 5
            && match e.kind {
                EncounterKind::HiddenGrotto => false,
                k if k.is_wild() => true,
                EncounterKind::Static => e.shiny == ShinyRule::Random && e.ball != Some(4) && e.ability != AbilityRule::OnlyHidden,
                _ => false,
            };
        let high = origin_gen == 5;
        let gender_target = match ratio {
            255 => 2,
            254 => 1,
            0 => 0,
            _ => gender_wish.min(1),
        };
        // Garde le PID actuel s'il convient déjà.
        let keep = old_pid != 0
            && (fgen <= 5)
            && verify::gender_from_pid(ratio, old_pid) == gender_target
            && ((((old_pid >> 16) ^ (old_pid & 0xFFFF) ^ tid as u32 ^ sid as u32) < 8) == shiny)
            && (!wild_xor || rng::gen5_xor_ok(old_pid, tid, sid))
            && (ability_number == 4 || (if high { (old_pid >> 16) & 1 } else { old_pid & 1 }) == ability_bit)
            && (origin_gen != 4 || old_pid % 25 == nature as u32);
        origin_pid = Some(if keep { old_pid } else { rng::generate_pid(rand, tid, sid, 8, wish, wild_xor, high) });
    }
    if let Some(fixed) = e.ivs {
        for i in 0..6 {
            if fixed[i] >= 0 {
                ivs[i] = fixed[i] as u8;
            }
        }
    }
    // IV parfaits garantis (Gen 6+).
    let mut flawless = e.flawless_ivs;
    if e.kind.is_wild() && origin_gen >= 6 && info.as_ref().is_some_and(|i| i.egg_groups[0] == 15) && !is_baby(e.species) {
        flawless = flawless.max(3);
    }
    if origin_gen >= 6 {
        let mut count = ivs.iter().filter(|&&v| v == 31).count() as u8;
        let mut order: Vec<usize> = (0..6).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(ivs[i]));
        for i in order {
            if count >= flawless {
                break;
            }
            if ivs[i] != 31 {
                ivs[i] = 31;
                count += 1;
            }
        }
    }
    if ivs != p.ivs() {
        let _ = p.set_ivs(ivs);
        if !changes.iter().any(|c| c.starts_with("IV")) {
            changes.push("IV ajustés pour la rencontre".into());
        }
    }

    match fgen {
        4 => {
            if let Some(pid) = origin_pid {
                if pid != old_pid {
                    changes.push("PID recalculé".into());
                }
                p.set_pid(pid);
            }
            p.set_gender(match verify::gender_from_pid(ratio, p.pid()) {
                0 => Gender::Male,
                1 => Gender::Female,
                _ => Gender::Genderless,
            });
        }
        5 => {
            if let Some(pid) = origin_pid {
                if pid != old_pid {
                    changes.push("PID recalculé".into());
                }
                p.set_pid(pid);
            }
            let n = if origin_gen <= 4 { (p.pid() % 25) as u8 } else { nature };
            let _ = p.set_nature(n);
            p.set_gender(match verify::gender_from_pid(ratio, p.pid()) {
                0 => Gender::Male,
                1 => Gender::Female,
                _ => Gender::Genderless,
            });
            p = set_gen5_hidden(&p, ability_number == 4, e.n_sparkle);
        }
        _ => {
            if let Some(opid) = origin_pid {
                // Transféré : EC = PID d'origine, PID recalculé (`PK5.GetTransferPID`).
                p.set_encryption_constant(opid);
                p.set_pid(rng::transfer_pid(opid, tid, sid));
                let n = if origin_gen <= 4 { (opid % 25) as u8 } else { nature };
                let _ = p.set_nature(n);
                p.set_gender(match verify::gender_from_pid(ratio, opid) {
                    0 => Gender::Male,
                    1 => Gender::Female,
                    _ => Gender::Genderless,
                });
                changes.push("PID et constante de chiffrement recalculés (transfert)".into());
            } else {
                let mut ec = p.encryption_constant();
                let mut pid = p.pid();
                let pid_shiny = ((pid >> 16) ^ (pid & 0xFFFF) ^ tid as u32 ^ sid as u32) < 16;
                if pid == 0 || pid_shiny != shiny {
                    pid = rng::generate_pid(rand, tid, sid, 16, PidWish { shiny: Some(shiny), ..Default::default() }, false, false);
                    changes.push("PID recalculé".into());
                }
                if ec == 0 || ec == pid {
                    ec = rand.next_u32();
                }
                p.set_encryption_constant(ec);
                p.set_pid(pid);
                let _ = p.set_nature(nature);
                let g = match ratio {
                    255 => Gender::Genderless,
                    254 => Gender::Female,
                    0 => Gender::Male,
                    _ if gender_wish == 1 => Gender::Female,
                    _ => Gender::Male,
                };
                p.set_gender(g);
            }
            let _ = p.set_ability_number(ability_number);
        }
    }
    if let Some(info) = &info {
        let idx = match ability_number {
            4 => 2,
            2 => 1,
            _ => 0,
        };
        let mut a = info.abilities[idx];
        if a == 0 {
            a = info.abilities[0];
        }
        if a != p.ability() {
            changes.push(format!("Talent : {}", dex::ability_name(a).unwrap_or("?")));
        }
        let _ = p.set_ability(a);
    }
    if fgen <= 5 && p.nature() != wishes.nature && e.nature.is_none() {
        changes.push(format!("Nature : {} (imposée par le PID)", dex::nature_name(p.nature()).unwrap_or("?")));
    }

    // Attaques.
    p = fix_moves(&p, game, Some(e), changes);

    // EV.
    let mut evs = p.evs();
    if p.is_egg() {
        evs = [0; 6];
    }
    if fgen >= 6 {
        for v in evs.iter_mut() {
            *v = (*v).min(252);
        }
    }
    let met_level_now = p.met_level();
    if fgen == 4 && verify::growth_level(&p, game) <= met_level_now {
        for v in evs.iter_mut() {
            *v = (*v).min(100) / 10 * 10;
        }
    }
    let mut excess = evs.iter().map(|&v| v as i32).sum::<i32>() - 510;
    for v in evs.iter_mut().rev() {
        if excess <= 0 {
            break;
        }
        let cut = excess.min(*v as i32);
        *v -= cut as u8;
        excess -= cut;
    }
    if evs != p.evs() {
        p.set_evs(evs);
        changes.push("EV ramenés dans les limites".into());
    }

    // Objet inconnu, Pokérus incohérent.
    let item = p.held_item();
    if item != 0 && (item > dex::max_item(game) || dex::item_name_in(game, item).is_none()) {
        p.set_held_item(0);
        changes.push("Objet inconnu retiré".into());
    }
    let (strain, days) = p.pokerus();
    if strain == 0 && days > 0 {
        p.set_pokerus(0, 0);
    }
    p.refresh_checksum();
    p
}

/// Garde les attaques légales, complète avec des attaques apprises par niveau, corrige
/// les PP et les attaques à réapprendre.
pub(crate) fn fix_moves(pk: &Pokemon, game: Game, enc: Option<&Encounter>, changes: &mut Vec<String>) -> Pokemon {
    let mut p = pk.clone();
    let Ok(ctx) = verify::context(pk, game) else { return p };
    let old = p.moves();
    let mut keep: Vec<u16> = Vec::new();
    let mut dropped: Vec<u16> = Vec::new();
    for &m in old.iter().filter(|&&m| m != 0) {
        if keep.contains(&m) {
            continue;
        }
        if verify::learn_sources(&ctx, enc, m).is_some() {
            keep.push(m);
        } else {
            dropped.push(m);
        }
    }
    // Suggestions : attaques de la rencontre, puis dernières attaques par niveau.
    let mut suggestions: Vec<u16> = Vec::new();
    if let Some(e) = enc {
        suggestions.extend(e.moves.iter().copied());
    }
    let level = ctx.level;
    suggestions.extend(learn::encounter_moves(game, p.species(), p.form(), level).into_iter().rev());
    if let Some(e) = enc {
        let og = ctx.origin_game().unwrap_or(game);
        suggestions.extend(learn::encounter_moves(og, e.species, e.form, e.level_min).into_iter().rev());
    }
    for m in suggestions {
        if keep.len() >= 4 {
            break;
        }
        if m != 0 && !keep.contains(&m) && verify::learn_sources(&ctx, enc, m).is_some() {
            keep.push(m);
        }
    }
    if keep.is_empty() {
        keep.push(33); // Charge
    }
    let mut moves = [0u16; 4];
    for (i, &m) in keep.iter().take(4).enumerate() {
        moves[i] = m;
    }
    if moves != old {
        if !dropped.is_empty() {
            let names: Vec<&str> = dropped.iter().map(|&m| dex::move_name(m).unwrap_or("?")).collect();
            changes.push(format!("Attaques impossibles retirées : {}", names.join(", ")));
        }
        let added: Vec<&str> = moves.iter().filter(|m| **m != 0 && !old.contains(m)).map(|&m| dex::move_name(m).unwrap_or("?")).collect();
        if !added.is_empty() {
            changes.push(format!("Attaques ajoutées : {}", added.join(", ")));
        }
    }
    p.set_moves(moves);
    // PP : maximum avec les PP Plus (3 au plus).
    let ups = p.pp_ups();
    let new_ups: [u8; 4] = std::array::from_fn(|i| if moves[i] == 0 { 0 } else { ups[i].min(3) });
    let pp: [u8; 4] = std::array::from_fn(|i| dex::move_info_in(game, moves[i]).map_or(0, |info| max_pp(info.pp, new_ups[i])));
    if p.pp() != pp || ups != new_ups {
        p.set_pp_ups(new_ups);
        p.set_pp(pp);
    }
    // Attaques à réapprendre.
    if p.format().generation() >= 6 {
        let mut relearn = [0u16; 4];
        if let Some(e) = enc {
            if ctx.origin_generation() >= 6 {
                if !e.relearn.is_empty() {
                    for (i, &m) in e.relearn.iter().take(4).enumerate() {
                        relearn[i] = m;
                    }
                } else if e.kind == EncounterKind::Egg {
                    let og = ctx.origin_game().unwrap_or(game);
                    let eggs = learn::egg_moves(og, e.species, e.form);
                    for (i, &m) in moves.iter().filter(|&&m| m != 0 && eggs.contains(&m)).enumerate() {
                        relearn[i] = m;
                    }
                }
            }
        }
        if verify::relearn_moves(&p) != relearn {
            p = set_relearn(&p, relearn);
            changes.push("Attaques à réapprendre corrigées".into());
        }
    }
    p.refresh_checksum();
    p
}

fn wishes_of(pk: &Pokemon) -> Wishes {
    let pid = pk.pid();
    let threshold = if pk.format().generation() >= 6 { 16 } else { 8 };
    Wishes {
        shiny: ((pid >> 16) ^ (pid & 0xFFFF) ^ pk.tid() as u32 ^ pk.sid() as u32) < threshold,
        nature: pk.nature().min(24),
        gender: pk.gender(),
        ability_number: pk.ability_number(),
        ball: pk.ball(),
    }
}

fn plan_for_current(pk: &Pokemon, game: Game) -> Option<Plan> {
    let ctx = verify::context(pk, game).ok()?;
    let (e, _) = verify::best_encounter(&ctx)?;
    Some(Plan { enc: e, version: pk.version() })
}

fn run(pk: &Pokemon, game: Game, trainer: &Trainer, plans: Vec<Plan>, wishes: Wishes) -> LegalizeOutcome {
    let mut rand = Rand::new(pk.encryption_constant() as u64 ^ (pk.pid() as u64) << 32 ^ 0xA11E_6A1E);
    let mut best: Option<LegalizeOutcome> = None;
    for plan in plans.into_iter().take(16) {
        let mut changes = Vec::new();
        let p = apply(pk, game, trainer, &plan, wishes, &mut rand, &mut changes);
        let report = analyze(&p, game);
        let ok = report.verdict != Verdict::Illegal;
        let better = best.as_ref().is_none_or(|b| report.errors < b.report.errors);
        if ok || better {
            let outcome = LegalizeOutcome { pokemon: p, changes, success: ok, report };
            if ok {
                return outcome;
            }
            best = Some(outcome);
        }
    }
    best.unwrap_or_else(|| LegalizeOutcome { pokemon: pk.clone(), changes: Vec::new(), report: analyze(pk, game), success: false })
}

/// « Rendre légal » : garde la rencontre actuelle si elle convient, sinon en choisit une
/// dans le jeu de la sauvegarde (ou un jeu plus ancien transférable).
pub fn legalize(pk: &Pokemon, game: Game, trainer: &Trainer) -> LegalizeOutcome {
    let before = analyze(pk, game);
    if before.verdict != Verdict::Illegal {
        return LegalizeOutcome { pokemon: pk.clone(), changes: Vec::new(), report: before, success: true };
    }
    // D'abord : corriger seulement les attaques / PP si c'est le seul problème.
    let only_moves = before.checks.iter().filter(|c| c.severity == verify::Severity::Invalid).all(|c| c.tab == Some("moves"));
    if only_moves {
        let mut changes = Vec::new();
        let enc = plan_for_current(pk, game).map(|p| p.enc);
        let p = fix_moves(pk, game, enc.as_ref(), &mut changes);
        let report = analyze(&p, game);
        if report.verdict != Verdict::Illegal {
            return LegalizeOutcome { pokemon: p, changes, report, success: true };
        }
    }
    let wishes = wishes_of(pk);
    let level = verify::growth_level(pk, game);
    let prefer = game_versions(game).first().copied().unwrap_or(0);
    let prefer = if encounters::version_game(pk.version()) == Some(game) { pk.version() } else { prefer };
    let mut list: Vec<Plan> = plan_for_current(pk, game).into_iter().collect();
    list.extend(plans(game, pk.species(), pk.form(), level, prefer));
    run(pk, game, trainer, list, wishes)
}

/// « Générer un Pokémon légal » : capture (ou éclosion) dans ce jeu, au niveau voulu,
/// par le dresseur de la sauvegarde. API publique utilisée par l'import Showdown.
pub fn generate_legal(game: Game, format: PkmFormat, trainer: &Trainer, req: &GenerateRequest) -> Result<LegalizeOutcome, String> {
    let species = req.species;
    if species == 0 || species > dex::max_species(game) || dex::species_name(species).is_none() {
        return Err(format!("l'espèce n°{species} n'existe pas dans ce jeu"));
    }
    let level = req.level.clamp(1, 100);
    let mut p = Pokemon::blank(format);
    let mut rand = Rand::from_clock();
    p.set_species(species);
    p.set_form(req.form).map_err(|e| e.to_string())?;
    let name = dex::species_name(species).unwrap_or("?");
    p.set_nickname(req.nickname.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(name)).map_err(|e| e.to_string())?;
    p.set_is_nicknamed(req.nickname.as_deref().is_some_and(|n| !n.trim().is_empty() && n != name));
    p.set_language(LANGUAGE_FR);
    p.set_ot_name(&trainer.name).map_err(|e| e.to_string())?;
    p.set_tid(trainer.tid);
    p.set_sid(trainer.sid);
    p.set_ot_gender(trainer.gender);
    if let Some(g) = growth(game, species, req.form) {
        p.set_exp(exp_for_level(g, level));
    }
    let info = dex::personal(game, species, req.form);
    p.set_friendship(info.as_ref().map_or(70, |i| i.base_friendship));
    p.set_encryption_constant(rand.next_u32());
    if format.generation() >= 6 {
        p.set_pid(rand.next_u32());
    }
    if let Some(ivs) = req.ivs {
        p.set_ivs(ivs.map(|v| v.min(31))).map_err(|e| e.to_string())?;
    } else {
        let ivs: [u8; 6] = std::array::from_fn(|_| rand.below(32) as u8);
        p.set_ivs(ivs).map_err(|e| e.to_string())?;
    }
    if let Some(evs) = req.evs {
        p.set_evs(evs);
    }
    if let Some(item) = req.held_item {
        p.set_held_item(item);
    }
    if let Some(moves) = req.moves {
        p.set_moves(moves);
    }
    let ratio = info.as_ref().map_or(127, |i| i.gender_ratio);
    let gender = req.gender.unwrap_or(match ratio {
        255 => Gender::Genderless,
        254 => Gender::Female,
        0 => Gender::Male,
        r if rand.below(254) < r as u32 => Gender::Female,
        _ => Gender::Male,
    });
    let wishes = Wishes {
        shiny: req.shiny.unwrap_or(false),
        nature: req.nature.unwrap_or_else(|| rand.below(25) as u8).min(24),
        gender,
        ability_number: req.ability_number.unwrap_or(1),
        ball: req.ball.unwrap_or(4),
    };
    let prefer = game_versions(game)[0];
    let mut list: Vec<Plan> = Vec::new();
    if let Some(i) = req.encounter_index {
        let eg = req.encounter_game.as_deref().and_then(super::db::game_from_id).unwrap_or(game);
        if eg.generation() <= game.generation() {
            if let Some(e) = encounters::encounters(eg).get(i) {
                let version = e.versions.first().copied().unwrap_or(prefer);
                list.push(Plan { enc: e.clone(), version });
            }
        }
    }
    list.extend(plans(game, species, req.form, level, prefer));
    if list.is_empty() {
        return Err(format!("{name} ne s'obtient pas dans ce jeu (ni par capture, ni par reproduction)"));
    }
    let mut out = run(&p, game, trainer, list, wishes);
    out.pokemon.refresh_checksum();
    Ok(out)
}

/// Type de corrélation PID / IV (exposé pour les tests et l'interface).
pub fn pid_type(pk: &Pokemon) -> PidType {
    let pid = pk.encryption_constant();
    rng::method_1_2_4(pid, verify::iv32(pk)).map_or(PidType::None, |(t, _)| t)
}

/// Requête légère de « Tout rendre légal » : vrai si la réécriture a réussi.
pub fn quick_legal(pk: &Pokemon, game: Game) -> bool {
    analyze(pk, game).verdict != Verdict::Illegal
}
