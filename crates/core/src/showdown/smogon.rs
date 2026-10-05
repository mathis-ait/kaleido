//! Sets compétitifs de Smogon, au format JSON de `https://data.pkmn.cc/sets/gen{N}.json`
//! (projet pkmn/smogon, MIT ; analyses de Smogon University) :
//! espèce → format (« ou », « uu »…) → nom du set → set.
//!
//! Chaque champ peut proposer plusieurs choix (`"item": ["Leftovers", "Life Orb"]`,
//! attaques `["Earthquake", ["Stone Edge", "Rock Slide"]]`) : le premier est appliqué,
//! les autres sont montrés comme variantes. Le téléchargement et le cache sont faits
//! par l'application ; ce module ne fait que lire le JSON.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::{resolve, resolve_species, stat_index, ResolvedSet, ShowdownSet};
use crate::dex::{self, Game, Lang};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T: Clone> OneOrMany<T> {
    fn all(&self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }

    fn first(&self) -> Option<T> {
        self.all().into_iter().next()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawSet {
    #[serde(default)]
    moves: Vec<OneOrMany<String>>,
    ability: Option<OneOrMany<String>>,
    item: Option<OneOrMany<String>>,
    nature: Option<OneOrMany<String>>,
    evs: Option<OneOrMany<HashMap<String, u16>>>,
    ivs: Option<OneOrMany<HashMap<String, u16>>>,
    level: Option<u8>,
}

/// Contenu d'un fichier `gen{N}.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct SmogonData(HashMap<String, BTreeMap<String, BTreeMap<String, RawSet>>>);

/// Variantes proposées par l'analyse (noms français), en plus des choix appliqués.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetOptions {
    /// Pour chaque attaque : les autres choix possibles.
    pub moves: Vec<Vec<String>>,
    pub items: Vec<String>,
    pub abilities: Vec<String>,
    pub natures: Vec<String>,
}

/// Un set prêt à afficher et à appliquer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmogonSet {
    /// Identifiant du format (« ou », « vgc2018 »…).
    pub format: String,
    /// Libellé lisible (« OU », « VGC 2018 »…).
    pub format_label: String,
    pub name: String,
    /// Nom Smogon de l'espèce (« Landorus-Therian », « Gengar-Mega »).
    pub species_key: String,
    /// Même forme que le Pokémon demandé.
    pub same_form: bool,
    pub set: ShowdownSet,
    pub resolved: ResolvedSet,
    pub options: SetOptions,
}

/// Ordre d'affichage et libellé des formats connus. Les autres suivent, en majuscules.
const FORMATS: [(&str, &str); 26] = [
    ("ubers", "Ubers"),
    ("ou", "OU"),
    ("uu", "UU"),
    ("ru", "RU"),
    ("nu", "NU"),
    ("pu", "PU"),
    ("zu", "ZU"),
    ("lc", "LC"),
    ("nfe", "NFE"),
    ("monotype", "Monotype"),
    ("1v1", "1v1"),
    ("anythinggoes", "Anything Goes"),
    ("doublesou", "Doubles OU"),
    ("battlespotsingles", "Battle Spot Simple"),
    ("battlespotdoubles", "Battle Spot Double"),
    ("vgc2009", "VGC 2009"),
    ("vgc2010", "VGC 2010"),
    ("vgc2011", "VGC 2011"),
    ("vgc2012", "VGC 2012"),
    ("vgc2013", "VGC 2013"),
    ("vgc2014", "VGC 2014"),
    ("vgc2015", "VGC 2015"),
    ("vgc2016", "VGC 2016"),
    ("vgc2017", "VGC 2017"),
    ("vgc2018", "VGC 2018"),
    ("vgc2019", "VGC 2019"),
];

/// Formats sans intérêt pour une sauvegarde : Let's Go et Pokémon inventés (CAP).
const SKIPPED: [&str; 3] = ["letsgoou", "cap", "balancedhackmons"];

fn format_rank(format: &str) -> (usize, String) {
    let i = FORMATS.iter().position(|(f, _)| *f == format).unwrap_or(FORMATS.len());
    (i, format.to_string())
}

pub fn format_label(format: &str) -> String {
    FORMATS.iter().find(|(f, _)| *f == format).map_or_else(|| format.to_uppercase(), |(_, l)| l.to_string())
}

/// Niveau par défaut du format : 5 en Little Cup, 50 en double / VGC, 100 sinon.
fn default_level(format: &str) -> u8 {
    if format == "lc" {
        5
    } else if format.starts_with("vgc") || format.starts_with("battlespot") || format.contains("doubles") {
        50
    } else {
        100
    }
}

fn stats(map: &HashMap<String, u16>) -> Vec<(usize, u16)> {
    map.iter().filter_map(|(k, &v)| Some((stat_index(k)?, v))).collect()
}

impl RawSet {
    fn to_set(&self, species: &str, format: &str) -> ShowdownSet {
        let mut set = ShowdownSet {
            species: species.to_string(),
            moves: self.moves.iter().filter_map(OneOrMany::first).collect(),
            ability: self.ability.as_ref().and_then(OneOrMany::first),
            item: self.item.as_ref().and_then(OneOrMany::first),
            nature: self.nature.as_ref().and_then(OneOrMany::first),
            level: Some(self.level.unwrap_or_else(|| default_level(format))),
            ..ShowdownSet::default()
        };
        if let Some(evs) = self.evs.as_ref().and_then(OneOrMany::first) {
            for (i, v) in stats(&evs) {
                set.evs[i] = v;
            }
        }
        if let Some(ivs) = self.ivs.as_ref().and_then(OneOrMany::first) {
            for (i, v) in stats(&ivs) {
                set.ivs[i] = v.min(31) as u8;
            }
        }
        set
    }

    fn options(&self, game: Game) -> SetOptions {
        let rest = |v: &Option<OneOrMany<String>>, f: &dyn Fn(&str) -> String| -> Vec<String> {
            v.as_ref().map(|v| v.all().iter().skip(1).map(|n| f(n)).collect()).unwrap_or_default()
        };
        let mv = |n: &str| dex::find_move(n, Lang::En, u16::MAX).and_then(dex::move_name).map_or_else(|| n.to_string(), str::to_string);
        SetOptions {
            moves: self.moves.iter().map(|m| m.all().iter().skip(1).map(|n| mv(n)).collect()).collect(),
            items: rest(&self.item, &|n| {
                dex::find_item(n, Lang::En, dex::max_item(game))
                    .and_then(|i| dex::item_name_in(game, i))
                    .map_or_else(|| n.to_string(), str::to_string)
            }),
            abilities: rest(&self.ability, &|n| {
                dex::find_ability(n, Lang::En, u16::MAX).and_then(dex::ability_name).map_or_else(|| n.to_string(), str::to_string)
            }),
            natures: rest(&self.nature, &|n| dex::find_nature(n, Lang::En).and_then(dex::nature_name).map_or_else(|| n.to_string(), str::to_string)),
        }
    }
}

impl SmogonData {
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(json).map_err(|e| format!("données Smogon illisibles : {e}"))
    }

    /// Nombre d'espèces décrites.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Sets de l'espèce (toutes formes confondues, la forme demandée d'abord), triés par format.
    pub fn sets_for(&self, game: Game, species: u16, form: u8) -> Vec<SmogonSet> {
        let mut out: Vec<SmogonSet> = Vec::new();
        for (key, formats) in &self.0 {
            let Some((sp, f, _)) = resolve_species(game, key, Lang::En) else { continue };
            if sp != species {
                continue;
            }
            for (format, sets) in formats {
                if SKIPPED.contains(&format.as_str()) {
                    continue;
                }
                for (name, raw) in sets {
                    let set = raw.to_set(key, format);
                    out.push(SmogonSet {
                        format: format.clone(),
                        format_label: format_label(format),
                        name: name.clone(),
                        species_key: key.clone(),
                        same_form: f == form,
                        resolved: resolve(&set, game, Lang::En),
                        options: raw.options(game),
                        set,
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            (!a.same_form, format_rank(&a.format), &a.species_key, &a.name).cmp(&(!b.same_form, format_rank(&b.format), &b.species_key, &b.name))
        });
        out
    }
}
