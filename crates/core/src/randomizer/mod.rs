//! Randomizer Gen 4 / 5 (Platine, Noire, Blanche).
//!
//! Chaque partie (types, statistiques, starters, sauvages, dresseurs) tire ses
//! nombres d'un générateur dérivé de la seed et de son nom : l'aperçu des
//! starters est donc identique au résultat final, et changer une option ne
//! bouleverse pas les autres parties.

pub mod ctr;
mod ctr_gen7;
mod ctr_xy;
mod extras;
pub mod items;
pub mod moves;
pub mod settings;
pub mod statics;

use std::collections::HashMap;
use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;

pub use settings::{parse_share_code, presets, share_code, CatchRateMode, Preset, Settings, StarterMode, StatsMode, TrainerMode, WildMode};

use crate::data::evolutions::{self, EvolutionInfo};
use crate::data::learnsets::{self, Learnset};
use crate::data::{encounters, starters, trainers, DataPaths};
use crate::pokemon::{Personal, PokeType};
use crate::rom::{GameRom, RomError};

/// Légendaires et fabuleux des Gen 1 à 5 (n° national).
const LEGENDARIES: &[u16] = &[
    144, 145, 146, 150, 151, 243, 244, 245, 249, 250, 251, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 480, 481, 482, 483, 484, 485, 486, 487,
    488, 489, 490, 491, 492, 493, 494, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648, 649, 716, 717, 718, 719, 720, 721,
    // Gen 7 (liste de l'Universal Pokémon Randomizer, sans les Ultra-Chimères) : Type:0,
    // Silvallié, Tokos, Cosmog et ses évolutions, Necrozma, Magearna, Marshadow, Zeraora.
    772, 773, 785, 786, 787, 788, 789, 790, 791, 792, 800, 801, 802, 807,
];

/// Talents jamais attribués au hasard : Garde Mystik, Multitype, Illusion, Mode Transe,
/// et les talents de changement de forme de la Gen 7 (Bouclier-Carcan, Banc, Fantômasque,
/// Synergie, Rassemblement, Système Alpha).
const BANNED_ABILITIES: &[u16] = &[25, 121, 149, 161, 197, 208, 209, 210, 211, 225];

/// Munja (1 PV) : ses statistiques ne sont jamais modifiées.
const SHEDINJA: u16 = 292;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PokemonRef {
    pub id: u16,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub seed: u64,
    pub share_code: String,
    pub starters: Vec<PokemonRef>,
    pub wild_slots: usize,
    pub trainer_pokemon: usize,
    pub log: String,
}

/// Signature écrite dans les ROMs générées (lue par la détection des fichiers).
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KaleidoTag {
    pub tool: String,
    pub version: String,
    pub seed: u64,
    pub share_code: String,
}

fn rng_for(seed: u64, part: &str) -> ChaCha8Rng {
    // FNV-1a du nom de la partie, pour des flux indépendants.
    let salt = part.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    ChaCha8Rng::seed_from_u64(seed ^ salt)
}

pub fn supports(game: &GameRom) -> bool {
    DataPaths::for_game(game.game).is_some()
}

/// Contexte partagé : données lues une fois et mises à jour au fil des étapes.
/// Indépendant de la console : les fichiers sont déjà extraits (NARC ou GARC).
pub(crate) struct Ctx {
    gen: u8,
    count: u16,
    names: Vec<String>,
    /// Fiche « personal » de chaque espèce (index = n° national).
    personal: Vec<Vec<u8>>,
    evo: EvolutionInfo,
    /// Évolutions de chaque espèce (méthode, paramètre, cible).
    evo_tables: Vec<Vec<evolutions::Evolution>>,
    learnsets: Vec<Learnset>,
    no_legendaries: bool,
    max_ability: u16,
    /// Formats des rencontres et des équipes (différents en Diamant / Perle et HGSS).
    encounter_format: encounters::Format,
    team_format: trainers::TeamFormat,
}

/// Fichiers bruts nécessaires pour construire le contexte.
pub(crate) struct Sources<'a> {
    pub gen: u8,
    pub count: u16,
    pub names: Vec<String>,
    pub personal: Vec<Vec<u8>>,
    pub evolutions: &'a [Vec<u8>],
    pub learnsets: &'a [Vec<u8>],
    pub max_ability: u16,
}

impl Ctx {
    pub(crate) fn new(src: Sources, settings: &Settings) -> Result<Self, RomError> {
        let count = src.count as usize;
        if src.names.len() <= count || src.personal.len() <= count {
            return Err(RomError::Layout("données d'espèces incomplètes".into()));
        }
        let evo_tables: Vec<_> = src.evolutions.iter().map(|f| evolutions::read(f)).collect();
        Ok(Self {
            gen: src.gen,
            count: src.count,
            names: src.names,
            personal: src.personal,
            evo: EvolutionInfo::build(&evo_tables, count),
            evo_tables,
            learnsets: src.learnsets.iter().map(|f| learnsets::read(src.gen, f)).collect(),
            no_legendaries: settings.no_legendaries,
            max_ability: src.max_ability,
            encounter_format: encounters::Format::for_generation(src.gen),
            team_format: trainers::TeamFormat::for_generation(src.gen),
        })
    }

    fn load(game: &GameRom, paths: &DataPaths, settings: &Settings) -> Result<Self, RomError> {
        let evolutions = game.narc(paths.evolutions)?.files;
        let learnsets = game.narc(paths.learnsets)?.files;
        let src = Sources {
            gen: game.generation(),
            count: game.layout.species_count,
            names: game.text_file(game.layout.species_names)?,
            personal: game.narc(game.layout.personal)?.files,
            evolutions: &evolutions,
            learnsets: &learnsets,
            max_ability: paths.max_ability,
        };
        let mut ctx = Self::new(src, settings)?;
        ctx.encounter_format = encounters::Format::for_game(game.game);
        ctx.team_format = trainers::TeamFormat::for_game(game.game);
        Ok(ctx)
    }

    fn name(&self, id: u16) -> &str {
        self.names.get(id as usize).map_or("?", String::as_str)
    }

    fn personal(&self, id: u16) -> Personal {
        Personal { generation: self.gen, data: self.personal[id as usize].clone() }
    }

    fn bst(&self, id: u16) -> u16 {
        self.personal(id).base_stats().total()
    }

    fn types(&self, id: u16) -> Vec<PokeType> {
        self.personal(id).types()
    }

    fn allowed(&self, id: u16) -> bool {
        !(self.no_legendaries && LEGENDARIES.contains(&id))
    }

    /// Choisit une espèce : filtrée, différente de `exclude`, et proche en
    /// puissance de `like` si demandé (tolérance élargie si trop peu de choix).
    fn pick(&self, rng: &mut ChaCha8Rng, like: Option<u16>, filter: impl Fn(u16) -> bool, exclude: &[u16]) -> u16 {
        let base: Vec<u16> = (1..=self.count).filter(|&s| self.allowed(s) && filter(s) && !exclude.contains(&s)).collect();
        let base = if base.is_empty() { (1..=self.count).filter(|&s| self.allowed(s)).collect() } else { base };
        if let Some(orig) = like {
            let target = self.bst(orig) as f32;
            for tolerance in [0.08, 0.15, 0.25, 0.4] {
                let close: Vec<u16> = base.iter().copied().filter(|&s| (self.bst(s) as f32 - target).abs() <= target * tolerance).collect();
                if close.len() >= 4 {
                    return *close.choose(rng).unwrap();
                }
            }
        }
        *base.choose(rng).unwrap_or(&1)
    }
}

/// Aperçu des starters, sans modifier la ROM.
pub fn preview_starters(game: &GameRom, settings: &Settings, seed: u64) -> Result<Vec<PokemonRef>, RomError> {
    let paths = DataPaths::for_game(game.game).ok_or_else(unsupported)?;
    let mut ctx = Ctx::load(game, &paths, settings)?;
    apply_personal(&mut ctx, settings, seed, &mut String::new());
    let current = starters::read(game, paths.starters)?;
    let chosen = choose_starters(&ctx, settings, seed, current);
    Ok(chosen.iter().map(|&id| PokemonRef { id, name: ctx.name(id).to_string() }).collect())
}

fn unsupported() -> RomError {
    RomError::Unsupported("ce jeu n'est pas pris en charge par le randomizer DS".into())
}

/// Randomise le jeu en mémoire. Appeler `game.save()` ensuite pour écrire la ROM.
pub fn randomize(game: &mut GameRom, settings: &Settings, seed: u64) -> Result<Outcome, RomError> {
    let paths = DataPaths::for_game(game.game).ok_or_else(unsupported)?;
    let mut ctx = Ctx::load(game, &paths, settings)?;
    let mut log = String::new();
    let code = share_code(seed, settings);
    let _ = writeln!(log, "Kaleido — journal de randomisation");
    let _ = writeln!(log, "Jeu : {} ({})", game.game.name_fr(), game.rom().header().game_code);
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}\n");
    if !game.layout.verified {
        let _ = writeln!(log, "Attention : emplacements des données de ce jeu non vérifiés sur une vraie ROM (à l'essai).\n");
    }

    // 1. Fiches des espèces (types, statistiques, talents).
    if apply_personal(&mut ctx, settings, seed, &mut log) {
        let mut narc = game.narc(game.layout.personal)?;
        narc.files = ctx.personal.clone();
        game.replace_narc(game.layout.personal, &narc)?;
    }

    // 2. Starters.
    let current = starters::read(game, paths.starters)?;
    let chosen = choose_starters(&ctx, settings, seed, current);
    if chosen != current {
        let labels = chosen.map(|s| {
            let type_name = ctx.types(s).first().map_or("Normal", |t| t.name_fr()).to_string();
            (type_name, ctx.name(s).to_string())
        });
        starters::write(game, paths.starters, chosen, &labels)?;
        let _ = writeln!(log, "== Starters ==");
        for (old, new) in current.iter().zip(chosen) {
            let _ = writeln!(log, "{} → {}", ctx.name(*old), ctx.name(new));
        }
        let _ = writeln!(log);
    }
    statics::apply(game, &ctx, &settings.statics, seed, &mut log)?; // Pokémon fixes, dons et échanges

    // 3. Évolutions et attaques apprises (avant les dresseurs, qui s'en servent).
    if settings.easy_evolutions {
        let mut narc = game.narc(paths.evolutions)?;
        extras::easy_evolutions(&mut ctx, &mut narc.files, &mut log);
        game.replace_narc(paths.evolutions, &narc)?;
    }
    if settings.random_movesets {
        let max_move = game.text_file(paths.move_names)?.len().saturating_sub(1) as u16;
        let mut narc = game.narc(paths.learnsets)?;
        extras::random_movesets(&mut ctx, &mut narc.files, max_move, seed, &mut log);
        game.replace_narc(paths.learnsets, &narc)?;
    }
    moves::apply(game, &mut ctx, &settings.moves, seed, &mut log)?;

    // 4. Pokémon sauvages.
    let wild_slots = if settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100 {
        let mut narc = game.narc(paths.encounters)?;
        let n = randomize_wild(&ctx, settings, seed, &mut narc.files, &mut log);
        game.replace_narc(paths.encounters, &narc)?;
        n
    } else {
        0
    };

    // 4 bis. Objets ramassables et boutiques.
    items::randomize_items(game, &settings.items, seed, &mut log)?;

    // 5. Taux de chromatiques (modification du code du jeu).
    let threshold = crate::data::shiny::threshold_for_odds(settings.shiny_odds);
    if threshold != crate::data::shiny::DEFAULT_THRESHOLD {
        let odds = crate::data::shiny::set_threshold(game.rom_mut(), threshold)?;
        let rate = if odds == 1 { "tous les Pokémon".to_string() } else { format!("1 / {odds}") };
        let _ = writeln!(log, "== Chromatiques ==\nTaux : {rate} (au lieu de 1 / 8192)\n");
    }

    // 6. Dresseurs.
    let trainer_pokemon = if trainers_changed(settings) {
        let mut trdata = game.narc(paths.trainer_data)?;
        let mut trpoke = game.narc(paths.trainer_pokemon)?;
        let moves = game.text_file(paths.move_names)?;
        let n = randomize_trainers(&ctx, settings, seed, &mut trdata.files, &mut trpoke.files, &moves, &mut log);
        game.replace_narc(paths.trainer_data, &trdata)?;
        game.replace_narc(paths.trainer_pokemon, &trpoke)?;
        n
    } else {
        0
    };

    // Signature lisible par la bibliothèque : « ROM randomisée par Kaleido, seed … ».
    let tag = KaleidoTag { tool: "Kaleido".into(), version: env!("CARGO_PKG_VERSION").into(), seed, share_code: code.clone() };
    game.rom_mut().set_signature(serde_json::to_vec(&tag).unwrap_or_default());

    Ok(Outcome {
        seed,
        share_code: code,
        starters: chosen.iter().map(|&id| PokemonRef { id, name: ctx.name(id).to_string() }).collect(),
        wild_slots,
        trainer_pokemon,
        log,
    })
}

/// Les dresseurs sont-ils modifiés par l'un des réglages ?
fn trainers_changed(settings: &Settings) -> bool {
    settings.trainers != TrainerMode::Unchanged
        || settings.trainer_level_percent != 100
        || settings.trainer_evolutions
        || settings.trainer_max_ivs
        || settings.random_movesets
}

/// Index de type tel que stocké par la génération.
fn type_index(gen: u8, t: PokeType) -> u8 {
    (0..18).find(|&i| PokeType::from_index(gen, i) == Some(t)).unwrap_or(0)
}

fn all_types(gen: u8) -> Vec<PokeType> {
    (0..18).filter_map(|i| PokeType::from_index(gen, i)).filter(|t| *t != PokeType::Mystery).collect()
}

/// Types, statistiques et talents. Renvoie `true` si quelque chose a changé.
pub(crate) fn apply_personal(ctx: &mut Ctx, settings: &Settings, seed: u64, log: &mut String) -> bool {
    let changed = settings.random_types
        || settings.stats != StatsMode::Unchanged
        || settings.random_abilities
        || settings.catch_rate != CatchRateMode::Unchanged;
    if !changed {
        return false;
    }

    if settings.catch_rate != CatchRateMode::Unchanged {
        for s in 1..=ctx.count as usize {
            let rate = &mut ctx.personal[s][8];
            *rate = match settings.catch_rate {
                CatchRateMode::Doubled => rate.saturating_mul(2),
                CatchRateMode::Max => 255,
                CatchRateMode::Unchanged => *rate,
            };
        }
    }
    let gen = ctx.gen;
    let count = ctx.count as usize;

    if settings.random_types {
        let mut rng = rng_for(seed, "types");
        let types = all_types(gen);
        // Une même famille d'évolution garde les mêmes types.
        for base in (1..=count).filter(|&s| ctx.evo.stage[s] == 0) {
            let t1 = *types.choose(&mut rng).unwrap();
            let t2 = if rng.gen_bool(0.5) { *types.choose(&mut rng).unwrap() } else { t1 };
            let mut family = vec![base];
            while let Some(s) = family.pop() {
                let d = &mut ctx.personal[s];
                d[6] = type_index(gen, t1);
                d[7] = type_index(gen, t2);
                family.extend(ctx.evo.targets[s].iter().map(|&t| t as usize));
            }
        }
    }

    if settings.stats != StatsMode::Unchanged {
        let mut rng = rng_for(seed, "stats");
        for s in (1..=count).filter(|&s| s as u16 != SHEDINJA) {
            let d = &mut ctx.personal[s];
            let mut stats: Vec<u8> = d[0..6].to_vec();
            match settings.stats {
                StatsMode::Shuffle => stats.shuffle(&mut rng),
                StatsMode::Random => stats = redistribute(&stats, &mut rng),
                StatsMode::Unchanged => {}
            }
            d[0..6].copy_from_slice(&stats);
        }
    }

    if settings.random_abilities {
        let mut rng = rng_for(seed, "abilities");
        let pool: Vec<u16> = (1..=ctx.max_ability).filter(|a| !BANNED_ABILITIES.contains(a)).collect();
        let slots = if gen <= 4 { 0x16..0x18 } else { 0x18..0x1B };
        for s in 1..=count {
            let d = &mut ctx.personal[s];
            let mut used: Vec<u16> = Vec::new();
            for at in slots.clone() {
                if d[at] == 0 && at != slots.start {
                    continue; // pas de second talent à l'origine
                }
                let choices: Vec<u16> = pool.iter().copied().filter(|a| !used.contains(a)).collect();
                let a = choices.choose(&mut rng).copied().unwrap_or(1);
                used.push(a);
                d[at] = a as u8;
            }
        }
    }

    let _ = writeln!(log, "== Espèces ==");
    for s in 1..=ctx.count {
        let p = ctx.personal(s);
        let b = p.base_stats();
        let types: Vec<_> = p.types().iter().map(|t| t.name_fr()).collect();
        let _ = writeln!(
            log,
            "{:03} {:<12} {:<18} PV {:3} Att {:3} Déf {:3} AtqS {:3} DéfS {:3} Vit {:3}",
            s,
            ctx.name(s),
            types.join("/"),
            b.hp,
            b.attack,
            b.defense,
            b.sp_attack,
            b.sp_defense,
            b.speed
        );
    }
    let _ = writeln!(log);
    true
}

/// Répartit à nouveau le total des statistiques (chaque stat entre 10 et 255).
fn redistribute(stats: &[u8], rng: &mut ChaCha8Rng) -> Vec<u8> {
    let total: u32 = stats.iter().map(|&s| s as u32).sum();
    let weights: Vec<f32> = (0..6).map(|_| rng.gen_range(0.4..1.6)).collect();
    let sum: f32 = weights.iter().sum();
    let mut out: Vec<u32> = weights.iter().map(|w| ((w / sum) * total as f32).round().clamp(10.0, 255.0) as u32).collect();
    // Ajuste l'arrondi pour garder exactement le même total.
    let mut diff = total as i64 - out.iter().sum::<u32>() as i64;
    let mut i = 0;
    while diff != 0 && i < 6000 {
        let k = i % 6;
        if diff > 0 && out[k] < 255 {
            out[k] += 1;
            diff -= 1;
        } else if diff < 0 && out[k] > 10 {
            out[k] -= 1;
            diff += 1;
        }
        i += 1;
    }
    out.into_iter().map(|v| v as u8).collect()
}

fn choose_starters(ctx: &Ctx, settings: &Settings, seed: u64, current: [u16; 3]) -> [u16; 3] {
    let mut rng = rng_for(seed, "starters");
    let three_stage = |s: u16| ctx.evo.stage[s as usize] == 0 && ctx.evo.targets[s as usize].iter().any(|&t| ctx.evo.evolves[t as usize]);
    let mut out = current;
    match settings.starters {
        StarterMode::Unchanged => {}
        StarterMode::Random | StarterMode::ThreeStage => {
            for i in 0..3 {
                let taken = out[..i].to_vec();
                out[i] = if settings.starters == StarterMode::Random {
                    ctx.pick(&mut rng, None, |_| true, &taken)
                } else {
                    ctx.pick(&mut rng, None, three_stage, &taken)
                };
            }
        }
        StarterMode::Triangle => {
            for (i, t) in [PokeType::Grass, PokeType::Fire, PokeType::Water].into_iter().enumerate() {
                out[i] = ctx.pick(&mut rng, None, |s| three_stage(s) && ctx.types(s).contains(&t), &out[..i]);
            }
        }
        StarterMode::Custom => {
            // Espèces choisies par le joueur ; une case vide ou hors du jeu garde l'original.
            for (slot, &wanted) in out.iter_mut().zip(&settings.custom_starters) {
                if (1..=ctx.count).contains(&wanted) {
                    *slot = wanted;
                }
            }
        }
    }
    out
}

/// Second trio de starters (X / Y : Pokémon de Kanto du Professeur Platane), tiré avec
/// les mêmes règles que les starters mais sur un autre flux aléatoire, sans reprendre
/// les espèces de `avoid` (les starters principaux).
pub(crate) fn choose_second_trio(ctx: &Ctx, settings: &Settings, seed: u64, current: [u16; 3], avoid: &[u16]) -> [u16; 3] {
    let trio = Settings { starters: settings.kanto_starters, custom_starters: settings.custom_kanto_starters, ..settings.clone() };
    let mut out = current;
    for attempt in 0..32u64 {
        out = choose_starters(ctx, &trio, seed ^ 0x4B41_4E54_4F00 ^ attempt.wrapping_mul(0x9E37_79B9), current);
        if trio.starters == StarterMode::Custom || !out.iter().any(|s| avoid.contains(s)) {
            break;
        }
    }
    out
}

fn scale_level(level: u16, percent: u16) -> u16 {
    ((level as u32 * percent as u32 + 50) / 100).clamp(1, 100) as u16
}

pub(crate) fn randomize_wild(ctx: &Ctx, settings: &Settings, seed: u64, files: &mut [Vec<u8>], log: &mut String) -> usize {
    let mut rng = rng_for(seed, "wild");
    let similar = settings.wild_similar_strength;
    let factor = settings.wild_level_percent as f32 / 100.0;

    // Correspondance globale : chaque espèce d'origine reçoit un remplaçant distinct si possible.
    let mut global: HashMap<u16, u16> = HashMap::new();
    if settings.wild == WildMode::Global {
        let mut order: Vec<u16> = (1..=ctx.count).collect();
        order.shuffle(&mut rng);
        let mut used: Vec<u16> = Vec::new();
        for s in order {
            let new = ctx.pick(&mut rng, similar.then_some(s), |_| true, &used);
            used.push(new);
            global.insert(s, new);
        }
    }

    let _ = writeln!(log, "== Pokémon sauvages ==");
    let mut total = 0;
    for (zone, file) in files.iter_mut().enumerate() {
        let slots = encounters::read_format(ctx.encounter_format, file);
        if slots.is_empty() {
            continue;
        }
        let mut area: HashMap<u16, u16> = HashMap::new();
        let mut changes: Vec<(u16, u16)> = Vec::new();
        for slot in &slots {
            let new = match settings.wild {
                WildMode::Unchanged => slot.species,
                WildMode::Random => ctx.pick(&mut rng, similar.then_some(slot.species), |_| true, &[]),
                WildMode::Global => *global.get(&slot.species).unwrap_or(&slot.species),
                WildMode::Area => match area.get(&slot.species) {
                    Some(&n) => n,
                    None => {
                        let taken: Vec<u16> = area.values().copied().collect();
                        let n = ctx.pick(&mut rng, similar.then_some(slot.species), |_| true, &taken);
                        area.insert(slot.species, n);
                        n
                    }
                },
            };
            encounters::set_species(file, slot, new);
            if factor != 1.0 {
                encounters::scale_levels(file, slot, factor);
            }
            if !changes.contains(&(slot.species, new)) {
                changes.push((slot.species, new));
            }
            total += 1;
        }
        let list: Vec<String> = changes.iter().map(|(o, n)| format!("{} → {}", ctx.name(*o), ctx.name(*n))).collect();
        let _ = writeln!(log, "Zone {zone:3} : {}", list.join(", "));
    }
    let _ = writeln!(log);
    total
}

pub(crate) fn randomize_trainers(
    ctx: &Ctx,
    settings: &Settings,
    seed: u64,
    trdata: &mut [Vec<u8>],
    trpoke: &mut [Vec<u8>],
    move_names: &[String],
    log: &mut String,
) -> usize {
    let mut rng = rng_for(seed, "trainers");
    let types = all_types(ctx.gen);
    let mut total = 0;
    let _ = writeln!(log, "== Dresseurs ==");

    let count = trdata.len().min(trpoke.len());
    for i in 1..count {
        let Some(mut team) = trainers::read_team_with(ctx.team_format, &trdata[i], &trpoke[i]) else { continue };
        if team.pokemon.is_empty() {
            continue;
        }
        // Type dominant : celui du premier Pokémon (garde le thème des champions).
        let theme = (settings.trainers == TrainerMode::TypeThemed)
            .then(|| ctx.types(team.pokemon[0].species).first().copied().unwrap_or_else(|| *types.choose(&mut rng).unwrap()));
        let mut lines = Vec::new();
        let mut used = Vec::new();
        for p in &mut team.pokemon {
            let (old_species, old_level) = (p.species, p.level);
            if settings.trainers != TrainerMode::Unchanged {
                let similar = settings.trainers_similar_strength.then_some(p.species);
                p.species = ctx.pick(&mut rng, similar, |s| theme.is_none_or(|t| ctx.types(s).contains(&t)), &used);
                p.form = 0;
                used.push(p.species);
            }
            p.level = scale_level(p.level, settings.trainer_level_percent);
            if settings.trainer_evolutions {
                let evolved = extras::evolve_for_level(ctx, &mut rng, p.species, p.level);
                if evolved != p.species {
                    p.species = evolved;
                    p.form = 0;
                }
            }
            if settings.trainer_max_ivs {
                p.difficulty = 255;
            }
            if team.flags & trainers::FLAG_MOVES != 0 {
                if let Some(ls) = ctx.learnsets.get(p.species as usize) {
                    p.moves = learnsets::moves_at_level(ls, p.level);
                }
            }
            let mut line = format!("{} niv. {} → {} niv. {}", ctx.name(old_species), old_level, ctx.name(p.species), p.level);
            if team.flags & trainers::FLAG_MOVES != 0 {
                let moves: Vec<&str> = p.moves.iter().filter(|&&m| m != 0).filter_map(|&m| move_names.get(m as usize).map(String::as_str)).collect();
                line.push_str(&format!(" [{}]", moves.join(", ")));
            }
            lines.push(line);
            total += 1;
        }
        trpoke[i] = trainers::write_team_with(ctx.team_format, &team, &mut trdata[i]);
        let theme_label = theme.map(|t| format!(" (type {})", t.name_fr())).unwrap_or_default();
        let _ = writeln!(log, "Dresseur n°{i}{theme_label} : {}", lines.join(" ; "));
    }
    let _ = writeln!(log);
    total
}
