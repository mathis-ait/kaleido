//! Dresseurs : fiche (trdata) + équipe (trpoke).
//!
//! trdata (20 octets) : drapeaux (bit 0 = attaques, bit 1 = objet), classe,
//! type de combat, nombre de Pokémon, objets, IA…
//!
//! trpoke, par Pokémon :
//! - Platine / HeartGold / SoulSilver : difficulté u8, genre/talent u8, niveau u16,
//!   espèce u16 (forme en bits 10-15), [objet u16], [4 attaques u16], sceau de Ball u16.
//! - Diamant / Perle : idem sans le sceau de Ball (6 octets de base ; UPR-ZX
//!   `getTrainers`, vérifié sur Diamant ADAF).
//! - Noire/Blanche : difficulté u8, genre/talent u8, niveau u8, u8, espèce u16,
//!   forme u16, [objet u16], [4 attaques u16].

use serde::Serialize;

use super::{put_u16, u16_at};
use crate::games::Game;

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

/// Format des fichiers trpoke d'un jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamFormat {
    pub generation: u8,
    /// Gen 4 : sceau de Ball (u16) en fin d'entrée (absent de Diamant / Perle).
    pub ball_seal: bool,
}

impl TeamFormat {
    /// Format par défaut d'une génération (Gen 4 : Platine / HGSS).
    pub fn for_generation(generation: u8) -> Self {
        Self { generation, ball_seal: generation == 4 }
    }

    pub fn for_game(game: Game) -> Self {
        let mut f = Self::for_generation(game.generation());
        if matches!(game, Game::Diamond | Game::Pearl) {
            f.ball_seal = false;
        }
        f
    }

    /// Taille d'une entrée : 8 octets de base (Platine : 6 + sceau de Ball ; Gen 5 : 8 ;
    /// Diamant / Perle : 6), plus l'objet et les attaques.
    fn entry_size(&self, flags: u8) -> usize {
        let base = if self.generation <= 4 && !self.ball_seal { 6 } else { 8 };
        base + if flags & FLAG_ITEM != 0 { 2 } else { 0 } + if flags & FLAG_MOVES != 0 { 8 } else { 0 }
    }
}

/// Position du nombre de Pokémon dans trdata (Rubis Oméga / Saphir Alpha : fiche de 0x18 octets).
fn count_offset(generation: u8) -> usize {
    if generation >= 6 {
        7
    } else {
        3
    }
}

/// Lit l'équipe d'un dresseur à partir de sa fiche et de son fichier trpoke
/// (format par défaut de la génération ; voir [read_team_with]).
pub fn read_team(generation: u8, trdata: &[u8], trpoke: &[u8]) -> Option<Team> {
    read_team_with(TeamFormat::for_generation(generation), trdata, trpoke)
}

pub fn read_team_with(format: TeamFormat, trdata: &[u8], trpoke: &[u8]) -> Option<Team> {
    let generation = format.generation;
    let flags = *trdata.first()?;
    let count = *trdata.get(count_offset(generation))? as usize;
    let size = format.entry_size(flags);
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
            extra = if format.ball_seal { u16_at(e, size - 2) } else { 0 };
        } else if generation >= 6 {
            level = u16_at(e, 2);
            species = u16_at(e, 4);
            form = u16_at(e, 6);
            extra = 0;
            at = 8;
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
    write_team_with(TeamFormat::for_generation(generation), team, trdata)
}

pub fn write_team_with(format: TeamFormat, team: &Team, trdata: &mut [u8]) -> Vec<u8> {
    let generation = format.generation;
    trdata[0] = team.flags;
    trdata[count_offset(generation)] = team.pokemon.len() as u8;
    let size = format.entry_size(team.flags);
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
            if format.ball_seal {
                put_u16(&mut e, size - 2, p.extra);
            }
        } else if generation >= 6 {
            put_u16(&mut e, 2, p.level);
            put_u16(&mut e, 4, p.species);
            put_u16(&mut e, 6, p.form);
            at = 8;
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

/// X / Y : fiche `trdata` de 0x14 octets — `u8` drapeaux @0, `u8` classe @1, `u8` type
/// de combat @2, `u8` nombre de Pokémon @3, 4 × `u16` objets @4, IA @0xC, `u8`
/// multiplicateur d'argent @0x11 (Universal Pokémon Randomizer, Gen6RomHandler.getTrainers ;
/// vérifié sur Pokémon Y : Violette n°6, Surskit niv. 10 et Prismillon niv. 12). Les
/// équipes (`trpoke`) ont le même format qu'en Rubis Oméga / Saphir Alpha.
///
/// Renvoie une fiche au format de Rubis Oméga / Saphir Alpha (0x18 octets : drapeaux
/// @0, classe @2, type de combat @6, nombre @7), lisible par [`read_team`] avec la
/// génération 6 ; vide si la fiche est trop courte.
pub fn xy_trdata_as_oras(d: &[u8]) -> Vec<u8> {
    if d.len() < 4 {
        return Vec::new();
    }
    let mut out = vec![0u8; 0x18];
    out[0] = d[0];
    out[2] = d[1];
    out[6] = d[2];
    out[7] = d[3];
    out
}

/// Recopie dans une fiche X / Y les drapeaux et le nombre de Pokémon d'une fiche
/// obtenue par [`xy_trdata_as_oras`] puis modifiée par [`write_team`].
pub fn xy_trdata_update(d: &mut [u8], oras: &[u8]) {
    if d.len() >= 4 && oras.len() >= 8 {
        d[0] = oras[0];
        d[3] = oras[7];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xy_trainer_roundtrip() {
        // Violette (Pokémon Y, dresseur n°6) : attaques personnalisées, 2 Pokémon.
        let mut trdata = [0x01, 0x04, 0x00, 0x02, 0x11, 0, 0, 0, 0, 0, 0, 0, 0x05, 0, 0, 0, 0, 0x28, 0, 0];
        let trpoke = [
            0x96, 0x00, 0x0A, 0x00, 0x1B, 0x01, 0x00, 0x00, 0x62, 0x00, 0x91, 0x00, 0x5A, 0x01, 0x00, 0x00, 0x96, 0x00, 0x0C, 0x00, 0x9A, 0x02, 0x06,
            0x00, 0x6A, 0x00, 0x63, 0x02, 0x21, 0x00, 0x00, 0x00,
        ];
        let oras = xy_trdata_as_oras(&trdata);
        let team = read_team(6, &oras, &trpoke).unwrap();
        let summary: Vec<(u16, u16, u16)> = team.pokemon.iter().map(|p| (p.species, p.form, p.level)).collect();
        assert_eq!(summary, [(283, 0, 10), (666, 6, 12)]); // Arakdo, Prismillon (forme 6)
        assert_eq!(team.pokemon[0].moves, [98, 145, 346, 0]);
        let mut oras = oras;
        let mut short = team.clone();
        short.pokemon.truncate(1);
        let written = write_team(6, &short, &mut oras);
        assert_eq!(written, trpoke[..16]);
        xy_trdata_update(&mut trdata, &oras);
        assert_eq!((trdata[0], trdata[1], trdata[3], trdata[0x11]), (0x01, 0x04, 1, 0x28));
        assert!(xy_trdata_as_oras(&[]).is_empty());
    }

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
    fn diamond_pearl_team_without_seal() {
        // Deux Pokémon sans attaques ni objet : 6 octets chacun en Diamant / Perle.
        let trdata = [0x00, 0x02, 0x00, 0x02];
        let trpoke = [0, 0, 3, 0, 0x0A, 0x01, 0, 0, 4, 0, 0x8C, 0x01];
        let fmt = TeamFormat::for_game(Game::Diamond);
        let team = read_team_with(fmt, &trdata, &trpoke).unwrap();
        assert_eq!(team.pokemon.iter().map(|p| (p.species, p.level)).collect::<Vec<_>>(), vec![(266, 3), (396, 4)]);
        let mut td = trdata;
        assert_eq!(write_team_with(fmt, &team, &mut td), trpoke);
        assert!(read_team(4, &trdata, &trpoke).is_none(), "format Platine : entrées de 8 octets");
        assert!(TeamFormat::for_game(Game::HeartGold).ball_seal);
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
