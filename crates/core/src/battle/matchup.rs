//! Duel détaillé (toutes les attaques d'un Pokémon sur un autre) et matrice
//! équipe × équipe (meilleure attaque de chaque côté, ordre d'action, verdict).

use serde::Serialize;

use super::calc::{calculate, effective_weather, speed, Damage, MoveCalc};
use super::ids::{ability as ab, item as it};
use super::ko::{ko_info, KoInfo};
use super::{types, Combatant, Field, SideState};
use crate::dex;

/// Une attaque calculée, avec ses dégâts normaux et critiques.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveLine {
    #[serde(flatten)]
    pub calc: MoveCalc,
    /// Dégâts minimum et maximum d'une utilisation (tous les coups).
    pub min: u16,
    pub max: u16,
    /// En % des PV max de la cible.
    pub min_percent: f32,
    pub max_percent: f32,
    pub crit_min: u16,
    pub crit_max: u16,
    pub crit_min_percent: f32,
    pub crit_max_percent: f32,
    /// Les 16 jets (somme des coups pour une attaque multi-coups).
    pub rolls: Vec<u16>,
    pub ko: KoInfo,
}

impl MoveLine {
    fn deals_damage(&self) -> bool {
        self.max > 0
    }
    fn average(&self) -> f32 {
        (self.min_percent + self.max_percent) / 2.0
    }
}

/// Toutes les attaques de `attacker` sur `defender`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Duel {
    pub attacker_speed: u32,
    pub defender_speed: u32,
    /// PV de la cible pris en compte (selon le % réglé) et PV max.
    pub defender_hp: u16,
    pub defender_max_hp: u16,
    pub moves: Vec<MoveLine>,
    /// Remarques (Restes, Ceinture Force, approximations…).
    pub notes: Vec<String>,
}

fn totals(damage: &Damage) -> (u16, u16, Vec<u16>) {
    match damage {
        Damage::None { .. } => (0, 0, Vec::new()),
        Damage::Fixed { amount } => (*amount, *amount, vec![*amount]),
        Damage::Rolls { hits } => {
            let rolls: Vec<u16> = (0..16).map(|i| hits.iter().map(|h| h[i] as u32).sum::<u32>().min(u16::MAX as u32) as u16).collect();
            (rolls[0], rolls[15], rolls)
        }
    }
}

fn pct(x: u16, max: u16) -> f32 {
    (x as f32 * 1000.0 / max.max(1) as f32).round() / 10.0
}

fn current_hp(c: &Combatant, side: &SideState) -> u16 {
    let max = c.stats[0].max(1) as u32;
    ((max * side.hp_percent.clamp(1, 100) as u32).div_ceil(100)).clamp(1, max) as u16
}

/// Calcule chaque attaque de `attacker` contre `defender`.
pub fn duel(
    game: dex::Game,
    attacker: &Combatant,
    att_side: &SideState,
    defender: &Combatant,
    def_side: &SideState,
    field: &Field,
) -> Duel {
    let generation = game.generation();
    let weather = effective_weather(field, attacker, defender);
    let max_hp = defender.stats[0].max(1);
    let hp = current_hp(defender, def_side);
    let full = hp == max_hp;
    let endure = full && (defender.item == it::FOCUS_SASH || (generation >= 5 && defender.ability == ab::STURDY));

    let moves = attacker
        .moves
        .iter()
        .filter(|&&m| m != 0)
        .filter_map(|&m| {
            let calc = calculate(game, attacker, att_side, defender, def_side, m, field, false)?;
            let crit = calculate(game, attacker, att_side, defender, def_side, m, field, true)?;
            let (min, max, rolls) = totals(&calc.damage);
            let (crit_min, crit_max, _) = totals(&crit.damage);
            let ko = ko_info(&calc.damage, hp, endure);
            Some(MoveLine {
                min,
                max,
                min_percent: pct(min, max_hp),
                max_percent: pct(max, max_hp),
                crit_min,
                crit_max,
                crit_min_percent: pct(crit_min, max_hp),
                crit_max_percent: pct(crit_max, max_hp),
                rolls,
                ko,
                calc,
            })
        })
        .collect();

    let mut notes = defender.notes.clone();
    let item = dex::item_name(defender.item).unwrap_or_default();
    if defender.item == it::LEFTOVERS || (defender.item == it::BLACK_SLUDGE && defender.types.contains(&types::POISON)) {
        notes.push(format!("{item} : la cible récupère 1/16 de ses PV à chaque tour (non compté dans les K.O.)."));
    } else if defender.item == it::SITRUS_BERRY {
        let heal = if generation >= 4 { "1/4" } else { "30 PV" };
        notes.push(format!("{item} : la cible récupère {heal} de ses PV sous la moitié (non compté)."));
    }
    if endure {
        let what = if defender.item == it::FOCUS_SASH {
            item.to_string()
        } else {
            dex::ability_name(ab::STURDY).unwrap_or("Fermeté").to_string()
        };
        notes.push(format!("{what} : la cible survit au premier coup avec 1 PV si elle a tous ses PV (compté)."));
    }
    Duel {
        attacker_speed: speed(generation, attacker, att_side, weather),
        defender_speed: speed(generation, defender, def_side, weather),
        defender_hp: hp,
        defender_max_hp: max_hp,
        moves,
        notes,
    }
}

/// Meilleure attaque d'un côté, résumée pour une case de la matrice.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestMove {
    pub move_id: u16,
    pub name: String,
    pub type_id: u8,
    pub priority: i8,
    pub min_percent: f32,
    pub max_percent: f32,
    pub ko: KoInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Je gagne même dans le pire des cas (jets faibles pour moi, forts pour lui).
    Win,
    /// Ça dépend des jets (ou de l'égalité de Vitesse).
    Uncertain,
    /// Je perds même dans le meilleur des cas.
    Lose,
    /// Aucun des deux ne peut infliger de dégâts.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    Me,
    Them,
    Tie,
}

/// Une case de la matrice : mon Pokémon (ligne) contre le sien (colonne).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub mine: Option<BestMove>,
    pub theirs: Option<BestMove>,
    pub my_speed: u32,
    pub their_speed: u32,
    /// Qui agit en premier avec ces attaques (priorité, puis Vitesse).
    pub first: Order,
    pub verdict: Verdict,
}

fn best(duel: &Duel) -> Option<BestMove> {
    let key = |l: &MoveLine| (if l.ko.hits == 0 { u8::MAX } else { l.ko.hits }, -(l.ko.chance * 1e6) as i64, -(l.average() * 10.0) as i64);
    duel.moves.iter().filter(|l| l.deals_damage()).min_by_key(|l| key(l)).map(|l| BestMove {
        move_id: l.calc.move_id,
        name: l.calc.name.clone(),
        type_id: l.calc.type_id,
        priority: l.calc.priority,
        min_percent: l.min_percent,
        max_percent: l.max_percent,
        ko: l.ko.clone(),
    })
}

/// Nombre de coups : possible (meilleurs jets) et assuré ; `u8::MAX` = jamais.
fn race(b: &Option<BestMove>) -> (u8, u8) {
    match b {
        Some(b) if b.ko.hits > 0 => (b.ko.hits, b.ko.guaranteed.unwrap_or(u8::MAX)),
        _ => (u8::MAX, u8::MAX),
    }
}

/// Je gagne la course au K.O. si j'ai besoin de moins de coups (ou autant en agissant en premier).
fn wins(me_first: bool, mine: u8, theirs: u8) -> bool {
    mine != u8::MAX && if me_first { mine <= theirs } else { mine < theirs }
}

/// Une case de la matrice.
pub fn cell(game: dex::Game, me: &Combatant, my_side: &SideState, them: &Combatant, their_side: &SideState, field: &Field) -> Cell {
    let attack = duel(game, me, my_side, them, their_side, field);
    let defense = duel(game, them, their_side, me, my_side, field);
    let (mine, theirs) = (best(&attack), best(&defense));
    let (my_speed, their_speed) = (attack.attacker_speed, attack.defender_speed);
    let (my_prio, their_prio) = (mine.as_ref().map_or(0, |b| b.priority), theirs.as_ref().map_or(0, |b| b.priority));
    let first = match my_prio.cmp(&their_prio).then(my_speed.cmp(&their_speed)) {
        std::cmp::Ordering::Greater => Order::Me,
        std::cmp::Ordering::Less => Order::Them,
        std::cmp::Ordering::Equal => Order::Tie,
    };
    let (my_n, my_g) = race(&mine);
    let (their_n, their_g) = race(&theirs);
    let orders: &[bool] = match first {
        Order::Me => &[true],
        Order::Them => &[false],
        Order::Tie => &[true, false],
    };
    let verdict = if my_n == u8::MAX && their_n == u8::MAX {
        Verdict::None
    } else if orders.iter().all(|&o| wins(o, my_g, their_n)) {
        Verdict::Win
    } else if orders.iter().any(|&o| wins(o, my_n, their_g)) {
        Verdict::Uncertain
    } else {
        Verdict::Lose
    };
    Cell { mine, theirs, my_speed, their_speed, first, verdict }
}

/// Matrice : une ligne par Pokémon de mon équipe, une colonne par Pokémon adverse.
pub fn matrix(
    game: dex::Game,
    mine: &[Combatant],
    my_side: &SideState,
    theirs: &[Combatant],
    their_side: &SideState,
    field: &Field,
) -> Vec<Vec<Cell>> {
    mine.iter().map(|me| theirs.iter().map(|them| cell(game, me, my_side, them, their_side, field)).collect()).collect()
}
