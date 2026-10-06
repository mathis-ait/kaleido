//! Un jeu 3DS ouvert : relie le RomFS (image ou dossier extrait) à l'emplacement
//! des données de chaque jeu, rangées dans des archives GARC (`a/x/y/z`).
//!
//! Formats relevés sur Rubis Oméga (Europe, Rev 2) ; ceux de X/Y (vérifiés sur
//! Pokémon Y Europe) sont décrits dans `randomizer/ctr_xy.rs`, `data::trainers` et
//! `data::encounters` :
//!
//! - **Dresseurs** (`trdata`, 0x18 octets) : `u16` drapeaux (bit 0 : attaques,
//!   bit 1 : objets) @0, `u16` classe @2, `u8` nombre de Pokémon @7, 4 × `u16` objets
//!   @8, `u32` IA @0x10, `u8` multiplicateur d'argent @0x15. L'octet @4 serait le type
//!   de combat (non vérifié). X/Y : 0x14 octets, drapeaux @0, classe @1, type de combat
//!   @2 et nombre @3 sur un octet chacun (vérifié).
//! - **Équipe** (`trpoke`) : 8 octets par Pokémon — `u8` IV, `u8` talent/sexe, `u16`
//!   niveau, `u16` espèce, `u16` forme — puis `u16` objet si bit 1, puis 4 × `u16`
//!   attaques si bit 0 (8, 10, 16 ou 18 octets).
//! - **Rencontres** : fichier de zone `ZO` (LZ11) ; la section 4 (position `u32` @0x10)
//!   contient 9 octets de taux (herbe, hautes herbes, groupe spécial, surf, éclate-roc,
//!   canne, super canne, méga canne, hordes) et 5 octets nuls, puis 61 emplacements
//!   de 4 octets (`u16` espèce | forme << 11, `u8` niveau min, `u8` niveau max) :
//!   herbe 12, hautes herbes 12, groupe spécial 3 (rôle non vérifié), surf 5,
//!   éclate-roc 5, cannes 3 + 3 + 3, hordes 3 × 5, puis 2 octets de remplissage.
//!   La dernière entrée (`EN`) concatène ces sections pour toutes les zones (même
//!   contenu, vérifié sur la zone 100) : laquelle le jeu lit n'est pas vérifié.
//! - **Starters** : `DllField.cro`, table des dons à 0xF906C (Rev 2), 0x24 octets par
//!   entrée (`u16` espèce @0, `u8` forme @4, `u8` niveau @5, `u16` objet @0xC) ;
//!   les trois premières sont Arcko, Poussifeu et Gobou. L'écran de choix
//!   (`DllPoke3Select.cro` @0x9FFC, pas de 0x54) contient aussi les trois espèces.
//!
//! Formats Gen 7 (Soleil/Lune, Ultra), relevés sur Lune et Ultra-Soleil (Europe) : voir
//! `randomizer::ctr_gen7` (fiches 0x54, évolutions 8 × 8 octets, dons, rencontres `EA`,
//! dresseurs 0x14 + 0x20 octets par Pokémon).

use std::path::Path;

use kaleido_formats::garc::Garc;
use kaleido_formats::romfs::{self, RomFsSource};

use crate::games::Game;
use crate::pokemon::Species;
use crate::rom::{assemble_species, RomError};
use crate::text::gen5::{MsgFile, Variant};

/// Où se trouvent les données dans le RomFS d'un jeu 3DS.
///
/// Les textes existent en une archive par langue (japonais kana, kanji, anglais,
/// français, italien, allemand, espagnol, coréen…) : on pointe directement vers le français.
#[derive(Debug, Clone, Copy)]
pub struct CtrLayout {
    /// Textes du jeu (noms, descriptions, menus) en français.
    pub text: &'static str,
    /// Textes de l'histoire (dialogues des cartes) en français.
    pub story_text: &'static str,
    pub species_names: usize,
    pub ability_names: usize,
    pub move_names: usize,
    pub item_names: usize,
    pub type_names: usize,
    pub trainer_names: usize,
    pub trainer_classes: usize,
    /// Fiches « personal » : une entrée par espèce et par forme, la dernière contient la table complète.
    pub personal: &'static str,
    /// Attaques apprises par niveau : couples `u16 attaque, u16 niveau`, fin `FFFF FFFF`.
    pub levelup: &'static str,
    /// Évolutions : 8 × (`u16 méthode, u16 paramètre, u16 espèce`) en Gen 6, 8 × 8 octets en Gen 7.
    pub evolution: &'static str,
    /// Attaques œuf : `u16 nombre`, puis les attaques (Gen 6).
    pub egg_moves: &'static str,
    /// Rencontres sauvages (Gen 6 : fichiers de zone `ZO`, voir la documentation du module).
    pub encounters: &'static str,
    pub trainer_data: &'static str,
    pub trainer_pokemon: &'static str,
    pub species_count: u16,
    /// Emplacements vérifiés sur une vraie ROM.
    pub verified: bool,
}

impl CtrLayout {
    pub fn for_game(game: Game) -> Option<Self> {
        use Game::*;
        Some(match game {
            // Vérifié sur Rubis Oméga (Europe, Rev 2) : voir `kaleido check3ds` / `species3ds`.
            OmegaRuby | AlphaSapphire => CtrLayout {
                text: "a/0/7/4",
                story_text: "a/0/8/2",
                species_names: 98,
                ability_names: 37,
                move_names: 14,
                item_names: 113,
                type_names: 18,
                trainer_names: 22,
                trainer_classes: 21,
                personal: "a/1/9/5",
                levelup: "a/1/9/1",
                evolution: "a/1/9/2",
                egg_moves: "a/1/9/0",
                encounters: "a/0/1/3",
                trainer_data: "a/0/3/6",
                trainer_pokemon: "a/0/3/8",
                species_count: 721,
                verified: true,
            },
            // Vérifié sur Pokémon Y (Europe) : noms, fiches, textes, dresseurs, rencontres ;
            // X identique d'après l'Universal Pokémon Randomizer (gen6_offsets.ini).
            X | Y => CtrLayout {
                text: "a/0/7/5",
                story_text: "a/0/8/3",
                species_names: 80,
                ability_names: 34,
                move_names: 13,
                item_names: 96,
                type_names: 17,
                trainer_names: 21,
                trainer_classes: 20,
                personal: "a/2/1/8",
                levelup: "a/2/1/4",
                evolution: "a/2/1/5",
                egg_moves: "a/2/1/3",
                encounters: "a/0/1/2",
                trainer_data: "a/0/3/8",
                trainer_pokemon: "a/0/4/0",
                species_count: 721,
                verified: true,
            },
            // D'après l'Universal Pokémon Randomizer (gen7_offsets.ini), vérifié sur Lune et
            // Ultra-Soleil (Europe) : voir `kaleido species3ds` / `randomize3ds` et les tests de
            // `randomizer::ctr_gen7`. Soleil / Ultra-Lune ne diffèrent que par l'archive des rencontres.
            Sun | Moon | UltraSun | UltraMoon => {
                let ultra = matches!(game, UltraSun | UltraMoon);
                CtrLayout {
                    text: "a/0/3/3",
                    story_text: "a/0/4/3",
                    species_names: if ultra { 60 } else { 55 },
                    ability_names: if ultra { 101 } else { 96 },
                    move_names: if ultra { 118 } else { 113 },
                    item_names: if ultra { 40 } else { 36 },
                    type_names: if ultra { 112 } else { 107 },
                    trainer_names: if ultra { 110 } else { 105 },
                    trainer_classes: if ultra { 111 } else { 106 },
                    personal: "a/0/1/7",
                    levelup: "a/0/1/3",
                    evolution: "a/0/1/4",
                    egg_moves: "a/0/1/2",
                    // L'archive de l'autre version est vide (0 octet) dans le RomFS.
                    encounters: if matches!(game, Sun | UltraSun) { "a/0/8/2" } else { "a/0/8/3" },
                    trainer_data: if ultra { "a/1/0/6" } else { "a/1/0/5" },
                    trainer_pokemon: if ultra { "a/1/0/7" } else { "a/1/0/6" },
                    species_count: if ultra { 807 } else { 802 },
                    verified: true,
                }
            }
            _ => return None,
        })
    }
}

/// Programme d'un jeu 3DS.
pub struct GameCode {
    /// Décompressé (c'est sur lui que portent les patchs `code.ips`).
    pub code: Vec<u8>,
    /// Section `.code` telle que stockée dans l'image (`None` pour un dossier extrait).
    pub stored: Option<kaleido_formats::ctr::CodeSection>,
}

/// Jeu 3DS ouvert (image déchiffrée ou dossier extrait).
pub struct CtrGameRom {
    pub game: Game,
    pub layout: CtrLayout,
    romfs: RomFsSource,
}

impl CtrGameRom {
    pub fn open(path: &Path) -> Result<Self, RomError> {
        Self::from_romfs(RomFsSource::open(path)?)
    }

    pub fn from_romfs(romfs: RomFsSource) -> Result<Self, RomError> {
        let title_id = romfs.title_id().ok_or_else(|| RomError::Unsupported("jeu 3DS non identifié (exheader.bin absent)".into()))?;
        let game = Game::from_title_id(title_id).ok_or_else(|| RomError::Unsupported(format!("jeu 3DS non pris en charge ({title_id:016X})")))?;
        let layout = CtrLayout::for_game(game).ok_or_else(|| RomError::Unsupported(format!("{} n'est pas un jeu 3DS", game.name_fr())))?;
        Ok(Self { game, layout, romfs })
    }

    pub fn romfs(&self) -> &RomFsSource {
        &self.romfs
    }

    pub fn generation(&self) -> u8 {
        self.game.generation()
    }

    pub fn title_id(&self) -> u64 {
        self.game.title_id().unwrap_or_default()
    }

    /// Archive GARC du RomFS (ex. `a/1/9/5`).
    pub fn garc(&self, path: &str) -> Result<Garc, RomError> {
        Ok(Garc::parse(&self.romfs.read(path)?)?)
    }

    /// Toutes les chaînes d'un fichier de l'archive de textes du jeu (en français).
    pub fn text_file(&self, index: usize) -> Result<Vec<String>, RomError> {
        let garc = self.garc(self.layout.text)?;
        let data = garc.file(index).ok_or_else(|| RomError::Layout(format!("fichier de texte n°{index} absent")))?;
        Ok(MsgFile::parse_with(data, Variant::Gen6)?.strings())
    }

    /// Fiches « personal » de chaque entrée (espèces puis formes alternatives).
    fn personal_records(&self) -> Result<Vec<Vec<u8>>, RomError> {
        let garc = self.garc(self.layout.personal)?;
        Ok(garc.entries.iter().map(|e| e.data().unwrap_or_default().to_vec()).collect())
    }

    pub fn species(&self) -> Result<Vec<Species>, RomError> {
        let names = self.text_file(self.layout.species_names)?;
        let abilities = self.text_file(self.layout.ability_names)?;
        let personal = self.personal_records()?;
        assemble_species(self.game, self.layout.species_count, &names, &abilities, &personal)
    }

    /// Programme du jeu (section `.code` de l'ExeFS), décompressé. Dans un dossier
    /// extrait, cherche `exefs/code.bin` (ou `code.bin`) à côté de `romfs`.
    pub fn code(&self) -> Result<GameCode, RomError> {
        match &self.romfs {
            RomFsSource::Image { path, .. } => {
                let mut r = std::io::BufReader::new(std::fs::File::open(path).map_err(kaleido_formats::FormatError::from)?);
                let img = kaleido_formats::ctr::CtrImage::probe(&mut r)?.ok_or_else(|| RomError::Unsupported("ce n'est pas une ROM 3DS".into()))?;
                let ncch = img.ncch.ok_or_else(|| RomError::Unsupported("partition principale illisible".into()))?;
                let stored = kaleido_formats::ctr::read_code(&mut r, ncch.offset)?;
                let code = if stored.compressed { kaleido_formats::lz::decompress_blz(&stored.raw)? } else { stored.raw.clone() };
                Ok(GameCode { code, stored: Some(stored) })
            }
            RomFsSource::Dir { root, .. } => {
                let base = root.parent().unwrap_or(root);
                let path = ["exefs/code.bin", "exefs/.code.bin", "code.bin", ".code.bin"]
                    .iter()
                    .map(|n| base.join(n))
                    .find(|p| p.is_file())
                    .ok_or_else(|| RomError::Unsupported("dossier extrait : code.bin introuvable (exefs/code.bin)".into()))?;
                let data = std::fs::read(path).map_err(kaleido_formats::FormatError::from)?;
                // Les outils d'extraction écrivent en général le programme décompressé
                // (taille multiple d'une page) ; sinon, on le décompresse.
                let code = if data.len() % 0x200 == 0 { data } else { kaleido_formats::lz::decompress_blz(&data)? };
                Ok(GameCode { code, stored: None })
            }
        }
    }

    /// Écrit des fichiers du RomFS modifiés au format LayeredFS (Luma3DS), sous `out_dir`.
    pub fn write_layeredfs(&self, out_dir: &Path, files: &[(&str, &[u8])]) -> Result<std::path::PathBuf, RomError> {
        Ok(romfs::write_layeredfs(out_dir, self.title_id(), files)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        for game in Game::ALL {
            let layout = CtrLayout::for_game(game);
            assert_eq!(layout.is_some(), game.generation() >= 6, "{game:?}");
        }
        let oras = CtrLayout::for_game(Game::AlphaSapphire).unwrap();
        assert!(oras.verified && oras.species_count == 721);
        let xy = CtrLayout::for_game(Game::Y).unwrap();
        assert!(xy.verified && xy.personal == "a/2/1/8");
        assert_eq!(CtrLayout::for_game(Game::UltraMoon).unwrap().species_count, 807);
        assert_eq!(CtrLayout::for_game(Game::Sun).unwrap().species_count, 802);
    }
}
