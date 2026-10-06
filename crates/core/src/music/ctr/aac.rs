//! Flux AAC (ADTS) de Pokémon X/Y.
//!
//! X/Y ne rangent pas leurs BGM en BCSTM mais en fichiers `sound/*.aac` : des
//! trames ADTS AAC-LC (48 kHz, stéréo, ~128 kbit/s, CRC présent), jouées par le
//! décodeur AAC du DSP (`sound/dspaudio.cdc`). L'archive `xy_sound.bcsar` ne
//! référence que le nom du fichier : aucune information de boucle (contrairement
//! aux BCSTM) : on rejoue le fichier depuis le début (voir `ctr.rs`).
//!
//! Le décodage AAC-LC est confié à `symphonia-codec-aac` (MPL 2.0) ; on se
//! contente de découper les trames ADTS.

use symphonia_codec_aac::AacDecoder;
use symphonia_core::audio::{AudioBufferRef, Channels, Signal};
use symphonia_core::codecs::{CodecParameters, Decoder, DecoderOptions, CODEC_TYPE_AAC};
use symphonia_core::formats::Packet;

use super::Decoded;
use crate::music::MusicError;

const RATES: [u32; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];

/// Une trame ADTS : charge utile AAC brute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdtsFrame<'a> {
    pub sample_rate: u32,
    pub channels: u8,
    /// Type d'objet audio MPEG-4 (2 = AAC-LC).
    pub profile: u8,
    pub payload: &'a [u8],
}

/// Découpe un flux ADTS en trames (s'arrête à la première trame invalide).
pub fn adts_frames(d: &[u8]) -> Result<Vec<AdtsFrame<'_>>, MusicError> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 7 <= d.len() {
        let h = &d[at..at + 7];
        if h[0] != 0xFF || h[1] & 0xF0 != 0xF0 {
            break;
        }
        let crc = h[1] & 1 == 0;
        let profile = (h[2] >> 6) + 1;
        let rate = *RATES.get(((h[2] >> 2) & 0xF) as usize).ok_or_else(|| MusicError::Unsupported("fréquence ADTS".into()))?;
        let channels = ((h[2] & 1) << 2) | (h[3] >> 6);
        let len = (((h[3] & 3) as usize) << 11) | ((h[4] as usize) << 3) | ((h[5] as usize) >> 5);
        let blocks = (h[6] & 3) as usize + 1;
        let header = if crc { 9 } else { 7 };
        if len < header || at + len > d.len() {
            break;
        }
        if blocks != 1 {
            return Err(MusicError::Unsupported("trames ADTS à plusieurs blocs".into()));
        }
        out.push(AdtsFrame { sample_rate: rate, channels, profile, payload: &d[at + header..at + len] });
        at += len;
    }
    if out.is_empty() {
        return Err(MusicError::Unsupported("flux AAC (ADTS) illisible".into()));
    }
    Ok(out)
}

/// Décode au plus `max_samples` échantillons par canal (tout le fichier, qui boucle,
/// s'il est plus court).
pub fn decode(file: &[u8], max_samples: usize) -> Result<Decoded, MusicError> {
    let frames = adts_frames(file)?;
    let first = frames[0];
    if first.profile != 2 || !(1..=2).contains(&first.channels) {
        return Err(MusicError::Unsupported(format!("AAC profil {} / {} canaux", first.profile, first.channels)));
    }
    let layout = if first.channels == 1 { Channels::FRONT_CENTRE } else { Channels::FRONT_LEFT | Channels::FRONT_RIGHT };
    let mut params = CodecParameters::new();
    params.for_codec(CODEC_TYPE_AAC).with_sample_rate(first.sample_rate).with_channels(layout);
    let err = |e: symphonia_core::errors::Error| MusicError::Unsupported(format!("AAC : {e}"));
    let mut dec = AacDecoder::try_new(&params, &DecoderOptions::default()).map_err(err)?;

    let mut out: Vec<Vec<i16>> = vec![Vec::with_capacity(max_samples.min(frames.len() * 1024)); first.channels as usize];
    for (i, f) in frames.iter().enumerate() {
        if out[0].len() >= max_samples {
            break;
        }
        let pkt = Packet::new_from_slice(0, i as u64 * 1024, 1024, f.payload);
        match dec.decode(&pkt) {
            Ok(AudioBufferRef::F32(buf)) => {
                for (c, o) in out.iter_mut().enumerate() {
                    o.extend(buf.chan(c).iter().map(|&s| (s * 32768.0).clamp(-32768.0, 32767.0) as i16));
                }
            }
            Ok(_) => return Err(MusicError::Unsupported("AAC : format de sortie inattendu".into())),
            // Une trame abîmée : du silence plutôt que d'abandonner.
            Err(symphonia_core::errors::Error::DecodeError(_)) => out.iter_mut().for_each(|o| o.extend(std::iter::repeat_n(0, 1024))),
            Err(e) => return Err(err(e)),
        }
    }
    for o in &mut out {
        o.truncate(max_samples);
    }
    Ok(Decoded { sample_rate: first.sample_rate, channels: out, loop_start: Some(0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(len: usize, crc: bool) -> [u8; 7] {
        // MPEG-4, LC (profil 1), 48 kHz (index 3), 2 canaux, 1 bloc.
        [0xFF, if crc { 0xF0 } else { 0xF1 }, 0x4C, 0x80 | (len >> 11) as u8, (len >> 3) as u8, ((len & 7) << 5) as u8 | 0x1F, 0xFC]
    }

    #[test]
    fn splits_adts_frames() {
        let mut d = header(9 + 3, true).to_vec();
        d.extend([0, 0, 1, 2, 3]);
        d.extend(header(7 + 2, false));
        d.extend([4, 5]);
        let f = adts_frames(&d).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].sample_rate, f[0].channels, f[0].profile), (48000, 2, 2));
        assert_eq!(f[0].payload, [1, 2, 3]);
        assert_eq!(f[1].payload, [4, 5]);
        assert!(adts_frames(b"garbage").is_err());
    }
}
