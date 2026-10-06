//! Dresseurs des jeux 3DS : rôles de X / Y et de la Gen 7, fiches de Soleil / Lune et
//! d'Ultra-Soleil / Ultra-Lune.
//!
//! **Gen 7** (pk3DS `TrainerData7` / `TrainerPoke7`, UPR-ZX `Gen7RomHandler`, vérifié sur
//! Lune et Ultra-Soleil Europe) : `trdata` de 0x14 octets — `u16` classe @0, `u8` type de
//! combat @2 (0 simple, 1 double, 2 multi ; toujours 0 dans les deux ROM), `u8` nombre de
//! Pokémon @3 ; `trpoke` de 0x20 octets par Pokémon — `u8` talent << 4 | sexe @0, `u8`
//! nature @1, 6 EV @2 (PV, Att, Déf, Atq Spé, Déf Spé, Vit), `u32` IV @8 (5 bits par
//! statistique, même ordre ; bit 30 : chromatique), `u16` niveau @0xE, `u16` espèce @0x10,
//! `u16` forme @0x12, `u16` objet @0x14, 4 × `u16` attaques @0x18 (toutes nulles : attaques
//! apprises par niveau). Contrairement aux Gen 4 à 6, nature, IV et EV sont donc exacts.
//!
//! **Rôles** : d'après les classes de dresseurs lues dans les ROM (textes 20 de Y, 106 de
//! Lune, 111 d'Ultra-Soleil), noms et équipes contrôlés (Violette n°6, Dianthéa n°276 ;
//! Pectorius n°23, Althéo n°52, Conseil 4, Euphorbe n°129 en Lune, Tili n°494 en Ultra-Soleil).

use super::Role;
use crate::data::trainers::TrainerPokemon;
use crate::data::u16_at;
use crate::games::Game;

/// Taille d'un Pokémon de dresseur Gen 7.
const TRPOKE7_SIZE: usize = 0x20;

/// Nature, IV et EV fixés par le jeu (Gen 7), dans l'ordre Kaleido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactStats {
    pub nature: u8,
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
}

/// Une fiche Gen 7 lue : classe, combat à plusieurs, équipe et valeurs exactes.
pub(super) struct Gen7Trainer {
    pub class: u16,
    pub double: bool,
    pub team: Vec<TrainerPokemon>,
    pub exact: Vec<ExactStats>,
}

/// Lit une fiche et une équipe Gen 7 ; `None` si illisible.
pub(super) fn read_gen7(trdata: &[u8], trpoke: &[u8]) -> Option<Gen7Trainer> {
    if trdata.len() < 4 {
        return None;
    }
    let count = trdata[3] as usize;
    let data = trpoke.get(..count * TRPOKE7_SIZE)?;
    let mut team = Vec::with_capacity(count);
    let mut exact = Vec::with_capacity(count);
    for p in data.as_chunks::<TRPOKE7_SIZE>().0 {
        let iv32 = u32::from_le_bytes([p[8], p[9], p[10], p[11]]);
        let ivs = std::array::from_fn(|i| ((iv32 >> (5 * i)) & 0x1F) as u8);
        exact.push(ExactStats { nature: p[1] % 25, ivs, evs: [p[2], p[3], p[4], p[5], p[6], p[7]] });
        team.push(TrainerPokemon {
            // Les IV sont exacts (ci-dessus) : la « difficulté » ne sert pas.
            difficulty: 0,
            gender_ability: p[0],
            level: u16_at(p, 0x0E),
            species: u16_at(p, 0x10),
            form: u16_at(p, 0x12),
            item: u16_at(p, 0x14),
            moves: std::array::from_fn(|m| u16_at(p, 0x18 + m * 2)),
            extra: 0,
        });
    }
    Some(Gen7Trainer { class: u16_at(trdata, 0), double: trdata[2] != 0, team, exact })
}

/// Rôle d'un dresseur de X / Y ou de la Gen 7 d'après sa classe (`None` pour les autres
/// jeux : voir `role_of`). Les entrées sans nom (dresseurs factices) ne sont jamais importantes.
pub(super) fn role(game: Game, class: u16, name: &str) -> Option<Role> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('{') || name.starts_with('●') {
        return None;
    }
    let ultra = matches!(game, Game::UltraSun | Game::UltraMoon);
    match game {
        // Classes de Y (texte 20).
        Game::X | Game::Y => Some(match class {
            // Violette (4), Lino (39), Astera (40), Cornélia (41 ; 177 à la Tour Maîtrise),
            // Amaro (42), Urup (43), Lem (44), Valériane (45).
            4 | 39..=45 | 177 => Role::Gym,
            35..=38 => Role::EliteFour,
            // Dianthéa.
            53 => Role::Champion,
            // Sannah (55), Tierno (56), Trovato (57), Kalem (103), Serena (104).
            55..=57 | 103 | 104 => Role::Rival,
            // Lysandre (81 en tenue Team Flare, 175 en chef).
            81 | 175 => Role::Boss,
            // Scientifiques (Ancolie, Brasénie, Cyane, Myosotis, Xanthin) et Managers.
            18..=22 | 77 | 78 => Role::Admin,
            _ => return None,
        }),
        // Classes de Lune (texte 106) et d'Ultra-Soleil (texte 111), identiques jusqu'à 185
        // sauf la 111 (« Professeur Pokémon » en Soleil / Lune, inutilisée en Ultra).
        Game::Sun | Game::Moon | Game::UltraSun | Game::UltraMoon => Some(match class {
            // Capitaines (38, 43-48, 142, 151-153) et Doyens (31, 49-51, 141, 164).
            31 | 38 | 43..=51 | 141 | 142 | 151..=153 | 164 => Role::Gym,
            // Conseil 4 (Molène : 191, Ultra).
            80 | 107 | 109 | 110 | 191 => Role::EliteFour,
            // Euphorbe : combat du Maître (111, Soleil / Lune) puis défenses du titre (165) ;
            // Tili est le dernier adversaire de la Ligue en Ultra (194).
            111 if !ultra => Role::Champion,
            165 | 194 => Role::Champion,
            // Tili (30, 100, 101 ; 221 et 222 en Ultra) et Gladio (70, 102, 103).
            30 | 70 | 100..=103 | 221 | 222 => Role::Rival,
            // Elsa-Mina (71, 82, 220), Guzma (76, 140, 219), chefs de la Team Rainbow Rocket (Ultra).
            71 | 76 | 82 | 140 | 219 | 220 | 198..=202 | 206 => Role::Boss,
            // Saubohne (72, 162, 185), Apocyne (78, 79).
            72 | 78 | 79 | 162 | 185 => Role::Admin,
            _ => return None,
        }),
        _ => None,
    }
}

/// Libellé d'un rôle selon le jeu : en Alola, pas d'arènes mais des capitaines et des doyens.
pub(super) fn label(game: Game, role: Role) -> &'static str {
    match role {
        Role::Gym if game.generation() == 7 => "Capitaine / Doyen",
        _ => role.label(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen7_trainer() {
        // Pectorius (Lune, n°23), premier Pokémon : Férosinge niv. 14, IV 31 partout.
        let trdata = [31, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0x07, 0, 0, 0, 0, 0x20, 0, 0];
        let mut trpoke = vec![0u8; 0x20];
        trpoke[0] = 0x10; // premier talent
        trpoke[1] = 3; // Rigide
        trpoke[2..8].copy_from_slice(&[0, 4, 0, 0, 0, 252]);
        trpoke[8..12].copy_from_slice(&0x3FFF_FFFFu32.to_le_bytes());
        trpoke[0x0E] = 14;
        trpoke[0x10..0x12].copy_from_slice(&56u16.to_le_bytes());
        trpoke[0x18..0x1A].copy_from_slice(&2u16.to_le_bytes());
        let t = read_gen7(&trdata, &trpoke).unwrap();
        assert_eq!((t.class, t.double, t.team.len()), (31, false, 1));
        let p = &t.team[0];
        assert_eq!((p.species, p.level, p.gender_ability, p.moves), (56, 14, 0x10, [2, 0, 0, 0]));
        assert_eq!(t.exact[0], ExactStats { nature: 3, ivs: [31; 6], evs: [0, 4, 0, 0, 0, 252] });
        assert!(read_gen7(&trdata, &trpoke[..0x10]).is_none());
        assert_eq!(role(Game::Moon, 31, "Pectorius"), Some(Role::Gym));
        assert_eq!(role(Game::Moon, 111, "Euphorbe"), Some(Role::Champion));
        assert_eq!(role(Game::UltraSun, 111, "Euphorbe"), None);
        assert_eq!(role(Game::Moon, 38, "{VAR:BDFF,0003}"), None);
        assert_eq!(role(Game::Y, 4, "Violette"), Some(Role::Gym));
        assert_eq!(role(Game::OmegaRuby, 200, "Roxanne"), None, "ROSA : voir role_of");
        assert_eq!(label(Game::Moon, Role::Gym), "Capitaine / Doyen");
        assert_eq!(label(Game::Y, Role::Gym), "Champion d'Arène");
    }
}
