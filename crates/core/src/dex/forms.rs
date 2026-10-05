//! Noms des formes par espèce, selon la logique de `FormConverter` de PKHeX (Gen 4 à 7).

use super::text::{forms_list, type_names};
use super::{max_species, Game};

/// Méga-évolutions à forme unique (X/Y et ROSA).
const SINGLE_MEGA: [u16; 44] = [
    3, 9, 65, 94, 115, 127, 130, 142, 181, 212, 214, 229, 248, 257, 282, 303, 306, 308, 310, 354, 359, 380, 381, 445, 448,
    460, // X/Y
    15, 18, 80, 208, 254, 260, 302, 319, 323, 334, 362, 373, 376, 384, 428, 475, 531, 719, // ROSA
];

/// Espèces ayant une forme dominante capturable en Gen 7.
const TOTEMS: [u16; 11] = [20, 105, 735, 738, 743, 752, 754, 758, 777, 778, 784];

/// Formes d'Alola (Gen 7).
const ALOLAN: [u16; 18] = [19, 20, 26, 27, 28, 37, 38, 50, 51, 52, 53, 74, 75, 76, 88, 89, 103, 105];

const MEGA: usize = 804;
const MEGA_X: usize = 805;
const MEGA_Y: usize = 806;
const ALOLA: usize = 810;
const LARGE: usize = 1007;

/// Noms des formes de l'espèce dans le jeu (index = numéro de forme).
/// Vide si l'espèce n'a pas de forme alternative.
pub fn form_names(game: Game, species: u16) -> Vec<String> {
    form_names_from(game, species, forms_list(), type_names())
}

/// Même logique avec d'autres listes de noms (formes et types), par exemple en anglais.
pub(super) fn form_names_from(game: Game, species: u16, forms: &[&str], types: &[&str]) -> Vec<String> {
    if species == 0 || species > max_species(game) {
        return Vec::new();
    }
    let f = |i: usize| forms.get(i).copied().unwrap_or("").to_string();
    let t = |i: usize| types.get(i).copied().unwrap_or("").to_string();
    let range = |a: usize, b: usize| (a..=b).map(f).collect::<Vec<_>>();
    let with = |first: String, rest: Vec<String>| std::iter::once(first).chain(rest).collect::<Vec<_>>();
    let generation = game.generation();
    let mega = generation >= 6;

    if mega && SINGLE_MEGA.contains(&species) {
        return vec![t(0), f(MEGA)];
    }
    if generation == 7 && TOTEMS.contains(&species) {
        return match species {
            778 => vec![f(778), f(1058), f(LARGE), format!("*{}", f(1058))],
            20 | 105 => vec![t(0), f(ALOLA), f(LARGE)],
            _ => vec![t(0), f(LARGE)],
        };
    }
    let arceus = |generation: u8| -> Vec<String> {
        match generation {
            4 => (0..=8).map(t).chain(std::iter::once("???".to_string())).chain((9..=16).map(t)).collect(),
            5 => (0..=16).map(t).collect(),
            _ => (0..=17).map(t).collect(),
        }
    };
    match species {
        6 | 150 if mega => vec![t(0), f(MEGA_X), f(MEGA_Y)],
        25 => match generation {
            6 => with(t(0), range(729, 734)),
            7 => with(t(0), range(813, 818).into_iter().chain([f(1063)]).collect()),
            _ => Vec::new(),
        },
        _ if generation >= 7 && ALOLAN.contains(&species) => vec![t(0), f(ALOLA)],
        172 if generation == 4 => vec![t(0), f(0)],
        201 => ('A'..='Z').map(String::from).chain(["!".to_string(), "?".to_string()]).collect(),
        351 => vec![t(0), f(889), f(890), f(891)],
        382 | 383 if mega => vec![t(0), f(899)],
        386 => vec![t(0), f(902), f(903), f(904)],
        412..=414 => vec![f(412), f(905), f(906)],
        421 => vec![f(421), f(909)],
        422 | 423 => vec![f(422), f(911)],
        479 => with(t(0), range(917, 921)),
        487 => vec![f(487), f(922)],
        492 => vec![f(492), f(923)],
        493 => arceus(generation),
        550 => vec![f(550), f(942)],
        555 => vec![f(555), f(943)],
        585 | 586 => vec![f(585), f(947), f(948), f(949)],
        641 | 642 | 645 => vec![f(641), f(952)],
        646 => vec![t(0), f(953), f(954)],
        647 => vec![f(647), f(955)],
        648 => vec![f(648), f(956)],
        649 => vec![t(0), t(10), t(12), t(9), t(14)],
        658 if mega => vec![t(0), f(962), f(1012)],
        658 => vec![t(0), f(962)],
        664..=666 => with(f(666), range(963, 981)),
        669 | 671 => vec![f(669), f(986), f(987), f(988), f(989)],
        670 if mega => vec![f(670), f(986), f(987), f(988), f(989), f(990)],
        670 => vec![f(670), f(986), f(987), f(988), f(989)],
        676 => with(f(676), range(995, 1003)),
        678 => vec!["♂".to_string(), "♀".to_string()],
        681 => vec![f(681), f(1005)],
        710 | 711 => vec![f(710), f(1006), f(1007), f(1008)],
        716 => vec![f(716), f(1012)],
        718 => vec![f(718), f(1013), format!("{}-C", f(1014)), format!("{}-C", f(1015)), f(1016)],
        720 => vec![f(720), f(1018)],
        741 => vec![f(741), f(1021), f(1022), f(1023)],
        744 => vec![t(0), f(1064)],
        745 => vec![f(745), f(1024), f(1064)],
        746 => vec![f(746), f(1025)],
        773 => arceus(7),
        774 => with(f(774), range(1045, 1057)),
        778 => vec![f(778), f(1058)],
        800 => vec![t(0), f(1065), f(1066), f(1067)],
        801 => vec![t(0), f(1062)],
        _ => Vec::new(),
    }
}

/// Nom de la forme `form` de l'espèce (`None` si l'espèce n'a pas de forme alternative).
pub fn form_name(game: Game, species: u16, form: u8) -> Option<String> {
    form_names(game, species).into_iter().nth(form as usize).filter(|n| !n.is_empty())
}
