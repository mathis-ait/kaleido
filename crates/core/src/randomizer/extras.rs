//! Options complémentaires : évolutions sans échange, attaques aléatoires,
//! évolution des Pokémon des dresseurs selon leur niveau.

use std::fmt::Write as _;

use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use super::{rng_for, Ctx};
use crate::data::evolutions::{self, METHOD_LEVEL};
use crate::data::learnsets;

/// Lutte : jamais attribuée.
const STRUGGLE: u16 = 165;
/// Niveau à partir duquel un Pokémon de dresseur évolue par une méthode autre que le niveau.
const OTHER_METHOD_LEVEL: u16 = 40;

/// Évolutions par échange → niveau 37 ou objet. `files` : une entrée par espèce.
pub(crate) fn easy_evolutions(ctx: &mut Ctx, files: &mut [Vec<u8>], log: &mut String) -> usize {
    let _ = writeln!(log, "== Évolutions sans échange ==");
    let mut total = 0;
    for s in 1..=(ctx.count as usize).min(files.len().saturating_sub(1)) {
        if evolutions::remove_trade_evolutions(ctx.gen, &mut files[s]) > 0 {
            total += 1;
            let targets: Vec<&str> = evolutions::read(&files[s]).iter().map(|e| ctx.name(e.target)).collect();
            let _ = writeln!(log, "{} → {}", ctx.name(s as u16), targets.join(" / "));
        }
        if let Some(table) = ctx.evo_tables.get_mut(s) {
            *table = evolutions::read(&files[s]);
        }
    }
    let _ = writeln!(log);
    total
}

/// Attaques apprises aléatoires ; la première (niveau 1) est gardée pour que chaque
/// Pokémon ait une attaque offensive dès le départ. `max_move` : dernier identifiant.
pub(crate) fn random_movesets(ctx: &mut Ctx, files: &mut [Vec<u8>], max_move: u16, seed: u64, log: &mut String) -> usize {
    let mut rng = rng_for(seed, "movesets");
    let pool: Vec<u16> = (1..=max_move).filter(|&m| m != STRUGGLE).collect();
    let mut total = 0;
    for s in 1..=(ctx.count as usize).min(files.len().saturating_sub(1)) {
        learnsets::map_moves(ctx.gen, &mut files[s], |i, mv, _| {
            if i == 0 {
                mv
            } else {
                total += 1;
                *pool.choose(&mut rng).unwrap_or(&mv)
            }
        });
        if let Some(ls) = ctx.learnsets.get_mut(s) {
            *ls = learnsets::read(ctx.gen, &files[s]);
        }
    }
    let _ = writeln!(log, "== Attaques apprises ==\n{total} attaques remplacées au hasard.\n");
    total
}

/// Fait évoluer une espèce tant que le niveau le permet (évolution par niveau atteinte,
/// ou niveau ≥ 40 pour les autres méthodes). Choix au hasard en cas d'évolutions multiples.
pub(crate) fn evolve_for_level(ctx: &Ctx, rng: &mut ChaCha8Rng, species: u16, level: u16) -> u16 {
    let mut current = species;
    for _ in 0..3 {
        let options: Vec<u16> = ctx
            .evo_tables
            .get(current as usize)
            .into_iter()
            .flatten()
            .filter(|e| e.target != 0 && e.target <= ctx.count && ctx.allowed(e.target))
            .filter(|e| if e.method == METHOD_LEVEL { e.param <= level } else { level >= OTHER_METHOD_LEVEL })
            .map(|e| e.target)
            .collect();
        match options.choose(rng) {
            Some(&next) => current = next,
            None => break,
        }
    }
    current
}
