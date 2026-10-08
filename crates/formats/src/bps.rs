//! Patchs BPS (beat), format courant des romhacks avec xdelta et IPS.
//!
//! Structure : `BPS1`, taille source, taille cible, taille des métadonnées (entiers
//! variables), métadonnées, actions, puis CRC32 de la source, de la cible et du patch.

use crate::{FormatError, Result};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8> {
        let b = *self.data.get(self.pos).ok_or_else(|| FormatError::Invalid("patch BPS tronqué"))?;
        self.pos += 1;
        Ok(b)
    }

    fn number(&mut self) -> Result<u64> {
        let mut value = 0u64;
        let mut shift = 1u64;
        loop {
            let b = self.byte()?;
            value = value.checked_add((b as u64 & 0x7F).checked_mul(shift).ok_or_else(|| FormatError::Invalid("patch BPS invalide"))?).ok_or_else(|| FormatError::Invalid("patch BPS invalide"))?;
            if b & 0x80 != 0 {
                return Ok(value);
            }
            shift <<= 7;
            value += shift;
        }
    }
}

/// Applique un patch BPS. La source doit correspondre exactement (CRC vérifié).
pub fn apply(source: &[u8], patch: &[u8]) -> Result<Vec<u8>> {
    if patch.len() < 4 + 12 || !patch.starts_with(b"BPS1") {
        return Err(FormatError::Invalid("pas un patch BPS"));
    }
    let footer = patch.len() - 12;
    let le = |i: usize| u32::from_le_bytes(patch[i..i + 4].try_into().unwrap());
    let (source_crc, target_crc, patch_crc) = (le(footer), le(footer + 4), le(footer + 8));
    if crc32(&patch[..footer + 8]) != patch_crc {
        return Err(FormatError::Invalid("patch BPS abîmé (somme de contrôle)"));
    }
    if crc32(source) != source_crc {
        return Err(FormatError::Invalid("ce patch BPS est prévu pour une autre ROM (version ou région différente)"));
    }
    let mut r = Reader { data: &patch[..footer], pos: 4 };
    let source_size = r.number()? as usize;
    let target_size = r.number()? as usize;
    let meta = r.number()? as usize;
    r.pos += meta;
    if source_size != source.len() {
        return Err(FormatError::Invalid("taille de la ROM source inattendue"));
    }
    let mut out: Vec<u8> = Vec::with_capacity(target_size);
    let (mut source_rel, mut target_rel) = (0i64, 0i64);
    let bad = || FormatError::Invalid("patch BPS invalide");
    while r.pos < footer {
        let data = r.number()?;
        let length = (data >> 2) as usize + 1;
        match data & 3 {
            0 => {
                let start = out.len();
                out.extend_from_slice(source.get(start..start + length).ok_or_else(bad)?);
            }
            1 => {
                let bytes = r.data.get(r.pos..r.pos + length).ok_or_else(bad)?;
                out.extend_from_slice(bytes);
                r.pos += length;
            }
            mode => {
                let n = r.number()?;
                let delta = if n & 1 != 0 { -((n >> 1) as i64) } else { (n >> 1) as i64 };
                if mode == 2 {
                    source_rel += delta;
                    let start = usize::try_from(source_rel).map_err(|_| bad())?;
                    out.extend_from_slice(source.get(start..start + length).ok_or_else(bad)?);
                    source_rel += length as i64;
                } else {
                    target_rel += delta;
                    // Copie octet par octet : la zone lue peut chevaucher celle écrite.
                    for _ in 0..length {
                        let i = usize::try_from(target_rel).map_err(|_| bad())?;
                        let b = *out.get(i).ok_or_else(bad)?;
                        out.push(b);
                        target_rel += 1;
                    }
                }
            }
        }
    }
    if out.len() != target_size || crc32(&out) != target_crc {
        return Err(FormatError::Invalid("résultat du patch BPS incorrect"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let x = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(0x80 | x);
                return;
            }
            out.push(x);
            v -= 1;
        }
    }

    #[test]
    fn applies_all_actions() {
        let source = b"ABCDEFGH".to_vec();
        let target = b"ABCDxyzEFxyz".to_vec();
        let mut p = b"BPS1".to_vec();
        number(source.len() as u64, &mut p);
        number(target.len() as u64, &mut p);
        number(0, &mut p);
        number((4 - 1) << 2, &mut p); // SourceRead « ABCD »
        number(((3 - 1) << 2) | 1, &mut p); // TargetRead « xyz »
        p.extend_from_slice(b"xyz");
        number(((2 - 1) << 2) | 2, &mut p); // SourceCopy « EF » (décalage +4)
        number(4 << 1, &mut p);
        number(((3 - 1) << 2) | 3, &mut p); // TargetCopy « xyz » depuis 4
        number(4 << 1, &mut p);
        p.extend_from_slice(&crc32(&source).to_le_bytes());
        p.extend_from_slice(&crc32(&target).to_le_bytes());
        let c = crc32(&p);
        p.extend_from_slice(&c.to_le_bytes());
        assert_eq!(apply(&source, &p).unwrap(), target);
        assert!(apply(b"ABCDEFGX", &p).is_err());
    }
}
