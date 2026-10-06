//! Musique des jeux : thème de l'écran titre, extrait de la ROM de l'utilisateur
//! et rendu en PCM, pour l'aperçu sonore de la bibliothèque.
//!
//! - DS (Gen 4 et 5) : séquences SSEQ du fichier SDAT, jouées par un petit
//!   synthétiseur qui imite le mixeur de la DS (`nds.rs`).
//! - 3DS (Gen 6 et 7) : flux BCSTM (DSP-ADPCM) décodés (`ctr.rs`).

use std::path::Path;

use crate::games::Game;

pub mod ctr;
pub mod nds;
pub mod nx;

/// Son PCM 16 bits stéréo entrelacé.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub sample_rate: u32,
    /// Échantillons gauche/droite entrelacés.
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Durée en secondes.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / 2.0 / self.sample_rate as f32
    }

    /// Fichier WAV (RIFF PCM 16 bits stéréo).
    pub fn to_wav(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&2u16.to_le_bytes()); // stéréo
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&(self.sample_rate * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    /// Fondu de sortie sur les `seconds` dernières secondes.
    pub fn fade_out(&mut self, seconds: f32) {
        let frames = self.samples.len() / 2;
        let fade = ((seconds * self.sample_rate as f32) as usize).min(frames);
        for i in 0..fade {
            let frame = frames - fade + i;
            let gain = 1.0 - i as f32 / fade as f32;
            for c in 0..2 {
                let s = &mut self.samples[frame * 2 + c];
                *s = (*s as f32 * gain) as i16;
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MusicError {
    #[error("{0}")]
    Format(#[from] kaleido_formats::FormatError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("musique introuvable : {0}")]
    NotFound(String),
    #[error("format audio non pris en charge : {0}")]
    Unsupported(String),
}

/// Thème de l'écran titre de `game`, lu dans la ROM (`.nds`, `.3ds`/`.cci`/`.cxi`
/// ou dossier extrait), limité à `max_seconds` avec un fondu de sortie.
pub fn title_theme(path: &Path, game: Game, max_seconds: f32) -> Result<Pcm, MusicError> {
    let mut pcm = if game.generation() >= 6 { ctr::title_theme(path, game, max_seconds)? } else { nds::title_theme(path, game, max_seconds)? };
    pcm.fade_out(3.0_f32.min(max_seconds / 4.0));
    Ok(pcm)
}
