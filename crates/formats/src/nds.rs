//! En-tête des ROMs Nintendo DS (`.nds`).
//!
//! Référence : GBATEK, « DS Cartridge Header ».

use std::io::{Read, Seek};

use serde::Serialize;

use crate::util::{ascii, read_at, stream_len, u16le, u32le};
use crate::Result;

pub const HEADER_SIZE: usize = 0x200;

/// CRC du logo Nintendo, identique sur toutes les cartouches officielles.
const LOGO_CRC: u16 = 0xCF56;

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
    pub fnt_offset: u32,
    pub fnt_size: u32,
    pub fat_offset: u32,
    pub fat_size: u32,
    /// Taille utilisée de la ROM, en octets.
    pub used_rom_size: u32,
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
        Self {
            title: ascii(&h[0x000..0x00C]),
            game_code: ascii(&h[0x00C..0x010]),
            maker_code: ascii(&h[0x010..0x012]),
            unit_code: h[0x012],
            rom_version: h[0x01E],
            fnt_offset: u32le(h, 0x040),
            fnt_size: u32le(h, 0x044),
            fat_offset: u32le(h, 0x048),
            fat_size: u32le(h, 0x04C),
            used_rom_size: u32le(h, 0x080),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn crc16_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn parse_synthetic_header() {
        let mut h = vec![0u8; HEADER_SIZE];
        h[..10].copy_from_slice(b"POKEMON PL");
        h[0x0C..0x10].copy_from_slice(b"CPUF");
        h[0x10..0x12].copy_from_slice(b"01");
        h[0x15C..0x15E].copy_from_slice(&LOGO_CRC.to_le_bytes());
        let crc = crc16(&h[..0x15E]);
        h[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());

        let header = NdsHeader::probe(&mut Cursor::new(h)).unwrap().unwrap();
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
}
