//! Flux BCSTM (`CSTM`) des jeux 3DS et décodage DSP-ADPCM.
//!
//! Références : 3dbrew (BCSTM), vgmstream (`meta/bcstm.c`, `coding/ngc_dsp_decoder.c`).
//! Tout est en petit-boutiste (BOM `FF FE`).
//!
//! - En-tête : `CSTM`, BOM, taille d'en-tête, version, taille du fichier, nombre de
//!   blocs @0x10 puis une référence `(u16 type, u16, u32 position, u32 taille)` par
//!   bloc dès 0x14 : `0x4000` INFO, `0x4001` SEEK, `0x4002` DATA.
//! - INFO (+8) : références (`u16 type`, `u16`, `s32` relatif à INFO+8) vers les infos
//!   du flux (`0x4100`), la table des pistes et la table des canaux.
//! - Infos du flux : `u8` codec (0 PCM8, 1 PCM16, 2 DSP-ADPCM, 3 IMA-ADPCM) @0,
//!   `u8` boucle @1, `u8` canaux @2, `u32` fréquence @4, `u32` début de boucle @8,
//!   `u32` nombre d'échantillons (fin de boucle) @0xC, `u32` nombre de blocs @0x10,
//!   `u32` taille d'un bloc (par canal) @0x14, `u32` échantillons par bloc @0x18,
//!   `u32` taille du dernier bloc @0x1C, `u32` échantillons du dernier bloc @0x20,
//!   `u32` taille du dernier bloc avec remplissage @0x24, puis la référence aux
//!   données (`0x1F00`, relative à DATA+8) @0x34.
//! - Table des canaux : `u32` nombre puis une référence (relative à la table) par
//!   canal, qui pointe elle-même (relativement à l'entrée) vers les infos DSP-ADPCM :
//!   16 coefficients `i16`, `u16` prédicteur/échelle, `i16` historiques 1 et 2.
//! - DATA : les blocs sont entrelacés par canal (bloc 0 canal 0, bloc 0 canal 1, …).
//!   Une trame DSP-ADPCM fait 8 octets : un octet prédicteur (bits 4-6) / échelle
//!   (bits 0-3) puis 14 échantillons de 4 bits signés, poids fort d'abord.

use super::{rd16, rd32, Decoded};
use crate::music::MusicError;

const BAD: fn(&'static str) -> MusicError = |m| MusicError::Unsupported(format!("BCSTM invalide ({m})"));

/// Échantillons par trame DSP-ADPCM (8 octets).
pub const DSP_FRAME_SAMPLES: usize = 14;
const DSP_FRAME_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Pcm8,
    Pcm16,
    DspAdpcm,
}

/// État DSP-ADPCM d'un canal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DspChannel {
    pub coefs: [i16; 16],
    pub hist1: i16,
    pub hist2: i16,
}

/// En-tête d'un BCSTM : ce qu'il faut pour lire et décoder les données.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bcstm {
    pub codec: Codec,
    pub sample_rate: u32,
    pub channels: Vec<DspChannel>,
    /// Début de boucle (en échantillons) si le flux boucle.
    pub loop_start: Option<usize>,
    /// Nombre total d'échantillons (fin de boucle).
    pub samples: usize,
    pub block_size: usize,
    pub block_samples: usize,
    pub last_block_size: usize,
    pub last_block_padded: usize,
    pub block_count: usize,
    /// Position des données (début du bloc 0) dans le fichier.
    pub data_offset: usize,
}

impl Bcstm {
    /// Lit l'en-tête ; `head` doit contenir le fichier jusqu'au début des données
    /// (voir [`Bcstm::header_len`]).
    pub fn parse(head: &[u8]) -> Result<Self, MusicError> {
        if head.len() < 0x40 || &head[..4] != b"CSTM" {
            return Err(BAD("en-tête CSTM absent"));
        }
        if head[4..6] != [0xFF, 0xFE] {
            return Err(BAD("flux gros-boutiste"));
        }
        let (info, data) = blocks(head)?;
        let get = |at: usize, n: usize| head.get(at..at + n).ok_or(BAD("tronqué"));
        if get(info, 4)? != b"INFO" || get(data, 4)? != b"DATA" {
            return Err(BAD("blocs INFO/DATA absents"));
        }
        let ib = info + 8;
        let refs = get(ib, 0x18)?;
        if rd16(refs, 0) != 0x4100 {
            return Err(BAD("infos du flux absentes"));
        }
        let si = ib + rd32(refs, 4) as usize;
        let s = get(si, 0x38)?;
        let codec = match s[0] {
            0 => Codec::Pcm8,
            1 => Codec::Pcm16,
            2 => Codec::DspAdpcm,
            3 => return Err(MusicError::Unsupported("BCSTM IMA-ADPCM".into())),
            c => return Err(MusicError::Unsupported(format!("codec BCSTM {c}"))),
        };
        let looping = s[1] != 0;
        let channel_count = s[2] as usize;
        if channel_count == 0 {
            return Err(BAD("aucun canal"));
        }
        let samples = rd32(s, 0x0C) as usize;
        let loop_start = rd32(s, 0x08) as usize;
        let data_offset = data + 8 + rd32(s, 0x34) as usize;

        let mut channels = vec![DspChannel::default(); channel_count];
        if codec == Codec::DspAdpcm {
            if rd16(refs, 0x10) != 0x0101 {
                return Err(BAD("table des canaux absente"));
            }
            let table = ib + rd32(refs, 0x14) as usize;
            if (rd32(get(table, 4)?, 0) as usize) < channel_count {
                return Err(BAD("table des canaux incomplète"));
            }
            for (c, ch) in channels.iter_mut().enumerate() {
                let entry = table + rd32(get(table + 4 + c * 8, 8)?, 4) as usize;
                let adpcm = entry + rd32(get(entry, 8)?, 4) as usize;
                let a = get(adpcm, 0x26)?;
                for (i, coef) in ch.coefs.iter_mut().enumerate() {
                    *coef = rd16(a, i * 2) as i16;
                }
                ch.hist1 = rd16(a, 0x22) as i16;
                ch.hist2 = rd16(a, 0x24) as i16;
            }
        }

        let st = Self {
            codec,
            sample_rate: rd32(s, 4),
            channels,
            loop_start: (looping && loop_start < samples).then_some(loop_start),
            samples,
            block_count: rd32(s, 0x10) as usize,
            block_size: rd32(s, 0x14) as usize,
            block_samples: rd32(s, 0x18) as usize,
            last_block_size: rd32(s, 0x1C) as usize,
            last_block_padded: rd32(s, 0x24) as usize,
            data_offset,
        };
        if st.sample_rate == 0 || st.block_count == 0 || st.block_size == 0 || st.block_samples == 0 {
            return Err(BAD("tailles nulles"));
        }
        Ok(st)
    }

    /// Octets à lire au début du fichier pour [`Bcstm::parse`] (d'après les 0x40
    /// premiers octets) : jusqu'au début du bloc DATA, plus son en-tête.
    pub fn header_len(first: &[u8]) -> Result<usize, MusicError> {
        let (info, data) = blocks(first)?;
        Ok(info.max(data) + 0x40)
    }

    /// Octets de données (depuis `data_offset`) à lire pour décoder `samples` échantillons.
    pub fn data_len_for(&self, samples: usize) -> usize {
        let blocks = samples.min(self.samples).div_ceil(self.block_samples).clamp(1, self.block_count);
        let ch = self.channels.len();
        if blocks == self.block_count {
            (blocks - 1) * self.block_size * ch + self.last_block_padded.max(self.last_block_size) * ch
        } else {
            blocks * self.block_size * ch
        }
    }

    /// Décode les `count` premiers échantillons de chaque canal (bornés par la
    /// longueur du flux) ; `data` commence au bloc 0.
    pub fn decode(&self, data: &[u8], count: usize) -> Result<Vec<Vec<i16>>, MusicError> {
        let count = count.min(self.samples);
        let ch = self.channels.len();
        let mut out: Vec<Vec<i16>> = (0..ch).map(|_| Vec::with_capacity(count)).collect();
        let mut state = self.channels.clone();
        let mut block = 0;
        let mut at = 0;
        while out[0].len() < count && block < self.block_count {
            let last = block + 1 == self.block_count;
            let (size, stride) =
                if last { (self.last_block_size, self.last_block_padded.max(self.last_block_size)) } else { (self.block_size, self.block_size) };
            let want = (count - out[0].len()).min(self.block_samples);
            for (c, o) in out.iter_mut().enumerate() {
                let start = at + c * stride;
                let bytes = data.get(start..start + size.min(stride)).ok_or(BAD("données tronquées"))?;
                match self.codec {
                    Codec::DspAdpcm => decode_dsp(bytes, &mut state[c], want, o),
                    Codec::Pcm16 => o.extend(bytes.as_chunks::<2>().0.iter().take(want).map(|b| i16::from_le_bytes(*b))),
                    Codec::Pcm8 => o.extend(bytes.iter().take(want).map(|&b| (b as i8 as i16) << 8)),
                }
            }
            at += stride * ch;
            block += 1;
        }
        Ok(out)
    }
}

/// Positions des blocs INFO et DATA.
fn blocks(head: &[u8]) -> Result<(usize, usize), MusicError> {
    if head.len() < 0x14 {
        return Err(BAD("tronqué"));
    }
    let n = rd16(head, 0x10) as usize;
    let (mut info, mut data) = (None, None);
    for i in 0..n.min(8) {
        let r = head.get(0x14 + i * 12..0x20 + i * 12).ok_or(BAD("tronqué"))?;
        match rd16(r, 0) {
            0x4000 => info = Some(rd32(r, 4) as usize),
            0x4002 => data = Some(rd32(r, 4) as usize),
            _ => {}
        }
    }
    info.zip(data).ok_or(BAD("blocs INFO/DATA absents"))
}

/// Décode au plus `count` échantillons DSP-ADPCM de `data` (trames de 8 octets) dans `out`.
/// D'après vgmstream (`decode_ngc_dsp`).
pub fn decode_dsp(data: &[u8], ch: &mut DspChannel, count: usize, out: &mut Vec<i16>) {
    let mut left = count;
    for frame in data.as_chunks::<DSP_FRAME_BYTES>().0 {
        if left == 0 {
            break;
        }
        let scale = 1i64 << (frame[0] & 0x0F);
        let pred = ((frame[0] >> 4) & 7) as usize;
        // En i64 : coefficients et historiques extrêmes déborderaient d'un i32.
        let (c1, c2) = (ch.coefs[pred * 2] as i64, ch.coefs[pred * 2 + 1] as i64);
        for i in 0..DSP_FRAME_SAMPLES.min(left) {
            let byte = frame[1 + i / 2];
            let nibble = if i % 2 == 0 { (byte as i8) >> 4 } else { ((byte << 4) as i8) >> 4 } as i64;
            let s = ((nibble * scale) << 11) + 1024 + c1 * ch.hist1 as i64 + c2 * ch.hist2 as i64;
            let s = (s >> 11).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
            ch.hist2 = ch.hist1;
            ch.hist1 = s;
            out.push(s);
        }
        left -= DSP_FRAME_SAMPLES.min(left);
    }
}

/// Décode un BCSTM complet en mémoire (tests et petits fichiers).
pub fn decode_file(file: &[u8], max_samples: usize) -> Result<Decoded, MusicError> {
    let st = Bcstm::parse(file)?;
    let data = file.get(st.data_offset..).ok_or(BAD("données absentes"))?;
    st.to_decoded(data, max_samples)
}

impl Bcstm {
    /// Décode ce qu'il faut pour produire `max_samples` échantillons (en bouclant si besoin).
    pub fn to_decoded(&self, data: &[u8], max_samples: usize) -> Result<Decoded, MusicError> {
        // Avec une boucle, la partie [début, fin de boucle] suffit : on la répète ensuite.
        let needed = if self.loop_start.is_some() { self.samples } else { max_samples.min(self.samples) };
        Ok(Decoded { sample_rate: self.sample_rate, channels: self.decode(data, needed)?, loop_start: self.loop_start })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn dsp_identity_predictor() {
        // Coefficient 1 = 1.0 (2048) : chaque échantillon = précédent + quartet × échelle.
        let mut ch = DspChannel { coefs: [0; 16], hist1: 0, hist2: 0 };
        ch.coefs[0] = 2048;
        // Prédicteur 0, échelle 2^0 ; quartets 1, 2, 3, -1 (0xF), 7, -8, puis zéros.
        let frame = [0x00, 0x12, 0x3F, 0x78, 0, 0, 0, 0];
        let mut out = Vec::new();
        decode_dsp(&frame, &mut ch, 14, &mut out);
        assert_eq!(out, [1, 3, 6, 5, 12, 4, 4, 4, 4, 4, 4, 4, 4, 4]);
        assert_eq!((ch.hist1, ch.hist2), (4, 4));
    }

    #[test]
    fn dsp_scale_coefs_and_clamp() {
        let mut ch = DspChannel { coefs: [0; 16], hist1: 100, hist2: 50 };
        // Prédicteur 1 : c1 = 0.5 (1024), c2 = -0.25 (-512) ; échelle 2^4.
        ch.coefs[2] = 1024;
        ch.coefs[3] = -512;
        let frame = [0x14, 0x7F, 0x80, 0, 0, 0, 0, 0];
        let mut out = Vec::new();
        decode_dsp(&frame, &mut ch, 4, &mut out);
        // s = plancher(quartet × 16 + 0.5 × h1 − 0.25 × h2 + 0.5) :
        // 112 + 50 − 12.5 → 150 ; −16 + 75 − 25 → 34 ; −128 + 17 − 37.5 → −148 ; 0 − 74 − 8.5 → −82.
        assert_eq!(out, [150, 34, -148, -82]);

        // Saturation : échelle 2^12, quartet 7, historique déjà haut.
        let mut ch = DspChannel { coefs: [0; 16], hist1: 32000, hist2: 0 };
        ch.coefs[0] = 2048;
        let mut out = Vec::new();
        decode_dsp(&[0x0C, 0x79, 0, 0, 0, 0, 0, 0], &mut ch, 2, &mut out);
        assert_eq!(out, [i16::MAX, i16::MAX - 7 * 4096]);
    }

    /// BCSTM synthétique : 2 canaux DSP-ADPCM, 2 blocs (le dernier partiel), boucle.
    pub(crate) fn synthetic() -> Vec<u8> {
        let mut f = vec![0u8; 0x40];
        f[..4].copy_from_slice(b"CSTM");
        f[4..6].copy_from_slice(&[0xFF, 0xFE]);
        f[6..8].copy_from_slice(&0x40u16.to_le_bytes());
        f[8..12].copy_from_slice(&0x0202_0000u32.to_le_bytes());
        f[0x10..0x12].copy_from_slice(&2u16.to_le_bytes());
        let info_at = 0x40usize;
        let mut info = vec![0u8; 0x140];
        info[..4].copy_from_slice(b"INFO");
        let w16 = |b: &mut [u8], at: usize, v: u16| b[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let w32 = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        // Références depuis INFO+8.
        w16(&mut info, 8, 0x4100);
        w32(&mut info, 12, 0x18);
        w16(&mut info, 8 + 0x10, 0x0101);
        w32(&mut info, 8 + 0x14, 0x60);
        // Infos du flux @ INFO+8+0x18.
        let s = 8 + 0x18;
        info[s] = 2;
        info[s + 1] = 1;
        info[s + 2] = 2;
        w32(&mut info, s + 4, 32000);
        w32(&mut info, s + 8, 14); // début de boucle
        w32(&mut info, s + 0xC, 42); // 3 trames
        w32(&mut info, s + 0x10, 2);
        w32(&mut info, s + 0x14, 16); // 2 trames par bloc
        w32(&mut info, s + 0x18, 28);
        w32(&mut info, s + 0x1C, 8);
        w32(&mut info, s + 0x20, 14);
        w32(&mut info, s + 0x24, 0x20);
        w32(&mut info, s + 0x34, 0x18);
        // Table des canaux @ INFO+8+0x60.
        let t = 8 + 0x60;
        w32(&mut info, t, 2);
        for c in 0..2 {
            w32(&mut info, t + 4 + c * 8 + 4, 0x14 + c as u32 * 8);
            w32(&mut info, t + 0x14 + c * 8 + 4, 0x10 + c as u32 * 0x28 - c as u32 * 8);
        }
        // Infos ADPCM : c1 = 1.0 pour les deux canaux.
        for c in 0..2 {
            let a = t + 0x24 + c * 0x28;
            w16(&mut info, a, 2048);
        }
        f.extend(&info);
        let data_at = f.len();
        let refs = [(0x4000u16, info_at as u32, info.len() as u32), (0x4002, data_at as u32, 0)];
        for (i, (ty, at, len)) in refs.iter().enumerate() {
            w16(&mut f, 0x14 + i * 12, *ty);
            w32(&mut f, 0x18 + i * 12, *at);
            w32(&mut f, 0x1C + i * 12, *len);
        }
        f.extend(b"DATA");
        f.resize(data_at + 0x20, 0);
        // Bloc 0 : canal 0 monte de 1 par échantillon, canal 1 descend.
        let frame = |n: u8| [0x00, n << 4 | n, n << 4 | n, n << 4 | n, n << 4 | n, n << 4 | n, n << 4 | n, n << 4 | n];
        f.extend(frame(1));
        f.extend(frame(1));
        f.extend(frame(0xF));
        f.extend(frame(0xF));
        // Dernier bloc (1 trame, rembourré à 0x20 octets par canal).
        f.extend(frame(2));
        f.resize(f.len() + 0x18, 0);
        f.extend(frame(0xE));
        f.resize(f.len() + 0x18, 0);
        f
    }

    #[test]
    fn parses_synthetic_header_and_decodes_blocks() {
        let file = synthetic();
        let st = Bcstm::parse(&file).unwrap();
        assert_eq!(st.codec, Codec::DspAdpcm);
        assert_eq!(st.sample_rate, 32000);
        assert_eq!(st.channels.len(), 2);
        assert_eq!(st.channels[1].coefs[0], 2048);
        assert_eq!(st.loop_start, Some(14));
        assert_eq!(st.samples, 42);
        assert!(Bcstm::header_len(&file[..0x40]).unwrap() >= st.data_offset);
        assert_eq!(st.data_len_for(10), 32);
        assert_eq!(st.data_len_for(1000), 32 + 64);

        let d = decode_file(&file, 5).unwrap();
        // Boucle : on décode tout le flux.
        assert_eq!(d.channels[0].len(), 42);
        assert_eq!(d.channels[0][..3], [1, 2, 3]);
        assert_eq!(d.channels[0][27], 28);
        assert_eq!(d.channels[0][28], 30);
        assert_eq!(d.channels[0][41], 56);
        assert_eq!(d.channels[1][41], -56);
    }
}
