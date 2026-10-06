//! Un jeu DS ouvert : relie la ROM brute à l'emplacement des données de chaque jeu.
//! (Les jeux 3DS sont dans [crate::ctr_rom].)

use std::path::Path;

use kaleido_formats::narc::Narc;
use kaleido_formats::nds::NdsRom;
use kaleido_formats::FormatError;

use crate::games::Game;
use crate::pokemon::{Personal, Species};
use crate::text::{gen4, gen5, TextError};

#[derive(Debug, thiserror::Error)]
pub enum RomError {
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error(transparent)]
    Text(#[from] TextError),
    #[error("{0}")]
    Unsupported(String),
    #[error("données inattendues dans la ROM : {0}")]
    Layout(String),
}

/// Où se trouvent les données dans la ROM d'un jeu.
#[derive(Debug, Clone, Copy)]
pub struct NdsLayout {
    pub text_archive: &'static str,
    pub species_names: usize,
    pub ability_names: usize,
    pub personal: &'static str,
    pub species_count: u16,
    /// Emplacements vérifiés sur une vraie ROM.
    pub verified: bool,
}

impl NdsLayout {
    pub fn for_game(game: Game) -> Option<Self> {
        use Game::*;
        let gen4 = |text_archive, species_names, ability_names, personal, verified| NdsLayout {
            text_archive,
            species_names,
            ability_names,
            personal,
            species_count: 493,
            verified,
        };
        let gen5 = |species_names, ability_names, verified| NdsLayout {
            text_archive: "a/0/0/2",
            species_names,
            ability_names,
            personal: "a/0/1/6",
            species_count: 649,
            verified,
        };
        Some(match game {
            // Diamant vérifié (ADAF) ; Perle a ses propres fiches (UPR-ZX), non vérifiées.
            Diamond => gen4("msgdata/msg.narc", 362, 552, "poketool/personal/personal.narc", true),
            Pearl => gen4("msgdata/msg.narc", 362, 552, "poketool/personal_pearl/personal.narc", false),
            Platinum => gen4("msgdata/pl_msg.narc", 412, 610, "poketool/personal/pl_personal.narc", true),
            // Vérifié sur SoulSilver (IPGF) ; HeartGold a les mêmes emplacements (UPR-ZX).
            HeartGold | SoulSilver => gen4("a/0/2/7", 237, 720, "a/0/0/2", true),
            Black | White => gen5(70, 182, true),
            Black2 | White2 => gen5(90, 374, true),
            _ => return None,
        })
    }
}

pub struct GameRom {
    pub game: Game,
    pub layout: NdsLayout,
    rom: NdsRom,
}

impl GameRom {
    pub fn open(path: &Path) -> Result<Self, RomError> {
        Self::from_rom(NdsRom::open(path)?)
    }

    pub fn from_rom(rom: NdsRom) -> Result<Self, RomError> {
        let code = &rom.header().game_code;
        let game = Game::from_nds_code(code).ok_or_else(|| RomError::Unsupported(format!("jeu DS non pris en charge ({code})")))?;
        let layout = NdsLayout::for_game(game).ok_or_else(|| RomError::Unsupported(format!("{} n'est pas un jeu DS", game.name_fr())))?;
        Ok(Self { game, layout, rom })
    }

    pub fn rom(&self) -> &NdsRom {
        &self.rom
    }

    pub fn rom_mut(&mut self) -> &mut NdsRom {
        &mut self.rom
    }

    pub fn generation(&self) -> u8 {
        self.game.generation()
    }

    pub fn narc(&self, path: &str) -> Result<Narc, RomError> {
        Ok(Narc::parse(self.rom.file_by_path(path)?)?)
    }

    pub fn replace_narc(&mut self, path: &str, narc: &Narc) -> Result<(), RomError> {
        Ok(self.rom.replace_file_by_path(path, narc.to_bytes())?)
    }

    /// Écrit la ROM complète (avec les fichiers modifiés) sur le disque.
    pub fn save(&self, path: &Path) -> Result<(), RomError> {
        let bytes = self.rom.to_bytes()?;
        std::fs::write(path, bytes).map_err(FormatError::from)?;
        Ok(())
    }

    /// Toutes les chaînes d'un fichier de l'archive de textes principale.
    pub fn text_file(&self, index: usize) -> Result<Vec<String>, RomError> {
        let narc = self.narc(self.layout.text_archive)?;
        let data = narc.files.get(index).ok_or_else(|| RomError::Layout(format!("fichier de texte n°{index} absent")))?;
        Ok(if self.generation() <= 4 { gen4::MsgFile::parse(data)?.strings() } else { gen5::MsgFile::parse(data)?.strings() })
    }

    pub fn species(&self) -> Result<Vec<Species>, RomError> {
        let names = self.text_file(self.layout.species_names)?;
        let abilities = self.text_file(self.layout.ability_names)?;
        let personal = self.narc(self.layout.personal)?;
        assemble_species(self.game, self.layout.species_count, &names, &abilities, &personal.files)
    }
}

/// Construit le Pokédex à partir des noms, des talents et des fiches « personal »
/// (indexées par numéro national, l'entrée 0 étant vide). Commun à la DS et à la 3DS.
pub(crate) fn assemble_species(
    game: Game,
    species_count: u16,
    names: &[String],
    abilities: &[String],
    personal: &[Vec<u8>],
) -> Result<Vec<Species>, RomError> {
    let count = species_count as usize;
    if names.len() <= count || personal.len() <= count || abilities.len() < 100 {
        return Err(RomError::Layout(format!(
            "{} : emplacements des données à vérifier ({} noms, {} fiches, {} talents)",
            game.name_fr(),
            names.len(),
            personal.len(),
            abilities.len()
        )));
    }
    let ability_name = |id: u16| abilities.get(id as usize).filter(|n| id != 0 && !n.is_empty() && *n != "-").cloned();

    (1..=count)
        .map(|id| {
            let p = Personal::new(game.generation(), personal[id].clone()).ok_or_else(|| RomError::Layout(format!("fiche n°{id} trop courte")))?;
            let ids = p.abilities();
            let mut regular: Vec<String> = ids.iter().take(2).filter_map(|&a| ability_name(a)).collect();
            regular.dedup();
            let hidden = ids.get(2).and_then(|&a| ability_name(a)).filter(|h| !regular.contains(h));
            let stats = p.base_stats();
            Ok(Species {
                id: id as u16,
                name: names[id].clone(),
                types: p.types().into_iter().map(Into::into).collect(),
                base_stats: stats,
                total: stats.total(),
                abilities: regular,
                hidden_ability: hidden,
                catch_rate: p.catch_rate(),
            })
        })
        .collect()
}
