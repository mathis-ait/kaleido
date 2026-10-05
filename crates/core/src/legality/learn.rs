//! Attaques apprenables par jeu : par niveau, CT/CS, donneurs de capacités, capacités
//! Œuf et cas particuliers (d'après `LearnSource4DP..7USUM.cs` et `PersonalInfo4..7.cs`
//! de PKHeX). Les bits des CT et des donneurs sont lus dans les fiches `personal_*`.

use std::sync::LazyLock;

use serde::Serialize;

use super::encounters::binlinker32;
use crate::dex::{self, Game};

/// Façon dont une attaque a pu être apprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LearnMethod {
    LevelUp,
    Machine,
    Tutor,
    Egg,
    Special,
}

impl LearnMethod {
    pub fn label(self) -> &'static str {
        match self {
            LearnMethod::LevelUp => "par niveau",
            LearnMethod::Machine => "par CT/CS",
            LearnMethod::Tutor => "par un donneur de capacités",
            LearnMethod::Egg => "comme capacité Œuf",
            LearnMethod::Special => "par un moyen spécial",
        }
    }
}

// --- Listes de PKHeX (indices des bits dans les fiches).

/// CT Gen 4 (`PersonalInfo4.MachineMovesTechnical`), bits 0 à 91.
const TM4: [u16; 92] = [
    264, 337, 352, 347, 46, 92, 258, 339, 331, 237, 241, 269, 58, 59, 63, 113, 182, 240, 202, 219, 218, 76, 231, 85, 87, 89, 216, 91, 94, 247,
    280, 104, 115, 351, 53, 188, 201, 126, 317, 332, 259, 263, 290, 156, 213, 168, 211, 285, 289, 315, 355, 411, 412, 206, 362, 374, 451, 203,
    406, 409, 261, 318, 373, 153, 421, 371, 278, 416, 397, 148, 444, 419, 86, 360, 14, 446, 244, 445, 399, 157, 404, 214, 363, 398, 138, 447,
    207, 365, 369, 164, 430, 433,
];
/// CS de DPPt puis de HGSS (bits 92 à 99) : Coupe, Vol, Surf, Force, Anti-Brume / Siphon, Éclate-Roc, Cascade, Escalade.
const HM4_DPPT: [u16; 8] = [15, 19, 57, 70, 432, 249, 127, 431];
const HM4_HGSS: [u16; 8] = [15, 19, 57, 70, 250, 249, 127, 431];
/// Donneurs de capacités de Platine / HGSS (`PersonalInfo4.TutorMoves`, bits de `tutors_g4.pkl`).
const TUTOR4: [u16; 52] = [
    291, 189, 210, 196, 205, 9, 7, 276, 8, 442, 401, 466, 380, 173, 180, 314, 270, 283, 200, 246, 235, 324, 428, 410, 414, 441, 239, 402, 334,
    393, 387, 340, 271, 257, 282, 389, 129, 253, 162, 220, 81, 366, 356, 388, 277, 272, 215, 67, 143, 335, 450, 29,
];
const BLAST_BURN: [u16; 4] = [6, 157, 257, 392];
const HYDRO_CANNON: [u16; 4] = [9, 160, 260, 395];
const FRENZY_PLANT: [u16; 4] = [3, 154, 254, 389];
const DRACO_METEOR: [u16; 19] = [147, 148, 149, 230, 329, 330, 334, 371, 372, 373, 380, 381, 384, 443, 444, 445, 483, 484, 487];

/// CT + CS Gen 5 (`PersonalInfo5BW.MachineMoves`), 101 bits.
const TM5: [u16; 101] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 477, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247,
    280, 104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 502, 411, 412, 206, 503, 374, 451, 507,
    510, 511, 261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 526, 398, 138, 447,
    207, 365, 369, 164, 430, 433, 528, 249, 555, 15, 19, 57, 70, 127, 291,
];
/// Capacités des donneurs « de type » (Aire d'Herbe… Draco Météore, Draco Ascension).
const TYPE_TUTOR: [u16; 8] = [520, 519, 518, 338, 307, 308, 434, 620];
/// Donneurs de N2/B2 et de ROSA (Méanville, Ondes-sur-Mer, Port Yoneuve, Maillard).
const TUTOR5_1: [u16; 15] = [450, 343, 162, 530, 324, 442, 402, 529, 340, 67, 441, 253, 9, 7, 8];
const TUTOR5_2: [u16; 17] = [277, 335, 414, 492, 356, 393, 334, 387, 276, 527, 196, 401, 399, 428, 406, 304, 231];
const TUTOR5_3: [u16; 13] = [20, 173, 282, 235, 257, 272, 215, 366, 143, 220, 202, 409, 355];
const TUTOR5_4: [u16; 15] = [380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278, 289, 446, 214, 285];
const TUTOR6_3: [u16; 16] = [20, 173, 282, 235, 257, 272, 215, 366, 143, 220, 202, 409, 355, 264, 351, 352];

/// CT + CS de XY (`LearnSource6XY.MachineMoves`), 105 bits.
const TM6_XY: [u16; 105] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247,
    280, 104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507,
    510, 511, 261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447,
    207, 214, 369, 164, 430, 433, 528, 249, 555, 267, 399, 612, 605, 590, 15, 19, 57, 70, 127,
];
/// CT + CS de ROSA (`LearnSource6AO.MachineMoves`), 107 bits.
const TM6_AO: [u16; 107] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247,
    280, 104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507,
    510, 511, 261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447,
    207, 214, 369, 164, 430, 433, 528, 290, 555, 267, 399, 612, 605, 590, 15, 19, 57, 70, 127, 249, 291,
];
/// CT Gen 7 (`PersonalInfo7.MachineMoves`), 100 bits.
const TM7: [u16; 100] = [
    526, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 141, 94, 247,
    280, 104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507,
    693, 511, 261, 512, 373, 153, 421, 371, 684, 416, 397, 694, 444, 521, 86, 360, 14, 19, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447,
    207, 214, 369, 164, 430, 433, 528, 57, 555, 267, 399, 127, 605, 590,
];
/// Donneurs contre des PCo d'USUL (`PersonalInfo7.BattlePointTutorMoves`), 67 bits.
const TUTOR7: [u16; 67] = [
    450, 343, 162, 530, 324, 442, 402, 529, 340, 67, 441, 253, 9, 7, 8, 277, 335, 414, 492, 356, 393, 334, 387, 276, 527, 196, 401, 428, 406,
    304, 231, 20, 173, 282, 235, 257, 272, 215, 366, 143, 220, 202, 409, 264, 351, 352, 380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278,
    289, 446, 285, 477, 502, 432, 710, 707, 675, 673,
];

/// Bits des donneurs de Platine / HGSS, par indice de fiche HGSS.
static TUTORS_G4: LazyLock<Vec<Vec<u8>>> =
    LazyLock::new(|| binlinker32(include_bytes!("../../data/pkhex/legality/tutors_g4.pkl")).into_iter().map(<[u8]>::to_vec).collect());

fn bit(data: &[u8], offset: usize, index: usize) -> bool {
    data.get(offset + index / 8).is_some_and(|b| b & (1 << (index % 8)) != 0)
}

fn bits_contain(data: &[u8], offset: usize, list: &[u16], mv: u16) -> bool {
    list.iter().position(|&m| m == mv).is_some_and(|i| bit(data, offset, i))
}

/// Rotom : attaque propre à chaque forme (Surchauffe, Hydrocanon, Blizzard, Lame d'Air, Tempête Verte).
fn rotom_move(form: u8) -> u16 {
    match form {
        1 => 315,
        2 => 56,
        3 => 59,
        4 => 403,
        5 => 437,
        _ => 0,
    }
}

/// Contexte d'une recherche.
pub struct LearnQuery {
    pub species: u16,
    pub form: u8,
    /// Niveau maximal pour les attaques par niveau.
    pub level: u8,
    /// Format actuel du Pokémon (pour les CS de Gen 4, à oublier avant le transfert).
    pub format: u8,
}

/// Peut-on apprendre `mv` dans le jeu `game` (hors capacités Œuf) ?
pub fn can_learn(game: Game, q: &LearnQuery, mv: u16) -> Option<LearnMethod> {
    if mv == 0 {
        return None;
    }
    let generation = game.generation();
    // Par niveau (Gen 7 : le Maître des Capacités enseigne toutes les attaques par niveau).
    let lvl = dex::levelup(game, q.species, q.form);
    let any_level = generation >= 7;
    if lvl.iter().any(|&(m, l)| m == mv && (any_level || l <= q.level)) {
        return Some(LearnMethod::LevelUp);
    }
    let Some((index, raw)) = dex::personal_raw(game, q.species, q.form) else { return None };
    match generation {
        4 => {
            if bits_contain(raw, 0x1C, &TM4, mv) {
                return Some(LearnMethod::Machine);
            }
            let hm: &[u16] = if game == Game::HGSS { &HM4_HGSS } else { &HM4_DPPT };
            if let Some(i) = hm.iter().position(|&m| m == mv) {
                // Les CS doivent être oubliées avant le Poké Transfert (sauf Anti-Brume).
                if bit(raw, 0x1C, 92 + i) && (q.format == 4 || mv == 432) {
                    return Some(LearnMethod::Machine);
                }
            }
            if game != Game::DP {
                let tutor_index = if game == Game::HGSS { index } else { dex::personal_raw(Game::HGSS, q.species, q.form).map_or(index, |(i, _)| i) };
                if let Some(bits) = TUTORS_G4.get(tutor_index) {
                    if bits_contain(bits, 0, &TUTOR4, mv) {
                        return Some(LearnMethod::Tutor);
                    }
                }
            }
            let special = match mv {
                307 => BLAST_BURN.contains(&q.species),
                308 => HYDRO_CANNON.contains(&q.species),
                338 => FRENZY_PLANT.contains(&q.species),
                434 => DRACO_METEOR.contains(&q.species),
                _ => false,
            };
            if special {
                return Some(LearnMethod::Tutor);
            }
        }
        5 => {
            if bits_contain(raw, 0x28, &TM5, mv) {
                return Some(LearnMethod::Machine);
            }
            if bits_contain(raw, 0x38, &TYPE_TUTOR[..7], mv) {
                return Some(LearnMethod::Tutor);
            }
            if game == Game::B2W2 {
                for (off, list) in [(0x3C, &TUTOR5_1[..]), (0x40, &TUTOR5_2[..]), (0x44, &TUTOR5_3[..]), (0x48, &TUTOR5_4[..])] {
                    if bits_contain(raw, off, list, mv) {
                        return Some(LearnMethod::Tutor);
                    }
                }
            }
        }
        6 => {
            let tm: &[u16] = if game == Game::ORAS { &TM6_AO } else { &TM6_XY };
            if bits_contain(raw, 0x28, tm, mv) {
                return Some(LearnMethod::Machine);
            }
            let types = if game == Game::ORAS { &TYPE_TUTOR[..] } else { &TYPE_TUTOR[..7] };
            if bits_contain(raw, 0x38, types, mv) {
                return Some(LearnMethod::Tutor);
            }
            if game == Game::ORAS {
                for (off, list) in [(0x40, &TUTOR5_1[..]), (0x44, &TUTOR5_2[..]), (0x48, &TUTOR6_3[..]), (0x4C, &TUTOR5_4[..])] {
                    if bits_contain(raw, off, list, mv) {
                        return Some(LearnMethod::Tutor);
                    }
                }
            }
        }
        _ => {
            if bits_contain(raw, 0x28, &TM7, mv) {
                return Some(LearnMethod::Machine);
            }
            if bits_contain(raw, 0x38, &TYPE_TUTOR, mv) {
                return Some(LearnMethod::Tutor);
            }
            if game == Game::USUM && bits_contain(raw, 0x3C, &TUTOR7, mv) {
                return Some(LearnMethod::Tutor);
            }
        }
    }
    special(game, q, mv).then_some(LearnMethod::Special)
}

/// Attaques propres à une espèce (Rotom, Keldeo, Meloetta, Necrozma, Pikachu…).
fn special(game: Game, q: &LearnQuery, mv: u16) -> bool {
    let generation = game.generation();
    match q.species {
        479 => generation >= 4 && game != Game::DP && mv == rotom_move(q.form),
        647 => generation >= 5 && mv == 548,       // Keldeo : Lame Ointe
        648 => generation >= 5 && mv == 547,       // Meloetta : Chant Antique
        25 | 26 => generation >= 7 && mv == 344,   // Pikachu / Raichu : Électacle (USUL)
        800 => generation >= 7 && matches!(mv, 713 | 714), // Necrozma : Choc Météore / Rayon Spectral
        384 => game == Game::ORAS && mv == 620,    // Rayquaza : Draco Ascension
        _ => false,
    }
}

/// Capacités Œuf de l'espèce de base dans un jeu (+ Électacle de Pichu, Lumi-Boule).
pub fn egg_moves(game: Game, species: u16, form: u8) -> Vec<u16> {
    let mut list = dex::egg_moves(game, species, form).to_vec();
    if species == 172 {
        list.push(344);
    }
    list
}

/// Toutes les attaques que l'espèce peut apprendre dans un jeu au niveau donné
/// (pour proposer des attaques légales).
pub fn learnable_moves(game: Game, q: &LearnQuery) -> Vec<u16> {
    let max = dex::max_move(game);
    (1..=max).filter(|&m| can_learn(game, q, m).is_some()).collect()
}

/// Les 4 dernières attaques apprises par niveau jusqu'à `level` (attaques de rencontre).
pub fn encounter_moves(game: Game, species: u16, form: u8, level: u8) -> Vec<u16> {
    let mut learned: Vec<u16> = Vec::new();
    for &(m, l) in dex::levelup(game, species, form) {
        if l <= level && m != 0 {
            learned.retain(|&x| x != m);
            learned.push(m);
        }
    }
    let start = learned.len().saturating_sub(4);
    learned[start..].to_vec()
}
