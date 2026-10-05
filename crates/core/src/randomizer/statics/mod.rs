//! Pokémon fixes, dons et échanges en jeu (Platine, Noire, Blanche).
//!
//! Portage de `StaticPokemonRandomizer` et `TradeRandomizer` de l'Universal
//! Pokémon Randomizer (FVX), avec ses emplacements (`StaticPokemon{}`,
//! `TradeScript[]`…) et ses correctifs pour garder le jeu terminable :
//! - Platine : sol du Monde Distorsion (sinon plantage si Giratina n'a plus
//!   d'animation d'entrée) ;
//! - Noire/Blanche : espèce du légendaire de la boîte (Reshiram / Zekrom), vérifiée
//!   en dur par le jeu pour le placer en tête d'équipe ;
//! - Noire/Blanche : vagabonds (Fulguris / Boréas), dont la seconde espèce était
//!   calculée par « Boréas + 1 ».
//!
//! Non pris en charge : vagabonds de Platine (l'UPR ajoute une routine à l'ARM9),
//! musique des légendaires (correctif IPS de l'UPR), formes alternatives.

mod access;
mod tables;
mod trades;

use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use super::{rng_for, Ctx, LEGENDARIES};
use crate::games::Game;
use crate::rom::{GameRom, RomError};
pub use tables::Kind;
pub use trades::Trade;

/// Pokémon fixes et dons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StaticMode {
    #[default]
    Unchanged,
    /// Légendaires ↔ légendaires, autres ↔ autres (UPR « Swap Legendaries & Swap Standards »).
    SwapLegendaries,
    /// Espèce de puissance comparable (UPR « Random (similar strength) »).
    SimilarStrength,
    /// Complètement au hasard (l'option « pas de légendaires » s'applique).
    Random,
}

/// Échanges en jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TradeMode {
    #[default]
    Unchanged,
    /// Pokémon donné au hasard (le Pokémon demandé ne change pas).
    Given,
    /// Pokémon donné et Pokémon demandé au hasard.
    GivenAndRequested,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StaticSettings {
    pub mode: StaticMode,
    /// Variation des niveaux des Pokémon fixes, en pourcentage (0 = inchangés,
    /// 20 = +20 %, -10 = −10 %), comme le « level modifier » de l'UPR. Les œufs ne changent pas.
    pub level_modifier: i16,
    pub trades: TradeMode,
    /// Objets tenus au hasard (échanges randomisés seulement).
    pub trade_random_items: bool,
    /// IV au hasard (échanges randomisés seulement).
    pub trade_random_ivs: bool,
}

/// Une rencontre fixe telle qu'elle est dans la ROM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticEncounter {
    /// Position dans la liste de l'UPR.
    pub index: usize,
    pub kind: Kind,
    pub species: u16,
    /// Niveau de chaque copie de la rencontre (vide pour un œuf).
    pub levels: Vec<u8>,
}

/// Rencontres fixes et échanges d'une ROM, avec les emplacements ignorés.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub statics: Vec<StaticEncounter>,
    pub trades: Vec<Trade>,
    pub notes: Vec<String>,
}

fn supported(game: Game) -> bool {
    matches!(game, Game::Platinum | Game::Black | Game::White)
}

/// Lit les rencontres fixes et les échanges, sans rien modifier.
pub fn list(game: &GameRom) -> Result<Listing, RomError> {
    if !supported(game.game) {
        return Err(RomError::Unsupported("Pokémon fixes : Platine, Noire et Blanche seulement".into()));
    }
    let mut files = access::Files::load(game)?;
    let (entries, mut notes) = access::entries(game, &mut files, game.layout.species_count);
    // Diagnostic des correctifs de code (appliqués à la copie en mémoire, jamais enregistrée).
    match game.game {
        Game::Platinum => {
            let ok = files.patch_distortion_world(game)?;
            notes.push(format!("correctif du Monde Distorsion : {}", if ok { "code reconnu" } else { "code introuvable" }));
        }
        _ => {
            // Seulement sur une ROM d'origine (le code corrigé ne ressemble plus à l'original).
            let index = if game.game == Game::Black { tables::BW_BOX_LEGENDARY_BLACK } else { tables::BW_BOX_LEGENDARY_WHITE };
            if entries.iter().any(|e| e.index == index && e.species == [643, 644][index - tables::BW_BOX_LEGENDARY_BLACK]) {
                let done = files.fix_box_legendary(game, 0)?;
                notes.push(format!("correctif du légendaire de la boîte : {done}/2 emplacements reconnus"));
            }
        }
    }
    let statics = entries
        .into_iter()
        .map(|e| StaticEncounter { index: e.index, kind: e.def.kind, species: e.species, levels: e.levels })
        .collect();
    let trades = trades::load(game)?.map(|t| t.list).unwrap_or_default();
    Ok(Listing { statics, trades, notes })
}

/// `applyPercentageLevelModifier` de l'UPR.
fn modify_level(level: u8, modifier: i16) -> u8 {
    (level as f64 * (1.0 + modifier as f64 / 100.0)).round().clamp(1.0, 100.0) as u8
}

/// Choisit au hasard dans `pool` une espèce pas encore utilisée (sinon n'importe laquelle).
fn pick_fresh(rng: &mut ChaCha8Rng, pool: &[u16], used: &[u16]) -> Option<u16> {
    let fresh: Vec<u16> = pool.iter().copied().filter(|s| !used.contains(s)).collect();
    fresh.choose(rng).or_else(|| pool.choose(rng)).copied()
}

/// Nouvelle espèce pour une rencontre fixe.
fn choose(ctx: &Ctx, rng: &mut ChaCha8Rng, mode: StaticMode, old: u16, kind: Kind, used: &[u16]) -> u16 {
    let legendary = |s: u16| LEGENDARIES.contains(&s);
    // Un œuf donne une espèce de base.
    let hatchable = |s: u16| kind != Kind::Egg || ctx.evo.stage.get(s as usize) == Some(&0);
    let all = 1..=ctx.count;
    let picked = match mode {
        StaticMode::Unchanged => Some(old),
        StaticMode::SwapLegendaries => {
            let pool: Vec<u16> = if legendary(old) {
                all.filter(|&s| legendary(s)).collect()
            } else {
                all.filter(|&s| !legendary(s) && hatchable(s)).collect()
            };
            pick_fresh(rng, &pool, used)
        }
        StaticMode::Random => {
            let pool: Vec<u16> = all.filter(|&s| ctx.allowed(s) && hatchable(s)).collect();
            pick_fresh(rng, &pool, used)
        }
        StaticMode::SimilarStrength => Some(ctx.pick(rng, Some(old), hatchable, used)),
    };
    picked.unwrap_or(old)
}

/// Étape du randomizer : Pokémon fixes, dons et échanges.
pub(crate) fn apply(game: &mut GameRom, ctx: &Ctx, settings: &StaticSettings, seed: u64, log: &mut String) -> Result<(), RomError> {
    let statics_on = settings.mode != StaticMode::Unchanged || settings.level_modifier != 0;
    let trades_on = settings.trades != TradeMode::Unchanged;
    if !(statics_on || trades_on) {
        return Ok(());
    }
    if !supported(game.game) {
        let _ = writeln!(log, "== Pokémon fixes et échanges ==\nNon pris en charge pour ce jeu.\n");
        return Ok(());
    }
    let mut files = access::Files::load(game)?;
    let mut notes = Vec::new();
    if statics_on {
        randomize_statics(game, &mut files, ctx, settings, seed, log, &mut notes)?;
    }
    if trades_on {
        randomize_trades(game, &mut files, ctx, settings, seed, log, &mut notes)?;
    }
    files.save(game)?;
    if !notes.is_empty() {
        let _ = writeln!(log, "== Pokémon fixes : remarques ==");
        for n in &notes {
            let _ = writeln!(log, "{n}");
        }
        let _ = writeln!(log);
    }
    Ok(())
}

fn randomize_statics(
    game: &mut GameRom,
    files: &mut access::Files,
    ctx: &Ctx,
    settings: &StaticSettings,
    seed: u64,
    log: &mut String,
    notes: &mut Vec<String>,
) -> Result<(), RomError> {
    let mut rng = rng_for(seed, "statics");
    let (entries, skipped) = access::entries(game, files, ctx.count);
    notes.extend(skipped);
    let _ = writeln!(log, "== Pokémon fixes et dons ==");
    let mut used: Vec<u16> = Vec::new();
    let mut changed: Vec<(usize, u16)> = Vec::new();
    for e in &entries {
        let new = choose(ctx, &mut rng, settings.mode, e.species, e.def.kind, &used);
        used.push(new);
        if new != e.species {
            changed.push((e.index, new));
            for &loc in &e.def.species {
                files.set_u16(loc, new);
            }
        }
        let levels: Vec<u8> = if e.def.kind == Kind::Egg { e.levels.clone() } else { e.levels.iter().map(|&l| modify_level(l, settings.level_modifier)).collect() };
        for ((&loc, &old), &lvl) in e.def.levels.iter().zip(&e.levels).zip(&levels) {
            if lvl != old {
                files.set_u8(loc, lvl);
            }
        }
        let show = |l: &[u8]| l.first().map(|l| format!(" niv. {l}")).unwrap_or_default();
        let _ = writeln!(log, "{:<10} {}{} → {}{}", e.def.kind.label(), ctx.name(e.species), show(&e.levels), ctx.name(new), show(&levels));
    }
    let _ = writeln!(log);

    // Correctifs de l'UPR quand une rencontre clé change.
    match game.game {
        Game::Platinum if changed.iter().any(|&(i, _)| i == 0) => {
            if !files.patch_distortion_world(game)? {
                notes.push("correctif du Monde Distorsion introuvable : remplacer Giratina peut faire planter le jeu".into());
            }
        }
        Game::Black | Game::White => {
            let index = if game.game == Game::Black { tables::BW_BOX_LEGENDARY_BLACK } else { tables::BW_BOX_LEGENDARY_WHITE };
            if let Some(&(_, species)) = changed.iter().find(|&&(i, _)| i == index) {
                let done = files.fix_box_legendary(game, species)?;
                if done < 2 {
                    notes.push(format!("légendaire de la boîte : {done}/2 correctifs appliqués (code de l'overlay 21 non reconnu)"));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Objets tenus possibles : baies et objets à tenir (n° identiques en Gen 4 et 5),
/// sans la Rosée Âme (interdite dans de nombreux combats).
fn held_items() -> Vec<u16> {
    (149..=288).filter(|&i| i != 225).collect()
}

fn randomize_trades(
    game: &mut GameRom,
    files: &mut access::Files,
    ctx: &Ctx,
    settings: &StaticSettings,
    seed: u64,
    log: &mut String,
    notes: &mut Vec<String>,
) -> Result<(), RomError> {
    let Some(mut data) = trades::load(game)? else {
        notes.push("échanges introuvables dans la ROM".into());
        return Ok(());
    };
    let mut rng = rng_for(seed, "trades");
    let old = data.list.clone();
    let items = held_items();
    let (mut givens, mut requests): (Vec<u16>, Vec<u16>) = (Vec::new(), Vec::new());
    for t in &mut data.list {
        let old_given = t.given;
        t.given = ctx.pick(&mut rng, None, |_| true, &givens);
        givens.push(t.given);
        if old_given == t.requested {
            t.requested = t.given; // échange d'une espèce contre la même
        } else if settings.trades == TradeMode::GivenAndRequested {
            let given = t.given;
            t.requested = ctx.pick(&mut rng, None, |s| s != given, &requests);
            requests.push(t.requested);
        }
        if t.nickname.to_lowercase() == ctx.name(old_given).to_lowercase() {
            t.nickname = ctx.name(t.given).to_string();
        }
        if settings.trade_random_ivs {
            for iv in &mut t.ivs {
                *iv = rng.gen_range(0..32);
            }
        }
        if settings.trade_random_items {
            t.item = *items.choose(&mut rng).unwrap_or(&0);
        }
    }

    let _ = writeln!(log, "== Échanges ==");
    for (o, n) in old.iter().zip(&data.list) {
        let _ = writeln!(
            log,
            "{} contre {} → {} « {} » contre {}",
            ctx.name(o.given),
            ctx.name(o.requested),
            ctx.name(n.given),
            n.nickname,
            ctx.name(n.requested)
        );
    }
    let _ = writeln!(log);

    let ability = |s: u16| ctx.personal(s).abilities().first().copied().unwrap_or(0);
    notes.extend(trades::write(game, files, data, &old, &ctx.names, ability)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_modifier_like_upr() {
        assert_eq!(modify_level(50, 0), 50);
        assert_eq!(modify_level(50, 20), 60);
        assert_eq!(modify_level(5, -50), 3);
        assert_eq!(modify_level(90, 50), 100);
        assert_eq!(modify_level(1, -100), 1);
    }

    #[test]
    fn settings_default_and_serde() {
        let s = StaticSettings::default();
        assert_eq!(s.mode, StaticMode::Unchanged);
        assert_eq!(s.level_modifier, 0);
        let parsed: StaticSettings = serde_json::from_str(r#"{"mode":"swap_legendaries","levelModifier":10,"trades":"given_and_requested"}"#).unwrap();
        assert_eq!(parsed.mode, StaticMode::SwapLegendaries);
        assert_eq!(parsed.level_modifier, 10);
        assert_eq!(parsed.trades, TradeMode::GivenAndRequested);
        assert!(!parsed.trade_random_items);
    }

    #[test]
    fn fresh_picks_avoid_duplicates() {
        let mut rng = rng_for(1, "test");
        let pool = [1, 2, 3];
        assert_eq!(pick_fresh(&mut rng, &pool, &[1, 2]), Some(3));
        assert!(pick_fresh(&mut rng, &pool, &[1, 2, 3]).is_some());
        assert_eq!(pick_fresh(&mut rng, &[], &[]), None);
    }

    #[test]
    fn tables_are_consistent() {
        let pt = tables::platinum();
        let bw = tables::black_white();
        assert_eq!(pt.len(), 20 + 7);
        assert_eq!(bw.len(), 22 + 9 + 2);
        assert!(pt.iter().chain(&bw).all(|d| !d.species.is_empty()));
        assert!(pt.iter().chain(&bw).filter(|d| d.kind == Kind::Egg).all(|d| d.levels.is_empty()));
        assert_eq!(tables::BW_BOX_LEGENDARY_WHITE, tables::BW_BOX_LEGENDARY_BLACK + 1);
    }
}
