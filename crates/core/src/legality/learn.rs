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
    264, 337, 352, 347, 46, 92, 258, 339, 331, 237, 241, 269, 58, 59, 63, 113, 182, 240, 202, 219, 218, 76, 231, 85, 87, 89, 216, 91, 94, 247, 280,
    104, 115, 351, 53, 188, 201, 126, 317, 332, 259, 263, 290, 156, 213, 168, 211, 285, 289, 315, 355, 411, 412, 206, 362, 374, 451, 203, 406, 409,
    261, 318, 373, 153, 421, 371, 278, 416, 397, 148, 444, 419, 86, 360, 14, 446, 244, 445, 399, 157, 404, 214, 363, 398, 138, 447, 207, 365, 369,
    164, 430, 433,
];
/// CS de DPPt puis de HGSS (bits 92 à 99) : Coupe, Vol, Surf, Force, Anti-Brume / Siphon, Éclate-Roc, Cascade, Escalade.
const HM4_DPPT: [u16; 8] = [15, 19, 57, 70, 432, 249, 127, 431];
const HM4_HGSS: [u16; 8] = [15, 19, 57, 70, 250, 249, 127, 431];
/// Donneurs de capacités de Platine / HGSS (`PersonalInfo4.TutorMoves`, bits de `tutors_g4.pkl`).
const TUTOR4: [u16; 52] = [
    291, 189, 210, 196, 205, 9, 7, 276, 8, 442, 401, 466, 380, 173, 180, 314, 270, 283, 200, 246, 235, 324, 428, 410, 414, 441, 239, 402, 334, 393,
    387, 340, 271, 257, 282, 389, 129, 253, 162, 220, 81, 366, 356, 388, 277, 272, 215, 67, 143, 335, 450, 29,
];
const BLAST_BURN: [u16; 4] = [6, 157, 257, 392];
const HYDRO_CANNON: [u16; 4] = [9, 160, 260, 395];
const FRENZY_PLANT: [u16; 4] = [3, 154, 254, 389];
const DRACO_METEOR: [u16; 19] = [147, 148, 149, 230, 329, 330, 334, 371, 372, 373, 380, 381, 384, 443, 444, 445, 483, 484, 487];

/// CT + CS Gen 5 (`PersonalInfo5BW.MachineMoves`), 101 bits.
const TM5: [u16; 101] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 477, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247, 280,
    104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 502, 411, 412, 206, 503, 374, 451, 507, 510, 511,
    261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 526, 398, 138, 447, 207, 365, 369,
    164, 430, 433, 528, 249, 555, 15, 19, 57, 70, 127, 291,
];
/// CT Gen 3 (`PersonalInfo3.MachineMovesTechnical`) et CS (`MachineMovesHidden`).
const TM3: [u16; 50] = [
    264, 337, 352, 347, 46, 92, 258, 339, 331, 237, 241, 269, 58, 59, 63, 113, 182, 240, 202, 219, 218, 76, 231, 85, 87, 89, 216, 91, 94, 247, 280,
    104, 115, 351, 53, 188, 201, 126, 317, 332, 259, 263, 290, 156, 213, 168, 211, 285, 289, 315,
];
const HM3: [u16; 8] = [15, 19, 57, 70, 148, 249, 127, 291];
/// Donneurs de capacités de RFVF (15 premiers) puis d'Émeraude (`LearnSource3E.Tutor_E`).
const TUTOR3: [u16; 30] = [5, 14, 25, 34, 38, 68, 69, 102, 118, 135, 138, 86, 153, 157, 164, 223, 205, 244, 173, 196, 203, 189, 8, 207, 214, 129, 111, 9, 7, 210];

/// Bits CT/CS et donneurs Gen 3 par n° national (`hmtm_g3.pkl`, `tutors_g3.pkl` : données d'Émeraude,
/// dont RFVF et RS sont des sous-ensembles, comme `PersonalTable.PopulateGen3Tutors`).
static HMTM_G3: LazyLock<Vec<Vec<u8>>> =
    LazyLock::new(|| binlinker32(include_bytes!("../../data/pkhex/legality/hmtm_g3.pkl")).into_iter().map(<[u8]>::to_vec).collect());
static TUTORS_G3: LazyLock<Vec<Vec<u8>>> =
    LazyLock::new(|| binlinker32(include_bytes!("../../data/pkhex/legality/tutors_g3.pkl")).into_iter().map(<[u8]>::to_vec).collect());

/// Attaques des CT puis des CS du jeu, dans l'ordre de leurs numéros (CT01…, puis CS01…).
pub fn machine_moves(game: Game) -> (&'static [u16], &'static [u16]) {
    match game.generation() {
        3 => (&TM3, &HM3),
        4 => (&TM4, if game == Game::HGSS { &HM4_HGSS } else { &HM4_DPPT }),
        5 => TM5.split_at(95),
        6 if game == Game::ORAS => TM6_AO.split_at(100),
        6 => TM6_XY.split_at(100),
        _ => (&TM7, &[]),
    }
}

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
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247, 280,
    104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507, 510, 511,
    261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447, 207, 214, 369,
    164, 430, 433, 528, 249, 555, 267, 399, 612, 605, 590, 15, 19, 57, 70, 127,
];
/// CT + CS de ROSA (`LearnSource6AO.MachineMoves`), 107 bits.
const TM6_AO: [u16; 107] = [
    468, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 91, 94, 247, 280,
    104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507, 510, 511,
    261, 512, 373, 153, 421, 371, 514, 416, 397, 148, 444, 521, 86, 360, 14, 522, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447, 207, 214, 369,
    164, 430, 433, 528, 290, 555, 267, 399, 612, 605, 590, 15, 19, 57, 70, 127, 249, 291,
];
/// CT Gen 7 (`PersonalInfo7.MachineMoves`), 100 bits.
const TM7: [u16; 100] = [
    526, 337, 473, 347, 46, 92, 258, 339, 474, 237, 241, 269, 58, 59, 63, 113, 182, 240, 355, 219, 218, 76, 479, 85, 87, 89, 216, 141, 94, 247, 280,
    104, 115, 482, 53, 188, 201, 126, 317, 332, 259, 263, 488, 156, 213, 168, 490, 496, 497, 315, 211, 411, 412, 206, 503, 374, 451, 507, 693, 511,
    261, 512, 373, 153, 421, 371, 684, 416, 397, 694, 444, 521, 86, 360, 14, 19, 244, 523, 524, 157, 404, 525, 611, 398, 138, 447, 207, 214, 369,
    164, 430, 433, 528, 57, 555, 267, 399, 127, 605, 590,
];
/// Donneurs contre des PCo d'USUL (`PersonalInfo7.BattlePointTutorMoves`), 67 bits.
const TUTOR7: [u16; 67] = [
    450, 343, 162, 530, 324, 442, 402, 529, 340, 67, 441, 253, 9, 7, 8, 277, 335, 414, 492, 356, 393, 334, 387, 276, 527, 196, 401, 428, 406, 304,
    231, 20, 173, 282, 235, 257, 272, 215, 366, 143, 220, 202, 409, 264, 351, 352, 380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278, 289, 446,
    285, 477, 502, 432, 710, 707, 675, 673,
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
    if generation == 3 {
        let bits = HMTM_G3.get(q.species as usize)?;
        if bits_contain(bits, 0, &TM3, mv) {
            return Some(LearnMethod::Machine);
        }
        if let Some(i) = HM3.iter().position(|&m| m == mv) {
            // Les CS doivent être oubliées avant le Parc des Amis.
            if bit(bits, 0, 50 + i) && q.format == 3 {
                return Some(LearnMethod::Machine);
            }
        }
        let tutors = match game {
            Game::E => 30,
            Game::FRLG => 15,
            _ => 0,
        };
        if TUTORS_G3.get(q.species as usize).is_some_and(|t| bits_contain(t, 0, &TUTOR3[..tutors], mv)) {
            return Some(LearnMethod::Tutor);
        }
        return None;
    }
    let (index, raw) = dex::personal_raw(game, q.species, q.form)?;
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
        647 => generation >= 5 && mv == 548,               // Keldeo : Lame Ointe
        648 => generation >= 5 && mv == 547,               // Meloetta : Chant Antique
        25 | 26 => generation >= 7 && mv == 344,           // Pikachu / Raichu : Électacle (USUL)
        800 => generation >= 7 && matches!(mv, 713 | 714), // Necrozma : Choc Météore / Rayon Spectral
        384 => game == Game::ORAS && mv == 620,            // Rayquaza : Draco Ascension
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

/// Parent qui transmet une capacité Œuf : espèce, façon dont il la connaît, et chaîne des
/// espèces intermédiaires (du bébé au parent qui l'apprend lui-même).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EggParent {
    pub species: u16,
    pub method: LearnMethod,
    /// Bébés successifs de la chaîne (le premier est le bébé demandé).
    pub chain: Vec<u16>,
}

/// Groupes Œuf « Inconnu » (15) et « Métamorph » (13) : pas de transmission d'attaque.
fn breeding_groups(game: Game, species: u16, form: u8) -> Option<([u8; 2], u8)> {
    let info = dex::personal(game, species, form)?;
    let g = info.egg_groups;
    if g.contains(&15) || g.contains(&13) {
        return None;
    }
    Some((g, info.gender_ratio))
}

type ParentKey = (u8, u16, u8, u16);
static PARENT_CACHE: LazyLock<std::sync::Mutex<std::collections::HashMap<ParentKey, Option<EggParent>>>> = LazyLock::new(Default::default);

/// Cherche une chaîne de parents pour la capacité Œuf `mv` du bébé `species` dans `game`
/// (PKHeX `EggMoveVerifier` / `LearnSource*.GetEggMoves`) : un parent d'un groupe Œuf
/// commun, capable d'être le père en Gen 4/5 (pas d'espèce toujours femelle ni asexuée),
/// qui connaît l'attaque par niveau, CT ou donneur, ou l'a lui-même reçue comme capacité
/// Œuf (on remonte alors d'une génération de reproduction, jusqu'à 4).
pub fn egg_move_parent(game: Game, species: u16, form: u8, mv: u16) -> Option<EggParent> {
    let key = (Game::ALL.iter().position(|&g| g == game).unwrap_or(0) as u8, species, form, mv);
    if let Some(hit) = PARENT_CACHE.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let found = search_parent(game, species, form, mv);
    if let Ok(mut c) = PARENT_CACHE.lock() {
        c.insert(key, found.clone());
    }
    found
}

/// (espèce, attaque) des Pokémon reçus avec une attaque imposée dans ce jeu ou les jeux
/// qui peuvent y être transférés (dons, rencontres fixes, distributions).
fn special_parents(game: Game) -> &'static [(u16, u16)] {
    static TABLES: LazyLock<Vec<Vec<(u16, u16)>>> = LazyLock::new(|| {
        Game::ALL
            .iter()
            .map(|&g| {
                let mut out: Vec<(u16, u16)> = Vec::new();
                let gens = 4..=g.generation();
                for og in Game::ALL.iter().filter(|o| gens.contains(&o.generation())) {
                    for e in super::encounters::encounters(*og) {
                        for &m in e.moves.iter().chain(&e.relearn) {
                            if !out.contains(&(e.species, m)) {
                                out.push((e.species, m));
                            }
                        }
                    }
                }
                for gen in gens {
                    for e in super::events::events(gen) {
                        for &m in e.moves.iter().chain(&e.relearn) {
                            if !out.contains(&(e.species, m)) {
                                out.push((e.species, m));
                            }
                        }
                    }
                }
                out
            })
            .collect()
    });
    let i = Game::ALL.iter().position(|&g| g == game).unwrap_or(0);
    &TABLES[i]
}

/// Groupes Œuf utilisés pour la reproduction : ceux de l'évolution pour un bébé (groupe
/// « Inconnu », Riolu, Babimanta…).
fn baby_groups(game: Game, species: u16, form: u8) -> Option<[u8; 2]> {
    if let Some((g, _)) = breeding_groups(game, species, form) {
        return Some(g);
    }
    let generation = game.generation();
    (1..=dex::max_species(game))
        .filter(|&s| s != species && super::evolution::base_of(generation, s, 0).0 == species)
        .find_map(|s| breeding_groups(game, s, 0).map(|(g, _)| g))
}

fn search_parent(game: Game, species: u16, form: u8, mv: u16) -> Option<EggParent> {
    // Électacle : Pichu né d'un Pikachu ou Raichu tenant l'Orbe Lumière.
    if species == 172 && mv == 344 {
        return Some(EggParent { species: 25, method: LearnMethod::Special, chain: vec![172] });
    }
    if !dex::egg_moves(game, species, form).contains(&mv) {
        return None;
    }
    let father_only = game.generation() <= 5;
    let max = dex::max_species(game);
    let mut visited: Vec<u16> = vec![species];
    let mut frontier: Vec<(u16, u8, Vec<u16>)> = vec![(species, form, vec![species])];
    for _depth in 0..4 {
        let mut next: Vec<(u16, u8, Vec<u16>)> = Vec::new();
        for (baby, baby_form, chain) in &frontier {
            let Some(groups) = baby_groups(game, *baby, *baby_form) else { continue };
            for s in 1..=max {
                let Some((pg, ratio)) = breeding_groups(game, s, 0) else { continue };
                if !pg.iter().any(|g| groups.contains(g)) || ratio == 255 || (father_only && ratio == 254) {
                    continue;
                }
                let forms = dex::personal(game, s, 0).map_or(1, |i| i.form_count.max(1));
                // Le parent a pu apprendre l'attaque dans un jeu plus ancien (CT, donneur) avant son transfert.
                for pg in Game::ALL.iter().filter(|g| g.generation() <= game.generation() && s <= dex::max_species(**g)) {
                    for f in 0..forms {
                        let q = LearnQuery { species: s, form: f, level: 100, format: game.generation() };
                        if let Some(method) = can_learn(*pg, &q, mv) {
                            return Some(EggParent { species: s, method, chain: chain.clone() });
                        }
                    }
                }
                // Parent reçu avec l'attaque (don, rencontre fixe, distribution : Minidraco de l'Antre Dragon…).
                if special_parents(game).iter().any(|&(sp, m)| m == mv && super::evolution::base_of(game.generation(), sp, 0).0 == super::evolution::base_of(game.generation(), s, 0).0) {
                    return Some(EggParent { species: s, method: LearnMethod::Special, chain: chain.clone() });
                }
                // Le parent a lui-même reçu l'attaque comme capacité Œuf : on remonte à son bébé.
                let base = super::evolution::base_of(game.generation(), s, 0);
                if !visited.contains(&base.0) && dex::egg_moves(game, base.0, base.1).contains(&mv) {
                    visited.push(base.0);
                    let mut c = chain.clone();
                    c.push(base.0);
                    next.push((base.0, base.1, c));
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    None
}

#[cfg(test)]
mod egg_tests {
    use super::*;

    #[test]
    fn egg_move_parent_chains() {
        // Marill (Gen 6) : Cognobidon, connu par niveau d'un parent du groupe Eau 1 / Fée.
        let p = egg_move_parent(Game::XY, 183, 0, 187).expect("Cognobidon");
        assert_ne!(p.species, 0);
        assert_eq!(p.chain[0], 183);
        // Riolu (Gen 4) : Coup Victoire n'existe pas encore → pas une capacité Œuf.
        assert!(egg_move_parent(Game::Pt, 447, 0, 526).is_none());
        // Électacle de Pichu (Orbe Lumière).
        assert_eq!(egg_move_parent(Game::Pt, 172, 0, 344).map(|p| p.method), Some(LearnMethod::Special));
        // Toutes les capacités Œuf de quelques bébés ont une chaîne de parents.
        for (game, s) in [(Game::Pt, 443u16), (Game::HGSS, 147), (Game::B2W2, 607), (Game::ORAS, 1), (Game::USUM, 722)] {
            for &m in dex::egg_moves(game, s, 0) {
                assert!(egg_move_parent(game, s, 0, m).is_some(), "{game:?} n°{s} : attaque {m} sans parent");
            }
        }
    }
}

#[cfg(test)]
mod egg_coverage {
    use super::*;

    /// Presque toutes les capacités Œuf des tables ont une chaîne de parents ; les autres
    /// (attaques d'événement transmises par un parent distribué) sont listées.
    #[test]
    fn egg_moves_have_parents() {
        let mut total = 0;
        let mut missing = Vec::new();
        for game in Game::ALL {
            for s in 1..=dex::max_species(game) {
                for &m in dex::egg_moves(game, s, 0) {
                    total += 1;
                    if egg_move_parent(game, s, 0, m).is_none() {
                        missing.push(format!("{game:?} {} : {}", dex::species_name(s).unwrap_or("?"), dex::move_name(m).unwrap_or("?")));
                    }
                }
            }
        }
        println!("{}", missing.join("\n"));
        println!("Capacités Œuf sans parent : {}/{total}", missing.len());
        assert!(missing.len() * 100 <= total, "{}/{total}", missing.len());
    }
}
