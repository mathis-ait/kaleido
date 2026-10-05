//! Compressions du BIOS DS : LZ10 / LZ11 (`0x10` / `0x11`) et BLZ (« backward LZ »,
//! utilisée pour l'ARM9 et les overlays).

use crate::{FormatError, Result};

const TRUNCATED: FormatError = FormatError::Invalid("données compressées tronquées");
const BAD_DISTANCE: FormatError = FormatError::Invalid("référence LZ hors des données");

/// Décompresse un bloc LZ10 ou LZ11 d'après son premier octet.
pub fn decompress(d: &[u8]) -> Result<Vec<u8>> {
    match d.first() {
        Some(0x10) => decompress_lz10(d),
        Some(0x11) => decompress_lz11(d),
        _ => Err(FormatError::Invalid("compression LZ inconnue")),
    }
}

pub fn is_lz(d: &[u8]) -> bool {
    matches!(d.first(), Some(0x10 | 0x11)) && d.len() >= 4
}

/// Taille décompressée et position du premier bloc.
fn header(d: &[u8]) -> Result<(usize, usize)> {
    let b = d.get(..4).ok_or(TRUNCATED)?;
    let size = u32::from_le_bytes([b[1], b[2], b[3], 0]) as usize;
    if size != 0 {
        return Ok((size, 4));
    }
    let b = d.get(4..8).ok_or(TRUNCATED)?;
    Ok((u32::from_le_bytes(b.try_into().unwrap()) as usize, 8))
}

fn copy_back(out: &mut Vec<u8>, disp: usize, len: usize) -> Result<()> {
    if disp > out.len() {
        return Err(BAD_DISTANCE);
    }
    for _ in 0..len {
        out.push(out[out.len() - disp]);
    }
    Ok(())
}

pub fn decompress_lz10(d: &[u8]) -> Result<Vec<u8>> {
    let (size, mut pos) = header(d)?;
    let byte = |at: usize| d.get(at).copied().ok_or(TRUNCATED);
    let mut out = Vec::with_capacity(size);

    while out.len() < size {
        let flags = byte(pos)?;
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(byte(pos)?);
                pos += 1;
            } else {
                let (b1, b2) = (byte(pos)? as usize, byte(pos + 1)? as usize);
                pos += 2;
                copy_back(&mut out, ((b1 & 0xF) << 8 | b2) + 1, (b1 >> 4) + 3)?;
            }
        }
    }
    out.truncate(size);
    Ok(out)
}

pub fn decompress_lz11(d: &[u8]) -> Result<Vec<u8>> {
    let (size, mut pos) = header(d)?;
    let byte = |at: usize| d.get(at).copied().map(usize::from).ok_or(TRUNCATED);
    let mut out = Vec::with_capacity(size);

    while out.len() < size {
        let flags = byte(pos)?;
        pos += 1;
        for bit in (0..8).rev() {
            if out.len() >= size {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(byte(pos)? as u8);
                pos += 1;
                continue;
            }
            let b1 = byte(pos)?;
            let (len, disp) = match b1 >> 4 {
                0 => {
                    let (b2, b3) = (byte(pos + 1)?, byte(pos + 2)?);
                    pos += 3;
                    (((b1 & 0xF) << 4 | b2 >> 4) + 0x11, ((b2 & 0xF) << 8 | b3) + 1)
                }
                1 => {
                    let (b2, b3, b4) = (byte(pos + 1)?, byte(pos + 2)?, byte(pos + 3)?);
                    pos += 4;
                    (((b1 & 0xF) << 12 | b2 << 4 | b3 >> 4) + 0x111, ((b3 & 0xF) << 8 | b4) + 1)
                }
                n => {
                    let b2 = byte(pos + 1)?;
                    pos += 2;
                    (n + 1, ((b1 & 0xF) << 8 | b2) + 1)
                }
            };
            copy_back(&mut out, disp, len)?;
        }
    }
    out.truncate(size);
    Ok(out)
}

/// Compression LZ10 gloutonne (fenêtre 4 Kio, distance ≥ 2 pour rester compatible VRAM).
pub fn compress_lz10(src: &[u8]) -> Vec<u8> {
    const MAX_LEN: usize = 18;
    const WINDOW: usize = 0x1000;

    let mut out = vec![0x10];
    out.extend(&(src.len() as u32).to_le_bytes()[..3]);

    let mut pos = 0;
    while pos < src.len() {
        let flag_at = out.len();
        out.push(0);
        for bit in (0..8).rev() {
            if pos >= src.len() {
                break;
            }
            let (len, disp) = longest_match(src, pos, WINDOW, MAX_LEN);
            if len >= 3 {
                out[flag_at] |= 1 << bit;
                let d = disp - 1;
                out.push((((len - 3) << 4) | (d >> 8)) as u8);
                out.push(d as u8);
                pos += len;
            } else {
                out.push(src[pos]);
                pos += 1;
            }
        }
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

fn longest_match(src: &[u8], pos: usize, window: usize, max_len: usize) -> (usize, usize) {
    let max_len = max_len.min(src.len() - pos);
    let mut best = (0, 0);
    for disp in 2..=window.min(pos) {
        let start = pos - disp;
        let len = (0..max_len).take_while(|&i| src[start + i] == src[pos + i]).count();
        if len > best.0 {
            best = (len, disp);
            if len == max_len {
                break;
            }
        }
    }
    best
}

/// Décompression BLZ (algorithme de CUE, `blz.c`) : les données sont lues à l'envers
/// depuis la fin du fichier, qui contient un pied de 8 octets.
pub fn decompress_blz(d: &[u8]) -> Result<Vec<u8>> {
    let len = d.len();
    if len < 8 {
        return Err(TRUNCATED);
    }
    let inc_len = u32::from_le_bytes(d[len - 4..].try_into().unwrap()) as usize;
    if inc_len == 0 {
        return Ok(d.to_vec());
    }
    let hdr_len = d[len - 5] as usize;
    let enc_len = u32::from_le_bytes(d[len - 8..len - 4].try_into().unwrap()) as usize & 0x00FF_FFFF;
    if enc_len > len || hdr_len > enc_len {
        return Err(FormatError::Invalid("pied BLZ incohérent"));
    }
    let dec_len = len - enc_len;
    let pak_len = enc_len - hdr_len;
    let raw_len = dec_len + enc_len + inc_len;

    let pak: Vec<u8> = d[dec_len..dec_len + pak_len].iter().rev().copied().collect();
    let mut raw = Vec::with_capacity(raw_len - dec_len);
    let target = raw_len - dec_len;

    let mut p = 0;
    let (mut flags, mut mask) = (0u8, 0u8);
    while raw.len() < target {
        mask >>= 1;
        if mask == 0 {
            let Some(&f) = pak.get(p) else { break };
            flags = f;
            p += 1;
            mask = 0x80;
        }
        if flags & mask == 0 {
            let Some(&b) = pak.get(p) else { break };
            raw.push(b);
            p += 1;
        } else {
            if p + 1 >= pak.len() {
                break;
            }
            let pos = (pak[p] as usize) << 8 | pak[p + 1] as usize;
            p += 2;
            let n = ((pos >> 12) + 3).min(target - raw.len());
            let disp = (pos & 0xFFF) + 3;
            if disp > raw.len() {
                return Err(BAD_DISTANCE);
            }
            for _ in 0..n {
                raw.push(raw[raw.len() - disp]);
            }
        }
    }

    let mut out = Vec::with_capacity(raw_len);
    out.extend_from_slice(&d[..dec_len]);
    out.extend(raw.iter().rev());
    out.resize(raw_len, 0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz10_roundtrip() {
        let src: Vec<u8> = b"Bulbizarre Herbizarre Florizarre ".repeat(20).into_iter().chain(0..=255).collect();
        let packed = compress_lz10(&src);
        assert!(packed.len() < src.len());
        assert_eq!(decompress(&packed).unwrap(), src);
    }

    #[test]
    fn lz11_short_and_long_matches() {
        // Littéral 'a', puis copie de 0x20 octets (forme 0), puis 3 octets (forme courte).
        let mut d = vec![0x11, 0x24, 0x00, 0x00, 0b0110_0000, b'a'];
        let len = 0x20 - 0x11;
        d.extend([(len >> 4) as u8, ((len & 0xF) << 4) as u8, 0x00]);
        d.extend([0x20, 0x00]);
        assert_eq!(decompress(&d).unwrap(), vec![b'a'; 0x24]);
    }

    #[test]
    fn blz_uncompressed_passthrough() {
        let d = vec![1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decompress_blz(&d).unwrap(), d);
    }
}
