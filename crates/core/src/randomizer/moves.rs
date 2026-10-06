//! CT / CS et donneurs de capacités (portage de l'Universal Pokémon Randomizer,
//! `TMTutorMoveRandomizer` et `TMHMTutorCompatibilityRandomizer`).
//!
//! - Attaques des CT tirées au hasard (jamais les CS), sans Lutte ni attaque de
//!   CS, en écartant si demandé Sonicboom et Draco-Rage, en gardant si demandé
//!   les CT de capacités de terrain, avec un pourcentage minimal de « bonnes »
//!   attaques offensives. Les descriptions des CT, la couleur de leur icône et les
//!   dialogues qui les nomment (Platine) suivent.
//! - Compatibilité : inchangée, aléatoire (50 %), aléatoire en privilégiant le
//!   type (90 % même type, 50 % Normal, 25 % sinon), ou totale. Les CS requises
//!   tôt (Coupe, Éclate-Roc) ont une chance ×1,8.
//! - Donneurs de capacités : Platine, Noire 2 et Blanche 2 (Noire/Blanche n'en ont pas).

use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use super::{rng_for, Ctx};
use crate::data::machines::{self, MachineSpec, MoveInfo, TutorTable};
use crate::data::DataPaths;
use crate::games::Game;
use crate::pokemon::PokeType;
use crate::rom::{GameRom, RomError};

/// Compatibilité des espèces avec les CT/CS ou les donneurs de capacités.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompatMode {
    #[default]
    Unchanged,
    /// Une chance sur deux pour chaque attaque.
    Random,
    /// Aléatoire, en privilégiant les attaques du type de l'espèce.
    RandomPreferType,
    /// Toutes les espèces apprennent toutes les attaques.
    Full,
}

/// Réglages des CT/CS et des donneurs de capacités.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MoveSettings {
    /// Attaques des CT tirées au hasard (les CS ne changent jamais).
    pub random_tms: bool,
    /// Attaques des donneurs de capacités tirées au hasard (Platine).
    pub random_tutors: bool,
    /// N'attribue jamais Sonicboom ni Draco-Rage (dégâts fixes, trop forts en début de jeu).
    pub no_game_breaking: bool,
    /// Garde les CT et donneurs qui enseignent une capacité de terrain (Flash, Tunnel…).
    pub keep_field_moves: bool,
    /// Part minimale (en %) d'attaques offensives fiables parmi les nouvelles attaques (0 = libre).
    pub good_damaging_percent: u8,
    /// Compatibilité avec les CT (et les CS, sauf `full_hm_compat`).
    pub tm_compat: CompatMode,
    /// Toutes les espèces peuvent apprendre toutes les CS.
    pub full_hm_compat: bool,
    /// Compatibilité avec les donneurs de capacités (Platine).
    pub tutor_compat: CompatMode,
    /// Compatibilité aléatoire : une évolution reprend celle de sa pré-évolution (avec quelques ajouts).
    pub follow_evolutions: bool,
    /// Une espèce peut toujours utiliser la CT / le donneur d'une attaque qu'elle apprend par niveau.
    pub levelup_sanity: bool,
}

impl MoveSettings {
    fn changes_tms(&self) -> bool {
        self.random_tms || self.tm_compat != CompatMode::Unchanged || self.full_hm_compat || self.levelup_sanity
    }

    fn changes_tutors(&self) -> bool {
        self.random_tutors || self.tutor_compat != CompatMode::Unchanged || self.levelup_sanity
    }
}

/// Lutte.
const STRUGGLE: u16 = 165;
/// Sonicboom et Draco-Rage (UPR `getGameBreakingMoves`).
const GAME_BREAKING: [u16; 2] = [49, 82];
/// Attaques jamais comptées comme « bonnes attaques offensives » (UPR `bannedForDamagingMove`).
const BAD_DAMAGING: [u16; 26] =
    [120, 138, 153, 173, 206, 248, 252, 264, 353, 364, 387, 389, 132, 99, 205, 301, 485, 704, 492, 255, 49, 82, 32, 12, 90, 329];
/// Capacités de terrain (UPR `dpptFieldMoves` / Gen 5 `fieldMoves`).
const FIELD_MOVES_GEN4: [u16; 12] = [15, 19, 57, 70, 148, 91, 100, 127, 249, 230, 432, 431];
const FIELD_MOVES_GEN5: [u16; 10] = [15, 19, 57, 70, 148, 91, 100, 127, 230, 291];
/// CS indispensables tôt dans l'aventure (Platine : Éclate-Roc, Coupe ; Noire/Blanche : Coupe).
const EARLY_HMS_GEN4: [u16; 2] = [249, 15];
const EARLY_HMS_GEN5: [u16; 1] = [15];
/// Puissance minimale d'une bonne attaque offensive (UPR `MIN_DAMAGING_MOVE_POWER`).
const MIN_DAMAGING_POWER: u8 = 50;
/// Caractères par ligne des descriptions d'objets (Platine).
const GEN4_CHARS_PER_LINE: usize = 38;
/// Dialogues de Platine qui nomment l'attaque d'une CT : (n° de CT, fichiers de texte).
const PT_TM_TEXTS: [(usize, &[usize]); 12] = [
    (27, &[561]),
    (42, &[594]),
    (48, &[61]),
    (56, &[517]),
    (63, &[143]),
    (66, &[504]),
    (67, &[98]),
    (76, &[67]),
    (77, &[488]),
    (78, &[470]),
    (88, &[574]),
    (92, &[500]),
];
/// Diamant / Perle (`TMText{}` de `[Diamond (U)]`, UPR-ZX ; mêmes fichiers en français).
const DP_TM_TEXTS: [(usize, &[usize]); 11] = [
    (42, &[538]),
    (48, &[54]),
    (56, &[466]),
    (63, &[135]),
    (66, &[453]),
    (67, &[90]),
    (76, &[60]),
    (77, &[437]),
    (78, &[419]),
    (88, &[518]),
    (92, &[449]),
];
/// HeartGold / SoulSilver (`TMText{}` de `[HeartGold (U)]`, UPR-ZX ; vérifié sur SoulSilver
/// IPGF : chaque fichier nomme bien l'attaque, parfois en majuscules ; CT10 et CT34 sont
/// écrites autrement dans les dialogues, « Puissance Cachée » et un dialogue sans le nom).
const HGSS_TM_TEXTS: [(usize, &[usize]); 29] = [
    (1, &[574]),
    (3, &[469]),
    (5, &[380]),
    (7, &[622]),
    (10, &[627]),
    (11, &[67]),
    (12, &[386]),
    (19, &[492]),
    (23, &[606]),
    (29, &[534]),
    (30, &[614]),
    (34, &[485]),
    (36, &[403]),
    (37, &[370]),
    (44, &[378]),
    (45, &[582]),
    (47, &[372]),
    (48, &[531]),
    (50, &[53]),
    (51, &[558]),
    (57, &[345]),
    (59, &[129, 631]),
    (70, &[56]),
    (80, &[462]),
    (83, &[397]),
    (84, &[514]),
    (85, &[452]),
    (89, &[567]),
    (92, &[454]),
];

/// Dialogues qui nomment l'attaque d'une CT, et largeur des descriptions d'objets.
fn tm_texts(game: Game) -> (&'static [(usize, &'static [usize])], usize) {
    match game {
        Game::Platinum => (&PT_TM_TEXTS, GEN4_CHARS_PER_LINE),
        Game::Diamond | Game::Pearl => (&DP_TM_TEXTS, GEN4_CHARS_PER_LINE),
        // `hgssTextCharsPerLine`.
        Game::HeartGold | Game::SoulSilver => (&HGSS_TM_TEXTS, 36),
        _ => (&[], GEN4_CHARS_PER_LINE),
    }
}

/// Ensemble des attaques du jeu.
struct MoveTable {
    names: Vec<String>,
    info: Vec<Option<MoveInfo>>,
    max: u16,
}

impl MoveTable {
    fn kind(&self, m: u16) -> Option<PokeType> {
        self.info.get(m as usize).copied().flatten().and_then(|i| i.kind)
    }

    fn name(&self, m: u16) -> &str {
        self.names.get(m as usize).map_or("?", String::as_str)
    }

    /// UPR `Move.isGoodDamaging` (sans tenir compte des attaques à coups multiples).
    fn good_damaging(&self, m: u16) -> bool {
        let Some(info) = self.info.get(m as usize).copied().flatten() else { return false };
        let perfect = info.accuracy == 0 || info.accuracy > 100;
        !BAD_DAMAGING.contains(&m) && (info.power >= 2 * MIN_DAMAGING_POWER || (info.power >= MIN_DAMAGING_POWER && (info.accuracy >= 90 || perfect)))
    }
}

/// Règles de tirage communes aux CT et aux donneurs.
struct PickRules<'a> {
    no_game_breaking: bool,
    keep_field_moves: bool,
    good_damaging_percent: u8,
    field_moves: &'a [u16],
    /// Attaques interdites en plus (CS, et CT pour les donneurs).
    excluded: &'a [u16],
}

/// Nouvelles attaques pour `old` (UPR `randomizeTMMoves` / `randomizeMoveTutorMoves`) :
/// toutes distinctes, en gardant à leur place les capacités de terrain si demandé.
fn pick_moves(old: &[u16], table: &MoveTable, rules: &PickRules, rng: &mut ChaCha8Rng) -> Vec<u16> {
    let keep = |m: u16| rules.keep_field_moves && rules.field_moves.contains(&m);
    let kept: Vec<u16> = old.iter().copied().filter(|&m| keep(m)).collect();
    let banned = |m: u16| m == STRUGGLE || rules.excluded.contains(&m) || kept.contains(&m) || (rules.no_game_breaking && GAME_BREAKING.contains(&m));
    let mut usable: Vec<u16> = (1..=table.max).filter(|&m| !banned(m)).collect();
    let mut damaging: Vec<u16> = usable.iter().copied().filter(|&m| table.good_damaging(m)).collect();

    let wanted = old.len() - kept.len();
    let mut good_left = (rules.good_damaging_percent.min(100) as f64 / 100.0 * wanted as f64).round() as usize;
    let mut picked = Vec::with_capacity(wanted);
    for _ in 0..wanted {
        let source = if good_left > 0 && !damaging.is_empty() { &damaging } else { &usable };
        let Some(&m) = source.choose(rng) else { break };
        picked.push(m);
        usable.retain(|&x| x != m);
        damaging.retain(|&x| x != m);
        good_left = good_left.saturating_sub(1);
    }
    // Mélange : sinon les bonnes attaques se retrouveraient dans les premières CT.
    picked.shuffle(rng);
    let mut picked = picked.into_iter();
    old.iter().map(|&m| if keep(m) { m } else { picked.next().unwrap_or(m) }).collect()
}

/// Une ligne de compatibilité : entrée de la table « personal » et un drapeau par attaque.
struct CompatRow {
    personal: usize,
    flags: Vec<bool>,
}

/// Paramètres d'un tirage de compatibilité.
struct CompatRules<'a> {
    prefer_type: bool,
    follow_evolutions: bool,
    /// Attaques favorisées (CS requises tôt).
    prioritized: &'a [u16],
}

/// Types (premier, second éventuel) de l'entrée `personal`.
fn types_of(ctx: &Ctx, personal: usize) -> (Option<PokeType>, Option<PokeType>) {
    let t = ctx.personal(personal as u16).types();
    (t.first().copied(), t.get(1).copied())
}

/// UPR `getMoveCompatibilityProbability`.
fn probability(types: (Option<PokeType>, Option<PokeType>), kind: Option<PokeType>, prioritized: bool, prefer_type: bool) -> f64 {
    let mut p: f64 = 0.5;
    if prefer_type {
        p = if kind.is_some() && (types.0 == kind || types.1 == kind) {
            0.9
        } else if kind == Some(PokeType::Normal) {
            0.5
        } else {
            0.25
        };
    }
    if prioritized {
        p = (p * 1.8).min(1.0);
    }
    p
}

/// UPR `randomizePokemonMoveCompatibility` / `copyPokemonMoveCompatibilityUpEvolutions`.
fn randomize_compat(ctx: &Ctx, rows: &mut [CompatRow], moves: &[u16], table: &MoveTable, rules: &CompatRules, rng: &mut ChaCha8Rng) {
    let count = ctx.count as usize;
    // Pré-évolution de chaque espèce (les formes n'en ont pas).
    let mut parent: Vec<Option<usize>> = vec![None; ctx.personal.len()];
    for (s, targets) in ctx.evo.targets.iter().enumerate().take(count + 1) {
        for &t in targets {
            if let Some(p) = parent.get_mut(t as usize) {
                p.get_or_insert(s);
            }
        }
    }
    // Les pré-évolutions d'abord.
    let stage = |p: usize| if p <= count { ctx.evo.stage.get(p).copied().unwrap_or(0) } else { 0 };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by_key(|&i| (stage(rows[i].personal), rows[i].personal));
    let row_of = |personal: usize, rows: &[CompatRow]| rows.iter().position(|r| r.personal == personal);

    for i in order {
        let me = rows[i].personal;
        let types = types_of(ctx, me);
        let from = if rules.follow_evolutions { parent.get(me).copied().flatten().and_then(|p| row_of(p, rows)) } else { None };
        match from {
            None => {
                for (k, &mv) in moves.iter().enumerate() {
                    let p = probability(types, table.kind(mv), rules.prioritized.contains(&mv), rules.prefer_type);
                    rows[i].flags[k] = rng.gen::<f64>() < p;
                }
            }
            Some(j) => {
                let prev_types = types_of(ctx, rows[j].personal);
                for (k, &mv) in moves.iter().enumerate() {
                    if rows[j].flags[k] {
                        rows[i].flags[k] = true;
                        continue;
                    }
                    // Petite chance d'apprendre une attaque inconnue de la pré-évolution,
                    // forte si elle est d'un type que l'évolution vient d'acquérir.
                    let kind = table.kind(mv);
                    let new_type = |t: Option<PokeType>| t.is_some() && t == kind && t != prev_types.0 && t != prev_types.1;
                    let p = if !rules.prefer_type {
                        0.25
                    } else if new_type(types.0) || new_type(types.1) {
                        0.9
                    } else {
                        0.1
                    };
                    rows[i].flags[k] = rng.gen::<f64>() < p;
                }
            }
        }
    }
}

/// Applique un mode de compatibilité. Renvoie `true` si quelque chose a été tiré ou forcé.
fn apply_mode(
    ctx: &Ctx,
    rows: &mut [CompatRow],
    moves: &[u16],
    table: &MoveTable,
    mode: CompatMode,
    rules: CompatRules,
    rng: &mut ChaCha8Rng,
) -> bool {
    match mode {
        CompatMode::Unchanged => return false,
        CompatMode::Full => rows.iter_mut().for_each(|r| r.flags.iter_mut().for_each(|f| *f = true)),
        CompatMode::Random | CompatMode::RandomPreferType => {
            let rules = CompatRules { prefer_type: mode == CompatMode::RandomPreferType, ..rules };
            randomize_compat(ctx, rows, moves, table, &rules, rng);
        }
    }
    true
}

/// UPR `ensureTMCompatSanity` : une attaque apprise par niveau reste enseignable.
fn levelup_sanity(ctx: &Ctx, rows: &mut [CompatRow], moves: &[u16]) -> usize {
    let mut fixed = 0;
    for row in rows.iter_mut().filter(|r| r.personal >= 1 && r.personal <= ctx.count as usize) {
        let Some(learnset) = ctx.learnsets.get(row.personal) else { continue };
        for &(mv, _) in learnset {
            if let Some(k) = moves.iter().position(|&m| m == mv) {
                if !row.flags[k] {
                    row.flags[k] = true;
                    fixed += 1;
                }
            }
        }
    }
    fixed
}

/// Entrées « personal » qui correspondent à une espèce ou à une forme réelle
/// (pas l'œuf de la Gen 4, ni les fiches vides).
fn species_entries(ctx: &Ctx) -> Vec<usize> {
    let count = ctx.count as usize;
    (1..ctx.personal.len())
        .filter(|&p| !(ctx.gen <= 4 && (p == count + 1 || p == count + 2)))
        .filter(|&p| ctx.personal[p].len() >= 0x2C && ctx.personal[p][..6].iter().any(|&b| b != 0))
        .collect()
}

/// Textes à mettre à jour : (descriptions d'objets, descriptions d'attaques).
fn description_texts(game: Game) -> Option<(usize, usize)> {
    match game {
        Game::Platinum => Some((391, 646)),
        // Diamant / Perle vérifiés sur Diamant ADAF ; HGSS d'après UPR-ZX.
        Game::Diamond | Game::Pearl => Some((343, 587)),
        Game::HeartGold | Game::SoulSilver => Some((221, 749)),
        Game::Black | Game::White => Some((53, 202)),
        Game::Black2 | Game::White2 => Some((63, 402)),
        _ => None,
    }
}

/// Recoupe une description d'attaque en lignes de `width` caractères.
fn rewrap(text: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.join("\n")
}

/// Lit, modifie et réécrit un fichier de texte d'une archive déjà chargée.
fn edit_text(files: &mut [Vec<u8>], gen: u8, index: usize, f: impl FnOnce(&mut Vec<String>)) -> Result<(), RomError> {
    let Some(file) = files.get_mut(index) else { return Ok(()) };
    if gen <= 4 {
        let msg = crate::text::gen4::MsgFile::parse(file)?;
        let mut lines = msg.strings();
        f(&mut lines);
        *file = crate::text::gen4::MsgFile::from_strings(msg.seed, &lines)?.to_bytes();
    } else {
        let mut msg = crate::text::gen5::MsgFile::parse(file)?;
        let mut lines = msg.strings();
        f(&mut lines);
        msg.set_strings(&lines)?;
        *file = msg.to_bytes();
    }
    Ok(())
}

/// Descriptions des CT et dialogues qui les nomment.
fn update_tm_texts(game: &mut GameRom, spec: &MachineSpec, table: &MoveTable, old: &[u16], new: &[u16]) -> Result<(), RomError> {
    let Some((item_desc, move_desc)) = description_texts(game.game) else { return Ok(()) };
    let gen = spec.generation;
    let mut narc = game.narc(game.layout.text_archive)?;
    let descriptions = game.text_file(move_desc)?;
    let (dialogs, width) = tm_texts(game.game);
    edit_text(&mut narc.files, gen, item_desc, |lines| {
        for (i, &mv) in new.iter().enumerate() {
            let (Some(line), Some(desc)) = (lines.get_mut(spec.tm_item(i) as usize), descriptions.get(mv as usize)) else { continue };
            *line = if gen <= 4 { rewrap(desc, width) } else { desc.clone() };
        }
    })?;
    for &(tm, files) in dialogs {
        let (Some(&o), Some(&n)) = (old.get(tm - 1), new.get(tm - 1)) else { continue };
        let (from, to) = (table.name(o).to_string(), table.name(n).to_string());
        if o == n || from.is_empty() {
            continue;
        }
        for &file in files {
            edit_text(&mut narc.files, gen, file, |lines| {
                // Le nom peut aussi être écrit en majuscules (HGSS : « C'EST HURLEMENT ! »).
                let (from_upper, to_upper) = (from.to_uppercase(), to.to_uppercase());
                for line in lines.iter_mut() {
                    *line = line.replace(&from, &to).replace(&from_upper, &to_upper);
                }
            })?;
        }
    }
    game.replace_narc(game.layout.text_archive, &narc)
}

fn label(spec: &MachineSpec, k: usize) -> String {
    if k < spec.tm_count {
        format!("CT{:02}", k + 1)
    } else {
        format!("CS{:02}", k - spec.tm_count + 1)
    }
}

/// CT/CS et donneurs de capacités. Met à jour les fiches « personal » de `ctx`.
pub(crate) fn apply(game: &mut GameRom, ctx: &mut Ctx, settings: &MoveSettings, seed: u64, log: &mut String) -> Result<(), RomError> {
    if !settings.changes_tms() && !settings.changes_tutors() {
        return Ok(());
    }
    let gen = ctx.gen;
    let spec = MachineSpec::for_game(game.game).ok_or_else(|| RomError::Unsupported("CT/CS : génération non prise en charge".into()))?;
    let paths = DataPaths::for_game(game.game).ok_or_else(|| RomError::Unsupported("CT/CS : jeu non pris en charge".into()))?;
    let data_path = machines::move_data_path(game.game).ok_or_else(|| RomError::Unsupported("CT/CS : données d'attaques inconnues".into()))?;
    let names = game.text_file(paths.move_names)?;
    let info: Vec<Option<MoveInfo>> = game.narc(data_path)?.files.iter().map(|d| machines::move_info(gen, d)).collect();
    let max = (names.len().min(info.len())).saturating_sub(1) as u16;
    let table = MoveTable { names, info, max };
    let b2w2 = matches!(game.game, Game::Black2 | Game::White2);
    let (field_moves, early_hms): (&[u16], &[u16]) = if gen <= 4 {
        (&FIELD_MOVES_GEN4, &EARLY_HMS_GEN4)
    } else if b2w2 {
        (&FIELD_MOVES_GEN5, &[]) // Noire 2 / Blanche 2 : aucune CS requise tôt (UPR)
    } else {
        (&FIELD_MOVES_GEN5, &EARLY_HMS_GEN5)
    };

    let mut arm9 = game.rom().arm9_decompressed()?;
    let current = machines::read_from(&arm9, &spec)?;
    let hms = current.hms.clone();
    let mut tms = current.tms.clone();

    // 1. Attaques des CT.
    if settings.random_tms {
        let mut rng = rng_for(seed, "tm_moves");
        let rules = PickRules {
            no_game_breaking: settings.no_game_breaking,
            keep_field_moves: settings.keep_field_moves,
            good_damaging_percent: settings.good_damaging_percent,
            field_moves,
            excluded: &hms,
        };
        tms = pick_moves(&current.tms, &table, &rules, &mut rng);
        machines::write_into(&mut arm9, &spec, &tms)?;
        if let Some(slots) = machines::palette_slots(&arm9, &spec) {
            for (&at, &mv) in slots.iter().zip(&tms) {
                let pal = table.kind(mv).map_or(411, machines::tm_palette);
                arm9[at..at + 2].copy_from_slice(&pal.to_le_bytes());
            }
        }
        game.rom_mut().replace_arm9(&arm9)?;
        update_tm_texts(game, &spec, &table, &current.tms, &tms)?;
        let _ = writeln!(log, "== CT ==");
        for (i, (&o, &n)) in current.tms.iter().zip(&tms).enumerate() {
            let kind = table.kind(n).map_or("?", |t| t.name_fr());
            let _ = writeln!(log, "{} : {} → {} ({kind})", label(&spec, i), table.name(o), table.name(n));
        }
        let _ = writeln!(log);
    }

    // 2. Compatibilité CT/CS.
    if settings.tm_compat != CompatMode::Unchanged || settings.full_hm_compat || settings.levelup_sanity {
        let moves: Vec<u16> = tms.iter().chain(&hms).copied().collect();
        let mut rows: Vec<CompatRow> = species_entries(ctx)
            .into_iter()
            .map(|p| CompatRow { personal: p, flags: (0..spec.total()).map(|k| machines::compatible(&ctx.personal[p], &spec, k)).collect() })
            .collect();
        let mut rng = rng_for(seed, "tm_compat");
        let rules = CompatRules { prefer_type: false, follow_evolutions: settings.follow_evolutions, prioritized: early_hms };
        apply_mode(ctx, &mut rows, &moves, &table, settings.tm_compat, rules, &mut rng);
        let fixed = if settings.levelup_sanity { levelup_sanity(ctx, &mut rows, &tms) } else { 0 };
        if settings.full_hm_compat {
            rows.iter_mut().for_each(|r| r.flags[spec.tm_count..].iter_mut().for_each(|f| *f = true));
        }
        for row in &rows {
            for (k, &v) in row.flags.iter().enumerate() {
                machines::set_compatible(&mut ctx.personal[row.personal], &spec, k, v);
            }
        }
        let mut narc = game.narc(game.layout.personal)?;
        narc.files = ctx.personal.clone();
        game.replace_narc(game.layout.personal, &narc)?;

        let _ = writeln!(log, "== Compatibilité CT/CS ==");
        let total: usize = rows.iter().map(|r| r.flags.iter().filter(|&&f| f).count()).sum();
        let _ = writeln!(log, "Mode : {:?}{}", settings.tm_compat, if settings.full_hm_compat { ", CS pour tous" } else { "" });
        if settings.levelup_sanity {
            let _ = writeln!(log, "{fixed} compatibilité(s) ajoutée(s) pour les attaques apprises par niveau");
        }
        let _ = writeln!(log, "{total} compatibilités pour {} espèces/formes", rows.len());
        for row in rows.iter().filter(|r| r.personal <= ctx.count as usize) {
            let list: Vec<String> = row.flags.iter().enumerate().filter(|(_, &f)| f).map(|(k, _)| label(&spec, k)).collect();
            let _ = writeln!(log, "{:03} {:<12} {}", row.personal, ctx.name(row.personal as u16), list.join(" "));
        }
        let _ = writeln!(log);
    }

    // 3. Donneurs de capacités (Platine).
    if settings.changes_tutors() && game.game == Game::Platinum {
        apply_tutors(game, ctx, settings, &table, &tms, &hms, seed, log)?;
    }
    if settings.changes_tutors() && b2w2 {
        apply_tutors_b2w2(game, ctx, settings, &table, &tms, &hms, seed, log)?;
    }
    Ok(())
}

/// Donneurs de capacités de Noire 2 / Blanche 2 : attaques dans l'overlay 36,
/// compatibilité dans les fiches « personal » (mises à jour dans `ctx`).
#[allow(clippy::too_many_arguments)]
fn apply_tutors_b2w2(
    game: &mut GameRom,
    ctx: &mut Ctx,
    settings: &MoveSettings,
    table: &MoveTable,
    tms: &[u16],
    hms: &[u16],
    seed: u64,
    log: &mut String,
) -> Result<(), RomError> {
    let mut ovl = game.rom().overlay(machines::B2W2_TUTOR_OVERLAY)?;
    let tutor = machines::B2w2Tutors::locate(&ovl, table.max, &game.rom().header().game_code)?;
    let old = tutor.moves(&ovl);
    let mut moves = old.clone();

    if settings.random_tutors {
        let mut rng = rng_for(seed, "tutor_moves");
        let excluded: Vec<u16> = tms.iter().chain(hms).copied().collect();
        let rules = PickRules {
            no_game_breaking: settings.no_game_breaking,
            keep_field_moves: settings.keep_field_moves,
            good_damaging_percent: settings.good_damaging_percent,
            field_moves: &FIELD_MOVES_GEN5,
            excluded: &excluded,
        };
        moves = pick_moves(&old, table, &rules, &mut rng);
        tutor.set_moves(&mut ovl, &moves);
        // Recompressé : l'overlay 36 (terrain) ne tiendrait plus non compressé dans une ROM DSi.
        game.rom_mut().replace_overlay_recompressed(machines::B2W2_TUTOR_OVERLAY, ovl)?;
        let _ = writeln!(log, "== Donneurs de capacités ==");
        for (o, n) in old.iter().zip(&moves) {
            let _ = writeln!(log, "{} → {}", table.name(*o), table.name(*n));
        }
        let _ = writeln!(log);
    }

    if settings.tutor_compat != CompatMode::Unchanged || settings.levelup_sanity {
        let mut rows: Vec<CompatRow> = species_entries(ctx)
            .into_iter()
            .map(|p| CompatRow { personal: p, flags: (0..moves.len()).map(|k| machines::b2w2_tutor_compatible(&ctx.personal[p], k)).collect() })
            .collect();
        let mut rng = rng_for(seed, "tutor_compat");
        let rules = CompatRules { prefer_type: false, follow_evolutions: settings.follow_evolutions, prioritized: &[] };
        apply_mode(ctx, &mut rows, &moves, table, settings.tutor_compat, rules, &mut rng);
        let fixed = if settings.levelup_sanity { levelup_sanity(ctx, &mut rows, &moves) } else { 0 };
        for row in &rows {
            for (k, &v) in row.flags.iter().enumerate() {
                machines::set_b2w2_tutor_compatible(&mut ctx.personal[row.personal], k, v);
            }
        }
        let mut narc = game.narc(game.layout.personal)?;
        narc.files = ctx.personal.clone();
        game.replace_narc(game.layout.personal, &narc)?;
        let total: usize = rows.iter().map(|r| r.flags.iter().filter(|&&f| f).count()).sum();
        let _ = writeln!(log, "== Compatibilité des donneurs de capacités ==");
        let _ =
            writeln!(log, "Mode : {:?} ; {total} compatibilités ; {fixed} ajoutée(s) pour les attaques apprises par niveau\n", settings.tutor_compat);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_tutors(
    game: &mut GameRom,
    ctx: &Ctx,
    settings: &MoveSettings,
    table: &MoveTable,
    tms: &[u16],
    hms: &[u16],
    seed: u64,
    log: &mut String,
) -> Result<(), RomError> {
    let count = ctx.count as usize;
    // Espèces puis formes (fiches 496+ en Platine, entrées 493+ de la table).
    let forms = ctx.personal.len().saturating_sub(count + 3);
    let mut ovl = game.rom().overlay(machines::PT_TUTOR_OVERLAY)?;
    let tutor = TutorTable::locate(&ovl, table.max, count + forms)?;
    let old = tutor.moves(&ovl);
    let mut moves = old.clone();

    if settings.random_tutors {
        let mut rng = rng_for(seed, "tutor_moves");
        let excluded: Vec<u16> = tms.iter().chain(hms).copied().collect();
        let rules = PickRules {
            no_game_breaking: settings.no_game_breaking,
            keep_field_moves: settings.keep_field_moves,
            good_damaging_percent: settings.good_damaging_percent,
            field_moves: &FIELD_MOVES_GEN4,
            excluded: &excluded,
        };
        moves = pick_moves(&old, table, &rules, &mut rng);
        tutor.set_moves(&mut ovl, &moves);
        let _ = writeln!(log, "== Donneurs de capacités ==");
        for (o, n) in old.iter().zip(&moves) {
            let _ = writeln!(log, "{} → {}", table.name(*o), table.name(*n));
        }
        let _ = writeln!(log);
    }

    if settings.tutor_compat != CompatMode::Unchanged || settings.levelup_sanity {
        let entry = |p: usize| if p <= count { p - 1 } else { p - 3 };
        let mut rows: Vec<CompatRow> = species_entries(ctx)
            .into_iter()
            .filter(|&p| entry(p) < count + forms)
            .map(|p| CompatRow { personal: p, flags: (0..moves.len()).map(|k| tutor.compatible(&ovl, entry(p), k)).collect() })
            .collect();
        let mut rng = rng_for(seed, "tutor_compat");
        let rules = CompatRules { prefer_type: false, follow_evolutions: settings.follow_evolutions, prioritized: &[] };
        apply_mode(ctx, &mut rows, &moves, table, settings.tutor_compat, rules, &mut rng);
        let fixed = if settings.levelup_sanity { levelup_sanity(ctx, &mut rows, &moves) } else { 0 };
        for row in &rows {
            for (k, &v) in row.flags.iter().enumerate() {
                tutor.set_compatible(&mut ovl, entry(row.personal), k, v);
            }
        }
        let total: usize = rows.iter().map(|r| r.flags.iter().filter(|&&f| f).count()).sum();
        let _ = writeln!(log, "== Compatibilité des donneurs de capacités ==");
        let _ =
            writeln!(log, "Mode : {:?} ; {total} compatibilités ; {fixed} ajoutée(s) pour les attaques apprises par niveau\n", settings.tutor_compat);
    }
    game.rom_mut().replace_overlay(machines::PT_TUTOR_OVERLAY, ovl)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn table() -> MoveTable {
        // Attaques 1..=400 : les multiples de 4 sont puissantes et précises.
        let info =
            (0..=400u16).map(|m| Some(MoveInfo { kind: Some(PokeType::Normal), power: if m % 4 == 0 { 90 } else { 0 }, accuracy: 100 })).collect();
        MoveTable { names: (0..=400).map(|m| format!("m{m}")).collect(), info, max: 400 }
    }

    #[test]
    fn picks_respect_bans_and_field_moves() {
        let t = table();
        let old: Vec<u16> = vec![15, 148, 91, 1, 2, 3, 4, 5, 6, 7];
        let hms = [57, 70];
        for seed in 0..50 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let rules = PickRules {
                no_game_breaking: true,
                keep_field_moves: true,
                good_damaging_percent: 0,
                field_moves: &FIELD_MOVES_GEN4,
                excluded: &hms,
            };
            let new = pick_moves(&old, &t, &rules, &mut rng);
            assert_eq!(new.len(), old.len());
            assert_eq!(&new[..3], &[15, 148, 91], "capacités de terrain gardées à leur place");
            for (i, m) in new.iter().enumerate() {
                assert!(!new[..i].contains(m), "attaques distinctes");
                assert!(!hms.contains(m) && *m != STRUGGLE && !GAME_BREAKING.contains(m));
                assert!((1..=400).contains(m));
            }
        }
    }

    #[test]
    fn good_damaging_share() {
        let t = table();
        let old: Vec<u16> = (1..=20).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let rules = PickRules { no_game_breaking: false, keep_field_moves: false, good_damaging_percent: 100, field_moves: &[], excluded: &[] };
        let new = pick_moves(&old, &t, &rules, &mut rng);
        assert!(new.iter().all(|&m| t.good_damaging(m)));
        assert!(!t.good_damaging(120), "Destruction n'est jamais une bonne attaque");
    }

    #[test]
    fn compat_probabilities() {
        let fire = (Some(PokeType::Fire), None);
        assert_eq!(probability(fire, Some(PokeType::Fire), false, true), 0.9);
        assert_eq!(probability(fire, Some(PokeType::Normal), false, true), 0.5);
        assert_eq!(probability(fire, Some(PokeType::Water), false, true), 0.25);
        assert_eq!(probability(fire, Some(PokeType::Water), false, false), 0.5);
        assert!((probability(fire, Some(PokeType::Water), true, false) - 0.9).abs() < 1e-9);
        assert_eq!(probability(fire, Some(PokeType::Fire), true, true), 1.0);
    }

    #[test]
    fn rewrap_lines() {
        let s = rewrap("The user focuses its\nmind before launching\na punch.", 20);
        assert_eq!(s, "The user focuses its\nmind before\nlaunching a punch.");
        assert!(s.lines().all(|l| l.chars().count() <= 20));
    }

    #[test]
    fn settings_default_is_noop() {
        let s = MoveSettings::default();
        assert!(!s.changes_tms() && !s.changes_tutors());
        let json = serde_json::to_string(&MoveSettings { tm_compat: CompatMode::RandomPreferType, ..s }).unwrap();
        assert!(json.contains("\"tmCompat\":\"random_prefer_type\""));
        let back: MoveSettings = serde_json::from_str("{\"randomTms\":true}").unwrap();
        assert!(back.random_tms && back.tm_compat == CompatMode::Unchanged);
    }
}
