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
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    out
}

/// Bloc LZ11 probable (utilisé par les jeux 3DS dans certaines entrées GARC).
pub fn is_lz11(d: &[u8]) -> bool {
    d.first() == Some(&0x11) && d.len() >= 4
}

/// Compression LZ11 (fenêtre 4 Kio, copies de 3 à 65 808 octets). Les
/// correspondances sont cherchées par chaînes de hachage, ce qui reste rapide
/// sur des fichiers de plusieurs Mo.
pub fn compress_lz11(src: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 0x1000;
    const MAX_LEN: usize = 0x10110;

    let mut out = vec![0x11];
    // Une taille nulle sur 3 octets annonce l'en-tête étendu : on l'utilise aussi pour un fichier vide.
    if (1..=0xFF_FFFF).contains(&src.len()) {
        out.extend(&(src.len() as u32).to_le_bytes()[..3]);
    } else {
        out.extend([0, 0, 0]);
        out.extend((src.len() as u32).to_le_bytes());
    }

    let mut finder = MatchFinder::new(src.len());
    let mut pos = 0;
    while pos < src.len() {
        let flag_at = out.len();
        out.push(0);
        for bit in (0..8).rev() {
            if pos >= src.len() {
                break;
            }
            let (len, disp) = finder.longest(src, pos, WINDOW, MAX_LEN);
            if len < 3 {
                out.push(src[pos]);
                finder.insert(src, pos);
                pos += 1;
                continue;
            }
            out[flag_at] |= 1 << bit;
            let d = disp - 1;
            let low = (d & 0xFF) as u8;
            let high = (d >> 8) as u8;
            if len <= 0x10 {
                out.extend([((len - 1) << 4) as u8 | high, low]);
            } else if len <= 0x110 {
                let l = len - 0x11;
                out.extend([(l >> 4) as u8, ((l & 0xF) << 4) as u8 | high, low]);
            } else {
                let l = len - 0x111;
                out.extend([0x10 | (l >> 12) as u8, (l >> 4) as u8, ((l & 0xF) << 4) as u8 | high, low]);
            }
            for p in pos..pos + len {
                finder.insert(src, p);
            }
            pos += len;
        }
    }
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
    out
}

/// Recherche de correspondances par hachage des 3 octets suivants.
struct MatchFinder {
    head: Vec<u32>,
    prev: Vec<u32>,
}

impl MatchFinder {
    const BITS: u32 = 15;
    const NONE: u32 = u32::MAX;
    const MAX_CHAIN: usize = 256;

    fn new(len: usize) -> Self {
        Self { head: vec![Self::NONE; 1 << Self::BITS], prev: vec![Self::NONE; len] }
    }

    fn hash(src: &[u8], pos: usize) -> Option<usize> {
        let b = src.get(pos..pos + 3)?;
        let v = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        Some((v.wrapping_mul(2_654_435_761) >> (32 - Self::BITS)) as usize)
    }

    fn insert(&mut self, src: &[u8], pos: usize) {
        if let Some(h) = Self::hash(src, pos) {
            self.prev[pos] = self.head[h];
            self.head[h] = pos as u32;
        }
    }

    fn longest(&self, src: &[u8], pos: usize, window: usize, max_len: usize) -> (usize, usize) {
        let Some(h) = Self::hash(src, pos) else { return (0, 0) };
        let max_len = max_len.min(src.len() - pos);
        let mut best = (0, 0);
        let mut cand = self.head[h];
        for _ in 0..Self::MAX_CHAIN {
            if cand == Self::NONE || pos - cand as usize > window {
                break;
            }
            let start = cand as usize;
            let len = src[start..].iter().zip(&src[pos..pos + max_len]).take_while(|(a, b)| a == b).count();
            if len > best.0 {
                best = (len, pos - start);
                if len == max_len {
                    break;
                }
            }
            cand = self.prev[start];
        }
        best
    }
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

/// Compression BLZ (portage de `BLZ_Code` de CUE, mode « best »). Les `keep`
/// premiers octets ne sont jamais compressés (0x4000 pour un ARM9, comme Nintendo).
pub fn compress_blz(input: &[u8], keep: usize) -> Vec<u8> {
    const THRESHOLD: usize = 2;
    const WINDOW: usize = 0x1002;
    const MAX_LEN: usize = 0x12;

    let raw_len = input.len();
    let raw_new = raw_len.saturating_sub(keep.min(raw_len));
    let raw: Vec<u8> = input.iter().rev().copied().collect();

    // Plus longue correspondance à la position `at` (distance ≥ 3), limitée à `end`.
    let search = |at: usize, end: usize| -> (usize, usize) {
        let (mut best_len, mut best_pos) = (THRESHOLD, 0);
        let max = at.min(WINDOW);
        for pos in 3..=max {
            let mut len = 0;
            while len < MAX_LEN && at + len < end && len < pos && raw[at + len] == raw[at + len - pos] {
                len += 1;
            }
            if len > best_len {
                best_len = len;
                best_pos = pos;
                if len == MAX_LEN {
                    break;
                }
            }
        }
        (best_len, best_pos)
    };

    let mut pak: Vec<u8> = Vec::with_capacity(raw_len + raw_len / 8 + 11);
    let (mut pak_tmp, mut raw_tmp) = (0usize, raw_len);
    let mut at = 0;
    let mut flag_at = 0;
    let mut mask = 0u8;
    while at < raw_new {
        mask >>= 1;
        if mask == 0 {
            flag_at = pak.len();
            pak.push(0);
            mask = 0x80;
        }
        let (mut len_best, pos_best) = search(at, raw_new);
        // Optimisation LZ-CUE : vérifie qu'un littéral suivi d'une copie ne serait pas meilleur.
        if len_best > THRESHOLD && at + len_best < raw_new {
            let (mut len_next, _) = search(at + len_best, raw_new);
            let (mut len_post, _) = search(at + 1, raw_new);
            if len_next <= THRESHOLD {
                len_next = 1;
            }
            if len_post <= THRESHOLD {
                len_post = 1;
            }
            if len_best + len_next <= 1 + len_post {
                len_best = 1;
            }
        }
        pak[flag_at] <<= 1;
        if len_best > THRESHOLD {
            at += len_best;
            pak[flag_at] |= 1;
            pak.push((((len_best - (THRESHOLD + 1)) << 4) | ((pos_best - 3) >> 8)) as u8);
            pak.push(((pos_best - 3) & 0xFF) as u8);
        } else {
            pak.push(raw[at]);
            at += 1;
        }
        if pak.len() + raw_len - at < pak_tmp + raw_tmp {
            pak_tmp = pak.len();
            raw_tmp = raw_len - at;
        }
    }
    while mask != 0 && mask != 1 {
        mask >>= 1;
        pak[flag_at] <<= 1;
    }
    let pak_len = pak.len();
    pak.reverse();

    if pak_tmp == 0 || raw_len + 4 < ((pak_tmp + raw_tmp + 3) & !3) + 8 {
        // Compression inutile : données brutes + pied nul.
        let mut out = input.to_vec();
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out.extend([0; 4]);
        return out;
    }
    let mut out = Vec::with_capacity(raw_tmp + pak_tmp + 11);
    out.extend_from_slice(&input[..raw_tmp]);
    out.extend_from_slice(&pak[pak_len - pak_tmp..]);
    let enc_len = pak_tmp;
    let mut hdr_len = 8;
    let inc_len = raw_len - pak_tmp - raw_tmp;
    while !out.len().is_multiple_of(4) {
        out.push(0xFF);
        hdr_len += 1;
    }
    out.extend(&((enc_len + hdr_len) as u32).to_le_bytes()[..3]);
    out.push(hdr_len as u8);
    out.extend(((inc_len - hdr_len) as u32).to_le_bytes());
    out
}

/// Modifie l'octet `out_pos` des données **décompressées** directement dans un bloc
/// BLZ, sans recompresser : possible si cet octet est stocké tel quel (littéral) et
/// qu'aucune copie ne le réutilise. La taille du bloc ne change pas.
pub fn blz_patch_byte(d: &mut [u8], out_pos: usize, value: u8) -> Result<()> {
    const NOT_PATCHABLE: FormatError = FormatError::Invalid("octet non modifiable dans le code compressé");
    let len = d.len();
    if len < 8 {
        return Err(TRUNCATED);
    }
    let inc_len = u32::from_le_bytes(d[len - 4..].try_into().unwrap()) as usize;
    if inc_len == 0 {
        // Pas compressé : modification directe.
        *d.get_mut(out_pos).ok_or(NOT_PATCHABLE)? = value;
        return Ok(());
    }
    let hdr_len = d[len - 5] as usize;
    let enc_len = u32::from_le_bytes(d[len - 8..len - 4].try_into().unwrap()) as usize & 0x00FF_FFFF;
    if enc_len > len || hdr_len > enc_len {
        return Err(FormatError::Invalid("pied BLZ incohérent"));
    }
    let dec_len = len - enc_len;
    if out_pos < dec_len {
        d[out_pos] = value; // partie laissée en clair au début
        return Ok(());
    }
    let pak_len = enc_len - hdr_len;
    let target = enc_len + inc_len;
    // Index dans `d` de l'octet compressé lu en position `p` (lecture à rebours).
    let src = |p: usize| dec_len + pak_len - 1 - p;

    let mut raw: Vec<u8> = Vec::with_capacity(target);
    let mut literal: Vec<Option<usize>> = Vec::with_capacity(target);
    let mut referenced: Vec<bool> = Vec::with_capacity(target);
    let (mut p, mut flags, mut mask) = (0usize, 0u8, 0u8);
    while raw.len() < target {
        mask >>= 1;
        if mask == 0 {
            if p >= pak_len {
                break;
            }
            flags = d[src(p)];
            p += 1;
            mask = 0x80;
        }
        if flags & mask == 0 {
            if p >= pak_len {
                break;
            }
            raw.push(d[src(p)]);
            literal.push(Some(src(p)));
            referenced.push(false);
            p += 1;
        } else {
            if p + 1 >= pak_len {
                break;
            }
            let pos = (d[src(p)] as usize) << 8 | d[src(p + 1)] as usize;
            p += 2;
            let n = ((pos >> 12) + 3).min(target - raw.len());
            let disp = (pos & 0xFFF) + 3;
            if disp > raw.len() {
                return Err(BAD_DISTANCE);
            }
            for _ in 0..n {
                let s = raw.len() - disp;
                referenced[s] = true;
                raw.push(raw[s]);
                literal.push(None);
                referenced.push(false);
            }
        }
    }
    // La sortie finale est `raw` à l'envers, après la partie en clair.
    let r = (dec_len + raw.len()).checked_sub(1 + out_pos).filter(|&r| r < raw.len()).ok_or(NOT_PATCHABLE)?;
    match literal[r] {
        Some(i) if !referenced[r] => {
            d[i] = value;
            Ok(())
        }
        _ => Err(NOT_PATCHABLE),
    }
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
    fn lz11_roundtrip_all_lengths() {
        let mut src: Vec<u8> = b"Salameche Reptincel Dracaufeu ".repeat(40);
        src.extend(vec![0xAB; 0x200]); // copie longue (forme 0 et forme 1)
        src.extend(vec![0xCD; 0x12000]);
        src.extend((0..=255u8).cycle().take(5000));
        let packed = compress_lz11(&src);
        assert!(is_lz11(&packed) && packed.len() < src.len() / 10);
        assert_eq!(decompress(&packed).unwrap(), src);
        assert_eq!(decompress(&compress_lz11(b"ab")).unwrap(), b"ab");
        assert_eq!(decompress(&compress_lz11(&[])).unwrap(), b"");
    }

    #[test]
    fn blz_roundtrip() {
        let src: Vec<u8> = (0..20_000u32).map(|i| ((i * 7) % 251) as u8 ^ if i % 97 < 40 { 0 } else { (i / 13) as u8 }).collect();
        let packed = compress_blz(&src, 0x400);
        assert!(packed.len() < src.len());
        assert_eq!(&packed[..0x400], &src[..0x400]);
        assert_eq!(decompress_blz(&packed).unwrap(), src);
    }

    #[test]
    fn blz_uncompressed_passthrough() {
        let d = vec![1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(decompress_blz(&d).unwrap(), d);
    }
}
