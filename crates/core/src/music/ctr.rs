//! Musique des jeux 3DS (Gen 6 et 7) : thème de l'écran titre.
//!
//! Où se trouve la musique de l'écran titre (relevé sur les ROM européennes) :
//!
//! | Jeu | Fichier du RomFS | Format |
//! |-----|------------------|--------|
//! | X / Y | `sound/bgm_xy_title_01.aac` | AAC-LC ADTS, 48 kHz stéréo, 12,8 s, rejoué en boucle |
//! | Rubis Oméga / Saphir Alpha | `sound/bgm_sg_title_01.dspadpcm.bcstm` | BCSTM DSP-ADPCM, 32 728 Hz stéréo, 45,4 s, sans boucle |
//! | Soleil / Lune, Ultra-Soleil / Ultra-Lune | `m/title_fr.moflex` | vidéo Mobiclip, audio IMA-ADPCM 32 728 Hz stéréo (Lune 106,8 s, Ultra-Soleil 88,4 s), rejouée en boucle |
//!
//! - X/Y et ROSA : le son `STRM_BGM_TITLE_01` (son n° 1) de l'archive
//!   `sound/xy_sound.bcsar` / `sound/sango_sound.bcsar` pointe vers ce fichier
//!   (vérifié dans la table de chaînes et la table des fichiers du BCSAR ; dans
//!   ROSA, `STRM_BGM_TITLE_02` pointe vers `bgm_xy_silence`). Les ouvertures
//!   (`bgm_xy_demo_opening`, `bgm_sg_demo_*`) sont des fichiers distincts.
//! - X/Y : la vidéo d'apparition du titre (`m/a.moflex`, PCM 44,1 kHz) porte le même
//!   morceau de 12,8 s (profil d'énergie identique), puis l'écran titre tourne sur une
//!   vidéo muette (`m/bx.moflex` / `m/by.moflex`) : le flux AAC est donc rejoué en
//!   boucle (le BCSAR ne donne pas de point de boucle ; boucle sur tout le fichier supposée).
//! - Gen 7 : l'archive `data/sound/niji_sound.bcsar` ne contient aucun son « TITLE » :
//!   l'écran titre est une vidéo par langue (`m/title_<langue>.moflex`) dont la
//!   bande-son est le thème. Vérifié sur Lune et Ultra-Soleil ; Soleil et Ultra-Lune
//!   supposés identiques.
//!
//! Les autres flux du jeu (`--list` de l'exemple `music_ctr`) se décodent avec [`stream`].

use std::path::Path;

use kaleido_formats::romfs::RomFsSource;

use super::{MusicError, Pcm};
use crate::games::Game;

pub mod aac;
pub mod bcstm;
pub mod moflex;

/// Son décodé, avant mise en forme : un vecteur par canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub sample_rate: u32,
    pub channels: Vec<Vec<i16>>,
    /// Début de boucle : la fin des données revient ici.
    pub loop_start: Option<usize>,
}

impl Decoded {
    /// PCM stéréo d'au plus `max_samples` échantillons, en bouclant si le flux boucle.
    /// Mono : dupliqué ; plus de deux canaux : les deux premiers (la première piste).
    pub fn render(&self, max_samples: usize) -> Pcm {
        let len = self.channels.first().map_or(0, Vec::len);
        let (l, r) = match self.channels.as_slice() {
            [] => return Pcm { sample_rate: self.sample_rate, samples: Vec::new() },
            [m] => (m, m),
            [l, r, ..] => (l, r),
        };
        let total = match self.loop_start {
            Some(s) if s < len => max_samples,
            _ => max_samples.min(len),
        };
        let mut samples = Vec::with_capacity(total * 2);
        let mut i = 0;
        for _ in 0..total {
            if i >= len {
                i = self.loop_start.unwrap_or(0);
            }
            samples.push(l[i]);
            samples.push(r.get(i).copied().unwrap_or(0));
            i += 1;
        }
        Pcm { sample_rate: self.sample_rate, samples }
    }
}

/// Fichiers candidats pour le thème de l'écran titre, dans l'ordre de préférence.
pub fn title_files(game: Game) -> &'static [&'static str] {
    match game {
        Game::X | Game::Y => &["sound/bgm_xy_title_01.aac"],
        Game::OmegaRuby | Game::AlphaSapphire => &["sound/bgm_sg_title_01.dspadpcm.bcstm"],
        Game::Sun | Game::Moon | Game::UltraSun | Game::UltraMoon => &["m/title_fr.moflex", "m/title_us.moflex", "m/title_jp.moflex"],
        _ => &[],
    }
}

/// Thème de l'écran titre, au plus `max_seconds` secondes.
pub fn title_theme(path: &Path, game: Game, max_seconds: f32) -> Result<Pcm, MusicError> {
    let src = RomFsSource::open(path)?;
    let file = title_files(game)
        .iter()
        .copied()
        .find(|f| src.contains(f))
        .or_else(|| {
            // Autre langue de la vidéo de titre (Gen 7).
            (game.generation() == 7)
                .then(|| src.files().iter().map(|f| f.path.as_str()).find(|p| p.starts_with("m/title_") && p.ends_with(".moflex")))
                .flatten()
        })
        .ok_or_else(|| MusicError::NotFound(format!("thème de l'écran titre de {}", game.name_fr())))?;
    decode_from(&src, file, max_seconds)
}

/// Flux audio du RomFS (BCSTM, AAC, vidéos moflex), triés par chemin.
pub fn list_streams(path: &Path) -> Result<Vec<String>, MusicError> {
    let src = RomFsSource::open(path)?;
    Ok(src.files().iter().map(|f| f.path.clone()).filter(|p| is_stream(p)).collect())
}

fn is_stream(p: &str) -> bool {
    let lower = p.to_ascii_lowercase();
    lower.ends_with(".bcstm") || (lower.ends_with(".aac") && lower.contains("sound")) || lower.ends_with(".moflex")
}

/// Décode le flux `file` du RomFS (chemin tel que listé par [`list_streams`]).
pub fn stream(path: &Path, file: &str, max_seconds: f32) -> Result<Pcm, MusicError> {
    decode_from(&RomFsSource::open(path)?, file, max_seconds)
}

fn decode_from(src: &RomFsSource, file: &str, max_seconds: f32) -> Result<Pcm, MusicError> {
    let size = src.size(file).ok_or_else(|| MusicError::NotFound(file.to_string()))?;
    let lower = file.to_ascii_lowercase();
    let max_at = |rate: u32| (max_seconds.max(0.0) as f64 * rate as f64).round() as usize;
    if lower.ends_with(".moflex") {
        let mut r = moflex::Chunked::new(size, |at, n| Ok(src.read_range(file, at, n)?));
        // On ne connaît la fréquence qu'après l'en-tête : marge pour 48 kHz.
        let d = moflex::decode(&mut r, max_at(48000))?;
        return Ok(d.render(max_at(d.sample_rate)));
    }
    if lower.ends_with(".aac") {
        let d = aac::decode(&src.read(file)?, max_at(48000))?;
        return Ok(d.render(max_at(d.sample_rate)));
    }
    // BCSTM : on ne lit que l'en-tête et les blocs nécessaires.
    let first = src.read_range(file, 0, 0x40.min(size as usize))?;
    let head_len = bcstm::Bcstm::header_len(&first)?.min(size as usize);
    let st = bcstm::Bcstm::parse(&src.read_range(file, 0, head_len)?)?;
    let max = max_at(st.sample_rate);
    let wanted = if st.loop_start.is_some() { st.samples } else { max };
    let len = st.data_len_for(wanted).min((size as usize).saturating_sub(st.data_offset));
    let data = src.read_range(file, st.data_offset as u64, len)?;
    Ok(st.to_decoded(&data, max)?.render(max))
}

pub(crate) fn rd16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

pub(crate) fn rd32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_loops_and_maps_channels() {
        let d = Decoded { sample_rate: 10, channels: vec![vec![1, 2, 3, 4]], loop_start: Some(2) };
        assert_eq!(d.render(7).samples, [1, 1, 2, 2, 3, 3, 4, 4, 3, 3, 4, 4, 3, 3]);
        let d = Decoded { sample_rate: 10, channels: vec![vec![1, 2], vec![-1, -2], vec![9, 9]], loop_start: None };
        assert_eq!(d.render(5).samples, [1, -1, 2, -2]);
        assert_eq!(d.render(1).seconds(), 0.1);
    }

    #[test]
    fn synthetic_bcstm_loops() {
        let d = bcstm::decode_file(&bcstm::tests::synthetic(), 100).unwrap();
        let pcm = d.render(50);
        assert_eq!(pcm.samples.len(), 100);
        // Échantillon 42 = début de boucle (14).
        assert_eq!(pcm.samples[42 * 2], d.channels[0][14]);
    }

    /// ROM présentes dans `~/Documents/NDS & 3DS` (ou `KALEIDO_ROMS`).
    fn roms() -> Vec<(std::path::PathBuf, Game)> {
        let Some(dir) = std::env::var_os("KALEIDO_ROMS").map(std::path::PathBuf::from).or_else(|| {
            let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
            Some(std::path::PathBuf::from(home).join("Documents").join("NDS & 3DS"))
        }) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("3ds")))
            .filter_map(|p| {
                let game = RomFsSource::open(&p).ok()?.title_id().and_then(Game::from_title_id)?;
                Some((p, game))
            })
            .collect()
    }

    /// Statistiques simples : RMS, crête, part d'énergie haute fréquence (différence
    /// d'échantillons successifs) — un décodage ADPCM faux donne du bruit blanc (≈ 2).
    pub(crate) fn stats(pcm: &Pcm) -> (f64, i32, f64) {
        let s = &pcm.samples;
        let left: Vec<f64> = s.iter().step_by(2).map(|&v| v as f64).collect();
        let energy: f64 = left.iter().map(|v| v * v).sum();
        let diff: f64 = left.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
        let rms = (s.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / s.len().max(1) as f64).sqrt();
        let peak = s.iter().map(|&v| (v as i32).abs()).max().unwrap_or(0);
        (rms, peak, diff / energy.max(1.0))
    }

    #[test]
    #[ignore = "nécessite les ROM 3DS"]
    fn title_themes_of_real_roms() {
        for (rom, game) in roms() {
            let t = std::time::Instant::now();
            let pcm = title_theme(&rom, game, 60.0).unwrap();
            let (rms, peak, hf) = stats(&pcm);
            eprintln!("{game:?} : {} Hz, {:.1} s, RMS {rms:.0}, crête {peak}, HF {hf:.4}, {:?}", pcm.sample_rate, pcm.seconds(), t.elapsed());
            assert!(pcm.seconds() > 10.0);
            assert!(rms > 500.0 && peak > 4000, "trop faible");
            assert!(hf < 0.3, "ressemble à du bruit");
        }
    }

    /// Le bloc SEEK d'un BCSTM donne, pour chaque bloc et chaque canal, les deux
    /// échantillons qui le précèdent tels que vus par l'encodeur : le décodage doit
    /// les retrouver à l'erreur de quantification près (un décodage faux diverge).
    #[test]
    #[ignore = "nécessite une ROM Rubis Oméga / Saphir Alpha"]
    fn dsp_matches_seek_table() {
        for (rom, game) in roms().into_iter().filter(|(_, g)| matches!(g, Game::OmegaRuby | Game::AlphaSapphire)) {
            let file = RomFsSource::open(&rom).unwrap().read(title_files(game)[0]).unwrap();
            let st = bcstm::Bcstm::parse(&file).unwrap();
            let pcm = st.decode(&file[st.data_offset..], usize::MAX).unwrap();
            let seek = (0..rd16(&file, 0x10) as usize)
                .map(|i| 0x14 + i * 12)
                .find(|&r| rd16(&file, r) == 0x4001)
                .map(|r| rd32(&file, r + 4) as usize)
                .unwrap();
            let ch = st.channels.len();
            let mut worst = 0;
            for b in 1..st.block_count {
                for (c, decoded) in pcm.iter().enumerate() {
                    let e = seek + 8 + (b * ch + c) * 4;
                    let (h1, h2) = (rd16(&file, e) as i16 as i32, rd16(&file, e + 2) as i16 as i32);
                    let at = b * st.block_samples;
                    worst = worst.max((h1 - decoded[at - 1] as i32).abs()).max((h2 - decoded[at - 2] as i32).abs());
                }
            }
            eprintln!("{game:?} : écart maximal avec la table SEEK {worst} sur {} blocs", st.block_count);
            assert!(worst < 512);
        }
    }
}
