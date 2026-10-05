//! Dresseurs : fiche (trdata) + équipe (trpoke).
//!
//! trdata (20 octets) : drapeaux (bit 0 = attaques, bit 1 = objet), classe,
//! type de combat, nombre de Pokémon, objets, IA…
//!
//! trpoke, par Pokémon :
//! - Platine : difficulté u8, genre/talent u8, niveau u16, espèce u16 (forme en
//!   bits 10-15), [objet u16], [4 attaques u16], sceau de Ball u16.
//! - Noire/Blanche : difficulté u8, genre/talent u8, niveau u8, u8, espèce u16,
//!   forme u16, [objet u16], [4 attaques u16].

use serde::Serialize;

use super::{put_u16, u16_at};

pub const FLAG_MOVES: u8 = 0x01;
pub const FLAG_ITEM: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerPokemon {
    pub difficulty: u8,
    pub gender_ability: u8,
    pub level: u16,
    pub species: u16,
    pub form: u16,
    pub item: u16,
    pub moves: [u16; 4],
    /// Platine : sceau de Ball ; Gen 5 : octet libre après le niveau.
    pub extra: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    pub flags: u8,
    pub pokemon: Vec<TrainerPokemon>,
}

/// Taille d'une entrée : 8 octets de base dans les deux générations
/// (Platine : 6 + sceau de Ball ; Gen 5 : 8), plus l'objet et les attaques.
fn entry_size(flags: u8) -> usize {
    8 + if flags & FLAG_ITEM != 0 { 2 } else { 0 } + if flags & FLAG_MOVES != 0 { 8 } else { 0 }
}

/// Lit l'équipe d'un dresseur à partir de sa fiche et de son fichier trpoke.
pub fn read_team(generation: u8, trdata: &[u8], trpoke: &[u8]) -> Option<Team> {
    let flags = *trdata.first()?;
    let count = *trdata.get(3)? as usize;
    let size = entry_size(flags);
    let mut pokemon = Vec::with_capacity(count);
    for i in 0..count {
        let e = trpoke.get(i * size..(i + 1) * size)?;
        let mut at;
        let (level, species, form, extra);
        if generation <= 4 {
            level = u16_at(e, 2);
            let raw = u16_at(e, 4);
            species = raw & 0x03FF;
            form = raw >> 10;
            at = 6;
            extra = u16_at(e, size - 2);
        } else {
            level = e[2] as u16;
            extra = e[3] as u16;
            species = u16_at(e, 4);
            form = u16_at(e, 6);
            at = 8;
        }
        let mut item = 0;
        if flags & FLAG_ITEM != 0 {
            item = u16_at(e, at);
            at += 2;
        }
        let mut moves = [0; 4];
        if flags & FLAG_MOVES != 0 {
            for (m, slot) in moves.iter_mut().enumerate() {
                *slot = u16_at(e, at + m * 2);
            }
        }
        pokemon.push(TrainerPokemon { difficulty: e[0], gender_ability: e[1], level, species, form, item, moves, extra });
    }
    Some(Team { flags, pokemon })
}

/// Réécrit le fichier trpoke ; met à jour les drapeaux et le nombre dans trdata.
pub fn write_team(generation: u8, team: &Team, trdata: &mut [u8]) -> Vec<u8> {
    trdata[0] = team.flags;
    trdata[3] = team.pokemon.len() as u8;
    let size = entry_size(team.flags);
    let mut out = Vec::with_capacity(size * team.pokemon.len());
    for p in &team.pokemon {
        let mut e = vec![0u8; size];
        e[0] = p.difficulty;
        e[1] = p.gender_ability;
        let mut at;
        if generation <= 4 {
            put_u16(&mut e, 2, p.level);
            put_u16(&mut e, 4, (p.species & 0x03FF) | (p.form << 10));
            at = 6;
            put_u16(&mut e, size - 2, p.extra);
        } else {
            e[2] = p.level.min(255) as u8;
            e[3] = p.extra as u8;
            put_u16(&mut e, 4, p.species);
            put_u16(&mut e, 6, p.form);
            at = 8;
        }
        if team.flags & FLAG_ITEM != 0 {
            put_u16(&mut e, at, p.item);
            at += 2;
        }
        if team.flags & FLAG_MOVES != 0 {
            for (m, &mv) in p.moves.iter().enumerate() {
                put_u16(&mut e, at + m * 2, mv);
            }
        }
        out.extend(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platinum_team_with_moves() {
        // Dresseur réel de Platine : Cheniti niv. 5 avec Charge.
        let trdata = [0x01, 0x02, 0x00, 0x01];
        let trpoke = [0, 0, 5, 0, 0x9C, 0x01, 0x21, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let team = read_team(4, &trdata, &trpoke).unwrap();
        assert_eq!(team.pokemon[0].species, 412);
        assert_eq!(team.pokemon[0].level, 5);
        assert_eq!(team.pokemon[0].moves, [33, 0, 0, 0]);
        let mut td = trdata;
        assert_eq!(write_team(4, &team, &mut td), trpoke);
    }

    #[test]
    fn bw_team_roundtrip() {
        let trdata = [0x00, 0x04, 0x00, 0x01];
        let trpoke = [50, 0, 13, 0, 0x0A, 0x02, 0, 0];
        let team = read_team(5, &trdata, &trpoke).unwrap();
        assert_eq!((team.pokemon[0].species, team.pokemon[0].level, team.pokemon[0].difficulty), (522, 13, 50));
        let mut td = trdata;
        assert_eq!(write_team(5, &team, &mut td), trpoke);
    }
}
