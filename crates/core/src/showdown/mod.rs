//! Format d'équipe de Pokémon Showdown : lecture, écriture et conversion vers les
//! identifiants du jeu.
//!
//! ```text
//! Surnom (Carchacrok) (F) @ Mouchoir Choix
//! Ability: Rough Skin
//! Level: 50
//! Shiny: Yes
//! EVs: 252 Atk / 4 SpD / 252 Spe
//! Jolly Nature
//! - Earthquake
//! - Outrage
//! ```
//!
//! Format repris de Pokémon Showdown (smogon/pokemon-showdown, MIT, `sim/teams.ts`,
//! `Teams.import` / `Teams.export`) et de PKHeX (kwsch/PKHeX, GPLv3, `ShowdownSet.cs`).
//! Les noms peuvent être anglais (ceux de Showdown) ou français ; l'export produit
//! l'un ou l'autre. Les mots-clés français (« Talent », « Niveau », « Chromatique »…)
//! sont aussi compris.

pub mod smogon;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::dex::{self, Game, Lang};
use crate::save::{Gender, Pokemon};

/// Attaque Puissance Cachée.
pub const HIDDEN_POWER: u16 = 237;
const RETURN: u16 = 216;
const FRUSTRATION: u16 = 218;

/// Un Pokémon tel qu'écrit dans le texte (noms bruts, pas encore résolus).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShowdownSet {
    pub nickname: Option<String>,
    /// Espèce, avec la forme éventuelle (« Rotom-Wash », « Motisma-Lavage »).
    pub species: String,
    /// 'M' ou 'F'.
    pub gender: Option<char>,
    pub item: Option<String>,
    pub ability: Option<String>,
    pub level: Option<u8>,
    pub shiny: bool,
    pub happiness: Option<u8>,
    pub ball: Option<String>,
    pub nature: Option<String>,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit (0 si absent).
    pub evs: [u16; 6],
    /// Même ordre (31 si absent).
    pub ivs: [u8; 6],
    pub moves: Vec<String>,
    /// Type voulu pour Puissance Cachée (« Hidden Power [Fire] » ou ligne « Hidden Power: Fire »).
    pub hidden_power: Option<String>,
    /// Lignes non comprises.
    pub ignored: Vec<String>,
}

impl Default for ShowdownSet {
    fn default() -> Self {
        Self {
            nickname: None,
            species: String::new(),
            gender: None,
            item: None,
            ability: None,
            level: None,
            shiny: false,
            happiness: None,
            ball: None,
            nature: None,
            evs: [0; 6],
            ivs: [31; 6],
            moves: Vec::new(),
            hidden_power: None,
            ignored: Vec::new(),
        }
    }
}

// --- Lecture ------------------------------------------------------------------------

/// Statistique d'après son abréviation (anglaise ou française).
fn stat_index(label: &str) -> Option<usize> {
    Some(match dex::normalize_name(label).as_str() {
        "hp" | "pv" => 0,
        "atk" | "att" | "atq" | "attack" | "attaque" => 1,
        "def" | "defense" => 2,
        "spa" | "spatk" | "satk" | "atqspe" | "attspe" | "spatt" | "attaquespeciale" | "specialattack" => 3,
        "spd" | "spdef" | "sdef" | "defspe" | "defensespeciale" | "specialdefense" => 4,
        "spe" | "speed" | "vit" | "vitesse" => 5,
        _ => return None,
    })
}

/// « 252 Atk / 4 SpD / 252 Spe » → valeurs par statistique.
fn parse_stats(text: &str) -> Vec<(usize, u16)> {
    text.split('/')
        .filter_map(|part| {
            let part = part.trim();
            let (num, label) = part.split_once(char::is_whitespace)?;
            Some((stat_index(label.trim())?, num.trim().parse().ok()?))
        })
        .collect()
}

fn yes(value: &str) -> bool {
    matches!(dex::normalize_name(value).as_str(), "yes" | "oui" | "true" | "vrai" | "1")
}

/// « Hidden Power [Fire] » → (« Hidden Power », Some(« Fire »)).
fn split_hidden_power(name: &str) -> (String, Option<String>) {
    if let (Some(open), Some(close)) = (name.find('['), name.rfind(']')) {
        if open < close {
            return (name[..open].trim().to_string(), Some(name[open + 1..close].trim().to_string()));
        }
    }
    (name.trim().to_string(), None)
}

/// Première ligne : « Surnom (Espèce) (F) @ Objet ».
fn parse_header(set: &mut ShowdownSet, line: &str) {
    let (mut name, item) = match line.rfind('@') {
        Some(i) => (line[..i].trim(), Some(line[i + 1..].trim())),
        None => (line.trim(), None),
    };
    set.item = item.filter(|s| !s.is_empty()).map(str::to_string);
    for (suffix, g) in [("(M)", 'M'), ("(F)", 'F'), ("(m)", 'M'), ("(f)", 'F')] {
        if let Some(rest) = name.strip_suffix(suffix) {
            set.gender = Some(g);
            name = rest.trim_end();
            break;
        }
    }
    if name.ends_with(')') {
        if let Some(open) = name.rfind('(') {
            let nick = name[..open].trim();
            let species = name[open + 1..name.len() - 1].trim();
            if !species.is_empty() {
                set.species = species.to_string();
                set.nickname = Some(nick.to_string()).filter(|n| !n.is_empty());
                return;
            }
        }
    }
    set.species = name.to_string();
}

fn parse_line(set: &mut ShowdownSet, line: &str) {
    if let Some(m) = line.strip_prefix('-').or_else(|| line.strip_prefix('~')) {
        // « - Move A / Move B » (variantes de Smogon) : la première.
        let first = m.split(" / ").next().unwrap_or(m);
        let (name, hp) = split_hidden_power(first);
        if hp.is_some() {
            set.hidden_power = hp;
        }
        if !name.is_empty() {
            set.moves.push(name);
        }
        return;
    }
    if let Some((key, value)) = line.split_once(':') {
        let value = value.trim();
        let parsed = match dex::normalize_name(key).as_str() {
            "ability" | "talent" => {
                set.ability = Some(value.to_string());
                true
            }
            "level" | "niveau" | "niv" => {
                set.level = value.parse().ok();
                true
            }
            "shiny" | "chromatique" => {
                set.shiny = yes(value);
                true
            }
            "happiness" | "friendship" | "bonheur" | "amitie" => {
                set.happiness = value.parse::<u16>().ok().map(|v| v.min(255) as u8);
                true
            }
            "ball" | "pokeball" => {
                set.ball = Some(value.to_string());
                true
            }
            "item" | "objet" => {
                set.item = Some(value.to_string());
                true
            }
            "nature" => {
                set.nature = Some(value.to_string());
                true
            }
            "evs" | "ev" => {
                for (i, v) in parse_stats(value) {
                    set.evs[i] = v;
                }
                true
            }
            "ivs" | "iv" => {
                for (i, v) in parse_stats(value) {
                    set.ivs[i] = v.min(31) as u8;
                }
                true
            }
            "hiddenpower" | "puissancecachee" => {
                set.hidden_power = Some(value.to_string());
                true
            }
            // Mécaniques des générations suivantes : sans objet ici.
            "teratype" | "gigantamax" | "dynamaxlevel" => true,
            _ => false,
        };
        if parsed {
            return;
        }
    }
    if let Some(n) = line.strip_suffix(" Nature").or_else(|| line.strip_suffix(" nature")) {
        set.nature = Some(n.trim().to_string());
    } else if let Some(n) = line.strip_prefix("Nature ") {
        set.nature = Some(n.trim().to_string());
    } else {
        set.ignored.push(line.to_string());
    }
}

/// Lit un texte Showdown (un ou plusieurs Pokémon séparés par des lignes vides).
pub fn parse_team(text: &str) -> Vec<ShowdownSet> {
    let mut sets = Vec::new();
    let mut current: Option<ShowdownSet> = None;
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with("===") {
            if let Some(set) = current.take() {
                sets.push(set);
            }
            continue;
        }
        match current.as_mut() {
            None => {
                let mut set = ShowdownSet::default();
                parse_header(&mut set, line);
                current = Some(set);
            }
            Some(set) => parse_line(set, line),
        }
    }
    sets.extend(current);
    sets.retain(|s| !s.species.is_empty());
    sets
}

/// Langue la plus probable des noms d'une équipe.
pub fn guess_lang(sets: &[ShowdownSet]) -> Lang {
    dex::guess_lang(sets.iter().flat_map(|s| {
        let species = s.species.split('-').next().unwrap_or("");
        std::iter::once(species)
            .chain(s.moves.iter().map(String::as_str))
            .chain(s.item.as_deref())
            .chain(s.ability.as_deref())
    }))
}

// --- Écriture ------------------------------------------------------------------------

const STATS_EN: [&str; 6] = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"];
const STATS_FR: [&str; 6] = ["PV", "Att", "Déf", "Atq Spé", "Déf Spé", "Vit"];

fn stats_line(values: impl Iterator<Item = (usize, u16)>, lang: Lang) -> String {
    let labels = if lang == Lang::En { STATS_EN } else { STATS_FR };
    values.map(|(i, v)| format!("{v} {}", labels[i])).collect::<Vec<_>>().join(" / ")
}

/// Texte d'un Pokémon. Les mots-clés suivent la langue (« Ability: » / « Talent : ») ;
/// les noms sont écrits tels quels.
pub fn format_set(set: &ShowdownSet, lang: Lang) -> String {
    let en = lang == Lang::En;
    let mut out = String::new();
    match &set.nickname {
        Some(n) if !n.is_empty() && dex::normalize_name(n) != dex::normalize_name(&set.species) => {
            out.push_str(&format!("{n} ({})", set.species))
        }
        _ => out.push_str(&set.species),
    }
    if let Some(g) = set.gender {
        out.push_str(&format!(" ({g})"));
    }
    if let Some(item) = set.item.as_deref().filter(|s| !s.is_empty()) {
        out.push_str(&format!(" @ {item}"));
    }
    out.push('\n');
    let mut line = |en_key: &str, fr_key: &str, value: &str| {
        let key = if en { format!("{en_key}: ") } else { format!("{fr_key} : ") };
        out.push_str(&key);
        out.push_str(value);
        out.push('\n');
    };
    if let Some(a) = &set.ability {
        line("Ability", "Talent", a);
    }
    if let Some(l) = set.level.filter(|&l| l != 100) {
        line("Level", "Niveau", &l.to_string());
    }
    if set.shiny {
        line("Shiny", "Chromatique", if en { "Yes" } else { "Oui" });
    }
    if let Some(h) = set.happiness.filter(|&h| h != 255) {
        line("Happiness", "Bonheur", &h.to_string());
    }
    if let Some(b) = &set.ball {
        line("Pokeball", "Ball", b);
    }
    if set.evs.iter().any(|&v| v != 0) {
        let evs = stats_line(set.evs.iter().enumerate().filter(|(_, &v)| v != 0).map(|(i, &v)| (i, v)), lang);
        line("EVs", "EVs", &evs);
    }
    if set.ivs.iter().any(|&v| v != 31) {
        let ivs = stats_line(set.ivs.iter().enumerate().filter(|(_, &v)| v != 31).map(|(i, &v)| (i, v as u16)), lang);
        line("IVs", "IVs", &ivs);
    }
    if let Some(n) = &set.nature {
        if en {
            out.push_str(&format!("{n} Nature\n"));
        } else {
            out.push_str(&format!("Nature : {n}\n"));
        }
    }
    let hp_name = [dex::move_name_lang(HIDDEN_POWER, Lang::En), dex::move_name_lang(HIDDEN_POWER, Lang::Fr)].map(|n| n.map(dex::normalize_name));
    for m in &set.moves {
        match &set.hidden_power {
            Some(t) if hp_name.contains(&Some(dex::normalize_name(m))) => out.push_str(&format!("- {m} [{t}]\n")),
            _ => out.push_str(&format!("- {m}\n")),
        }
    }
    out
}

/// Texte de plusieurs Pokémon, séparés par une ligne vide.
pub fn format_team(sets: &[ShowdownSet], lang: Lang) -> String {
    sets.iter().map(|s| format_set(s, lang)).collect::<Vec<_>>().join("\n")
}

/// Nom de l'espèce avec sa forme, comme l'écrit Showdown (« Rotom-Wash ») ou en
/// français (« Motisma-Lavage »). La forme de base n'a pas de suffixe.
pub fn species_display_name(game: Game, species: u16, form: u8, lang: Lang) -> String {
    let base = dex::species_name_lang(species, lang).map_or_else(|| format!("#{species}"), str::to_string);
    let suffix: Option<String> = if lang == Lang::En {
        match (species, form) {
            (774, 0..=6) => Some("Meteor".into()),
            (774, _) => None,
            (_, 0) => None,
            (800, 1) => Some("Dusk-Mane".into()),
            (800, 2) => Some("Dawn-Wings".into()),
            (550, 1) => Some("Blue-Striped".into()),
            (649, 1..=4) => Some(["Douse", "Shock", "Burn", "Chill"][form as usize - 1].into()),
            (678, 1) => Some("F".into()),
            (718, 1 | 2) => Some("10%".into()),
            (718, 3) => None,
            (718, 4) => Some("Complete".into()),
            (658, _) => Some("Ash".into()),
            _ => dex::form_names_en(game, species).get(form as usize).map(|n| n.replace(' ', "-").replace('’', "'").replace('.', "")),
        }
    } else if form == 0 {
        None
    } else {
        dex::form_names(game, species).get(form as usize).cloned()
    };
    match suffix.filter(|s| !s.is_empty()) {
        Some(s) => format!("{base}-{s}"),
        None => base,
    }
}

/// Type de Puissance Cachée des IV (identifiant de type : 1 Combat … 16 Ténèbres).
pub fn hidden_power_type(ivs: [u8; 6]) -> u8 {
    // Ordre de la formule : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
    let order = [0, 1, 2, 5, 3, 4];
    let bits: u32 = order.iter().enumerate().map(|(k, &i)| ((ivs[i] & 1) as u32) << k).sum();
    (bits * 15 / 63) as u8 + 1
}

/// IV les plus proches donnant le type voulu à Puissance Cachée (on ne touche qu'au
/// bit de poids faible : 31 devient 30 au pire).
pub fn ivs_for_hidden_power(ivs: [u8; 6], type_id: u8) -> Option<[u8; 6]> {
    if hidden_power_type(ivs) == type_id {
        return Some(ivs);
    }
    let mut best: Option<([u8; 6], u32, u32)> = None;
    for mask in 0u32..64 {
        let mut c = ivs;
        for (i, iv) in c.iter_mut().enumerate() {
            if mask >> i & 1 != 0 {
                *iv ^= 1;
            }
        }
        if hidden_power_type(c) != type_id {
            continue;
        }
        let flips = mask.count_ones();
        let total: u32 = c.iter().map(|&v| v as u32).sum();
        if best.is_none_or(|(_, f, t)| flips < f || (flips == f && total > t)) {
            best = Some((c, flips, total));
        }
    }
    best.map(|(c, _, _)| c)
}

// --- Conversion vers le jeu -----------------------------------------------------------

/// Un Pokémon du texte converti en identifiants du jeu, avec les noms français pour
/// l'aperçu et les avertissements.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSet {
    pub species: u16,
    pub form: u8,
    pub species_name: String,
    pub form_name: Option<String>,
    pub nickname: Option<String>,
    pub gender: Option<Gender>,
    pub held_item: u16,
    pub item_name: Option<String>,
    pub ability: Option<u16>,
    /// 1, 2 ou 4 (caché).
    pub ability_number: Option<u8>,
    pub ability_name: Option<String>,
    pub level: u8,
    pub shiny: bool,
    pub friendship: Option<u8>,
    pub ball: Option<u8>,
    pub ball_name: Option<String>,
    pub nature: Option<u8>,
    pub nature_name: Option<String>,
    pub evs: [u8; 6],
    pub ivs: [u8; 6],
    pub moves: [u16; 4],
    pub move_names: Vec<String>,
    pub warnings: Vec<String>,
    /// Espèce introuvable ou absente du jeu : ce Pokémon ne peut pas être ajouté.
    pub error: Option<String>,
}

/// Formes qui n'existent qu'en combat : un Pokémon rangé garde sa forme de base.
fn stored_form(species: u16, form: u8, form_en: &str) -> Option<u8> {
    if form_en.starts_with("Mega") || form_en.starts_with("Primal") {
        return Some(0);
    }
    match (species, form) {
        (774, 0..=6) => Some(form + 7), // Minior Météore → Noyau de la même couleur
        (351, 1..) | (421, 1) | (555, 1) | (648, 1) | (681, 1) | (746, 1) | (778, 1) | (800, 3) => Some(0),
        (718, 4) => Some(3), // Zygarde Parfait → 50 % avec Rassemblement
        _ => None,
    }
}

/// Espèce et forme d'après « Rotom-Wash », « Motisma-Lavage », « Ho-Oh », « Necrozma-Dusk-Mane »…
/// Renvoie aussi un avertissement si la forme n'est pas reconnue ou n'existe qu'en combat.
pub fn resolve_species(game: Game, name: &str, prefer: Lang) -> Option<(u16, u8, Option<String>)> {
    species_form(game, name, prefer).map(|s| (s.species, s.form, s.note))
}

struct SpeciesForm {
    species: u16,
    form: u8,
    note: Option<String>,
    /// La forme était écrite explicitement (« Rotom-Wash »).
    explicit: bool,
    /// Forme de combat ramenée à la forme rangée (Méga, Blade…).
    battle: bool,
}

fn species_form(game: Game, name: &str, prefer: Lang) -> Option<SpeciesForm> {
    let name = name.trim();
    let plain = |species, form, note| Some(SpeciesForm { species, form, note, explicit: false, battle: false });
    if let Some(sp) = dex::find_species(name, prefer) {
        // Minior sans précision : Noyau rouge (la forme Météore ne se range pas).
        return plain(sp, if sp == 774 { 7 } else { 0 }, None);
    }
    for (i, _) in name.match_indices(['-', ' ']) {
        let Some(sp) = dex::find_species(&name[..i], prefer) else { continue };
        let suffix = &name[i + 1..];
        let Some(form) = dex::find_form(game, sp, suffix) else {
            let base = dex::species_name(sp).unwrap_or("?");
            return plain(sp, 0, Some(format!("Forme « {suffix} » inconnue pour {base} : forme de base utilisée")));
        };
        let form_en = dex::form_names_en(game, sp).get(form as usize).cloned().unwrap_or_default();
        if let Some(stored) = stored_form(sp, form, &form_en) {
            let base = dex::species_name(sp).unwrap_or("?");
            let why = if form_en.starts_with("Mega") || form_en.starts_with("Primal") {
                format!("La forme « {suffix} » apparaît en combat : {base} est rangé sous sa forme normale (garde l'objet qui la déclenche)")
            } else {
                format!("La forme « {suffix} » n'existe qu'en combat : {base} est rangé sous sa forme normale")
            };
            return Some(SpeciesForm { species: sp, form: stored, note: Some(why), explicit: true, battle: true });
        }
        return Some(SpeciesForm { species: sp, form, note: None, explicit: true, battle: false });
    }
    None
}

/// Type de Puissance Cachée (« Fire », « Feu », « [Fire] ») → identifiant 1-16.
fn hidden_power_type_named(name: &str, prefer: Lang) -> Option<u8> {
    dex::find_type(name.trim_matches(['[', ']', ' ']), prefer).filter(|t| (1..=16).contains(t))
}

/// « Hidden Power Fire » (écriture de Smogon) → type.
fn hidden_power_suffix(name: &str, prefer: Lang) -> Option<u8> {
    let key = dex::normalize_name(name);
    for lang in [Lang::En, Lang::Fr] {
        let hp = dex::normalize_name(dex::move_name_lang(HIDDEN_POWER, lang).unwrap_or("-"));
        if let Some(rest) = key.strip_prefix(&hp).filter(|r| !r.is_empty()) {
            return hidden_power_type_named(rest, prefer);
        }
    }
    None
}

/// Convertit un Pokémon du texte pour le jeu de la sauvegarde.
pub fn resolve(set: &ShowdownSet, game: Game, prefer: Lang) -> ResolvedSet {
    let mut w: Vec<String> = Vec::new();
    let generation = game.generation();
    let mut r = ResolvedSet {
        species: 0,
        form: 0,
        species_name: set.species.clone(),
        form_name: None,
        nickname: None,
        gender: None,
        held_item: 0,
        item_name: None,
        ability: None,
        ability_number: None,
        ability_name: None,
        level: set.level.unwrap_or(100).clamp(1, 100),
        shiny: set.shiny,
        friendship: set.happiness,
        ball: None,
        ball_name: None,
        nature: None,
        nature_name: None,
        evs: [0; 6],
        ivs: set.ivs.map(|v| v.min(31)),
        moves: [0; 4],
        move_names: Vec::new(),
        warnings: Vec::new(),
        error: None,
    };

    // Espèce et forme.
    let (mut explicit_form, mut battle_form) = (false, false);
    match species_form(game, &set.species, prefer) {
        None => r.error = Some(format!("Espèce inconnue : « {} »", set.species)),
        Some(s) if s.species > dex::max_species(game) => {
            r.error = Some(format!("{} n'existe pas dans ce jeu (Gen {generation})", dex::species_name(s.species).unwrap_or("?")));
            r.species = s.species;
        }
        Some(s) => {
            r.species = s.species;
            r.form = s.form;
            explicit_form = s.explicit;
            battle_form = s.battle;
            w.extend(s.note);
        }
    }
    if r.species != 0 {
        r.species_name = dex::species_name(r.species).unwrap_or("?").to_string();
    }
    let info = (r.error.is_none()).then(|| dex::personal(game, r.species, r.form)).flatten();

    // Surnom (10 caractères sur DS, 12 sur 3DS).
    if let Some(nick) = set.nickname.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        let max = if generation >= 6 { 12 } else { 10 };
        if nick.chars().count() > max {
            let cut: String = nick.chars().take(max).collect();
            w.push(format!("Surnom « {nick} » trop long : raccourci en « {cut} »"));
            r.nickname = Some(cut);
        } else {
            r.nickname = Some(nick.to_string());
        }
    }

    // Sexe.
    if let Some(g) = set.gender {
        let wanted = if g == 'F' { Gender::Female } else { Gender::Male };
        match info.as_ref().map(|i| i.gender_ratio) {
            Some(255) => w.push("Ce Pokémon est asexué : sexe ignoré".into()),
            Some(0) if wanted == Gender::Female => w.push("Cette espèce est toujours mâle : sexe ignoré".into()),
            Some(254) if wanted == Gender::Male => w.push("Cette espèce est toujours femelle : sexe ignoré".into()),
            _ => r.gender = Some(wanted),
        }
    }

    // Objet.
    if let Some(item) = set.item.as_deref().filter(|s| !s.trim().is_empty()) {
        let max = dex::max_item(game);
        match dex::find_item(item, prefer, max) {
            Some(id) if id <= max => {
                r.held_item = id;
                r.item_name = dex::item_name_in(game, id).map(str::to_string);
            }
            Some(_) => w.push(format!("L'objet « {item} » n'existe pas en Gen {generation} : aucun objet tenu")),
            None => w.push(format!("Objet inconnu : « {item} »")),
        }
    }

    // Talent et emplacement du talent.
    if let Some(found) = info.clone() {
        let mut abilities = found.abilities;
        let slot_of = |abilities: [u16; 3], a: u16| abilities.iter().position(|&x| x == a && a != 0).map(|i| [1u8, 2, 4][i]);
        let mut chosen = None;
        if let Some(name) = set.ability.as_deref().filter(|s| !s.trim().is_empty()) {
            match dex::find_ability(name, prefer, dex::max_ability(game)) {
                Some(a) => match slot_of(abilities, a) {
                    Some(n) => chosen = Some((a, n)),
                    None => {
                        // Talent d'une autre forme : forme rangée (Amphinobi Synergie) → on la
                        // prend si le texte n'en imposait pas ; forme de combat (Méga) → ignoré.
                        let forms_en = dex::form_names_en(game, r.species);
                        let other = (1..found.form_count.max(1)).find_map(|f| {
                            let fi = dex::personal(game, r.species, f)?;
                            let battle = stored_form(r.species, f, forms_en.get(f as usize).map_or("", String::as_str)).is_some();
                            slot_of(fi.abilities, a).map(|n| (f, fi.abilities, n, battle))
                        });
                        if let Some((f, other_abilities, n, false)) = other.filter(|_| !explicit_form) {
                            r.form = f;
                            abilities = other_abilities;
                            chosen = Some((a, n));
                        } else if battle_form || matches!(other, Some((_, _, _, true))) {
                            w.push(format!("Talent « {} » de la forme de combat : talent normal gardé", dex::ability_name(a).unwrap_or(name)));
                        } else {
                            w.push(format!(
                                "{} ne peut pas avoir le talent « {} » dans ce jeu : talent par défaut",
                                r.species_name,
                                dex::ability_name(a).unwrap_or(name)
                            ));
                        }
                    }
                },
                None => w.push(format!("Talent inconnu : « {name} »")),
            }
        }
        let (a, n) = chosen.unwrap_or((abilities[0], 1));
        if a != 0 {
            r.ability = Some(a);
            r.ability_number = Some(n);
            r.ability_name = dex::ability_name(a).map(str::to_string);
        }
    }
    if r.species != 0 {
        r.form_name = dex::form_name(game, r.species, r.form).filter(|_| r.form != 0);
    }

    // Nature.
    if let Some(n) = set.nature.as_deref().filter(|s| !s.trim().is_empty()) {
        match dex::find_nature(n, prefer) {
            Some(id) => {
                r.nature = Some(id);
                r.nature_name = dex::nature_name(id).map(str::to_string);
            }
            None => w.push(format!("Nature inconnue : « {n} »")),
        }
    }

    // EV (252 au plus chacun, 510 au total).
    for (i, &v) in set.evs.iter().enumerate() {
        if v > 252 {
            w.push(format!("EV {} : {v} ramené à 252", STATS_FR[i]));
        }
        r.evs[i] = v.min(252) as u8;
    }
    let total: u16 = r.evs.iter().map(|&v| v as u16).sum();
    if total > 510 {
        w.push(format!("Total des EV {total} > 510 : les derniers seront réduits"));
    }

    // Attaques.
    let max_move = dex::max_move(game);
    let mut hp_type = set.hidden_power.as_deref().and_then(|t| hidden_power_type_named(t, prefer));
    if set.hidden_power.is_some() && hp_type.is_none() {
        w.push(format!("Type de Puissance Cachée inconnu : « {} »", set.hidden_power.as_deref().unwrap_or("")));
    }
    let mut moves: Vec<u16> = Vec::new();
    for name in &set.moves {
        let id = match dex::find_move(name, prefer, max_move) {
            Some(id) => Some(id),
            None => match hidden_power_suffix(name, prefer) {
                Some(t) => {
                    hp_type = Some(t);
                    Some(HIDDEN_POWER)
                }
                None => None,
            },
        };
        match id {
            None => w.push(format!("Attaque inconnue : « {name} »")),
            Some(id) if id > max_move => w.push(format!("L'attaque « {name} » n'existe pas en Gen {generation}")),
            Some(id) if moves.contains(&id) => w.push(format!("Attaque en double ignorée : « {name} »")),
            Some(_) if moves.len() == 4 => w.push(format!("Plus de 4 attaques : « {name} » ignorée")),
            Some(id) => moves.push(id),
        }
    }
    for (i, &m) in moves.iter().enumerate() {
        r.moves[i] = m;
    }
    r.move_names = moves.iter().map(|&m| dex::move_name(m).unwrap_or("?").to_string()).collect();
    if let (Some(t), true) = (hp_type, moves.contains(&HIDDEN_POWER)) {
        if hidden_power_type(r.ivs) != t {
            if let Some(ivs) = ivs_for_hidden_power(r.ivs, t) {
                r.ivs = ivs;
                w.push(format!("IV ajustés pour une Puissance Cachée de type {}", dex::type_name(t).unwrap_or("?")));
            }
        }
    }
    // Puissance Cachée : le type affiché est celui des IV finaux.
    if let Some(pos) = moves.iter().position(|&m| m == HIDDEN_POWER) {
        r.move_names[pos] = format!("{} {}", dex::move_name(HIDDEN_POWER).unwrap_or("?"), dex::type_name(hidden_power_type(r.ivs)).unwrap_or("?"));
    }
    if moves.is_empty() && r.error.is_none() {
        w.push("Aucune attaque reconnue : celles apprises par niveau sont gardées".into());
    }

    // Bonheur : Retour aime un Pokémon heureux, Frustration un Pokémon malheureux.
    if r.friendship.is_none() {
        if moves.contains(&RETURN) {
            r.friendship = Some(255);
        } else if moves.contains(&FRUSTRATION) {
            r.friendship = Some(0);
        }
    }

    // Ball.
    if let Some(b) = set.ball.as_deref().filter(|s| !s.trim().is_empty()) {
        match dex::find_ball(b) {
            Some(id) if id <= dex::max_ball(game) => {
                r.ball = Some(id);
                r.ball_name = dex::ball_name(id).map(str::to_string);
            }
            Some(_) => w.push(format!("La « {b} » n'existe pas en Gen {generation}")),
            None => w.push(format!("Ball inconnue : « {b} »")),
        }
    }

    for line in &set.ignored {
        w.push(format!("Ligne ignorée : « {line} »"));
    }
    r.warnings = w;
    r
}

/// Texte Showdown d'un Pokémon de la sauvegarde, avec les noms dans la langue voulue.
pub fn set_from_pokemon(game: Game, p: &Pokemon, lang: Lang) -> ShowdownSet {
    let species = p.species();
    let form = p.form();
    let info = dex::personal(game, species, form);
    let level = info.as_ref().and_then(|i| p.level(Some(i.growth_rate))).unwrap_or(100);
    let species_name = species_display_name(game, species, form, lang);
    let nickname = Some(p.nickname()).filter(|n| p.is_nicknamed() && !n.is_empty() && dex::species_name_lang(species, Lang::Fr) != Some(n.as_str()));
    let gender = match (info.as_ref().map(|i| i.gender_ratio), p.gender()) {
        (Some(0 | 254 | 255), _) | (_, Gender::Genderless) => None,
        (_, Gender::Male) => Some('M'),
        (_, Gender::Female) => Some('F'),
    };
    let moves: Vec<u16> = p.moves().into_iter().filter(|&m| m != 0).collect();
    let ivs = p.ivs();
    ShowdownSet {
        nickname,
        species: species_name,
        gender,
        item: dex::item_name_lang(game, p.held_item(), lang).filter(|_| p.held_item() != 0).map(str::to_string),
        ability: dex::ability_name_lang(p.ability(), lang).map(str::to_string),
        level: Some(level),
        shiny: p.is_shiny(),
        happiness: Some(p.friendship()),
        ball: Some(p.ball()).filter(|&b| b != 4 && b != 0).and_then(|b| dex::ball_name_lang(b, lang)).map(str::to_string),
        nature: dex::nature_name_lang(p.nature(), lang).map(str::to_string),
        evs: p.evs().map(u16::from),
        ivs,
        hidden_power: moves.contains(&HIDDEN_POWER).then(|| dex::type_name_lang(hidden_power_type(ivs), lang).unwrap_or("?").to_string()),
        moves: moves.iter().map(|&m| dex::move_name_lang(m, lang).map_or_else(|| format!("#{m}"), str::to_string)).collect(),
        ignored: Vec::new(),
    }
}
