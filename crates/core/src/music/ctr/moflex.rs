//! Vidéos Mobiclip `.moflex` : on n'en extrait que la piste audio.
//!
//! Soleil/Lune et Ultra-Soleil/Ultra-Lune n'ont pas de BGM d'écran titre dans
//! leur archive son : l'écran titre est une vidéo (`m/title_<langue>.moflex`) dont
//! la bande-son est le thème. Démultiplexeur et décodeur portés de FFmpeg
//! (`libavformat/moflex.c` et `ADPCM_IMA_MOFLEX` dans `libavcodec/adpcm.c`,
//! Paul B Mahol, LGPL 2.1+), simplifiés : pistes vidéo ignorées.
//!
//! Structure : une suite de blocs. Un bloc commence (facultativement) par une
//! synchro `4C 32` (« L2 »), 2 octets, un horodatage `u64`, `u16` taille − 1 puis
//! des descripteurs de pistes (type et taille en entier variable ; type 2 = audio :
//! `u8` piste, `u8` codec — 0 FastAudio, 1 IMA-ADPCM, 2 PCM16 —, `u24` fréquence − 1,
//! `u8` canaux − 1) terminés par le type 0. Viennent un octet de drapeaux (+2 octets
//! si bit 1) puis des paquets, chacun précédé d'un petit en-tête lu bit à bit
//! (numéro de piste, fin de trame, taille − 1 sur 13 bits) ; un octet nul clôt la
//! liste. Les paquets d'une trame se concatènent jusqu'au bit « fin de trame ».
//!
//! Un paquet IMA-ADPCM : pour chaque canal `i16` index du pas et `i16` prédicteur,
//! puis des sous-trames de 256 échantillons par canal (128 octets, quartet bas d'abord).

use super::Decoded;
use crate::music::MusicError;

const SYNC: u16 = 0x4C32;

fn bad(m: &'static str) -> MusicError {
    MusicError::Unsupported(format!("vidéo moflex invalide ({m})"))
}

/// Lecture séquentielle par morceaux : la vidéo pèse ~20 Mo, on ne lit que ce qui sert.
pub struct Chunked<F> {
    fetch: F,
    len: u64,
    pos: u64,
    buf: Vec<u8>,
    buf_at: u64,
}

const CHUNK: usize = 512 * 1024;

impl<F: FnMut(u64, usize) -> Result<Vec<u8>, MusicError>> Chunked<F> {
    pub fn new(len: u64, fetch: F) -> Self {
        Self { fetch, len, pos: 0, buf: Vec::new(), buf_at: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.len
    }

    fn u8(&mut self) -> Result<u8, MusicError> {
        if self.eof() {
            return Err(bad("fin de fichier inattendue"));
        }
        if self.pos < self.buf_at || self.pos >= self.buf_at + self.buf.len() as u64 {
            let n = CHUNK.min((self.len - self.pos) as usize);
            self.buf = (self.fetch)(self.pos, n)?;
            self.buf_at = self.pos;
        }
        let b = self.buf[(self.pos - self.buf_at) as usize];
        self.pos += 1;
        Ok(b)
    }

    fn be(&mut self, n: usize) -> Result<u64, MusicError> {
        (0..n).try_fold(0u64, |v, _| Ok(v << 8 | self.u8()? as u64))
    }

    fn bytes(&mut self, n: usize, out: &mut Vec<u8>) -> Result<(), MusicError> {
        if self.pos + n as u64 > self.len {
            return Err(bad("paquet tronqué"));
        }
        out.reserve(n);
        for _ in 0..n {
            out.push(self.u8()?);
        }
        Ok(())
    }

    /// Entier variable : 7 bits par octet, bit 7 = suite (4 octets au plus).
    fn var(&mut self) -> Result<u32, MusicError> {
        let mut v = 0u32;
        for i in 0..4 {
            let b = self.u8()? as u32;
            if i == 3 {
                return Ok(v << 8 | b);
            }
            v = v << 7 | (b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Ok(v)
    }
}

/// Lecteur de bits (poids fort d'abord) qui consomme les octets du flux au fil de l'eau.
struct Bits {
    pos: u32,
    last: u32,
}

impl Bits {
    fn pop<F: FnMut(u64, usize) -> Result<Vec<u8>, MusicError>>(&mut self, r: &mut Chunked<F>) -> Result<u32, MusicError> {
        if self.pos & 7 == 0 {
            self.last = (r.u8()? as u32) << 24;
        } else {
            self.last <<= 1;
        }
        self.pos += 1;
        Ok(self.last >> 31)
    }

    fn int<F: FnMut(u64, usize) -> Result<Vec<u8>, MusicError>>(&mut self, r: &mut Chunked<F>, n: u32) -> Result<u32, MusicError> {
        if n > 31 {
            return Err(bad("champ trop long"));
        }
        (0..n).try_fold(0, |v, _| Ok(v << 1 | self.pop(r)?))
    }

    fn length<F: FnMut(u64, usize) -> Result<Vec<u8>, MusicError>>(&mut self, r: &mut Chunked<F>) -> Result<u32, MusicError> {
        let mut n = 1;
        while self.pop(r)? == 0 {
            n += 1;
            if n > 32 {
                return Err(bad("longueur invalide"));
            }
        }
        Ok(n)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Audio {
    stream: u32,
    codec: u8,
    sample_rate: u32,
    channels: usize,
}

/// Décode la piste audio jusqu'à `max_samples` échantillons par canal (ou la fin).
/// La vidéo de l'écran titre tourne en boucle : `loop_start` vaut 0.
pub fn decode<F: FnMut(u64, usize) -> Result<Vec<u8>, MusicError>>(r: &mut Chunked<F>, max_samples: usize) -> Result<Decoded, MusicError> {
    let mut audio: Option<Audio> = None;
    let mut out: Vec<Vec<i16>> = Vec::new();
    let mut packet = Vec::new();
    let mut size = 0u64;
    let mut blocks = 0usize;

    while !r.eof() && out.first().is_none_or(|c| c.len() < max_samples) {
        let block_at = r.pos;
        // Synchro et descripteurs de pistes.
        if r.be(2)? as u16 == SYNC {
            r.be(2)?;
            r.be(8)?; // horodatage
            size = r.be(2)? + 1;
            loop {
                let (ty, len) = (r.var()?, r.var()?);
                match ty {
                    0 => {
                        r.pos += len as u64;
                        break;
                    }
                    2 => {
                        let stream = r.u8()? as u32;
                        let codec = r.u8()?;
                        let sample_rate = r.be(3)? as u32 + 1;
                        let channels = r.u8()? as usize + 1;
                        let a = Audio { stream, codec, sample_rate, channels };
                        if audio.is_none() {
                            audio = Some(a);
                            out = vec![Vec::with_capacity(max_samples.min(1 << 24)); channels];
                        }
                    }
                    1 | 3 => r.pos += if ty == 3 { 13 } else { 12 },
                    4 => r.pos += 2,
                    _ => r.pos += len as u64,
                }
            }
        } else {
            r.pos -= 2;
        }
        if size == 0 {
            return Err(bad("bloc sans taille"));
        }
        let flags = r.u8()?;
        if flags & 2 != 0 {
            r.pos += 2;
        }

        while r.pos < block_at + size && !r.eof() {
            if r.u8()? == 0 {
                break;
            }
            r.pos -= 1;
            let mut br = Bits { pos: 0, last: 0 };
            let bits = br.length(r)?;
            let stream = br.int(r, bits)?;
            let end_frame = br.pop(r)? == 1;
            if end_frame {
                let bits = br.length(r)?;
                br.int(r, bits)?;
                br.pop(r)?;
                let bits = br.length(r)?;
                br.int(r, bits * 2 + 26)?;
            }
            let pkt_size = br.int(r, 13)? as u64 + 1;
            if pkt_size > size {
                return Err(bad("paquet plus grand que le bloc"));
            }
            match audio {
                Some(a) if a.stream == stream => {
                    r.bytes(pkt_size as usize, &mut packet)?;
                    if end_frame && !packet.is_empty() {
                        decode_packet(a, &packet, &mut out)?;
                        packet.clear();
                    }
                }
                _ => r.pos += pkt_size,
            }
        }
        if flags % 2 == 0 {
            r.pos = block_at + size;
        }
        blocks += 1;
        if blocks > 10_000_000 {
            return Err(bad("trop de blocs"));
        }
    }

    let a = audio.ok_or_else(|| MusicError::NotFound("piste audio de la vidéo".into()))?;
    for c in &mut out {
        c.truncate(max_samples);
    }
    Ok(Decoded { sample_rate: a.sample_rate, channels: out, loop_start: Some(0) })
}

fn decode_packet(a: Audio, p: &[u8], out: &mut [Vec<i16>]) -> Result<(), MusicError> {
    let ch = a.channels;
    match a.codec {
        1 => {
            let header = 4 * ch;
            if p.len() < header {
                return Err(bad("paquet IMA trop court"));
            }
            let mut state: Vec<(i32, i32)> = Vec::with_capacity(ch);
            for c in 0..ch {
                let step = i16::from_le_bytes([p[c * 4], p[c * 4 + 1]]) as i32;
                let pred = i16::from_le_bytes([p[c * 4 + 2], p[c * 4 + 3]]) as i32;
                if !(0..=88).contains(&step) {
                    return Err(bad("index de pas IMA invalide"));
                }
                state.push((pred, step));
            }
            let samples = (p.len() - header) * 2 / ch;
            let mut at = header;
            for _ in 0..samples / 256 {
                for (c, st) in state.iter_mut().enumerate() {
                    for &b in &p[at..at + 128] {
                        out[c].push(ima_nibble(st, b & 0x0F));
                        out[c].push(ima_nibble(st, b >> 4));
                    }
                    at += 128;
                }
            }
            Ok(())
        }
        2 => {
            for (i, s) in p.as_chunks::<2>().0.iter().enumerate() {
                out[i % ch].push(i16::from_le_bytes(*s));
            }
            Ok(())
        }
        0 => Err(MusicError::Unsupported("audio FastAudio (moflex)".into())),
        c => Err(MusicError::Unsupported(format!("codec audio moflex {c}"))),
    }
}

const IMA_STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190,
    209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499,
    2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350,
    22385, 24623, 27086, 29794, 32767,
];
const IMA_INDEX: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// Un quartet IMA-ADPCM ; `st` = (prédicteur, index du pas).
fn ima_nibble(st: &mut (i32, i32), nibble: u8) -> i16 {
    let step = IMA_STEPS[st.1 as usize];
    st.1 = (st.1 + IMA_INDEX[(nibble & 7) as usize]).clamp(0, 88);
    let diff = ((2 * (nibble & 7) as i32 + 1) * step) >> 3;
    st.0 = if nibble & 8 != 0 { st.0 - diff } else { st.0 + diff }.clamp(i16::MIN as i32, i16::MAX as i32);
    st.0 as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ima_nibbles() {
        let mut st = (0, 0);
        // pas 7 : (2·4+1)·7 >> 3 = 7, index 0 → 2.
        assert_eq!(ima_nibble(&mut st, 4), 7);
        assert_eq!(st, (7, 2));
        // pas 9, signe : (2·7+1)·9 >> 3 = 16 → −9, index 2 + 8 = 10.
        assert_eq!(ima_nibble(&mut st, 0xF), -9);
        assert_eq!(st, (-9, 10));
        let mut st = (32760, 88);
        assert_eq!(ima_nibble(&mut st, 7), i16::MAX);
    }

    /// Bloc moflex synthétique : une piste audio IMA mono et un paquet vidéo ignoré.
    fn synthetic() -> Vec<u8> {
        let mut f = vec![0x4C, 0x32, 0, 0];
        f.extend(0u64.to_be_bytes());
        let size_at = f.len();
        f.extend([0, 0]);
        // Piste vidéo (type 1, 12 octets) puis audio (type 2) : piste 1, IMA, 32000 Hz, mono.
        f.extend([1, 12, 0, 0, 0, 30, 0, 1, 1, 0x90, 0, 0xF0, 0, 0]);
        f.extend([2, 6, 1, 1]);
        f.extend((32000u32 - 1).to_be_bytes()[1..].iter());
        f.extend([0]);
        f.extend([0, 0]); // fin des descripteurs
        f.push(0); // drapeaux
                   // Paquet vidéo, piste 0 : longueur 1 (bit 1), piste sur 1 bit (0), pas de fin de trame (0),
                   // taille − 1 = 3 sur 13 bits → bits : 1 0 0 0000000000011 (16 bits).
        f.extend([0b1000_0000, 0b0000_0011]);
        f.extend([0xAA; 4]);
        // Paquet audio, piste 1 : 1 1 1 (fin de trame), puis longueur 1 (1), valeur 0 (1 bit),
        // 1 bit, longueur 1 (1), 28 bits nuls, taille − 1 = 131 sur 13 bits.
        let mut bits = format!("111{}{}", "1001", "0".repeat(28));
        bits += &format!("{:013b}", 131);
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        for c in bits.as_bytes().chunks(8) {
            f.push(u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap());
        }
        f.extend(0i16.to_le_bytes()); // index du pas
        f.extend(0i16.to_le_bytes()); // prédicteur
        f.extend([0x44; 128]); // 256 échantillons
        f.push(0); // fin des paquets
        let size = f.len() as u16 - 1;
        f[size_at..size_at + 2].copy_from_slice(&size.to_be_bytes());
        f
    }

    #[test]
    fn demuxes_synthetic_audio() {
        let file = synthetic();
        let mut r = Chunked::new(file.len() as u64, |at, n| Ok(file[at as usize..at as usize + n].to_vec()));
        let d = decode(&mut r, 10_000).unwrap();
        assert_eq!(d.sample_rate, 32000);
        assert_eq!(d.channels.len(), 1);
        assert_eq!(d.channels[0].len(), 256);
        assert_eq!(d.channels[0][..2], [7, 17]);
        assert!(d.channels[0].windows(2).all(|w| w[1] >= w[0]));
    }
}
