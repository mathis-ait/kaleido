//! Courbes d'expérience, natures et calcul des statistiques (formules Gen 3 et suivantes).
//!
//! Convention de Kaleido pour les tableaux de six statistiques (`[_; 6]`) :
//! **PV, Attaque, Défense, Attaque Spé., Défense Spé., Vitesse** (même ordre que
//! [`BaseStats`]). Les jeux stockent eux la Vitesse avant les attaques spéciales ;
//! la conversion est faite dans [`super::pkm`].

use serde::Serialize;

use crate::pokemon::BaseStats;

/// Courbe d'expérience, dans l'ordre des index des fiches « personal » des jeux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthRate {
    MediumFast,
    Erratic,
    Fluctuating,
    MediumSlow,
    Fast,
    Slow,
}

impl GrowthRate {
    pub const ALL: [GrowthRate; 6] =
        [GrowthRate::MediumFast, GrowthRate::Erratic, GrowthRate::Fluctuating, GrowthRate::MediumSlow, GrowthRate::Fast, GrowthRate::Slow];

    /// Index tel que stocké dans les fiches « personal » (0 = Moyenne … 5 = Lente).
    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }

    pub fn name_fr(self) -> &'static str {
        match self {
            GrowthRate::MediumFast => "Moyenne",
            GrowthRate::Erratic => "Erratique",
            GrowthRate::Fluctuating => "Fluctuante",
            GrowthRate::MediumSlow => "Parabolique",
            GrowthRate::Fast => "Rapide",
            GrowthRate::Slow => "Lente",
        }
    }
}

pub const MAX_LEVEL: u8 = 100;

/// Expérience totale nécessaire pour atteindre `level` (borné à 1–100).
pub fn exp_for_level(growth: GrowthRate, level: u8) -> u32 {
    let n = level.clamp(1, MAX_LEVEL) as i64;
    if n == 1 {
        return 0;
    }
    let n3 = n * n * n;
    let exp = match growth {
        GrowthRate::MediumFast => n3,
        GrowthRate::Erratic => match n {
            0..=50 => n3 * (100 - n) / 50,
            51..=68 => n3 * (150 - n) / 100,
            69..=98 => n3 * ((1911 - 10 * n) / 3) / 500,
            _ => n3 * (160 - n) / 100,
        },
        GrowthRate::Fluctuating => match n {
            0..=15 => n3 * ((n + 1) / 3 + 24) / 50,
            16..=36 => n3 * (n + 14) / 50,
            _ => n3 * (n / 2 + 32) / 50,
        },
        GrowthRate::MediumSlow => 6 * n3 / 5 - 15 * n * n + 100 * n - 140,
        GrowthRate::Fast => 4 * n3 / 5,
        GrowthRate::Slow => 5 * n3 / 4,
    };
    exp.max(0) as u32
}

/// Niveau correspondant à une quantité d'expérience (1 à 100).
pub fn level_from_exp(growth: GrowthRate, exp: u32) -> u8 {
    (1..=MAX_LEVEL).rev().find(|&l| exp_for_level(growth, l) <= exp).unwrap_or(1)
}

/// Noms français des 25 natures, dans l'ordre des jeux (Hardi = 0 … Bizarre = 24).
pub const NATURES_FR: [&str; 25] = [
    "Hardi", "Solo", "Brave", "Rigide", "Mauvais", "Assuré", "Docile", "Relax", "Malin", "Lâche", "Timide", "Pressé", "Sérieux", "Jovial", "Naïf",
    "Modeste", "Doux", "Discret", "Pudique", "Foufou", "Calme", "Gentil", "Malpoli", "Prudent", "Bizarre",
];

/// Nom français d'une nature (« ? » si l'index est invalide).
pub fn nature_name(nature: u8) -> &'static str {
    NATURES_FR.get(nature as usize).copied().unwrap_or("?")
}

/// Statistiques modifiées par une nature, en index Kaleido (1 = Attaque … 5 = Vitesse) :
/// `(augmentée, diminuée)`, ou `None` pour une nature neutre.
pub fn nature_modifiers(nature: u8) -> Option<(usize, usize)> {
    // Ordre des jeux pour les natures : Attaque, Défense, Vitesse, Atq. Spé., Déf. Spé.
    const GAME_TO_KALEIDO: [usize; 5] = [1, 2, 5, 3, 4];
    if nature >= 25 {
        return None;
    }
    let (up, down) = ((nature / 5) as usize, (nature % 5) as usize);
    (up != down).then(|| (GAME_TO_KALEIDO[up], GAME_TO_KALEIDO[down]))
}

/// Statistiques réelles (Gen 3+) dans l'ordre Kaleido : PV, Att, Déf, Atq Spé, Déf Spé, Vit.
///
/// Ne gère pas le cas particulier de Munja (toujours 1 PV).
pub fn calc_stats(base: &BaseStats, level: u8, ivs: [u8; 6], evs: [u8; 6], nature: u8) -> [u16; 6] {
    let bases = [base.hp, base.attack, base.defense, base.sp_attack, base.sp_defense, base.speed];
    let level = level.clamp(1, MAX_LEVEL) as u32;
    let modifiers = nature_modifiers(nature);
    let mut out = [0u16; 6];
    for i in 0..6 {
        let core = (2 * bases[i] as u32 + ivs[i].min(31) as u32 + evs[i] as u32 / 4) * level / 100;
        out[i] = if i == 0 {
            core + level + 10
        } else {
            let raw = core + 5;
            match modifiers {
                Some((up, _)) if up == i => raw * 110 / 100,
                Some((_, down)) if down == i => raw * 90 / 100,
                _ => raw,
            }
        } as u16;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn garchomp() -> BaseStats {
        BaseStats { hp: 108, attack: 130, defense: 95, sp_attack: 80, sp_defense: 85, speed: 102 }
    }

    #[test]
    fn garchomp_stats() {
        // Rigide (+Att, -Atq Spé), 31 IV partout, 252 EV en Attaque.
        let stats = calc_stats(&garchomp(), 100, [31; 6], [0, 252, 0, 0, 0, 0], 3);
        assert_eq!(stats[1], 394);
        assert_eq!(stats[3], (((160 + 31) + 5) * 90 / 100) as u16);
        let lv50 = calc_stats(&garchomp(), 50, [31; 6], [0; 6], 0);
        assert_eq!(lv50[0], 183);
    }

    #[test]
    fn nature_order() {
        assert_eq!(nature_name(0), "Hardi");
        assert_eq!(nature_name(3), "Rigide");
        assert_eq!(nature_name(24), "Bizarre");
        assert_eq!(nature_name(25), "?");
        assert_eq!(nature_modifiers(3), Some((1, 3))); // Rigide : +Att -Atq Spé
        assert_eq!(nature_modifiers(10), Some((5, 1))); // Timide : +Vit -Att
        assert_eq!(nature_modifiers(15), Some((3, 1))); // Modeste : +Atq Spé -Att
        assert_eq!(nature_modifiers(12), None); // Sérieux
    }

    #[test]
    fn exp_tables() {
        use GrowthRate::*;
        assert_eq!(exp_for_level(MediumFast, 100), 1_000_000);
        assert_eq!(exp_for_level(Erratic, 100), 600_000);
        assert_eq!(exp_for_level(Fluctuating, 100), 1_640_000);
        assert_eq!(exp_for_level(MediumSlow, 100), 1_059_860);
        assert_eq!(exp_for_level(Fast, 100), 800_000);
        assert_eq!(exp_for_level(Slow, 100), 1_250_000);
        // Valeurs connues des tables officielles.
        assert_eq!(exp_for_level(MediumSlow, 2), 9);
        assert_eq!(exp_for_level(Erratic, 2), 15);
        assert_eq!(exp_for_level(Erratic, 68), 257_834);
        assert_eq!(exp_for_level(Erratic, 69), 267_406);
        assert_eq!(exp_for_level(Fluctuating, 2), 4);
        assert_eq!(exp_for_level(Fluctuating, 15), 1_957);
        for g in GrowthRate::ALL {
            assert_eq!(exp_for_level(g, 1), 0);
            for l in 2..=100 {
                assert!(exp_for_level(g, l) > exp_for_level(g, l - 1), "{g:?} {l}");
            }
        }
    }

    #[test]
    fn level_lookup() {
        use GrowthRate::*;
        assert_eq!(level_from_exp(MediumFast, 0), 1);
        assert_eq!(level_from_exp(MediumFast, 124_999), 49);
        assert_eq!(level_from_exp(MediumFast, 125_000), 50);
        assert_eq!(level_from_exp(Slow, u32::MAX), 100);
        for g in GrowthRate::ALL {
            for l in 1..=100 {
                assert_eq!(level_from_exp(g, exp_for_level(g, l)), l);
            }
        }
        assert_eq!(GrowthRate::from_index(3), Some(MediumSlow));
        assert_eq!(GrowthRate::from_index(6), None);
    }
}
