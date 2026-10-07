//! Mode Nuzlocke : croise la ROM (souvent randomisée par Kaleido) et la sauvegarde
//! pour suivre un défi Nuzlocke : première rencontre par route, morts, doublons,
//! niveau maximum avant chaque champion, et règles enfreintes.
//!
//! - [`rom::read`] lit les routes (rencontres groupées par lieu), les champions et
//!   les familles d'évolution dans la ROM ;
//! - [`RunState`] garde les choix de l'utilisateur (règles, marques manuelles,
//!   boîte « Cimetière », ROM liée) dans `<sauvegarde>.nuzlocke.json` ;
//! - [`report`] calcule le bilan à partir de la sauvegarde ouverte.
//!
//! Pris en charge : Platine, Noire, Blanche (Gen 4 et 5, où le lieu de rencontre
//! du Pokémon est l'identifiant de lieu des cartes de la ROM), et les jeux 3DS
//! ([`rom_ctr`] : X / Y, ROSA, Soleil / Lune, Ultra, où c'est le lieu des zones).

pub mod rom;
pub mod rom_ctr;
pub mod rom_gba;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use rom::{Encounter, Leader, LeaderKind, RomInfo, Route};

use crate::dex;
use crate::games::Game;
use crate::save::session::{SaveSession, Slot};
use crate::save::{PkmDate, SaveError, SaveVersion};

#[derive(Debug, thiserror::Error)]
pub enum NuzlockeError {
    #[error(transparent)]
    Save(#[from] SaveError),
    #[error("{0}")]
    Mismatch(String),
}

/// Règles choisies pour la partie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rules {
    /// Une seule capture par route (la première rencontre).
    pub one_per_route: bool,
    /// Clause doublons : une espèce (famille) déjà capturée ne compte pas.
    pub dupes_clause: bool,
    /// Clause chromatique : un chromatique peut toujours être capturé, en plus.
    pub shiny_clause: bool,
    /// Clause surnom : chaque Pokémon capturé doit avoir un surnom.
    pub nickname_clause: bool,
    /// Niveau maximum : pas de Pokémon au-dessus du meilleur Pokémon du prochain champion.
    pub level_caps: bool,
    /// Pas d'objets de soin en combat (rappel, non vérifiable dans la sauvegarde).
    pub no_items_in_battle: bool,
    /// Mode « Set » : pas de changement de Pokémon après un K.O. adverse (rappel).
    pub set_mode: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            one_per_route: true,
            dupes_clause: true,
            shiny_clause: true,
            nickname_clause: true,
            level_caps: true,
            no_items_in_battle: false,
            set_mode: false,
        }
    }
}

/// État d'une partie Nuzlocke, enregistré à côté de la sauvegarde.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunState {
    /// ROM (randomisée) liée à cette sauvegarde.
    pub rom_path: Option<String>,
    pub rules: Rules,
    /// Boîte « Cimetière » choisie ; `None` = détection par le nom (RIP, Cimetière, Morts…).
    pub graveyard_box: Option<usize>,
    /// Routes marquées « ratée » (première rencontre perdue : fuite, K.O.…).
    pub missed: BTreeSet<String>,
    /// Pokémon déclarés morts à la main (clé : voir [`mon_key`]).
    pub dead: BTreeSet<String>,
    /// Pokémon déclarés vivants malgré des PV à 0 ou leur place au cimetière.
    pub alive: BTreeSet<String>,
    /// Morts détectées par le compagnon de partie en comparant les sauvegardes
    /// (K.O. puis déposé en boîte ou relâché), avec leur cause.
    pub auto_dead: BTreeMap<String, String>,
    /// Lieux rattachés à la main (identifiant de lieu de rencontre → clé de route) : étages de
    /// grotte, Parc Safari, lieux sans rencontres de la ROM… Une clé vide = « pas une route »
    /// (cadeau, rencontre fixe) : le lieu n'est plus proposé.
    pub location_routes: BTreeMap<u16, String>,
    /// Nombre de badges forcé (sinon lu dans la sauvegarde).
    pub badges: Option<u8>,
}

/// Fichier d'état d'une sauvegarde : `<sauvegarde>.nuzlocke.json`.
pub fn state_path(save: &Path) -> PathBuf {
    let mut name = save.file_name().unwrap_or_default().to_os_string();
    name.push(".nuzlocke.json");
    save.with_file_name(name)
}

/// État enregistré (ou état par défaut si absent ou illisible).
pub fn load_state(save: &Path) -> RunState {
    std::fs::read(state_path(save)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn store_state(save: &Path, state: &RunState) -> std::io::Result<()> {
    let json = serde_json::to_vec_pretty(state).map_err(std::io::Error::other)?;
    std::fs::write(state_path(save), json)
}

/// Identifiant stable d'un Pokémon (le PID ne change pas à l'évolution).
pub fn mon_key(pid: u32) -> String {
    format!("{pid:08X}")
}

/// Comment le Pokémon a été obtenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// Capturé sur une route de la liste.
    Route,
    Starter,
    Egg,
    Trade,
    Event,
    /// Lieu sans rencontres sauvages : cadeau ou rencontre fixe.
    Gift,
}

/// Rôle d'une capture sur sa route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatchKind {
    /// La capture qui compte pour la route.
    Counted,
    /// Famille déjà capturée (clause doublons) : ne compte pas.
    Dupe,
    /// Chromatique en plus (clause chromatique).
    ShinyBonus,
    /// Capture de trop sur la route.
    Extra,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonView {
    pub key: String,
    pub slot: Slot,
    pub species: u16,
    pub species_name: String,
    pub nickname: String,
    pub is_nicknamed: bool,
    pub level: u8,
    pub shiny: bool,
    /// « Équipe » ou nom de la boîte.
    pub place: String,
    pub met_location: u16,
    pub met_location_name: Option<String>,
    pub met_level: u8,
    pub met_date: Option<PkmDate>,
    pub origin: Origin,
    pub route: Option<String>,
    pub catch: Option<CatchKind>,
    pub dead: bool,
    /// Pourquoi il est compté mort : « Rangé au cimetière », « Marqué mort à la main », ou la
    /// cause relevée par le compagnon (« K.O. puis déposé en boîte »…).
    pub death_cause: Option<String>,
    /// K.O. dans l'équipe mais pas encore compté mort : un Rappel peut encore le sauver,
    /// il le sera s'il est déposé en boîte ou relâché dans cet état.
    pub fainted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteStatus {
    /// Rien encore.
    Pending,
    Caught,
    /// Marquée ratée à la main.
    Missed,
    /// Seulement des doublons capturés : la route reste ouverte.
    DupeOnly,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterView {
    #[serde(flatten)]
    pub encounter: Encounter,
    pub name: String,
    /// Famille déjà capturée (doublon si rencontré).
    pub owned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteView {
    pub key: String,
    pub name: String,
    pub order: u32,
    pub location_ids: Vec<u16>,
    pub encounters: Vec<EncounterView>,
    pub status: RouteStatus,
    pub capture: Option<MonView>,
    /// Autres captures sur la route (doublons, chromatiques, captures de trop).
    pub others: Vec<MonView>,
    pub marked_missed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapView {
    #[serde(flatten)]
    pub leader: Leader,
    pub beaten: bool,
    pub current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub severity: Severity,
    /// Règle concernée : `levelCap`, `nickname`, `onePerRoute`, `dupes`, `dead`.
    pub rule: &'static str,
    pub title: String,
    pub detail: String,
    pub mon: Option<String>,
    pub route: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub routes: usize,
    pub routes_caught: usize,
    pub routes_missed: usize,
    pub captures: usize,
    pub alive: usize,
    pub dead: usize,
    pub badges: u8,
    /// `true` si le nombre de badges vient de la sauvegarde (sinon forcé à la main).
    pub badges_from_save: bool,
    pub level_cap: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graveyard {
    pub index: usize,
    pub name: String,
    /// Trouvée d'après son nom (et non choisie).
    pub auto: bool,
}

/// Lieu de capture qui ne correspond à aucune route : à rattacher (ou confirmer) une fois.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unassigned {
    pub location: u16,
    pub name: String,
    /// Pokémon capturés là.
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub game: Game,
    pub game_name: &'static str,
    pub seed: Option<u64>,
    pub stats: Stats,
    pub caps: Vec<CapView>,
    pub routes: Vec<RouteView>,
    /// Pokémon morts (cimetière, K.O., marques).
    pub graveyard: Vec<MonView>,
    pub graveyard_box: Option<Graveyard>,
    /// Équipe actuelle, avec l'état de chaque Pokémon.
    pub party: Vec<MonView>,
    /// Pokémon obtenus hors des routes (starter, œufs, échanges, cadeaux).
    pub others: Vec<MonView>,
    pub violations: Vec<Violation>,
    /// Lieux de capture à rattacher à une route.
    pub unassigned: Vec<Unassigned>,
    pub rules: Rules,
    pub box_names: Vec<String>,
}

/// Le jeu de la ROM correspond-il à la sauvegarde ?
pub fn compatible(game: Game, version: SaveVersion) -> bool {
    matches!(
        (game, version),
        (Game::Ruby | Game::Sapphire, SaveVersion::RubySapphire)
            | (Game::Emerald, SaveVersion::Emerald)
            | (Game::FireRed | Game::LeafGreen, SaveVersion::FireRedLeafGreen)
            | (Game::Diamond | Game::Pearl, SaveVersion::DiamondPearl)
            | (Game::Platinum, SaveVersion::Platinum)
            | (Game::HeartGold | Game::SoulSilver, SaveVersion::HeartGoldSoulSilver)
            | (Game::Black | Game::White, SaveVersion::BlackWhite)
            | (Game::Black2 | Game::White2, SaveVersion::Black2White2)
            | (Game::X | Game::Y, SaveVersion::XY)
            | (Game::OmegaRuby | Game::AlphaSapphire, SaveVersion::OmegaRubyAlphaSapphire)
            | (Game::Sun | Game::Moon, SaveVersion::SunMoon)
            | (Game::UltraSun | Game::UltraMoon, SaveVersion::UltraSunUltraMoon)
    )
}

/// Données d'un Pokémon utiles au bilan.
#[derive(Debug, Clone)]
struct Mon {
    view: MonView,
    is_egg: bool,
    egg_location: u16,
    fateful: bool,
    tid: u16,
    sid: u16,
    /// PV actuels si le Pokémon a des statistiques d'équipe.
    hp: Option<u16>,
    in_party: bool,
    box_index: Option<usize>,
    /// Ordre de lecture, pour départager deux captures du même jour.
    seq: usize,
}

fn graveyard_by_name(names: &[String]) -> Option<usize> {
    names.iter().position(|n| {
        let lower = n.to_lowercase();
        lower.split(|c: char| !c.is_alphanumeric()).any(|w| w == "rip" || w == "mort" || w == "morts" || w == "dead" || w == "grave")
            || lower.contains("cimeti")
            || lower.contains("graveyard")
            || lower.contains("tombe")
    })
}

fn read_mons(session: &SaveSession, box_names: &[String]) -> Result<Vec<Mon>, SaveError> {
    let gen = session.save.generation();
    let mut out = Vec::new();
    for v in session.all()? {
        let p = session.get(v.slot)?;
        let (in_party, box_index, place) = match v.slot {
            Slot::Party { .. } => (true, None, "Équipe".to_string()),
            Slot::Box { r#box, .. } => (false, Some(r#box), box_names.get(r#box).cloned().unwrap_or_else(|| format!("Boîte {}", r#box + 1))),
        };
        let hp = p.as_ref().filter(|p| p.party_stats().is_some()).map(|p| p.current_hp());
        let s = &v.summary;
        let d = &v.details;
        out.push(Mon {
            view: MonView {
                key: mon_key(s.pid),
                slot: v.slot,
                species: s.species,
                species_name: v.species_name.clone(),
                nickname: s.nickname.clone(),
                is_nicknamed: s.is_nicknamed,
                level: s.level,
                shiny: s.shiny,
                place,
                met_location: d.met_location,
                met_location_name: dex::location_name(gen, d.met_location).map(str::to_string),
                met_level: d.met_level,
                met_date: d.met_date,
                origin: Origin::Gift,
                route: None,
                catch: None,
                dead: false,
                death_cause: None,
                fainted: false,
            },
            is_egg: s.is_egg,
            egg_location: d.egg_location,
            fateful: d.fateful_encounter,
            tid: s.tid,
            sid: s.sid,
            hp,
            in_party,
            box_index,
            seq: out.len(),
        });
    }
    Ok(out)
}

/// Bilan de la partie : routes, captures, morts, niveau maximum, règles enfreintes.
pub fn report(rom: &RomInfo, session: &SaveSession, state: &RunState) -> Result<Report, NuzlockeError> {
    let version = session.save.version();
    if !compatible(rom.game, version) {
        return Err(NuzlockeError::Mismatch(format!("la ROM ({}) ne correspond pas à la sauvegarde ({})", rom.game.name_fr(), version.label())));
    }
    let view = session.view()?;
    let box_names = view.box_names.clone();
    let mons = read_mons(session, &box_names)?;
    let mut save_badges = session.save.badges();
    if rom.game.generation() == 7 {
        // Alola : îles terminées, converties en capitaines / doyens battus.
        save_badges = save_badges.map(|islands| rom_ctr::alola_badges(rom, islands));
    }
    Ok(compute(rom, &view.trainer, save_badges, &box_names, mons, state))
}

fn compute(
    rom: &RomInfo,
    trainer: &crate::save::Trainer,
    save_badges: Option<u8>,
    box_names: &[String],
    mut mons: Vec<Mon>,
    state: &RunState,
) -> Report {
    let rules = &state.rules;
    let route_of: HashMap<u16, usize> = rom.routes.iter().enumerate().flat_map(|(i, r)| r.location_ids.iter().map(move |&l| (l, i))).collect();
    let starter_families: HashSet<u16> = rom.starters.iter().map(|&s| rom.family_of(s)).collect();

    // --- Cimetière et morts.
    let graveyard_box = match state.graveyard_box {
        Some(i) if i < box_names.len() => Some(Graveyard { index: i, name: box_names[i].clone(), auto: false }),
        Some(_) => None,
        None => graveyard_by_name(box_names).map(|i| Graveyard { index: i, name: box_names[i].clone(), auto: true }),
    };
    mons.retain(|m| !m.is_egg);
    for m in &mut mons {
        // Origine.
        let traded = m.tid != trainer.tid || m.sid != trainer.sid;
        let family = rom.family_of(m.view.species);
        m.view.origin = if traded {
            Origin::Trade
        } else if m.egg_location != 0 {
            Origin::Egg
        } else if m.fateful {
            Origin::Event
        } else if starter_families.contains(&family) && m.view.met_level <= 5 {
            Origin::Starter
        } else if let Some(key) = state.location_routes.get(&m.view.met_location) {
            // Choix de l'utilisateur, prioritaire sur le regroupement par nom de lieu.
            match rom.routes.iter().find(|r| &r.key == key) {
                Some(r) => {
                    m.view.route = Some(r.key.clone());
                    Origin::Route
                }
                None => Origin::Gift,
            }
        } else if let Some(&r) = route_of.get(&m.view.met_location) {
            m.view.route = Some(rom.routes[r].key.clone());
            Origin::Route
        } else {
            Origin::Gift
        };
        // Mort ?
        let key = &m.view.key;
        let cause = if state.alive.contains(key) {
            None
        } else if state.dead.contains(key) {
            Some("Marqué mort à la main")
        } else if let Some(c) = state.auto_dead.get(key) {
            Some(c.as_str())
        } else if graveyard_box.as_ref().is_some_and(|g| m.box_index == Some(g.index)) {
            Some("Rangé au cimetière")
        } else {
            None
        };
        m.view.dead = cause.is_some();
        m.view.death_cause = cause.map(str::to_string);
        // Décision du mode Nuzlocke automatique : un K.O. en équipe n'est qu'un avertissement.
        m.view.fainted = !m.view.dead && m.in_party && m.hp == Some(0) && !state.alive.contains(key);
    }

    // --- Captures par route, dans l'ordre chronologique.
    let mut order: Vec<usize> = (0..mons.len()).filter(|&i| mons[i].view.origin == Origin::Route).collect();
    let route_order = |m: &Mon| m.view.route.as_ref().and_then(|k| rom.routes.iter().find(|r| &r.key == k)).map_or(u32::MAX, |r| r.order);
    order.sort_by_key(|&i| {
        let m = &mons[i];
        let date = m.view.met_date.map_or((u16::MAX, 0, 0), |d| (d.year, d.month, d.day));
        (date, route_order(m), m.seq)
    });
    let mut owned: HashSet<u16> = mons.iter().filter(|m| m.view.origin == Origin::Starter).map(|m| rom.family_of(m.view.species)).collect();
    let mut counted: HashSet<String> = HashSet::new();
    for i in order {
        let route = mons[i].view.route.clone().unwrap_or_default();
        let family = rom.family_of(mons[i].view.species);
        let kind = if counted.contains(&route) {
            if rules.shiny_clause && mons[i].view.shiny {
                CatchKind::ShinyBonus
            } else {
                CatchKind::Extra
            }
        } else if rules.shiny_clause && mons[i].view.shiny && rules.dupes_clause && owned.contains(&family) {
            CatchKind::ShinyBonus
        } else if rules.dupes_clause && owned.contains(&family) {
            CatchKind::Dupe
        } else {
            counted.insert(route);
            owned.insert(family);
            CatchKind::Counted
        };
        mons[i].view.catch = Some(kind);
    }

    // --- Niveaux maximum.
    let badges_from_save = state.badges.is_none() && save_badges.is_some();
    let badges = state.badges.or(save_badges.map(|b| b.count_ones() as u8)).unwrap_or(0).min(8);
    let gyms: Vec<usize> = (0..rom.leaders.len()).filter(|&i| rom.leaders[i].kind == LeaderKind::Gym).collect();
    let elites: Vec<usize> = (0..rom.leaders.len()).filter(|&i| rom.leaders[i].kind == LeaderKind::Elite).collect();
    let (current, level_cap) = match gyms.get(badges as usize) {
        Some(&g) => (vec![g], Some(rom.leaders[g].ace_level)),
        None if !elites.is_empty() => (elites.clone(), elites.iter().map(|&e| rom.leaders[e].ace_level).max()),
        None => (Vec::new(), None),
    };
    let caps: Vec<CapView> = rom
        .leaders
        .iter()
        .enumerate()
        .map(|(i, l)| CapView {
            leader: l.clone(),
            beaten: gyms.iter().position(|&g| g == i).is_some_and(|n| n < badges as usize),
            current: current.contains(&i),
        })
        .collect();
    let level_cap = level_cap.filter(|&c| c > 0);

    // --- Règles enfreintes.
    let mut violations = Vec::new();
    let mut push = |severity, rule, title: String, detail: String, m: &MonView| {
        violations.push(Violation { severity, rule, title, detail, mon: Some(m.key.clone()), route: m.route.clone() })
    };
    let name = |m: &MonView| {
        if m.is_nicknamed && m.nickname != m.species_name {
            format!("{} ({})", m.nickname, m.species_name)
        } else {
            m.species_name.clone()
        }
    };
    let cap_leader = current.first().map(|&i| &rom.leaders[i]);
    for m in &mons {
        let v = &m.view;
        if m.in_party && v.dead {
            push(
                Severity::Error,
                "dead",
                format!("{} est mort mais encore dans l'équipe", name(v)),
                "Un Pokémon mort ne doit plus combattre : range-le dans le cimetière.".into(),
                v,
            );
        }
        if v.fainted {
            push(
                Severity::Warning,
                "fainted",
                format!("{} est K.O. : à déposer en boîte", name(v)),
                "En Nuzlocke, un Pokémon K.O. est perdu. Dépose-le dans une boîte : Kaleido le comptera mort à la sauvegarde suivante. S'il a été ranimé par erreur avec un Rappel, marque-le vivant.".into(),
                v,
            );
        }
        if rules.level_caps && m.in_party && !v.dead {
            if let (Some(cap), Some(l)) = (level_cap, cap_leader) {
                if v.level > cap {
                    push(
                        Severity::Error,
                        "levelCap",
                        format!("{} dépasse le niveau maximum", name(v)),
                        format!("Niveau {} pour un maximum de {cap} ({}, {}).", v.level, l.name, l.label),
                        v,
                    );
                }
            }
        }
        if rules.nickname_clause && v.origin == Origin::Route && v.catch == Some(CatchKind::Counted) && !v.is_nicknamed {
            push(
                Severity::Warning,
                "nickname",
                format!("{} n'a pas de surnom", v.species_name),
                "Clause surnom : chaque Pokémon capturé doit être surnommé.".into(),
                v,
            );
        }
        if rules.one_per_route && v.catch == Some(CatchKind::Extra) {
            push(
                Severity::Error,
                "onePerRoute",
                format!("Deuxième capture : {}", v.route.as_deref().unwrap_or("?")),
                format!("{} a été capturé là où une capture compte déjà. Il ne devrait pas être utilisé.", name(v)),
                v,
            );
        }
        if rules.dupes_clause && v.catch == Some(CatchKind::Dupe) {
            push(
                Severity::Info,
                "dupes",
                format!("Doublon : {}", name(v)),
                format!("Sa famille est déjà capturée : il ne compte pas pour {}. La route reste ouverte.", v.route.as_deref().unwrap_or("la route")),
                v,
            );
        }
    }

    // --- Routes.
    let views: Vec<RouteView> = rom
        .routes
        .iter()
        .map(|r| {
            let here: Vec<&Mon> = mons.iter().filter(|m| m.view.route.as_deref() == Some(r.key.as_str())).collect();
            let capture = here.iter().find(|m| m.view.catch == Some(CatchKind::Counted)).map(|m| m.view.clone());
            let others: Vec<MonView> = here.iter().filter(|m| m.view.catch != Some(CatchKind::Counted)).map(|m| m.view.clone()).collect();
            let marked_missed = state.missed.contains(&r.key);
            let status = match (&capture, marked_missed) {
                (Some(_), _) => RouteStatus::Caught,
                (None, true) => RouteStatus::Missed,
                (None, false) if !others.is_empty() => RouteStatus::DupeOnly,
                _ => RouteStatus::Pending,
            };
            let encounters = r
                .encounters
                .iter()
                .map(|e| EncounterView {
                    encounter: e.clone(),
                    name: dex::species_name(e.species).map_or_else(|| format!("n°{}", e.species), str::to_string),
                    owned: owned.contains(&rom.family_of(e.species)),
                })
                .collect();
            RouteView {
                key: r.key.clone(),
                name: r.name.clone(),
                order: r.order,
                location_ids: r.location_ids.clone(),
                encounters,
                status,
                capture,
                others,
                marked_missed,
            }
        })
        .collect();

    let run_mons = || mons.iter().filter(|m| m.view.origin != Origin::Trade);
    let stats = Stats {
        routes: views.len(),
        routes_caught: views.iter().filter(|r| r.status == RouteStatus::Caught).count(),
        routes_missed: views.iter().filter(|r| r.status == RouteStatus::Missed).count(),
        captures: mons.iter().filter(|m| m.view.catch == Some(CatchKind::Counted)).count(),
        alive: run_mons().filter(|m| !m.view.dead).count(),
        dead: mons.iter().filter(|m| m.view.dead).count(),
        badges,
        badges_from_save,
        level_cap,
    };
    violations.sort_by_key(|v| v.severity as u8);
    let mut unassigned: Vec<Unassigned> = Vec::new();
    for m in mons.iter().filter(|m| m.view.origin == Origin::Gift && m.view.met_location != 0 && !state.location_routes.contains_key(&m.view.met_location)) {
        match unassigned.iter_mut().find(|u| u.location == m.view.met_location) {
            Some(u) => u.count += 1,
            None => unassigned.push(Unassigned {
                location: m.view.met_location,
                name: m.view.met_location_name.clone().unwrap_or_else(|| format!("Lieu n°{}", m.view.met_location)),
                count: 1,
            }),
        }
    }
    Report {
        game: rom.game,
        game_name: rom.game.name_fr(),
        seed: rom.seed,
        stats,
        caps,
        routes: views,
        graveyard: mons.iter().filter(|m| m.view.dead).map(|m| m.view.clone()).collect(),
        graveyard_box,
        party: mons.iter().filter(|m| m.in_party).map(|m| m.view.clone()).collect(),
        others: mons.iter().filter(|m| m.view.origin != Origin::Route).map(|m| m.view.clone()).collect(),
        violations,
        unassigned,
        rules: rules.clone(),
        box_names: box_names.to_vec(),
    }
}
