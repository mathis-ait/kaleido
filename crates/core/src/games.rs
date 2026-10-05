//! Jeux pris en charge et leurs identifiants (code cartouche DS, title ID 3DS).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Nds,
    #[serde(rename = "3ds")]
    N3ds,
}

impl Platform {
    pub fn label(self) -> &'static str {
        match self {
            Platform::Nds => "Nintendo DS",
            Platform::N3ds => "Nintendo 3DS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Game {
    Diamond,
    Pearl,
    Platinum,
    HeartGold,
    SoulSilver,
    Black,
    White,
    Black2,
    White2,
    X,
    Y,
    OmegaRuby,
    AlphaSapphire,
    Sun,
    Moon,
    UltraSun,
    UltraMoon,
}

impl Game {
    pub const ALL: [Game; 17] = [
        Game::Diamond,
        Game::Pearl,
        Game::Platinum,
        Game::HeartGold,
        Game::SoulSilver,
        Game::Black,
        Game::White,
        Game::Black2,
        Game::White2,
        Game::X,
        Game::Y,
        Game::OmegaRuby,
        Game::AlphaSapphire,
        Game::Sun,
        Game::Moon,
        Game::UltraSun,
        Game::UltraMoon,
    ];

    /// Nom officiel français.
    pub fn name_fr(self) -> &'static str {
        match self {
            Game::Diamond => "Pokémon Diamant",
            Game::Pearl => "Pokémon Perle",
            Game::Platinum => "Pokémon Platine",
            Game::HeartGold => "Pokémon Or HeartGold",
            Game::SoulSilver => "Pokémon Argent SoulSilver",
            Game::Black => "Pokémon Noire",
            Game::White => "Pokémon Blanche",
            Game::Black2 => "Pokémon Noire 2",
            Game::White2 => "Pokémon Blanche 2",
            Game::X => "Pokémon X",
            Game::Y => "Pokémon Y",
            Game::OmegaRuby => "Pokémon Rubis Oméga",
            Game::AlphaSapphire => "Pokémon Saphir Alpha",
            Game::Sun => "Pokémon Soleil",
            Game::Moon => "Pokémon Lune",
            Game::UltraSun => "Pokémon Ultra-Soleil",
            Game::UltraMoon => "Pokémon Ultra-Lune",
        }
    }

    pub fn generation(self) -> u8 {
        match self {
            Game::Diamond | Game::Pearl | Game::Platinum | Game::HeartGold | Game::SoulSilver => 4,
            Game::Black | Game::White | Game::Black2 | Game::White2 => 5,
            Game::X | Game::Y | Game::OmegaRuby | Game::AlphaSapphire => 6,
            Game::Sun | Game::Moon | Game::UltraSun | Game::UltraMoon => 7,
        }
    }

    pub fn platform(self) -> Platform {
        if self.generation() <= 5 {
            Platform::Nds
        } else {
            Platform::N3ds
        }
    }

    /// Les trois premiers caractères du code cartouche DS.
    pub fn nds_code(self) -> Option<&'static str> {
        Some(match self {
            Game::Diamond => "ADA",
            Game::Pearl => "APA",
            Game::Platinum => "CPU",
            Game::HeartGold => "IPK",
            Game::SoulSilver => "IPG",
            Game::Black => "IRB",
            Game::White => "IRA",
            Game::Black2 => "IRE",
            Game::White2 => "IRD",
            _ => return None,
        })
    }

    /// Title ID 3DS (identique dans toutes les régions : les jeux sont multilingues).
    pub fn title_id(self) -> Option<u64> {
        Some(match self {
            Game::X => 0x0004_0000_0005_5D00,
            Game::Y => 0x0004_0000_0005_5E00,
            Game::OmegaRuby => 0x0004_0000_0011_C400,
            Game::AlphaSapphire => 0x0004_0000_0011_C500,
            Game::Sun => 0x0004_0000_0016_4800,
            Game::Moon => 0x0004_0000_0017_5E00,
            Game::UltraSun => 0x0004_0000_001B_5000,
            Game::UltraMoon => 0x0004_0000_001B_5100,
            _ => return None,
        })
    }

    pub fn from_nds_code(code: &str) -> Option<Game> {
        let prefix = code.get(..3)?;
        Game::ALL.into_iter().find(|g| g.nds_code() == Some(prefix))
    }

    pub fn from_title_id(id: u64) -> Option<Game> {
        Game::ALL.into_iter().find(|g| g.title_id() == Some(id))
    }
}

/// Langue d'une cartouche DS d'après la dernière lettre du code jeu.
pub fn nds_language(region: char) -> Option<&'static str> {
    Some(match region {
        'F' => "Français",
        'E' | 'P' => "Anglais",
        'D' => "Allemand",
        'I' => "Italien",
        'S' => "Espagnol",
        'J' => "Japonais",
        'K' => "Coréen",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups() {
        assert_eq!(Game::from_nds_code("CPUF"), Some(Game::Platinum));
        assert_eq!(Game::from_nds_code("IRDF"), Some(Game::White2));
        assert_eq!(Game::from_nds_code("ZZZZ"), None);
        assert_eq!(Game::from_title_id(0x0004_0000_001B_5100), Some(Game::UltraMoon));
        assert_eq!(Game::UltraMoon.platform(), Platform::N3ds);
    }
}
