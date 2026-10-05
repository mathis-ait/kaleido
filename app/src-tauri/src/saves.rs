//! Commandes de l'éditeur de sauvegardes.

use std::path::PathBuf;
use std::sync::Mutex;

use kaleido_core::save::edit::TrainerPatch;
use kaleido_core::save::pokedex::DexEntry;
use kaleido_core::save::session::{self, PokemonPatch, SaveSession, SaveView, Slot, SlotView};
use kaleido_core::{dex, names, save};
use serde::Serialize;
use tauri::State;

/// Sauvegarde ouverte et son chemin d'origine.
#[derive(Default)]
pub struct OpenSave(Mutex<Option<(PathBuf, SaveSession)>>);

impl OpenSave {
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut SaveSession) -> Result<T, save::SaveError>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        let (_, session) = slot.as_mut().ok_or("aucune sauvegarde ouverte")?;
        f(session).map_err(|e| e.to_string())
    }

    /// Lecture seule du chemin et de la session ouverts (pour les autres modules, ex. Nuzlocke).
    pub(crate) fn read<T>(&self, f: impl FnOnce(&std::path::Path, &SaveSession) -> Result<T, String>) -> Result<T, String> {
        let slot = self.0.lock().map_err(|e| e.to_string())?;
        let (path, session) = slot.as_ref().ok_or("aucune sauvegarde ouverte")?;
        f(path, session)
    }

    /// Accès à la sauvegarde ouverte et à son chemin (`None` si aucune), pour la banque.
    pub(crate) fn with_open<T>(&self, f: impl FnOnce(Option<(&std::path::Path, &mut SaveSession)>) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.0.lock().map_err(|e| e.to_string())?;
        f(slot.as_mut().map(|(p, s)| (p.as_path(), s)))
    }
}

#[tauri::command]
pub fn open_save(path: PathBuf, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
    let session = SaveSession::open(&bytes).map_err(|e| e.to_string())?;
    let view = session.view().map_err(|e| e.to_string())?;
    *state.0.lock().map_err(|e| e.to_string())? = Some((path, session));
    Ok(view)
}

#[tauri::command]
pub fn save_view(state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| s.view())
}

#[tauri::command]
pub fn save_box(index: usize, state: State<'_, OpenSave>) -> Result<Vec<Option<SlotView>>, String> {
    state.with(|s| s.box_view(index))
}

/// Déplace (ou échange) un Pokémon, puis renvoie l'état de l'équipe.
#[tauri::command]
pub fn save_move(from: Slot, to: Slot, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.move_pokemon(from, to)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_patch(slot: Slot, patch: PokemonPatch, state: State<'_, OpenSave>) -> Result<SlotView, String> {
    state.with(|s| s.patch(slot, &patch))
}

#[tauri::command]
pub fn save_delete(slot: Slot, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.delete(slot)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_export_pokemon(slot: Slot, output: PathBuf, state: State<'_, OpenSave>) -> Result<(), String> {
    let (bytes, _) = state.with(|s| s.export(slot))?;
    std::fs::write(output, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_import_pokemon(slot: Slot, file: PathBuf, state: State<'_, OpenSave>) -> Result<SlotView, String> {
    let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
    state.with(|s| s.import(slot, &bytes))
}

/// Écrit la sauvegarde. Avant d'écraser le fichier d'origine, une copie de
/// sécurité est faite à côté (`.kaleido.bak`). Renvoie le chemin de cette copie.
#[tauri::command]
pub fn save_write(output: Option<PathBuf>, state: State<'_, OpenSave>) -> Result<Option<String>, String> {
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    let (path, session) = slot.as_ref().ok_or("aucune sauvegarde ouverte")?;
    let target = output.unwrap_or_else(|| path.clone());
    let mut backup = None;
    if target.exists() {
        let mut name = target.file_name().unwrap_or_default().to_os_string();
        name.push(".kaleido.bak");
        let bak = target.with_file_name(name);
        if !bak.exists() {
            std::fs::copy(&target, &bak).map_err(|e| format!("copie de sécurité impossible : {e}"))?;
        }
        backup = Some(bak.display().to_string());
    }
    std::fs::write(&target, session.to_bytes()).map_err(|e| e.to_string())?;
    Ok(backup)
}

#[derive(Serialize)]
pub struct NameLists {
    species: &'static [String],
    moves: &'static [String],
    items: &'static [String],
}

/// Listes de noms pour les menus (espèces, attaques, objets).
#[tauri::command]
pub fn name_lists() -> NameLists {
    NameLists { species: names::all_species(), moves: names::all_moves(), items: names::all_items() }
}

/// Annule (`redo = false`) ou rétablit la dernière modification.
#[tauri::command]
pub fn save_history(redo: bool, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        if redo {
            s.redo();
        } else {
            s.undo();
        }
        s.view()
    })
}

/// Copie (`overwrite = false`) ou déplace en écrasant la cible (`overwrite = true`).
#[tauri::command]
pub fn save_copy(from: Slot, to: Slot, overwrite: bool, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        if overwrite {
            s.overwrite_pokemon(from, to)?;
        } else {
            s.clone_pokemon(from, to)?;
        }
        s.view()
    })
}

#[tauri::command]
pub fn save_create(slot: Slot, species: u16, level: u8, state: State<'_, OpenSave>) -> Result<SlotView, String> {
    state.with(|s| s.create(slot, species, level))
}

#[tauri::command]
pub fn save_all(state: State<'_, OpenSave>) -> Result<Vec<SlotView>, String> {
    state.with(|s| s.all())
}

#[tauri::command]
pub fn save_set_trainer(patch: TrainerPatch, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.set_trainer(&patch)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_set_box_name(index: usize, name: String, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.set_box_name(index, &name)?;
        s.view()
    })
}

/// Chemin de la sauvegarde ouverte.
#[tauri::command]
pub fn save_path(state: State<'_, OpenSave>) -> Result<Option<String>, String> {
    Ok(state.0.lock().map_err(|e| e.to_string())?.as_ref().map(|(p, _)| p.display().to_string()))
}

#[tauri::command]
pub fn save_inventory(state: State<'_, OpenSave>) -> Result<Vec<save::Pouch>, String> {
    state.with(|s| s.inventory())
}

#[tauri::command]
pub fn save_set_inventory(pouches: Vec<save::Pouch>, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.set_inventory(&pouches)?;
        s.view()
    })
}

/// Talents (index = numéro), pour les menus.
#[tauri::command]
pub fn ability_names() -> Vec<String> {
    let mut list: Vec<String> = (0..400u16).map(|i| names::ability(i).unwrap_or("").to_string()).collect();
    while list.last().is_some_and(String::is_empty) {
        list.pop();
    }
    list
}

/// Option d'une liste déroulante : identifiant et nom.
#[derive(Serialize)]
pub struct Named {
    value: u16,
    label: String,
}

/// Listes propres au jeu de la sauvegarde ouverte (espèces, attaques, objets, lieux…).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLists {
    species: Vec<Named>,
    moves: Vec<Named>,
    items: Vec<Named>,
    abilities: Vec<Named>,
    locations: Vec<Named>,
    balls: Vec<Named>,
    types: Vec<String>,
}

#[tauri::command]
pub fn save_lists(state: State<'_, OpenSave>) -> Result<SaveLists, String> {
    let game = state.with(|s| Ok(s.game()))?;
    let range = |max: u16, name: &dyn Fn(u16) -> Option<&'static str>| -> Vec<Named> {
        (1..=max).filter_map(|i| name(i).filter(|n| !n.is_empty() && *n != "???").map(|n| Named { value: i, label: n.to_string() })).collect()
    };
    let mut locations: Vec<Named> =
        dex::locations(game.generation()).into_iter().filter(|(_, n)| !n.is_empty()).map(|(value, n)| Named { value, label: n.to_string() }).collect();
    locations.sort_by(|a, b| (a.value != 0).cmp(&(b.value != 0)).then_with(|| a.label.cmp(&b.label)));
    Ok(SaveLists {
        species: range(dex::max_species(game), &dex::species_name),
        moves: range(dex::max_move(game), &dex::move_name),
        items: range(dex::max_item(game), &|i| dex::item_name_in(game, i)),
        abilities: range(dex::max_ability(game), &dex::ability_name),
        locations,
        balls: (1..=dex::max_ball(game)).filter_map(|b| dex::ball_name(b).map(|n| Named { value: b as u16, label: n.to_string() })).collect(),
        types: dex::type_names().iter().map(|s| s.to_string()).collect(),
    })
}

/// Attaques que l'espèce connaît à ce niveau (4 dernières apprises par niveau).
#[tauri::command]
pub fn save_suggest_moves(species: u16, form: u8, level: u8, state: State<'_, OpenSave>) -> Result<SuggestedMoves, String> {
    let game = state.with(|s| Ok(s.game()))?;
    let moves = session::suggested_moves(game, species, form, level);
    Ok(SuggestedMoves { pp: moves.map(|m| dex::move_info_in(game, m).map_or(0, |i| i.pp)), moves })
}

#[derive(Serialize)]
pub struct SuggestedMoves {
    moves: [u16; 4],
    pp: [u8; 4],
}

/// Attaques apprises par niveau et par reproduction, pour aider à choisir.
#[tauri::command]
pub fn save_learnset(species: u16, form: u8, state: State<'_, OpenSave>) -> Result<Learnset, String> {
    let game = state.with(|s| Ok(s.game()))?;
    Ok(Learnset { levelup: dex::levelup(game, species, form).to_vec(), egg: dex::egg_moves(game, species, form).to_vec() })
}

#[derive(Serialize)]
pub struct Learnset {
    levelup: Vec<(u16, u8)>,
    egg: Vec<u16>,
}

#[tauri::command]
pub fn save_dex(state: State<'_, OpenSave>) -> Result<Vec<DexEntry>, String> {
    state.with(|s| s.pokedex())
}

#[tauri::command]
pub fn save_set_dex(entries: Vec<DexEntry>, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.set_dex(&entries)?;
        s.view()
    })
}

#[tauri::command]
pub fn save_dex_all(seen: bool, caught: bool, state: State<'_, OpenSave>) -> Result<SaveView, String> {
    state.with(|s| {
        s.dex_set_all(seen, caught)?;
        s.view()
    })
}

/// Aperçu d'une sauvegarde pour le gestionnaire (sans l'ouvrir dans l'éditeur).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePeek {
    path: String,
    file_name: String,
    game: &'static str,
    version: save::SaveVersion,
    generation: u8,
    trainer: save::Trainer,
    party: Vec<(u16, bool)>,
    caught: usize,
    seen: usize,
    dex_max: u16,
    stored: usize,
    /// Dernière modification du fichier (secondes depuis 1970).
    modified: Option<u64>,
}

#[tauri::command]
pub async fn peek_save(path: PathBuf) -> Result<SavePeek, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
        let s = SaveSession::open(&bytes).map_err(|e| e.to_string())?;
        let v = s.view().map_err(|e| e.to_string())?;
        let dex = s.pokedex().unwrap_or_default();
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
        Ok(SavePeek {
            file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            path: path.display().to_string(),
            game: v.game,
            version: v.version,
            generation: v.generation,
            party: v.party.iter().map(|p| (p.summary.species, p.summary.shiny)).collect(),
            caught: dex.iter().filter(|e| e.caught).count(),
            seen: dex.iter().filter(|e| e.seen).count(),
            dex_max: s.save.dex_max_species(),
            stored: v.box_fill.iter().sum(),
            trainer: v.trainer,
            modified,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
