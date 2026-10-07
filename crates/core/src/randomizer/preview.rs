//! Aperçu d'une aventure randomisée, avant d'écrire la ROM : starters tirés, Pokémon de
//! la première route et équipe du premier champion. La randomisation tourne pour de vrai
//! (même seed, même résultat que la ROM écrite ensuite), mais en mémoire pour un jeu DS
//! et dans un dossier LayeredFS temporaire pour un jeu 3DS (seuls les fichiers modifiés
//! sont écrits, quelques Mo : la ROM de plusieurs Go n'est jamais recopiée).

use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use super::ctr::{CtrOutput, LayeredFsTarget};
use super::{randomize, Outcome, PokemonRef, Settings};
use crate::battle::trainers::RomTrainers;
use crate::ctr_rom::CtrGameRom;
use crate::dex;
use crate::nuzlocke::{self, LeaderKind, RomInfo};
use crate::rom::{GameRom, RomError};
use kaleido_formats::romfs::RomFsSource;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMon {
    pub species: u16,
    pub name: String,
    pub min_level: u8,
    pub max_level: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRoute {
    pub name: String,
    pub encounters: Vec<PreviewMon>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewTrainer {
    pub name: String,
    pub label: String,
    pub team: Vec<PreviewMon>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub seed: u64,
    pub share_code: String,
    pub starters: Vec<PokemonRef>,
    pub first_route: Option<PreviewRoute>,
    pub first_leader: Option<PreviewTrainer>,
    /// Durée du calcul, pour vérifier l'objectif des 3 secondes.
    pub elapsed_ms: u64,
}

fn species_name(id: u16) -> String {
    dex::species_name(id).map_or_else(|| format!("n°{id}"), str::to_string)
}

fn build(outcome: Outcome, info: Option<RomInfo>, trainers: Option<RomTrainers>, started: Instant) -> Preview {
    let first_route = info.as_ref().and_then(|i| {
        let mut routes: Vec<_> = i.routes.iter().filter(|r| !r.encounters.is_empty()).collect();
        routes.sort_by_key(|r| r.order);
        routes.first().map(|r| PreviewRoute {
            name: r.name.clone(),
            encounters: r
                .encounters
                .iter()
                .map(|e| PreviewMon { species: e.species, name: species_name(e.species), min_level: e.min_level, max_level: e.max_level })
                .collect(),
        })
    });
    let first_leader = info.as_ref().zip(trainers.as_ref()).and_then(|(i, t)| {
        let leader = i.leaders.iter().find(|l| l.kind == LeaderKind::Gym)?;
        let team = t.team(*leader.trainer_ids.first()?)?;
        Some(PreviewTrainer {
            name: leader.name.to_string(),
            label: leader.label.clone(),
            team: team.iter().map(|c| PreviewMon { species: c.species, name: species_name(c.species), min_level: c.level, max_level: c.level }).collect(),
        })
    });
    Preview {
        seed: outcome.seed,
        share_code: outcome.share_code,
        // Noms en français, quelle que soit la langue de la ROM.
        starters: outcome.starters.into_iter().map(|s| PokemonRef { name: species_name(s.id), id: s.id }).collect(),
        first_route,
        first_leader,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

/// Aperçu d'un jeu DS : randomisation en mémoire, rien n'est écrit.
pub fn preview_nds(path: &Path, settings: &Settings, seed: u64) -> Result<Preview, RomError> {
    let started = Instant::now();
    let mut game = GameRom::open(path)?;
    let outcome = randomize(&mut game, settings, seed)?;
    let info = nuzlocke::rom::read(&game).ok();
    let trainers = RomTrainers::from_nds(&game).ok();
    Ok(build(outcome, info, trainers, started))
}

/// Aperçu d'un jeu 3DS : mod LayeredFS dans `scratch` (dossier temporaire, vidé ensuite),
/// relu par-dessus le jeu d'origine.
pub fn preview_ctr(path: &Path, settings: &Settings, seed: u64, scratch: &Path) -> Result<Preview, RomError> {
    let started = Instant::now();
    let game = CtrGameRom::open(path)?;
    let mut fast = settings.clone();
    // Jamais de ROM complète pour un aperçu : seulement les fichiers modifiés.
    fast.ctr_output = CtrOutput::default();
    let _ = std::fs::remove_dir_all(scratch);
    std::fs::create_dir_all(scratch).map_err(kaleido_formats::FormatError::from)?;
    let result = (|| {
        let (outcome, written) = super::ctr::randomize(&game, &fast, seed, scratch, LayeredFsTarget::Emulator)?;
        let romfs = written.romfs.ok_or_else(|| RomError::Unsupported("aucun fichier modifié".into()))?;
        let layered = CtrGameRom::from_romfs(RomFsSource::layered(RomFsSource::open(path)?, &romfs)?)?;
        let info = nuzlocke::rom_ctr::read(&layered).ok();
        let trainers = RomTrainers::from_ctr(&layered).ok();
        Ok(build(outcome, info, trainers, started))
    })();
    let _ = std::fs::remove_dir_all(scratch);
    result
}

/// Aperçu d'un jeu GBA : randomisation en mémoire, rien n'est écrit.
pub fn preview_gba(path: &Path, settings: &Settings, seed: u64) -> Result<Preview, RomError> {
    let started = Instant::now();
    let mut game = crate::gba_rom::GbaGameRom::open(path)?;
    preview_gba_rom(&mut game, settings, seed, started)
}

pub(crate) fn preview_gba_rom(game: &mut crate::gba_rom::GbaGameRom, settings: &Settings, seed: u64, started: Instant) -> Result<Preview, RomError> {
    let outcome = super::gba::randomize(game, settings, seed)?;
    let info = nuzlocke::rom_gba::read(game).ok();
    let mut preview = build(outcome, info.clone(), None, started);
    // Premier champion : équipe lue directement dans la table des dresseurs Gen 3.
    preview.first_leader = info.as_ref().and_then(|i| {
        let leader = i.leaders.iter().find(|l| l.kind == LeaderKind::Gym)?;
        let t = crate::data::gen3::trainer(game, *leader.trainer_ids.first()? as usize).ok().filter(|t| !t.party.is_empty())?;
        Some(PreviewTrainer {
            name: leader.name.to_string(),
            label: leader.label.clone(),
            team: t.party.iter().map(|m| PreviewMon { species: m.species, name: species_name(m.species), min_level: m.level as u8, max_level: m.level as u8 }).collect(),
        })
    });
    Ok(preview)
}

#[cfg(test)]
mod gba_tests {
    use super::*;
    use crate::gba_rom::{synthetic, GbaGameRom};

    #[test]
    fn preview_synthetic_firered() {
        let mut rom = synthetic::build("BPRF", 0);
        synthetic::add_grass(&mut rom, 3, 19, &[16, 16, 19, 19, 16, 19, 16, 19, 10, 10, 13, 13]);
        let mut g = GbaGameRom::from_rom(rom).unwrap();
        let settings = Settings { starters: super::super::StarterMode::Random, wild: super::super::WildMode::Area, ..Settings::default() };
        let p = preview_gba_rom(&mut g, &settings, 5, Instant::now()).unwrap();
        assert_eq!(p.starters.len(), 3);
        let route = p.first_route.unwrap();
        assert_eq!(route.encounters.len(), 4);
        // Dresseurs vides dans la ROM synthétique : pas d'équipe de champion.
        assert!(p.first_leader.is_none());
    }
}

/// Aperçu d'un jeu Game Boy : randomisation en mémoire ; « première route » = première zone
/// d'herbe de la table des rencontres (pas encore de noms de lieux ni de champions en Gen 1 / 2).
pub fn preview_gb(path: &Path, settings: &Settings, seed: u64) -> Result<Preview, RomError> {
    let started = Instant::now();
    let mut game = crate::gb_rom::GbGameRom::open(path)?;
    preview_gb_rom(&mut game, settings, seed, started)
}

pub(crate) fn preview_gb_rom(game: &mut crate::gb_rom::GbGameRom, settings: &Settings, seed: u64, started: Instant) -> Result<Preview, RomError> {
    let outcome = super::gb::randomize(game, settings, seed)?;
    let mut preview = build(outcome, None, None, started);
    preview.first_route = crate::data::gen12::wild_areas(game).ok().and_then(|areas| {
        let area = areas.into_iter().find(|a| !a.slots.is_empty())?;
        let mut encounters: Vec<PreviewMon> = Vec::new();
        for s in area.slots.iter().filter(|s| s.species != 0) {
            match encounters.iter_mut().find(|e| e.species == s.species) {
                Some(e) => {
                    e.min_level = e.min_level.min(s.level);
                    e.max_level = e.max_level.max(s.level);
                }
                None => encounters.push(PreviewMon { species: s.species, name: species_name(s.species), min_level: s.level, max_level: s.level }),
            }
        }
        Some(PreviewRoute { name: area.label, encounters })
    });
    Ok(preview)
}

#[cfg(test)]
mod gb_preview_tests {
    use super::*;

    #[test]
    fn preview_synthetic_red() {
        let mut g = crate::gb_rom::GbGameRom::from_rom(crate::gb_rom::synthetic::build("Red (F)")).unwrap();
        let s = Settings { starters: super::super::StarterMode::Random, ..Settings::default() };
        let p = preview_gb_rom(&mut g, &s, 9, Instant::now()).unwrap();
        assert_eq!(p.starters.len(), 3);
        assert!(p.first_route.is_some_and(|r| !r.encounters.is_empty()));
    }
}
