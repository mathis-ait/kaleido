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
