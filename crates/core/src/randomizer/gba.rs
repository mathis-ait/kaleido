//! Randomizer Gen 3 (Rubis, Saphir, Émeraude, Rouge Feu, Vert Feuille) : mêmes réglages,
//! même seed et même code de partage que sur DS.
//!
//! Les tables de la ROM (voir `data::gen3`) sont converties dans les formats Gen 4 du
//! contexte commun (fiches de 0x2C octets, évolutions de 6 octets, n° nationaux), si bien
//! que types, statistiques, talents, starters, évolutions et attaques suivent exactement
//! le code DS ; sauvages, dresseurs, Pokémon fixes et CT ont leur propre boucle.
//!
//! Correctifs repris de l'UPR (`attemptObedienceEvolutionPatches`) :
//! - Émeraude / Rouge Feu / Vert Feuille : Mew et Deoxys obéissent (sinon un Deoxys de
//!   départ ou sauvage refuserait d'attaquer) ;
//! - Rouge Feu / Vert Feuille : les espèces hors du Pokédex de Kanto évoluent avant le
//!   Pokédex National (le jeu bloquait toute évolution de n° > 151).
//!
//! Non pris en charge pour l'instant : objets ramassables et boutiques, taux de
//! chromatiques, donneurs de capacités, échanges en jeu. **Rien n'a été validé sur une
//! vraie ROM** (aucune n'était disponible) : le journal le rappelle.

use std::collections::HashMap;
use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::moves::CompatMode;
use super::statics::StaticMode;
use super::{apply_personal, choose_starters, extras, rng_for, share_code, Ctx, KaleidoTag, Outcome, PokemonRef, Settings, Sources, TrainerMode, WildMode, LEGENDARIES};
use crate::data::gen3::{self, WildArea};
use crate::data::learnsets;
use crate::gba_rom::{GbaGameRom, BASE_STATS_SIZE, SPECIES_COUNT, TM_COUNT};
use crate::pokemon::PokeType;
use crate::rom::RomError;

/// Plus grand talent de la Gen 3 (Air Lock).
const MAX_ABILITY: u16 = 77;
/// Plus grande attaque de la Gen 3 (Frappe Psy).
const MAX_MOVE: u16 = 354;
const STRUGGLE: u16 = 165;
/// Attaques des CS de la Gen 3 (Coupe, Vol, Surf, Force, Flash, Éclate-Roc, Cascade, Plongée).
const HM_MOVES: [u16; 8] = [15, 19, 57, 70, 148, 249, 127, 291];
/// Sonicboom et Draco-Rage.
const GAME_BREAKING: [u16; 2] = [49, 82];
/// Capacités de terrain enseignées par CT (Tunnel, Téléport, Doux Parfum, Force Cachée).
const FIELD_TM_MOVES: [u16; 4] = [91, 100, 230, 290];
const MIN_DAMAGING_POWER: u8 = 50;
/// Zarbi : Rouge Feu / Vert Feuille plantent s'il apparaît hors des Ruines Tanoby.
const UNOWN: u16 = 201;

const DEOXYS_OBEY_CODE: &str = "CD21490088420FD0";
const MEW_OBEY_FROM_DEOXYS: usize = 0x16;
const LEVEL_EVO_KANTO_CHECK: &str = "972814DD";
const STONE_EVO_KANTO_CHECK: &str = "972808D9";

pub fn supports(_game: &GbaGameRom) -> bool {
    true
}

/// Fiche Gen 3 (28 octets) complétée au format Gen 4 (0x2C), qui a le même début.
fn padded(record: &[u8]) -> Vec<u8> {
    let mut d = record.to_vec();
    d.resize(0x2C, 0);
    d
}

struct Loaded {
    ctx: Ctx,
    evolutions: Vec<Vec<u8>>,
    learnsets: Vec<Vec<u8>>,
}

fn load(g: &GbaGameRom, settings: &Settings) -> Result<Loaded, RomError> {
    let n = SPECIES_COUNT as usize;
    let mut evolutions = vec![Vec::new()];
    let mut learnsets_raw = vec![vec![0xFF, 0xFF]];
    for s in 1..=n as u16 {
        evolutions.push(gen3::evolutions(g, s)?);
        learnsets_raw.push(gen3::learnset(g, s)?);
    }
    let names: Vec<String> = crate::dex::species_names().iter().map(|s| s.to_string()).collect();
    let personal: Vec<Vec<u8>> = g.personal_table().iter().map(|r| padded(r)).collect();
    let src = Sources { gen: 3, count: SPECIES_COUNT, names, personal, evolutions: &evolutions, learnsets: &learnsets_raw, max_ability: MAX_ABILITY };
    Ok(Loaded { ctx: Ctx::new(src, settings)?, evolutions, learnsets: learnsets_raw })
}

/// Aperçu des starters, sans modifier la ROM.
pub fn preview_starters(g: &GbaGameRom, settings: &Settings, seed: u64) -> Result<Vec<PokemonRef>, RomError> {
    let mut l = load(g, settings)?;
    apply_personal(&mut l.ctx, settings, seed, &mut String::new());
    let chosen = choose_starters(&l.ctx, settings, seed, gen3::starters(g)?);
    Ok(chosen.iter().map(|&id| PokemonRef { id, name: l.ctx.name(id).to_string() }).collect())
}

/// Correctifs d'obéissance et d'évolution (voir l'en-tête du module).
fn patch_obedience_and_evolutions(g: &mut GbaGameRom, log: &mut String) -> Result<(), RomError> {
    use kaleido_formats::gba::hex;
    if g.kind.is_rs() {
        return Ok(());
    }
    let mut done = Vec::new();
    if let Some(at) = g.rom().find(&hex(DEOXYS_OBEY_CODE)) {
        // MOVS R1, #0 deux fois à la place de MOVS R1, #0x19A (Deoxys).
        g.rom_mut().write(at, &[0x00, 0x21, 0x00, 0x21])?;
        // CMP R0, #0x97 (Mew) → CMP R0, #0.
        if g.rom().u16(at + MEW_OBEY_FROM_DEOXYS) == Some(0x2897) {
            g.rom_mut().write_u16(at + MEW_OBEY_FROM_DEOXYS, 0x2800)?;
        }
        done.push("Mew et Deoxys obéissent");
    }
    if g.kind.is_frlg() {
        for (code, jump) in [(LEVEL_EVO_KANTO_CHECK, 0x14u16), (STONE_EVO_KANTO_CHECK, 0x08)] {
            if let Some(at) = g.rom().find(&hex(code)) {
                // NOP puis saut inconditionnel : l'évolution n'est plus réservée aux n° ≤ 151.
                g.rom_mut().write_u16(at, 0x46C0)?;
                g.rom_mut().write_u16(at + 2, 0xE000 | jump)?;
            }
        }
        done.push("les espèces hors de Kanto évoluent avant le Pokédex National");
    }
    if !done.is_empty() {
        let _ = writeln!(log, "== Correctifs ==\n{}\n", done.join(" ; "));
    }
    Ok(())
}

/// Randomise le jeu en mémoire. Appeler `g.save()` ensuite pour écrire la ROM.
pub fn randomize(g: &mut GbaGameRom, settings: &Settings, seed: u64) -> Result<Outcome, RomError> {
    let mut l = load(g, settings)?;
    let mut log = String::new();
    let code = share_code(seed, settings);
    let _ = writeln!(log, "Kaleido — journal de randomisation");
    let _ = writeln!(log, "Jeu : {} ({}, version {})", g.game.name_fr(), g.rom().header().game_code, g.rom().header().version);
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}\n");
    let _ = writeln!(log, "Attention : la prise en charge des jeux GBA n'a pas encore été vérifiée sur une vraie ROM (à l'essai).\n");

    patch_obedience_and_evolutions(g, &mut log)?;

    // 1. Fiches des espèces.
    if apply_personal(&mut l.ctx, settings, seed, &mut log) {
        for s in 1..=SPECIES_COUNT {
            let rec = l.ctx.personal[s as usize][..BASE_STATS_SIZE].to_vec();
            g.set_personal(s, &rec)?;
        }
    }

    // 2. Starters.
    let current = gen3::starters(g)?;
    let chosen = choose_starters(&l.ctx, settings, seed, current);
    if chosen != current {
        gen3::set_starters(g, chosen)?;
        let _ = writeln!(log, "== Starters ==");
        for (old, new) in current.iter().zip(chosen) {
            let _ = writeln!(log, "{} → {}", l.ctx.name(*old), l.ctx.name(new));
        }
        let _ = writeln!(log);
    }

    // 3. Pokémon fixes.
    randomize_statics(g, &l.ctx, settings, seed, &mut log)?;

    // 4. Évolutions et attaques apprises.
    if settings.easy_evolutions {
        extras::easy_evolutions(&mut l.ctx, &mut l.evolutions, &mut log);
        for s in 1..=SPECIES_COUNT {
            gen3::set_evolutions(g, s, &l.evolutions[s as usize])?;
        }
    }
    if settings.random_movesets {
        extras::random_movesets(&mut l.ctx, &mut l.learnsets, MAX_MOVE, seed, &mut log);
        for s in 1..=SPECIES_COUNT {
            gen3::set_learnset(g, s, &l.learnsets[s as usize])?;
        }
    }
    randomize_tms(g, &l.ctx, settings, seed, &mut log)?;

    // 5. Pokémon sauvages.
    let wild_slots = if settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100 {
        let mut areas = gen3::wild_areas(g)?;
        let n = randomize_wild(&l.ctx, settings, seed, g.kind.is_frlg(), &mut areas, &mut log);
        for a in &areas {
            gen3::set_wild_area(g, a)?;
        }
        n
    } else {
        0
    };

    if settings.items.changes_anything() {
        let _ = writeln!(log, "== Objets ==\nObjets ramassables et boutiques : pas encore pris en charge sur GBA, inchangés.\n");
    }
    if settings.shiny_odds != 8192 && settings.shiny_odds != 0 {
        let _ = writeln!(log, "== Chromatiques ==\nTaux de chromatiques : pas encore pris en charge sur GBA, inchangé (1 / 8192).\n");
    }

    // 6. Dresseurs.
    let trainer_pokemon = if super::trainers_changed(settings) { randomize_trainers(g, &l.ctx, settings, seed, &mut log)? } else { 0 };

    let tag = KaleidoTag { tool: "Kaleido".into(), version: env!("CARGO_PKG_VERSION").into(), seed, share_code: code.clone() };
    g.rom_mut().set_signature(&serde_json::to_vec(&tag).unwrap_or_default())?;

    Ok(Outcome {
        seed,
        share_code: code,
        starters: chosen.iter().map(|&id| PokemonRef { id, name: l.ctx.name(id).to_string() }).collect(),
        wild_slots,
        trainer_pokemon,
        log,
    })
}

fn scale(level: u8, percent: u16) -> u8 {
    ((level as u32 * percent as u32 + 50) / 100).clamp(1, 100) as u8
}

fn randomize_wild(ctx: &Ctx, settings: &Settings, seed: u64, ban_unown: bool, areas: &mut [WildArea], log: &mut String) -> usize {
    let mut rng = rng_for(seed, "wild");
    let similar = settings.wild_similar_strength;
    let ok = |s: u16| !(ban_unown && s == UNOWN);
    let mut global: HashMap<u16, u16> = HashMap::new();
    if settings.wild == WildMode::Global {
        let mut order: Vec<u16> = (1..=ctx.count).collect();
        order.shuffle(&mut rng);
        let mut used: Vec<u16> = Vec::new();
        for s in order {
            let new = ctx.pick(&mut rng, similar.then_some(s), ok, &used);
            used.push(new);
            global.insert(s, new);
        }
    }
    let _ = writeln!(log, "== Pokémon sauvages ==");
    let mut total = 0;
    for a in areas.iter_mut() {
        let mut local: HashMap<u16, u16> = HashMap::new();
        let mut changes: Vec<(u16, u16)> = Vec::new();
        for slot in &mut a.slots {
            let old = slot.species;
            let new = match settings.wild {
                WildMode::Unchanged => old,
                WildMode::Random => ctx.pick(&mut rng, similar.then_some(old), ok, &[]),
                WildMode::Global => *global.get(&old).unwrap_or(&old),
                WildMode::Area => match local.get(&old) {
                    Some(&n) => n,
                    None => {
                        let taken: Vec<u16> = local.values().copied().collect();
                        let n = ctx.pick(&mut rng, similar.then_some(old), ok, &taken);
                        local.insert(old, n);
                        n
                    }
                },
            };
            slot.species = new;
            if settings.wild_level_percent != 100 {
                slot.min_level = scale(slot.min_level, settings.wild_level_percent);
                slot.max_level = scale(slot.max_level, settings.wild_level_percent).max(slot.min_level);
            }
            if !changes.contains(&(old, new)) {
                changes.push((old, new));
            }
            total += 1;
        }
        let list: Vec<String> = changes.iter().map(|(o, n)| format!("{} → {}", ctx.name(*o), ctx.name(*n))).collect();
        let _ = writeln!(log, "Carte {}.{} ({}) : {}", a.bank, a.map, a.kind.label(), list.join(", "));
    }
    let _ = writeln!(log);
    total
}

fn randomize_trainers(g: &mut GbaGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<usize, RomError> {
    let mut rng = rng_for(seed, "trainers");
    let types: Vec<PokeType> = super::all_types(3);
    let mut total = 0;
    let _ = writeln!(log, "== Dresseurs ==");
    for mut t in gen3::trainers(g)? {
        if t.party.is_empty() || t.party.iter().any(|m| m.species == 0) {
            continue;
        }
        let theme = (settings.trainers == TrainerMode::TypeThemed)
            .then(|| ctx.types(t.party[0].species).first().copied().unwrap_or_else(|| *types.choose(&mut rng).unwrap()));
        let mut used = Vec::new();
        let mut lines = Vec::new();
        let has_moves = t.has_moves();
        for m in &mut t.party {
            let (old, old_level) = (m.species, m.level);
            if settings.trainers != TrainerMode::Unchanged {
                let similar = settings.trainers_similar_strength.then_some(m.species);
                m.species = ctx.pick(&mut rng, similar, |s| theme.is_none_or(|ty| ctx.types(s).contains(&ty)), &used);
                used.push(m.species);
            }
            m.level = super::scale_level(m.level, settings.trainer_level_percent);
            if settings.trainer_evolutions {
                m.species = extras::evolve_for_level(ctx, &mut rng, m.species, m.level);
            }
            if settings.trainer_max_ivs {
                m.iv = 255;
            }
            if has_moves {
                if let Some(ls) = ctx.learnsets.get(m.species as usize) {
                    m.moves = learnsets::moves_at_level(ls, m.level);
                }
            }
            lines.push(format!("{} niv. {} → {} niv. {}", ctx.name(old), old_level, ctx.name(m.species), m.level));
            total += 1;
        }
        gen3::set_party(g, &t)?;
        let theme_label = theme.map(|ty| format!(" (type {})", ty.name_fr())).unwrap_or_default();
        let _ = writeln!(log, "Dresseur n°{} {}{theme_label} : {}", t.index, t.name, lines.join(" ; "));
    }
    let _ = writeln!(log);
    Ok(total)
}

fn randomize_statics(g: &mut GbaGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<(), RomError> {
    let s = &settings.statics;
    if s.mode == StaticMode::Unchanged && s.level_modifier == 0 {
        return Ok(());
    }
    let mut rng: ChaCha8Rng = rng_for(seed, "statics");
    let current = gen3::statics(g);
    let _ = writeln!(log, "== Pokémon fixes ==");
    for (i, (species, levels)) in current.iter().enumerate() {
        if *species == 0 {
            continue;
        }
        let legendary = LEGENDARIES.contains(species);
        let new = match s.mode {
            StaticMode::Unchanged => *species,
            StaticMode::SwapLegendaries => {
                let pool: Vec<u16> = (1..=ctx.count).filter(|&x| LEGENDARIES.contains(&x) == legendary && x != *species).collect();
                *pool.choose(&mut rng).unwrap_or(species)
            }
            StaticMode::SimilarStrength => ctx.pick(&mut rng, Some(*species), |_| true, &[*species]),
            StaticMode::Random => ctx.pick(&mut rng, None, |_| true, &[]),
        };
        let level = levels.first().map(|&l| {
            let pct = (100 + s.level_modifier as i32).max(1) as u16;
            scale(l, pct)
        });
        gen3::set_static(g, i, new, level.filter(|_| s.level_modifier != 0))?;
        let label = &g.entry.statics[i].label;
        let lv = level.map(|l| format!(" niv. {l}")).unwrap_or_default();
        let _ = writeln!(log, "{} → {}{lv} ({label})", ctx.name(*species), ctx.name(new));
    }
    let _ = writeln!(log);
    Ok(())
}

fn move_type(id: u16) -> Option<u8> {
    crate::dex::move_info_in(crate::dex::Game::E, id).map(|m| m.type_id)
}

fn good_damaging(id: u16) -> bool {
    crate::dex::move_info_in(crate::dex::Game::E, id)
        .is_some_and(|m| m.category != crate::dex::MoveCategory::Status && m.power.unwrap_or(0) >= MIN_DAMAGING_POWER && m.accuracy.unwrap_or(100) >= 80)
}

fn randomize_tms(g: &mut GbaGameRom, ctx: &Ctx, settings: &Settings, seed: u64, log: &mut String) -> Result<(), RomError> {
    let m = &settings.moves;
    let changes_compat = m.tm_compat != CompatMode::Unchanged || m.full_hm_compat || m.levelup_sanity;
    if !m.random_tms && !changes_compat {
        if m.random_tutors || m.tutor_compat != CompatMode::Unchanged {
            let _ = writeln!(log, "== Donneurs de capacités ==\nPas encore pris en charge sur GBA, inchangés.\n");
        }
        return Ok(());
    }
    let mut rng = rng_for(seed, "tms");
    let mut tms = gen3::tm_moves(g)?;
    if m.random_tms {
        let mut pool: Vec<u16> = (1..=MAX_MOVE).filter(|x| *x != STRUGGLE && !HM_MOVES.contains(x)).filter(|x| !(m.no_game_breaking && GAME_BREAKING.contains(x))).collect();
        pool.shuffle(&mut rng);
        let keep = |mv: u16| m.keep_field_moves && FIELD_TM_MOVES.contains(&mv);
        let wanted_good = (TM_COUNT * m.good_damaging_percent as usize).div_ceil(100);
        let mut good_count = 0;
        let mut taken: Vec<u16> = tms.iter().copied().filter(|&mv| keep(mv)).collect();
        let _ = writeln!(log, "== CT ==");
        for (k, tm) in tms.iter_mut().enumerate() {
            if keep(*tm) {
                continue;
            }
            let need_good = good_count < wanted_good;
            let pick = pool.iter().position(|x| !taken.contains(x) && (!need_good || good_damaging(*x))).or_else(|| pool.iter().position(|x| !taken.contains(x)));
            if let Some(p) = pick {
                let new = pool.remove(p);
                if good_damaging(new) {
                    good_count += 1;
                }
                let _ = writeln!(log, "CT{:02} : {} → {}", k + 1, crate::dex::move_name(*tm).unwrap_or("?"), crate::dex::move_name(new).unwrap_or("?"));
                *tm = new;
                taken.push(new);
            }
        }
        let _ = writeln!(log);
        gen3::set_tm_moves(g, &tms)?;
    }
    if changes_compat {
        let all: Vec<u16> = tms.iter().copied().chain(HM_MOVES).collect();
        for s in 1..=SPECIES_COUNT {
            let mut bits = gen3::tmhm_compat(g, s)?;
            let types: Vec<u8> = ctx.types(s).iter().map(|&t| super::type_index(3, t)).collect();
            for (b, &mv) in all.iter().enumerate() {
                let is_hm = b >= TM_COUNT;
                if is_hm && m.full_hm_compat {
                    bits[b] = true;
                    continue;
                }
                bits[b] = match m.tm_compat {
                    CompatMode::Unchanged => bits[b],
                    CompatMode::Full => true,
                    CompatMode::Random => rng.gen_bool(0.5),
                    CompatMode::RandomPreferType => {
                        let t = move_type(mv).unwrap_or(0);
                        let p = if types.contains(&t) {
                            0.9
                        } else if t == 0 {
                            0.5
                        } else {
                            0.25
                        };
                        rng.gen_bool(p)
                    }
                };
                if m.levelup_sanity && ctx.learnsets.get(s as usize).is_some_and(|ls| ls.iter().any(|&(x, _)| x == mv)) {
                    bits[b] = true;
                }
            }
            gen3::set_tmhm_compat(g, s, &bits)?;
        }
        let _ = writeln!(log, "== Compatibilité CT / CS ==\nModifiée pour {} espèces.\n", SPECIES_COUNT);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gba_rom::synthetic;
    use crate::randomizer::{StarterMode, StatsMode};

    fn rom(code: &str) -> GbaGameRom {
        let mut r = synthetic::build(code, 0);
        synthetic::add_grass(&mut r, 3, 19, &[16, 16, 19, 19, 16, 19, 16, 19, 10, 10, 13, 13]);
        GbaGameRom::from_rom(r).unwrap()
    }

    fn chaos() -> Settings {
        Settings {
            starters: StarterMode::Random,
            wild: WildMode::Random,
            trainers: TrainerMode::Random,
            stats: StatsMode::Shuffle,
            random_types: true,
            random_abilities: true,
            easy_evolutions: true,
            random_movesets: true,
            trainer_max_ivs: true,
            ..Settings::default()
        }
    }

    #[test]
    fn randomize_synthetic_emerald_and_firered() {
        for code in ["BPEF", "BPRF", "AXVE"] {
            let mut g = rom(code);
            let settings = chaos();
            let preview = preview_starters(&g, &settings, 1234).unwrap();
            let out = randomize(&mut g, &settings, 1234).unwrap();
            assert_eq!(out.starters.iter().map(|s| s.id).collect::<Vec<_>>(), preview.iter().map(|s| s.id).collect::<Vec<_>>(), "{code}");
            assert_eq!(gen3::starters(&g).unwrap().to_vec(), preview.iter().map(|s| s.id).collect::<Vec<_>>());
            assert_eq!(out.wild_slots, 12);
            assert_eq!(out.trainer_pokemon, 2);
            assert!(out.log.contains("vraie ROM"));
            // La ROM se relit : signature, fiches cohérentes, même résultat pour la même seed.
            let bytes = g.rom().to_bytes();
            let again = GbaGameRom::from_rom(kaleido_formats::gba::GbaRom::from_bytes(bytes.clone()).unwrap()).unwrap();
            let tag: KaleidoTag = serde_json::from_slice(again.rom().signature().unwrap()).unwrap();
            assert_eq!(tag.seed, 1234);
            assert_eq!(again.species().unwrap().len(), 386);
            let mut g2 = rom(code);
            randomize(&mut g2, &settings, 1234).unwrap();
            assert_eq!(g2.rom().to_bytes(), bytes, "{code} : même seed, même ROM");
        }
    }

    #[test]
    fn unchanged_settings_touch_only_the_signature() {
        let mut g = rom("BPRF");
        let before = g.rom().data().to_vec();
        randomize(&mut g, &Settings::default(), 7).unwrap();
        let after = g.rom().data();
        let diff: Vec<usize> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
        // Correctifs d'obéissance absents de la ROM synthétique : seule la signature change.
        assert!(!diff.is_empty() && diff.iter().all(|&i| i >= before.len() - 0x10000), "{} octets hors de la signature", diff.len());
    }

    #[test]
    fn tms_and_statics() {
        let mut g = rom("BPEF");
        let mut s = Settings::default();
        s.moves.random_tms = true;
        s.moves.no_game_breaking = true;
        s.moves.tm_compat = CompatMode::Full;
        s.statics.mode = StaticMode::SwapLegendaries;
        let out = randomize(&mut g, &s, 99).unwrap();
        let tms = gen3::tm_moves(&g).unwrap();
        assert!(tms.iter().all(|m| !HM_MOVES.contains(m) && !GAME_BREAKING.contains(m) && *m != 0));
        let mut sorted = tms.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), TM_COUNT);
        assert!(gen3::tmhm_compat(&g, 25).unwrap()[..TM_COUNT].iter().all(|&b| b));
        for (sp, _) in gen3::statics(&g) {
            assert!(LEGENDARIES.contains(&sp), "{sp}");
        }
        assert!(out.log.contains("== CT =="));
    }
}
