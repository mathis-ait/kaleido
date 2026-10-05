//! Table des types par génération.
//!
//! Identifiants « canoniques » (ceux de PKHeX et de [`crate::dex`]) : 0 Normal,
//! 1 Combat, 2 Vol, 3 Poison, 4 Sol, 5 Roche, 6 Insecte, 7 Spectre, 8 Acier,
//! 9 Feu, 10 Eau, 11 Plante, 12 Électrik, 13 Psy, 14 Glace, 15 Dragon,
//! 16 Ténèbres, 17 Fée. Les ROM Gen 4 rangent « ??? » en 9 : voir [`from_rom`].
//!
//! Différences entre générations :
//! - Gen 2 à 5 : Acier résiste à Spectre et à Ténèbres.
//! - Gen 6+ : ces deux résistances disparaissent, le type Fée apparaît.

use crate::pokemon::PokeType;

pub const NORMAL: u8 = 0;
pub const FIGHTING: u8 = 1;
pub const FLYING: u8 = 2;
pub const POISON: u8 = 3;
pub const GROUND: u8 = 4;
pub const ROCK: u8 = 5;
pub const BUG: u8 = 6;
pub const GHOST: u8 = 7;
pub const STEEL: u8 = 8;
pub const FIRE: u8 = 9;
pub const WATER: u8 = 10;
pub const GRASS: u8 = 11;
pub const ELECTRIC: u8 = 12;
pub const PSYCHIC: u8 = 13;
pub const ICE: u8 = 14;
pub const DRAGON: u8 = 15;
pub const DARK: u8 = 16;
pub const FAIRY: u8 = 17;

/// Efficacité d'une attaque sur un type, en quarts : 0 (immunité), 2 (½), 4 (×1), 8 (×2).
pub type Eff = u8;
pub const IMMUNE: Eff = 0;
pub const RESISTED: Eff = 2;
pub const NEUTRAL: Eff = 4;
pub const SUPER: Eff = 8;

/// Couples (attaque, défense, efficacité) différents de ×1, table de la Gen 6.
#[rustfmt::skip]
const CHART_G6: &[(u8, u8, Eff)] = &[
    (NORMAL, ROCK, RESISTED), (NORMAL, GHOST, IMMUNE), (NORMAL, STEEL, RESISTED),
    (FIGHTING, NORMAL, SUPER), (FIGHTING, FLYING, RESISTED), (FIGHTING, POISON, RESISTED), (FIGHTING, ROCK, SUPER),
    (FIGHTING, BUG, RESISTED), (FIGHTING, GHOST, IMMUNE), (FIGHTING, STEEL, SUPER), (FIGHTING, PSYCHIC, RESISTED),
    (FIGHTING, ICE, SUPER), (FIGHTING, DARK, SUPER), (FIGHTING, FAIRY, RESISTED),
    (FLYING, FIGHTING, SUPER), (FLYING, ROCK, RESISTED), (FLYING, BUG, SUPER), (FLYING, STEEL, RESISTED),
    (FLYING, GRASS, SUPER), (FLYING, ELECTRIC, RESISTED),
    (POISON, POISON, RESISTED), (POISON, GROUND, RESISTED), (POISON, ROCK, RESISTED), (POISON, GHOST, RESISTED),
    (POISON, STEEL, IMMUNE), (POISON, GRASS, SUPER), (POISON, FAIRY, SUPER),
    (GROUND, FLYING, IMMUNE), (GROUND, POISON, SUPER), (GROUND, ROCK, SUPER), (GROUND, BUG, RESISTED),
    (GROUND, STEEL, SUPER), (GROUND, FIRE, SUPER), (GROUND, GRASS, RESISTED), (GROUND, ELECTRIC, SUPER),
    (ROCK, FIGHTING, RESISTED), (ROCK, FLYING, SUPER), (ROCK, GROUND, RESISTED), (ROCK, BUG, SUPER),
    (ROCK, STEEL, RESISTED), (ROCK, FIRE, SUPER), (ROCK, ICE, SUPER),
    (BUG, FIGHTING, RESISTED), (BUG, FLYING, RESISTED), (BUG, POISON, RESISTED), (BUG, GHOST, RESISTED),
    (BUG, STEEL, RESISTED), (BUG, FIRE, RESISTED), (BUG, GRASS, SUPER), (BUG, PSYCHIC, SUPER), (BUG, DARK, SUPER),
    (BUG, FAIRY, RESISTED),
    (GHOST, NORMAL, IMMUNE), (GHOST, GHOST, SUPER), (GHOST, PSYCHIC, SUPER), (GHOST, DARK, RESISTED),
    (STEEL, ROCK, SUPER), (STEEL, STEEL, RESISTED), (STEEL, FIRE, RESISTED), (STEEL, WATER, RESISTED),
    (STEEL, ELECTRIC, RESISTED), (STEEL, ICE, SUPER), (STEEL, FAIRY, SUPER),
    (FIRE, ROCK, RESISTED), (FIRE, BUG, SUPER), (FIRE, STEEL, SUPER), (FIRE, FIRE, RESISTED), (FIRE, WATER, RESISTED),
    (FIRE, GRASS, SUPER), (FIRE, ICE, SUPER), (FIRE, DRAGON, RESISTED),
    (WATER, GROUND, SUPER), (WATER, ROCK, SUPER), (WATER, FIRE, SUPER), (WATER, WATER, RESISTED),
    (WATER, GRASS, RESISTED), (WATER, DRAGON, RESISTED),
    (GRASS, FLYING, RESISTED), (GRASS, POISON, RESISTED), (GRASS, GROUND, SUPER), (GRASS, ROCK, SUPER),
    (GRASS, BUG, RESISTED), (GRASS, STEEL, RESISTED), (GRASS, FIRE, RESISTED), (GRASS, WATER, SUPER),
    (GRASS, GRASS, RESISTED), (GRASS, DRAGON, RESISTED),
    (ELECTRIC, FLYING, SUPER), (ELECTRIC, GROUND, IMMUNE), (ELECTRIC, WATER, SUPER), (ELECTRIC, GRASS, RESISTED),
    (ELECTRIC, ELECTRIC, RESISTED), (ELECTRIC, DRAGON, RESISTED),
    (PSYCHIC, FIGHTING, SUPER), (PSYCHIC, POISON, SUPER), (PSYCHIC, STEEL, RESISTED), (PSYCHIC, PSYCHIC, RESISTED),
    (PSYCHIC, DARK, IMMUNE),
    (ICE, FLYING, SUPER), (ICE, GROUND, SUPER), (ICE, STEEL, RESISTED), (ICE, FIRE, RESISTED), (ICE, WATER, RESISTED),
    (ICE, GRASS, SUPER), (ICE, ICE, RESISTED), (ICE, DRAGON, SUPER),
    (DRAGON, STEEL, RESISTED), (DRAGON, DRAGON, SUPER), (DRAGON, FAIRY, IMMUNE),
    (DARK, FIGHTING, RESISTED), (DARK, GHOST, SUPER), (DARK, PSYCHIC, SUPER), (DARK, DARK, RESISTED), (DARK, FAIRY, RESISTED),
    (FAIRY, FIGHTING, SUPER), (FAIRY, POISON, RESISTED), (FAIRY, STEEL, RESISTED), (FAIRY, FIRE, RESISTED),
    (FAIRY, DRAGON, SUPER), (FAIRY, DARK, SUPER),
];

/// Efficacité (en quarts) d'une attaque de type `attack` sur un type `defense`.
pub fn effectiveness(generation: u8, attack: u8, defense: u8) -> Eff {
    if generation <= 5 {
        // Pas de type Fée ; Acier résiste encore à Spectre et Ténèbres.
        if attack == FAIRY || defense == FAIRY {
            return NEUTRAL;
        }
        if defense == STEEL && (attack == GHOST || attack == DARK) {
            return RESISTED;
        }
    }
    CHART_G6.iter().find(|&&(a, d, _)| a == attack && d == defense).map_or(NEUTRAL, |&(_, _, e)| e)
}

/// Multiplicateur total sur un Pokémon à un ou deux types (×0, ×0,25 … ×4), en float pour l'affichage.
pub fn total(generation: u8, attack: u8, types: &[u8]) -> f32 {
    distinct(types).iter().map(|&t| effectiveness(generation, attack, t) as f32 / 4.0).product()
}

/// Types sans doublon (un Pokémon mono-type a ses deux types égaux dans les données).
pub fn distinct(types: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(2);
    for &t in types {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Convertit un type tel qu'il est stocké dans la ROM vers l'identifiant canonique.
pub fn from_rom(generation: u8, raw: u8) -> u8 {
    match PokeType::from_index(generation, raw) {
        Some(PokeType::Mystery) | None => NORMAL,
        Some(t) => canonical(t),
    }
}

fn canonical(t: PokeType) -> u8 {
    // L'ordre de la Gen 6 (sans « ??? ») est l'ordre canonique.
    (0..18).find(|&i| PokeType::from_index(6, i) == Some(t)).unwrap_or(NORMAL)
}

/// Nom français d'un type canonique.
pub fn name(t: u8) -> &'static str {
    PokeType::from_index(6, t).map_or("?", PokeType::name_fr)
}
