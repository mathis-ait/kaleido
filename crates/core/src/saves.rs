//! Identification des fichiers de sauvegarde (Gen 4 à 7).
//!
//! Les sauvegardes 3DS ont une taille propre à chaque jeu. Les sauvegardes DS font
//! toutes 512 Kio ; on distingue la Gen 4 grâce au pied de bloc « général », qui
//! répète la taille du bloc (même méthode que PKHeX).

use serde::Serialize;

pub const NDS_SAVE_SIZE: usize = 0x80000;
/// Pied de page ajouté par DeSmuME aux fichiers `.dsv`.
pub const DESMUME_FOOTER: usize = 0x7A;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    DiamondPearl,
    Platinum,
    HeartGoldSoulSilver,
    /// Sauvegarde DS de 512 Kio qui n'est pas une Gen 4 : très probablement Gen 5.
    Gen5,
    XY,
    OmegaRubyAlphaSapphire,
    SunMoon,
    UltraSunUltraMoon,
}

impl SaveKind {
    pub fn label(self) -> &'static str {
        match self {
            SaveKind::DiamondPearl => "Pokémon Diamant / Perle",
            SaveKind::Platinum => "Pokémon Platine",
            SaveKind::HeartGoldSoulSilver => "Pokémon Or HeartGold / Argent SoulSilver",
            SaveKind::Gen5 => "Pokémon Noire / Blanche (1 ou 2)",
            SaveKind::XY => "Pokémon X / Y",
            SaveKind::OmegaRubyAlphaSapphire => "Pokémon Rubis Oméga / Saphir Alpha",
            SaveKind::SunMoon => "Pokémon Soleil / Lune",
            SaveKind::UltraSunUltraMoon => "Pokémon Ultra-Soleil / Ultra-Lune",
        }
    }

    pub fn generation(self) -> u8 {
        match self {
            SaveKind::DiamondPearl | SaveKind::Platinum | SaveKind::HeartGoldSoulSilver => 4,
            SaveKind::Gen5 => 5,
            SaveKind::XY | SaveKind::OmegaRubyAlphaSapphire => 6,
            SaveKind::SunMoon | SaveKind::UltraSunUltraMoon => 7,
        }
    }

    /// `true` quand la détection repose sur une hypothèse plutôt qu'une signature.
    pub fn is_guess(self) -> bool {
        self == SaveKind::Gen5
    }
}

/// Taille utile pour identifier une sauvegarde (en ignorant le pied DeSmuME).
pub fn effective_size(len: u64) -> u64 {
    if len == (NDS_SAVE_SIZE + DESMUME_FOOTER) as u64 {
        NDS_SAVE_SIZE as u64
    } else {
        len
    }
}

pub fn identify(data: &[u8]) -> Option<SaveKind> {
    match effective_size(data.len() as u64) {
        0x65600 => Some(SaveKind::XY),
        0x76000 => Some(SaveKind::OmegaRubyAlphaSapphire),
        0x6BE00 => Some(SaveKind::SunMoon),
        0x6CC00 => Some(SaveKind::UltraSunUltraMoon),
        0x80000 => Some(identify_gen4(&data[..NDS_SAVE_SIZE]).unwrap_or(SaveKind::Gen5)),
        _ => None,
    }
}

fn identify_gen4(data: &[u8]) -> Option<SaveKind> {
    const GENERAL_SIZES: [(usize, SaveKind); 3] = [
        (0xC100, SaveKind::DiamondPearl),
        (0xCF2C, SaveKind::Platinum),
        (0xF628, SaveKind::HeartGoldSoulSilver),
    ];
    // Deux copies de la sauvegarde : l'une peut être vide sur une partie récente.
    for partition in [0, 0x40000] {
        for (size, kind) in GENERAL_SIZES {
            let at = partition + size - 0xC;
            if u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) == size as u32 {
                return Some(kind);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_size() {
        assert_eq!(identify(&vec![0; 0x6CC00]), Some(SaveKind::UltraSunUltraMoon));
        assert_eq!(identify(&vec![0; 0x1234]), None);
    }

    #[test]
    fn gen4_footer() {
        let mut data = vec![0u8; NDS_SAVE_SIZE + DESMUME_FOOTER];
        let at = 0x40000 + 0xCF2C - 0xC;
        data[at..at + 4].copy_from_slice(&0xCF2Cu32.to_le_bytes());
        assert_eq!(identify(&data), Some(SaveKind::Platinum));
    }

    #[test]
    fn blank_nds_save_is_gen5_guess() {
        let kind = identify(&vec![0; NDS_SAVE_SIZE]).unwrap();
        assert!(kind.is_guess());
    }
}
