//! Nombre de coups pour mettre K.O. et probabilité (comme le « 2HKO » de Showdown).
//!
//! Chaque jet aléatoire (16 valeurs) est équiprobable ; les coups successifs sont
//! indépendants. On convolue les distributions de dégâts, plafonnées aux PV de la
//! cible. Les coups critiques, la précision et les soins (Restes, baies) ne sont
//! pas comptés.

use serde::Serialize;

use super::Damage;

/// Au-delà, on ne cherche plus (« plus de 10 coups »).
const MAX_HITS: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KoInfo {
    /// Plus petit nombre d'utilisations qui peut mettre K.O. (0 = jamais : pas de dégâts ou plus de 10).
    pub hits: u8,
    /// Probabilité de K.O. en `hits` utilisations (0 à 1).
    pub chance: f64,
    /// Nombre d'utilisations qui met K.O. à coup sûr (`None` au-delà de 10).
    pub guaranteed: Option<u8>,
    /// Libellé français (« K.O. assuré en 2 coups »…).
    pub label: String,
}

/// Distribution des dégâts d'une utilisation : probabilité de chaque valeur 0..=cap.
fn distribution(damage: &Damage, cap: usize) -> Option<Vec<f64>> {
    match damage {
        Damage::None { .. } => None,
        Damage::Fixed { amount } => {
            if *amount == 0 {
                return None;
            }
            let mut d = vec![0.0; cap + 1];
            d[(*amount as usize).min(cap)] = 1.0;
            Some(d)
        }
        Damage::Rolls { hits } => {
            let mut acc = vec![0.0; cap + 1];
            acc[0] = 1.0;
            for rolls in hits {
                let mut one = vec![0.0; cap + 1];
                for &r in rolls {
                    one[(r as usize).min(cap)] += 1.0 / 16.0;
                }
                acc = convolve(&acc, &one, cap);
            }
            Some(acc)
        }
    }
}

fn convolve(a: &[f64], b: &[f64], cap: usize) -> Vec<f64> {
    let mut out = vec![0.0; cap + 1];
    for (i, &pa) in a.iter().enumerate() {
        if pa == 0.0 {
            continue;
        }
        for (j, &pb) in b.iter().enumerate() {
            if pb != 0.0 {
                out[(i + j).min(cap)] += pa * pb;
            }
        }
    }
    out
}

fn percent(p: f64) -> String {
    let v = (p * 1000.0).round() / 10.0;
    format!("{v}").replace('.', ",")
}

/// Calcule le nombre de coups pour mettre K.O. une cible à `hp` PV.
/// `endure` : Ceinture Force ou Fermeté avec les PV au maximum (survit au premier coup à 1 PV).
pub fn ko_info(damage: &Damage, hp: u16, endure: bool) -> KoInfo {
    let never = |label: &str| KoInfo { hits: 0, chance: 0.0, guaranteed: None, label: label.to_string() };
    let hp = hp.max(1) as usize;
    let Some(one) = distribution(damage, hp) else {
        return never("Pas de dégâts");
    };
    // Premier coup : avec Ceinture Force / Fermeté, au plus PV − 1 (sauf attaques multi-coups, ignoré).
    let first = if endure && hp > 1 {
        let mut f = one.clone();
        let over = f[hp - 1..].iter().sum::<f64>();
        f.truncate(hp - 1);
        f.push(over);
        f.push(0.0);
        f
    } else {
        one.clone()
    };
    let mut acc = first;
    let (mut hits, mut chance, mut guaranteed) = (0u8, 0.0, None);
    for n in 1..=MAX_HITS {
        if n > 1 {
            acc = convolve(&acc, &one, hp);
        }
        let p = acc[hp];
        if hits == 0 && p > 1e-9 {
            hits = n;
            chance = p.min(1.0);
        }
        if p >= 1.0 - 1e-9 {
            guaranteed = Some(n);
            break;
        }
    }
    let word = |n: u8| {
        if n == 1 {
            "1 coup".to_string()
        } else {
            format!("{n} coups")
        }
    };
    let label = match (hits, guaranteed) {
        (0, _) => format!("Plus de {MAX_HITS} coups"),
        (h, Some(g)) if g == h => format!("K.O. assuré en {}", word(h)),
        (h, Some(g)) => format!("{} % de K.O. en {} ({} assuré{})", percent(chance), word(h), word(g), if g > 1 { "s" } else { "" }),
        (h, None) => format!("{} % de K.O. en {}", percent(chance), word(h)),
    };
    KoInfo { hits, chance, guaranteed, label }
}
