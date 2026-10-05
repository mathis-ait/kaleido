//! Conteneurs Nintendo 3DS (« CTR ») : CCI (`.3ds`), CIA et CXI (NCCH brut).
//!
//! Référence : 3dbrew — NCSD, NCCH, CIA, Title metadata, ExHeader.

use std::io::{Read, Seek};

use serde::Serialize;

use crate::util::{align, ascii, read_at, stream_len, u32be, u32le, u64be, u64le};
use crate::Result;

/// Unité de média des en-têtes NCSD / NCCH.
pub const MEDIA_UNIT: u64 = 0x200;

const NCCH_HEADER_SIZE: usize = 0x200;
const CIA_HEADER_SIZE: u32 = 0x2020;
const CIA_ALIGN: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    /// Image de cartouche (`.3ds` / `.cci`), magic `NCSD`.
    Cci,
    /// Paquet installable (`.cia`).
    Cia,
    /// Partition NCCH brute (`.cxi`).
    Cxi,
}

impl Container {
    pub fn label(self) -> &'static str {
        match self {
            Container::Cci => "CCI (.3ds)",
            Container::Cia => "CIA",
            Container::Cxi => "CXI",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NcchInfo {
    pub program_id: u64,
    /// Ex. `CTR-P-EKJA`.
    pub product_code: String,
    /// `true` si le contenu est chiffré (flag `NoCrypto` absent).
    pub encrypted: bool,
    /// Position absolue de la partition NCCH dans le fichier.
    pub offset: u64,
    /// RomFS : position absolue et taille en octets (taille 0 si absent).
    pub romfs_offset: u64,
    pub romfs_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CtrImage {
    pub container: Container,
    pub title_id: u64,
    /// `None` si la partition principale est illisible (contenu CIA chiffré).
    pub ncch: Option<NcchInfo>,
}

impl CtrImage {
    pub fn probe<R: Read + Seek>(r: &mut R) -> Result<Option<Self>> {
        let len = stream_len(r)?;
        if len < NCCH_HEADER_SIZE as u64 {
            return Ok(None);
        }
        let h = read_at(r, 0, NCCH_HEADER_SIZE)?;
        match &h[0x100..0x104] {
            b"NCSD" => {
                let partition0 = u32le(&h, 0x120) as u64 * MEDIA_UNIT;
                let ncch = read_ncch(r, partition0, len)?;
                let title_id = ncch.as_ref().map_or(u64le(&h, 0x108), |n| n.program_id);
                Ok(Some(Self { container: Container::Cci, title_id, ncch }))
            }
            b"NCCH" => {
                let ncch = parse_ncch(&h, 0);
                Ok(Some(Self { container: Container::Cxi, title_id: ncch.program_id, ncch: Some(ncch) }))
            }
            _ => probe_cia(r, &h, len),
        }
    }
}

fn parse_ncch(h: &[u8], offset: u64) -> NcchInfo {
    let romfs_size = u32le(h, 0x1B4) as u64 * MEDIA_UNIT;
    NcchInfo {
        program_id: u64le(h, 0x118),
        product_code: ascii(&h[0x150..0x160]),
        encrypted: h[0x18F] & 0x04 == 0,
        offset,
        romfs_offset: if romfs_size == 0 { 0 } else { offset + u32le(h, 0x1B0) as u64 * MEDIA_UNIT },
        romfs_size,
    }
}

fn read_ncch<R: Read + Seek>(r: &mut R, offset: u64, len: u64) -> Result<Option<NcchInfo>> {
    if offset + NCCH_HEADER_SIZE as u64 > len {
        return Ok(None);
    }
    let h = read_at(r, offset, NCCH_HEADER_SIZE)?;
    Ok((&h[0x100..0x104] == b"NCCH").then(|| parse_ncch(&h, offset)))
}

fn probe_cia<R: Read + Seek>(r: &mut R, h: &[u8], len: u64) -> Result<Option<CtrImage>> {
    if u32le(h, 0x00) != CIA_HEADER_SIZE {
        return Ok(None);
    }
    let cert = u32le(h, 0x08) as u64;
    let ticket = u32le(h, 0x0C) as u64;
    let tmd = u32le(h, 0x10) as u64;

    let tmd_offset = align(CIA_HEADER_SIZE as u64, CIA_ALIGN) + align(cert, CIA_ALIGN) + align(ticket, CIA_ALIGN);
    let content_offset = tmd_offset + align(tmd, CIA_ALIGN);
    if tmd == 0 || content_offset > len {
        return Ok(None);
    }

    // Le TMD commence par un bloc de signature de taille variable.
    let sig_type = u32be(&read_at(r, tmd_offset, 4)?, 0);
    let Some(sig_block) = signature_block_size(sig_type) else {
        return Ok(None);
    };
    let tmd_header = tmd_offset + 4 + sig_block;
    let title_id = u64be(&read_at(r, tmd_header + 0x4C, 8)?, 0);

    let ncch = read_ncch(r, content_offset, len)?;
    Ok(Some(CtrImage { container: Container::Cia, title_id, ncch }))
}

/// Taille signature + remplissage pour un type de signature 3DS.
fn signature_block_size(sig_type: u32) -> Option<u64> {
    Some(match sig_type {
        0x010000 | 0x010003 => 0x200 + 0x3C,
        0x010001 | 0x010004 => 0x100 + 0x3C,
        0x010002 | 0x010005 => 0x3C + 0x40,
        _ => return None,
    })
}

/// Program ID contenu dans un `exheader.bin` extrait (début de l'ACI, offset 0x200).
pub fn exheader_program_id(exheader: &[u8]) -> Option<u64> {
    (exheader.len() >= 0x208).then(|| u64le(exheader, 0x200))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn ncch_header(program_id: u64, no_crypto: bool) -> Vec<u8> {
        let mut h = vec![0u8; NCCH_HEADER_SIZE];
        h[0x100..0x104].copy_from_slice(b"NCCH");
        h[0x118..0x120].copy_from_slice(&program_id.to_le_bytes());
        h[0x150..0x15A].copy_from_slice(b"CTR-P-EKJA");
        if no_crypto {
            h[0x18F] = 0x04;
        }
        h[0x1B0..0x1B4].copy_from_slice(&0x10u32.to_le_bytes());
        h[0x1B4..0x1B8].copy_from_slice(&0x20u32.to_le_bytes());
        h
    }

    #[test]
    fn probe_cci() {
        let mut data = vec![0u8; 0x4000 + NCCH_HEADER_SIZE];
        data[0x100..0x104].copy_from_slice(b"NCSD");
        data[0x120..0x124].copy_from_slice(&(0x4000u32 / MEDIA_UNIT as u32).to_le_bytes());
        data[0x4000..].copy_from_slice(&ncch_header(0x0004_0000_0005_5D00, true));

        let img = CtrImage::probe(&mut Cursor::new(data)).unwrap().unwrap();
        assert_eq!(img.container, Container::Cci);
        assert_eq!(img.title_id, 0x0004_0000_0005_5D00);
        let ncch = img.ncch.unwrap();
        assert!(!ncch.encrypted);
        assert_eq!(ncch.product_code, "CTR-P-EKJA");
        assert_eq!(ncch.offset, 0x4000);
        assert_eq!((ncch.romfs_offset, ncch.romfs_size), (0x4000 + 0x10 * MEDIA_UNIT, 0x20 * MEDIA_UNIT));
    }

    #[test]
    fn probe_cxi_encrypted() {
        let img = CtrImage::probe(&mut Cursor::new(ncch_header(42, false))).unwrap().unwrap();
        assert_eq!(img.container, Container::Cxi);
        assert!(img.ncch.unwrap().encrypted);
    }

    #[test]
    fn probe_cia_reads_title_id_from_tmd() {
        let (cert, ticket, tmd) = (0xA00u64, 0x350u64, 0xB34u64);
        let tmd_offset = align(0x2020, 64) + align(cert, 64) + align(ticket, 64);
        let content_offset = tmd_offset + align(tmd, 64);
        let mut data = vec![0u8; (content_offset as usize) + NCCH_HEADER_SIZE];

        data[0..4].copy_from_slice(&CIA_HEADER_SIZE.to_le_bytes());
        data[0x08..0x0C].copy_from_slice(&(cert as u32).to_le_bytes());
        data[0x0C..0x10].copy_from_slice(&(ticket as u32).to_le_bytes());
        data[0x10..0x14].copy_from_slice(&(tmd as u32).to_le_bytes());

        let t = tmd_offset as usize;
        data[t..t + 4].copy_from_slice(&0x010004u32.to_be_bytes());
        let id_at = t + 4 + 0x13C + 0x4C;
        data[id_at..id_at + 8].copy_from_slice(&0x0004_0000_0016_4800u64.to_be_bytes());
        data[content_offset as usize..].copy_from_slice(&ncch_header(0x0004_0000_0016_4800, true));

        let img = CtrImage::probe(&mut Cursor::new(data)).unwrap().unwrap();
        assert_eq!(img.container, Container::Cia);
        assert_eq!(img.title_id, 0x0004_0000_0016_4800);
        assert!(img.ncch.is_some());
    }
}
