//! Patchs xdelta (VCDIFF, RFC 3284) : le format des romhacks DS distribués avec
//! xdelta3 ou Delta Patcher. Chaque fenêtre reconstruit un morceau du fichier
//! cible à partir d'une plage de l'original (ou de la cible déjà écrite) et de
//! trois sections : données littérales, instructions et adresses.
//!
//! Seule la table de codes par défaut est prise en charge, sans compression
//! secondaire (le cas de tous les patchs créés sans `-S`).

const BAD: crate::FormatError = crate::FormatError::Invalid("patch xdelta invalide");

const VCD_DECOMPRESS: u8 = 0x01;
const VCD_CODETABLE: u8 = 0x02;
const VCD_APPHEADER: u8 = 0x04;
const VCD_SOURCE: u8 = 0x01;
const VCD_TARGET: u8 = 0x02;
/// Extension xdelta3 : somme Adler-32 de la fenêtre cible.
const VCD_ADLER32: u8 = 0x04;

const NEAR: usize = 4;
const SAME: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Noop,
    Add,
    Run,
    Copy(u8),
}

#[derive(Clone, Copy)]
struct Inst {
    op: Op,
    size: u8,
}

/// Table de codes par défaut (RFC 3284, section 5.6) : 256 paires d'instructions.
fn default_table() -> Vec<[Inst; 2]> {
    let noop = Inst { op: Op::Noop, size: 0 };
    let mut t = Vec::with_capacity(256);
    t.push([Inst { op: Op::Run, size: 0 }, noop]);
    for size in 0..=17 {
        t.push([Inst { op: Op::Add, size }, noop]);
    }
    for mode in 0..9 {
        t.push([Inst { op: Op::Copy(mode), size: 0 }, noop]);
        for size in 4..=18 {
            t.push([Inst { op: Op::Copy(mode), size }, noop]);
        }
    }
    for mode in 0..6 {
        for add in 1..=4 {
            for copy in 4..=6 {
                t.push([Inst { op: Op::Add, size: add }, Inst { op: Op::Copy(mode), size: copy }]);
            }
        }
    }
    for mode in 6..9 {
        for add in 1..=4 {
            t.push([Inst { op: Op::Add, size: add }, Inst { op: Op::Copy(mode), size: 4 }]);
        }
    }
    for mode in 0..9 {
        t.push([Inst { op: Op::Copy(mode), size: 4 }, Inst { op: Op::Add, size: 1 }]);
    }
    debug_assert_eq!(t.len(), 256);
    t
}

/// Curseur sur une section du patch.
struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> crate::Result<u8> {
        let b = *self.d.get(self.p).ok_or(BAD)?;
        self.p += 1;
        Ok(b)
    }

    /// Entier VCDIFF : base 128, gros-boutiste, bit de poids fort = suite.
    fn int(&mut self) -> crate::Result<usize> {
        let mut v: u64 = 0;
        loop {
            let b = self.byte()?;
            v = v.checked_mul(128).ok_or(BAD)? | u64::from(b & 0x7F);
            if b & 0x80 == 0 {
                return usize::try_from(v).map_err(|_| BAD);
            }
        }
    }

    fn take(&mut self, n: usize) -> crate::Result<&'a [u8]> {
        let s = self.d.get(self.p..self.p.checked_add(n).ok_or(BAD)?).ok_or(BAD)?;
        self.p += n;
        Ok(s)
    }

    fn done(&self) -> bool {
        self.p >= self.d.len()
    }
}

/// Cache d'adresses « near » / « same » (RFC 3284, section 5.1), remis à zéro à chaque fenêtre.
struct Cache {
    near: [usize; NEAR],
    next: usize,
    same: Vec<usize>,
}

impl Cache {
    fn new() -> Self {
        Cache { near: [0; NEAR], next: 0, same: vec![0; SAME * 256] }
    }

    fn decode(&mut self, here: usize, mode: u8, addrs: &mut Reader) -> crate::Result<usize> {
        let mode = mode as usize;
        let addr = match mode {
            0 => addrs.int()?,
            1 => here.checked_sub(addrs.int()?).ok_or(BAD)?,
            m if m < 2 + NEAR => self.near[m - 2].checked_add(addrs.int()?).ok_or(BAD)?,
            m => self.same[(m - 2 - NEAR) * 256 + addrs.byte()? as usize],
        };
        self.near[self.next] = addr;
        self.next = (self.next + 1) % NEAR;
        self.same[addr % (SAME * 256)] = addr;
        Ok(addr)
    }
}

/// Somme Adler-32 (zlib).
fn adler32(d: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in d.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    b << 16 | a
}

/// Applique un patch xdelta à `source` et renvoie le fichier cible.
pub fn apply(source: &[u8], patch: &[u8]) -> crate::Result<Vec<u8>> {
    let mut r = Reader { d: patch, p: 0 };
    if r.take(4)? != [0xD6, 0xC3, 0xC4, 0x00] {
        return Err(crate::FormatError::Invalid("ce fichier n'est pas un patch xdelta"));
    }
    let hdr = r.byte()?;
    if hdr & VCD_DECOMPRESS != 0 {
        return Err(crate::FormatError::Invalid("patch xdelta avec compression secondaire non pris en charge"));
    }
    if hdr & VCD_CODETABLE != 0 {
        return Err(crate::FormatError::Invalid("patch xdelta avec table de codes personnalisée non pris en charge"));
    }
    if hdr & VCD_APPHEADER != 0 {
        let n = r.int()?;
        r.take(n)?;
    }

    let table = default_table();
    let mut out: Vec<u8> = Vec::with_capacity(source.len());
    while !r.done() {
        let win = r.byte()?;
        let (seg_len, seg_pos) = if win & (VCD_SOURCE | VCD_TARGET) != 0 {
            (r.int()?, r.int()?)
        } else {
            (0, 0)
        };
        let seg_end = seg_pos.checked_add(seg_len).ok_or(BAD)?;
        if win & VCD_SOURCE != 0 && seg_end > source.len() {
            return Err(crate::FormatError::Invalid("patch xdelta : le fichier d'origine est trop petit (mauvaise ROM ?)"));
        }
        if win & VCD_TARGET != 0 && seg_end > out.len() {
            return Err(BAD);
        }
        let _delta_len = r.int()?;
        let target_len = r.int()?;
        if r.byte()? != 0 {
            return Err(crate::FormatError::Invalid("patch xdelta avec compression secondaire non pris en charge"));
        }
        let (data_len, inst_len, addr_len) = (r.int()?, r.int()?, r.int()?);
        let checksum = if win & VCD_ADLER32 != 0 {
            Some(u32::from_be_bytes(r.take(4)?.try_into().unwrap()))
        } else {
            None
        };
        let mut data = Reader { d: r.take(data_len)?, p: 0 };
        let mut insts = Reader { d: r.take(inst_len)?, p: 0 };
        let mut addrs = Reader { d: r.take(addr_len)?, p: 0 };

        let start = out.len();
        let mut cache = Cache::new();
        while !insts.done() {
            let pair = table[insts.byte()? as usize];
            for inst in pair {
                if inst.op == Op::Noop {
                    continue;
                }
                let size = if inst.size == 0 { insts.int()? } else { inst.size as usize };
                match inst.op {
                    Op::Noop => {}
                    Op::Add => out.extend_from_slice(data.take(size)?),
                    Op::Run => {
                        let b = data.byte()?;
                        out.resize(out.len() + size, b);
                    }
                    Op::Copy(mode) => {
                        let here = seg_len + (out.len() - start);
                        let addr = cache.decode(here, mode, &mut addrs)?;
                        if addr < seg_len {
                            // Copie depuis le segment (fichier d'origine ou cible déjà écrite).
                            if addr + size > seg_len {
                                return Err(BAD);
                            }
                            let from = seg_pos + addr;
                            if win & VCD_TARGET != 0 {
                                out.extend_from_within(from..from + size);
                            } else {
                                out.extend_from_slice(&source[from..from + size]);
                            }
                        } else {
                            // Copie depuis la fenêtre en cours : peut chevaucher ce qu'on écrit.
                            let from = start + (addr - seg_len);
                            if from >= out.len() {
                                return Err(BAD);
                            }
                            for i in 0..size {
                                let b = out[from + i];
                                out.push(b);
                            }
                        }
                    }
                }
            }
        }
        if out.len() - start != target_len {
            return Err(BAD);
        }
        if let Some(sum) = checksum {
            if adler32(&out[start..]) != sum {
                return Err(crate::FormatError::Invalid("patch xdelta : somme de contrôle incorrecte (mauvaise ROM ?)"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: usize) -> Vec<u8> {
        let mut bytes = vec![(v & 0x7F) as u8];
        let mut v = v >> 7;
        while v > 0 {
            bytes.insert(0, (v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        bytes
    }

    #[test]
    fn copie_ajout_et_repetition() {
        let source = b"Bonjour le monde";
        // Cible : « Bonjour » (COPY 7 depuis la source, mode 0) + « !! » (ADD 2) + « zzzz » (RUN 4).
        let data = b"!!z".to_vec();
        // Instructions : COPY mode 0 taille 0 (index 19) + taille 7, ADD 2 (index 3), RUN (index 0) + taille 4.
        let mut inst = vec![19];
        inst.extend(int(7));
        inst.push(3);
        inst.push(0);
        inst.extend(int(4));
        let addr = int(0);
        let target = b"Bonjour!!zzzz";
        let mut p = vec![0xD6, 0xC3, 0xC4, 0x00, 0x00, VCD_SOURCE | VCD_ADLER32];
        p.extend(int(source.len()));
        p.extend(int(0));
        let mut body = int(target.len());
        body.push(0);
        body.extend(int(data.len()));
        body.extend(int(inst.len()));
        body.extend(int(addr.len()));
        body.extend(adler32(target).to_be_bytes());
        body.extend(&data);
        body.extend(&inst);
        body.extend(&addr);
        p.extend(int(body.len()));
        p.extend(body);
        assert_eq!(apply(source, &p).unwrap(), target);
    }

    #[test]
    fn rejette_un_autre_format() {
        assert!(apply(b"x", b"PATCH").is_err());
    }
}
