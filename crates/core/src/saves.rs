//! Identification des fichiers de sauvegarde (Gen 3 à 7).
//!
//! Gen 3 (128 Kio, ou 64 Kio, avec ou sans le pied d'horloge de mGBA) : reconnue à ses
//! secteurs (identifiants 0 à 13, signature 0x08012025), voir `save::gen3`.
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
    RedBlue,
    Yellow,
    GoldSilver,
    Crystal,
    RubySapphire,
    Emerald,
    FireRedLeafGreen,
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
            SaveKind::RedBlue => "Pokémon Rouge / Bleu",
            SaveKind::Yellow => "Pokémon Jaune",
            SaveKind::GoldSilver => "Pokémon Or / Argent",
            SaveKind::Crystal => "Pokémon Cristal",
            SaveKind::RubySapphire => "Pokémon Rubis / Saphir",
            SaveKind::Emerald => "Pokémon Émeraude",
            SaveKind::FireRedLeafGreen => "Pokémon Rouge Feu / Vert Feuille",
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
            SaveKind::RedBlue | SaveKind::Yellow => 1,
            SaveKind::GoldSilver | SaveKind::Crystal => 2,
            SaveKind::RubySapphire | SaveKind::Emerald | SaveKind::FireRedLeafGreen => 3,
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

// Vérifié : PKHeX Saves/Util/SaveUtil.cs (SIZE_G6XY, SIZE_G6ORAS, SIZE_G7SM, SIZE_G7USUM,
// SIZE_G4RAW = SIZE_G5RAW = 0x80000).
pub fn identify(data: &[u8]) -> Option<SaveKind> {
    if let Some(v) = crate::save::gen12_version(data) {
        return Some(v.kind());
    }
    if let Some(v) = crate::save::gen3_version(data) {
        return Some(v.kind());
    }
    match effective_size(data.len() as u64) {
        0x65600 => Some(SaveKind::XY),
        0x76000 => Some(SaveKind::OmegaRubyAlphaSapphire),
        0x6BE00 => Some(SaveKind::SunMoon),
        0x6CC00 => Some(SaveKind::UltraSunUltraMoon),
        0x80000 => Some(identify_gen4(&data[..NDS_SAVE_SIZE]).unwrap_or(SaveKind::Gen5)),
        _ => None,
    }
}

/// Date du SDK écrite dans le pied de chaque bloc Gen 4 (`SAV4.MAGIC_JAPAN_INTL`).
pub const GEN4_MAGIC_INTL: u32 = 0x2006_0623;
/// Variante des versions coréennes (`SAV4.MAGIC_KOREAN`).
pub const GEN4_MAGIC_KOREAN: u32 = 0x2007_0903;

/// Pied du bloc général : taille du bloc à `fin − 0xC`, puis date du SDK à `fin − 0x8`.
// Vérifié : PKHeX Saves/Util/SaveUtil.cs (IsValidGeneralFooter2, ordre DP → Pt → HGSS).
fn identify_gen4(data: &[u8]) -> Option<SaveKind> {
    const GENERAL_SIZES: [(usize, SaveKind); 3] =
        [(0xC100, SaveKind::DiamondPearl), (0xCF2C, SaveKind::Platinum), (0xF628, SaveKind::HeartGoldSoulSilver)];
    let rd = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    // PKHeX ne regarde que la 2e partition (la première sauvegarde du jeu y est
    // écrite) ; on accepte aussi la 1re, pour les fichiers dont la copie de secours
    // est vide.
    for partition in [0x40000, 0] {
        for (size, kind) in GENERAL_SIZES {
            let end = partition + size;
            // Correction (PKHeX) : la date du SDK doit aussi correspondre, sinon une
            // sauvegarde Gen 5 dont un mot vaut par hasard la taille passerait pour Gen 4.
            if rd(end - 0xC) == size as u32 && matches!(rd(end - 0x8), GEN4_MAGIC_INTL | GEN4_MAGIC_KOREAN) {
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
        // Taille seule : pas assez (date du SDK absente).
        assert_eq!(identify(&data), Some(SaveKind::Gen5));
        data[at + 4..at + 8].copy_from_slice(&GEN4_MAGIC_INTL.to_le_bytes());
        assert_eq!(identify(&data), Some(SaveKind::Platinum));
        data[at + 4..at + 8].copy_from_slice(&GEN4_MAGIC_KOREAN.to_le_bytes());
        assert_eq!(identify(&data), Some(SaveKind::Platinum));
    }

    #[test]
    fn blank_nds_save_is_gen5_guess() {
        let kind = identify(&vec![0; NDS_SAVE_SIZE]).unwrap();
        assert!(kind.is_guess());
    }
}
