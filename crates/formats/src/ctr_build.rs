//! Reconstruction d'une image 3DS déchiffrée (« NoCrypto ») dont le RomFS contient
//! des fichiers remplacés : CCI (`.3ds`) ou NCCH seul (`.cxi`).
//!
//! Références :
//! - 3dbrew — RomFS (<https://www.3dbrew.org/wiki/RomFS>) : en-tête IVFC, trois
//!   niveaux de hachage SHA-256 par blocs de `1 << taille_bloc` octets (0x1000 dans
//!   les jeux), disposition physique « en-tête IVFC + hachages maîtres, niveau 3,
//!   niveau 1, niveau 2 », chacun aligné sur sa taille de bloc.
//! - 3dbrew — NCCH (<https://www.3dbrew.org/wiki/NCCH>) : tailles et positions en
//!   unités de média (0x200 octets), hachage du « superbloc » RomFS = SHA-256 des
//!   `taille de la région de hachage` premières unités du RomFS (en-tête IVFC +
//!   hachages maîtres), tous les hachages portent sur les données en clair.
//! - 3dbrew — NCSD (<https://www.3dbrew.org/wiki/NCSD>) : table des 8 partitions
//!   (position, taille en unités de média), taille de l'image @0x104, taille
//!   remplie @0x300 (en octets), puis les « InitialData » @0x1000, suivies d'une
//!   copie de l'en-tête NCCH de la partition 0 (sans signature, @0x1100).
//!
//! Méthode (même résultat qu'un outil comme 3dstool, mais en flux) : les tables du
//! niveau 3 (dossiers, fichiers, tables de hachage des noms) sont conservées telles
//! quelles — les tables de hachage ne dépendent que des noms et des parents — et
//! seuls la position et la taille des fichiers remplacés changent. Les fichiers
//! suivants sont décalés en gardant leur alignement sur 16 octets. Reconstruire
//! sans rien remplacer redonne donc exactement l'image d'origine.
//!
//! Les signatures RSA des en-têtes NCSD / NCCH ne peuvent pas être refaites : les
//! émulateurs (Azahar, Citra) ne les vérifient pas, une console sous Luma3DS non
//! plus une fois le jeu installé en CIA (correctifs de signature).

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::ctr::{Container, CtrImage, MEDIA_UNIT};
use crate::romfs::{normalize, walk_tables};
use crate::util::{align, read_at, stream_len, u32le, u64le};
use crate::{FormatError, Result};

const IVFC_HEADER_SIZE: u64 = 0x60;
const LEVEL3_HEADER_SIZE: usize = 0x28;
const NCCH_HEADER_SIZE: usize = 0x200;
/// En-tête NCSD + « card info » + « InitialData » (avec la copie de l'en-tête NCCH).
const NCSD_HEADER_SIZE: usize = 0x1200;
/// Copie de l'en-tête NCCH de la partition 0 : ses 0x100 premiers octets (la
/// signature) sont remplacés par les InitialData, la partie utile commence à 0x1100.
const NCSD_NCCH_COPY: usize = 0x1000;
/// Alignement des fichiers dans la zone de données du niveau 3 (comme les jeux d'origine).
const FILE_ALIGN: u64 = 0x10;
const BUF_SIZE: usize = 1 << 20;

/// Résultat d'une reconstruction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    /// Format écrit : `Cci` (`.3ds`) ou `Cxi`.
    pub container: Container,
    /// Taille du fichier écrit, en octets.
    pub size: u64,
    pub romfs_size: u64,
    pub replaced: usize,
}

/// Extension du fichier produit pour une image d'entrée (`3ds` ou `cxi`).
pub fn output_extension(input: Container) -> &'static str {
    match input {
        Container::Cci => "3ds",
        Container::Cxi | Container::Cia => "cxi",
    }
}

/// Reconstruit l'image `input` (`.3ds`, `.cxi` ou `.cia` déchiffrés) vers `output`
/// en remplaçant des fichiers existants du RomFS. Une entrée CCI donne un `.3ds`
/// (les autres partitions — manuel, téléchargement, mises à jour — sont recopiées),
/// une entrée CXI ou CIA donne un `.cxi` (la partition principale seule).
pub fn rebuild_image(input: &Path, output: &Path, files: &[(&str, &[u8])]) -> Result<RebuildReport> {
    if same_file(input, output) {
        return Err(FormatError::Invalid("le fichier de sortie doit être différent de la ROM d'origine"));
    }
    let mut r = BufReader::with_capacity(BUF_SIZE, File::open(input)?);
    let img = CtrImage::probe(&mut r)?.ok_or(FormatError::Invalid("ce n'est pas une ROM 3DS"))?;
    let ncch = img.ncch.ok_or(FormatError::Invalid("partition principale illisible (CIA chiffré ?)"))?;
    if ncch.encrypted {
        return Err(FormatError::Invalid("ROM 3DS chiffrée : déchiffre-la d'abord"));
    }
    if ncch.romfs_size == 0 {
        return Err(FormatError::Invalid("cette partition n'a pas de RomFS"));
    }
    let input_len = stream_len(&mut r)?;
    let mut header = read_at(&mut r, ncch.offset, NCCH_HEADER_SIZE)?;
    let content_mu = u32le(&header, 0x104) as u64;
    let romfs_mu = u32le(&header, 0x1B0) as u64;
    let old_romfs_size_mu = u32le(&header, 0x1B4) as u64;
    // Le RomFS est la dernière section d'un NCCH (3dbrew — NCCH, « NCCH Format »).
    if romfs_mu + old_romfs_size_mu != content_mu {
        return Err(FormatError::Invalid("le RomFS n'est pas la dernière section de la partition NCCH"));
    }
    if ncch.offset + content_mu * MEDIA_UNIT > input_len {
        return Err(FormatError::Invalid("partition NCCH tronquée"));
    }

    let cci = img.container == Container::Cci;
    let out_ncch = if cci { ncch.offset } else { 0 };
    let file = File::options().read(true).write(true).create(true).truncate(true).open(output)?;
    let mut w = BufWriter::with_capacity(BUF_SIZE, file);

    // Tout ce qui précède le RomFS est recopié tel quel (en-têtes, ExHeader, logo, ExeFS…).
    if cci {
        copy_range(&mut r, &mut w, 0, ncch.offset + romfs_mu * MEDIA_UNIT)?;
    } else {
        copy_range(&mut r, &mut w, ncch.offset, romfs_mu * MEDIA_UNIT)?;
    }
    let out_romfs = out_ncch + romfs_mu * MEDIA_UNIT;
    let mut built = build_romfs(&mut r, ncch.romfs_offset, files, &mut w, out_romfs)?;
    // Hachage du superbloc : relu dans le fichier écrit (avec de petits blocs, la
    // région peut déborder de l'en-tête IVFC sur les données).
    w.flush()?;
    built.superblock_hash = sha_range(w.get_mut(), out_romfs, built.hash_region_mu * MEDIA_UNIT)?;

    // En-tête NCCH : taille du contenu, du RomFS, région et hachage du superbloc.
    let new_romfs_mu = built.size / MEDIA_UNIT;
    let new_content_mu = romfs_mu + new_romfs_mu;
    patch_ncch_header(&mut header, new_content_mu, new_romfs_mu, &built)?;
    w.seek(SeekFrom::Start(out_ncch))?;
    w.write_all(&header)?;

    let mut size = out_ncch + new_content_mu * MEDIA_UNIT;
    if cci {
        size = relocate_partitions(&mut r, &mut w, ncch.offset, content_mu, new_content_mu, &built)?;
    }
    w.flush()?;
    let file = w.into_inner().map_err(|e| e.into_error())?;
    file.set_len(size)?;
    Ok(RebuildReport { container: if cci { Container::Cci } else { Container::Cxi }, size, romfs_size: built.size, replaced: files.len() })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn to_u32(v: u64) -> Result<u32> {
    u32::try_from(v).map_err(|_| FormatError::Invalid("image trop grande"))
}

fn patch_ncch_header(h: &mut [u8], content_mu: u64, romfs_mu: u64, built: &BuiltRomFs) -> Result<()> {
    h[0x104..0x108].copy_from_slice(&to_u32(content_mu)?.to_le_bytes());
    h[0x1B4..0x1B8].copy_from_slice(&to_u32(romfs_mu)?.to_le_bytes());
    h[0x1B8..0x1BC].copy_from_slice(&to_u32(built.hash_region_mu)?.to_le_bytes());
    h[0x1E0..0x200].copy_from_slice(&built.superblock_hash);
    Ok(())
}

/// CCI : décale les partitions qui suivent la partition 0, puis met à jour la table
/// des partitions, la taille de l'image et la taille remplie. Renvoie la taille du fichier.
fn relocate_partitions<R: Read + Seek, W: Write + Seek>(
    r: &mut R,
    w: &mut W,
    p0_offset: u64,
    old_mu: u64,
    new_mu: u64,
    built: &BuiltRomFs,
) -> Result<u64> {
    let mut h = read_at(r, 0, NCSD_HEADER_SIZE)?;
    let p0_mu = p0_offset / MEDIA_UNIT;
    let (old_end, new_end) = (p0_mu + old_mu, p0_mu + new_mu);
    let mut used_old = old_end;
    for i in 1..8 {
        let (off, len) = (u32le(&h, 0x120 + i * 8) as u64, u32le(&h, 0x124 + i * 8) as u64);
        if len == 0 {
            continue;
        }
        if off < old_end {
            return Err(FormatError::Invalid("partition placée avant la fin de la partition principale"));
        }
        used_old = used_old.max(off + len);
    }
    // Les partitions suivantes (et ce qui les sépare) sont recopiées d'un bloc, décalées.
    w.seek(SeekFrom::Start(new_end * MEDIA_UNIT))?;
    copy_range(r, w, old_end * MEDIA_UNIT, (used_old - old_end) * MEDIA_UNIT)?;
    let used_new = used_old + new_end - old_end;

    h[0x124..0x128].copy_from_slice(&to_u32(new_mu)?.to_le_bytes());
    for i in 1..8 {
        let (off, len) = (u32le(&h, 0x120 + i * 8) as u64, u32le(&h, 0x124 + i * 8) as u64);
        if len != 0 {
            h[0x120 + i * 8..0x124 + i * 8].copy_from_slice(&to_u32(off - old_end + new_end)?.to_le_bytes());
        }
    }
    // Taille remplie (en octets, 3dbrew — NCSD « Card Info Header » @0x300).
    h[0x300..0x304].copy_from_slice(&to_u32(used_new * MEDIA_UNIT)?.to_le_bytes());
    // Adresse de la sauvegarde CARD2 : si les données la dépassent, on la repousse
    // après elles (alignée sur 1 Mio). Les émulateurs ne s'en servent pas.
    let writable = u32le(&h, 0x200);
    let mut needed = used_new;
    if writable != u32::MAX && (writable as u64) < used_new {
        let moved = align(used_new * MEDIA_UNIT, 0x10_0000) / MEDIA_UNIT;
        h[0x200..0x204].copy_from_slice(&to_u32(moved)?.to_le_bytes());
        needed = moved + 1;
    }
    // Taille de l'image (capacité de la carte, une puissance de deux) : agrandie si besoin.
    let mut capacity = u32le(&h, 0x104) as u64;
    if capacity < needed {
        capacity = needed.next_power_of_two();
        h[0x104..0x108].copy_from_slice(&to_u32(capacity)?.to_le_bytes());
    }
    // Copie de l'en-tête NCCH de la partition 0 dans les InitialData, gardée cohérente.
    let copy = NCSD_NCCH_COPY;
    if &h[copy + 0x100..copy + 0x104] == b"NCCH" && h[copy + 0x118..copy + 0x120] == h[0x108..0x110] {
        patch_ncch_header(&mut h[copy..copy + NCCH_HEADER_SIZE], new_mu, built.size / MEDIA_UNIT, built)?;
    }
    w.seek(SeekFrom::Start(0))?;
    w.write_all(&h)?;
    Ok(used_new * MEDIA_UNIT)
}

/// Copie `len` octets de `r` (à partir de `from`) vers la position courante de `w`.
fn copy_range<R: Read + Seek, W: Write>(r: &mut R, w: &mut W, from: u64, len: u64) -> io::Result<()> {
    r.seek(SeekFrom::Start(from))?;
    let copied = io::copy(&mut r.by_ref().take(len), w)?;
    if copied != len {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "image tronquée"));
    }
    Ok(())
}

fn write_zeros<W: Write + ?Sized>(w: &mut W, mut n: u64) -> io::Result<()> {
    const ZEROS: [u8; 0x1000] = [0; 0x1000];
    while n > 0 {
        let k = n.min(ZEROS.len() as u64) as usize;
        w.write_all(&ZEROS[..k])?;
        n -= k as u64;
    }
    Ok(())
}

/// SHA-256 de chaque bloc de `block` octets (le dernier complété par des zéros).
pub(crate) fn hash_blocks(data: &[u8], block: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(block) * 32);
    for chunk in data.chunks(block) {
        let mut sha = Sha256::new();
        sha.update(chunk);
        if chunk.len() < block {
            sha.update(vec![0u8; block - chunk.len()]);
        }
        out.extend_from_slice(&sha.finalize());
    }
    out
}

/// Écrit en flux en calculant le SHA-256 de chaque bloc (niveau 3 → hachages du niveau 2).
struct BlockHasher<'a, W: Write> {
    w: &'a mut W,
    block: usize,
    fill: usize,
    sha: Sha256,
    hashes: Vec<u8>,
}

impl<'a, W: Write> BlockHasher<'a, W> {
    fn new(w: &'a mut W, block: usize, expected_blocks: u64) -> Self {
        Self { w, block, fill: 0, sha: Sha256::new(), hashes: Vec::with_capacity(expected_blocks as usize * 32) }
    }

    /// Complète le dernier bloc avec des zéros (écrits aussi) et renvoie les hachages.
    fn finish(mut self) -> io::Result<Vec<u8>> {
        if self.fill > 0 {
            let pad = (self.block - self.fill) as u64;
            write_zeros(&mut self, pad)?;
        }
        Ok(self.hashes)
    }
}

impl<W: Write> Write for BlockHasher<'_, W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.w.write_all(data)?;
        let mut rest = data;
        while !rest.is_empty() {
            let take = (self.block - self.fill).min(rest.len());
            self.sha.update(&rest[..take]);
            self.fill += take;
            rest = &rest[take..];
            if self.fill == self.block {
                self.hashes.extend_from_slice(&self.sha.finalize_reset());
                self.fill = 0;
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

/// RomFS reconstruit.
pub(crate) struct BuiltRomFs {
    /// Taille totale (multiple de l'unité de média).
    pub size: u64,
    pub hash_region_mu: u64,
    pub superblock_hash: [u8; 32],
}

/// Disposition des trois niveaux d'un RomFS (3dbrew — RomFS, « Format »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IvfcLayout {
    pub master_size: u64,
    /// (position logique, taille, log2 de la taille de bloc) des niveaux 1 à 3.
    pub levels: [(u64, u64, u32); 3],
}

impl IvfcLayout {
    pub(crate) fn parse(ivfc: &[u8]) -> Result<Self> {
        if &ivfc[..4] != b"IVFC" || u32le(ivfc, 4) != 0x10000 {
            return Err(FormatError::Invalid("RomFS illisible (en-tête IVFC absent : contenu chiffré ?)"));
        }
        let level = |at: usize| (u64le(ivfc, at), u64le(ivfc, at + 8), u32le(ivfc, at + 0x10));
        let layout = Self { master_size: u32le(ivfc, 0x08) as u64, levels: [level(0x0C), level(0x24), level(0x3C)] };
        if layout.levels.iter().any(|l| !(6..=20).contains(&l.2)) {
            return Err(FormatError::Invalid("taille de bloc IVFC invalide"));
        }
        Ok(layout)
    }

    pub(crate) fn block(&self, level: usize) -> u64 {
        1 << self.levels[level].2
    }

    /// Recalcule tailles et positions logiques pour un niveau 3 de `l3_size` octets.
    fn resized(&self, l3_size: u64) -> Self {
        let (b1, b2, b3) = (self.block(0), self.block(1), self.block(2));
        let l2_size = l3_size.div_ceil(b3) * 32;
        let l1_size = l2_size.div_ceil(b2) * 32;
        let master_size = l1_size.div_ceil(b1) * 32;
        let l1_logical = self.levels[0].0;
        let l2_logical = align(l1_logical + l1_size, b2);
        let l3_logical = align(l2_logical + l2_size, b3);
        Self {
            master_size,
            levels: [(l1_logical, l1_size, self.levels[0].2), (l2_logical, l2_size, self.levels[1].2), (l3_logical, l3_size, self.levels[2].2)],
        }
    }

    /// Positions physiques (relatives au RomFS) : niveau 3, niveau 1, niveau 2, fin.
    pub(crate) fn physical(&self) -> (u64, u64, u64, u64) {
        let (b1, b2, b3) = (self.block(0), self.block(1), self.block(2));
        let l3 = align(IVFC_HEADER_SIZE + self.master_size, b3);
        let l1 = align(align(l3 + self.levels[2].1, b3), b1);
        let l2 = align(align(l1 + self.levels[0].1, b1), b2);
        let end = align(align(l2 + self.levels[1].1, b2), MEDIA_UNIT);
        (l3, l1, l2, end)
    }

    /// Région couverte par le hachage du superbloc, en unités de média.
    pub(crate) fn hash_region_mu(&self) -> u64 {
        align(IVFC_HEADER_SIZE + self.master_size, MEDIA_UNIT) / MEDIA_UNIT
    }

    fn write_into(&self, ivfc: &mut [u8]) -> Result<()> {
        ivfc[0x08..0x0C].copy_from_slice(&to_u32(self.master_size)?.to_le_bytes());
        for (i, at) in [0x0Cusize, 0x24, 0x3C].into_iter().enumerate() {
            ivfc[at..at + 8].copy_from_slice(&self.levels[i].0.to_le_bytes());
            ivfc[at + 8..at + 16].copy_from_slice(&self.levels[i].1.to_le_bytes());
        }
        Ok(())
    }
}

/// Morceau de la nouvelle zone de données du niveau 3.
enum Chunk<'a> {
    /// Octets recopiés de l'original (fichier inchangé et son remplissage).
    Copy { from: u64, len: u64 },
    /// Fichier remplacé, suivi de `pad` octets nuls.
    New { data: &'a [u8], pad: u64 },
}

/// Lit le RomFS d'origine (à `src` dans `r`), remplace des fichiers et écrit le
/// nouveau RomFS à `dst` dans `w`, avec tous ses hachages.
pub(crate) fn build_romfs<R: Read + Seek, W: Write + Seek>(r: &mut R, src: u64, files: &[(&str, &[u8])], w: &mut W, dst: u64) -> Result<BuiltRomFs> {
    let mut ivfc = read_at(r, src, IVFC_HEADER_SIZE as usize)?;
    let old = IvfcLayout::parse(&ivfc)?;
    let l3_src = src + old.physical().0;
    let l3_old_size = old.levels[2].1;

    let head = read_at(r, l3_src, LEVEL3_HEADER_SIZE)?;
    if u32le(&head, 0) as usize != LEVEL3_HEADER_SIZE {
        return Err(FormatError::Invalid("en-tête du niveau 3 du RomFS invalide"));
    }
    let data_at = u32le(&head, 0x24) as u64;
    if data_at > l3_old_size {
        return Err(FormatError::Invalid("tables du RomFS hors du niveau 3"));
    }
    // Tables (dossiers, fichiers et leurs tables de hachage) : gardées, sauf position et taille des fichiers.
    let mut tables = read_at(r, l3_src, data_at as usize)?;
    let table = |at: usize| (u32le(&head, at) as usize, u32le(&head, at + 4) as usize);
    let (dir_at, dir_len) = table(0x0C);
    let (file_at, file_len) = table(0x1C);
    let dirs = tables.get(dir_at..dir_at + dir_len).ok_or(FormatError::Invalid("tables du RomFS hors du niveau 3"))?;
    let metas = tables.get(file_at..file_at + file_len).ok_or(FormatError::Invalid("tables du RomFS hors du niveau 3"))?;
    let entries = walk_tables(dirs, metas)?;

    let index: HashMap<&str, usize> = entries.iter().enumerate().map(|(i, (e, _))| (e.path.as_str(), i)).collect();
    let mut replaced: Vec<Option<&[u8]>> = vec![None; entries.len()];
    for &(path, data) in files {
        let i = *index.get(normalize(path)).ok_or_else(|| FormatError::NotFound(path.to_string()))?;
        replaced[i] = Some(data);
    }

    // Zone de données : fichiers dans l'ordre de leur position d'origine.
    let data_end = l3_old_size - data_at;
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&i| (entries[i].0.offset, entries[i].0.size, i));
    let mut chunks = Vec::with_capacity(order.len());
    let mut new_pos = vec![(0u64, 0u64); entries.len()];
    let mut shift: i128 = 0;
    let mut max_end = 0u64;
    for (k, &i) in order.iter().enumerate() {
        let e = &entries[i].0;
        let end = e.offset.checked_add(e.size).filter(|&end| end <= data_end).ok_or(FormatError::Invalid("fichier du RomFS hors des données"))?;
        let next = order.get(k + 1).map_or(data_end, |&j| entries[j].0.offset);
        let gap = next - e.offset;
        let new_off = u64::try_from(e.offset as i128 + shift).map_err(|_| FormatError::Invalid("RomFS incohérent"))?;
        let new_len = match replaced[i] {
            None => {
                chunks.push(Chunk::Copy { from: l3_src + data_at + e.offset, len: gap });
                new_pos[i] = (new_off, e.size);
                gap
            }
            Some(data) => {
                // Un fichier remplacé ne doit partager ses octets avec aucun autre.
                if (e.size > 0 && max_end > e.offset) || next < end {
                    return Err(FormatError::Invalid("fichier du RomFS partagé : remplacement impossible"));
                }
                let n = data.len() as u64;
                let len = n.max(gap + align(n, FILE_ALIGN) - align(e.size, FILE_ALIGN));
                chunks.push(Chunk::New { data, pad: len - n });
                new_pos[i] = (new_off, n);
                len
            }
        };
        max_end = max_end.max(end);
        shift += new_len as i128 - gap as i128;
    }
    for (i, (_, meta)) in entries.iter().enumerate() {
        let at = file_at + *meta as usize;
        tables[at + 0x08..at + 0x10].copy_from_slice(&new_pos[i].0.to_le_bytes());
        tables[at + 0x10..at + 0x18].copy_from_slice(&new_pos[i].1.to_le_bytes());
    }
    let l3_size = u64::try_from(l3_old_size as i128 + shift).map_err(|_| FormatError::Invalid("RomFS incohérent"))?;

    let layout = old.resized(l3_size);
    let (l3_phys, l1_phys, l2_phys, end) = layout.physical();

    // Niveau 3, haché au fil de l'écriture.
    w.seek(SeekFrom::Start(dst + l3_phys))?;
    let mut hasher = BlockHasher::new(w, layout.block(2) as usize, layout.levels[1].1 / 32);
    hasher.write_all(&tables)?;
    let mut buf = vec![0u8; BUF_SIZE];
    for chunk in &chunks {
        match *chunk {
            Chunk::Copy { from, mut len } => {
                r.seek(SeekFrom::Start(from))?;
                while len > 0 {
                    let k = len.min(buf.len() as u64) as usize;
                    r.read_exact(&mut buf[..k])?;
                    hasher.write_all(&buf[..k])?;
                    len -= k as u64;
                }
            }
            Chunk::New { data, pad } => {
                hasher.write_all(data)?;
                write_zeros(&mut hasher, pad)?;
            }
        }
    }
    let l2 = hasher.finish()?;
    debug_assert_eq!(l2.len() as u64, layout.levels[1].1);
    let l1 = hash_blocks(&l2, layout.block(1) as usize);
    let master = hash_blocks(&l1, layout.block(0) as usize);

    // Niveaux 1 et 2 après le niveau 3, chacun aligné sur sa taille de bloc.
    let l3_written = align(l3_size, layout.block(2));
    write_zeros(w, l1_phys - l3_phys - l3_written)?;
    w.write_all(&l1)?;
    write_zeros(w, l2_phys - l1_phys - l1.len() as u64)?;
    w.write_all(&l2)?;
    write_zeros(w, end - l2_phys - l2.len() as u64)?;

    // En-tête IVFC et hachages maîtres, au début du RomFS.
    layout.write_into(&mut ivfc)?;
    let mut head = ivfc;
    head.extend_from_slice(&master);
    head.resize(l3_phys as usize, 0);
    w.seek(SeekFrom::Start(dst))?;
    w.write_all(&head)?;
    w.seek(SeekFrom::Start(dst + end))?;
    Ok(BuiltRomFs { size: end, hash_region_mu: layout.hash_region_mu(), superblock_hash: [0; 32] })
}

/// Contrôle d'une partition NCCH : chaque hachage recalculé indépendamment.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NcchCheck {
    pub offset: u64,
    /// Partition chiffrée : non vérifiée.
    pub encrypted: bool,
    pub exheader_ok: Option<bool>,
    pub logo_ok: Option<bool>,
    pub exefs_superblock_ok: Option<bool>,
    /// Fichiers de l'ExeFS dont le SHA-256 correspond / nombre de fichiers.
    pub exefs_files: (usize, usize),
    pub romfs_superblock_ok: Option<bool>,
    /// Blocs vérifiés par niveau IVFC (1, 2, 3) et blocs en erreur.
    pub ivfc_blocks: [u64; 3],
    pub ivfc_bad: u64,
}

impl NcchCheck {
    pub fn ok(&self) -> bool {
        [self.exheader_ok, self.logo_ok, self.exefs_superblock_ok, self.romfs_superblock_ok].iter().all(|c| c.unwrap_or(true))
            && self.exefs_files.0 == self.exefs_files.1
            && self.ivfc_bad == 0
    }
}

/// Résultat de [`verify_image`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCheck {
    pub container: Container,
    pub partitions: Vec<NcchCheck>,
    /// Incohérences de structure (table des partitions, tailles).
    pub problems: Vec<String>,
}

impl ImageCheck {
    pub fn ok(&self) -> bool {
        self.problems.is_empty() && !self.partitions.is_empty() && self.partitions.iter().all(NcchCheck::ok)
    }
}

/// Vérifie une image déchiffrée (`.3ds` / `.cxi`) en relisant tout : table des
/// partitions, hachages de l'ExHeader, du logo, de l'ExeFS (superbloc et fichiers),
/// du superbloc RomFS, puis les trois niveaux IVFC bloc par bloc.
pub fn verify_image(path: &Path) -> Result<ImageCheck> {
    let mut r = BufReader::with_capacity(BUF_SIZE, File::open(path)?);
    let len = stream_len(&mut r)?;
    let h = read_at(&mut r, 0, NCCH_HEADER_SIZE)?;
    let mut problems = Vec::new();
    let mut partitions = Vec::new();
    let container = match &h[0x100..0x104] {
        b"NCSD" => {
            let mut used = 0;
            let mut spans = Vec::new();
            for i in 0..8 {
                let (off, size) = (u32le(&h, 0x120 + i * 8) as u64 * MEDIA_UNIT, u32le(&h, 0x124 + i * 8) as u64 * MEDIA_UNIT);
                if size == 0 {
                    continue;
                }
                if off + size > len {
                    problems.push(format!("partition {i} hors du fichier"));
                    continue;
                }
                spans.push((off, off + size, i));
                used = used.max(off + size);
                let ph = read_at(&mut r, off, NCCH_HEADER_SIZE)?;
                if &ph[0x100..0x104] != b"NCCH" {
                    problems.push(format!("partition {i} : en-tête NCCH absent"));
                } else if u32le(&ph, 0x104) as u64 * MEDIA_UNIT != size {
                    problems.push(format!("partition {i} : taille différente de la table NCSD"));
                } else {
                    partitions.push(verify_ncch(&mut r, off)?);
                }
            }
            spans.sort();
            if spans.windows(2).any(|s| s[0].1 > s[1].0) {
                problems.push("partitions qui se chevauchent".into());
            }
            if u32le(&h, 0x104) as u64 * MEDIA_UNIT < used {
                problems.push("taille de l'image inférieure aux données".into());
            }
            Container::Cci
        }
        b"NCCH" => {
            if u32le(&h, 0x104) as u64 * MEDIA_UNIT > len {
                problems.push("partition NCCH tronquée".into());
            } else {
                partitions.push(verify_ncch(&mut r, 0)?);
            }
            Container::Cxi
        }
        _ => return Err(FormatError::Invalid("ni NCSD ni NCCH")),
    };
    Ok(ImageCheck { container, partitions, problems })
}

fn sha_range<R: Read + Seek>(r: &mut R, from: u64, len: u64) -> Result<[u8; 32]> {
    r.seek(SeekFrom::Start(from))?;
    let mut sha = Sha256::new();
    let copied = io::copy(&mut r.by_ref().take(len), &mut sha)?;
    if copied != len {
        return Err(FormatError::Invalid("zone hors du fichier"));
    }
    Ok(sha.finalize().into())
}

fn verify_ncch<R: Read + Seek>(r: &mut R, offset: u64) -> Result<NcchCheck> {
    let h = read_at(r, offset, NCCH_HEADER_SIZE)?;
    let mu = |at: usize| u32le(&h, at) as u64 * MEDIA_UNIT;
    let mut c = NcchCheck { offset, ..Default::default() };
    if h[0x18F] & 0x04 == 0 {
        // Partition chiffrée (ex. données de mise à jour) : rien à recalculer en clair.
        c.encrypted = true;
        return Ok(c);
    }
    // ExHeader : SHA-256 de 0x400 octets (ExHeader + descripteur d'accès), juste après l'en-tête.
    if u32le(&h, 0x180) > 0 {
        c.exheader_ok = Some(sha_range(r, offset + 0x200, 0x400)? == h[0x160..0x180]);
    }
    if mu(0x19C) > 0 {
        c.logo_ok = Some(sha_range(r, offset + mu(0x198), mu(0x19C))? == h[0x130..0x150]);
    }
    if mu(0x1A4) > 0 {
        let exefs = offset + mu(0x1A0);
        c.exefs_superblock_ok = Some(sha_range(r, exefs, mu(0x1A8))? == h[0x1C0..0x1E0]);
        // 3dbrew — ExeFS : 10 entrées (nom, position, taille), hachages en ordre inverse à 0xC0.
        let eh = read_at(r, exefs, 0x200)?;
        for i in 0..10 {
            let e = &eh[i * 0x10..i * 0x10 + 0x10];
            if e[..8].iter().all(|&b| b == 0) {
                continue;
            }
            c.exefs_files.1 += 1;
            let hash = &eh[0xC0 + (9 - i) * 0x20..0xE0 + (9 - i) * 0x20];
            if sha_range(r, exefs + 0x200 + u32le(e, 8) as u64, u32le(e, 12) as u64)? == hash {
                c.exefs_files.0 += 1;
            }
        }
    }
    if mu(0x1B4) > 0 {
        let romfs = offset + mu(0x1B0);
        c.romfs_superblock_ok = Some(sha_range(r, romfs, mu(0x1B8))? == h[0x1E0..0x200]);
        let ivfc = read_at(r, romfs, IVFC_HEADER_SIZE as usize)?;
        let layout = IvfcLayout::parse(&ivfc)?;
        let (l3, l1, l2, end) = layout.physical();
        if end > mu(0x1B4) {
            return Err(FormatError::Invalid("niveaux IVFC hors du RomFS"));
        }
        // Chaque niveau est vérifié avec les hachages du niveau au-dessus (maître → 1 → 2 → 3).
        let mut upper = read_at(r, romfs + IVFC_HEADER_SIZE, layout.master_size as usize)?;
        for (level, phys) in [(0usize, l1), (1, l2), (2, l3)] {
            let block = layout.block(level);
            let size = layout.levels[level].1;
            let blocks = size.div_ceil(block);
            let keep = level < 2;
            let mut data = Vec::with_capacity(if keep { size as usize } else { 0 });
            r.seek(SeekFrom::Start(romfs + phys))?;
            let mut buf = vec![0u8; block as usize];
            for b in 0..blocks {
                r.read_exact(&mut buf)?;
                let expected = upper.get(b as usize * 32..b as usize * 32 + 32);
                if expected != Some(&Sha256::digest(&buf)[..]) {
                    c.ivfc_bad += 1;
                }
                if keep {
                    let n = (size - b * block).min(block) as usize;
                    data.extend_from_slice(&buf[..n]);
                }
            }
            c.ivfc_blocks[level] = blocks;
            upper = data;
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::romfs::tests::tiny_romfs;
    use crate::romfs::RomFsSource;

    /// Image CCI minimale : NCSD, partition 0 (NCCH + ExeFS + RomFS), partition 1 factice.
    fn tiny_cci() -> Vec<u8> {
        let romfs = tiny_romfs(0);
        let romfs_mu = align(romfs.len() as u64, MEDIA_UNIT) / MEDIA_UNIT;
        // NCCH : en-tête (1 unité), ExeFS (2 unités), RomFS.
        let mut ncch = vec![0u8; 0x200];
        ncch[0x100..0x104].copy_from_slice(b"NCCH");
        ncch[0x118..0x120].copy_from_slice(&0x0004_0000_0011_C400u64.to_le_bytes());
        ncch[0x18F] = 0x04;
        let mut exefs = vec![0u8; 0x400];
        exefs[..5].copy_from_slice(b".code");
        exefs[0x0C..0x10].copy_from_slice(&4u32.to_le_bytes());
        exefs[0x200..0x204].copy_from_slice(b"CODE");
        let code_hash = Sha256::digest(b"CODE");
        exefs[0x1E0..0x200].copy_from_slice(&code_hash);
        let exefs_hash = Sha256::digest(&exefs[..0x200]);
        for (at, v) in [(0x1A0, 1u32), (0x1A4, 2), (0x1A8, 1), (0x1B0, 3), (0x1B4, romfs_mu as u32), (0x104, 3 + romfs_mu as u32)] {
            ncch[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        ncch[0x1C0..0x1E0].copy_from_slice(&exefs_hash);
        ncch.extend(exefs);
        ncch.extend(&romfs);
        ncch.resize(((3 + romfs_mu) * MEDIA_UNIT) as usize, 0);

        let p0 = 0x4000u64;
        let p0_mu = (p0 / MEDIA_UNIT) as u32;
        let p0_len = (ncch.len() as u64 / MEDIA_UNIT) as u32;
        let mut img = vec![0u8; p0 as usize];
        img[0x100..0x104].copy_from_slice(b"NCSD");
        img[0x104..0x108].copy_from_slice(&0x1000u32.to_le_bytes());
        img[0x108..0x110].copy_from_slice(&0x0004_0000_0011_C400u64.to_le_bytes());
        img[0x120..0x124].copy_from_slice(&p0_mu.to_le_bytes());
        img[0x124..0x128].copy_from_slice(&p0_len.to_le_bytes());
        // Partition 1 : un NCCH vide (sans section), juste après.
        img[0x128..0x12C].copy_from_slice(&(p0_mu + p0_len).to_le_bytes());
        img[0x12C..0x130].copy_from_slice(&1u32.to_le_bytes());
        img[0x200..0x204].copy_from_slice(&u32::MAX.to_le_bytes());
        img[0x1100..0x1200].copy_from_slice(&ncch[0x100..0x200]);
        img.extend(ncch);
        let mut p1 = vec![0u8; 0x200];
        p1[0x100..0x104].copy_from_slice(b"NCCH");
        p1[0x104..0x108].copy_from_slice(&1u32.to_le_bytes());
        p1[0x18F] = 0x04;
        p1[0x150..0x154].copy_from_slice(b"MANU");
        img.extend(p1);
        img
    }

    #[test]
    fn rebuild_tiny_cci() {
        let tmp = std::env::temp_dir().join(format!("kaleido-ctr-build-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let input = tmp.join("in.3ds");
        std::fs::write(&input, tiny_cci()).unwrap();

        // Sans changement : hachages recalculés, contenu identique.
        let same = tmp.join("same.3ds");
        rebuild_image(&input, &same, &[]).unwrap();
        let check = verify_image(&same).unwrap();
        assert!(check.ok(), "{check:?}");
        assert_eq!(check.partitions.len(), 2);
        assert_eq!(check.partitions[0].exefs_files, (1, 1));

        // Fichier agrandi : la partition 1 est décalée et toujours lisible.
        let big = vec![0x42u8; 0x3000];
        let out = tmp.join("out.3ds");
        let report = rebuild_image(&input, &out, &[("a.bin", &big), ("/d/b.bin", b"CCC")]).unwrap();
        assert_eq!(report.container, Container::Cci);
        assert_eq!(report.size, std::fs::metadata(&out).unwrap().len());
        let check = verify_image(&out).unwrap();
        assert!(check.ok(), "{check:?}");
        let src = RomFsSource::open(&out).unwrap();
        assert_eq!(src.read("a.bin").unwrap(), big);
        assert_eq!(src.read("d/b.bin").unwrap(), b"CCC");
        let data = std::fs::read(&out).unwrap();
        let p1 = u32le(&data, 0x128) as usize * 0x200;
        assert_eq!(&data[p1 + 0x150..p1 + 0x154], b"MANU");
        assert_eq!(u32le(&data, 0x300) as usize, p1 + 0x200);
        // La copie de l'en-tête NCCH (InitialData) suit l'en-tête réel.
        assert_eq!(data[0x1100..0x1200], data[0x4100..0x4200]);

        // CXI : la partition principale seule.
        let cxi = tmp.join("out.cxi");
        std::fs::write(&cxi, &data[0x4000..p1]).unwrap();
        let again = tmp.join("again.cxi");
        let report = rebuild_image(&cxi, &again, &[("a.bin", b"x")]).unwrap();
        assert_eq!(report.container, Container::Cxi);
        assert!(verify_image(&again).unwrap().ok());
        assert_eq!(RomFsSource::open(&again).unwrap().read("a.bin").unwrap(), b"x");

        assert!(rebuild_image(&input, &tmp.join("x.3ds"), &[("absent.bin", b"")]).is_err());
        assert!(rebuild_image(&input, &input, &[]).is_err());
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn ivfc_layout_matches_retail() {
        // Valeurs relevées sur Rubis Oméga (Europe, Rev 2).
        let old = IvfcLayout { master_size: 0, levels: [(0, 0, 12), (0, 0, 12), (0, 0, 12)] };
        let l = old.resized(0x6F96_B3C8);
        assert_eq!(l.master_size, 0x380);
        assert_eq!(l.levels[0], (0, 0x1BE60, 12));
        assert_eq!(l.levels[1], (0x1C000, 0xDF2D80, 12));
        assert_eq!(l.levels[2], (0xE0F000, 0x6F96_B3C8, 12));
        assert_eq!(l.physical(), (0x1000, 0x6F96_D000, 0x6F98_9000, 0x7077_C000));
        assert_eq!(l.hash_region_mu(), 2);
    }
}
