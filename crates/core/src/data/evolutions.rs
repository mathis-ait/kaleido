//! Évolutions : entrées de 6 octets (méthode u16, paramètre u16, espèce u16),
//! 7 en Gen 4 / 5, 8 en Gen 6.

use super::u16_at;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evolution {
    pub method: u16,
    pub param: u16,
    pub target: u16,
}

pub fn read(d: &[u8]) -> Vec<Evolution> {
    (0..(d.len() / 6).min(8))
        .filter_map(|i| {
            let at = i * 6;
            let e = d.get(at..at + 6)?;
            let ev = Evolution { method: u16_at(e, 0), param: u16_at(e, 2), target: u16_at(e, 4) };
            (ev.method != 0 && ev.target != 0).then_some(ev)
        })
        .collect()
}

/// Méthode « niveau » (paramètre = niveau requis), identique en Gen 4 à 6.
pub const METHOD_LEVEL: u16 = 4;
const METHOD_TRADE: u16 = 5;
const METHOD_TRADE_ITEM: u16 = 6;
/// Gen 5+ : échange contre une espèce précise (Carabing / Escargaume).
const METHOD_TRADE_SPECIES: u16 = 7;
/// Niveau donné aux évolutions par échange (comme l'Universal Pokémon Randomizer).
pub const TRADE_REPLACEMENT_LEVEL: u16 = 37;

/// Remplace les évolutions par échange : échange simple → niveau 37 ; échange avec
/// objet → utilisation de cet objet. Modifie `d` sur place ; renvoie le nombre de changements.
pub fn remove_trade_evolutions(generation: u8, d: &mut [u8]) -> usize {
    let use_item = if generation <= 4 { 7 } else { 8 };
    let mut changed = 0;
    for at in (0..(d.len() / 6).min(8)).map(|i| i * 6) {
        let method = u16_at(d, at);
        let new = match method {
            METHOD_TRADE => Some((METHOD_LEVEL, TRADE_REPLACEMENT_LEVEL)),
            METHOD_TRADE_ITEM => Some((use_item, u16_at(d, at + 2))),
            METHOD_TRADE_SPECIES if generation >= 5 => Some((METHOD_LEVEL, TRADE_REPLACEMENT_LEVEL)),
            _ => None,
        };
        if let Some((m, p)) = new.filter(|_| u16_at(d, at + 4) != 0) {
            d[at..at + 2].copy_from_slice(&m.to_le_bytes());
            d[at + 2..at + 4].copy_from_slice(&p.to_le_bytes());
            changed += 1;
        }
    }
    changed
}

/// Stade d'évolution de chaque espèce (0 = de base) et présence d'une évolution.
pub struct EvolutionInfo {
    pub stage: Vec<u8>,
    pub evolves: Vec<bool>,
    pub targets: Vec<Vec<u16>>,
}

impl EvolutionInfo {
    pub fn build(tables: &[Vec<Evolution>], species_count: usize) -> Self {
        let mut targets = vec![Vec::new(); species_count + 1];
        let mut has_parent = vec![false; species_count + 1];
        for (species, evos) in tables.iter().enumerate().take(species_count + 1) {
            for e in evos {
                if (e.target as usize) <= species_count {
                    targets[species].push(e.target);
                    has_parent[e.target as usize] = true;
                }
            }
        }
        let mut stage = vec![0u8; species_count + 1];
        for s in 1..=species_count {
            if !has_parent[s] {
                let mut frontier = vec![(s, 0u8)];
                while let Some((cur, depth)) = frontier.pop() {
                    stage[cur] = stage[cur].max(depth);
                    frontier.extend(targets[cur].iter().map(|&t| (t as usize, depth + 1)).filter(|&(_, d)| d < 4));
                }
            }
        }
        let evolves = targets.iter().map(|t| !t.is_empty()).collect();
        Self { stage, evolves, targets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_bulbasaur_entry() {
        let mut d = vec![0u8; 42];
        d[..6].copy_from_slice(&[4, 0, 16, 0, 2, 0]);
        assert_eq!(read(&d), vec![Evolution { method: 4, param: 16, target: 2 }]);
    }

    #[test]
    fn trade_evolutions() {
        // Kadabra (échange) et Onix (échange avec Peau Métal, objet 233) en Gen 4.
        let mut d = vec![0u8; 44];
        d[..6].copy_from_slice(&[5, 0, 0, 0, 65, 0]);
        d[6..12].copy_from_slice(&[6, 0, 233, 0, 208, 0]);
        assert_eq!(remove_trade_evolutions(4, &mut d), 2);
        let evos = read(&d);
        assert_eq!(evos[0], Evolution { method: 4, param: 37, target: 65 });
        assert_eq!(evos[1], Evolution { method: 7, param: 233, target: 208 });
    }

    #[test]
    fn stages() {
        let tables = vec![vec![], vec![Evolution { method: 4, param: 16, target: 2 }], vec![Evolution { method: 4, param: 32, target: 3 }], vec![]];
        let info = EvolutionInfo::build(&tables, 3);
        assert_eq!(&info.stage[1..], &[0, 1, 2]);
        assert_eq!(&info.evolves[1..], &[true, true, false]);
    }
}
