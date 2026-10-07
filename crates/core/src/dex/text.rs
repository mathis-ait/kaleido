//! Textes français de PKHeX : noms, Balls, rubans, lieux de rencontre.

use std::sync::LazyLock;

use super::Game;

/// Découpe un fichier texte PKHeX (UTF-8, une entrée par ligne, BOM éventuel).
pub(super) fn lines(text: &'static str) -> Vec<&'static str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect()
}

macro_rules! text_list {
    ($name:ident, $file:literal) => {
        static $name: LazyLock<Vec<&'static str>> = LazyLock::new(|| lines(include_str!(concat!("../../data/pkhex/text/", $file))));
    };
}

text_list!(SPECIES, "Species.txt");
text_list!(MOVES, "Moves.txt");
text_list!(ABILITIES, "Abilities.txt");
text_list!(ITEMS, "Items.txt");
text_list!(MAIL4, "Mail4.txt");
text_list!(NATURES, "Natures.txt");
text_list!(TYPES, "Types.txt");
text_list!(GAMES, "Games.txt");
text_list!(FORMS, "Forms.txt");

/// Entrée non vide d'une liste ; l'identifiant 0 (« aucun ») est exclu.
fn lookup(list: &[&'static str], id: usize) -> Option<&'static str> {
    list.get(id).copied().filter(|s| id != 0 && !s.is_empty())
}

pub(super) fn forms_list() -> &'static [&'static str] {
    &FORMS
}

/// Nom de l'espèce ; 0 donne « Œuf ».
pub fn species_name(id: u16) -> Option<&'static str> {
    SPECIES.get(id as usize).copied().filter(|s| !s.is_empty())
}

pub fn move_name(id: u16) -> Option<&'static str> {
    lookup(&MOVES, id as usize)
}

pub fn ability_name(id: u16) -> Option<&'static str> {
    lookup(&ABILITIES, id as usize)
}

/// Nom de l'objet (noms actuels, Gen 5+).
pub fn item_name(id: u16) -> Option<&'static str> {
    lookup(&ITEMS, id as usize)
}

/// Nom de l'objet tel qu'affiché dans le jeu : les Lettres 137-148 ont changé de nom après la Gen 4.
pub fn item_name_in(game: Game, id: u16) -> Option<&'static str> {
    if game.generation() == 4 && (137..137 + MAIL4.len() as u16).contains(&id) {
        let mail = MAIL4.get((id - 137) as usize).copied().filter(|s| !s.is_empty());
        return mail.or_else(|| item_name(id));
    }
    item_name(id)
}

/// Nature 0-24 (Hardi, Solo, Brave…).
pub fn nature_name(id: u8) -> Option<&'static str> {
    NATURES.get(id as usize).copied().filter(|s| !s.is_empty())
}

/// Type 0-17 dans l'ordre du jeu (Normal, Combat, Vol…).
pub fn type_name(id: u8) -> Option<&'static str> {
    TYPES.get(id as usize).copied().filter(|s| !s.is_empty())
}

/// Version d'origine (`GameVersion` de PKHeX, valeur stockée dans les Pokémon).
pub fn game_name(version: u8) -> Option<&'static str> {
    lookup(&GAMES, version as usize)
}

/// Listes complètes (index = identifiant), pour les menus de l'interface.
pub fn species_names() -> &'static [&'static str] {
    &SPECIES
}

pub fn move_names() -> &'static [&'static str] {
    &MOVES
}

pub fn ability_names() -> &'static [&'static str] {
    &ABILITIES
}

pub fn item_names() -> &'static [&'static str] {
    &ITEMS
}

pub fn nature_names() -> &'static [&'static str] {
    &NATURES[..25.min(NATURES.len())]
}

pub fn type_names() -> &'static [&'static str] {
    &TYPES
}

pub fn game_names() -> &'static [&'static str] {
    &GAMES
}

/// Objet correspondant à chaque Ball (index = valeur stockée dans le Pokémon), jusqu'à l'Ultra Ball (Gen 7).
pub(super) const BALL_ITEMS: [u16; 27] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 492, 493, 494, 495, 496, 497, 498, 499, 576, 851];

pub fn ball_name(ball: u8) -> Option<&'static str> {
    BALL_ITEMS.get(ball as usize).and_then(|&item| item_name(item))
}

/// Noms des Balls 0-26 (0 = aucune).
pub fn ball_names() -> Vec<&'static str> {
    BALL_ITEMS.iter().map(|&item| ITEMS.get(item as usize).copied().unwrap_or("")).collect()
}

static RIBBONS: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| lines(include_str!("../../data/pkhex/text/Ribbons.txt")).into_iter().filter_map(|line| line.split_once('\t')).collect());

/// Nom d'un ruban d'après sa clé PKHeX (`RibbonChampionSinnoh` → « Maître de Sinnoh »).
pub fn ribbon_name(key: &str) -> Option<&'static str> {
    RIBBONS.iter().find(|(k, _)| *k == key).map(|&(_, name)| name)
}

/// Couples (clé PKHeX, nom français) de tous les rubans.
pub fn ribbon_names() -> &'static [(&'static str, &'static str)] {
    &RIBBONS
}

// --- Fiche Pokémon : caractéristiques, terrains, Super Training, souvenirs, pays 3DS ---

text_list!(CHARACTERISTICS, "Character.txt");
text_list!(GROUND_TILES, "GroundTile.txt");
text_list!(SUPER_TRAINING, "SuperTraining.txt");
text_list!(MEMORIES, "Memories.txt");
text_list!(INTENSITIES, "Intensity.txt");
text_list!(FEELINGS6, "Feeling6.txt");
text_list!(GENERAL_LOCATIONS, "GenLoc.txt");
text_list!(CONSOLE_REGIONS, "Console3DS.txt");

/// Lignes « id\tnom » ou « pays\tid\tnom » des pays et régions 3DS (`locale3DS` de PKHeX).
fn tabbed(text: &'static str, columns: usize) -> Vec<Vec<&'static str>> {
    lines(text).into_iter().map(|l| l.splitn(columns, '\t').collect::<Vec<_>>()).filter(|c| c.len() == columns).collect()
}

static COUNTRIES: LazyLock<Vec<(u8, &'static str)>> = LazyLock::new(|| {
    tabbed(include_str!("../../data/pkhex/text/Countries3DS.txt"), 2).into_iter().filter_map(|c| Some((c[0].parse().ok()?, c[1]))).collect()
});

static REGIONS: LazyLock<Vec<(u8, u8, &'static str)>> = LazyLock::new(|| {
    tabbed(include_str!("../../data/pkhex/text/Regions3DS.txt"), 3)
        .into_iter()
        .filter_map(|c| Some((c[0].parse().ok()?, c[1].parse().ok()?, c[2])))
        .collect()
});

/// Caractéristiques 0-29 (« Il adore manger. »…), par statistique (ordre du jeu) puis IV % 5.
pub fn characteristic_names() -> &'static [&'static str] {
    &CHARACTERISTICS
}

/// Types de terrain de rencontre Gen 4 à 6 (`GroundTileType`), entrées vides = inutilisées.
pub fn ground_tile_names() -> &'static [&'static str] {
    &GROUND_TILES
}

/// Médailles du Super Training : 30 entraînements (bits 2 à 31) puis 8 distribués.
pub fn super_training_names() -> &'static [&'static str] {
    &SUPER_TRAINING
}

/// Souvenirs Gen 6/7 : phrases avec {0} Pokémon, {1} dresseur, {2} variable, {3} ressenti, {4} intensité.
pub fn memory_texts() -> &'static [&'static str] {
    &MEMORIES
}

pub fn memory_intensities() -> &'static [&'static str] {
    &INTENSITIES
}

pub fn memory_feelings() -> &'static [&'static str] {
    &FEELINGS6
}

/// Lieux génériques des souvenirs (« à la maison »…).
pub fn general_locations() -> &'static [&'static str] {
    &GENERAL_LOCATIONS
}

/// Régions de la console 3DS (0 = Japon, 1 = Amériques, 2 = Europe, 4 = Chine…).
pub fn console_region_names() -> &'static [&'static str] {
    &CONSOLE_REGIONS
}

/// Pays des 3DS (identifiant, nom français).
pub fn country_names() -> &'static [(u8, &'static str)] {
    &COUNTRIES
}

/// Régions des pays des 3DS (pays, identifiant, nom français).
pub fn region_names() -> &'static [(u8, u8, &'static str)] {
    &REGIONS
}

// --- Lieux de rencontre ---------------------------------------------------------

/// Lieux d'une génération : banques de PKHeX (0, 2000, 3000 en Gen 4 ; 0, 30000, 40000, 60000 ensuite).
struct LocationSet {
    banks: Vec<(u16, Vec<String>)>,
}

impl LocationSet {
    fn bank_mut(&mut self, base: u16) -> &mut Vec<String> {
        &mut self.banks.iter_mut().find(|(b, _)| *b == base).expect("banque de lieux").1
    }

    fn name(&self, id: u16) -> Option<&str> {
        let (base, names) = self.banks.iter().rev().find(|(base, _)| id >= *base)?;
        names.get((id - base) as usize).map(String::as_str).filter(|s| !s.is_empty() && !is_placeholder(s))
    }
}

/// Les entrées inutilisées sont des tirets pleine chasse.
fn is_placeholder(name: &str) -> bool {
    name.chars().all(|c| c == '－' || c == '-' || c == '―')
}

fn bank(text: &'static str) -> Vec<String> {
    lines(text).into_iter().map(str::to_string).collect()
}

/// Ajoute un suffixe à une entrée existante et non vide.
fn suffix(names: &mut [String], index: usize, suffix: &str) {
    if let Some(name) = names.get_mut(index).filter(|n| !n.is_empty()) {
        name.push_str(suffix);
    }
}

const NPC: &str = "PNJ";

fn egg_name() -> &'static str {
    species_name(0).unwrap_or("Œuf")
}

/// Mêmes retouches que PKHeX (`GameStrings.SanitizeMetGen*`) pour lever les doublons.
fn gen4() -> LocationSet {
    let mut set = LocationSet {
        banks: vec![
            (0, bank(include_str!("../../data/pkhex/text/met4_00000.txt"))),
            (2000, bank(include_str!("../../data/pkhex/text/met4_02000.txt"))),
            (3000, bank(include_str!("../../data/pkhex/text/met4_03000.txt"))),
        ],
    };
    let met0 = set.bank_mut(0);
    suffix(met0, 54, " (D/P/Pt)"); // Route Victoire
    suffix(met0, 221, " (HG/SS)");
    suffix(met0, 104, " (D/P/Pt)"); // Phare
    suffix(met0, 212, " (HG/SS)");
    let met2 = set.bank_mut(2000);
    suffix(met2, 1, &format!(" ({NPC})"));
    suffix(met2, 2, &format!(" ({})", egg_name()));
    set
}

fn gen5() -> LocationSet {
    let mut set = LocationSet {
        banks: vec![
            (0, bank(include_str!("../../data/pkhex/text/met5_00000.txt"))),
            (30000, bank(include_str!("../../data/pkhex/text/met5_30000.txt"))),
            (40000, bank(include_str!("../../data/pkhex/text/met5_40000.txt"))),
            (60000, bank(include_str!("../../data/pkhex/text/met5_60000.txt"))),
        ],
    };
    let met0 = set.bank_mut(0);
    if met0.len() > 84 {
        // Chambre froide de N/B = Tournoi Mondial de B2/N2.
        met0[36] = format!("{}/{}", met0[84], met0[36]);
    }
    suffix(met0, 40, " (N/B)");
    suffix(met0, 134, " (N2/B2)");
    for i in 76..106 {
        suffix(met0, i, " (N/B)"); // Heylink de N/B
    }
    suffix(met0, 2, " (-)");
    let entralink = met0.get(69).cloned();
    let met3 = set.bank_mut(30000);
    if met3.get(14).is_some() && met3.get(14) == entralink.as_ref() {
        met3[14].push_str(" (-)");
    }
    if met3.len() > 13 {
        met3[1] = "Poké Fret".to_string();
        let (celebi, zorua, zoroark) = (species_name(251).unwrap_or(""), species_name(570).unwrap_or(""), species_name(571).unwrap_or(""));
        met3[10] = format!("{celebi} ({zorua} 1)");
        met3[11] = format!("{celebi} ({zorua} 2)");
        met3[12] = format!("{zoroark} (1)");
        met3[13] = format!("{zoroark} (2)");
    }
    suffix(met3, 2, &format!(" ({NPC})"));
    suffix(met3, 3, &format!(" ({})", egg_name()));
    let met4 = set.bank_mut(40000);
    for i in 97..109 {
        suffix(met4, i, &format!(" ({})", i - 97));
    }
    suffix(set.bank_mut(60000), 3, &format!(" ({})", egg_name()));
    set
}

/// Les Gen 6 et 7 rangent les lieux par paires : le second est le sous-lieu du premier.
fn merge_sublocations(met0: &mut [String], range: impl Iterator<Item = usize>) {
    for i in range {
        let Some(next) = met0.get(i + 1).filter(|n| !n.is_empty()).cloned() else { continue };
        met0[i + 1].clear();
        if !met0[i].is_empty() {
            met0[i] = format!("{} ({next})", met0[i]);
        } else {
            met0[i] = next;
        }
    }
}

fn gen6() -> LocationSet {
    let mut set = LocationSet {
        banks: vec![
            (0, bank(include_str!("../../data/pkhex/text/met6_00000.txt"))),
            (30000, bank(include_str!("../../data/pkhex/text/met6_30000.txt"))),
            (40000, bank(include_str!("../../data/pkhex/text/met6_40000.txt"))),
            (60000, bank(include_str!("../../data/pkhex/text/met6_60000.txt"))),
        ],
    };
    let met0 = set.bank_mut(0);
    merge_sublocations(met0, (8..=136).step_by(2));
    suffix(met0, 104, " (X/Y)");
    suffix(met0, 106, " (X/Y)");
    suffix(met0, 202, " (ROSA)");
    suffix(met0, 298, " (ROSA)");
    suffix(met0, 4, " (-)");
    let met3 = set.bank_mut(30000);
    suffix(met3, 1, &format!(" ({NPC})"));
    suffix(met3, 2, &format!(" ({})", egg_name()));
    let met4 = set.bank_mut(40000);
    for i in 63..=69 {
        suffix(met4, i, &format!(" ({})", i - 62));
    }
    set
}

fn gen7() -> LocationSet {
    let mut set = LocationSet {
        banks: vec![
            (0, bank(include_str!("../../data/pkhex/text/met7_00000.txt"))),
            (30000, bank(include_str!("../../data/pkhex/text/met7_30000.txt"))),
            (40000, bank(include_str!("../../data/pkhex/text/met7_40000.txt"))),
            (60000, bank(include_str!("../../data/pkhex/text/met7_60000.txt"))),
        ],
    };
    let met0 = set.bank_mut(0);
    let len = met0.len();
    merge_sublocations(met0, (6..len).step_by(2).filter(|i| !(194..198).contains(i)));
    suffix(met0, 32, " (2)");
    suffix(met0, 102, " (2)");
    suffix(met0, 4, " (-)");
    let go = set.bank_mut(30000).get(12).cloned();
    let met4 = set.bank_mut(40000);
    if met4.get(84).is_some() && met4.get(84) == go.as_ref() {
        met4[84].push_str(" (-)");
    }
    for i in 59..66 {
        suffix(met4, i, " (-)");
    }
    let met3 = set.bank_mut(30000);
    suffix(met3, 1, &format!(" ({NPC})"));
    suffix(met3, 2, &format!(" ({})", egg_name()));
    for i in 3..=6 {
        suffix(met3, i, " (-)");
    }
    set
}

/// Gen 3 : un seul bloc ; l'identifiant est la section de carte (`mapsec`) du jeu.
fn gen3() -> LocationSet {
    LocationSet { banks: vec![(0, bank(include_str!("../../data/pkhex/text/met3_00000.txt")))] }
}

static LOCATIONS_G3: LazyLock<LocationSet> = LazyLock::new(gen3);

static LOCATIONS: LazyLock<[LocationSet; 4]> = LazyLock::new(|| [gen4(), gen5(), gen6(), gen7()]);

fn location_set(generation: u8) -> Option<&'static LocationSet> {
    if generation == 3 {
        return Some(&LOCATIONS_G3);
    }
    LOCATIONS.get(generation.checked_sub(4)? as usize)
}

/// Nom du lieu de rencontre `id` pour la génération donnée (3 à 7).
pub fn location_name(generation: u8, id: u16) -> Option<&'static str> {
    location_set(generation)?.name(id)
}

/// Identifiants proposés par PKHeX dans ses menus (`MetDataSource`, `Locations4..7`).
fn location_ids(generation: u8) -> Vec<u16> {
    let mut ids: Vec<u16> = Vec::new();
    match generation {
        3 => {
            ids.extend([0, 253, 254, 255]);
            ids.extend(1..=212);
        }
        4 => {
            ids.extend([0, 2000, 2002, 3001]);
            ids.extend(0..=234);
            ids.extend((2000..=2014).filter(|&i| i != 2007));
            ids.extend(3000..=3076);
        }
        5 => {
            ids.extend([0, 60002, 30003]);
            ids.extend((1..=153).filter(|i| ![3, 138].contains(i)));
            ids.extend([30001, 30002]);
            ids.extend((30004..=30008).chain(30010..=30015));
            ids.extend(40001..=40109);
            ids.extend([60001, 60003]);
        }
        6 => {
            ids.extend([0, 60002, 30002]);
            ids.extend((2..=354).step_by(2).filter(|i| ![4, 80].contains(i)));
            ids.extend([30001]);
            ids.extend(30003..=30011);
            ids.extend(40001..=40079);
            ids.extend([60001, 60003, 60004]);
        }
        7 => {
            ids.extend([0, 60002, 30002]);
            ids.extend((2..=232).step_by(2).filter(|i| ![80, 96, 98, 194, 196].contains(i)));
            ids.extend([30001]);
            ids.extend(30003..=30017);
            ids.extend(40001..=40088);
            ids.extend([60001, 60003, 60004]);
        }
        _ => {}
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

/// Lieux sélectionnables pour la génération (menu déroulant) : « aucun » (0) d'abord,
/// puis pension et échange, puis le reste par identifiant. L'interface peut trier par nom.
pub fn locations(generation: u8) -> Vec<(u16, &'static str)> {
    let Some(set) = location_set(generation) else { return Vec::new() };
    location_ids(generation).into_iter().filter_map(|id| if id == 0 { Some((0, "(Aucun)")) } else { set.name(id).map(|n| (id, n)) }).collect()
}
