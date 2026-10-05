//! Noms anglais (ceux de Pokémon Showdown et de Smogon) et recherche de noms
//! tolérante, en anglais comme en français.
//!
//! Les listes anglaises viennent de PKHeX (kwsch/PKHeX, GPLv3) :
//! `Resources/text/other/en/text_{Species,Moves,Abilities,Natures,Types,Forms}_en.txt`
//! et `Resources/text/items/text_Items_en.txt`, même commit que les listes françaises
//! (voir `data/pkhex/README.md`). Les identifiants sont ceux de PKHeX : la ligne n
//! est l'objet / l'attaque / l'espèce n, dans les deux langues.
//!
//! La recherche ignore la casse, les accents, les espaces et la ponctuation :
//! « Mr. Mime », « mr mime » et « M. Mime » se valent, « Nidoran-F » = « Nidoran♀ »,
//! « Farfetch’d » = « Farfetch'd ».

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::forms::{form_names, form_names_from};
use super::text::{self, lines, BALL_ITEMS};
use super::Game;

/// Langue des noms d'un texte Showdown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    /// Anglais : langue native de Pokémon Showdown et de Smogon.
    #[default]
    En,
    Fr,
}

impl Lang {
    fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Fr,
            Lang::Fr => Lang::En,
        }
    }
}

macro_rules! en_list {
    ($name:ident, $file:literal) => {
        static $name: LazyLock<Vec<&'static str>> = LazyLock::new(|| lines(include_str!(concat!("../../data/pkhex/text/en/", $file))));
    };
}

en_list!(SPECIES_EN, "text_Species_en.txt");
en_list!(MOVES_EN, "text_Moves_en.txt");
en_list!(ABILITIES_EN, "text_Abilities_en.txt");
en_list!(ITEMS_EN, "text_Items_en.txt");
en_list!(NATURES_EN, "text_Natures_en.txt");
en_list!(TYPES_EN, "text_Types_en.txt");
en_list!(FORMS_EN, "text_Forms_en.txt");

fn get(list: &[&'static str], id: usize) -> Option<&'static str> {
    list.get(id).copied().filter(|s| id != 0 && !s.is_empty())
}

pub fn species_name_en(id: u16) -> Option<&'static str> {
    get(&SPECIES_EN, id as usize)
}

pub fn move_name_en(id: u16) -> Option<&'static str> {
    get(&MOVES_EN, id as usize)
}

pub fn ability_name_en(id: u16) -> Option<&'static str> {
    get(&ABILITIES_EN, id as usize)
}

pub fn item_name_en(id: u16) -> Option<&'static str> {
    get(&ITEMS_EN, id as usize)
}

pub fn nature_name_en(id: u8) -> Option<&'static str> {
    NATURES_EN.get(id as usize).copied().filter(|s| !s.is_empty())
}

pub fn type_name_en(id: u8) -> Option<&'static str> {
    TYPES_EN.get(id as usize).copied().filter(|s| !s.is_empty())
}

/// Noms anglais des formes de l'espèce dans le jeu (même logique que [`form_names`]).
pub fn form_names_en(game: Game, species: u16) -> Vec<String> {
    form_names_from(game, species, &FORMS_EN, &TYPES_EN)
}

// --- Noms dans une langue donnée ---------------------------------------------------

pub fn species_name_lang(id: u16, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => species_name_en(id),
        Lang::Fr => text::species_name(id).filter(|_| id != 0),
    }
}

pub fn move_name_lang(id: u16, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => move_name_en(id),
        Lang::Fr => text::move_name(id),
    }
}

pub fn ability_name_lang(id: u16, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => ability_name_en(id),
        Lang::Fr => text::ability_name(id),
    }
}

pub fn item_name_lang(game: Game, id: u16, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => item_name_en(id),
        Lang::Fr => text::item_name_in(game, id),
    }
}

pub fn nature_name_lang(id: u8, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => nature_name_en(id),
        Lang::Fr => text::nature_name(id),
    }
}

pub fn type_name_lang(id: u8, lang: Lang) -> Option<&'static str> {
    match lang {
        Lang::En => type_name_en(id),
        Lang::Fr => text::type_name(id),
    }
}

pub fn ball_name_lang(ball: u8, lang: Lang) -> Option<&'static str> {
    BALL_ITEMS.get(ball as usize).filter(|_| ball != 0).and_then(|&item| match lang {
        Lang::En => item_name_en(item),
        Lang::Fr => text::item_name(item),
    })
}

pub fn form_names_lang(game: Game, species: u16, lang: Lang) -> Vec<String> {
    match lang {
        Lang::En => form_names_en(game, species),
        Lang::Fr => form_names(game, species),
    }
}

// --- Recherche tolérante ------------------------------------------------------------

/// Retire accents, casse, espaces et ponctuation (« Électhor » → « electhor »).
/// ♂ et ♀ deviennent « m » et « f » (« Nidoran♀ » = « Nidoran-F »).
pub fn normalize_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '♂' => out.push('m'),
            '♀' => out.push('f'),
            'œ' | 'Œ' => out.push_str("oe"),
            'æ' | 'Æ' => out.push_str("ae"),
            'ß' => out.push_str("ss"),
            _ => {
                for l in c.to_lowercase() {
                    let l = fold_accent(l);
                    if l.is_alphanumeric() {
                        out.push(l);
                    }
                }
            }
        }
    }
    out
}

fn fold_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        _ => c,
    }
}

/// Index « nom normalisé → identifiants » pour les deux langues.
struct NameIndex {
    en: HashMap<String, Vec<u16>>,
    fr: HashMap<String, Vec<u16>>,
}

impl NameIndex {
    fn build(en: &[&str], fr: &[&str]) -> Self {
        let index = |list: &[&str]| {
            let mut map: HashMap<String, Vec<u16>> = HashMap::new();
            for (id, name) in list.iter().enumerate().skip(1) {
                let key = normalize_name(name);
                if !key.is_empty() {
                    map.entry(key).or_default().push(id as u16);
                }
            }
            map
        };
        Self { en: index(en), fr: index(fr) }
    }

    fn map(&self, lang: Lang) -> &HashMap<String, Vec<u16>> {
        match lang {
            Lang::En => &self.en,
            Lang::Fr => &self.fr,
        }
    }

    /// Identifiant du nom, en essayant d'abord la langue préférée. Parmi les
    /// homonymes, le premier qui ne dépasse pas `max` ; sinon le premier tout court
    /// (l'appelant signale alors qu'il n'existe pas dans ce jeu).
    fn find(&self, name: &str, prefer: Lang, max: u16) -> Option<u16> {
        let key = normalize_name(name);
        if key.is_empty() {
            return None;
        }
        let hits: Vec<&Vec<u16>> = [prefer, prefer.other()].iter().filter_map(|&l| self.map(l).get(&key)).collect();
        hits.iter().find_map(|ids| ids.iter().copied().find(|&id| id <= max)).or_else(|| hits.first().and_then(|ids| ids.first().copied()))
    }

    fn lang_of(&self, name: &str) -> (bool, bool) {
        let key = normalize_name(name);
        (self.en.contains_key(&key), self.fr.contains_key(&key))
    }
}

static SPECIES_IDX: LazyLock<NameIndex> = LazyLock::new(|| NameIndex::build(&SPECIES_EN, text::species_names()));
static MOVES_IDX: LazyLock<NameIndex> = LazyLock::new(|| NameIndex::build(&MOVES_EN, text::move_names()));
static ABILITIES_IDX: LazyLock<NameIndex> = LazyLock::new(|| NameIndex::build(&ABILITIES_EN, text::ability_names()));
static ITEMS_IDX: LazyLock<NameIndex> = LazyLock::new(|| NameIndex::build(&ITEMS_EN, text::item_names()));

pub fn find_species(name: &str, prefer: Lang) -> Option<u16> {
    SPECIES_IDX.find(name, prefer, u16::MAX)
}

pub fn find_move(name: &str, prefer: Lang, max: u16) -> Option<u16> {
    MOVES_IDX.find(name, prefer, max)
}

pub fn find_ability(name: &str, prefer: Lang, max: u16) -> Option<u16> {
    ABILITIES_IDX.find(name, prefer, max)
}

/// Anciens noms anglais encore employés par Showdown / Smogon pour les vieilles générations.
const ITEM_ALIASES: [(&str, &str); 1] = [("stick", "Leek")];

pub fn find_item(name: &str, prefer: Lang, max: u16) -> Option<u16> {
    ITEMS_IDX.find(name, prefer, max).or_else(|| {
        let key = normalize_name(name);
        let (_, current) = ITEM_ALIASES.iter().find(|(old, _)| *old == key)?;
        ITEMS_IDX.find(current, Lang::En, max)
    })
}

/// Les listes de natures et de types commencent à 0 (Hardi / Normal).
fn find_small(name: &str, prefer: Lang, en: &[&str], fr: &[&str], len: usize) -> Option<u8> {
    let key = normalize_name(name);
    if key.is_empty() {
        return None;
    }
    let pos = |list: &[&str]| list.iter().take(len).position(|n| normalize_name(n) == key);
    let (a, b) = match prefer {
        Lang::En => (en, fr),
        Lang::Fr => (fr, en),
    };
    pos(a).or_else(|| pos(b)).map(|i| i as u8)
}

pub fn find_nature(name: &str, prefer: Lang) -> Option<u8> {
    find_small(name, prefer, &NATURES_EN, text::nature_names(), 25)
}

pub fn find_type(name: &str, prefer: Lang) -> Option<u8> {
    find_small(name, prefer, &TYPES_EN, text::type_names(), 18)
}

/// Ball d'après son nom (« Ultra Ball », « Bis Ball », « Dive »…).
pub fn find_ball(name: &str) -> Option<u8> {
    let key = normalize_name(name);
    if key.is_empty() {
        return None;
    }
    (1..BALL_ITEMS.len() as u8).find(|&b| {
        [Lang::En, Lang::Fr].iter().filter_map(|&l| ball_name_lang(b, l)).any(|n| {
            let n = normalize_name(n);
            n == key || n == format!("{key}ball") || n == format!("{key}s")
        })
    })
}

/// Formes nommées autrement par Showdown que par PKHeX : (espèce, nom normalisé, forme).
const FORM_ALIASES: [(u16, &str, u8); 9] = [
    (550, "bluestriped", 1),
    (649, "douse", 1),
    (649, "shock", 2),
    (649, "burn", 3),
    (649, "chill", 4),
    (774, "meteor", 0),
    (800, "duskmane", 1),
    (800, "dawnwings", 2),
    (658, "ash", 1),
];

/// Numéro de forme d'après son nom (anglais ou français) pour l'espèce dans le jeu.
pub fn find_form(game: Game, species: u16, name: &str) -> Option<u8> {
    let key = normalize_name(name);
    if key.is_empty() {
        return None;
    }
    if let Some(&(_, _, form)) = FORM_ALIASES.iter().find(|(s, k, _)| *s == species && *k == key) {
        return Some(form);
    }
    for names in [form_names_en(game, species), form_names(game, species)] {
        if let Some(i) = names.iter().position(|n| normalize_name(n) == key) {
            return Some(i as u8);
        }
    }
    None
}

/// Langue la plus probable d'une liste de noms (espèces, attaques, objets, talents) :
/// on compte les noms qui n'existent que dans une des deux langues.
pub fn guess_lang<'a>(names: impl IntoIterator<Item = &'a str>) -> Lang {
    let (mut en, mut fr) = (0, 0);
    for name in names {
        let found = [&*SPECIES_IDX, &*MOVES_IDX, &*ITEMS_IDX, &*ABILITIES_IDX].iter().map(|i| i.lang_of(name)).find(|&(e, f)| e || f);
        match found {
            Some((true, false)) => en += 1,
            Some((false, true)) => fr += 1,
            _ => {}
        }
    }
    if fr > en {
        Lang::Fr
    } else {
        Lang::En
    }
}
