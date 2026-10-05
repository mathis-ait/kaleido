//! Commandes « Préparer un combat » : ROM liée à la sauvegarde ouverte, liste
//! des dresseurs, matrice équipe × équipe et duel détaillé.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use kaleido_core::battle::trainers::{RomTrainers, TrainerSummary};
use kaleido_core::battle::{self, party, Cell, Combatant, Duel, Field, SideState};
use kaleido_core::dex;
use kaleido_core::save::session::SlotView;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::blocking;
use crate::saves::OpenSave;

/// ROM liée (données de combat lues une fois, gardées en mémoire).
#[derive(Default)]
pub struct LinkedRom(Mutex<Option<(PathBuf, Arc<RomTrainers>)>>);

impl LinkedRom {
    fn get(&self) -> Result<Arc<RomTrainers>, String> {
        let slot = self.0.lock().map_err(|e| e.to_string())?;
        slot.as_ref().map(|(_, r)| r.clone()).ok_or_else(|| "aucune ROM liée".to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInfo {
    path: String,
    game: &'static str,
    verified: bool,
    trainers: Vec<TrainerSummary>,
}

/// Réglages de l'interface : météo, Intimidation, état de chaque camp.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    field: Field,
    mine: SideState,
    theirs: SideState,
}

fn info(path: &std::path::Path, rom: &RomTrainers) -> LinkInfo {
    LinkInfo { path: path.display().to_string(), game: rom.game.name_fr(), verified: rom.verified, trainers: rom.summaries() }
}

fn save_game(save: &OpenSave) -> Result<dex::Game, String> {
    save.with(|s| Ok(s.game()))
}

/// Lie une ROM (d'origine ou randomisée) à la sauvegarde ouverte.
#[tauri::command]
pub async fn battle_link_rom(path: PathBuf, app: AppHandle) -> Result<LinkInfo, String> {
    let wanted = save_game(&app.state::<OpenSave>())?;
    let p = path.clone();
    let rom = blocking(move || RomTrainers::open(&p).map_err(|e| e.to_string())).await?;
    if dex::Game::from(rom.game) != wanted {
        return Err(format!("{} ne correspond pas au jeu de la sauvegarde ouverte.", rom.game.name_fr()));
    }
    let out = info(&path, &rom);
    *app.state::<LinkedRom>().0.lock().map_err(|e| e.to_string())? = Some((path, Arc::new(rom)));
    Ok(out)
}

/// ROM déjà liée, si elle correspond toujours à la sauvegarde ouverte.
#[tauri::command]
pub fn battle_linked(linked: State<'_, LinkedRom>, save: State<'_, OpenSave>) -> Result<Option<LinkInfo>, String> {
    let Ok(game) = save_game(&save) else {
        return Ok(None);
    };
    let slot = linked.0.lock().map_err(|e| e.to_string())?;
    Ok(slot.as_ref().filter(|(_, r)| dex::Game::from(r.game) == game).map(|(p, r)| info(p, r)))
}

#[tauri::command]
pub fn battle_unlink(linked: State<'_, LinkedRom>) -> Result<(), String> {
    *linked.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

/// Équipe de la sauvegarde ouverte, prête pour le calcul.
fn my_team(app: &AppHandle, rom: &RomTrainers) -> Result<(dex::Game, Vec<Combatant>), String> {
    let (game, party): (dex::Game, Vec<SlotView>) = app.state::<OpenSave>().with(|s| Ok((s.game(), s.view()?.party)))?;
    let team: Vec<Combatant> = party.iter().filter_map(|v| party::from_slot(game, v, Some(rom))).collect();
    if team.is_empty() {
        return Err("l'équipe de la sauvegarde est vide".into());
    }
    Ok((game, team))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixView {
    trainer: Option<TrainerSummary>,
    mine: Vec<Combatant>,
    theirs: Vec<Combatant>,
    /// `cells[i][j]` : mon Pokémon `i` contre son Pokémon `j`.
    cells: Vec<Vec<Cell>>,
}

/// Matrice équipe × équipe contre un dresseur.
#[tauri::command]
pub async fn battle_matrix(trainer: u16, options: Options, app: AppHandle) -> Result<MatrixView, String> {
    let rom = app.state::<LinkedRom>().get()?;
    let (game, mine) = my_team(&app, &rom)?;
    blocking(move || {
        let theirs = rom.team(trainer).ok_or("dresseur introuvable")?;
        let cells = battle::matrix(game, &mine, &options.mine, &theirs, &options.theirs, &options.field);
        let summary = rom.summaries().into_iter().find(|s| s.id == trainer);
        Ok(MatrixView { trainer: summary, mine, theirs, cells })
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuelView {
    me: Combatant,
    them: Combatant,
    /// Mes attaques sur lui.
    attack: Duel,
    /// Ses attaques sur moi.
    defense: Duel,
}

/// Détail d'une case : toutes les attaques des deux côtés.
#[tauri::command]
pub async fn battle_duel(trainer: u16, mine: usize, theirs: usize, options: Options, app: AppHandle) -> Result<DuelView, String> {
    let rom = app.state::<LinkedRom>().get()?;
    let (game, team) = my_team(&app, &rom)?;
    blocking(move || {
        let me = team.get(mine).cloned().ok_or("Pokémon introuvable dans l'équipe")?;
        let them = rom.team(trainer).and_then(|t| t.get(theirs).cloned()).ok_or("Pokémon adverse introuvable")?;
        let attack = battle::duel(game, &me, &options.mine, &them, &options.theirs, &options.field);
        let defense = battle::duel(game, &them, &options.theirs, &me, &options.mine, &options.field);
        Ok(DuelView { me, them, attack, defense })
    })
    .await
}
