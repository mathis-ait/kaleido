//! Détection automatique de ce que l'utilisateur a déposé : ROM DS, ROM 3DS,
//! dossier de jeu 3DS extrait, sauvegarde… ou fichier inconnu.

use std::fs::{self, File};
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use kaleido_formats::ctr::{self, CtrImage};
use kaleido_formats::nds::NdsHeader;
use kaleido_formats::{stream_len, FormatError};
use serde::Serialize;

use crate::games::{nds_language, Game, Platform};
use crate::saves;

/// Au-delà, ce ne peut pas être une sauvegarde : inutile de lire le fichier en entier.
const MAX_SAVE_SIZE: u64 = 0x100000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    NdsRom,
    CtrRom,
    CtrDump,
    Save,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub id: Game,
    pub name: &'static str,
    pub generation: u8,
    pub platform: Platform,
}

impl From<Game> for GameInfo {
    fn from(g: Game) -> Self {
        Self { id: g, name: g.name_fr(), generation: g.generation(), platform: g.platform() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Detail {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub path: String,
    pub file_name: String,
    pub kind: FileKind,
    /// Titre à afficher, toujours renseigné.
    pub title: String,
    pub game: Option<GameInfo>,
    pub platform: Option<Platform>,
    pub generation: Option<u8>,
    pub language: Option<String>,
    pub is_french: bool,
    pub size: u64,
    pub details: Vec<Detail>,
    pub warnings: Vec<String>,
}

impl Detection {
    fn new(kind: FileKind, title: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            file_name: String::new(),
            kind,
            title: title.into(),
            game: None,
            platform: None,
            generation: None,
            language: None,
            is_french: false,
            size: 0,
            details: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn with_game(mut self, game: Game) -> Self {
        self.platform = Some(game.platform());
        self.generation = Some(game.generation());
        self.game = Some(game.into());
        self
    }

    fn detail(&mut self, label: &str, value: impl Into<String>) {
        self.details.push(Detail { label: label.into(), value: value.into() });
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    #[error("impossible de lire « {0} » : {1}")]
    Io(String, std::io::Error),
    #[error(transparent)]
    Format(#[from] FormatError),
}

pub fn detect_path(path: &Path) -> Result<Detection, DetectError> {
    let io_err = |e| DetectError::Io(path.display().to_string(), e);
    let mut detection = if path.is_dir() {
        detect_dump_dir(path).map_err(io_err)?
    } else {
        let file = File::open(path).map_err(io_err)?;
        detect_stream(&mut BufReader::new(file))?
    };
    detection.path = path.display().to_string();
    detection.file_name = path.file_name().map_or_else(|| detection.path.clone(), |n| n.to_string_lossy().into_owned());
    Ok(detection)
}

pub fn detect_stream<R: Read + Seek>(r: &mut R) -> Result<Detection, DetectError> {
    let size = stream_len(r).map_err(FormatError::from)?;

    let mut detection = if let Some(header) = NdsHeader::probe(r)? {
        from_nds(&header)
    } else if let Some(image) = CtrImage::probe(r)? {
        from_ctr(&image)
    } else if size <= MAX_SAVE_SIZE {
        let mut data = Vec::with_capacity(size as usize);
        r.rewind().map_err(FormatError::from)?;
        r.read_to_end(&mut data).map_err(FormatError::from)?;
        saves::identify(&data).map_or_else(unknown, from_save)
    } else {
        unknown()
    };
    detection.size = size;
    Ok(detection)
}

fn unknown() -> Detection {
    let mut d = Detection::new(FileKind::Unknown, "Fichier non reconnu");
    d.warnings.push("Ce fichier n'est ni une ROM DS/3DS, ni une sauvegarde Gen 4 à 7 connue.".into());
    d
}

fn from_nds(h: &NdsHeader) -> Detection {
    let game = Game::from_nds_code(&h.game_code);
    let mut d = match game {
        Some(g) => Detection::new(FileKind::NdsRom, g.name_fr()).with_game(g),
        None => {
            let mut d = Detection::new(FileKind::NdsRom, format!("ROM DS « {} »", h.title));
            d.platform = Some(Platform::Nds);
            d.warnings.push("Jeu DS non pris en charge par Kaleido.".into());
            d
        }
    };
    d.language = h.region().and_then(nds_language).map(Into::into);
    d.is_french = h.region() == Some('F');
    d.detail("Titre interne", &h.title);
    d.detail("Code jeu", &h.game_code);
    d.detail("Révision", format!("1.{}", h.rom_version));
    if !h.header_crc_ok {
        d.warnings.push("Le CRC de l'en-tête est incorrect : la ROM a peut-être été modifiée.".into());
    }
    d
}

fn from_ctr(img: &CtrImage) -> Detection {
    let mut d = ctr_game_detection(FileKind::CtrRom, img.title_id, "ROM 3DS");
    d.detail("Conteneur", img.container.label());
    match &img.ncch {
        Some(ncch) => {
            d.detail("Code produit", &ncch.product_code);
            if ncch.encrypted {
                d.warnings.push(
                    "ROM chiffrée : déchiffre-la d'abord (Godmode9 ou l'outil de ton émulateur).".into(),
                );
            }
        }
        None => d.warnings.push("Contenu du CIA chiffré ou illisible.".into()),
    }
    d
}

fn from_save(kind: saves::SaveKind) -> Detection {
    let mut d = Detection::new(FileKind::Save, format!("Sauvegarde · {}", kind.label()));
    d.generation = Some(kind.generation());
    d.platform = Some(if kind.generation() <= 5 { Platform::Nds } else { Platform::N3ds });
    if kind.is_guess() {
        d.warnings.push("Identification par la taille seulement : Gen 5 supposée.".into());
    }
    d
}

/// Dossier extrait d'un jeu 3DS (comme pour pk3DS) : `romfs/` + `exheader.bin`.
fn detect_dump_dir(dir: &Path) -> std::io::Result<Detection> {
    // L'utilisateur peut aussi déposer directement le dossier `romfs`.
    let root = if dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("romfs")) {
        dir.parent().unwrap_or(dir)
    } else {
        dir
    };

    if !root.join("romfs").is_dir() {
        let mut d = Detection::new(FileKind::Unknown, "Dossier non reconnu");
        d.warnings.push("Un dossier de jeu 3DS doit contenir un sous-dossier « romfs ».".into());
        return Ok(d);
    }

    let program_id = ["exheader.bin", "ExHeader.bin", "DecryptedExHeader.bin"]
        .iter()
        .map(|name| root.join(name))
        .find(|p| p.is_file())
        .map(fs::read)
        .transpose()?
        .and_then(|data| ctr::exheader_program_id(&data));

    let mut d = match program_id {
        Some(id) => ctr_game_detection(FileKind::CtrDump, id, "Dossier 3DS"),
        None => {
            let mut d = Detection::new(FileKind::CtrDump, "Dossier 3DS extrait");
            d.platform = Some(Platform::N3ds);
            d.warnings.push("exheader.bin introuvable : impossible d'identifier le jeu.".into());
            d
        }
    };
    d.detail("Dossier", root.display().to_string());
    Ok(d)
}

fn ctr_game_detection(kind: FileKind, title_id: u64, fallback: &str) -> Detection {
    let mut d = match Game::from_title_id(title_id) {
        Some(g) => {
            let mut d = Detection::new(kind, g.name_fr()).with_game(g);
            // Les jeux Pokémon 3DS contiennent toutes les langues.
            d.language = Some("Multilingue (français inclus)".into());
            d.is_french = true;
            d
        }
        None => {
            let mut d = Detection::new(kind, fallback);
            d.platform = Some(Platform::N3ds);
            d.warnings.push("Jeu 3DS non pris en charge par Kaleido.".into());
            d
        }
    };
    d.detail("Title ID", format!("{title_id:016X}"));
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn detects_french_platinum() {
        let mut h = vec![0u8; 0x200];
        h[..10].copy_from_slice(b"POKEMON PL");
        h[0x0C..0x10].copy_from_slice(b"CPUF");
        h[0x15C..0x15E].copy_from_slice(&0xCF56u16.to_le_bytes());
        let crc = kaleido_formats::nds::crc16(&h[..0x15E]);
        h[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());

        let d = detect_stream(&mut Cursor::new(h)).unwrap();
        assert_eq!(d.kind, FileKind::NdsRom);
        assert_eq!(d.title, "Pokémon Platine");
        assert!(d.is_french);
        assert!(d.warnings.is_empty());
    }

    #[test]
    fn detects_sun_moon_save() {
        let d = detect_stream(&mut Cursor::new(vec![0u8; 0x6BE00])).unwrap();
        assert_eq!(d.kind, FileKind::Save);
        assert_eq!(d.generation, Some(7));
    }

    #[test]
    fn unknown_file() {
        let d = detect_stream(&mut Cursor::new(vec![1u8; 100])).unwrap();
        assert_eq!(d.kind, FileKind::Unknown);
    }
}
