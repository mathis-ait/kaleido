//! Journal de partie : ce que le compagnon a vu se passer d'une sauvegarde à l'autre
//! (captures, montées de niveau, évolutions, badges, K.O., morts), avec l'heure, le temps
//! de jeu et le lieu. Gardé dans `<config>/runs/<empreinte du chemin>.json`, même si la
//! sauvegarde est supprimée, pour pouvoir revoir une partie terminée.

use std::fs;
use std::path::{Path, PathBuf};

use kaleido_core::save::diff::{Brief, GameEvent};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Nombre d'entrées gardées (les plus anciennes partent au-delà).
const MAX_ENTRIES: usize = 3000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    /// Moment où le compagnon l'a vu (ms depuis 1970).
    pub at: u64,
    /// Temps de jeu affiché par le jeu à cette sauvegarde (secondes).
    pub play_seconds: u32,
    /// Lieu de la sauvegarde, si la ROM est connue.
    pub place: Option<String>,
    /// Type d'évènement : `caught`, `levelUp`, `died`…
    pub kind: String,
    pub text: String,
    /// Clé Nuzlocke du Pokémon concerné.
    pub key: Option<String>,
    pub species: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RunLog {
    pub save_path: String,
    /// État de la dernière sauvegarde lue, pour la comparaison suivante.
    pub last: Option<Brief>,
    pub journal: Vec<JournalEntry>,
}

fn file(app: &AppHandle, save: &Path) -> Result<PathBuf, String> {
    let key = save.display().to_string().replace('\\', "/").to_lowercase();
    let hash: String = Md5::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("runs").join(format!("{hash}.json")))
}

pub fn load(app: &AppHandle, save: &Path) -> RunLog {
    let mut log: RunLog = file(app, save).ok().and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    log.save_path = save.display().to_string();
    log
}

pub fn store(app: &AppHandle, save: &Path, log: &RunLog) -> Result<(), String> {
    let path = file(app, save)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec(log).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Nom court du type d'évènement (le même que la balise `kind` de la sérialisation).
pub fn kind_of(e: &GameEvent) -> &'static str {
    match e {
        GameEvent::Caught { .. } => "caught",
        GameEvent::Hatched { .. } => "hatched",
        GameEvent::Evolved { .. } => "evolved",
        GameEvent::LevelUp { .. } => "levelUp",
        GameEvent::Fainted { .. } => "fainted",
        GameEvent::Revived { .. } => "revived",
        GameEvent::Died { .. } => "died",
        GameEvent::Gone { .. } => "gone",
        GameEvent::Badge { .. } => "badge",
    }
}

impl RunLog {
    /// Entrée décidée par le joueur (issue d'une rencontre vue en mémoire).
    pub fn note(&mut self, kind: &str, text: String, at: u64, play_seconds: u32, place: Option<&str>, species: Option<u16>) {
        self.journal.push(JournalEntry { at, play_seconds, place: place.map(str::to_string), kind: kind.to_string(), text, key: None, species });
        if self.journal.len() > MAX_ENTRIES {
            let extra = self.journal.len() - MAX_ENTRIES;
            self.journal.drain(..extra);
        }
    }

    /// Ajoute les évènements d'une sauvegarde au journal.
    pub fn record(&mut self, events: &[GameEvent], at: u64, play_seconds: u32, place: Option<&str>) {
        for e in events {
            self.journal.push(JournalEntry {
                at,
                play_seconds,
                place: place.map(str::to_string),
                kind: kind_of(e).to_string(),
                text: e.text(),
                key: e.mon().map(|m| m.key.clone()),
                species: e.mon().map(|m| m.species),
            });
        }
        if self.journal.len() > MAX_ENTRIES {
            let extra = self.journal.len() - MAX_ENTRIES;
            self.journal.drain(..extra);
        }
    }
}
