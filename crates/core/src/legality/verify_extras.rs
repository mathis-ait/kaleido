//! Vérification des champs secondaires : rubans, concours, Hyper Training, soigneur, pays
//! visités, souvenirs, Super Training et feuilles brillantes.
//!
//! Sous-ensemble des règles de PKHeX (`RibbonVerifierCommon3/4/6/7`, `RibbonVerifierUnique3/4`,
//! `RibbonRules`, `ContestStatVerifier`, `HyperTrainingVerifier`, `MemoryVerifier`,
//! `GeoTrackVerifier`) : seules les règles sûres sont reprises, pour éviter les faux positifs.
//! Les rubans d'événement ne sont pas comparés aux distributions.

use std::collections::HashSet;

use super::{species_name, Ctx, Lines, TAB_STATS};
use crate::save::PkmFormat;

pub(super) const TAB_RIBBONS: &str = "ribbons";
pub(super) const TAB_MEMORIES: &str = "memories";
const TAB_TRAINER: &str = super::TAB_TRAINER;
const TAB_EXTRAS: &str = super::TAB_EXTRAS;

/// Espèces interdites à la Tour de Combat / Maison de Combat / Arbre de Combat
/// (`RibbonRules.BattleFrontierBanlist`, jusqu'à la Gen 7).
const FRONTIER_BANLIST: [u16; 39] = [
    150, 151, 249, 250, 251, 382, 383, 384, 385, 386, 483, 484, 487, 489, 490, 491, 492, 493, 494, 643, 644, 646, 647, 648, 649, 716, 717, 718,
    719, 720, 721, 789, 790, 791, 792, 800, 801, 802, 807,
];

/// Espèce (ou une de ses pré-évolutions) interdite dans les installations de combat.
fn is_banned(ctx: &Ctx) -> bool {
    FRONTIER_BANLIST.contains(&ctx.pk.species()) || ctx.chain.iter().any(|s| FRONTIER_BANLIST.contains(&s.species))
}

/// Générations traversées, d'après l'origine et le format actuel.
struct Visited {
    g3: bool,
    g4: bool,
    g6: bool,
    g7: bool,
}

fn visited(ctx: &Ctx) -> Visited {
    let og = ctx.origin_generation();
    let f = ctx.format;
    Visited { g3: og == 3, g4: og == 3 || og == 4, g6: (3..=6).contains(&og) && f >= 6, g7: f >= 7 }
}

const G3_CONTEST: [&str; 5] = ["Cool", "Beauty", "Cute", "Smart", "Tough"];
const DAILY: [&str; 7] = ["RibbonAlert", "RibbonShock", "RibbonDowncast", "RibbonCareless", "RibbonRelax", "RibbonSnooze", "RibbonSmile"];
const ABILITY: [&str; 6] = ["RibbonAbility", "RibbonAbilityGreat", "RibbonAbilityDouble", "RibbonAbilityMulti", "RibbonAbilityPair", "RibbonAbilityWorld"];
const ORAS_CONTEST: [&str; 6] =
    ["RibbonContestStar", "RibbonMasterCoolness", "RibbonMasterBeauty", "RibbonMasterCuteness", "RibbonMasterCleverness", "RibbonMasterToughness"];

pub(super) fn check_extras(ctx: &Ctx, out: &mut Lines) {
    check_ribbons(ctx, out);
    check_memory_ribbons(ctx, out);
    check_contest(ctx, out);
    check_hyper_training(ctx, out);
    check_handler(ctx, out);
    check_memories(ctx, out);
    check_super_training(ctx, out);
    check_shiny_leaf(ctx, out);
}

fn check_ribbons(ctx: &Ctx, out: &mut Lines) {
    let pk = ctx.pk;
    let x = pk.extras();
    let on: HashSet<&str> = x.ribbons.iter().filter(|r| r.on).map(|r| r.key).collect();
    if on.is_empty() {
        return;
    }
    let name = |key: &str| x.ribbons.iter().find(|r| r.key == key).map_or(key, |r| r.name).to_string();
    if pk.is_egg() {
        out.bad("ribbon-egg", "Rubans sur un œuf", format!("Un œuf ne peut porter aucun ruban ({} posé(s)).", on.len()), TAB_RIBBONS);
        return;
    }
    let v = visited(ctx);
    let species = pk.species();
    let banned = is_banned(ctx);
    let level_gain = ctx.level.saturating_sub(pk.met_level());

    let mut bad: Vec<(String, &'static str)> = Vec::new();
    let mut flag = |key: &str, ok: bool, why: &'static str| {
        if on.contains(key) && !ok {
            bad.push((name(key), why));
        }
    };

    // Gen 3.
    flag("RibbonChampionG3", v.g3, "s'obtient seulement en Gen 3");
    flag("RibbonArtist", v.g3, "s'obtient seulement en Gen 3");
    flag("RibbonWinning", v.g3 && !banned, "Tour de Combat de la Gen 3 (espèces autorisées)");
    flag("RibbonVictory", v.g3 && !banned, "Tour de Combat de la Gen 3 (espèces autorisées)");
    for c in G3_CONTEST {
        for tier in ["", "Super", "Hyper", "Master"] {
            flag(&format!("RibbonG3{c}{tier}"), v.g3, "concours de la Gen 3");
        }
    }
    // Effort : partout sauf en Gen 5.
    flag("RibbonEffort", v.g3 || v.g4 || v.g6 || v.g7, "pas disponible en Gen 5");
    // Empreinte : bonheur maximal en Gen 3/4, ou 30 niveaux gagnés depuis la rencontre en Gen 6/7.
    flag("RibbonFootprint", v.g3 || v.g4 || (ctx.format >= 6 && level_gain >= 30), "Gen 3/4, ou +30 niveaux depuis la rencontre en Gen 6/7");
    flag("RibbonRecord", false, "jamais distribué");
    // Gen 4.
    flag("RibbonLegend", v.g4, "s'obtient seulement en Gen 4");
    flag("RibbonChampionSinnoh", v.g4, "s'obtient seulement en Gen 4");
    for key in DAILY {
        flag(key, v.g4 || v.g6, "Gen 4 ou ROSA");
    }
    for key in ["RibbonGorgeous", "RibbonRoyal", "RibbonGorgeousRoyal"] {
        flag(key, v.g4 || v.g6, "Gen 4 ou ROSA");
    }
    for key in ABILITY {
        flag(key, v.g4 && !banned, "Tour de Combat de la Gen 4 (espèces autorisées)");
    }
    let contest4 = v.g4 && !matches!(species, 132 | 201);
    for c in G3_CONTEST {
        for tier in ["", "Great", "Ultra", "Master"] {
            flag(&format!("RibbonG4{c}{tier}"), contest4, "concours de la Gen 4 (sauf Métamorph et Zarbi)");
        }
    }
    // Gen 6.
    flag("RibbonChampionKalos", v.g6, "s'obtient seulement en X/Y");
    flag("RibbonChampionG6Hoenn", v.g6, "s'obtient seulement en ROSA");
    for key in ORAS_CONTEST {
        flag(key, v.g6, "concours de ROSA");
    }
    flag("RibbonTraining", v.g6, "Super Training (Gen 6)");
    flag("RibbonBattlerSkillful", v.g6 && !banned, "Maison de Combat (espèces autorisées)");
    flag("RibbonBattlerExpert", v.g6 && !banned, "Maison de Combat (espèces autorisées)");
    // Affection au maximum (non échangé : l'affection actuelle doit être de 255).
    if let Some(h) = &x.handler {
        flag("RibbonBestFriends", !h.name.is_empty() || h.ot_affection == 255, "affection au maximum (255)");
    }
    // Gen 7.
    flag("RibbonChampionAlola", v.g7, "s'obtient seulement en Gen 7");
    flag("RibbonBattleRoyale", v.g7 && !banned, "Battle Royal (espèces autorisées)");
    flag("RibbonBattleTreeGreat", v.g7, "Arbre de Combat");
    flag("RibbonBattleTreeMaster", v.g7 && !banned, "Arbre de Combat (espèces autorisées)");

    // Rubans de concours : chaque rang demande le précédent.
    let mut missing: Vec<String> = Vec::new();
    for (gen, tiers) in [("G3", ["", "Super", "Hyper", "Master"]), ("G4", ["", "Great", "Ultra", "Master"])] {
        for c in G3_CONTEST {
            let keys: Vec<String> = tiers.iter().map(|t| format!("Ribbon{gen}{c}{t}")).collect();
            if let Some(top) = keys.iter().rposition(|k| on.contains(k.as_str())) {
                missing.extend(keys[..top].iter().filter(|k| !on.contains(k.as_str())).map(|k| name(k)));
            }
        }
    }
    if on.contains("RibbonGorgeousRoyal") && !(on.contains("RibbonGorgeous") && on.contains("RibbonRoyal")) && !v.g6 {
        missing.push(format!("{} et {}", name("RibbonGorgeous"), name("RibbonRoyal")));
    }

    for (n, why) in &bad {
        out.bad("ribbon", format!("Ruban impossible : {n}"), format!("Ce ruban {why} : impossible pour ce Pokémon (origine et jeux traversés)."), TAB_RIBBONS);
    }
    if !missing.is_empty() {
        out.bad("ribbon-order", "Rubans manquants", format!("Il manque les rubans qui précèdent : {}.", missing.join(", ")), TAB_RIBBONS);
    }
    if bad.is_empty() && missing.is_empty() {
        out.ok("ribbons", "Rubans", format!("{} ruban(s), compatibles avec les jeux traversés.", on.len()));
    }
}

/// Rubans « mémoire » Gen 6/7 (0x38 concours, 0x39 combat) : nombre de rubans de Gen 3/4
/// regroupés au transfert (PKHeX `RibbonRules.GetMaxMemoryCounts`).
fn check_memory_ribbons(ctx: &Ctx, out: &mut Lines) {
    if ctx.format < 6 {
        return;
    }
    let d = ctx.pk.data();
    let (contest, battle) = (d[0x38], d[0x39]);
    if contest == 0 && battle == 0 {
        return;
    }
    let banned = is_banned(ctx);
    let v = visited(ctx);
    let species = ctx.pk.species();
    let contest4 = !matches!(species, 132 | 201);
    let (max_contest, max_battle) = if v.g3 {
        // Ribbon Winning : impossible pour les Pokémon Obscurs de niveau 55 (Tyranitar, Dracolosse).
        let winning = !(ctx.version == 15 && matches!(species, 149 | 248));
        (if contest4 { 40 } else { 20 }, if banned { 0 } else if winning { 8 } else { 7 })
    } else if v.g4 {
        (if contest4 { 20 } else { 0 }, if banned { 0 } else { 6 })
    } else {
        (0, 0)
    };
    if contest > max_contest {
        out.bad("ribbon-memory", "Ruban mémoire (concours)", format!("{contest} rubans de concours regroupés, au plus {max_contest} pour ce Pokémon."), TAB_RIBBONS);
    }
    if battle > max_battle {
        out.bad("ribbon-memory", "Ruban mémoire (combat)", format!("{battle} rubans de combat regroupés, au plus {max_battle} pour ce Pokémon."), TAB_RIBBONS);
    }
}

/// Concours : Gen 3, Gen 4 (Poffins) ou ROSA (Pokébloc) uniquement.
fn check_contest(ctx: &Ctx, out: &mut Lines) {
    let v = visited(ctx);
    let c = ctx.pk.contest_stats();
    if c.iter().any(|&s| s != 0) && !(v.g3 || v.g4 || v.g6) {
        out.bad(
            "contest",
            "Concours impossibles",
            "Les caractéristiques de concours ne montent qu'en Gen 3, en Gen 4 ou dans ROSA : elles doivent rester à 0.",
            TAB_EXTRAS,
        );
    }
}

/// Hyper Training : niveau 100 et IV pas déjà à 31.
fn check_hyper_training(ctx: &Ctx, out: &mut Lines) {
    let Some(ht) = ctx.pk.extras().hyper_training else { return };
    if !ht.iter().any(|&b| b) {
        return;
    }
    if ctx.level < 100 {
        out.bad("hyper-level", "Hyper Training trop tôt", "M. Hyper n'entraîne que les Pokémon de niveau 100.", TAB_STATS);
    }
    let ivs = ctx.pk.ivs();
    if ht.iter().zip(ivs).any(|(&on, iv)| on && iv == 31) {
        out.bad("hyper-perfect", "Hyper Training inutile", "Une statistique dont l'IV vaut déjà 31 ne peut pas être entraînée.", TAB_STATS);
    }
}

/// Soigneur et pays visités (Gen 6/7).
fn check_handler(ctx: &Ctx, out: &mut Lines) {
    let Some(h) = ctx.pk.extras().handler else { return };
    if h.name.is_empty() {
        if h.current != 0 {
            out.bad("handler-current", "Soigneur absent", "Le Pokémon est confié à un soigneur, mais aucun soigneur n'est enregistré.", TAB_TRAINER);
        }
        if h.friendship != 0 || h.ht_affection != 0 || h.ht_memory.id != 0 {
            out.bad("handler-data", "Données de soigneur orphelines", "Bonheur, affection ou souvenir du soigneur sans soigneur enregistré.", TAB_TRAINER);
        }
    }
    // Pays visités : remplis du plus récent au plus ancien, sans trou ; pas de région sans pays.
    let mut gap = false;
    let mut bad = false;
    for [region, country] in h.geo {
        if country == 0 {
            gap = true;
            bad |= region != 0;
        } else if gap {
            bad = true;
        }
    }
    if bad {
        out.bad("geo", "Pays visités incohérents", "Les pays des échanges doivent se suivre sans case vide, et une région demande un pays.", TAB_TRAINER);
    }
}

/// Souvenirs : un souvenir avec le dresseur d'origine seulement (et toujours) pour les Pokémon nés en Gen 6.
fn check_memories(ctx: &Ctx, out: &mut Lines) {
    let Some(h) = ctx.pk.extras().handler else { return };
    let og = ctx.origin_generation();
    // Les cadeaux (rencontre fatidique) peuvent porter un souvenir fixé par la distribution.
    if h.ot_memory.id != 0 && og != 6 && !ctx.pk.fateful_encounter() {
        out.bad(
            "memory-ot",
            "Souvenir impossible",
            format!("Seuls les Pokémon nés en Gen 6 ont un souvenir avec leur dresseur d'origine (origine : Gen {og})."),
            TAB_MEMORIES,
        );
    }
    // Né en Gen 6 (hors œuf et cadeau) : le jeu pose toujours un souvenir avec le dresseur d'origine.
    if og == 6 && !ctx.pk.is_egg() && !ctx.pk.fateful_encounter() && h.ot_memory.id == 0 {
        out.fishy("memory-ot-missing", "Souvenir manquant", "Un Pokémon obtenu en X/Y ou ROSA a toujours un souvenir avec son dresseur d'origine.", TAB_MEMORIES);
    }
    let texts = crate::dex::memory_texts().len();
    for (m, who) in [(h.ot_memory, "dresseur d'origine"), (h.ht_memory, "soigneur")] {
        if m.id as usize >= texts {
            out.bad("memory-id", "Souvenir inconnu", format!("Souvenir n°{} avec le {who} : il n'existe pas.", m.id), TAB_MEMORIES);
        } else if m.id != 0 && (m.intensity == 0 || m.intensity > 7) {
            out.fishy("memory-intensity", "Intensité du souvenir", format!("Le souvenir avec le {who} doit avoir une intensité de 1 à 7."), TAB_MEMORIES);
        }
    }
}

/// Super Training : X/Y et ROSA seulement.
fn check_super_training(ctx: &Ctx, out: &mut Lines) {
    let Some(st) = ctx.pk.extras().super_training else { return };
    if visited(ctx).g6 {
        return;
    }
    if st.medals.iter().any(|&m| m) || st.secret_unlocked || st.supremely_trained {
        out.bad("super-training", "Médailles impossibles", "Le Super Training n'existe qu'en X/Y et ROSA : ce Pokémon n'y est jamais passé.", TAB_EXTRAS);
    }
}

/// Feuilles brillantes (HGSS) : la couronne demande les 5 feuilles.
fn check_shiny_leaf(ctx: &Ctx, out: &mut Lines) {
    if ctx.pk.format() != PkmFormat::Gen4 {
        return;
    }
    let Some(v) = ctx.pk.extras().shiny_leaf else { return };
    if v & 0x20 != 0 && v & 0x1F != 0x1F {
        out.bad("shiny-leaf", "Couronne sans feuilles", format!("{} a la couronne sans avoir les 5 feuilles brillantes.", species_name(ctx.pk.species())), TAB_EXTRAS);
    }
}
