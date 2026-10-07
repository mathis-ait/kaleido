//! Randomizer Gen 1 et 2 (Rouge, Bleu, Jaune, Or, Argent, Cristal) : mêmes réglages, même
//! seed et même code de partage que sur DS.
//!
//! Les fiches GB sont converties au format Gen 3/4 du contexte commun (`data::gen12`) :
//! statistiques, taux de capture, starters, évolutions et choix d'espèces suivent le code DS.
//! Les types sont tirés à part (types de la génération seulement : pas d'Acier ni de Ténèbres
//! en Gen 1) ; il n'y a ni talents ni natures. Sauvages, dresseurs et Pokémon fixes sont
//! réécrits sur place, avec le même nombre d'emplacements.
//!
//! Non pris en charge : objets, CT, échanges, taux de chromatiques. **Rien n'a été validé
//! sur une vraie ROM** (aucune n'était disponible) : le journal le rappelle.

use std::collections::HashMap;
use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand::Rng;

use super::statics::StaticMode;
use super::{apply_personal, choose_starters, extras, rng_for, share_code, Ctx, KaleidoTag, Outcome, PokemonRef, Settings, Sources, TrainerMode, WildMode, LEGENDARIES};
use crate::data::gen12;
use crate::data::learnsets;
use crate::gb_rom::GbGameRom;
use crate::pokemon::PokeType;
use crate::rom::RomError;

const STRUGGLE: u16 = 165;

pub fn supports(_game: &GbGameRom) -> bool {
    true
}

fn max_move(g: &GbGameRom) -> u16 {
    if g.generation == 1 {
        165
    } else {
        251
    }
}

/// Types de la génération (Gen 1 : 15 types, Gen 2 : + Acier et Ténèbres).
fn types_of(generation: u8) -> Vec<PokeType> {
    use PokeType::*;
    let mut t = vec![Normal, Fighting, Flying, Poison, Ground, Rock, Bug, Ghost, Fire, Water, Grass, Electric, Psychic, Ice, Dragon];
    if generation == 2 {
        t.extend([Steel, Dark]);
    }
    t
}

struct Loaded {
    ctx: Ctx,
    learnsets: Vec<Vec<u8>>,
}

fn load(g: &GbGameRom, settings: &Settings) -> Result<Loaded, RomError> {
    let n = g.species_count();
    let mut evolutions = vec![Vec::new()];
    let mut learnsets_raw = vec![vec![0xFF, 0xFF]];
    let mut personal = vec![vec![0u8; 0x2C]];
    for s in 1..=n {
        evolutions.push(gen12::evolutions_gen4(g, s)?);
        learnsets_raw.push(gen12::learnset_gen4(g, s)?);
        personal.push(gen12::to_gen4_record(g.generation, g.personal(s).ok_or_else(|| RomError::Layout(format!("fiche n°{s}")))?));
    }
    let names: Vec<String> = crate::dex::species_names().iter().map(|s| s.to_string()).collect();
    let src = Sources { gen: g.generation, count: n, names, personal, evolutions: &evolutions, learnsets: &learnsets_raw, max_ability: 1 };
    Ok(Loaded { ctx: Ctx::new(src, settings)?, learnsets: learnsets_raw })
}

/// Réglages passés au code commun : types et talents sont traités ici (ou absents).
fn common(settings: &Settings) -> Settings {
    Settings { random_types: false, random_abilities: false, ..settings.clone() }
}

fn current_starters(g: &GbGameRom) -> Result<[u16; 3], RomError> {
    let s = gen12::starters(g)?;
    Ok([s[0], s[1], *s.get(2).unwrap_or(&s[1])])
}

/// Aperçu des starters, sans modifier la ROM.
pub fn preview_starters(g: &GbGameRom, settings: &Settings, seed: u64) -> Result<Vec<PokemonRef>, RomError> {
    let mut l = load(g, settings)?;
    apply_personal(&mut l.ctx, &common(settings), seed, &mut String::new());
    random_types(&mut l.ctx, g.generation, settings, seed);
    let n = gen12::starters(g)?.len();
    let chosen = choose_starters(&l.ctx, settings, seed, current_starters(g)?);
    Ok(chosen[..n].iter().map(|&id| PokemonRef { id, name: l.ctx.name(id).to_string() }).collect())
}

/// Types aléatoires, identiques dans une famille d'évolution (comme `apply_personal`).
fn random_types(ctx: &mut Ctx, generation: u8, settings: &Settings, seed: u64) -> bool {
    if !settings.random_types {
        return false;
    }
    let mut rng = rng_for(seed, "types");
    let types = types_of(generation);
    let count = ctx.count as usize;
    for base in (1..=count).filter(|&s| ctx.evo.stage[s] == 0) {
        let t1 = *types.choose(&mut rng).unwrap();
        let t2 = if rng.gen_bool(0.5) { *types.choose(&mut rng).unwrap() } else { t1 };
        let mut family = vec![base];
        while let Some(s) = family.pop() {
            let d = &mut ctx.personal[s];
            d[6] = super::type_index(3, t1);
            d[7] = super::type_index(3, t2);
            family.extend(ctx.evo.targets[s].iter().map(|&t| t as usize));
        }
    }
    true
}

/// Randomise le jeu en mémoire. Appeler `g.save()` ensuite pour écrire la ROM.
pub fn randomize(g: &mut GbGameRom, settings: &Settings, seed: u64) -> Result<Outcome, RomError> {
    let mut l = load(g, settings)?;
    let mut log = String::new();
    let code = share_code(seed, settings);
    let _ = writeln!(log, "Kaleido — journal de randomisation");
    let _ = writeln!(log, "Jeu : {} ({}, version {})", g.game.name_fr(), g.entry.name, g.rom().header().version);
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}\n");
    let _ = writeln!(log, "Attention : la prise en charge des jeux Game Boy n'a pas encore été vérifiée sur une vraie ROM (à l'essai).\n");

    // 1. Fiches : statistiques et capture (code commun), puis types.
    let changed = apply_personal(&mut l.ctx, &common(settings), seed, &mut log) | random_types(&mut l.ctx, g.generation, settings, seed);
    if changed {
        if settings.random_types {
            let _ = writeln!(log, "== Types ==");
            for s in 1..=l.ctx.count {
                let types: Vec<_> = l.ctx.types(s).iter().map(|t| t.name_fr()).collect();
                let _ = writeln!(log, "{:03} {:<12} {}", s, l.ctx.name(s), types.join("/"));
            }
            let _ = writeln!(log);
        }
        for s in 1..=g.species_count() {
            let original = g.personal(s).unwrap_or_default().to_vec();
            let rec = gen12::from_gen4_record(g.generation, &original, &l.ctx.personal[s as usize]);
            g.set_personal(s, &rec)?;
        }
    }

    // 2. Starters.
    let current = current_starters(g)?;
    let n = gen12::starters(g)?.len();
    let chosen = choose_starters(&l.ctx, settings, seed, current);
    if chosen != current {
        gen12::set_starters(g, &chosen[..n])?;
        let _ = writeln!(log, "== Starters ==");
        for (old, new) in current.iter().zip(chosen).take(n) {
            let _ = writeln!(log, "{} → {}", l.ctx.name(*old), l.ctx.name(new));
        }
        let _ = writeln!(log);
    }

    // 3. Pokémon fixes.
    randomize_statics(g, &l.ctx, settings, seed, &mut log)?;

    // 4. Évolutions et attaques.
    if settings.easy_evolutions {
        let _ = writeln!(log, "== Évolutions sans échange ==");
        for s in 1..=g.species_count() {
            let targets = gen12::remove_trade_evolutions(g, s)?;
            if !targets.is_empty() {
                let names: Vec<&str> = targets.iter().map(|&t| l.ctx.name(t)).collect();
                let _ = writeln!(log, "{} → {}", l.ctx.name(s), names.join(" / "));
            }
            l.ctx.evo_tables[s as usize] = crate::data::evolutions::read(&gen12::evolutions_gen4(g, s)?);
        }
        let _ = writeln!(log);
    }
    if settings.random_movesets {
        extras::random_movesets(&mut l.ctx, &mut l.learnsets, max_move(g), seed, &mut log);
        for s in 1..=g.species_count() {
            gen12::set_learnset_gen4(g, s, &l.learnsets[s as usize])?;
        }
    }
    if settings.moves.random_tms || settings.moves.tm_compat != super::moves::CompatMode::Unchanged {
        let _ = writeln!(log, "== CT ==\nPas encore pris en charge sur Game Boy, inchangées.\n");
    }

    // 5. Sauvages.
    let wild_slots = if settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100 { randomize_wild(g, &l.ctx, settings, seed, &mut log)? } else { 0 };

    if settings.items.changes_anything() {
        let _ = writeln!(log, "== Objets ==\nPas encore pris en charge sur Game Boy, inchangés.\n");
    }

    // 6. Dresseurs.
    let trainer_pokemon = if super::trainers_changed(settings) { randomize_trainers(g, &l.ctx, settings, seed, &mut log)? } else { 0 };

    let tag = KaleidoTag { tool: "Kaleido".into(), version: env!("CARGO_PKG_VERSION").into(), seed, share_code: code.clone() };
    if !g.rom_mut().set_signature(&serde_json::to_vec(&tag).unwrap_or_default()) {
        let _ = writeln!(log, "Pas de place libre en fin de ROM : la signature Kaleido n'a pas été écrite (la seed figure dans le nom du fichier).");
    }
    Ok(Outcome {
        seed,
        share_code: code,
        starters: chosen[..n].iter().map(|&id| PokemonRef { id, name: l.ctx.name(id).to_string() }).collect(),
        wild_slots,
        trainer_pokemon,
        log,
    })
}

fn scale(level: u8, percent: u16) -> u8 {
    ((level as u32 * percent as u32 + 50) / 100).clamp(1, 100) as u8
}

fn randomize_wild(g: &mut GbGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<usize, RomError> {
    let mut rng = rng_for(seed, "wild");
    let similar = settings.wild_similar_strength;
    let mut global: HashMap<u16, u16> = HashMap::new();
    if settings.wild == WildMode::Global {
        let mut order: Vec<u16> = (1..=ctx.count).collect();
        order.shuffle(&mut rng);
        let mut used = Vec::new();
        for s in order {
            let new = ctx.pick(&mut rng, similar.then_some(s), |_| true, &used);
            used.push(new);
            global.insert(s, new);
        }
    }
    let _ = writeln!(log, "== Pokémon sauvages ==");
    let mut total = 0;
    for mut area in gen12::wild_areas(g)? {
        let mut local: HashMap<u16, u16> = HashMap::new();
        let mut changes: Vec<(u16, u16)> = Vec::new();
        for s in &mut area.slots {
            if s.species == 0 {
                continue;
            }
            let old = s.species;
            let new = match settings.wild {
                WildMode::Unchanged => old,
                WildMode::Random => ctx.pick(&mut rng, similar.then_some(old), |_| true, &[]),
                WildMode::Global => *global.get(&old).unwrap_or(&old),
                WildMode::Area => *local.entry(old).or_insert_with(|| ctx.pick(&mut rng, similar.then_some(old), |_| true, &[])),
            };
            s.species = new;
            if settings.wild_level_percent != 100 {
                s.level = scale(s.level, settings.wild_level_percent);
            }
            if !changes.contains(&(old, new)) {
                changes.push((old, new));
            }
            total += 1;
        }
        gen12::set_wild_area(g, &area)?;
        let list: Vec<String> = changes.iter().map(|(o, n)| format!("{} → {}", ctx.name(*o), ctx.name(*n))).collect();
        let _ = writeln!(log, "{} : {}", area.label, list.join(", "));
    }
    let _ = writeln!(log);
    Ok(total)
}

fn randomize_trainers(g: &mut GbGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<usize, RomError> {
    let mut rng = rng_for(seed, "trainers");
    let types = types_of(g.generation);
    let mut total = 0;
    let mut shared_levels: Vec<usize> = Vec::new();
    let _ = writeln!(log, "== Dresseurs ==");
    for mut t in gen12::trainers(g)? {
        if t.party.is_empty() || t.party.iter().any(|m| m.species == 0) {
            continue;
        }
        let theme = (settings.trainers == TrainerMode::TypeThemed).then(|| ctx.types(t.party[0].species).first().copied().unwrap_or_else(|| *types.choose(&mut rng).unwrap()));
        let mut used = Vec::new();
        let mut lines = Vec::new();
        let mut moves = Vec::new();
        for m in &mut t.party {
            let (old, old_level) = (m.species, m.level);
            if settings.trainers != TrainerMode::Unchanged {
                let similar = settings.trainers_similar_strength.then_some(m.species);
                m.species = ctx.pick(&mut rng, similar, |s| theme.is_none_or(|ty| ctx.types(s).contains(&ty)), &used);
                used.push(m.species);
            }
            // Gen 1 : un niveau commun à toute l'équipe, mis à l'échelle une seule fois.
            if !shared_levels.contains(&m.level_at) {
                m.level = scale(m.level, settings.trainer_level_percent);
            }
            if settings.trainer_evolutions {
                m.species = extras::evolve_for_level(ctx, &mut rng, m.species, m.level as u16);
            }
            let mv = ctx.learnsets.get(m.species as usize).map_or([0; 4], |ls| learnsets::moves_at_level(ls, m.level as u16));
            moves.push(mv.map(|x| x.min(255) as u8));
            lines.push(format!("{} niv. {} → {} niv. {}", ctx.name(old), old_level, ctx.name(m.species), m.level));
            total += 1;
        }
        shared_levels.extend(t.party.iter().map(|m| m.level_at));
        gen12::set_party(g, &t, &moves)?;
        let name = if t.name.is_empty() { format!("n°{}", t.index) } else { format!("n°{} {}", t.index, t.name) };
        let _ = writeln!(log, "Dresseur {name} : {}", lines.join(" ; "));
    }
    let _ = writeln!(log);
    Ok(total)
}

fn randomize_statics(g: &mut GbGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<(), RomError> {
    let s = &settings.statics;
    if s.mode == StaticMode::Unchanged && s.level_modifier == 0 {
        return Ok(());
    }
    let mut rng = rng_for(seed, "statics");
    let _ = writeln!(log, "== Pokémon fixes ==");
    for (i, (species, levels)) in gen12::statics(g).into_iter().enumerate() {
        if species == 0 || species > ctx.count {
            continue;
        }
        let legendary = LEGENDARIES.contains(&species);
        let new = match s.mode {
            StaticMode::Unchanged => species,
            StaticMode::SwapLegendaries => {
                let pool: Vec<u16> = (1..=ctx.count).filter(|&x| LEGENDARIES.contains(&x) == legendary && x != species).collect();
                *pool.choose(&mut rng).unwrap_or(&species)
            }
            StaticMode::SimilarStrength => ctx.pick(&mut rng, Some(species), |_| true, &[species]),
            StaticMode::Random => ctx.pick(&mut rng, None, |_| true, &[]),
        };
        let level = levels.first().map(|&l| scale(l, (100 + s.level_modifier as i32).max(1) as u16)).filter(|_| s.level_modifier != 0);
        gen12::set_static(g, i, new, level)?;
        let _ = writeln!(log, "{} → {} ({})", ctx.name(species), ctx.name(new), g.entry.statics[i].label);
    }
    let _ = writeln!(log);
    let _ = STRUGGLE;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_rom::synthetic;
    use crate::randomizer::{StarterMode, StatsMode};

    fn chaos() -> Settings {
        Settings {
            starters: StarterMode::Random,
            wild: WildMode::Random,
            trainers: TrainerMode::TypeThemed,
            stats: StatsMode::Shuffle,
            random_types: true,
            easy_evolutions: true,
            random_movesets: true,
            ..Settings::default()
        }
    }

    #[test]
    fn randomize_synthetic_gb_roms() {
        for section in ["Red (F)", "Yellow (F)", "Crystal (F)", "Silver (F)"] {
            let rom = synthetic::build(section);
            let mut g = GbGameRom::from_rom(rom.clone()).unwrap();
            let s = chaos();
            let preview = preview_starters(&g, &s, 77).unwrap();
            let out = randomize(&mut g, &s, 77).unwrap();
            let ids: Vec<u16> = out.starters.iter().map(|p| p.id).collect();
            assert_eq!(ids, preview.iter().map(|p| p.id).collect::<Vec<_>>(), "{section}");
            assert_eq!(gen12::starters(&g).unwrap(), ids);
            assert!(out.wild_slots > 0 && out.trainer_pokemon == 2, "{section}");
            // Gen 1 : jamais d'Acier ni de Ténèbres.
            if g.generation == 1 {
                for sp in g.species().unwrap() {
                    assert!(sp.types.iter().all(|t| !matches!(t.key, PokeType::Steel | PokeType::Dark)), "{}", sp.name);
                }
            }
            let bytes = g.rom().to_bytes();
            let again = GbGameRom::from_rom(kaleido_formats::gb::GbRom::from_bytes(bytes.clone()).unwrap()).unwrap();
            let tag: KaleidoTag = serde_json::from_slice(again.rom().signature().unwrap()).unwrap();
            assert_eq!(tag.seed, 77);
            let mut g2 = GbGameRom::from_rom(rom).unwrap();
            randomize(&mut g2, &s, 77).unwrap();
            assert_eq!(g2.rom().to_bytes(), bytes, "{section} : même seed, même ROM");
        }
    }
}

