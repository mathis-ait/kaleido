//! Vérifications de légalité d'un Pokémon (d'après les « Verifiers » de PKHeX,
//! simplifiés) : rencontre, niveau, attaques, talent, Ball, sexe, nature, PID / IV,
//! chromatique, dresseur, EV, PP, objet, langue, rencontre fatidique.

use serde::Serialize;

use super::encounters::{self, version_game, version_generation, AbilityRule, Encounter, EncounterKind, ShinyRule};
use super::events;
use super::evolution::{self, Stage};
use super::learn::{self, LearnQuery};
use super::rng::{self, PidType};
use crate::dex::{self, Game, PersonalInfo};
use crate::save::pkm::PkmDate;
use crate::save::{exp_for_level, Gender, PkmFormat, Pokemon};

/// Gravité d'un résultat (Valide / Douteux / Illégal dans PKHeX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Valid,
    Fishy,
    Invalid,
}

/// Verdict global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Legal,
    Fishy,
    Illegal,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Legal => "Légal",
            Verdict::Fishy => "Douteux",
            Verdict::Illegal => "Illégal",
        }
    }
}

/// Un résultat de vérification.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub severity: Severity,
    /// Code stable (tests, regroupements).
    pub code: &'static str,
    pub title: String,
    pub detail: String,
    /// Onglet de la fiche où corriger : overview, met, stats, moves, trainer, extras, ribbons, memories.
    pub tab: Option<&'static str>,
}

/// Résumé de la rencontre retenue, pour l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterSummary {
    pub kind: EncounterKind,
    pub kind_label: &'static str,
    pub species: u16,
    pub species_name: String,
    pub location: u16,
    pub location_name: Option<String>,
    pub level_min: u8,
    pub level_max: u8,
    pub versions: Vec<String>,
}

/// Rapport complet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub verdict: Verdict,
    pub verdict_label: &'static str,
    pub checks: Vec<Check>,
    pub encounter: Option<EncounterSummary>,
    /// Phrase d'origine (« Capturé dans Platine », « Transféré depuis la Gen 3 »…).
    pub origin: String,
    pub pid_type: Option<PidType>,
    pub errors: usize,
    pub warnings: usize,
}

const TAB_OVERVIEW: &str = "overview";
const TAB_MET: &str = "met";
const TAB_STATS: &str = "stats";
const TAB_MOVES: &str = "moves";
const TAB_TRAINER: &str = "trainer";
const TAB_EXTRAS: &str = "extras";

#[path = "verify_extras.rs"]
mod extras;

/// Liste de résultats avec raccourcis.
#[derive(Default)]
pub(crate) struct Lines(pub Vec<Check>);

impl Lines {
    fn push(&mut self, severity: Severity, code: &'static str, title: impl Into<String>, detail: impl Into<String>, tab: Option<&'static str>) {
        self.0.push(Check { severity, code, title: title.into(), detail: detail.into(), tab });
    }
    fn bad(&mut self, code: &'static str, title: impl Into<String>, detail: impl Into<String>, tab: &'static str) {
        self.push(Severity::Invalid, code, title, detail, Some(tab));
    }
    fn fishy(&mut self, code: &'static str, title: impl Into<String>, detail: impl Into<String>, tab: &'static str) {
        self.push(Severity::Fishy, code, title, detail, Some(tab));
    }
    fn ok(&mut self, code: &'static str, title: impl Into<String>, detail: impl Into<String>) {
        self.push(Severity::Valid, code, title, detail, None);
    }
    fn errors(&self) -> usize {
        self.0.iter().filter(|c| c.severity == Severity::Invalid).count()
    }
    fn warnings(&self) -> usize {
        self.0.iter().filter(|c| c.severity == Severity::Fishy).count()
    }
}

// --- Lecture directe de quelques champs (offsets de PKHeX).

pub(crate) fn iv32(pk: &Pokemon) -> u32 {
    let at = if pk.format().generation() <= 5 { 0x38 } else { 0x74 };
    let d = pk.data();
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) & 0x3FFF_FFFF
}

/// Attaques à réapprendre (PK6/PK7, 0x6A).
pub(crate) fn relearn_moves(pk: &Pokemon) -> [u16; 4] {
    if pk.format().generation() < 6 {
        return [0; 4];
    }
    let d = pk.data();
    std::array::from_fn(|i| u16::from_le_bytes([d[0x6A + 2 * i], d[0x6B + 2 * i]]))
}

/// Pokémon de N (PK5, bit 1 de 0x42).
fn n_sparkle(pk: &Pokemon) -> bool {
    pk.format() == PkmFormat::Gen5 && pk.data()[0x42] & 2 != 0
}

fn shiny_threshold(format: PkmFormat) -> u32 {
    if format.generation() >= 6 {
        16
    } else {
        8
    }
}

fn is_shiny_pid(pid: u32, tid: u16, sid: u16, threshold: u32) -> bool {
    ((pid >> 16) ^ (pid & 0xFFFF) ^ tid as u32 ^ sid as u32) < threshold
}

fn gender_code(g: Gender) -> u8 {
    match g {
        Gender::Male => 0,
        Gender::Female => 1,
        Gender::Genderless => 2,
    }
}

pub(crate) fn gender_from_pid(ratio: u8, pid: u32) -> u8 {
    match ratio {
        255 => 2,
        254 => 1,
        0 => 0,
        r => ((pid & 0xFF) < r as u32) as u8,
    }
}

fn species_name(s: u16) -> String {
    dex::species_name(s).map_or_else(|| format!("n°{s}"), str::to_string)
}

fn move_name(m: u16) -> String {
    dex::move_name(m).map_or_else(|| format!("n°{m}"), str::to_string)
}

fn ball_name(b: u8) -> String {
    dex::ball_name(b).map_or_else(|| format!("Ball n°{b}"), str::to_string)
}

fn version_name(v: u8) -> String {
    dex::game_name(v).map_or_else(|| format!("version n°{v}"), str::to_string)
}

fn location_name(generation: u8, loc: u16) -> String {
    dex::location_name(generation, loc).map_or_else(|| format!("lieu n°{loc}"), str::to_string)
}

/// Espèces dont la forme change hors combat (`FormInfo.FormChange` de PKHeX, + saisons).
fn form_changeable(species: u16) -> bool {
    matches!(
        species,
        412 | 676 | 741 | 479 | 386 | 483 | 484 | 487 | 492 | 493 | 641 | 642 | 645 | 646 | 647 | 649 | 720 | 773 | 800 | 585 | 586 | 718 | 351 | 421
    )
}

/// Méga-évolutions (Gen 6/7) : forme ≠ 0 impossible hors combat.
const MEGA: [u16; 48] = [
    3, 6, 9, 65, 94, 115, 127, 130, 142, 150, 181, 212, 214, 229, 248, 257, 282, 303, 306, 308, 310, 354, 359, 380, 381, 445, 448, 460, 15, 18, 80,
    208, 254, 260, 302, 319, 323, 334, 362, 373, 376, 384, 428, 475, 531, 719, 382, 383,
];

/// Forme qui n'existe qu'en combat (`FormInfo.IsBattleOnlyForm`).
fn battle_only_form(species: u16, form: u8) -> bool {
    if form == 0 {
        return false;
    }
    match species {
        555 => form & 1 == 1,
        658 => form == 2,
        718 => form == 4,
        774 => form < 7,
        778 => form & 1 == 1,
        800 => form == 3,
        351 | 421 | 648 | 681 | 716 | 746 => true,
        s => MEGA.contains(&s),
    }
}

/// Lieux d'événement (distributions).
fn is_event_location(generation: u8, loc: u16) -> bool {
    match generation {
        4 => (3000..=3076).contains(&loc),
        _ => (40001..50000).contains(&loc) || loc == 30011,
    }
}

/// Balls sauvages par génération (`BallUseLegality.GetWildBalls`).
pub(crate) fn wild_balls(generation: u8, version: u8) -> Vec<u8> {
    let mut v: Vec<u8> = vec![1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let hgss = version == encounters::HG || version == encounters::SS;
    if (generation == 4 && hgss) || generation == 7 {
        v.extend(17..=23);
    }
    if generation == 7 {
        v.push(26);
    }
    v
}

/// Valeur d'IV conservée lors d'un transfert : rien ne change, sauf la VC (3 IV à 31).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Origin {
    /// Gen 4 à 7, rencontre vérifiable.
    Known { generation: u8, game: Game },
    /// Gen 3 (GBA / GameCube) via le Pal Parc.
    Gen3,
    /// Console virtuelle (Gen 1 / 2) via la Banque.
    VirtualConsole { generation: u8 },
}

/// Contexte d'analyse.
pub(crate) struct Ctx<'a> {
    pub pk: &'a Pokemon,
    pub game: Game,
    pub format: u8,
    pub version: u8,
    origin: Origin,
    pub level: u8,
    pub chain: Vec<Stage>,
    pub info: Option<PersonalInfo>,
    pub tid: u16,
    pub sid: u16,
    pub pid: u32,
    pub shiny: bool,
    /// Le lieu de rencontre a été remplacé par le transfert (Gen 3 → 4, Gen 3/4 → 5+).
    pub met_replaced: bool,
}

impl<'a> Ctx<'a> {
    pub fn origin_generation(&self) -> u8 {
        match self.origin {
            Origin::Known { generation, .. } => generation,
            Origin::Gen3 => 3,
            Origin::VirtualConsole { generation } => generation,
        }
    }

    pub fn origin_game(&self) -> Option<Game> {
        match self.origin {
            Origin::Known { game, .. } => Some(game),
            _ => None,
        }
    }

    /// Jeux traversés possibles (de la génération d'origine à celle de la sauvegarde).
    pub fn games(&self) -> Vec<Game> {
        let from = self.origin_generation().max(4);
        Game::ALL.iter().copied().filter(|g| g.generation() >= from && g.generation() <= self.format).collect()
    }
}

pub(crate) fn growth_level(pk: &Pokemon, game: Game) -> u8 {
    let growth = dex::personal(game, pk.species(), pk.form()).map(|p| p.growth_rate).or_else(|| crate::names::growth_rate(pk.species()));
    pk.level(growth).unwrap_or(1)
}

/// Prépare le contexte ; `Err` si l'origine elle-même est impossible.
pub(crate) fn context(pk: &Pokemon, game: Game) -> Result<Ctx<'_>, Check> {
    let format = pk.format().generation();
    let version = pk.version();
    let generation = version_generation(version).ok_or_else(|| Check {
        severity: Severity::Invalid,
        code: "origin-unknown",
        title: "Jeu d'origine inconnu".into(),
        detail: format!("Version n°{version} : ce jeu n'existe pas."),
        tab: Some(TAB_MET),
    })?;
    let impossible = generation > format || (generation <= 2 && format != 7) || (generation == 3 && format < 4);
    if impossible {
        return Err(Check {
            severity: Severity::Invalid,
            code: "origin-future",
            title: "Jeu d'origine impossible".into(),
            detail: format!("{} ne peut pas envoyer de Pokémon vers ce format ({}).", version_name(version), pk.format().label()),
            tab: Some(TAB_MET),
        });
    }
    let origin = match generation {
        1 | 2 => Origin::VirtualConsole { generation },
        3 => Origin::Gen3,
        g => Origin::Known { generation: g, game: version_game(version).unwrap_or(game) },
    };
    let level = growth_level(pk, game);
    let chain = evolution::chain(format, pk.species(), pk.form(), level);
    let met_replaced = (generation <= 4 && format >= 5) || (generation == 3 && format == 4) || generation <= 2;
    let pid = pk.pid();
    let (tid, sid) = (pk.tid(), pk.sid());
    Ok(Ctx {
        pk,
        game,
        format,
        version,
        origin,
        level,
        chain,
        info: dex::personal(game, pk.species(), pk.form()),
        tid,
        sid,
        pid,
        shiny: is_shiny_pid(pid, tid, sid, shiny_threshold(pk.format())),
        met_replaced,
    })
}

// --- Recherche des rencontres.

fn form_ok(e: &Encounter, stage: &Stage) -> bool {
    if e.species == 718 && e.form != stage.form {
        // Zygarde (Gen 7) : seules les formes Rassemblement (2, 3) s'obtiennent ensuite ; 10 % → 50 % possible.
        return e.form < 30 && (stage.form >= 2 || (e.form == 1 && stage.form == 0));
    }
    e.form == stage.form || e.form >= 30 || form_changeable(e.species) || (e.species == 664 || e.species == 665) && stage.species == 666
}

/// Position du stade correspondant à la rencontre dans la lignée.
fn stage_of(ctx: &Ctx, e: &Encounter) -> Option<usize> {
    ctx.chain.iter().position(|st| st.species == e.species && form_ok(e, st))
}

/// Niveau maximal de l'espèce dans la même zone (Gen 4 : « pression » des hautes herbes).
fn area_max(list: &[Encounter], e: &Encounter) -> u8 {
    list.iter().filter(|x| x.kind == e.kind && x.location == e.location && x.species == e.species).map(|x| x.level_max).max().unwrap_or(e.level_max)
}

fn level_matches(ctx: &Ctx, list: &[Encounter], e: &Encounter) -> bool {
    let met = ctx.pk.met_level();
    if ctx.met_replaced {
        // Niveau de rencontre = niveau au moment du transfert.
        return e.level_min <= met.max(1) || e.is_egg();
    }
    match e.kind {
        EncounterKind::DreamRadar => met.is_multiple_of(5) && (5..=40).contains(&met),
        k if k.is_wild() => {
            let (mut lo, mut hi) = (e.level_min, e.level_max);
            if e.generation == 4 && k == EncounterKind::Grass {
                hi = area_max(list, e);
            }
            if e.generation == 6 && e.versions.iter().any(|&v| v == encounters::AS || v == encounters::OR) {
                // Flûtes (± 4) et chaînes de PokéRadar Nav.
                lo = lo.saturating_sub(4).max(1);
                hi = hi.saturating_add(if k == EncounterKind::RockSmash { 4 } else { 33 });
            }
            if e.generation == 7 && k == EncounterKind::Sos {
                hi = hi.saturating_add(5);
            }
            (lo..=hi).contains(&met)
        }
        _ => met == e.met_level.unwrap_or(e.level_min),
    }
}

/// Rencontres compatibles avec l'espèce, la version, le lieu et le niveau.
fn candidates(ctx: &Ctx) -> Vec<Encounter> {
    let Some(game) = ctx.origin_game() else { return Vec::new() };
    let pk = ctx.pk;
    let list = encounters::encounters(game);
    let mut out = Vec::new();
    let egg_loc = pk.egg_location();
    for e in list {
        if !e.versions.contains(&ctx.version) {
            continue;
        }
        let Some(stage) = stage_of(ctx, e) else { continue };
        if e.kind == EncounterKind::EggGift {
            if egg_loc == e.egg_location || (egg_loc != 0 && ctx.met_replaced) {
                out.push(e.clone());
            }
            continue;
        }
        if !ctx.met_replaced && pk.met_location() != e.location {
            continue;
        }
        if !level_matches(ctx, list, e) {
            continue;
        }
        let _ = stage;
        out.push(e.clone());
    }
    out.extend(event_candidates(ctx));
    // Œufs de Pension : espèce de base (et l'espèce suivante pour les bébés « encens »).
    if pk.is_egg() || (egg_loc != 0 && !out.iter().any(|e| e.kind == EncounterKind::EggGift)) {
        let n = ctx.chain.len();
        for st in ctx.chain[n.saturating_sub(2)..].iter().rev() {
            out.push(Encounter::egg(game, st.species, st.form));
        }
    }
    out
}

/// Le Pokémon peut venir d'une distribution (rencontre fatidique, Mémoire Ball, lieu d'événement…).
fn may_be_event(ctx: &Ctx) -> bool {
    let pk = ctx.pk;
    let g = ctx.origin_generation();
    pk.fateful_encounter()
        || pk.ball() == 16
        || is_event_location(g, pk.met_location())
        || is_event_location(g, pk.egg_location())
        || pk.met_location() == 30011
}

/// Distributions compatibles (espèce, version, lieu, niveau).
fn event_candidates(ctx: &Ctx) -> Vec<Encounter> {
    if !may_be_event(ctx) {
        return Vec::new();
    }
    let pk = ctx.pk;
    let met = pk.met_level();
    let egg_loc = pk.egg_location();
    events::events(ctx.origin_generation())
        .iter()
        .filter(|e| e.versions.contains(&ctx.version) && stage_of(ctx, e).is_some())
        .filter(|e| {
            if e.is_egg() {
                egg_loc == e.egg_location || matches!(egg_loc, 2002 | 30002 | 30003) || ctx.met_replaced
            } else if ctx.met_replaced {
                e.level_min <= met
            } else {
                pk.met_location() == e.location && met == e.met_level.unwrap_or(e.level_min)
            }
        })
        .cloned()
        .collect()
}

/// La base des distributions connaît-elle cette espèce pour la génération d'origine ?
fn species_has_event(ctx: &Ctx) -> bool {
    events::events(ctx.origin_generation()).iter().any(|e| ctx.chain.iter().any(|s| s.species == e.species))
}

/// Rencontre « événement » non vérifiable (aucune base des distributions).
fn event_marker(ctx: &Ctx) -> bool {
    let pk = ctx.pk;
    let g = ctx.origin_generation();
    let special_location = g >= 5 && (30001..=30020).contains(&pk.met_location());
    pk.fateful_encounter()
        && (pk.ball() == 16
            || ctx.met_replaced
            || special_location
            || is_event_location(g, pk.met_location())
            || is_event_location(g, pk.egg_location()))
        || (pk.ball() == 16 && g >= 4)
}

// --- Vérifications liées à une rencontre.

fn check_encounter(ctx: &Ctx, e: &Encounter, out: &mut Lines) {
    let pk = ctx.pk;
    let origin_gen = ctx.origin_generation();
    let stage = stage_of(ctx, e).unwrap_or(0);

    // Évolution depuis l'espèce rencontrée.
    if stage > 0 && !e.is_egg() {
        let need = ctx.chain[..stage].iter().map(|s| s.level_min).max().unwrap_or(1);
        if ctx.level < need {
            out.bad(
                "evo-level",
                "Évolution trop précoce",
                format!("{} évolue au niveau {need} au plus tôt, il n'est que niveau {}.", species_name(pk.species()), ctx.level),
                TAB_STATS,
            );
        } else if !ctx.met_replaced && e.kind != EncounterKind::Trade && ctx.level <= pk.met_level() && needs_level_up(ctx, stage) {
            out.bad(
                "evo-no-levelup",
                "Évolution sans montée de niveau",
                format!("Rencontré au niveau {} sous la forme {}, il a dû monter de niveau pour évoluer.", pk.met_level(), species_name(e.species)),
                TAB_STATS,
            );
        }
    }

    // Ball.
    let ball = pk.ball();
    if let Some(fixed) = e.ball {
        let shedinja = pk.species() == 292 && ball == 4;
        if ball != fixed && !shedinja {
            out.bad(
                "ball-fixed",
                "Ball impossible",
                format!("Cette rencontre se fait obligatoirement dans une {} (ici : {}).", ball_name(fixed), ball_name(ball)),
                TAB_MET,
            );
        }
    } else if e.kind == EncounterKind::Egg {
        // Gen 6+ : la Ball de la mère (ou du père en Gen 7) est transmise.
        let max = if e.generation >= 7 { 26 } else { 25 };
        if ball == 1 || ball == 16 || ball == 0 || ball > max {
            out.bad("ball-egg", "Ball impossible pour un œuf", format!("Un œuf ne peut pas hériter d'une {}.", ball_name(ball)), TAB_MET);
        }
    } else if e.kind == EncounterKind::DreamWorld {
        let mut allowed = wild_balls(5, ctx.version);
        allowed.push(25);
        if !allowed.contains(&ball) {
            out.bad(
                "ball-dream",
                "Ball impossible",
                format!("Les Pokémon du Monde des Rêves arrivent dans une Ball de capture classique ou une Rêve Ball, pas une {}.", ball_name(ball)),
                TAB_MET,
            );
        }
    } else if !e.is_egg() {
        let allowed = wild_balls(e.generation, ctx.version);
        if !allowed.contains(&ball) {
            out.bad(
                "ball-wild",
                "Ball impossible",
                format!("Impossible de capturer ce Pokémon avec une {} dans {}.", ball_name(ball), version_name(ctx.version)),
                TAB_MET,
            );
        }
    }

    // Talent caché / imposé.
    check_ability_rule(ctx, e, out);

    // Chromatique.
    match e.shiny {
        ShinyRule::Never if ctx.shiny => out.bad(
            "shiny-lock",
            "Verrou chromatique",
            format!("{} ne peut pas être chromatique dans cette rencontre (verrou chromatique).", species_name(e.species)),
            TAB_OVERVIEW,
        ),
        ShinyRule::Always if !ctx.shiny => {
            out.bad("shiny-forced", "Doit être chromatique", "Cette rencontre donne toujours un Pokémon chromatique.", TAB_OVERVIEW)
        }
        ShinyRule::FixedPid => {
            if let Some(pid) = e.pid {
                let actual = if ctx.format >= 6 && origin_gen <= 5 { pk.encryption_constant() } else { ctx.pid };
                if actual != pid {
                    out.bad("pid-fixed", "PID différent du don", format!("Ce Pokémon a un PID imposé ({pid:08X}), ici {actual:08X}."), TAB_OVERVIEW);
                }
            }
        }
        _ => {}
    }

    // Sexe imposé.
    if let (Some(g), Some(info)) = (e.gender, ctx.info.as_ref()) {
        let ratio = info.gender_ratio;
        if g < 2 && !matches!(ratio, 0 | 254 | 255) && gender_code(pk.gender()) != g && stage == 0 {
            let label = if g == 0 { "mâle" } else { "femelle" };
            out.bad("gender-fixed", "Sexe imposé", format!("Cette rencontre donne toujours un Pokémon {label}."), TAB_OVERVIEW);
        }
    }

    // Nature imposée.
    if let Some(n) = e.nature {
        if pk.nature() != n {
            out.bad(
                "nature-fixed",
                "Nature imposée",
                format!("Cette rencontre a toujours la nature {}.", dex::nature_name(n).unwrap_or("?")),
                TAB_STATS,
            );
        }
    }

    // IV imposés / parfaits.
    let ivs = pk.ivs();
    if let Some(fixed) = e.ivs {
        let wrong: Vec<usize> = (0..6).filter(|&i| fixed[i] >= 0 && ivs[i] != fixed[i] as u8).collect();
        if !wrong.is_empty() && !(origin_gen <= 2) {
            out.bad("iv-fixed", "IV imposés", "Les IV de ce Pokémon sont fixés par la rencontre.".to_string(), TAB_STATS);
        }
    }
    let mut flawless = e.flawless_ivs;
    if e.kind.is_wild() && e.generation >= 6 {
        let undiscovered = dex::personal(ctx.game, e.species, e.form).is_some_and(|p| p.egg_groups[0] == 15);
        if undiscovered
            && !matches!(e.species, 172 | 173 | 174 | 175 | 236 | 238 | 239 | 240 | 298 | 360 | 406 | 433 | 438 | 439 | 440 | 446 | 447 | 458)
        {
            flawless = flawless.max(3);
        }
    }
    if flawless > 0 && origin_gen >= 6 {
        let perfect = ivs.iter().filter(|&&v| v == 31).count() as u8;
        if perfect < flawless {
            out.bad(
                "iv-flawless",
                "IV parfaits manquants",
                format!("Cette rencontre garantit au moins {flawless} IV à 31 (ici {perfect})."),
                TAB_STATS,
            );
        }
    }

    // Dresseur imposé (échanges, Pokémon de N, Ranch).
    if let Some(t) = &e.trainer {
        let id_ok = pk.tid() == t.tid && pk.sid() == t.sid;
        if !id_ok {
            out.bad(
                "trainer-id",
                "ID du dresseur incorrect",
                format!("Ce Pokémon d'échange appartient au dresseur ID {} (ID secret {}).", t.tid, t.sid),
                TAB_TRAINER,
            );
        }
        let ot = pk.ot_name();
        // Noms japonais / coréens des cartes Gen 4 : jeu de caractères non décodé, comparaison impossible.
        let undecodable = t.names.iter().any(|(_, n)| n.contains("{X:"));
        let same = |n: &str| n == ot || (ctx.met_replaced && n.eq_ignore_ascii_case(&ot));
        if !t.names.is_empty() && !undecodable && !t.names.iter().any(|(_, n)| same(n)) {
            let fr = t.names.iter().find(|(l, _)| *l == 3).or(t.names.first()).map(|(_, n)| n.clone()).unwrap_or_default();
            out.bad(
                "trainer-name",
                "Nom du dresseur incorrect",
                format!("Le dresseur d'origine de ce Pokémon s'appelle « {fr} » (version française)."),
                TAB_TRAINER,
            );
        }
        if let Some(g) = t.ot_gender {
            if gender_code(pk.ot_gender()) != g && e.kind == EncounterKind::Trade {
                out.fishy("trainer-gender", "Sexe du dresseur", "Le sexe du dresseur d'origine ne correspond pas à l'échange.", TAB_TRAINER);
            }
        }
    }

    // Distribution : langue et constante de chiffrement imposées.
    if let Some(lang) = e.language {
        if pk.language() != lang && !pk.is_egg() {
            out.bad("event-language", "Langue de la distribution", "Cette distribution n'existait que dans une autre langue.", TAB_TRAINER);
        }
    }
    if let Some(ec) = e.ec {
        if pk.encryption_constant() != ec {
            out.bad(
                "event-ec",
                "Constante de chiffrement de la distribution",
                format!("Cette distribution a une constante de chiffrement fixe ({ec:08X})."),
                TAB_OVERVIEW,
            );
        }
    }

    // Rencontre fatidique.
    if pk.fateful_encounter() != e.fateful && !(n_sparkle(pk) && e.n_sparkle) {
        if pk.fateful_encounter() {
            out.bad(
                "fateful-extra",
                "Rencontre fatidique en trop",
                "Seuls les Pokémon d'événement (et quelques Pokémon fixes) ont la « rencontre fatidique ».",
                TAB_MET,
            );
        } else {
            out.bad("fateful-missing", "Rencontre fatidique manquante", "Ce Pokémon s'obtient avec la mention « rencontre fatidique ».", TAB_MET);
        }
    }
    if n_sparkle(pk) && !e.n_sparkle {
        out.bad("n-sparkle", "Étincelles de N", "Seuls les Pokémon de N brillent de ses étincelles.", TAB_EXTRAS);
    }

    // Lieu d'éclosion / de l'œuf.
    let egg_loc = pk.egg_location();
    if !e.is_egg() && egg_loc != 0 && e.egg_location == 0 && !ctx.met_replaced {
        out.bad("egg-location", "Lieu de l'œuf en trop", "Ce Pokémon n'est pas né d'un œuf : le lieu de l'œuf doit être vide.", TAB_MET);
    }
    if e.kind == EncounterKind::Egg && !pk.is_egg() && !ctx.met_replaced {
        let expected = if e.generation == 4 { 0 } else { 1 };
        if pk.met_level() != expected {
            out.bad(
                "hatch-level",
                "Niveau d'éclosion",
                format!("Un Pokémon né d'un œuf en Gen {} est « rencontré » au niveau {expected}.", e.generation),
                TAB_MET,
            );
        }
        let daycare = matches!(egg_loc, 2000 | 2002 | 60002 | 30002 | 30003);
        if !daycare && !(2009..=2014).contains(&egg_loc) && !(3000..4000).contains(&egg_loc) {
            out.fishy("egg-loc", "Lieu de l'œuf inhabituel", format!("Œuf reçu à « {} ».", location_name(e.generation, egg_loc)), TAB_MET);
        }
    }

    // PID / IV.
    check_pidiv(ctx, e, out);

    // Attaques à réapprendre (Gen 6+).
    check_relearn(ctx, e, out);
}

fn needs_level_up(ctx: &Ctx, stage: usize) -> bool {
    (0..stage).any(|i| {
        let st = ctx.chain[i];
        evolution::pre_evolutions(ctx.format, st.species, st.form).iter().all(|l| l.needs_level_up())
    })
}

fn check_ability_rule(ctx: &Ctx, e: &Encounter, out: &mut Lines) {
    let pk = ctx.pk;
    let n = pk.ability_number();
    let has_hidden = ctx.info.as_ref().is_some_and(|i| i.abilities[2] != 0);
    let allows_hidden = match e.kind {
        // Gen 5 : il faut une mère au talent caché (Monde des Rêves, Trouées Cachées…).
        EncounterKind::Egg if e.generation == 5 => gen5_hidden_source(ctx),
        EncounterKind::Egg => e.generation >= 6,
        _ => e.ability.allows_hidden(),
    };
    if n == 4 && !allows_hidden && has_hidden {
        out.bad("ability-hidden", "Talent caché impossible", "Cette rencontre ne donne pas le talent caché.", TAB_OVERVIEW);
    }
    if e.ability == AbilityRule::OnlyHidden && n != 4 && has_hidden {
        out.bad("ability-hidden-required", "Talent caché attendu", "Cette rencontre donne toujours le talent caché.", TAB_OVERVIEW);
    }
    // Talent 1 / 2 imposé : la Pilule Talent (Gen 6+) permet de changer ensuite.
    if ctx.format <= 5 && e.kind != EncounterKind::Trade {
        let wrong = match e.ability {
            AbilityRule::OnlyFirst => n == 2,
            AbilityRule::OnlySecond => n == 1,
            _ => false,
        };
        let same = ctx.info.as_ref().is_some_and(|i| i.abilities[0] == i.abilities[1]);
        if wrong && !same {
            out.bad("ability-fixed", "Talent imposé", e.ability.label().to_string(), TAB_OVERVIEW);
        }
    }
}

/// Un Pokémon de la lignée a-t-il un talent caché obtenable en Gen 5 ?
fn gen5_hidden_source(ctx: &Ctx) -> bool {
    let species: Vec<u16> = ctx.chain.iter().map(|s| s.species).collect();
    [Game::BW, Game::B2W2].iter().any(|&g| encounters::encounters(g).iter().any(|e| e.ability.allows_hidden() && species.contains(&e.species)))
}

/// Zone Gen 4 dont les tirages (méthodes J et K) sont vérifiés.
pub(crate) fn area4(kind: EncounterKind) -> Option<rng::Area4> {
    Some(match kind {
        EncounterKind::Grass => rng::Area4::Grass,
        EncounterKind::Surf => rng::Area4::Surf,
        EncounterKind::OldRod => rng::Area4::OldRod,
        EncounterKind::GoodRod => rng::Area4::GoodRod,
        EncounterKind::SuperRod => rng::Area4::SuperRod,
        _ => return None,
    })
}

/// Slots de la zone de `e` qui donnent la même espèce dans cette version.
fn slots4(game: Game, e: &Encounter, version: u8) -> Vec<rng::Slot4> {
    let Some(area) = area4(e.kind) else { return Vec::new() };
    let mut out = Vec::new();
    for x in encounters::encounters(game) {
        if x.kind == e.kind && x.location == e.location && x.species == e.species && x.form == e.form && x.versions.contains(&version) {
            for &slot in &x.slots {
                out.push(rng::Slot4 { area, slot, level_min: x.level_min, level_max: x.level_max });
            }
        }
    }
    out
}

fn check_pidiv(ctx: &Ctx, e: &Encounter, out: &mut Lines) {
    let pk = ctx.pk;
    let origin_gen = ctx.origin_generation();
    // PID d'origine (avant transfert vers la Gen 6, PID = EC).
    let pid = if ctx.format >= 6 && origin_gen <= 5 { pk.encryption_constant() } else { ctx.pid };
    if origin_gen == 4 && !e.is_egg() && !matches!(e.kind, EncounterKind::Trade | EncounterKind::Event) && e.pid.is_none() {
        let ratio = ctx.info.as_ref().map_or(127, |i| i.gender_ratio);
        let t = rng::analyze_gen34(pid, iv32(pk), ctx.tid, ctx.sid, ctx.shiny, gender_code(pk.gender()), ratio);
        let ok = match e.kind {
            EncounterKind::Pokewalker => t == PidType::Pokewalker,
            EncounterKind::Grass if t == PidType::ChainShiny => {
                e.versions.iter().any(|&v| v == encounters::D || v == encounters::P || v == encounters::PT)
            }
            k if k.is_wild() => matches!(t, PidType::Method1 | PidType::CuteCharm),
            _ => t == PidType::Method1,
        };
        // Méthodes J / K : les tirages qui précèdent le PID doivent donner le slot (et le niveau).
        if ok && t == PidType::Method1 && area4(e.kind).is_some() {
            let game = ctx.origin_game().unwrap_or(ctx.game);
            let slots = slots4(game, e, ctx.version);
            if !slots.is_empty() {
                let method = rng::Method4::of_version(ctx.version);
                let met = (ctx.format == 4 && pk.met_level() > 1).then(|| pk.met_level());
                if rng::frame4(method, &slots, met, pid, iv32(pk)).is_none() {
                    out.bad(
                        "pidiv-frame",
                        "Tirage impossible pour cette rencontre",
                        format!(
                            "En {} ({}), le jeu tire l'emplacement de la rencontre{}, puis la nature, avant le PID : aucune suite de tirages ne mène à ce PID depuis cet emplacement.",
                            version_name(ctx.version),
                            method.label(),
                            if e.kind == EncounterKind::Grass { "" } else { " et le niveau" }
                        ),
                        TAB_STATS,
                    );
                }
            }
        }
        if !ok {
            out.bad(
                "pidiv",
                "PID et IV sans lien",
                format!(
                    "En Gen 4, le PID et les IV d'un Pokémon {} sont tirés à la suite (méthode 1) : ici aucune corrélation ({}).",
                    if e.kind.is_wild() { "sauvage" } else { "fixe ou offert" },
                    t.label()
                ),
                TAB_STATS,
            );
        }
    }
    if origin_gen == 5 {
        let wild_xor = match e.kind {
            EncounterKind::HiddenGrotto => false,
            k if k.is_wild() => true,
            EncounterKind::Static => e.shiny == ShinyRule::Random && e.ball != Some(4) && e.ability != AbilityRule::OnlyHidden,
            _ => false,
        };
        if wild_xor && !rng::gen5_xor_ok(pid, ctx.tid, ctx.sid) {
            out.bad(
                "pid-gen5",
                "PID incompatible (Gen 5)",
                "En Gen 5, le bit de poids fort du PID d'un Pokémon sauvage dépend de l'ID du dresseur.",
                TAB_OVERVIEW,
            );
        }
        if e.kind == EncounterKind::HiddenGrotto && ctx.shiny {
            out.bad(
                "shiny-grotto",
                "Chromatique de Trouée Cachée",
                "Les Pokémon des Trouées Cachées ne peuvent pas être chromatiques.",
                TAB_OVERVIEW,
            );
        }
    }
}

/// Les attaques à réapprendre actuelles conviennent-elles à la rencontre ?
pub(crate) fn relearn_valid(ctx: &Ctx, e: &Encounter) -> bool {
    let mut lines = Lines::default();
    check_relearn(ctx, e, &mut lines);
    lines.errors() == 0
}

fn check_relearn(ctx: &Ctx, e: &Encounter, out: &mut Lines) {
    if ctx.format < 6 {
        return;
    }
    let relearn = relearn_moves(ctx.pk);
    let set: Vec<u16> = relearn.iter().copied().filter(|&m| m != 0).collect();
    let origin_gen = ctx.origin_generation();
    if origin_gen < 6 {
        if !set.is_empty() {
            out.bad(
                "relearn-transfer",
                "Attaques à réapprendre en trop",
                "Un Pokémon venu d'une ancienne génération n'a pas d'attaque à réapprendre.",
                TAB_MOVES,
            );
        }
        return;
    }
    let game = ctx.origin_game().unwrap_or(ctx.game);
    if e.kind == EncounterKind::Egg {
        let base = e.species;
        let eggs = learn::egg_moves(game, base, e.form);
        let lvl = dex::levelup(game, base, e.form);
        let q = LearnQuery { species: base, form: e.form, level: 100, format: ctx.format };
        for &m in &set {
            let ok = eggs.contains(&m) || lvl.iter().any(|&(x, _)| x == m) || learn::can_learn(game, &q, m) == Some(learn::LearnMethod::Machine);
            if !ok {
                out.bad("relearn-egg", "Attaque à réapprendre impossible", format!("{} ne peut pas venir de l'œuf.", move_name(m)), TAB_MOVES);
            }
        }
        return;
    }
    if !e.relearn.is_empty() {
        let mut want = e.relearn.clone();
        want.sort();
        let mut have = set.clone();
        have.sort();
        if want != have {
            out.bad("relearn-fixed", "Attaques à réapprendre", "Cette rencontre a des attaques à réapprendre imposées.", TAB_MOVES);
        }
        return;
    }
    // PokéRadar Nav (ROSA) : une capacité Œuf possible en premier.
    let dexnav = e.generation == 6 && e.kind.is_wild() && e.ability == AbilityRule::Any12H && e.kind != EncounterKind::Horde;
    if dexnav && set.len() == 1 && relearn[0] != 0 {
        let base = evolution::base_of(6, e.species, e.form);
        if learn::egg_moves(game, base.0, base.1).contains(&relearn[0]) || learn::egg_moves(game, e.species, e.form).contains(&relearn[0]) {
            return;
        }
    }
    if !set.is_empty() && !e.is_egg() {
        out.bad("relearn-extra", "Attaques à réapprendre en trop", "Cette rencontre ne donne aucune attaque à réapprendre.", TAB_MOVES);
    }
}

// --- Attaques.

/// Attaques que le Pokémon peut connaître (sans la rencontre fixe).
pub(crate) fn learn_sources(ctx: &Ctx, e: Option<&Encounter>, mv: u16) -> Option<learn::LearnMethod> {
    // Queulorior : Gribouille copie n'importe quelle attaque (sauf Lutte et Babil).
    if ctx.pk.species() == 235 && !matches!(mv, 165 | 448) {
        return Some(learn::LearnMethod::Special);
    }
    if let Some(e) = e {
        if e.moves.contains(&mv) || e.relearn.contains(&mv) {
            return Some(learn::LearnMethod::Special);
        }
    }
    if ctx.format >= 6 && relearn_moves(ctx.pk).contains(&mv) {
        return Some(learn::LearnMethod::Special);
    }
    for game in ctx.games() {
        for st in &ctx.chain {
            if st.species > dex::max_species(game) {
                continue;
            }
            let q = LearnQuery { species: st.species, form: st.form, level: st.level_max, format: ctx.format };
            if let Some(m) = learn::can_learn(game, &q, mv) {
                return Some(m);
            }
        }
    }
    if let Some(e) = e {
        if e.is_egg() {
            let game = ctx.origin_game().unwrap_or(ctx.game);
            if learn::egg_moves(game, e.species, e.form).contains(&mv) && e.kind == EncounterKind::Egg {
                return Some(learn::LearnMethod::Egg);
            }
            // Attaques héritées du père (par niveau à n'importe quel niveau, CT en Gen 4/5).
            if dex::levelup(game, e.species, e.form).iter().any(|&(m, _)| m == mv) && e.kind == EncounterKind::Egg {
                return Some(learn::LearnMethod::Egg);
            }
            let q = LearnQuery { species: e.species, form: e.form, level: 100, format: ctx.format };
            if e.kind == EncounterKind::Egg && e.generation <= 5 && learn::can_learn(game, &q, mv) == Some(learn::LearnMethod::Machine) {
                return Some(learn::LearnMethod::Egg);
            }
        }
    }
    None
}

fn check_moves(ctx: &Ctx, e: Option<&Encounter>, unverifiable: bool, out: &mut Lines) {
    let pk = ctx.pk;
    let moves = pk.moves();
    let present: Vec<u16> = moves.iter().copied().filter(|&m| m != 0).collect();
    if present.is_empty() {
        out.bad("moves-none", "Aucune attaque", "Un Pokémon doit connaître au moins une attaque.", TAB_MOVES);
        return;
    }
    if (1..4).any(|i| moves[i] != 0 && moves[i - 1] == 0) {
        out.bad("moves-gap", "Emplacement d'attaque vide", "Les attaques doivent se suivre, sans case vide entre elles.", TAB_MOVES);
    }
    let mut seen = Vec::new();
    for &m in &present {
        if seen.contains(&m) {
            out.bad("moves-duplicate", "Attaque en double", format!("{} apparaît deux fois.", move_name(m)), TAB_MOVES);
        }
        seen.push(m);
    }
    let max = dex::max_move(ctx.game);
    for &m in &present {
        if m > max || dex::move_name(m).is_none() {
            out.bad("move-unknown", "Attaque inconnue", format!("L'attaque n°{m} n'existe pas dans ce jeu."), TAB_MOVES);
            continue;
        }
        if learn_sources(ctx, e, m).is_none() {
            let name = move_name(m);
            if unverifiable {
                out.fishy(
                    "move-unverified",
                    format!("{name} non vérifiable"),
                    "Cette attaque a pu être apprise dans un jeu plus ancien (Gen 1 à 3) ou lors d'un événement : impossible de le confirmer.",
                    TAB_MOVES,
                );
            } else {
                out.bad(
                    "move-illegal",
                    format!("{name} impossible"),
                    format!(
                        "{} ne peut pas apprendre {name} par niveau, CT/CS, donneur de capacités ni comme capacité Œuf.",
                        species_name(pk.species())
                    ),
                    TAB_MOVES,
                );
            }
        }
    }
}

// --- Vérifications générales.

fn check_general(ctx: &Ctx, out: &mut Lines) {
    let pk = ctx.pk;
    let game = ctx.game;
    if !pk.checksum_valid() {
        out.bad(
            "checksum",
            "Données abîmées",
            "La somme de contrôle du Pokémon ne correspond pas : le jeu le verra comme un « Œuf corrompu ».",
            TAB_EXTRAS,
        );
    }
    let species = pk.species();
    if species > dex::max_species(game) {
        out.bad("species-game", "Espèce absente du jeu", format!("{} n'existe pas encore dans ce jeu.", species_name(species)), TAB_OVERVIEW);
    }
    // Forme.
    let forms = dex::form_names(game, species).len().max(ctx.info.as_ref().map_or(1, |i| i.form_count as usize));
    if pk.form() as usize >= forms.max(1) && !(species == 25 && pk.form() <= 6) {
        out.bad("form", "Forme inconnue", format!("Forme n°{} impossible pour {}.", pk.form(), species_name(species)), TAB_OVERVIEW);
    } else if battle_only_form(species, pk.form()) {
        out.bad("form-battle", "Forme de combat", "Cette forme n'existe qu'en combat (Méga-Évolution, forme Primo, Mode Transe…).", TAB_OVERVIEW);
    }

    // Forme inexistante dans le jeu d'origine (forme d'Alola venue de la Gen 6…).
    if let Some(og) = ctx.origin_game() {
        let form = pk.form();
        let base_forms = dex::personal(og, species, 0).map_or(1, |p| p.form_count.max(1));
        let names = dex::form_names(og, species).len();
        if form != 0 && !form_changeable(species) && form as usize >= (base_forms as usize).max(names) && species <= dex::max_species(og) {
            out.bad(
                "form-origin",
                "Forme absente du jeu d'origine",
                format!("Cette forme de {} n'existe pas dans {}.", species_name(species), version_name(ctx.version)),
                TAB_OVERVIEW,
            );
        }
    }
    // Mistigrix : la forme suit le sexe.
    if species == 678 && pk.form() != (pk.gender() == Gender::Female) as u8 {
        out.bad("form-gender", "Forme incohérente avec le sexe", "Mistigrix mâle a la forme 0, Mistigrix femelle la forme 1.", TAB_OVERVIEW);
    }

    // Longueur du surnom (`Legal.GetMaxLengthNickname`).
    let lang = pk.language();
    let asian = matches!(lang, 1 | 8 | 9 | 10);
    let max_nick = match (asian, ctx.format >= 6) {
        (true, true) => 6,
        (true, false) => 5,
        (false, true) => 12,
        (false, false) => 10,
    };
    let nick_len = pk.nickname().chars().count();
    // (Gen 4 : jeu de caractères propre, le décodage des noms japonais n'est pas fiable.)
    if !pk.is_egg() && pk.is_nicknamed() && ctx.format >= 5 && nick_len > max_nick {
        out.bad("nickname-length", "Surnom trop long", format!("{nick_len} caractères : {max_nick} au maximum dans cette langue."), TAB_OVERVIEW);
    }

    // Console virtuelle : espèces, langue, statistiques de Concours.
    if let Origin::VirtualConsole { generation } = ctx.origin {
        let max_vc = if generation == 1 { 151 } else { 251 };
        // La Banque donne au moins 3 IV à 31 (5 pour Mew et Celebi).
        let need = if matches!(species, 151 | 251) { 5 } else { 3 };
        let perfect = pk.ivs().iter().filter(|&&v| v == 31).count();
        if perfect < need {
            out.bad(
                "vc-ivs",
                "IV parfaits manquants",
                format!("Un Pokémon de la Console virtuelle reçoit au moins {need} IV à 31 lors du transfert (ici {perfect})."),
                TAB_STATS,
            );
        }
        // Stade transféré : le premier de la lignée qui existait dans le jeu d'origine.
        let transferred = ctx.chain.iter().position(|s| s.species <= max_vc);
        let impossible = match transferred {
            None => true,
            Some(0) => false,
            Some(i) => ctx.level <= pk.met_level() && needs_level_up(ctx, i),
        };
        if impossible {
            out.bad(
                "vc-species",
                "Espèce absente de la Console virtuelle",
                format!("{} ne pouvait pas être transféré depuis la Gen {generation}.", species_name(species)),
                TAB_MET,
            );
        }
        if !matches!(lang, 1..=5 | 7) {
            out.bad("vc-language", "Langue impossible", "Les jeux de la Console virtuelle n'existent pas dans cette langue.", TAB_TRAINER);
        }
        if ctx.format >= 6 && pk.data()[0x24..0x2A].iter().any(|&v| v != 0) {
            out.bad("vc-contest", "Statistiques de Concours", "Un Pokémon de la Console virtuelle n'a jamais participé à un Concours.", TAB_EXTRAS);
        }
    }
    // Gen 3 : Mentali / Noctali ne peuvent pas évoluer dans RFVF (pas d'horloge).
    if matches!(species, 196 | 197) && matches!(ctx.version, 4 | 5) && ctx.level <= pk.met_level() {
        out.bad(
            "evo-frlg",
            "Évolution impossible dans RFVF",
            "Rouge Feu et Vert Feuille n'ont pas d'horloge : Évoli n'a pas pu y évoluer ainsi.",
            TAB_MET,
        );
    }

    // Niveau de rencontre.
    if !pk.is_egg() && pk.met_level() > ctx.level {
        out.bad(
            "met-level",
            "Niveau de rencontre trop haut",
            format!("Rencontré au niveau {}, mais il est niveau {}.", pk.met_level(), ctx.level),
            TAB_MET,
        );
    }
    if pk.is_egg() {
        if ctx.level != 1 {
            out.bad("egg-level", "Œuf de niveau incorrect", "Un œuf est toujours niveau 1.", TAB_STATS);
        }
        if pk.evs().iter().any(|&v| v != 0) {
            out.bad("egg-evs", "EV sur un œuf", "Un œuf ne peut pas avoir d'EV.", TAB_STATS);
        }
        if pk.egg_location() == 0 {
            out.bad("egg-noloc", "Œuf sans lieu", "Un œuf doit avoir un lieu de réception (Pension…).", TAB_MET);
        }
    }

    // Talent.
    if let Some(info) = &ctx.info {
        let n = pk.ability_number();
        let ability = pk.ability();
        let idx = match n {
            1 => Some(0),
            2 => Some(1),
            4 => Some(2),
            _ => None,
        };
        match idx {
            None => out.bad("ability-number", "Numéro de talent invalide", format!("Valeur {n} (1, 2 ou 4 attendu)."), TAB_OVERVIEW),
            Some(i) => {
                let expected = info.abilities[i];
                // Talent du jeu d'origine (les fiches changent parfois d'une version à l'autre).
                let mut origin_expected = ctx.origin_game().and_then(|og| dex::personal(og, species, pk.form())).map(|p| p.abilities[i]);
                // Bargantua bleu en Gen 5 (format 5) : Téméraire par erreur du jeu (fiche de la forme rouge).
                if species == 550 && pk.form() == 1 && i == 0 && ctx.format == 5 && ctx.origin_generation() == 5 {
                    origin_expected = Some(120);
                }
                if i == 2 && expected == 0 {
                    out.bad(
                        "ability-nohidden",
                        "Pas de talent caché",
                        format!("{} n'a pas de talent caché dans ce jeu.", species_name(species)),
                        TAB_OVERVIEW,
                    );
                } else if ability != expected && origin_expected != Some(ability) {
                    let in_list = info.abilities.contains(&ability) && ability != 0;
                    let detail = if in_list {
                        format!(
                            "Le talent {} ne correspond pas à l'emplacement {} (attendu : {}).",
                            dex::ability_name(ability).unwrap_or("?"),
                            if n == 4 { "caché".to_string() } else { n.to_string() },
                            dex::ability_name(expected).unwrap_or("?")
                        )
                    } else {
                        format!("{} ne peut pas avoir le talent {}.", species_name(species), dex::ability_name(ability).unwrap_or("?"))
                    };
                    // Échanges de NB : talent imposé, sans lien avec le PID ; Bargantua bleu échangé avec Téméraire (bogue du jeu).
                    let bw_trade = ctx.origin_generation() == 5 && pk.met_location() == 30002 && (in_list || (species == 550 && ability == 120));
                    if bw_trade {
                        out.ok("ability-trade", "Talent d'échange", "Les échanges de Noir et Blanc imposent le talent, sans lien avec le PID.");
                    } else if ctx.origin_generation() <= 4
                        && ctx.format <= 5
                        && in_list
                        && i < 2
                        && info.abilities[0] != info.abilities[1]
                        && ability == info.abilities[0]
                    {
                        // Gen 3 → 4/5 : le talent 2 n'existait parfois pas encore (on garde l'ancien).
                        out.fishy("ability-mismatch-old", "Talent d'une ancienne génération", detail, TAB_OVERVIEW);
                    } else {
                        out.bad("ability-mismatch", "Talent incohérent", detail, TAB_OVERVIEW);
                    }
                }
            }
        }
        // Gen 3 à 5 transférés vers la Gen 6 : l'emplacement est recalculé d'après le talent
        // (`PK5.CalculateTransferAbilityIndex`) ; talents identiques → emplacement 1.
        if ctx.format >= 6 && ctx.origin_generation() <= 5 && ctx.origin_generation() >= 3 && n == 2 && info.abilities[0] == info.abilities[1] {
            out.bad(
                "ability-number-transfer",
                "Numéro de talent incohérent",
                "Après transfert vers la Gen 6, un Pokémon aux deux talents identiques a le talent n°1.",
                TAB_OVERVIEW,
            );
        }
        // VC (Gen 1/2) : talent caché à l'arrivée dans la Banque.
        if let Origin::VirtualConsole { .. } = ctx.origin {
            if n != 4 && info.abilities[2] != 0 && !matches!(species, 151 | 251) {
                out.bad(
                    "ability-vc",
                    "Talent de la Console virtuelle",
                    "Les Pokémon de la Console virtuelle reçoivent leur talent caché lors du transfert.",
                    TAB_OVERVIEW,
                );
            }
        }

        // Sexe.
        let g = gender_code(pk.gender());
        let ratio = info.gender_ratio;
        let expected = match ratio {
            255 => Some(2),
            254 => Some(1),
            0 => Some(0),
            _ => None,
        };
        if let Some(x) = expected {
            if g != x {
                out.bad(
                    "gender-ratio",
                    "Sexe impossible",
                    format!("{} est toujours {}.", species_name(species), ["mâle", "femelle", "asexué"][x as usize]),
                    TAB_OVERVIEW,
                );
            }
        } else if g == 2 {
            out.bad("gender-none", "Sexe manquant", format!("{} a un sexe (mâle ou femelle).", species_name(species)), TAB_OVERVIEW);
        } else if ctx.origin_generation() <= 5 && ctx.origin_generation() >= 3 {
            let pid = if ctx.format >= 6 { pk.encryption_constant() } else { ctx.pid };
            if gender_from_pid(ratio, pid) != g && !matches!(species, 183 | 184 | 298) {
                out.bad("gender-pid", "Sexe incohérent avec le PID", "Jusqu'à la Gen 5, le sexe découle de l'octet bas du PID.", TAB_OVERVIEW);
            }
        }
    }

    // Nature des Gen 3/4 : PID % 25.
    if ctx.origin_generation() <= 4 && ctx.origin_generation() >= 3 && ctx.format >= 5 {
        let pid = if ctx.format >= 6 { pk.encryption_constant() } else { ctx.pid };
        if pk.nature() as u32 != pid % 25 {
            out.bad("nature-pid", "Nature incohérente avec le PID", "Pour un Pokémon des Gen 3/4, la nature vaut PID % 25.", TAB_STATS);
        }
    }
    if pk.nature() >= 25 {
        out.bad("nature", "Nature invalide", format!("Nature n°{}.", pk.nature()), TAB_STATS);
    }

    // PID / EC (Gen 6+).
    if ctx.format >= 6 {
        let ec = pk.encryption_constant();
        if ctx.origin_generation() >= 3 && ctx.origin_generation() <= 5 {
            if ctx.pid != rng::transfer_pid(ec, ctx.tid, ctx.sid) {
                out.bad(
                    "pid-transfer",
                    "PID incohérent après transfert",
                    "Après Poké Transfert, le PID doit être égal à la constante de chiffrement (bit 31 inversé si nécessaire).",
                    TAB_OVERVIEW,
                );
            }
        } else if ctx.origin_generation() >= 6 && ec == ctx.pid {
            out.bad(
                "pid-ec",
                "PID égal à la constante de chiffrement",
                "En Gen 6+, le PID et la constante de chiffrement sont tirés séparément.",
                TAB_OVERVIEW,
            );
        }
        if ec == 0 {
            out.fishy("ec-zero", "Constante de chiffrement nulle", "Valeur très improbable.", TAB_OVERVIEW);
        }
    }
    if ctx.pid == 0 {
        out.fishy("pid-zero", "PID nul", "Valeur très improbable.", TAB_OVERVIEW);
    }

    // EV.
    let evs = pk.evs();
    let total: u32 = evs.iter().map(|&v| v as u32).sum();
    if total > 510 {
        out.bad("ev-total", "Trop d'EV", format!("{total} EV au total, 510 au maximum."), TAB_STATS);
    }
    if ctx.format >= 6 && evs.iter().any(|&v| v > 252) {
        out.bad("ev-252", "EV au-delà de 252", "Depuis la Gen 6, une statistique ne peut pas dépasser 252 EV.", TAB_STATS);
    }
    if ctx.format == 4 && !pk.is_egg() && total > 0 {
        let vitamins = evs.iter().all(|&v| v == 0 || (v <= 100 && v % 10 == 0));
        if !vitamins {
            if let Some(info) = &ctx.info {
                if pk.exp() == exp_for_level(info.growth_rate, pk.met_level().max(1)) && ctx.origin_generation() == 4 {
                    out.bad("ev-untrained", "EV sans combat", "Ce Pokémon n'a gagné aucune expérience depuis sa rencontre : seules les vitamines (100 EV max par statistique, par 10) ont pu lui donner des EV.", TAB_STATS);
                }
            }
        }
    }

    // PP et PP Plus.
    let moves = pk.moves();
    let pp = pk.pp();
    let ups = pk.pp_ups();
    for i in 0..4 {
        if moves[i] == 0 {
            if pp[i] != 0 || ups[i] != 0 {
                out.fishy("pp-empty", "PP sur une case vide", "Une case d'attaque vide ne doit pas avoir de PP.", TAB_MOVES);
            }
            continue;
        }
        if ups[i] > 3 {
            out.bad("ppup", "Trop de PP Plus", format!("{} : {} PP Plus (3 au maximum).", move_name(moves[i]), ups[i]), TAB_MOVES);
        }
        if let Some(info) = dex::move_info_in(game, moves[i]) {
            let max = crate::save::session::max_pp(info.pp, ups[i]);
            if pk.is_egg() && (ups[i] != 0 || pp[i] != info.pp) {
                out.bad("egg-pp", "PP d'un œuf", "Un œuf a les PP de base de ses attaques, sans PP Plus.", TAB_MOVES);
            } else if pp[i] > max {
                let detail = format!("{} : {} PP pour {} au maximum.", move_name(moves[i]), pp[i], max);
                // Console virtuelle et transferts : les PP sont recopiés tels quels (les PP de base ont pu baisser).
                if ctx.origin_generation() < ctx.format {
                    out.fishy("pp-high-vc", "PP au-delà du maximum", detail, TAB_MOVES);
                } else {
                    out.bad("pp-high", "Trop de PP", detail, TAB_MOVES);
                }
            }
        }
    }

    // Objet tenu.
    let item = pk.held_item();
    if item != 0 && (item > dex::max_item(game) || dex::item_name_in(game, item).is_none()) {
        out.bad("item", "Objet inconnu", format!("L'objet n°{item} n'existe pas dans ce jeu."), TAB_OVERVIEW);
    }

    // Langue.
    let lang = pk.language();
    let max_lang = if ctx.format >= 7 { 10 } else { 8 };
    let bw_trade = lang == 0 && ctx.format == 5 && (20..=21).contains(&ctx.version) && pk.met_location() == 30002;
    if !bw_trade && (lang == 0 || lang == 6 || lang > max_lang) {
        out.bad("language", "Langue invalide", format!("Langue n°{lang}."), TAB_TRAINER);
    }

    // Nom du dresseur.
    if pk.ot_name().trim().is_empty() {
        out.bad("ot-empty", "Dresseur sans nom", "Le nom du dresseur d'origine est vide.", TAB_TRAINER);
    }

    // Pokérus.
    let (strain, days) = pk.pokerus();
    if strain == 0 && days > 0 {
        out.bad("pokerus", "Pokérus incohérent", "Des jours d'infection sans souche de virus.", TAB_EXTRAS);
    }
    if !pk.is_egg() && pk.met_date().is_none() && ctx.origin_generation() >= 4 {
        out.fishy("met-date", "Date de rencontre absente", "Les Pokémon capturés ont normalement une date.", TAB_MET);
    }
    // Dates hors de la fenêtre de sortie du jeu.
    let key = |d: PkmDate| (d.year, d.month, d.day);
    if let (Some(met), Some(first)) = (pk.met_date(), earliest_met_date(pk)) {
        if key(met) < key(first) {
            out.fishy(
                "met-date-early",
                "Date de rencontre trop ancienne",
                format!("Rencontre le {:02}/{:02}/{}, avant la sortie du jeu ({:02}/{:02}/{}).", met.day, met.month, met.year, first.day, first.month, first.year),
                TAB_MET,
            );
        }
        if let Some(egg) = pk.egg_date().filter(|_| pk.egg_location() != 0) {
            if key(egg) > key(met) {
                out.fishy("egg-date-late", "Œuf reçu après l'éclosion", "La date de l'œuf est postérieure à la date d'éclosion.", TAB_MET);
            }
        }
    }
}

/// Sortie japonaise d'une version (`GameVersion` de PKHeX).
pub(crate) fn release_date(version: u8) -> Option<PkmDate> {
    let (year, month, day) = match version {
        encounters::D | encounters::P => (2006, 9, 28),
        encounters::PT => (2008, 9, 13),
        encounters::HG | encounters::SS => (2009, 9, 12),
        encounters::B | encounters::W => (2010, 9, 18),
        encounters::B2 | encounters::W2 => (2012, 6, 23),
        encounters::X | encounters::Y => (2013, 10, 12),
        encounters::AS | encounters::OR => (2014, 11, 21),
        encounters::SN | encounters::MN => (2016, 11, 18),
        encounters::US | encounters::UM => (2017, 11, 17),
        _ => return None,
    };
    Some(PkmDate { year, month, day })
}

/// Première date de rencontre possible : sortie du jeu d'origine, ou du premier jeu qui
/// reçoit les transferts (Transfert Pokémon, Poké Transfert, Banque Pokémon).
pub(crate) fn earliest_met_date(pk: &Pokemon) -> Option<PkmDate> {
    let transfer = matches!(pk.met_location(), 30001 | 30002) && pk.format().generation() >= 5;
    if transfer {
        return Some(match pk.format().generation() {
            5 => PkmDate { year: 2010, month: 9, day: 18 },
            6 => PkmDate { year: 2013, month: 12, day: 25 },
            _ => PkmDate { year: 2017, month: 1, day: 24 },
        });
    }
    release_date(pk.version())
}

fn summarize(game_gen: u8, e: &Encounter) -> EncounterSummary {
    EncounterSummary {
        kind: e.kind,
        kind_label: e.kind.label(),
        species: e.species,
        species_name: species_name(e.species),
        location: e.location,
        location_name: (e.location != 0).then(|| location_name(game_gen, e.location)),
        level_min: e.level_min,
        level_max: e.level_max,
        versions: e.versions.iter().map(|&v| version_name(v)).collect(),
    }
}

/// Évalue une rencontre : (erreurs, avertissements, lignes).
fn evaluate(ctx: &Ctx, e: &Encounter) -> Lines {
    let mut lines = Lines::default();
    check_encounter(ctx, e, &mut lines);
    check_moves(ctx, Some(e), false, &mut lines);
    lines
}

/// Meilleure rencontre (moins d'erreurs, puis moins d'avertissements).
pub(crate) fn best_encounter(ctx: &Ctx) -> Option<(Encounter, Lines)> {
    candidates(ctx)
        .into_iter()
        .map(|e| {
            let lines = evaluate(ctx, &e);
            (e, lines)
        })
        .min_by_key(|(_, l)| (l.errors(), l.warnings()))
}

/// Analyse complète d'un Pokémon dans le contexte du jeu de la sauvegarde.
pub fn analyze(pk: &Pokemon, game: Game) -> Report {
    let mut out = Lines::default();
    if pk.is_empty() || pk.species() == 0 {
        out.bad("empty", "Emplacement vide", "Aucun Pokémon ici.", TAB_OVERVIEW);
        return finish(out, None, String::new(), None);
    }
    let ctx = match context(pk, game) {
        Ok(c) => c,
        Err(check) => {
            out.0.push(check);
            return finish(out, None, String::new(), None);
        }
    };
    let mut origin = match ctx.origin {
        Origin::Known { .. } => format!("{} ({})", version_name(ctx.version), if ctx.met_replaced { "transféré" } else { "d'origine" }),
        Origin::Gen3 => format!("{} (Gen 3, via le Pal Parc)", version_name(ctx.version)),
        Origin::VirtualConsole { generation } => format!("{} (Console virtuelle, Gen {generation})", version_name(ctx.version)),
    };
    check_general(&ctx, &mut out);
    extras::check_extras(&ctx, &mut out);

    let mut summary = None;
    let mut pid_type = None;
    match ctx.origin {
        Origin::Known { generation, .. } => {
            let best = best_encounter(&ctx);
            let event = event_marker(&ctx);
            match best {
                Some((e, lines)) if lines.errors() == 0 || !event || e.kind == EncounterKind::Event => {
                    out.ok("encounter", format!("Rencontre : {}", e.kind.label()), describe(&e, generation));
                    summary = Some(summarize(generation, &e));
                    out.0.extend(lines.0);
                }
                _ if event && !species_has_event(&ctx) => {
                    out.fishy(
                        "event",
                        "Distribution non vérifiable",
                        "Pokémon de distribution (Cadeau Mystère) absent de la base de PKHeX (distribution locale ?) : la rencontre n'est pas vérifiée.",
                        TAB_MET,
                    );
                    check_moves(&ctx, None, true, &mut out);
                    origin = format!("{origin} · distribution");
                }
                _ if event => {
                    out.bad(
                        "event-none",
                        "Aucune distribution ne correspond",
                        format!(
                            "Aucune distribution connue de {} n'a été reçue à « {} » au niveau {}.",
                            species_name(pk.species()),
                            location_name(generation, pk.met_location()),
                            pk.met_level()
                        ),
                        TAB_MET,
                    );
                    check_moves(&ctx, None, true, &mut out);
                }
                _ => {
                    out.bad(
                        "encounter-none",
                        "Aucune rencontre ne correspond",
                        format!(
                            "{} ne s'obtient pas à « {} » au niveau {} dans {}.",
                            species_name(pk.species()),
                            location_name(generation, pk.met_location()),
                            pk.met_level(),
                            version_name(ctx.version)
                        ),
                        TAB_MET,
                    );
                    check_moves(&ctx, None, false, &mut out);
                }
            }
            if generation == 4 && !pk.is_egg() {
                let pid = if ctx.format >= 6 { pk.encryption_constant() } else { ctx.pid };
                let ratio = ctx.info.as_ref().map_or(127, |i| i.gender_ratio);
                pid_type = Some(rng::analyze_gen34(pid, iv32(pk), ctx.tid, ctx.sid, ctx.shiny, gender_code(pk.gender()), ratio));
            }
        }
        Origin::Gen3 | Origin::VirtualConsole { .. } => {
            out.fishy(
                "origin-old",
                "Rencontre non vérifiable",
                format!("Pokémon venu de {} : Kaleido ne connaît pas les rencontres des Gen 1 à 3.", version_name(ctx.version)),
                TAB_MET,
            );
            let expected = match ctx.origin {
                Origin::Gen3 if ctx.format == 4 => Some(55),
                Origin::Gen3 => Some(30001),
                Origin::VirtualConsole { generation: 1 } => Some(30013),
                Origin::VirtualConsole { .. } => Some(30017),
                _ => None,
            };
            if let Some(loc) = expected {
                let celebi = matches!(pk.met_location(), 30010..=30013);
                if pk.met_location() != loc && !celebi {
                    out.bad(
                        "transfer-location",
                        "Lieu de transfert incorrect",
                        format!("Ce Pokémon doit avoir « {} » comme lieu de rencontre.", location_name(ctx.format.min(7), loc)),
                        TAB_MET,
                    );
                }
            }
            check_moves(&ctx, None, true, &mut out);
        }
    }
    finish(out, summary, origin, pid_type)
}

fn describe(e: &Encounter, generation: u8) -> String {
    let lvl = if e.level_min == e.level_max { format!("niv. {}", e.level_min) } else { format!("niv. {} à {}", e.level_min, e.level_max) };
    let place = if e.location != 0 { format!(" à « {} »", location_name(generation, e.location)) } else { String::new() };
    let title = e.title.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| format!(" Carte : « {t} ».")).unwrap_or_default();
    format!("{}{place}, {lvl}.{title}", species_name(e.species))
}

fn finish(mut out: Lines, encounter: Option<EncounterSummary>, origin: String, pid_type: Option<PidType>) -> Report {
    // Messages identiques regroupés.
    let mut seen = std::collections::HashSet::new();
    out.0.retain(|c| seen.insert((c.code, c.title.clone(), c.detail.clone())));
    let errors = out.errors();
    let warnings = out.warnings();
    let verdict = if errors > 0 {
        Verdict::Illegal
    } else if warnings > 0 {
        Verdict::Fishy
    } else {
        Verdict::Legal
    };
    out.0.sort_by_key(|c| std::cmp::Reverse(c.severity));
    Report { verdict, verdict_label: verdict.label(), checks: out.0, encounter, origin, pid_type, errors, warnings }
}
