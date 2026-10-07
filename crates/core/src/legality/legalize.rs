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
use crate::save::pkm::{ExtrasPatch, Memory};
use crate::save::{exp_for_level, Gender, PkmDate, PkmFormat, Pokemon, Trainer};

/// Une modification faite par « Rendre légal », reliée à un terme du glossaire de
/// l'interface (`app/src/glossary.ts`) pour la bulle « i ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub text: String,
    pub term: &'static str,
}

impl Change {
    fn new(text: String) -> Self {
        Change { term: term_of(&text), text }
    }
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// Terme du glossaire qui explique une modification (d'après le début du texte).
pub fn term_of(text: &str) -> &'static str {
    const TABLE: &[(&str, &str)] = &[
        ("Forme", "form"),
        ("Niveau", "level"),
        ("Jeu d'origine", "version"),
        ("Langue", "language"),
        ("Dresseur", "ot"),
        ("Rencontre fatidique", "fateful"),
        ("Rencontre : œuf", "eggOrigin"),
        ("Rencontre", "encounter"),
        ("Date", "metDate"),
        ("Souvenir", "memories"),
        ("Pays", "country"),
        ("Ball", "ball"),
        ("Chromatique", "shiny"),
        ("IV recalculés (méthode J", "methodJK"),
        ("IV recalculés (Poké Radar", "pokeRadar"),
        ("IV recalculés", "pidiv"),
        ("IV", "iv"),
        ("PID et constante", "transfer"),
        ("PID régénéré en méthode J", "methodJK"),
        ("PID régénéré en méthode K", "methodJK"),
        ("PID régénéré (Poké Radar", "pokeRadar"),
        ("PID régénéré en méthode 1", "pidiv"),
        ("PID", "pid"),
        ("Constante de chiffrement", "ec"),
        ("Talent", "ability"),
        ("Nature", "nature"),
        ("Sexe", "gender"),
        ("Attaques à réapprendre", "relearn"),
        ("Attaque Œuf", "eggMoves"),
        ("Attaques", "moves"),
        ("PP", "pp"),
        ("EV", "ev"),
        ("Objet", "heldItem"),
        ("Pokérus", "pokerus"),
        ("Surnom", "nickname"),
        ("Rubans", "ribbons"),
        ("Concours", "contest"),
        ("Hyper Training", "hyperTraining"),
        ("Super Training", "superTraining"),
        ("Soigneur", "handler"),
        ("Feuilles", "shinyLeaf"),
    ];
    TABLE.iter().find(|(p, _)| text.starts_with(p)).map_or("legalize", |(_, t)| t)
}

/// Rencontre retenue (ou proposée) par « Rendre légal ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterOption {
    /// Numéro du plan, à renvoyer pour choisir cette rencontre.
    pub id: usize,
    pub kind_label: String,
    pub family: &'static str,
    pub species: u16,
    pub species_name: String,
    pub location: u16,
    pub location_name: String,
    pub level_min: u8,
    pub level_max: u8,
    pub version: u8,
    pub version_name: String,
    pub generation: u8,
}

/// Résultat de « Rendre légal ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalizeOutcome {
    #[serde(skip)]
    pub pokemon: Pokemon,
    /// Modifications faites, en français.
    pub changes: Vec<Change>,
    pub report: Report,
    /// Le Pokémon est désormais légal (ou seulement douteux).
    pub success: bool,
    /// Rencontre retenue (`None` : rencontre actuelle gardée).
    pub encounter: Option<EncounterOption>,
    /// Autres rencontres qui donnent aussi un Pokémon légal (aperçu seulement), la
    /// rencontre retenue comprise, de la plus naturelle à la plus exotique.
    pub options: Vec<EncounterOption>,
}

impl LegalizeOutcome {
    fn plain(pokemon: Pokemon, changes: Vec<String>, report: Report, success: bool) -> Self {
        LegalizeOutcome { pokemon, changes: changes.into_iter().map(Change::new).collect(), report, success, encounter: None, options: Vec::new() }
    }
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
        s if form != 0
            && [
                3, 6, 9, 65, 94, 115, 127, 130, 142, 150, 181, 212, 214, 229, 248, 257, 282, 303, 306, 308, 310, 354, 359, 380, 381, 445, 448, 460,
                15, 18, 80, 208, 254, 260, 302, 319, 323, 334, 362, 373, 376, 384, 428, 475, 531, 719,
            ]
            .contains(&s) =>
        {
            0
        }
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
    if let Some(l) = e.language {
        if p.language() != l {
            p.set_language(l);
            changes.push("Langue de la distribution".into());
        }
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
    fix_dates(&mut p, changes);
    // Souvenir avec le dresseur d'origine : toujours en Gen 6 (PKHeX `SetRandomMemory6` /
    // `SetHatchMemory6`), jamais sinon. Les cadeaux gardent celui de la distribution.
    if fgen >= 6 && !e.fateful {
        if let Some(h) = p.extras().handler {
            let want = if origin_gen != 6 || p.is_egg() {
                Memory::default()
            } else if e.is_egg() {
                // « … a brisé la coquille de son Œuf » : Route 7 (X/Y) ou Spot Combat (ROSA).
                Memory { id: 2, intensity: 1, feeling: 0, variable: if matches!(plan.version, 24 | 25) { 43 } else { 27 } }
            } else if h.ot_memory.id != 0 {
                h.ot_memory
            } else {
                // « … a failli se perdre en explorant une forêt » : ressenti 1 autorisé pour ce souvenir.
                Memory { id: 63, intensity: 7, feeling: 1, variable: 0 }
            };
            if want != h.ot_memory {
                let _ = p.apply_extras(&ExtrasPatch { ot_memory: Some(want), ..Default::default() });
                changes.push(if want.id == 0 { "Souvenir du dresseur d'origine retiré".into() } else { "Souvenir du dresseur d'origine ajouté".to_string() });
            }
        }
    }
    if p.fateful_encounter() != e.fateful {
        p.set_fateful_encounter(e.fateful);
        changes.push(if e.fateful { "Rencontre fatidique activée".into() } else { "Rencontre fatidique retirée".to_string() });
    }

    // Ball.
    let ball = if ball_allowed(e, plan.version, wishes.ball) { wishes.ball } else { e.ball.unwrap_or(4) };
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
        let wish = PidWish {
            nature: (origin_gen <= 4).then_some(nature),
            shiny: Some(shiny),
            ability_bit: (ability_number != 4).then_some(ability_bit),
            gender: Some((gender_wish, ratio)),
        };
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
                if let Some(fixed) = e.ec {
                    ec = fixed;
                } else if ec == 0 || ec == pid {
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
    fix_evs(&mut p, game, changes);

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
    // Attaques à réapprendre impossibles pour la rencontre : remises à zéro d'abord, sinon
    // elles feraient passer pour apprises les attaques qu'elles contiennent.
    if p.format().generation() >= 6 {
        if let (Some(e), Ok(ctx)) = (enc, verify::context(pk, game)) {
            if !verify::relearn_valid(&ctx, e) {
                let mut start = [0u16; 4];
                if ctx.origin_generation() >= 6 {
                    for (i, &m) in e.relearn.iter().take(4).enumerate() {
                        start[i] = m;
                    }
                }
                p = set_relearn(&p, start);
            }
        }
    }
    let base = p.clone();
    let Ok(ctx) = verify::context(&base, game) else { return p };
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
        let keep = enc.is_some_and(|e| {
            let now = p.clone();
            verify::context(&now, game).is_ok_and(|c| verify::relearn_valid(&c, e))
        });
        if verify::relearn_moves(&p) != relearn && !keep {
            p = set_relearn(&p, relearn);
        }
        if verify::relearn_moves(&p) != verify::relearn_moves(pk) {
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

/// Codes de vérification corrigés sans changer de rencontre ([`fix_extras`]).
const LOCAL_CODES: &[&str] = &[
    "form-gender",
    "nickname-length",
    "contest",
    "vc-contest",
    "ribbon",
    "ribbon-memory",
    "ribbon-order",
    "ribbon-egg",
    "hyper-level",
    "hyper-perfect",
    "super-training",
    "geo",
    "handler-current",
    "handler-data",
    "memory-id",
    "memory-ot",
    "shiny-leaf",
    "pokerus",
    "item",
    "ev-total",
    "ev-252",
    "egg-evs",
    "ev-untrained",
    "met-date",
];

fn invalid_codes(report: &Report) -> Vec<&'static str> {
    report.checks.iter().filter(|c| c.severity == verify::Severity::Invalid).map(|c| c.code).collect()
}

/// EV dans les limites (510 au total, 252 par statistique en Gen 6+, aucun sur un œuf,
/// 100 au plus avant tout combat en Gen 4).
fn fix_evs(p: &mut Pokemon, game: Game, changes: &mut Vec<String>) {
    let fgen = p.format().generation();
    let mut evs = p.evs();
    if p.is_egg() {
        evs = [0; 6];
    }
    if fgen >= 6 {
        for v in evs.iter_mut() {
            *v = (*v).min(252);
        }
    }
    if fgen == 4 && verify::growth_level(p, game) <= p.met_level() {
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
}

/// Dates de rencontre et d'œuf dans la fenêtre de sortie du jeu (et l'œuf avant l'éclosion).
fn fix_dates(p: &mut Pokemon, changes: &mut Vec<String>) {
    if p.is_egg() {
        return;
    }
    let key = |d: PkmDate| (d.year, d.month, d.day);
    let now = today();
    let first = verify::earliest_met_date(p);
    let met = p.met_date();
    let bad = |d: PkmDate| first.is_some_and(|f| key(d) < key(f)) || key(d) > key(now);
    match met {
        None => p.set_met_date(Some(now)),
        Some(d) if bad(d) => {
            p.set_met_date(Some(now));
            changes.push("Date de rencontre ramenée dans la période du jeu".into());
        }
        _ => {}
    }
    if p.egg_location() != 0 {
        let met = p.met_date().unwrap_or(now);
        if let Some(egg) = p.egg_date() {
            if key(egg) > key(met) || bad(egg) {
                p.set_egg_date(Some(met));
                changes.push("Date de l'œuf alignée sur l'éclosion".into());
            }
        }
    }
}

fn ribbon_errors(p: &Pokemon, game: Game) -> usize {
    invalid_codes(&analyze(p, game)).iter().filter(|c| c.starts_with("ribbon")).count()
}

/// Corrige les champs secondaires signalés par `report` (rubans, concours, souvenirs,
/// surnom, forme liée au sexe…) sans toucher à la rencontre.
fn fix_extras(pk: &Pokemon, game: Game, report: &Report, changes: &mut Vec<String>) -> Pokemon {
    let codes = invalid_codes(report);
    let has = |c: &str| codes.contains(&c);
    let mut p = pk.clone();
    let patch = |p: &mut Pokemon, x: ExtrasPatch| {
        let _ = p.apply_extras(&x);
    };
    if has("form-gender") && p.species() == 678 {
        let _ = p.set_form((p.gender() == Gender::Female) as u8);
        changes.push("Forme accordée au sexe".into());
    }
    if has("nickname-length") {
        let _ = p.set_nickname(dex::species_name(p.species()).unwrap_or("?"));
        p.set_is_nicknamed(false);
        changes.push("Surnom trop long remplacé par le nom de l'espèce".into());
    }
    if has("contest") || has("vc-contest") {
        p.set_contest_stats([0; 6]);
        changes.push("Concours : caractéristiques remises à 0".into());
    }
    if has("ribbon-memory") && p.format().generation() >= 6 {
        p = raw(&p, |d| {
            d[0x38] = 0;
            d[0x39] = 0;
        });
        changes.push("Rubans mémoire retirés".into());
    }
    if codes.iter().any(|c| c.starts_with("ribbon")) {
        let on: Vec<String> = p.extras().ribbons.iter().filter(|r| r.on).map(|r| r.key.to_string()).collect();
        let mut removed = 0;
        if has("ribbon-egg") {
            patch(&mut p, ExtrasPatch { ribbons: Some(on.iter().map(|k| (k.clone(), false)).collect()), ..Default::default() });
            removed = on.len();
        } else {
            // Retire un à un les rubans dont l'absence fait baisser le nombre d'erreurs.
            let mut current = ribbon_errors(&p, game);
            for key in on.iter().rev() {
                if current == 0 {
                    break;
                }
                let mut q = p.clone();
                patch(&mut q, ExtrasPatch { ribbons: Some([(key.clone(), false)].into()), ..Default::default() });
                q.refresh_checksum();
                let n = ribbon_errors(&q, game);
                if n < current {
                    p = q;
                    current = n;
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            changes.push(format!("Rubans impossibles retirés ({removed})"));
        }
    }
    if has("hyper-level") || has("hyper-perfect") {
        patch(&mut p, ExtrasPatch { hyper_training: Some([false; 6]), ..Default::default() });
        changes.push("Hyper Training retiré".into());
    }
    if has("super-training") {
        patch(&mut p, ExtrasPatch { medals: Some(Vec::new()), secret_unlocked: Some(false), supremely_trained: Some(false), ..Default::default() });
        changes.push("Super Training : médailles retirées".into());
    }
    if let Some(h) = p.extras().handler {
        if has("geo") {
            let mut geo = [[0u8; 2]; 5];
            for (i, g) in h.geo.iter().filter(|g| g[1] != 0).enumerate() {
                geo[i] = *g;
            }
            patch(&mut p, ExtrasPatch { geo: Some(geo), ..Default::default() });
            changes.push("Pays visités remis en ordre".into());
        }
        if has("handler-current") || has("handler-data") {
            patch(
                &mut p,
                ExtrasPatch {
                    current_handler: Some(0),
                    ht_friendship: Some(0),
                    ht_affection: Some(0),
                    ht_memory: Some(Memory::default()),
                    ..Default::default()
                },
            );
            changes.push("Soigneur : données orphelines retirées".into());
        }
        if has("memory-ot") || has("memory-id") {
            let texts = dex::memory_texts().len();
            let ot = if has("memory-ot") || h.ot_memory.id as usize >= texts { Memory::default() } else { h.ot_memory };
            let ht = if h.ht_memory.id as usize >= texts { Memory::default() } else { h.ht_memory };
            patch(&mut p, ExtrasPatch { ot_memory: Some(ot), ht_memory: Some(ht), ..Default::default() });
            changes.push("Souvenir impossible retiré".into());
        }
    }
    if has("shiny-leaf") {
        if let Some(v) = p.extras().shiny_leaf {
            patch(&mut p, ExtrasPatch { shiny_leaf: Some(v & 0x1F), ..Default::default() });
            changes.push("Feuilles brillantes : couronne retirée".into());
        }
    }
    if has("pokerus") {
        p.set_pokerus(0, 0);
        changes.push("Pokérus incohérent retiré".into());
    }
    if has("item") {
        p.set_held_item(0);
        changes.push("Objet impossible retiré".into());
    }
    if has("ev-total") || has("ev-252") || has("egg-evs") || has("ev-untrained") {
        fix_evs(&mut p, game, changes);
    }
    if has("met-date") {
        fix_dates(&mut p, changes);
    }
    p.refresh_checksum();
    p
}

/// Nombre maximal de rencontres essayées.
const MAX_PLANS: usize = 24;
/// Nombre maximal de rencontres proposées dans l'aperçu.
const MAX_OPTIONS: usize = 8;

fn option_of(id: usize, plan: &Plan) -> EncounterOption {
    let e = &plan.enc;
    let location_name = if e.is_egg() && e.location == 0 {
        "Pension".to_string()
    } else {
        dex::location_name(e.generation, e.location).unwrap_or("?").to_string()
    };
    EncounterOption {
        id,
        kind_label: e.kind.label().to_string(),
        family: e.kind.family(),
        species: e.species,
        species_name: dex::species_name(e.species).unwrap_or("?").to_string(),
        location: e.location,
        location_name,
        level_min: e.level_min,
        level_max: e.level_max,
        version: plan.version,
        version_name: dex::game_name(plan.version).unwrap_or("?").to_string(),
        generation: e.generation,
    }
}

/// Deux rencontres équivalentes pour l'utilisateur (même type, lieu, version et espèce).
fn same_option(a: &EncounterOption, b: &EncounterOption) -> bool {
    a.kind_label == b.kind_label && a.location == b.location && a.version == b.version && a.species == b.species
}

/// Applique une rencontre, puis corrige les champs secondaires si besoin.
fn try_plan(pk: &Pokemon, game: Game, trainer: &Trainer, plan: &Plan, wishes: Wishes, seed: u64) -> (Pokemon, Vec<String>, Report) {
    let mut rand = Rand::new(seed);
    let mut changes = Vec::new();
    let mut p = apply(pk, game, trainer, plan, wishes, &mut rand, &mut changes);
    let mut report = analyze(&p, game);
    if report.verdict == Verdict::Illegal && invalid_codes(&report).iter().any(|c| LOCAL_CODES.contains(c)) {
        p = fix_extras(&p, game, &report, &mut changes);
        report = analyze(&p, game);
    }
    #[cfg(test)]
    if std::env::var("KALEIDO_DEBUG_PLANS").is_ok() && report.verdict == Verdict::Illegal {
        let bad: Vec<String> =
            report.checks.iter().filter(|c| c.severity == verify::Severity::Invalid).map(|c| format!("{} — {}", c.title, c.detail)).collect();
        println!("  essai {:?} n°{} lieu {} : {bad:?}", plan.enc.kind, plan.enc.species, plan.enc.location);
    }
    (p, changes, report)
}

/// Essaie les rencontres dans l'ordre. `choice` : rencontre imposée (numéro de plan) ;
/// `explore` : continue après la première réussite pour lister les autres rencontres valides.
#[allow(clippy::too_many_arguments)]
fn run(pk: &Pokemon, game: Game, trainer: &Trainer, plans: &[Plan], wishes: Wishes, seed: u64, choice: Option<usize>, explore: bool) -> LegalizeOutcome {
    let plan_seed = |id: usize| seed ^ (id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut chosen: Option<LegalizeOutcome> = None;
    let mut best: Option<LegalizeOutcome> = None;
    if let Some(id) = choice.filter(|&i| i < plans.len()) {
        let (p, changes, report) = try_plan(pk, game, trainer, &plans[id], wishes, plan_seed(id));
        let ok = report.verdict != Verdict::Illegal;
        if ok {
            let mut out = LegalizeOutcome::plain(p, changes, report, true);
            out.encounter = Some(option_of(id, &plans[id]));
            chosen = Some(out);
        }
    }
    let mut options: Vec<EncounterOption> = Vec::new();
    let mut tried: Vec<EncounterOption> = Vec::new();
    for (id, plan) in plans.iter().enumerate().take(MAX_PLANS) {
        if chosen.is_some() && (!explore || options.len() >= MAX_OPTIONS) {
            break;
        }
        let opt = option_of(id, plan);
        if tried.iter().any(|o| same_option(o, &opt)) {
            continue;
        }
        tried.push(opt.clone());
        if Some(id) == choice && chosen.is_some() {
            options.push(opt);
            continue;
        }
        let (p, changes, report) = try_plan(pk, game, trainer, plan, wishes, plan_seed(id));
        if report.verdict != Verdict::Illegal {
            options.push(opt.clone());
            if chosen.is_none() {
                let mut out = LegalizeOutcome::plain(p, changes, report, true);
                out.encounter = Some(opt);
                chosen = Some(out);
            }
        } else if chosen.is_none() && best.as_ref().is_none_or(|b| report.errors < b.report.errors) {
            let mut out = LegalizeOutcome::plain(p, changes, report, false);
            out.encounter = Some(opt);
            best = Some(out);
        }
    }
    let mut out = chosen.or(best).unwrap_or_else(|| LegalizeOutcome::plain(pk.clone(), Vec::new(), analyze(pk, game), false));
    if explore {
        out.options = options;
    }
    out
}

/// Graine stable d'un Pokémon : l'aperçu et l'application donnent le même résultat.
fn stable_seed(pk: &Pokemon) -> u64 {
    pk.encryption_constant() as u64 ^ (pk.pid() as u64) << 32 ^ 0xA11E_6A1E
}

/// Rencontres envisagées pour un Pokémon de la sauvegarde (rencontre actuelle d'abord).
fn plans_for(pk: &Pokemon, game: Game) -> Vec<Plan> {
    let level = verify::growth_level(pk, game);
    let prefer = game_versions(game).first().copied().unwrap_or(0);
    let prefer = if encounters::version_game(pk.version()) == Some(game) { pk.version() } else { prefer };
    let mut list: Vec<Plan> = plan_for_current(pk, game).into_iter().collect();
    list.extend(plans(game, pk.species(), pk.form(), level, prefer));
    list
}

/// Corrections sur place (attaques, champs secondaires) quand la rencontre convient.
fn local_fix(pk: &Pokemon, game: Game, before: &Report) -> Option<LegalizeOutcome> {
    let local = before.checks.iter().filter(|c| c.severity == verify::Severity::Invalid).all(|c| c.tab == Some("moves") || LOCAL_CODES.contains(&c.code));
    if !local {
        return None;
    }
    let mut changes = Vec::new();
    let mut p = pk.clone();
    if before.checks.iter().any(|c| c.severity == verify::Severity::Invalid && c.tab == Some("moves")) {
        let enc = plan_for_current(pk, game).map(|p| p.enc);
        p = fix_moves(&p, game, enc.as_ref(), &mut changes);
    }
    let mid = analyze(&p, game);
    if mid.verdict == Verdict::Illegal {
        p = fix_extras(&p, game, &mid, &mut changes);
    }
    let report = analyze(&p, game);
    (report.verdict != Verdict::Illegal).then(|| LegalizeOutcome::plain(p, changes, report, true))
}

/// « Rendre légal » : garde la rencontre actuelle si elle convient, sinon en choisit une
/// dans le jeu de la sauvegarde (ou un jeu plus ancien transférable).
pub fn legalize(pk: &Pokemon, game: Game, trainer: &Trainer) -> LegalizeOutcome {
    legalize_with(pk, game, trainer, None, false)
}

/// « Rendre légal » avec choix de la rencontre (`choice` : `id` d'une des `options` d'un
/// aperçu) et, si `explore`, la liste des rencontres possibles. Le résultat ne dépend que
/// du Pokémon et du choix : l'aperçu et l'application donnent le même Pokémon.
pub fn legalize_with(pk: &Pokemon, game: Game, trainer: &Trainer, choice: Option<usize>, explore: bool) -> LegalizeOutcome {
    let before = analyze(pk, game);
    if before.verdict != Verdict::Illegal {
        return LegalizeOutcome::plain(pk.clone(), Vec::new(), before, true);
    }
    let list = plans_for(pk, game);
    let seed = stable_seed(pk);
    if choice.is_none() {
        if let Some(mut out) = local_fix(pk, game, &before) {
            if explore {
                out.options = run(pk, game, trainer, &list, wishes_of(pk), seed, None, true).options;
            }
            return out;
        }
    }
    run(pk, game, trainer, &list, wishes_of(pk), seed, choice, explore)
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
            if let Some(e) = super::db::encounter_by_index(eg, i) {
                let mine = game_versions(eg);
                let version = e.versions.iter().copied().find(|v| mine.contains(v)).or(e.versions.first().copied()).unwrap_or(prefer);
                list.push(Plan { enc: e, version });
            }
        }
    }
    list.extend(plans(game, species, req.form, level, prefer));
    if list.is_empty() {
        return Err(format!("{name} ne s'obtient pas dans ce jeu (ni par capture, ni par reproduction)"));
    }
    let seed = rand.next_u32() as u64 | (rand.next_u32() as u64) << 32;
    let mut out = run(&p, game, trainer, &list, wishes, seed, None, false);
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
