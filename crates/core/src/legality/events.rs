//! Distributions (Cadeaux Mystère) Gen 4 à 7 vues comme des rencontres : chaque carte de la
//! base de PKHeX lue par [`crate::gifts`] est convertie en [`Encounter`]
//! (`EncounterKind::Event`) pour réutiliser les vérifications des rencontres.

use std::sync::LazyLock;

use super::encounters::{AbilityRule, Encounter, EncounterKind, FixedTrainer, ShinyRule};
use crate::gifts::{self, Gift};
use crate::save::Gender;

fn versions_for(generation: u8, origin: u8) -> Vec<u8> {
    if origin != 0 {
        return vec![origin];
    }
    match generation {
        4 => vec![7, 8, 10, 11, 12],
        5 => vec![20, 21, 22, 23],
        6 => vec![24, 25, 26, 27],
        _ => vec![30, 31, 32, 33],
    }
}

fn gender_code(g: Gender) -> u8 {
    match g {
        Gender::Male => 0,
        Gender::Female => 1,
        Gender::Genderless => 2,
    }
}

fn convert(gift: &Gift) -> Option<Encounter> {
    let pk = gift.pokemon.as_ref()?;
    if pk.species == 0 {
        return None;
    }
    let generation = gift.generation();
    // Gen 5 : les restrictions de version de la base servent à la création, pas à la vérification.
    let versions =
        if generation != 5 && pk.origin_game == 0 && !gift.games.is_empty() { gift.games.clone() } else { versions_for(generation, pk.origin_game) };
    let level = pk.level.max(1);
    let met = if pk.met_level != 0 { pk.met_level } else { level };
    let location = if pk.egg { 0 } else { pk.met_location };
    let mut e = Encounter::base(EncounterKind::Event, generation, versions, pk.species, pk.form, level, level, location);
    e.egg_location = pk.egg_location;
    e.met_level = Some(met);
    e.ball = Some(pk.ball);
    e.held_item = pk.held_item;
    e.moves = pk.moves.iter().copied().filter(|&m| m != 0).collect();
    e.relearn = pk.relearn.iter().copied().filter(|&m| m != 0).collect();
    e.fateful = pk.fateful;
    e.nature = pk.nature;
    e.gender = pk.gender.map(gender_code);
    e.ability = match pk.ability {
        gifts::AbilityRule::Fixed(0) => AbilityRule::OnlyFirst,
        gifts::AbilityRule::Fixed(1) => AbilityRule::OnlySecond,
        gifts::AbilityRule::Fixed(_) => AbilityRule::OnlyHidden,
        gifts::AbilityRule::OneOrTwo => AbilityRule::Any12,
        gifts::AbilityRule::Any => AbilityRule::Any12H,
    };
    e.shiny = match (pk.pid, pk.shiny) {
        _ if pk.egg => ShinyRule::Random,
        (Some(pid), _) => {
            e.pid = Some(pid);
            ShinyRule::FixedPid
        }
        (None, gifts::ShinyRule::Never) => ShinyRule::Never,
        (None, gifts::ShinyRule::Random) => ShinyRule::Random,
        (None, _) => ShinyRule::Always,
    };
    if pk.ivs.iter().any(Option::is_some) {
        e.ivs = Some(pk.ivs.map(|v| v.map_or(-1, |x| x as i8)));
    }
    e.flawless_ivs = pk.perfect_ivs;
    if pk.language != 0 {
        e.language = Some(pk.language);
    }
    if let (Some(tid), false) = (pk.tid, pk.egg) {
        e.trainer = Some(FixedTrainer {
            tid,
            sid: pk.sid.unwrap_or(0),
            ot_gender: pk.ot_gender.map(gender_code),
            names: pk.ot_name.iter().map(|n| (pk.language, n.clone())).collect(),
        });
    }
    let title = gift.display_title();
    e.title = (!title.trim().is_empty()).then_some(title);
    Some(e)
}

fn build(generation: u8) -> Vec<Encounter> {
    gifts::database().iter().filter(|g| g.generation() == generation).filter_map(convert).collect()
}

static EVENTS: [LazyLock<Vec<Encounter>>; 4] =
    [LazyLock::new(|| build(4)), LazyLock::new(|| build(5)), LazyLock::new(|| build(6)), LazyLock::new(|| build(7))];

/// Distributions d'une génération.
pub fn events(generation: u8) -> &'static [Encounter] {
    match generation {
        4..=7 => &EVENTS[(generation - 4) as usize],
        _ => &[],
    }
}
