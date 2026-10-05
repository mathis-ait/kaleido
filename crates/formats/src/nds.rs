//! ROMs Nintendo DS (`.nds`) : en-tête, système de fichiers NitroFS, overlays.
//!
//! Référence : GBATEK, « DS Cartridge Header » et « DS Cartridge NitroROM and NitroARC File Systems ».

use std::collections::BTreeMap;
use std::io::{Read, Seek};
use std::path::Path;

use serde::Serialize;

use crate::util::{align, ascii, read_at, slice, stream_len, u16le, u32le};
use crate::{lz, FormatError, Result};

pub const HEADER_SIZE: usize = 0x200;

/// CRC du logo Nintendo, identique sur toutes les cartouches officielles.
const LOGO_CRC: u16 = 0xCF56;
/// Taille de la signature RSA « download play » placée après la ROM utilisée.
const RSA_SIGNATURE_SIZE: u32 = 0x88;
const FILE_ALIGN: u32 = 0x200;
const OVERLAY_ENTRY_SIZE: usize = 32;
const OVERLAY_COMPRESSED: u32 = 0x0100_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NdsHeader {
    /// Titre interne, ex. `POKEMON PL`.
    pub title: String,
    /// Code jeu sur 4 caractères, ex. `CPUF` (le dernier = région / langue).
    pub game_code: String,
    pub maker_code: String,
    /// 0 = DS, 2 = DS + DSi, 3 = DSi uniquement.
    pub unit_code: u8,
    pub rom_version: u8,
    pub arm9_offset: u32,
    pub arm9_size: u32,
    pub arm7_offset: u32,
    pub arm7_size: u32,
    pub fnt_offset: u32,
    pub fnt_size: u32,
    pub fat_offset: u32,
    pub fat_size: u32,
    pub arm9_overlay_offset: u32,
    pub arm9_overlay_size: u32,
    pub arm7_overlay_offset: u32,
    pub arm7_overlay_size: u32,
    pub banner_offset: u32,
    /// Taille utilisée de la ROM, en octets.
    pub used_rom_size: u32,
    /// Fin de la zone DS sur les ROMs « DSi Enhanced » (0 sinon).
    pub ntr_region_end: u32,
    pub header_crc_ok: bool,
}

impl NdsHeader {
    /// Renvoie `None` si le flux ne ressemble pas à une ROM DS.
    pub fn probe<R: Read + Seek>(r: &mut R) -> Result<Option<Self>> {
        if stream_len(r)? < HEADER_SIZE as u64 {
            return Ok(None);
        }
        let h = read_at(r, 0, HEADER_SIZE)?;
        if u16le(&h, 0x15C) != LOGO_CRC {
            return Ok(None);
        }
        Ok(Some(Self::parse(&h)))
    }

    pub fn parse(h: &[u8]) -> Self {
        let unit_code = h[0x012];
        Self {
            title: ascii(&h[0x000..0x00C]),
            game_code: ascii(&h[0x00C..0x010]),
            maker_code: ascii(&h[0x010..0x012]),
            unit_code,
            rom_version: h[0x01E],
            arm9_offset: u32le(h, 0x020),
            arm9_size: u32le(h, 0x02C),
            arm7_offset: u32le(h, 0x030),
            arm7_size: u32le(h, 0x03C),
            fnt_offset: u32le(h, 0x040),
            fnt_size: u32le(h, 0x044),
            fat_offset: u32le(h, 0x048),
            fat_size: u32le(h, 0x04C),
            arm9_overlay_offset: u32le(h, 0x050),
            arm9_overlay_size: u32le(h, 0x054),
            arm7_overlay_offset: u32le(h, 0x058),
            arm7_overlay_size: u32le(h, 0x05C),
            banner_offset: u32le(h, 0x068),
            used_rom_size: u32le(h, 0x080),
            ntr_region_end: if unit_code & 0x02 != 0 { u16le(h, 0x090) as u32 * 0x80000 } else { 0 },
            header_crc_ok: crc16(&h[..0x15E]) == u16le(h, 0x15E),
        }
    }

    /// Lettre de région (`F` = France, `E` = USA, `P` = Europe anglais…).
    pub fn region(&self) -> Option<char> {
        self.game_code.chars().nth(3)
    }
}

/// CRC-16/MODBUS, utilisé par l'en-tête DS.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

/// Entrée de la table des overlays ARM9.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    pub id: u32,
    pub ram_address: u32,
    pub ram_size: u32,
    pub bss_size: u32,
    pub file_id: u16,
    /// Bits 0-23 : taille compressée ; bit 24 : compressé (BLZ).
    pub flags: u32,
}

impl Overlay {
    fn parse(c: &[u8]) -> Self {
        Self {
            id: u32le(c, 0),
            ram_address: u32le(c, 4),
            ram_size: u32le(c, 8),
            bss_size: u32le(c, 12),
            file_id: u32le(c, 24) as u16,
            flags: u32le(c, 28),
        }
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & OVERLAY_COMPRESSED != 0
    }
}

/// ROM DS chargée en mémoire, avec ses fichiers modifiables.
///
/// Les fichiers remplacés sont gardés à part ; `to_bytes` reconstruit une ROM
/// en réutilisant l'emplacement d'origine quand il y a la place, sinon en
/// déplaçant le fichier après les données existantes.
pub struct NdsRom {
    data: Vec<u8>,
    header: NdsHeader,
    fat: Vec<(u32, u32)>,
    paths: Vec<Option<String>>,
    overlays: Vec<Overlay>,
    replaced: BTreeMap<u16, Vec<u8>>,
}

impl NdsRom {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_bytes(std::fs::read(path)?)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let h = data.get(..HEADER_SIZE).ok_or(FormatError::Invalid("ROM DS tronquée"))?;
        if u16le(h, 0x15C) != LOGO_CRC {
            return Err(FormatError::Invalid("ce n'est pas une ROM DS"));
        }
        let header = NdsHeader::parse(h);

        let fat: Vec<(u32, u32)> = slice(&data, header.fat_offset, header.fat_size)?
            .chunks_exact(8)
            .map(|c| (u32le(c, 0), u32le(c, 4)))
            .collect();
        if fat.iter().any(|&(s, e)| s > e || e as usize > data.len()) {
            return Err(FormatError::Invalid("table FAT incohérente"));
        }

        let paths = parse_fnt(slice(&data, header.fnt_offset, header.fnt_size)?, fat.len())?;
        let overlays = slice(&data, header.arm9_overlay_offset, header.arm9_overlay_size)?
            .chunks_exact(OVERLAY_ENTRY_SIZE)
            .map(Overlay::parse)
            .collect();

        Ok(Self { data, header, fat, paths, overlays, replaced: BTreeMap::new() })
    }

    pub fn header(&self) -> &NdsHeader {
        &self.header
    }

    pub fn file_count(&self) -> usize {
        self.fat.len()
    }

    /// Fichiers nommés du système de fichiers (les overlays n'ont pas de nom).
    pub fn files(&self) -> impl Iterator<Item = (u16, &str)> {
        self.paths.iter().enumerate().filter_map(|(id, p)| p.as_deref().map(|p| (id as u16, p)))
    }

    pub fn file_id(&self, path: &str) -> Option<u16> {
        let path = path.trim_start_matches('/');
        self.files().find(|(_, p)| *p == path).map(|(id, _)| id)
    }

    pub fn file(&self, id: u16) -> Option<&[u8]> {
        if let Some(data) = self.replaced.get(&id) {
            return Some(data);
        }
        let &(s, e) = self.fat.get(id as usize)?;
        Some(&self.data[s as usize..e as usize])
    }

    pub fn file_by_path(&self, path: &str) -> Result<&[u8]> {
        self.file_id(path)
            .and_then(|id| self.file(id))
            .ok_or_else(|| FormatError::NotFound(path.to_string()))
    }

    pub fn replace_file(&mut self, id: u16, data: Vec<u8>) -> Result<()> {
        if id as usize >= self.fat.len() {
            return Err(FormatError::NotFound(format!("fichier n°{id}")));
        }
        self.replaced.insert(id, data);
        Ok(())
    }

    pub fn replace_file_by_path(&mut self, path: &str, data: Vec<u8>) -> Result<()> {
        let id = self.file_id(path).ok_or_else(|| FormatError::NotFound(path.to_string()))?;
        self.replace_file(id, data)
    }

    pub fn arm9(&self) -> &[u8] {
        let start = self.header.arm9_offset as usize;
        &self.data[start..start + self.header.arm9_size as usize]
    }

    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }

    /// Contenu décompressé d'un overlay ARM9.
    pub fn overlay(&self, id: u32) -> Result<Vec<u8>> {
        let ovl = self.find_overlay(id)?;
        let raw = self.file(ovl.file_id).ok_or_else(|| FormatError::NotFound(format!("overlay {id}")))?;
        if ovl.is_compressed() {
            lz::decompress_blz(raw)
        } else {
            Ok(raw.to_vec())
        }
    }

    /// Remplace un overlay. Il est écrit non compressé : le jeu le charge tel quel.
    pub fn replace_overlay(&mut self, id: u32, data: Vec<u8>) -> Result<()> {
        let idx = self.overlays.iter().position(|o| o.id == id).ok_or_else(|| FormatError::NotFound(format!("overlay {id}")))?;
        let ovl = &mut self.overlays[idx];
        if data.len() as u32 > ovl.ram_size {
            return Err(FormatError::Invalid("overlay plus grand que sa zone mémoire"));
        }
        ovl.flags &= !(OVERLAY_COMPRESSED | 0x00FF_FFFF);
        let file_id = ovl.file_id;
        self.replace_file(file_id, data)
    }

    fn find_overlay(&self, id: u32) -> Result<&Overlay> {
        self.overlays.iter().find(|o| o.id == id).ok_or_else(|| FormatError::NotFound(format!("overlay {id}")))
    }

    /// Reconstruit l'image complète de la ROM.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = self.data.clone();
        if self.replaced.is_empty() {
            return Ok(out);
        }
        let h = &self.header;

        // Débuts de toutes les zones occupées, pour savoir jusqu'où un fichier peut grandir sur place.
        let mut starts: Vec<u32> = self.fat.iter().filter(|(s, e)| e > s).map(|&(s, _)| s).collect();
        starts.extend(
            [h.arm9_offset, h.arm7_offset, h.fnt_offset, h.fat_offset, h.arm9_overlay_offset, h.arm7_overlay_offset, h.banner_offset]
                .into_iter()
                .filter(|&o| o != 0),
        );
        starts.sort_unstable();
        starts.dedup();

        let data_end = self.fat.iter().map(|&(_, e)| e).max().unwrap_or(0).max(h.used_rom_size + RSA_SIGNATURE_SIZE);
        let mut append = align(data_end as u64, FILE_ALIGN as u64) as u32;
        let limit = if h.ntr_region_end != 0 { h.ntr_region_end } else { u32::MAX };

        let mut fat = self.fat.clone();
        for (&id, new) in &self.replaced {
            let (s, e) = fat[id as usize];
            let len = u32::try_from(new.len()).map_err(|_| FormatError::Invalid("fichier trop grand"))?;
            let slot_end = if e > s { starts.iter().copied().find(|&x| x > s).unwrap_or(e) } else { s };

            let start = if s + len <= slot_end && e > s {
                // Sur place : on efface l'ancien contenu au-delà de la nouvelle taille.
                out[(s + len) as usize..e.max(s + len) as usize].fill(0xFF);
                s
            } else {
                let start = append;
                append = align((start + len) as u64, FILE_ALIGN as u64) as u32;
                if start + len > limit {
                    return Err(FormatError::Invalid("plus de place dans la zone DS de la ROM"));
                }
                if out.len() < (start + len) as usize {
                    out.resize((start + len) as usize, 0xFF);
                }
                start
            };
            out[start as usize..(start + len) as usize].copy_from_slice(new);
            fat[id as usize] = (start, start + len);
        }

        // FAT, table des overlays, puis en-tête.
        let fat_at = h.fat_offset as usize;
        for (i, &(s, e)) in fat.iter().enumerate() {
            out[fat_at + i * 8..fat_at + i * 8 + 4].copy_from_slice(&s.to_le_bytes());
            out[fat_at + i * 8 + 4..fat_at + i * 8 + 8].copy_from_slice(&e.to_le_bytes());
        }
        let ovt_at = h.arm9_overlay_offset as usize;
        for (i, ovl) in self.overlays.iter().enumerate() {
            let at = ovt_at + i * OVERLAY_ENTRY_SIZE + 28;
            out[at..at + 4].copy_from_slice(&ovl.flags.to_le_bytes());
        }

        let used = fat.iter().map(|&(_, e)| e).max().unwrap_or(0).max(h.used_rom_size);
        out[0x80..0x84].copy_from_slice(&used.to_le_bytes());
        if h.ntr_region_end == 0 {
            // Capacité de la puce : 128 Kio << n.
            let mut capacity = out[0x14];
            while (0x20000u64 << capacity) < out.len() as u64 {
                capacity += 1;
            }
            out[0x14] = capacity;
        }
        let crc = crc16(&out[..0x15E]);
        out[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        Ok(out)
    }
}

/// Lit la table des noms (FNT) et renvoie le chemin de chaque identifiant de fichier.
fn parse_fnt(fnt: &[u8], file_count: usize) -> Result<Vec<Option<String>>> {
    const BAD: FormatError = FormatError::Invalid("table des noms (FNT) incohérente");
    let byte = |at: usize| fnt.get(at).copied().ok_or(BAD);

    let mut paths = vec![None; file_count];
    let dir_count = u16le(slice(fnt, 0, 8)?, 6) as usize;
    let mut stack = vec![(0usize, String::new())];
    let mut visited = 0;

    while let Some((dir, prefix)) = stack.pop() {
        visited += 1;
        if visited > dir_count {
            return Err(BAD);
        }
        let entry = slice(fnt, (dir * 8) as u32, 8)?;
        let mut at = u32le(entry, 0) as usize;
        let mut file_id = u16le(entry, 4) as usize;

        loop {
            let len_type = byte(at)?;
            at += 1;
            if len_type == 0 {
                break;
            }
            let len = (len_type & 0x7F) as usize;
            let name = String::from_utf8_lossy(fnt.get(at..at + len).ok_or(BAD)?);
            at += len;
            let path = if prefix.is_empty() { name.into_owned() } else { format!("{prefix}/{name}") };

            if len_type & 0x80 != 0 {
                let sub = (u16le(fnt.get(at..at + 2).ok_or(BAD)?, 0) & 0x0FFF) as usize;
                at += 2;
                if sub >= dir_count {
                    return Err(BAD);
                }
                stack.push((sub, path));
            } else {
                if let Some(slot) = paths.get_mut(file_id) {
                    *slot = Some(path);
                }
                file_id += 1;
            }
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn crc16_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    fn synthetic_header() -> Vec<u8> {
        let mut h = vec![0u8; HEADER_SIZE];
        h[..10].copy_from_slice(b"POKEMON PL");
        h[0x0C..0x10].copy_from_slice(b"CPUF");
        h[0x10..0x12].copy_from_slice(b"01");
        h[0x15C..0x15E].copy_from_slice(&LOGO_CRC.to_le_bytes());
        let crc = crc16(&h[..0x15E]);
        h[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        h
    }

    #[test]
    fn parse_synthetic_header() {
        let header = NdsHeader::probe(&mut Cursor::new(synthetic_header())).unwrap().unwrap();
        assert_eq!(header.title, "POKEMON PL");
        assert_eq!(header.game_code, "CPUF");
        assert_eq!(header.region(), Some('F'));
        assert!(header.header_crc_ok);
    }

    #[test]
    fn rejects_non_nds() {
        let data = vec![0u8; HEADER_SIZE];
        assert!(NdsHeader::probe(&mut Cursor::new(data)).unwrap().is_none());
    }

    /// ROM minimale : racine contenant `a.bin` et `d/b.bin`.
    fn tiny_rom() -> Vec<u8> {
        let mut rom = synthetic_header();
        rom.resize(0x1000, 0xFF);

        let mut fnt = Vec::new();
        // Table principale : racine (sous-table @0x10, 1er fichier 0, 2 dossiers) et `d`.
        fnt.extend(0x10u32.to_le_bytes());
        fnt.extend(0u16.to_le_bytes());
        fnt.extend(2u16.to_le_bytes());
        fnt.extend(0x1Bu32.to_le_bytes());
        fnt.extend(1u16.to_le_bytes());
        fnt.extend(0xF000u16.to_le_bytes());
        fnt.extend([0x05, b'a', b'.', b'b', b'i', b'n', 0x81, b'd', 0x01, 0xF0, 0x00]);
        fnt.extend([0x05, b'b', b'.', b'b', b'i', b'n', 0x00]);

        rom[0x400..0x400 + fnt.len()].copy_from_slice(&fnt);
        rom[0x40..0x44].copy_from_slice(&0x400u32.to_le_bytes());
        rom[0x44..0x48].copy_from_slice(&(fnt.len() as u32).to_le_bytes());

        let fat = [(0x800u32, 0x804u32), (0xA00, 0xA02)];
        for (i, (s, e)) in fat.iter().enumerate() {
            rom[0x600 + i * 8..0x604 + i * 8].copy_from_slice(&s.to_le_bytes());
            rom[0x604 + i * 8..0x608 + i * 8].copy_from_slice(&e.to_le_bytes());
        }
        rom[0x48..0x4C].copy_from_slice(&0x600u32.to_le_bytes());
        rom[0x4C..0x50].copy_from_slice(&16u32.to_le_bytes());
        rom[0x800..0x804].copy_from_slice(b"AAAA");
        rom[0xA00..0xA02].copy_from_slice(b"BB");
        rom[0x80..0x84].copy_from_slice(&0xA02u32.to_le_bytes());
        rom
    }

    #[test]
    fn nitrofs_paths_and_files() {
        let rom = NdsRom::from_bytes(tiny_rom()).unwrap();
        let files: Vec<_> = rom.files().collect();
        assert_eq!(files, vec![(0, "a.bin"), (1, "d/b.bin")]);
        assert_eq!(rom.file_by_path("/d/b.bin").unwrap(), b"BB");
    }

    #[test]
    fn rebuild_in_place_and_moved() {
        let original = tiny_rom();
        let mut rom = NdsRom::from_bytes(original.clone()).unwrap();
        assert_eq!(rom.to_bytes().unwrap(), original);

        rom.replace_file(0, b"CC".to_vec()).unwrap(); // plus petit : sur place
        rom.replace_file(1, vec![7; 0x900]).unwrap(); // plus grand : déplacé
        let rebuilt = NdsRom::from_bytes(rom.to_bytes().unwrap()).unwrap();
        assert_eq!(rebuilt.file(0).unwrap(), b"CC");
        assert_eq!(rebuilt.file(1).unwrap(), &vec![7; 0x900][..]);
        assert!(rebuilt.header().header_crc_ok);
    }
}
