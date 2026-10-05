//! Noms français embarqués (espèces 1-721, attaques, talents, objets) et données
//! de base des espèces, extraits de Rubis Oméga avec `kaleido export-names`.
//!
//! Sert à afficher une sauvegarde sans avoir de ROM sous la main. Les données
//! sont celles de la Gen 6 : pour les Gen 4/5 quelques statistiques diffèrent.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::pokemon::BaseStats;
use crate::save::stats::GrowthRate;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    species: Vec<String>,
    moves: Vec<String>,
    abilities: Vec<String>,
    items: Vec<String>,
    growth: Vec<u8>,
    /// Ordre du jeu : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
    base_stats: Vec<[u8; 6]>,
    /// Premier talent de chaque espèce.
    #[serde(default, rename = "abilityIds")]
    abilities_by_species: Vec<u16>,
}

static DATA: LazyLock<Raw> = LazyLock::new(|| {
    let mut raw: Raw = serde_json::from_str(include_str!("../data/noms-fr.json")).expect("noms-fr.json invalide");
    for list in [&mut raw.species, &mut raw.moves, &mut raw.abilities, &mut raw.items] {
        for name in list.iter_mut() {
            *name = clean(name);
        }
    }
    raw
});

/// Garde la forme au singulier des noms à balise de grammaire.
///
/// `{VAR:1101,00FE,PPSS}` est suivi de SS caractères propres au singulier, puis de
/// PP caractères propres au pluriel, puis de la suite commune :
/// « Eau{…0A08} Fraîchex Fraîches » → « Eau Fraîche » ; « Potion{…0100}s Max » → « Potion Max ».
fn clean(name: &str) -> String {
    let mut out = String::new();
    let mut rest = name;
    while let Some(start) = rest.find("{VAR:") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|e| start + e) else { break };
        let args: Vec<u16> = rest[start + 5..end].split(',').filter_map(|v| u16::from_str_radix(v, 16).ok()).collect();
        rest = &rest[end + 1..];
        if let [0x1101, _, lengths] = args.as_slice() {
            let (singular, plural) = ((lengths & 0xFF) as usize, (lengths >> 8) as usize);
            let mut chars = rest.char_indices().map(|(i, _)| i).chain(std::iter::once(rest.len()));
            let sing_end = chars.clone().nth(singular).unwrap_or(rest.len());
            let plural_end = chars.nth(singular + plural).unwrap_or(rest.len());
            out.push_str(&rest[..sing_end]);
            rest = &rest[plural_end..];
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn lookup(list: &[String], id: u16) -> Option<&str> {
    list.get(id as usize).map(String::as_str).filter(|s| id != 0 && !s.is_empty() && *s != "-" && *s != "???")
}

pub fn species(id: u16) -> Option<&'static str> {
    lookup(&DATA.species, id)
}

pub fn move_name(id: u16) -> Option<&'static str> {
    lookup(&DATA.moves, id)
}

pub fn ability(id: u16) -> Option<&'static str> {
    lookup(&DATA.abilities, id)
}

pub fn item(id: u16) -> Option<&'static str> {
    lookup(&DATA.items, id)
}

pub fn growth_rate(species: u16) -> Option<GrowthRate> {
    DATA.growth.get(species as usize).and_then(|&g| GrowthRate::from_index(g))
}

/// Premier talent de l'espèce (données Gen 6 ; les Gen 4/5 peuvent différer).
pub fn first_ability(species: u16) -> Option<u16> {
    DATA.abilities_by_species.get(species as usize).copied().filter(|&a| a != 0)
}

pub fn base_stats(species: u16) -> Option<BaseStats> {
    let s = DATA.base_stats.get(species as usize)?;
    (s.iter().any(|&v| v != 0)).then(|| BaseStats { hp: s[0], attack: s[1], defense: s[2], speed: s[3], sp_attack: s[4], sp_defense: s[5] })
}

/// Listes complètes, pour les menus de l'interface (index = identifiant).
pub fn all_moves() -> &'static [String] {
    &DATA.moves
}

pub fn all_items() -> &'static [String] {
    &DATA.items
}

pub fn all_species() -> &'static [String] {
    &DATA.species
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_tags() {
        assert_eq!(clean("Poké Ball{VAR:1101,00FE,0100}s"), "Poké Ball");
        assert_eq!(clean("Potion{VAR:1101,00FE,0100}s Max"), "Potion Max");
        assert_eq!(clean("Eau{VAR:1101,00FE,0A08} Fraîchex Fraîches"), "Eau Fraîche");
        assert_eq!(clean("{VAR:1101,00FE,030B}Aucun objet???"), "Aucun objet");
        assert_eq!(clean("Anti-Brûle{VAR:1101,00FE,0000}"), "Anti-Brûle");
        assert_eq!(clean("Pikachu"), "Pikachu");
    }

    #[test]
    fn embedded_names() {
        assert_eq!(species(1), Some("Bulbizarre"));
        assert_eq!(species(445), Some("Carchacrok"));
        assert_eq!(move_name(33), Some("Charge"));
        assert_eq!(item(4), Some("Poké Ball"));
        assert_eq!(species(0), None);
        let garchomp = base_stats(445).unwrap();
        assert_eq!((garchomp.attack, garchomp.speed), (130, 102));
        assert!(growth_rate(1).is_some());
    }
}
