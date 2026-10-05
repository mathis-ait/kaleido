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
    /// Présent si le fichier a été généré par Kaleido (seed, code de partage).
    pub kaleido: Option<crate::randomizer::KaleidoTag>,
    /// Empreinte du contenu (fichiers ≤ 600 Mo), pour repérer les doublons.
    pub fingerprint: Option<String>,
}

/// Au-delà, pas d'empreinte (lecture trop longue, ex. ROM 3DS de 2 Go).
const FINGERPRINT_MAX: u64 = 600 * 1024 * 1024;

/// Empreinte rapide du contenu d'un flux (SipHash par blocs de 1 Mo).
fn fingerprint<R: Read + Seek>(r: &mut R) -> std::io::Result<String> {
    use std::hash::Hasher;
    r.rewind()?;
    let mut hasher = std::hash::DefaultHasher::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Ok(format!("{:016x}", hasher.finish()))
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
            kaleido: None,
            fingerprint: None,
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

/// Extensions des fichiers ajoutés quand on dépose un dossier ordinaire.
const KNOWN_EXTENSIONS: &[&str] = &["nds", "3ds", "cci", "cia", "cxi", "sav", "dsv"];

fn is_ctr_dump(dir: &Path) -> bool {
    dir.join("romfs").is_dir() || dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("romfs"))
}

/// Développe un chemin déposé : un fichier ou un dossier de jeu 3DS extrait reste tel
/// quel ; un dossier ordinaire est remplacé par les ROMs, sauvegardes et dossiers
/// 3DS qu'il contient (sur deux niveaux de profondeur).
pub fn expand_path(path: &Path) -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, depth: u8, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                if is_ctr_dump(&p) {
                    out.push(p);
                } else if depth > 0 {
                    walk(&p, depth - 1, out);
                }
            } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| KNOWN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())) {
                out.push(p);
            }
        }
    }
    if !path.is_dir() || is_ctr_dump(path) {
        return vec![path.to_path_buf()];
    }
    let mut out = Vec::new();
    walk(path, 1, &mut out);
    out
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
    // ROM générée avant l'ajout de la signature : la seed figure dans le nom proposé par Kaleido.
    if detection.kind == FileKind::NdsRom && detection.kaleido.is_none() {
        if let Some(seed) = seed_from_file_name(&detection.file_name) {
            detection.detail("Randomisée par", "Kaleido (d'après le nom du fichier)");
            detection.detail("Seed", seed.to_string());
        }
    }
    Ok(detection)
}

/// « … - Kaleido 4145467160.nds » → 4145467160.
fn seed_from_file_name(name: &str) -> Option<u64> {
    let rest = &name[name.rfind("Kaleido ")? + "Kaleido ".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub fn detect_stream<R: Read + Seek>(r: &mut R) -> Result<Detection, DetectError> {
    let size = stream_len(r).map_err(FormatError::from)?;

    let mut detection = if let Some(header) = NdsHeader::probe(r)? {
        let mut d = from_nds(&header);
        // ROM générée par Kaleido : signature pointée depuis l'en-tête.
        let mut raw = vec![0u8; kaleido_formats::nds::HEADER_SIZE];
        r.rewind().map_err(FormatError::from)?;
        r.read_exact(&mut raw).map_err(FormatError::from)?;
        if let Some(tag) =
            kaleido_formats::nds::read_signature(r, &raw)?.and_then(|json| serde_json::from_slice::<crate::randomizer::KaleidoTag>(&json).ok())
        {
            d.detail("Randomisée par", format!("Kaleido {}", tag.version));
            d.detail("Seed", tag.seed.to_string());
            d.kaleido = Some(tag);
        }
        d
    } else if let Some(image) = CtrImage::probe(r)? {
        from_ctr(&image)
    } else if size <= MAX_SAVE_SIZE {
        let mut data = Vec::with_capacity(size as usize);
        r.rewind().map_err(FormatError::from)?;
        r.read_to_end(&mut data).map_err(FormatError::from)?;
        match saves::identify(&data) {
            Some(kind) => from_save(kind, &data),
            None => unknown(),
        }
    } else {
        unknown()
    };
    detection.size = size;
    if size <= FINGERPRINT_MAX {
        detection.fingerprint = fingerprint(r).ok();
    }
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
                d.warnings.push("ROM chiffrée : déchiffre-la d'abord (Godmode9 ou l'outil de ton émulateur).".into());
            }
        }
        None => d.warnings.push("Contenu du CIA chiffré ou illisible.".into()),
    }
    d
}

fn from_save(kind: saves::SaveKind, data: &[u8]) -> Detection {
    // Le moteur de sauvegardes identifie le jeu exact (Noire/Blanche ou leurs suites)
    // et lit le nom du dresseur.
    let parsed = crate::save::SaveFile::from_bytes(data).ok();
    let label = parsed.as_ref().map_or(kind.label(), |s| s.version().label());
    let mut d = Detection::new(FileKind::Save, format!("Sauvegarde · {label}"));
    d.generation = Some(kind.generation());
    d.platform = Some(if kind.generation() <= 5 { Platform::Nds } else { Platform::N3ds });
    match &parsed {
        Some(save) => {
            let trainer = save.trainer();
            if !trainer.name.trim().is_empty() {
                d.detail("Dresseur", trainer.name.trim());
            }
            d.detail("Équipe", format!("{} Pokémon", save.party_count()));
            let t = trainer.play_time;
            d.detail("Temps de jeu", format!("{} h {:02}", t.hours, t.minutes));
        }
        None if kind.is_guess() => d.warnings.push("Identification par la taille seulement : Gen 5 supposée.".into()),
        None => {}
    }
    d
}

/// Dossier extrait d'un jeu 3DS (comme pour pk3DS) : `romfs/` + `exheader.bin`.
fn detect_dump_dir(dir: &Path) -> std::io::Result<Detection> {
    // L'utilisateur peut aussi déposer directement le dossier `romfs`.
    let root = if dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("romfs")) { dir.parent().unwrap_or(dir) } else { dir };

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
    fn seed_in_file_name() {
        assert_eq!(seed_from_file_name("Blanche - Kaleido 4145467160.nds"), Some(4145467160));
        assert_eq!(seed_from_file_name("Blanche.nds"), None);
    }

    #[test]
    fn expand_plain_folder() {
        let root = std::env::temp_dir().join(format!("kaleido-expand-{}", std::process::id()));
        let dump = root.join("Rubis").join("romfs");
        fs::create_dir_all(&dump).unwrap();
        fs::create_dir_all(root.join("sous")).unwrap();
        for f in ["a.nds", "b.SAV", "notes.txt", "sous/c.3ds"] {
            fs::write(root.join(f), b"x").unwrap();
        }
        let found: Vec<String> = expand_path(&root).iter().map(|p| p.strip_prefix(&root).unwrap().display().to_string().replace('\\', "/")).collect();
        assert_eq!(found, vec!["Rubis", "a.nds", "b.SAV", "sous/c.3ds"]);
        assert_eq!(expand_path(&root.join("Rubis")), vec![root.join("Rubis")]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unknown_file() {
        let d = detect_stream(&mut Cursor::new(vec![1u8; 100])).unwrap();
        assert_eq!(d.kind, FileKind::Unknown);
    }
}
