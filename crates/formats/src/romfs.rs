//! Système de fichiers RomFS des jeux 3DS, lu directement dans une image
//! (CCI / CXI / CIA déchiffrée) ou dans un dossier extrait (`romfs/` + `exheader.bin`).
//!
//! Référence : 3dbrew — RomFS. L'en-tête IVFC décrit trois niveaux d'arbres de
//! hachage ; seul le niveau 3 (les métadonnées et les données) nous intéresse.
//! Les fichiers sont lus à la demande : une image peut peser plusieurs Go.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::ctr::{self, CtrImage};
use crate::util::{align, read_at, slice, stream_len, u32le, u64le};
use crate::{FormatError, Result};

const IVFC_HEADER_SIZE: usize = 0x5C;
/// Taille de l'en-tête du niveau 3 (tables des dossiers et des fichiers).
const LEVEL3_HEADER_SIZE: usize = 0x28;
const NONE: u32 = 0xFFFF_FFFF;
/// Garde-fou contre les tables corrompues (les jeux Pokémon en ont quelques dizaines de milliers).
const MAX_ENTRIES: usize = 1_000_000;

/// Un fichier du RomFS : chemin (séparateur `/`, sans `/` initial), position
/// relative au début de la zone de données, taille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomFsEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

/// RomFS contenu dans un flux (image de jeu ou fichier `romfs.bin`).
pub struct RomFsImage<R> {
    reader: Mutex<R>,
    /// Position absolue de la zone de données des fichiers.
    data_offset: u64,
    files: Vec<RomFsEntry>,
    index: HashMap<String, usize>,
}

impl<R: Read + Seek> RomFsImage<R> {
    /// Ouvre le RomFS qui commence à `offset` (en-tête IVFC) dans le flux.
    pub fn open(mut r: R, offset: u64) -> Result<Self> {
        let ivfc = read_at(&mut r, offset, IVFC_HEADER_SIZE)?;
        if &ivfc[..4] != b"IVFC" || u32le(&ivfc, 4) != 0x10000 {
            return Err(FormatError::Invalid("RomFS illisible (en-tête IVFC absent : contenu chiffré ?)"));
        }
        let master_hash_size = u32le(&ivfc, 0x08) as u64;
        let level3_block = 1u64 << (u32le(&ivfc, 0x4C) & 0x1F);
        let level3_size = u64le(&ivfc, 0x44);
        let level3 = offset + align(0x60 + master_hash_size, level3_block);

        let h = read_at(&mut r, level3, LEVEL3_HEADER_SIZE)?;
        if u32le(&h, 0) as usize != LEVEL3_HEADER_SIZE {
            return Err(FormatError::Invalid("en-tête du niveau 3 du RomFS invalide"));
        }
        let table = |at: usize| (u32le(&h, at) as u64, u32le(&h, at + 4) as usize);
        let (dir_meta_at, dir_meta_len) = table(0x0C);
        let (file_meta_at, file_meta_len) = table(0x1C);
        let data_at = u32le(&h, 0x24) as u64;
        if [dir_meta_at + dir_meta_len as u64, file_meta_at + file_meta_len as u64, data_at].iter().any(|&e| e > level3_size) {
            return Err(FormatError::Invalid("tables du RomFS hors du niveau 3"));
        }
        let dirs = read_at(&mut r, level3 + dir_meta_at, dir_meta_len)?;
        let file_meta = read_at(&mut r, level3 + file_meta_at, file_meta_len)?;
        let files = walk_tables(&dirs, &file_meta)?;
        if files.iter().any(|f| data_at + f.offset + f.size > level3_size) {
            return Err(FormatError::Invalid("fichier du RomFS hors des données"));
        }
        let index = files.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
        Ok(Self { reader: Mutex::new(r), data_offset: level3 + data_at, files, index })
    }

    pub fn files(&self) -> &[RomFsEntry] {
        &self.files
    }

    pub fn entry(&self, path: &str) -> Option<&RomFsEntry> {
        self.index.get(normalize(path)).map(|&i| &self.files[i])
    }

    /// Contenu d'un fichier, lu à la demande.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let e = self.entry(path).ok_or_else(|| FormatError::NotFound(path.to_string()))?;
        let len = usize::try_from(e.size).map_err(|_| FormatError::Invalid("fichier trop grand"))?;
        let mut r = self.reader.lock().unwrap_or_else(|p| p.into_inner());
        Ok(read_at(&mut *r, self.data_offset + e.offset, len)?)
    }
}

fn normalize(path: &str) -> &str {
    path.trim_start_matches(['/', '\\'])
}

/// Parcourt les tables de métadonnées du niveau 3 à partir du dossier racine.
fn walk_tables(dirs: &[u8], files: &[u8]) -> Result<Vec<RomFsEntry>> {
    const BAD: FormatError = FormatError::Invalid("métadonnées du RomFS incohérentes");
    let name = |table: &[u8], at: u32, header: u32| -> Result<String> {
        let len = u32le(slice(table, at + header - 4, 4)?, 0);
        let raw = slice(table, at + header, len)?;
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(String::from_utf16_lossy(&units))
    };

    let mut out = Vec::new();
    let mut stack = vec![(0u32, String::new())];
    let mut visited = 0;
    while let Some((dir, prefix)) = stack.pop() {
        visited += 1;
        if visited > MAX_ENTRIES {
            return Err(BAD);
        }
        let d = slice(dirs, dir, 0x18)?;
        let (first_child, first_file) = (u32le(d, 0x08), u32le(d, 0x0C));

        let mut file = first_file;
        while file != NONE {
            let f = slice(files, file, 0x20)?;
            let path = format!("{prefix}{}", name(files, file, 0x20)?);
            out.push(RomFsEntry { path, offset: u64le(f, 0x08), size: u64le(f, 0x10) });
            if out.len() > MAX_ENTRIES {
                return Err(BAD);
            }
            file = u32le(f, 0x04);
        }

        let mut child = first_child;
        let mut children = Vec::new();
        while child != NONE {
            let c = slice(dirs, child, 0x18)?;
            children.push((child, format!("{prefix}{}/", name(dirs, child, 0x18)?)));
            if children.len() > MAX_ENTRIES {
                return Err(BAD);
            }
            child = u32le(c, 0x04);
        }
        // Pile : on empile à l'envers pour garder l'ordre des tables.
        stack.extend(children.into_iter().rev());
    }
    Ok(out)
}

/// RomFS d'un jeu 3DS, quelle que soit sa provenance.
pub enum RomFsSource {
    /// Image de jeu (`.3ds`, `.cci`, `.cxi`, `.cia` déchiffré).
    Image { title_id: u64, romfs: RomFsImage<BufReader<File>> },
    /// Dossier extrait (`romfs/` à côté de `exheader.bin`).
    Dir { title_id: Option<u64>, root: PathBuf, files: Vec<RomFsEntry> },
}

impl RomFsSource {
    /// Ouvre une image de jeu ou un dossier extrait (le dossier du jeu ou directement `romfs`).
    pub fn open(path: &Path) -> Result<Self> {
        if path.is_dir() {
            return Self::open_dir(path);
        }
        let mut r = BufReader::new(File::open(path)?);
        let img = CtrImage::probe(&mut r)?.ok_or(FormatError::Invalid("ce n'est pas une ROM 3DS"))?;
        let ncch = img.ncch.ok_or(FormatError::Invalid("partition principale illisible (CIA chiffré ?)"))?;
        if ncch.encrypted {
            return Err(FormatError::Invalid("ROM 3DS chiffrée : déchiffre-la d'abord"));
        }
        if ncch.romfs_size == 0 || ncch.romfs_offset + ncch.romfs_size > stream_len(&mut r)? {
            return Err(FormatError::Invalid("RomFS absent ou tronqué"));
        }
        Ok(Self::Image { title_id: img.title_id, romfs: RomFsImage::open(r, ncch.romfs_offset)? })
    }

    fn open_dir(dir: &Path) -> Result<Self> {
        let is_romfs = dir.file_name().is_some_and(|n| n.eq_ignore_ascii_case("romfs"));
        let (base, root) = if is_romfs { (dir.parent().unwrap_or(dir), dir.to_path_buf()) } else { (dir, dir.join("romfs")) };
        if !root.is_dir() {
            return Err(FormatError::NotFound(format!("dossier « romfs » dans {}", dir.display())));
        }
        let title_id = ["exheader.bin", "ExHeader.bin", "DecryptedExHeader.bin"]
            .iter()
            .map(|n| base.join(n))
            .find(|p| p.is_file())
            .map(std::fs::read)
            .transpose()?
            .and_then(|d| ctr::exheader_program_id(&d));

        let mut files = Vec::new();
        let mut stack = vec![(root.clone(), String::new())];
        while let Some((dir, prefix)) = stack.pop() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)?.collect::<std::io::Result<_>>()?;
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let name = format!("{prefix}{}", e.file_name().to_string_lossy());
                let meta = e.metadata()?;
                if meta.is_dir() {
                    stack.push((e.path(), format!("{name}/")));
                } else {
                    files.push(RomFsEntry { path: name, offset: 0, size: meta.len() });
                }
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Self::Dir { title_id, root, files })
    }

    pub fn title_id(&self) -> Option<u64> {
        match self {
            Self::Image { title_id, .. } => Some(*title_id),
            Self::Dir { title_id, .. } => *title_id,
        }
    }

    /// Tous les fichiers (dans un dossier extrait, `offset` vaut 0).
    pub fn files(&self) -> &[RomFsEntry] {
        match self {
            Self::Image { romfs, .. } => romfs.files(),
            Self::Dir { files, .. } => files,
        }
    }

    pub fn contains(&self, path: &str) -> bool {
        match self {
            Self::Image { romfs, .. } => romfs.entry(path).is_some(),
            Self::Dir { files, .. } => files.binary_search_by(|f| f.path.as_str().cmp(normalize(path))).is_ok(),
        }
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        match self {
            Self::Image { romfs, .. } => romfs.read(path),
            Self::Dir { root, .. } => {
                let path = normalize(path);
                if !self.contains(path) {
                    return Err(FormatError::NotFound(path.to_string()));
                }
                Ok(std::fs::read(root.join(path))?)
            }
        }
    }
}

/// Écrit des fichiers modifiés sous forme de LayeredFS (Luma3DS) :
/// `<out_dir>/luma/titles/<TITLE ID>/romfs/<chemin>`. Renvoie le dossier `romfs` créé.
pub fn write_layeredfs(out_dir: &Path, title_id: u64, files: &[(&str, &[u8])]) -> Result<PathBuf> {
    let romfs = out_dir.join("luma").join("titles").join(format!("{title_id:016X}")).join("romfs");
    for (path, data) in files {
        let rel = normalize(path);
        if rel.is_empty() || rel.split(['/', '\\']).any(|c| c.is_empty() || c == "." || c == "..") {
            return Err(FormatError::Invalid("chemin RomFS invalide"));
        }
        let target = romfs.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, data)?;
    }
    Ok(romfs)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Cursor;

    fn utf16(s: &str) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
        while v.len() % 4 != 0 {
            v.push(0);
        }
        v
    }

    fn dir_entry(parent: u32, sibling: u32, child: u32, file: u32, name: &str) -> Vec<u8> {
        let mut e = Vec::new();
        for v in [parent, sibling, child, file, NONE, (name.len() * 2) as u32] {
            e.extend(v.to_le_bytes());
        }
        e.extend(utf16(name));
        e
    }

    fn file_entry(parent: u32, sibling: u32, offset: u64, size: u64, name: &str) -> Vec<u8> {
        let mut e = Vec::new();
        e.extend(parent.to_le_bytes());
        e.extend(sibling.to_le_bytes());
        e.extend(offset.to_le_bytes());
        e.extend(size.to_le_bytes());
        e.extend(NONE.to_le_bytes());
        e.extend(((name.len() * 2) as u32).to_le_bytes());
        e.extend(utf16(name));
        e
    }

    /// RomFS minimal : `a.bin` à la racine et `d/b.bin`, précédé de `prefix` octets.
    pub(crate) fn tiny_romfs(prefix: usize) -> Vec<u8> {
        // Dossiers : racine @0 (enfant `d` @0x18, fichier @0), `d` @0x18 (fichier @0x2C).
        let mut dirs = dir_entry(0, NONE, 0x18, 0, "");
        dirs.extend(dir_entry(0, NONE, NONE, 0x2C, "d"));
        let mut files = file_entry(0, NONE, 0, 4, "a.bin");
        assert_eq!(files.len(), 0x2C);
        files.extend(file_entry(0x18, NONE, 0x10, 2, "b.bin"));

        let mut l3 = vec![0u8; LEVEL3_HEADER_SIZE];
        let dir_at = LEVEL3_HEADER_SIZE as u32;
        let file_at = dir_at + dirs.len() as u32;
        let data_at = align((file_at as usize + files.len()) as u64, 16) as u32;
        let fields = [
            LEVEL3_HEADER_SIZE as u32,
            dir_at,
            0,
            dir_at,
            dirs.len() as u32,
            file_at,
            0,
            file_at,
            files.len() as u32,
            data_at,
        ];
        for (i, v) in fields.iter().enumerate() {
            l3[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        l3.extend(&dirs);
        l3.extend(&files);
        l3.resize(data_at as usize, 0);
        l3.extend(b"AAAA");
        l3.resize(data_at as usize + 0x10, 0);
        l3.extend(b"BB");

        let mut out = vec![0u8; prefix];
        let mut ivfc = vec![0u8; 0x60];
        ivfc[..4].copy_from_slice(b"IVFC");
        ivfc[4..8].copy_from_slice(&0x10000u32.to_le_bytes());
        ivfc[0x08..0x0C].copy_from_slice(&0x20u32.to_le_bytes());
        ivfc[0x44..0x4C].copy_from_slice(&(l3.len() as u64).to_le_bytes());
        ivfc[0x4C..0x50].copy_from_slice(&7u32.to_le_bytes()); // blocs de 0x80
        out.extend(ivfc);
        out.resize(prefix + 0x80, 0);
        out.extend(l3);
        out
    }

    #[test]
    fn reads_level3_tables() {
        let romfs = RomFsImage::open(Cursor::new(tiny_romfs(0x200)), 0x200).unwrap();
        let paths: Vec<_> = romfs.files().iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.bin", "d/b.bin"]);
        assert_eq!(romfs.read("/d/b.bin").unwrap(), b"BB");
        assert_eq!(romfs.read("a.bin").unwrap(), b"AAAA");
        assert!(romfs.read("c.bin").is_err());
    }

    #[test]
    fn rejects_encrypted() {
        assert!(RomFsImage::open(Cursor::new(vec![0x55u8; 0x400]), 0).is_err());
    }

    #[test]
    fn extracted_dir_and_layeredfs() {
        let tmp = std::env::temp_dir().join(format!("kaleido-romfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let game = tmp.join("jeu");
        std::fs::create_dir_all(game.join("romfs/a/0")).unwrap();
        std::fs::write(game.join("romfs/a/0/1"), b"xyz").unwrap();
        let mut exheader = vec![0u8; 0x400];
        exheader[0x200..0x208].copy_from_slice(&0x0004_0000_0011_C400u64.to_le_bytes());
        std::fs::write(game.join("exheader.bin"), exheader).unwrap();

        let src = RomFsSource::open(&game).unwrap();
        assert_eq!(src.title_id(), Some(0x0004_0000_0011_C400));
        assert_eq!(src.files().len(), 1);
        assert_eq!(src.read("a/0/1").unwrap(), b"xyz");
        assert!(RomFsSource::open(&game.join("romfs")).unwrap().contains("/a/0/1"));

        let out = write_layeredfs(&tmp.join("sortie"), 0x0004_0000_0011_C400, &[("a/0/1", b"new")]).unwrap();
        assert!(out.ends_with("luma/titles/000400000011C400/romfs"));
        assert_eq!(std::fs::read(out.join("a/0/1")).unwrap(), b"new");
        assert!(write_layeredfs(&tmp, 1, &[("../x", b"")]).is_err());
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
