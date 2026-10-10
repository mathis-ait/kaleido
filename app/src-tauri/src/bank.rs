//! Commandes de la Banque Kaleido (PC partagé par toutes les sauvegardes).
//!
//! Le dossier de la banque est, par défaut, `banque/` dans le dossier de données de
//! l'application ; un autre dossier peut être choisi (mémorisé dans `banque.json`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kaleido_core::bank::{self, Bank, BankBoxInfo, BankDetail, BankQuery, BankSlot, BankSlotView};
use kaleido_core::save::convert;
use kaleido_core::save::session::{SaveView, Slot};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::saves::OpenSave;

/// Banque ouverte (chargée au premier usage).
#[derive(Default)]
pub struct OpenBank(Mutex<Option<Bank>>);

#[derive(Serialize, Deserialize, Default)]
struct BankConfig {
    path: Option<PathBuf>,
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn default_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("banque"))
}

fn config_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("banque.json"))
}

fn configured_path(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg: BankConfig = std::fs::read(config_file(app)?).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match cfg.path {
        Some(p) => Ok(p),
        None => default_path(app),
    }
}

impl OpenBank {
    pub(crate) fn with<T>(&self, app: &AppHandle, f: impl FnOnce(&mut Bank) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        if slot.is_none() {
            *slot = Some(Bank::open(configured_path(app)?).map_err(|e| e.to_string())?);
        }
        f(slot.as_mut().expect("banque ouverte"))
    }
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankInfo {
    path: String,
    default_path: String,
    trash_path: String,
    count: usize,
    boxes: Vec<BankBoxInfo>,
}

fn info(app: &AppHandle, bank: &Bank) -> Result<BankInfo, String> {
    Ok(BankInfo {
        path: bank.root().display().to_string(),
        default_path: default_path(app)?.display().to_string(),
        trash_path: bank.root().join("corbeille").display().to_string(),
        count: bank.count(),
        boxes: bank.boxes(),
    })
}

#[tauri::command]
pub fn bank_info(app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    state.with(&app, |b| info(&app, b))
}

/// Change le dossier de la banque (`None` = dossier par défaut).
#[tauri::command]
pub fn bank_set_path(path: Option<PathBuf>, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    let target = match &path {
        Some(p) => p.clone(),
        None => default_path(&app)?,
    };
    let bank = Bank::open(&target).map_err(err)?;
    std::fs::create_dir_all(data_dir(&app)?).map_err(err)?;
    std::fs::write(config_file(&app)?, serde_json::to_vec_pretty(&BankConfig { path }).map_err(err)?).map_err(err)?;
    let result = info(&app, &bank);
    *state.0.lock().map_err(err)? = Some(bank);
    result
}

#[tauri::command]
pub fn bank_box(index: usize, app: AppHandle, state: State<'_, OpenBank>) -> Result<Vec<Option<BankSlotView>>, String> {
    state.with(&app, |b| b.box_view(index).map_err(err))
}

/// Pour chaque case : le Pokémon peut-il aller dans la sauvegarde ouverte ? (`None` : case vide)
#[tauri::command]
pub fn bank_box_compat(index: usize, app: AppHandle, state: State<'_, OpenBank>, save: State<'_, OpenSave>) -> Result<Vec<Option<bool>>, String> {
    let game = save.with_open(|s| Ok(s.map(|(_, s)| s.game())))?;
    state.with(&app, |b| {
        let slots = b.box_view(index).map_err(err)?;
        Ok(slots
            .iter()
            .map(|v| {
                let v = v.as_ref()?;
                let Some(game) = game else { return Some(true) };
                Some(b.read(v.slot).map(|p| convert::compatibility(&p, game)).is_ok_and(|c| c.ok || c.fixable))
            })
            .collect())
    })
}

#[tauri::command]
pub fn bank_detail(slot: BankSlot, app: AppHandle, state: State<'_, OpenBank>, save: State<'_, OpenSave>) -> Result<BankDetail, String> {
    let game = save.with_open(|s| Ok(s.map(|(_, s)| s.game())))?;
    state.with(&app, |b| b.detail(slot, game).map_err(err))
}

#[tauri::command]
pub fn bank_search(query: BankQuery, app: AppHandle, state: State<'_, OpenBank>) -> Result<Vec<BankSlotView>, String> {
    state.with(&app, |b| Ok(b.search(&query)))
}

#[tauri::command]
pub fn bank_move(from: BankSlot, to: BankSlot, app: AppHandle, state: State<'_, OpenBank>) -> Result<(), String> {
    state.with(&app, |b| b.move_slot(from, to).map_err(err))
}

#[tauri::command]
pub fn bank_delete(slot: BankSlot, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    state.with(&app, |b| {
        b.remove(slot).map_err(err)?;
        info(&app, b)
    })
}

#[tauri::command]
pub fn bank_add_box(name: Option<String>, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    state.with(&app, |b| {
        b.add_box(name).map_err(err)?;
        info(&app, b)
    })
}

#[tauri::command]
pub fn bank_rename_box(index: usize, name: String, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    state.with(&app, |b| {
        b.rename_box(index, &name).map_err(err)?;
        info(&app, b)
    })
}

#[tauri::command]
pub fn bank_delete_box(index: usize, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankInfo, String> {
    state.with(&app, |b| {
        b.delete_box(index).map_err(err)?;
        info(&app, b)
    })
}

/// Importe des fichiers `.pk4` à `.pk7` (le premier dans `to` si donné, les autres
/// dans les cases libres). Renvoie la case du dernier.
#[tauri::command]
pub fn bank_import(files: Vec<PathBuf>, to: Option<BankSlot>, app: AppHandle, state: State<'_, OpenBank>) -> Result<BankSlot, String> {
    state.with(&app, |b| {
        let mut last = None;
        for (i, file) in files.iter().enumerate() {
            let bytes = std::fs::read(file).map_err(|e| format!("lecture impossible : {e}"))?;
            let ext = file.extension().and_then(|e| e.to_str());
            let origin = bank::Origin { game: None, save: file.file_name().map(|n| n.to_string_lossy().into_owned()) };
            last = Some(b.import_bytes(&bytes, ext, if i == 0 { to } else { None }, &origin).map_err(err)?);
        }
        last.ok_or_else(|| "aucun fichier".to_string())
    })
}

#[tauri::command]
pub fn bank_export(slot: BankSlot, output: PathBuf, app: AppHandle, state: State<'_, OpenBank>) -> Result<(), String> {
    let (bytes, _) = state.with(&app, |b| b.export(slot).map_err(err))?;
    std::fs::write(output, bytes).map_err(err)
}

fn file_name(p: &Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    view: SaveView,
    bank: BankInfo,
    /// Case de la banque (dépôt) ou de la sauvegarde (retrait) utilisée.
    bank_slot: Option<BankSlot>,
    save_slot: Option<Slot>,
}

/// Sauvegarde → banque. `copy = false` : le Pokémon quitte la sauvegarde.
#[tauri::command]
pub fn bank_deposit(
    from: Slot,
    to: Option<BankSlot>,
    copy: bool,
    app: AppHandle,
    state: State<'_, OpenBank>,
    save: State<'_, OpenSave>,
) -> Result<TransferResult, String> {
    state.with(&app, |b| {
        save.with_open(|open| {
            let (path, s) = open.ok_or("aucune sauvegarde ouverte")?;
            let slot = bank::deposit_from_save(b, s, from, to, copy, file_name(path)).map_err(err)?;
            Ok(TransferResult { view: s.view().map_err(err)?, bank: info(&app, b)?, bank_slot: Some(slot), save_slot: None })
        })
    })
}

/// Banque → sauvegarde, avec conversion au format du jeu. `strip` : retire les
/// attaques / l'objet absents du jeu au lieu de refuser.
#[tauri::command]
pub fn bank_withdraw(
    from: BankSlot,
    to: Slot,
    copy: bool,
    strip: bool,
    app: AppHandle,
    state: State<'_, OpenBank>,
    save: State<'_, OpenSave>,
) -> Result<TransferResult, String> {
    state.with(&app, |b| {
        save.with_open(|open| {
            let (_, s) = open.ok_or("ouvre d'abord une sauvegarde pour y retirer ce Pokémon")?;
            let v = bank::withdraw_to_save(b, s, from, to, copy, strip).map_err(err)?;
            Ok(TransferResult { view: s.view().map_err(err)?, bank: info(&app, b)?, bank_slot: None, save_slot: Some(v.slot) })
        })
    })
}
