//! Évolutions : 7 entrées de 6 octets (méthode u16, paramètre u16, espèce u16),
//! identique en Gen 4 et 5.

use super::u16_at;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evolution {
    pub method: u16,
    pub param: u16,
    pub target: u16,
}

pub fn read(d: &[u8]) -> Vec<Evolution> {
    (0..7)
        .filter_map(|i| {
            let at = i * 6;
            let e = d.get(at..at + 6)?;
            let ev = Evolution { method: u16_at(e, 0), param: u16_at(e, 2), target: u16_at(e, 4) };
            (ev.method != 0 && ev.target != 0).then_some(ev)
        })
        .collect()
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
    fn stages() {
        let tables = vec![vec![], vec![Evolution { method: 4, param: 16, target: 2 }], vec![Evolution { method: 4, param: 32, target: 3 }], vec![]];
        let info = EvolutionInfo::build(&tables, 3);
        assert_eq!(&info.stage[1..], &[0, 1, 2]);
        assert_eq!(&info.evolves[1..], &[true, true, false]);
    }
}
