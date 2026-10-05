//! Commandes de la page « Cadeaux mystère » : base de PKHeX, fichiers de cartes, réception.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use kaleido_core::dex;
use kaleido_core::gifts::{self, AddOutcome, Gift, GiftDetails, GiftQuery, GiftSummary};
use kaleido_core::save::session::{SaveView, Slot};
use kaleido_core::save::{SaveError, SaveVersion};
use serde::Serialize;
use tauri::State;

use crate::saves::OpenSave;

/// Les cartes ouvertes depuis un fichier ont un identifiant à partir de cette valeur.
const IMPORTED_BASE: usize = 1_000_000;

/// Cartes ouvertes depuis des fichiers pendant la session.
#[derive(Default)]
pub struct GiftFiles(Mutex<Vec<Gift>>);

fn save_version(open: &OpenSave) -> Option<SaveVersion> {
    open.with(|s| Ok(s.save.version())).ok()
}

/// Exécute `f` sur la carte `id` (base ou fichier ouvert).
fn with_gift<T>(id: usize, files: &GiftFiles, f: impl FnOnce(&Gift) -> Result<T, String>) -> Result<T, String> {
    if id >= IMPORTED_BASE {
        let list = files.0.lock().map_err(|e| e.to_string())?;
        let g = list.get(id - IMPORTED_BASE).ok_or("cadeau introuvable")?;
        f(g)
    } else {
        f(gifts::database().get(id).ok_or("cadeau introuvable")?)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftPage {
    /// Nombre total de cartes retenues par les filtres.
    total: usize,
    offset: usize,
    items: Vec<GiftSummary>,
}

/// Recherche paginée : `only_this_game` limite aux cartes que le jeu de la sauvegarde
/// ouverte peut recevoir. Les cartes ouvertes depuis un fichier viennent en premier.
#[tauri::command]
pub fn gifts_search(
    mut query: GiftQuery,
    only_this_game: bool,
    offset: usize,
    limit: usize,
    open: State<'_, OpenSave>,
    files: State<'_, GiftFiles>,
) -> Result<GiftPage, String> {
    if only_this_game {
        if let Some(v) = save_version(&open) {
            query.versions = gifts::versions_of(v).to_vec();
        }
    }
    let imported = files.0.lock().map_err(|e| e.to_string())?;
    let db = gifts::database();
    let mut refs: Vec<&Gift> = imported.iter().collect();
    refs.extend(db.iter());
    let ids: Vec<usize> = (0..imported.len()).map(|i| IMPORTED_BASE + i).chain(0..db.len()).collect();
    let hits = gifts::search(&refs, &query);
    let items = hits.iter().skip(offset).take(limit.clamp(1, 500)).map(|&i| refs[i].summary(ids[i])).collect();
    Ok(GiftPage { total: hits.len(), offset, items })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftSpecies {
    value: u16,
    label: &'static str,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftOverview {
    total: usize,
    /// Nombre de cartes par génération (4 à 7).
    per_generation: BTreeMap<u8, usize>,
    /// Espèces présentes dans la base (pour le filtre « Pokémon »).
    species: Vec<GiftSpecies>,
}

#[tauri::command]
pub fn gifts_overview() -> GiftOverview {
    let db = gifts::database();
    let mut per_generation = BTreeMap::new();
    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    for g in db {
        *per_generation.entry(g.generation()).or_default() += 1;
        if g.species() != 0 {
            *counts.entry(g.species()).or_default() += 1;
        }
    }
    let mut species: Vec<GiftSpecies> =
        counts.into_iter().map(|(value, count)| GiftSpecies { value, label: dex::species_name(value).unwrap_or("?"), count }).collect();
    species.sort_by(|a, b| a.label.cmp(b.label));
    GiftOverview { total: db.len(), per_generation, species }
}

#[tauri::command]
pub fn gifts_details(id: usize, open: State<'_, OpenSave>, files: State<'_, GiftFiles>) -> Result<GiftDetails, String> {
    let version = save_version(&open);
    with_gift(id, &files, |g| Ok(g.details(id, version)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiftAdded {
    #[serde(flatten)]
    outcome: AddOutcome,
    view: SaveView,
}

/// Ajoute le cadeau à la sauvegarde ouverte (une étape d'historique) : Pokémon dans `slot`
/// s'il est libre, sinon dans le premier emplacement libre des boîtes ; objets dans le sac.
#[tauri::command]
pub fn gifts_add(id: usize, slot: Option<Slot>, open: State<'_, OpenSave>, files: State<'_, GiftFiles>) -> Result<GiftAdded, String> {
    let gift = with_gift(id, &files, |g| Ok(g.clone()))?;
    open.with(|s| {
        let outcome = gifts::receive(s, &gift, slot).map_err(|e| SaveError::Invalid(e.to_string()))?;
        Ok(GiftAdded { outcome, view: s.view()? })
    })
}

/// Enregistre la carte telle quelle (extension selon son format : .pcd, .pgf, .wc6full…).
#[tauri::command]
pub fn gifts_export(id: usize, output: PathBuf, files: State<'_, GiftFiles>) -> Result<(), String> {
    let bytes = with_gift(id, &files, |g| Ok(g.file().to_vec()))?;
    std::fs::write(&output, bytes).map_err(|e| format!("écriture impossible : {e}"))
}

/// Ouvre un fichier de carte et l'ajoute à la liste (en tête des résultats).
#[tauri::command]
pub fn gifts_import(file: PathBuf, open: State<'_, OpenSave>, files: State<'_, GiftFiles>) -> Result<GiftDetails, String> {
    let bytes = std::fs::read(&file).map_err(|e| format!("lecture impossible : {e}"))?;
    let ext = file.extension().and_then(|e| e.to_str()).map(str::to_string);
    let gift = Gift::from_file(&bytes, ext.as_deref()).map_err(|e| e.to_string())?;
    let version = save_version(&open);
    let mut list = files.0.lock().map_err(|e| e.to_string())?;
    // Même fichier déjà ouvert : on le réutilise.
    let index = match list.iter().position(|g| g.file() == gift.file()) {
        Some(i) => i,
        None => {
            list.push(gift);
            list.len() - 1
        }
    };
    let id = IMPORTED_BASE + index;
    Ok(list[index].details(id, version))
}
