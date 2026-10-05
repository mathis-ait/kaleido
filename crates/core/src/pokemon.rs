//! Modèle de données commun : types, statistiques de base, espèces.
//!
//! Le format « personal » (fiche de chaque espèce) diffère selon la génération,
//! mais aboutit à la même structure `Species`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PokeType {
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    /// Type « ??? » (Gen 4 uniquement, attaque Malédiction).
    Mystery,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
}

impl PokeType {
    const GEN4: [PokeType; 18] = [
        PokeType::Normal,
        PokeType::Fighting,
        PokeType::Flying,
        PokeType::Poison,
        PokeType::Ground,
        PokeType::Rock,
        PokeType::Bug,
        PokeType::Ghost,
        PokeType::Steel,
        PokeType::Mystery,
        PokeType::Fire,
        PokeType::Water,
        PokeType::Grass,
        PokeType::Electric,
        PokeType::Psychic,
        PokeType::Ice,
        PokeType::Dragon,
        PokeType::Dark,
    ];

    /// Index de type tel que stocké dans les données du jeu.
    pub fn from_index(generation: u8, index: u8) -> Option<PokeType> {
        let i = index as usize;
        if generation <= 4 {
            Self::GEN4.get(i).copied()
        } else {
            // La Gen 5 supprime « ??? » : les types suivants sont décalés d'un cran.
            Self::GEN4.iter().copied().filter(|t| *t != PokeType::Mystery).nth(i)
        }
    }

    pub fn name_fr(self) -> &'static str {
        match self {
            PokeType::Normal => "Normal",
            PokeType::Fighting => "Combat",
            PokeType::Flying => "Vol",
            PokeType::Poison => "Poison",
            PokeType::Ground => "Sol",
            PokeType::Rock => "Roche",
            PokeType::Bug => "Insecte",
            PokeType::Ghost => "Spectre",
            PokeType::Steel => "Acier",
            PokeType::Mystery => "???",
            PokeType::Fire => "Feu",
            PokeType::Water => "Eau",
            PokeType::Grass => "Plante",
            PokeType::Electric => "Électrik",
            PokeType::Psychic => "Psy",
            PokeType::Ice => "Glace",
            PokeType::Dragon => "Dragon",
            PokeType::Dark => "Ténèbres",
        }
    }
}

/// Type sérialisé pour l'interface : clé stable + nom français.
#[derive(Debug, Clone, Serialize)]
pub struct TypeTag {
    pub key: PokeType,
    pub name: &'static str,
}

impl From<PokeType> for TypeTag {
    fn from(t: PokeType) -> Self {
        Self { key: t, name: t.name_fr() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseStats {
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub sp_attack: u8,
    pub sp_defense: u8,
    pub speed: u8,
}

impl BaseStats {
    pub fn total(&self) -> u16 {
        [self.hp, self.attack, self.defense, self.sp_attack, self.sp_defense, self.speed].iter().map(|&s| s as u16).sum()
    }
}

/// Fiche « personal » brute d'une espèce, avec accès aux champs selon la génération.
#[derive(Debug, Clone)]
pub struct Personal {
    pub generation: u8,
    pub data: Vec<u8>,
}

impl Personal {
    /// Taille minimale d'une fiche, par génération.
    fn min_size(generation: u8) -> usize {
        if generation <= 4 {
            0x2C
        } else {
            0x3C
        }
    }

    pub fn new(generation: u8, data: Vec<u8>) -> Option<Self> {
        (data.len() >= Self::min_size(generation)).then_some(Self { generation, data })
    }

    // Les six statistiques sont dans le même ordre en Gen 4 et 5 : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
    pub fn base_stats(&self) -> BaseStats {
        let d = &self.data;
        BaseStats { hp: d[0], attack: d[1], defense: d[2], speed: d[3], sp_attack: d[4], sp_defense: d[5] }
    }

    pub fn types(&self) -> Vec<PokeType> {
        let mut types: Vec<PokeType> = [self.data[6], self.data[7]].iter().filter_map(|&t| PokeType::from_index(self.generation, t)).collect();
        types.dedup();
        types
    }

    pub fn catch_rate(&self) -> u8 {
        self.data[8]
    }

    /// Identifiants des talents (le 3ᵉ, caché, n'existe qu'à partir de la Gen 5).
    pub fn abilities(&self) -> Vec<u16> {
        let ids: &[u8] = if self.generation <= 4 { &self.data[0x16..0x18] } else { &self.data[0x18..0x1B] };
        ids.iter().map(|&a| a as u16).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Species {
    /// Numéro du Pokédex national.
    pub id: u16,
    pub name: String,
    pub types: Vec<TypeTag>,
    pub base_stats: BaseStats,
    pub total: u16,
    /// Noms des talents, sans doublon ; le talent caché (Gen 5) est à part.
    pub abilities: Vec<String>,
    pub hidden_ability: Option<String>,
    pub catch_rate: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_indices_shift_in_gen5() {
        assert_eq!(PokeType::from_index(4, 10), Some(PokeType::Fire));
        assert_eq!(PokeType::from_index(5, 9), Some(PokeType::Fire));
        assert_eq!(PokeType::from_index(5, 16), Some(PokeType::Dark));
        assert_eq!(PokeType::from_index(5, 17), None);
    }
}
