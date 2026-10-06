//! Randomizer 3DS, Gen 7 : Soleil / Lune et Ultra-Soleil / Ultra-Lune. Mêmes réglages et
//! même sortie (LayeredFS et/ou ROM `.3ds` complète) que Rubis Oméga / Saphir Alpha
//! (voir [`super::ctr`]), mais les formats diffèrent ; tout vient du Gen7RomHandler de
//! l'Universal Pokémon Randomizer (UPR-ZX) et a été vérifié sur Lune et Ultra-Soleil (Europe) :
//!
//! - **Fiches** (`a/0/1/7`) : 0x54 octets, mêmes champs que la Gen 6 pour ce qui est modifié
//!   (statistiques, types, taux de capture, talents en 0x18-0x1A) ; la dernière entrée
//!   contient la table complète.
//! - **Évolutions** (`a/0/1/4`) : 8 × 8 octets (`u16` méthode, `u16` paramètre, `u16` espèce,
//!   `i8` forme, `u8` niveau). Le niveau requis est dans le dernier octet, pas dans le paramètre.
//! - **Starters** : table des dons, entrée 0 de `a/1/5/5` (Soleil/Lune) ou `a/1/5/9` (Ultra),
//!   0x14 octets par don (`u16` espèce, `u8` forme, `u8` niveau) ; les trois premiers sont
//!   Brindibou, Flamiaou et Otaquin. Rien à modifier dans les `.cro`, contrairement à ROSA.
//! - **Rencontres** : archive `a/0/8/2` (Soleil, Ultra-Soleil) ou `a/0/8/3` (Lune, Ultra-Lune),
//!   11 entrées par zone ; l'entrée `9 + 11 × zone` (LZ11) contient les tables (voir
//!   [`crate::data::encounters::alola_tables`]).
//! - **Dresseurs** : `trdata` de 0x14 octets (`u8` classe @0, `u8` type de combat @2, `u8`
//!   nombre de Pokémon @3, `u8` IA @0xC) ; `trpoke` de 0x20 octets par Pokémon : `u8`
//!   talent << 4 | sexe @0, `u8` nature @1, 6 EV @2, `u32` IV @8 (5 bits par statistique),
//!   `u16` niveau @0xE, `u16` espèce @0x10, `u16` forme @0x12, `u16` objet @0x14, 4 × `u16`
//!   attaques @0x18 (toutes nulles : attaques par défaut du niveau).

use std::fmt::Write as _;
use std::path::Path;

use kaleido_formats::garc::Garc;
use kaleido_formats::lz;
use rand::seq::SliceRandom;

use super::ctr::{entries, image_output_path, shiny_patch, write_outputs, CtrWritten, LayeredFsTarget};
use super::{apply_personal, randomize_trainers, randomize_wild, rng_for, share_code, Ctx, Outcome, PokemonRef, Settings, Sources, WildMode};
use crate::ctr_rom::CtrGameRom;
use crate::data::evolutions::{self, METHOD_LEVEL, TRADE_REPLACEMENT_LEVEL};
use crate::data::{learnsets, put_u16, trainers, u16_at};
use crate::games::Game;
use crate::rom::RomError;

/// Brindibou, Flamiaou, Otaquin.
const ORIGINAL_STARTERS: [u16; 3] = [722, 725, 728];
/// Types d'origine des starters, tels qu'écrits dans les textes.
const ORIGINAL_TYPES: [&str; 3] = ["Plante", "Feu", "Eau"];
const GIFT_SIZE: usize = 0x14;

/// Évolutions : 8 entrées de 8 octets.
const EVO_ENTRY: usize = 8;
const EVO_ENTRIES: usize = 8;
const METHOD_TRADE: u16 = 5;
const METHOD_TRADE_ITEM: u16 = 6;
const METHOD_TRADE_SPECIES: u16 = 7;
const METHOD_USE_ITEM: u16 = 8;

/// Rencontres : 11 entrées par zone, les tables sont dans la 10ᵉ.
const AREA_FIRST: usize = 9;
const AREA_STRIDE: usize = 11;

/// Taille d'un Pokémon de dresseur.
const TRPOKE_SIZE: usize = 0x20;
/// Fiche de dresseur au format ROSA (drapeaux @0, nombre @7) utilisée pour passer
/// les équipes Gen 7 au randomizer commun.
const ORAS_TRDATA_SIZE: usize = 0x18;

/// Lutte : jamais attribuée.
const STRUGGLE: u16 = 165;

pub(super) fn supports(game: Game) -> bool {
    matches!(game, Game::Sun | Game::Moon | Game::UltraSun | Game::UltraMoon)
}

/// Emplacements propres à chaque paire de versions (gen7_offsets.ini de l'UPR, vérifiés).
struct Gen7Info {
    /// Plus grand identifiant de talent (Prisme-Armure en Soleil/Lune, Cérébro-Force en Ultra).
    max_ability: u16,
    /// Archive des Pokémon offerts (entrée 0 : les dons, starters en tête).
    gifts: &'static str,
    /// Fichier des textes de l'histoire de l'écran de choix des starters.
    starter_text: usize,
    ultra: bool,
}

/// Plus grand identifiant de talent (Soleil/Lune ou Ultra).
pub(crate) fn max_ability(game: Game) -> u16 {
    info(game).max_ability
}

fn info(game: Game) -> Gen7Info {
    if matches!(game, Game::UltraSun | Game::UltraMoon) {
        Gen7Info { max_ability: 233, gifts: "a/1/5/9", starter_text: 39, ultra: true }
    } else {
        Gen7Info { max_ability: 232, gifts: "a/1/5/5", starter_text: 41, ultra: false }
    }
}

/// Capacités Z (UPR, GlobalConstants.zMoves) : jamais apprises par niveau.
pub(crate) fn is_z_move(m: u16) -> bool {
    matches!(m, 622..=658 | 695..=703 | 719 | 723..=728)
}

/// Convertit les évolutions Gen 7 au format Gen 6 (`u16` méthode, `u16` paramètre, `u16`
/// espèce) lu par [`evolutions::read`] ; le niveau requis devient le paramètre.
fn evolutions_as_gen6(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(EVO_ENTRIES * 6);
    for e in d.as_chunks::<EVO_ENTRY>().0.iter().take(EVO_ENTRIES) {
        let (method, param, target) = (u16_at(e, 0), u16_at(e, 2), u16_at(e, 4));
        let param = if param == 0 { e[7] as u16 } else { param };
        for v in [method, param, target] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Évolutions par échange → niveau 37 (échange simple ou contre une espèce) ou utilisation
/// de l'objet (échange avec objet), comme [`evolutions::remove_trade_evolutions`].
fn remove_trade_evolutions(d: &mut [u8]) -> usize {
    let mut changed = 0;
    for e in d.as_chunks_mut::<EVO_ENTRY>().0.iter_mut().take(EVO_ENTRIES) {
        if u16_at(e, 4) == 0 {
            continue;
        }
        let (method, param, level) = match u16_at(e, 0) {
            METHOD_TRADE | METHOD_TRADE_SPECIES => (METHOD_LEVEL, 0, TRADE_REPLACEMENT_LEVEL as u8),
            METHOD_TRADE_ITEM => (METHOD_USE_ITEM, u16_at(e, 2), 0),
            _ => continue,
        };
        put_u16(e, 0, method);
        put_u16(e, 2, param);
        e[7] = level;
        changed += 1;
    }
    changed
}

/// Équipe Gen 7 → fiche et équipe au format ROSA (lisibles par [`trainers::read_team`]) :
/// objet toujours présent, attaques seulement si le jeu en donne. `None` si illisible.
fn team_to_oras(trdata: &[u8], trpoke: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let count = *trdata.get(3)? as usize;
    let team = trpoke.get(..count * TRPOKE_SIZE)?;
    let with_moves = team.as_chunks::<TRPOKE_SIZE>().0.iter().any(|p| (0..4).any(|m| u16_at(p, 0x18 + m * 2) != 0));
    let flags = trainers::FLAG_ITEM | if with_moves { trainers::FLAG_MOVES } else { 0 };
    let mut oras_trdata = vec![0u8; ORAS_TRDATA_SIZE];
    oras_trdata[0] = flags;
    oras_trdata[7] = count as u8;
    let mut oras = Vec::new();
    for p in team.as_chunks::<TRPOKE_SIZE>().0 {
        oras.push(0); // difficulté : 255 demande des IV au maximum
        oras.push(p[0]);
        oras.extend_from_slice(&p[0x0E..0x16]); // niveau, espèce, forme, objet
        if with_moves {
            oras.extend_from_slice(&p[0x18..0x20]);
        }
    }
    Some((oras_trdata, oras))
}

/// Réécrit l'équipe Gen 7 d'origine avec l'équipe modifiée au format ROSA : nature,
/// EV et octets inconnus sont conservés.
fn team_from_oras(trpoke: &[u8], oras_trdata: &[u8], oras: &[u8]) -> Option<Vec<u8>> {
    let team = trainers::read_team(7, oras_trdata, oras)?;
    let mut out = trpoke.to_vec();
    let used = out.get_mut(..team.pokemon.len() * TRPOKE_SIZE)?;
    for (p, e) in team.pokemon.iter().zip(used.as_chunks_mut::<TRPOKE_SIZE>().0) {
        e[0] = p.gender_ability;
        put_u16(e, 0x0E, p.level);
        put_u16(e, 0x10, p.species);
        put_u16(e, 0x12, p.form);
        put_u16(e, 0x14, p.item);
        if team.flags & trainers::FLAG_MOVES != 0 {
            for (m, &mv) in p.moves.iter().enumerate() {
                put_u16(e, 0x18 + m * 2, mv);
            }
        }
        if p.difficulty == 255 {
            let ivs = u32::from_le_bytes(e[8..12].try_into().unwrap());
            e[8..12].copy_from_slice(&((ivs & 0xC000_0000) | 0x3FFF_FFFF).to_le_bytes());
        }
    }
    Some(out)
}

fn load(game: &CtrGameRom, settings: &Settings) -> Result<(Ctx, Garc), RomError> {
    let l = game.layout;
    let personal = game.garc(l.personal)?;
    let evolutions: Vec<Vec<u8>> = entries(&game.garc(l.evolution)?).iter().map(|d| evolutions_as_gen6(d)).collect();
    let learnsets = entries(&game.garc(l.levelup)?);
    let src = Sources {
        gen: 7,
        count: l.species_count,
        names: game.text_file(l.species_names)?,
        personal: entries(&personal),
        evolutions: &evolutions,
        learnsets: &learnsets,
        max_ability: info(game.game).max_ability,
    };
    Ok((Ctx::new(src, settings)?, personal))
}

/// Aperçu des starters, sans rien écrire.
pub(super) fn preview_starters(game: &CtrGameRom, settings: &Settings, seed: u64) -> Result<Vec<PokemonRef>, RomError> {
    let (mut ctx, _) = load(game, settings)?;
    apply_personal(&mut ctx, settings, seed, &mut String::new());
    let chosen = super::choose_starters(&ctx, settings, seed, ORIGINAL_STARTERS);
    Ok(chosen.iter().map(|&id| PokemonRef { id, name: ctx.name(id).to_string() }).collect())
}

/// Remplace le type d'origine d'un starter dans une ligne (« de type Plante » ou « Pokémon Plante »).
fn retype(line: &str, old: &str, new: &str) -> String {
    let long = format!("de type {old}");
    if line.contains(&long) {
        line.replacen(&long, &format!("de type {new}"), 1)
    } else {
        line.replacen(&format!("Pokémon {old}"), &format!("Pokémon {new}"), 1)
    }
}

/// Remplace le nom d'origine en début de ligne.
fn rename(line: &str, old: &str, new: &str) -> String {
    line.strip_prefix(old).map_or_else(|| line.to_string(), |rest| format!("{new}{rest}"))
}

/// Écrit les starters dans la table des dons et adapte les textes de l'écran de choix.
/// Comme l'UPR (setStarterText), les noms sont écrits en clair : les variables du script
/// désignent encore les starters d'origine.
fn write_starters(game: &CtrGameRom, ctx: &Ctx, starters: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
    let gi = info(game.game);
    let bad = || RomError::Layout("table des starters inattendue (révision du jeu différente ?)".into());
    let mut garc = game.garc(gi.gifts)?;
    let mut gifts = garc.file(0).ok_or_else(bad)?.to_vec();
    for (i, &s) in starters.iter().enumerate() {
        let at = i * GIFT_SIZE;
        if gifts.len() < at + GIFT_SIZE || u16_at(&gifts, at) != ORIGINAL_STARTERS[i] {
            return Err(bad());
        }
        put_u16(&mut gifts, at, s);
        gifts[at + 2] = 0; // forme
    }
    garc.set_file(0, gifts)?;
    files.push((gi.gifts.to_string(), garc.to_bytes()));

    let l = game.layout;
    let mut text = game.garc(l.story_text)?;
    if let Some(data) = text.file(gi.starter_text).map(<[u8]>::to_vec) {
        use crate::text::gen5::{MsgFile, Variant};
        let mut msg = MsgFile::parse_with(&data, Variant::Gen6)?;
        let mut lines = msg.strings();
        for (i, &s) in starters.iter().enumerate() {
            let (old_name, name) = (ctx.name(ORIGINAL_STARTERS[i]).to_string(), ctx.name(s).to_string());
            let new_type = ctx.types(s).first().map_or("Normal", |t| t.name_fr());
            let old_type = ORIGINAL_TYPES[i];
            let var = format!("{{VAR:0101,000{}}}", i + 1);
            let mut edit = |n: usize, f: &dyn Fn(&str) -> String| {
                if let Some(line) = lines.get_mut(n) {
                    *line = f(line);
                }
            };
            if gi.ultra {
                // « Brindibou... Une menace silencieuse… », « Tu prends {VAR}, le Pokémon Plante ? », noms seuls.
                edit(1 + i, &|t| rename(t, &old_name, &name));
                edit(7 + i, &|t| retype(&t.replace(&var, &name), old_type, new_type));
                edit(14 + i, &|_| name.clone());
            } else {
                // « Brindibou (Plante) », « {VAR}, le Pokémon de type Plante... », « Brindibou regarde… »,
                // « {VAR}, le Pokémon de type Plante, est un peu chatouilleux ! ».
                edit(1 + i, &|_| format!("{name} ({new_type})"));
                edit(4 + i, &|t| retype(&t.replace(&var, &name), old_type, new_type));
                edit(11 + i, &|t| rename(t, &old_name, &name));
                edit(35 + i, &|t| retype(t, old_type, new_type));
            }
        }
        msg.set_strings(&lines)?;
        text.set_file(gi.starter_text, msg.to_bytes())?;
        files.push((l.story_text.to_string(), text.to_bytes()));
    }
    Ok(())
}

/// Attaques apprises aléatoires, comme [`super::extras::random_movesets`] mais sans capacités Z.
fn random_movesets(ctx: &mut Ctx, files: &mut [Vec<u8>], max_move: u16, seed: u64, log: &mut String) -> usize {
    let mut rng = rng_for(seed, "movesets");
    let pool: Vec<u16> = (1..=max_move).filter(|&m| m != STRUGGLE && !is_z_move(m)).collect();
    let mut total = 0;
    for s in 1..=(ctx.count as usize).min(files.len().saturating_sub(1)) {
        learnsets::map_moves(7, &mut files[s], |i, mv, _| {
            if i == 0 {
                mv
            } else {
                total += 1;
                *pool.choose(&mut rng).unwrap_or(&mv)
            }
        });
        if let Some(ls) = ctx.learnsets.get_mut(s) {
            *ls = learnsets::read(7, &files[s]);
        }
    }
    let _ = writeln!(log, "== Attaques apprises ==\n{total} attaques remplacées au hasard.\n");
    total
}

/// Évolutions sans échange (voir [`super::extras::easy_evolutions`]).
fn easy_evolutions(ctx: &mut Ctx, files: &mut [Vec<u8>], log: &mut String) -> usize {
    let _ = writeln!(log, "== Évolutions sans échange ==");
    let mut total = 0;
    for s in 1..=(ctx.count as usize).min(files.len().saturating_sub(1)) {
        if remove_trade_evolutions(&mut files[s]) > 0 {
            total += 1;
            let evos = evolutions::read(&evolutions_as_gen6(&files[s]));
            let targets: Vec<&str> = evos.iter().map(|e| ctx.name(e.target)).collect();
            let _ = writeln!(log, "{} → {}", ctx.name(s as u16), targets.join(" / "));
            if let Some(table) = ctx.evo_tables.get_mut(s) {
                *table = evos;
            }
        }
    }
    let _ = writeln!(log);
    total
}

/// Randomise et écrit sous `out_dir` le dossier LayeredFS et/ou la ROM reconstruite.
pub(super) fn randomize(
    game: &CtrGameRom,
    settings: &Settings,
    seed: u64,
    out_dir: &Path,
    target: LayeredFsTarget,
) -> Result<(Outcome, CtrWritten), RomError> {
    let output = settings.ctr_output;
    let image_out = if output.image() {
        let input = game
            .romfs()
            .image_path()
            .ok_or_else(|| RomError::Unsupported("la sortie .3ds demande une ROM (.3ds, .cxi ou .cia déchiffré), pas un dossier extrait".into()))?;
        Some(image_output_path(input, out_dir, seed)?)
    } else {
        None
    };
    let l = game.layout;
    let (mut ctx, mut personal) = load(game, settings)?;
    let code = share_code(seed, settings);
    let mut log = String::new();
    let _ = writeln!(log, "Kaleido — journal de randomisation (3DS)");
    let _ = writeln!(log, "Jeu : {} ({:016X})", game.game.name_fr(), game.title_id());
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}\n");

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    // 1. Fiches des espèces : chaque entrée, plus la table complète en dernière position.
    if apply_personal(&mut ctx, settings, seed, &mut log) {
        let size = ctx.personal.get(1).map_or(0, Vec::len);
        let last = personal.len() - 1;
        let mut table = personal.file(last).map(<[u8]>::to_vec).unwrap_or_default();
        for s in 1..=l.species_count as usize {
            personal.set_file(s, ctx.personal[s].clone())?;
            if size > 0 && (s + 1) * size <= table.len() {
                table[s * size..(s + 1) * size].copy_from_slice(&ctx.personal[s]);
            }
        }
        personal.set_file(last, table)?;
        files.push((l.personal.to_string(), personal.to_bytes()));
    }

    // 2. Starters.
    let starters = super::choose_starters(&ctx, settings, seed, ORIGINAL_STARTERS);
    if starters != ORIGINAL_STARTERS {
        write_starters(game, &ctx, starters, &mut files)?;
        let _ = writeln!(log, "== Starters ==");
        for (old, new) in ORIGINAL_STARTERS.iter().zip(starters) {
            let _ = writeln!(log, "{} → {}", ctx.name(*old), ctx.name(new));
        }
        let _ = writeln!(log);
    }

    // Évolutions et attaques apprises (avant les dresseurs, qui s'en servent).
    if settings.easy_evolutions {
        let mut garc = game.garc(l.evolution)?;
        let mut evo_files = entries(&garc);
        easy_evolutions(&mut ctx, &mut evo_files, &mut log);
        for (i, f) in evo_files.into_iter().enumerate() {
            garc.set_file(i, f)?;
        }
        files.push((l.evolution.to_string(), garc.to_bytes()));
    }
    if settings.random_movesets {
        let max_move = game.text_file(l.move_names)?.len().saturating_sub(1) as u16;
        let mut garc = game.garc(l.levelup)?;
        let mut learn_files = entries(&garc);
        random_movesets(&mut ctx, &mut learn_files, max_move, seed, &mut log);
        for (i, f) in learn_files.into_iter().enumerate() {
            garc.set_file(i, f)?;
        }
        files.push((l.levelup.to_string(), garc.to_bytes()));
    }

    // 3. Pokémon sauvages : tables `EA` (LZ11) de chaque zone, jour et nuit, SOS compris.
    let mut wild_slots = 0;
    if settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100 {
        let mut garc = game.garc(l.encounters)?;
        let area_ids: Vec<usize> = (AREA_FIRST..garc.len()).step_by(AREA_STRIDE).collect();
        let mut areas: Vec<Vec<u8>> =
            area_ids.iter().map(|&i| garc.file(i).filter(|d| lz::is_lz11(d)).and_then(|d| lz::decompress(d).ok()).unwrap_or_default()).collect();
        let before = areas.clone();
        let _ = writeln!(log, "(Zone n : entrée {AREA_FIRST} + {AREA_STRIDE} × n de {})", l.encounters);
        wild_slots = randomize_wild(&ctx, settings, seed, &mut areas, &mut log);
        for (k, &i) in area_ids.iter().enumerate() {
            if areas[k] != before[k] {
                garc.set_file(i, lz::compress_lz11(&areas[k]))?;
            }
        }
        files.push((l.encounters.to_string(), garc.to_bytes()));
    }

    // 4. Dresseurs : passés au format ROSA pour le randomizer commun, puis réécrits.
    let mut trainer_pokemon = 0;
    if super::trainers_changed(settings) {
        // Le nombre de Pokémon ne change pas : `trdata` n'est que lu.
        let trdata = entries(&game.garc(l.trainer_data)?);
        let mut trpoke_garc = game.garc(l.trainer_pokemon)?;
        let trpoke = entries(&trpoke_garc);
        let converted: Vec<Option<(Vec<u8>, Vec<u8>)>> = trdata.iter().zip(&trpoke).map(|(d, p)| team_to_oras(d, p)).collect();
        let mut oras_data: Vec<Vec<u8>> =
            converted.iter().map(|c| c.as_ref().map_or_else(|| vec![0; ORAS_TRDATA_SIZE], |(d, _)| d.clone())).collect();
        let mut oras_poke: Vec<Vec<u8>> = converted.iter().map(|c| c.as_ref().map_or_else(Vec::new, |(_, p)| p.clone())).collect();
        let moves = game.text_file(l.move_names)?;
        trainer_pokemon = randomize_trainers(&ctx, settings, seed, &mut oras_data, &mut oras_poke, &moves, &mut log);
        for i in 0..converted.len() {
            if converted[i].is_none() {
                continue;
            }
            let team = team_from_oras(&trpoke[i], &oras_data[i], &oras_poke[i])
                .ok_or_else(|| RomError::Layout(format!("dresseur n°{i} : équipe illisible")))?;
            if team != trpoke[i] {
                trpoke_garc.set_file(i, team)?;
            }
        }
        files.push((l.trainer_pokemon.to_string(), trpoke_garc.to_bytes()));
    }

    // 5. Taux de chromatiques (programme du jeu, code.bin).
    let code_patch = shiny_patch(game, settings.shiny_odds, &mut log)?;
    let written = write_outputs(game, out_dir, target, output, image_out, &files, code_patch.as_ref(), &mut log)?;
    let starters = starters.iter().map(|&id| PokemonRef { id, name: ctx.name(id).to_string() }).collect();
    let outcome = Outcome { seed, share_code: code, starters, wild_slots, trainer_pokemon, log };
    Ok((outcome, written))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::evolutions::Evolution;
    use crate::data::{encounters, trainers::read_team};
    use crate::randomizer::{CatchRateMode, StarterMode, StatsMode, TrainerMode};
    use crate::text::gen5::{MsgFile, Variant};

    #[test]
    fn evolutions_gen7() {
        // Entrées réelles de Lune : Bulbizarre (niveau 16 dans le dernier octet), Kadabra
        // (échange), Onix (échange avec Peau Métal, objet 233).
        let mut d = vec![0u8; 64];
        d[..8].copy_from_slice(&[4, 0, 0, 0, 2, 0, 0xFF, 16]);
        assert_eq!(evolutions::read(&evolutions_as_gen6(&d)), vec![Evolution { method: 4, param: 16, target: 2 }]);

        let mut kadabra = vec![0u8; 64];
        kadabra[..8].copy_from_slice(&[5, 0, 0, 0, 65, 0, 0xFF, 0]);
        kadabra[8..16].copy_from_slice(&[6, 0, 233, 0, 208, 0, 0xFF, 0]);
        assert_eq!(remove_trade_evolutions(&mut kadabra), 2);
        assert_eq!(&kadabra[..8], &[4, 0, 0, 0, 65, 0, 0xFF, 37]);
        assert_eq!(&kadabra[8..16], &[8, 0, 233, 0, 208, 0, 0xFF, 0]);
        let evos = evolutions::read(&evolutions_as_gen6(&kadabra));
        assert_eq!(evos[0], Evolution { method: 4, param: 37, target: 65 });
    }

    #[test]
    fn trainer_roundtrip() {
        // Deux Pokémon (d'après le dresseur n°149 de Lune) : le premier avec attaques.
        let trdata = [0x6B, 0, 0, 2, 0x17, 0, 0x17, 0, 0x1B, 0, 0, 0, 0x87, 1, 0, 0, 0, 0x32, 0, 0];
        let mut trpoke = vec![0u8; 0x40];
        trpoke[..0x20].copy_from_slice(&[
            0x12, 0x0C, 0x00, 0xFC, 0x00, 0x00, 0x00, 0xFC, 0xFF, 0xBF, 0xF7, 0x3E, 0x00, 0x00, 0x36, 0x00, 0x2E, 0x01, 0, 0, 0, 0, 0, 0, 0xA5, 0x01,
            0xAC, 0x01, 0x6D, 0x00, 0xFC, 0x00,
        ]);
        trpoke[0x20..0x30].copy_from_slice(&[0x10, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x37, 0]);
        trpoke[0x30..0x32].copy_from_slice(&770u16.to_le_bytes());
        trpoke[0x34..0x36].copy_from_slice(&789u16.to_le_bytes());
        let (od, op) = team_to_oras(&trdata, &trpoke).unwrap();
        let mut team = read_team(7, &od, &op).unwrap();
        assert_eq!(team.pokemon.len(), 2);
        assert_eq!((team.pokemon[0].species, team.pokemon[0].level, team.pokemon[0].moves), (302, 54, [421, 428, 109, 252]));
        assert_eq!((team.pokemon[1].species, team.pokemon[1].form, team.pokemon[1].item), (770, 0, 789));
        // Sans modification, l'équipe d'origine est réécrite à l'identique.
        assert_eq!(team_from_oras(&trpoke, &od, &op).unwrap(), trpoke);

        team.pokemon[1].species = 25;
        team.pokemon[1].moves = [84, 0, 0, 0];
        team.pokemon[0].difficulty = 255;
        let mut od2 = od.clone();
        let op2 = trainers::write_team(7, &team, &mut od2);
        let out = team_from_oras(&trpoke, &od2, &op2).unwrap();
        assert_eq!(u16_at(&out, 0x30), 25);
        assert_eq!(u16_at(&out, 0x38), 84);
        assert_eq!(&out[8..12], &[0xFF, 0xFF, 0xFF, 0x3F]); // IV au maximum
        assert_eq!(&out[..8], &trpoke[..8]); // talent, nature et EV conservés
    }

    #[test]
    fn trainer_without_moves() {
        // Premier dresseur de Lune : Goélise niv. 6, attaques par défaut (toutes nulles).
        let trdata = [3, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 6, 0, 0];
        let mut trpoke = vec![0u8; 0x20];
        trpoke[1] = 0x0C;
        trpoke[0x0E] = 6;
        trpoke[0x10..0x12].copy_from_slice(&278u16.to_le_bytes());
        let (od, op) = team_to_oras(&trdata, &trpoke).unwrap();
        let team = read_team(7, &od, &op).unwrap();
        assert_eq!(team.flags & trainers::FLAG_MOVES, 0);
        assert_eq!((team.pokemon[0].species, team.pokemon[0].level), (278, 6));
        assert!(team_to_oras(&trdata, &trpoke[..0x10]).is_none());
    }

    #[test]
    fn z_moves() {
        assert!(is_z_move(622) && is_z_move(658) && is_z_move(719) && is_z_move(728));
        assert!(!is_z_move(621) && !is_z_move(659) && !is_z_move(720) && !is_z_move(722));
    }

    fn rom(name: &str) -> Option<CtrGameRom> {
        let p = crate::test_rom_path(name);
        p.exists().then(|| CtrGameRom::open(&p).unwrap())
    }

    const MOON: &str = "Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds";
    const ULTRA_SUN: &str = "Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds";

    /// Formats relus sur les vraies ROM : noms, fiches, starters, Route 1, dresseurs.
    fn check_formats(game: &CtrGameRom, first_trainer: u16) {
        let l = game.layout;
        let names = game.text_file(l.species_names).unwrap();
        assert_eq!((names[722].as_str(), names[725].as_str(), names[728].as_str()), ("Brindibou", "Flamiaou", "Otaquin"));
        assert_eq!(names.len() as u16, l.species_count + 1);
        assert_eq!(game.text_file(l.trainer_names).unwrap()[1], "Dorothée");
        assert_eq!(game.text_file(l.trainer_classes).unwrap()[3], "Fillette");
        assert_eq!(game.text_file(l.type_names).unwrap()[11], "Plante");

        let (ctx, _) = load(game, &Settings::default()).unwrap();
        let rowlet = ctx.personal(722);
        assert_eq!(rowlet.data.len(), 0x54);
        assert_eq!(rowlet.base_stats().total(), 320);
        assert_eq!(ctx.types(722), vec![crate::pokemon::PokeType::Grass, crate::pokemon::PokeType::Flying]);
        // Évolutions converties : Brindibou → Efflèche au niveau 17.
        assert!(ctx.evo_tables[722].contains(&Evolution { method: METHOD_LEVEL, param: 17, target: 723 }));

        let gifts = game.garc(info(game.game).gifts).unwrap();
        let gifts = gifts.file(0).unwrap();
        for (i, s) in ORIGINAL_STARTERS.iter().enumerate() {
            assert_eq!(u16_at(gifts, i * GIFT_SIZE), *s);
            assert_eq!(gifts[i * GIFT_SIZE + 3], 5);
        }

        // Route 1 (zone 0) : Manglouton et Picassaut le jour, Rattata d'Alola la nuit.
        let garc = game.garc(l.encounters).unwrap();
        let zone = lz::decompress(garc.file(AREA_FIRST).unwrap()).unwrap();
        let slots = encounters::read(7, &zone);
        let first = encounters::alola_tables(&zone)[0];
        let day: Vec<u16> = slots.iter().filter(|s| s.offset < first + 4 + encounters::ALOLA_HALF).map(|s| s.species).collect();
        assert!(day.contains(&734) && day.contains(&731));
        assert!(slots.iter().any(|s| s.species == 19 && u16_at(&zone, s.offset) >> 11 == 1));
        assert_eq!((slots[0].min_level, slots[0].max_level), (2, 3));

        let trdata = entries(&game.garc(l.trainer_data).unwrap());
        let trpoke = entries(&game.garc(l.trainer_pokemon).unwrap());
        let (od, op) = team_to_oras(&trdata[1], &trpoke[1]).unwrap();
        let team = read_team(7, &od, &op).unwrap();
        assert_eq!((team.pokemon[0].species, team.pokemon[0].level), (first_trainer, 6));
        // Toutes les équipes se relisent et se réécrivent à l'identique.
        for i in 1..trdata.len() {
            let (od, op) = team_to_oras(&trdata[i], &trpoke[i]).unwrap();
            assert_eq!(team_from_oras(&trpoke[i], &od, &op).unwrap(), trpoke[i], "dresseur {i}");
        }
    }

    #[test]
    fn moon_formats() {
        let Some(game) = rom(MOON) else { return };
        assert_eq!(game.layout.encounters, "a/0/8/3");
        check_formats(&game, 278); // Goélise
    }

    #[test]
    fn ultra_sun_formats() {
        let Some(game) = rom(ULTRA_SUN) else { return };
        assert_eq!(game.layout.encounters, "a/0/8/2");
        check_formats(&game, 734); // Manglouton
    }

    /// Randomisation complète vers un dossier LayeredFS, puis relecture des fichiers écrits.
    fn full_randomization(game: &CtrGameRom, tag: &str) {
        let settings = Settings {
            starters: StarterMode::Custom,
            custom_starters: [1, 4, 7],
            wild: WildMode::Area,
            wild_level_percent: 150,
            trainers: TrainerMode::Random,
            trainer_level_percent: 120,
            trainer_max_ivs: true,
            stats: StatsMode::Shuffle,
            random_types: true,
            random_abilities: true,
            catch_rate: CatchRateMode::Max,
            easy_evolutions: true,
            random_movesets: true,
            ..Settings::default()
        };
        let out = std::env::temp_dir().join(format!("kaleido-gen7-{tag}-{}", std::process::id()));
        let (outcome, written) = super::super::ctr::randomize(game, &settings, 42, &out, LayeredFsTarget::Emulator).unwrap();
        let romfs = written.romfs.clone().unwrap();
        let read = |p: &str| Garc::parse(&std::fs::read(romfs.join(p)).unwrap()).unwrap();
        let l = game.layout;
        let gi = info(game.game);
        assert!(romfs.ends_with(format!("{:016X}/romfs", game.title_id())));
        assert!(outcome.wild_slots > 10_000 && outcome.trainer_pokemon > 500);
        let ids: Vec<u16> = outcome.starters.iter().map(|s| s.id).collect();
        assert_eq!(ids, [1, 4, 7]);

        // Starters et textes de l'écran de choix.
        let gifts = read(gi.gifts);
        let gifts = gifts.file(0).unwrap();
        assert_eq!([u16_at(gifts, 0), u16_at(gifts, GIFT_SIZE), u16_at(gifts, 2 * GIFT_SIZE)], [1, 4, 7]);
        let story = read(l.story_text);
        let text = MsgFile::parse_with(story.file(gi.starter_text).unwrap(), Variant::Gen6).unwrap().strings();
        let all = text.join("\n");
        assert!(all.contains("Bulbizarre") && all.contains("Salamèche") && all.contains("Carapuce"), "{all}");
        assert!(!all.contains("Brindibou") && !all.contains("Otaquin"), "{all}");

        // Fiches : taux de capture au maximum, dans chaque entrée et dans la table complète.
        let personal = read(l.personal);
        assert_eq!(personal.file(722).unwrap()[8], 255);
        let table = personal.file(personal.len() - 1).unwrap();
        assert_eq!(&table[722 * 0x54..723 * 0x54], personal.file(722).unwrap());

        // Évolutions : plus d'échange pour Kadabra.
        let evo = read(l.evolution);
        assert_eq!(&evo.file(64).unwrap()[..2], &METHOD_LEVEL.to_le_bytes());

        // Attaques apprises : aucune capacité Z.
        let learn = read(l.levelup);
        for s in 1..=l.species_count as usize {
            assert!(learnsets::read(7, learn.file(s).unwrap()).iter().all(|&(m, _)| !is_z_move(m)), "espèce {s}");
        }

        // Route 1 : niveaux × 1,5 (2-3 → 3-5).
        let wild = read(l.encounters);
        let zone = lz::decompress(wild.file(AREA_FIRST).unwrap()).unwrap();
        let slots = encounters::read(7, &zone);
        assert!(!slots.is_empty());
        assert_eq!((slots[0].min_level, slots[0].max_level), (3, 5));

        // Dresseurs : niveaux × 1,2, IV au maximum, équipes relisibles.
        let trdata = entries(&game.garc(l.trainer_data).unwrap());
        let trpoke = read(l.trainer_pokemon);
        let p = trpoke.file(1).unwrap();
        assert_eq!(u16_at(p, 0x0E), 7);
        assert_eq!(u32::from_le_bytes(p[8..12].try_into().unwrap()) & 0x3FFF_FFFF, 0x3FFF_FFFF);
        for (i, d) in trdata.iter().enumerate().skip(1) {
            let (od, op) = team_to_oras(d, trpoke.file(i).unwrap()).unwrap();
            assert!(read_team(7, &od, &op).is_some());
        }
        std::fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn moon_randomization() {
        let Some(game) = rom(MOON) else { return };
        full_randomization(&game, "lune");
    }

    #[test]
    fn ultra_sun_randomization() {
        let Some(game) = rom(ULTRA_SUN) else { return };
        full_randomization(&game, "ultra-soleil");
    }

    #[test]
    fn preview_matches() {
        let Some(game) = rom(MOON) else { return };
        let settings = Settings { starters: StarterMode::Triangle, ..Settings::default() };
        let preview = super::super::ctr::preview_starters(&game, &settings, 7).unwrap();
        let (outcome, _) = super::super::ctr::randomize(
            &game,
            &Settings { ctr_output: super::super::ctr::CtrOutput::LayeredFs, ..settings.clone() },
            7,
            &std::env::temp_dir().join(format!("kaleido-gen7-apercu-{}", std::process::id())),
            LayeredFsTarget::Luma,
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("kaleido-gen7-apercu-{}", std::process::id())));
        let ids = |v: &[PokemonRef]| v.iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(ids(&preview), ids(&outcome.starters));
        assert_ne!(ids(&preview), ORIGINAL_STARTERS.to_vec());
    }
}
