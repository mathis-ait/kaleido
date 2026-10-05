//! Sommes de contrôle CRC-16 des sauvegardes.

/// CRC-16/CCITT-FALSE (polynôme 0x1021, valeur initiale 0xFFFF, sans réflexion) :
/// blocs des sauvegardes Gen 4, Gen 5 et Gen 6 (`Checksums.CRC16_CCITT` de PKHeX).
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

/// CRC-16 réfléchi (polynôme 0xA001), valeur initiale 0xFFFF et résultat inversé
/// (CRC-16/USB) : blocs des sauvegardes Gen 7 (`Checksums.CRC16Invert` de PKHeX).
pub fn crc16_invert(data: &[u8]) -> u16 {
    !kaleido_formats::nds::crc16(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_values() {
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);
        assert_eq!(crc16_invert(b"123456789"), 0xB4C8);
    }
}
