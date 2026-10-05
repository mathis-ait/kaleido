//! Arbres d'évolution (`evos_g4/g5/g6/uu.pkl` de PKHeX) : pré-évolutions et
//! niveaux minimaux, pour remonter d'un Pokémon à l'espèce rencontrée.
//!
//! Format (`EvolutionSet.cs`) : table « BinLinker16 » indexée par espèce (Gen 4 à 6)
//! ou par indice de fiche (Gen 7, formes comprises) ; chaque méthode tient sur 8 octets :
//! type, (vide), argument u16, espèce u16, forme (255 = même forme), niveau.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::dex::{self, Game};

/// Une évolution vers l'avant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvoLink {
    pub from_species: u16,
    pub from_form: u8,
    pub method: u8,
    pub argument: u16,
    pub level: u8,
}

impl EvoLink {
    /// La méthode demande un passage de niveau (`EvolutionType.IsLevelUpRequired`).
    pub fn needs_level_up(&self) -> bool {
        matches!(self.method, 1..=4 | 9..=16 | 19..=34 | 36..=41)
    }

    /// Niveau minimal imposé par la méthode (0 si aucun).
    pub fn min_level(&self) -> u8 {
        if self.needs_level_up() {
            self.level
        } else {
            0
        }
    }

    pub fn is_trade(&self) -> bool {
        matches!(self.method, 5..=7)
    }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

/// Table « BinLinker16 » de PKHeX.
fn binlinker16(data: &[u8]) -> Vec<&[u8]> {
    let count = u16_at(data, 2) as usize;
    (0..count)
        .map(|i| {
            let start = u16_at(data, 4 + i * 2) as usize;
            let end = u16_at(data, 6 + i * 2) as usize;
            data.get(start..end).unwrap_or(&[])
        })
        .collect()
}

/// (espèce, forme) → pré-évolutions.
type Reverse = HashMap<(u16, u8), Vec<EvoLink>>;

/// Espèce et forme correspondant à un indice de fiche (Gen 7).
fn index_owner(game: Game) -> HashMap<usize, (u16, u8)> {
    let mut map = HashMap::new();
    for s in 1..=dex::max_species(game) {
        let Some(info) = dex::personal(game, s, 0) else { continue };
        map.insert(s as usize, (s, 0));
        if info.form_stats_index != 0 {
            for f in 1..info.form_count {
                map.insert(info.form_stats_index as usize + f as usize - 1, (s, f));
            }
        }
    }
    map
}

fn build(data: &[u8], game: Game, by_personal: bool) -> Reverse {
    let mut rev: Reverse = HashMap::new();
    let entries = binlinker16(data);
    let owners = by_personal.then(|| index_owner(game));
    for (i, entry) in entries.iter().enumerate() {
        let sources: Vec<(u16, u8)> = match &owners {
            Some(o) => o.get(&i).copied().into_iter().collect(),
            None => {
                let s = i as u16;
                if s == 0 || s > dex::max_species(game) {
                    continue;
                }
                let forms = dex::personal(game, s, 0).map_or(1, |p| p.form_count.max(1));
                (0..forms).map(|f| (s, f)).collect()
            }
        };
        for chunk in entry.as_chunks::<8>().0 {
            let species = u16_at(chunk, 4);
            if species == 0 {
                continue;
            }
            for &(from_species, from_form) in &sources {
                let to_form = if chunk[6] == 0xFF { from_form } else { chunk[6] };
                let link = EvoLink { from_species, from_form, method: chunk[0], argument: u16_at(chunk, 2), level: chunk[7] };
                let list = rev.entry((species, to_form)).or_default();
                if !list.contains(&link) {
                    list.push(link);
                }
            }
        }
    }
    rev
}

static EVOS: LazyLock<[Reverse; 4]> = LazyLock::new(|| {
    [
        build(include_bytes!("../../data/pkhex/legality/evos_g4.pkl"), Game::HGSS, false),
        build(include_bytes!("../../data/pkhex/legality/evos_g5.pkl"), Game::B2W2, false),
        build(include_bytes!("../../data/pkhex/legality/evos_g6.pkl"), Game::ORAS, false),
        build(include_bytes!("../../data/pkhex/legality/evos_uu.pkl"), Game::USUM, true),
    ]
});

fn table(generation: u8) -> &'static Reverse {
    &EVOS[(generation.clamp(4, 7) - 4) as usize]
}

/// Pré-évolutions directes de (espèce, forme) dans une génération.
pub fn pre_evolutions(generation: u8, species: u16, form: u8) -> Vec<EvoLink> {
    let t = table(generation);
    if let Some(list) = t.get(&(species, form)) {
        return list.clone();
    }
    // Forme sans entrée propre (motifs de Prismillon, formes de taille…) : on regarde la forme 0,
    // sauf pour les Pokémon dominants (formes « Totem », reçues telles quelles).
    let totem = matches!((species, form), (20, 2) | (105, 2) | (735, 1) | (738, 1) | (743, 1) | (752, 1) | (754, 1) | (758, 1) | (777, 1) | (778, 2) | (778, 3) | (784, 1));
    if form != 0 && !totem {
        if let Some(list) = t.get(&(species, 0)) {
            return list.clone();
        }
    }
    Vec::new()
}

/// Un stade de la lignée, du Pokémon actuel jusqu'à la forme de base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage {
    pub species: u16,
    pub form: u8,
    /// Niveau maximal auquel le Pokémon a pu être à ce stade.
    pub level_max: u8,
    /// Niveau minimal requis pour atteindre ce stade (évolution par niveau).
    pub level_min: u8,
}

/// Lignée du Pokémon (stade actuel en premier), d'après les tables de la génération.
pub fn chain(generation: u8, species: u16, form: u8, level: u8) -> Vec<Stage> {
    let mut out = vec![Stage { species, form, level_max: level, level_min: 1 }];
    let mut cur = (species, form);
    let mut max = level;
    for _ in 0..4 {
        let links = pre_evolutions(generation, cur.0, cur.1);
        let Some(link) = links.first().copied() else { break };
        // Niveau minimal du stade évolué.
        let need = links.iter().map(|l| l.min_level()).min().unwrap_or(0);
        if let Some(last) = out.last_mut() {
            last.level_min = last.level_min.max(need.max(1));
        }
        // Le pré-stade a évolué au plus tard au niveau actuel (avant s'il fallait monter d'un niveau).
        let lvlup = links.iter().all(|l| l.needs_level_up());
        if lvlup && max > 1 {
            max -= 1;
        }
        out.push(Stage { species: link.from_species, form: link.from_form, level_max: max, level_min: 1 });
        cur = (link.from_species, link.from_form);
    }
    out
}

/// Évolutions directes vers l'avant (pour proposer une espèce de rencontre).
pub fn evolves_into(generation: u8, species: u16, form: u8) -> Vec<(u16, u8, EvoLink)> {
    table(generation)
        .iter()
        .flat_map(|(&(s, f), links)| links.iter().filter(|l| l.from_species == species && l.from_form == form).map(move |l| (s, f, *l)))
        .collect()
}

/// Forme de base de la lignée.
pub fn base_of(generation: u8, species: u16, form: u8) -> (u16, u8) {
    let c = chain(generation, species, form, 100);
    let last = c.last().copied().unwrap_or(Stage { species, form, level_max: 100, level_min: 1 });
    (last.species, last.form)
}
