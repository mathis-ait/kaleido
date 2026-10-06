//! Patchs IPS : `PATCH`, puis des enregistrements (position sur 3 octets, taille
//! sur 2 octets, données, gros-boutiste), terminés par `EOF`. Luma3DS et les
//! émulateurs 3DS (Azahar, Citra) appliquent un `code.ips` au programme du jeu
//! décompressé.

/// Position interdite : ses trois octets s'écrivent « EOF », la fin du patch.
const EOF_OFFSET: usize = 0x45_4F46;
const MAX_OFFSET: usize = 0xFF_FFFF;
const MAX_RECORD: usize = 0xFFFF;

/// Patch IPS qui transforme `original` en `patched` (même taille, au plus 16 Mio).
pub fn create(original: &[u8], patched: &[u8]) -> crate::Result<Vec<u8>> {
    if original.len() != patched.len() {
        return Err(crate::FormatError::Invalid("IPS : les deux fichiers doivent avoir la même taille"));
    }
    let mut out = b"PATCH".to_vec();
    let mut i = 0;
    while i < patched.len() {
        if original[i] == patched[i] {
            i += 1;
            continue;
        }
        // Un enregistrement par zone modifiée ; on démarre un octet plus tôt pour éviter « EOF ».
        let start = if i == EOF_OFFSET { i - 1 } else { i };
        let mut end = i + 1;
        while end < patched.len() && end - start < MAX_RECORD && original[end] != patched[end] {
            end += 1;
        }
        if start > MAX_OFFSET {
            return Err(crate::FormatError::Invalid("IPS : modification au-delà de 16 Mio"));
        }
        out.extend_from_slice(&(start as u32).to_be_bytes()[1..]);
        out.extend_from_slice(&((end - start) as u16).to_be_bytes());
        out.extend_from_slice(&patched[start..end]);
        i = end;
    }
    out.extend_from_slice(b"EOF");
    Ok(out)
}

/// Applique un patch IPS (enregistrements simples et répétés « RLE »).
pub fn apply(data: &mut Vec<u8>, ips: &[u8]) -> crate::Result<()> {
    const BAD: crate::FormatError = crate::FormatError::Invalid("patch IPS invalide");
    let rest = ips.strip_prefix(b"PATCH").ok_or(BAD)?;
    let mut p = 0;
    loop {
        let rec = rest.get(p..p + 3).ok_or(BAD)?;
        if rec == b"EOF" {
            return Ok(());
        }
        let at = (rec[0] as usize) << 16 | (rec[1] as usize) << 8 | rec[2] as usize;
        let size = u16::from_be_bytes(rest.get(p + 3..p + 5).ok_or(BAD)?.try_into().unwrap()) as usize;
        p += 5;
        let bytes = if size == 0 {
            let n = u16::from_be_bytes(rest.get(p..p + 2).ok_or(BAD)?.try_into().unwrap()) as usize;
            let v = *rest.get(p + 2).ok_or(BAD)?;
            p += 3;
            vec![v; n]
        } else {
            let b = rest.get(p..p + size).ok_or(BAD)?.to_vec();
            p += size;
            b
        };
        if data.len() < at + bytes.len() {
            data.resize(at + bytes.len(), 0);
        }
        data[at..at + bytes.len()].copy_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let original = vec![0u8; 0x50_0000];
        let mut patched = original.clone();
        patched[0x10..0x14].copy_from_slice(&[1, 2, 3, 4]);
        patched[EOF_OFFSET] = 9;
        patched[0x4F_FFFF] = 7;
        let ips = create(&original, &patched).unwrap();
        assert!(!ips.windows(3).skip(5).take(ips.len() - 8).any(|w| w == b"EOF"));
        let mut out = original.clone();
        apply(&mut out, &ips).unwrap();
        assert_eq!(out, patched);
        assert_eq!(create(&original, &original).unwrap(), b"PATCHEOF");
    }
}
