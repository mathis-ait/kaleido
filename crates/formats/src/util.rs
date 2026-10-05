use std::io::{Read, Seek, SeekFrom};

/// Taille totale du flux, sans modifier la position courante de façon observable.
pub fn stream_len<R: Seek>(r: &mut R) -> std::io::Result<u64> {
    let pos = r.stream_position()?;
    let len = r.seek(SeekFrom::End(0))?;
    r.seek(SeekFrom::Start(pos))?;
    Ok(len)
}

pub(crate) fn read_at<R: Read + Seek>(r: &mut R, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    r.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Sous-tranche vérifiée : erreur au lieu de panique si elle dépasse.
pub(crate) fn slice(data: &[u8], offset: u32, len: u32) -> crate::Result<&[u8]> {
    let (start, len) = (offset as usize, len as usize);
    data.get(start..start + len).ok_or(crate::FormatError::Invalid("zone hors du fichier"))
}

pub(crate) fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

pub(crate) fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub(crate) fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

pub(crate) fn u32be(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}

pub(crate) fn u64be(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}

/// Chaîne ASCII terminée par un zéro (ou remplie d'espaces).
pub(crate) fn ascii(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '?' }).collect::<String>().trim().to_string()
}

pub(crate) const fn align(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}
