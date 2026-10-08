//! Ce qui s'est passé entre deux sauvegardes en jeu : captures, K.O., morts, évolutions,
//! badges… Le compagnon de partie compare chaque nouvelle sauvegarde à la précédente et
//! en tire des évènements, qui alimentent le journal de partie et le suivi Nuzlocke.
//!
//! Règles (décidées pour le Nuzlocke automatique) :
//! - un Pokémon absent de la sauvegarde précédente est une **capture** (ou un œuf reçu) ;
//! - un Pokémon de l'équipe dont les PV tombent à 0 est **K.O.** : simple avertissement,
//!   un Rappel ou un Centre Pokémon peut encore le sauver ;
//! - un Pokémon K.O. qui passe de l'équipe à une boîte, ou qui disparaît (relâché), est **mort**.
//!
//! Une rencontre ratée (fuite, K.O. du sauvage) ne laisse aucune trace dans la sauvegarde :
//! elle reste à signaler à la main.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::session::LiveSnapshot;

/// Résumé d'un Pokémon, gardé d'une sauvegarde à l'autre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefMon {
    pub uid: String,
    /// Clé du suivi Nuzlocke (PID, voir `nuzlocke::mon_key`).
    pub key: String,
    pub species: u16,
    pub species_name: String,
    pub nickname: String,
    pub level: u8,
    pub is_egg: bool,
    pub in_party: bool,
    /// PV actuels (équipe seulement : les Pokémon en boîte n'en ont pas).
    pub hp: Option<u16>,
    pub met_location: u16,
}

impl BriefMon {
    /// Nom affiché : surnom, sinon espèce.
    pub fn name(&self) -> &str {
        if self.is_egg {
            "Œuf"
        } else if self.nickname.trim().is_empty() {
            &self.species_name
        } else {
            &self.nickname
        }
    }
}

/// État de la partie retenu pour la comparaison suivante.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    pub mons: Vec<BriefMon>,
    /// Nombre de badges (ou d'épreuves en Gen 7).
    pub badges: u8,
    /// Temps de jeu en secondes.
    pub play_seconds: u32,
    pub map: u16,
}

impl From<&LiveSnapshot> for Brief {
    fn from(s: &LiveSnapshot) -> Self {
        let key = |pid: u32| format!("{pid:08X}");
        let mut mons: Vec<BriefMon> = s
            .party
            .iter()
            .map(|m| BriefMon {
                uid: m.uid.clone(),
                key: key(m.pid),
                species: m.species,
                species_name: m.species_name.clone(),
                nickname: m.nickname.clone(),
                level: m.level,
                is_egg: m.is_egg,
                in_party: true,
                hp: (m.max_hp > 0).then_some(m.hp),
                met_location: m.met_location,
            })
            .collect();
        mons.extend(s.boxes.iter().flat_map(|b| b.mons.iter()).map(|m| BriefMon {
            uid: m.uid.clone(),
            key: key(m.pid),
            species: m.species,
            species_name: m.species_name.clone(),
            nickname: m.nickname.clone(),
            level: m.level,
            is_egg: m.is_egg,
            in_party: false,
            hp: None,
            met_location: m.met_location,
        }));
        let t = &s.play_time;
        Brief {
            mons,
            badges: s.badges.map_or(0, |b| b.count_ones() as u8),
            play_seconds: t.hours as u32 * 3600 + t.minutes as u32 * 60 + t.seconds as u32,
            map: s.map,
        }
    }
}

/// Comment un Pokémon est mort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeathKind {
    /// K.O. puis rangé dans une boîte.
    Deposited,
    /// K.O. puis disparu de la sauvegarde (relâché).
    Released,
}

/// Ce qui a changé entre deux sauvegardes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GameEvent {
    /// Nouveau Pokémon (capture, cadeau, échange) ou nouvel œuf.
    Caught { mon: BriefMon },
    Hatched { mon: BriefMon },
    Evolved { mon: BriefMon, from: String },
    LevelUp { mon: BriefMon, from: u8 },
    /// PV à 0 dans l'équipe (pas encore une mort).
    Fainted { mon: BriefMon },
    /// Soigné après un K.O. (Centre Pokémon, Rappel).
    Revived { mon: BriefMon },
    Died { mon: BriefMon, how: DeathKind },
    /// Pokémon disparu sans être K.O. (relâché, échangé).
    Gone { mon: BriefMon },
    Badge { count: u8 },
}

impl GameEvent {
    /// Phrase courte pour le journal et les notifications.
    pub fn text(&self) -> String {
        match self {
            GameEvent::Caught { mon } if mon.is_egg => "Nouvel œuf".into(),
            GameEvent::Caught { mon } => format!("Nouveau Pokémon : {} (N. {})", mon.name(), mon.level),
            GameEvent::Hatched { mon } => format!("{} est sorti de l'œuf", mon.species_name),
            GameEvent::Evolved { mon, from } => format!("{from} a évolué en {}", mon.species_name),
            GameEvent::LevelUp { mon, .. } => format!("{} passe au N. {}", mon.name(), mon.level),
            GameEvent::Fainted { mon } => format!("{} est K.O. : à déposer en boîte", mon.name()),
            GameEvent::Revived { mon } => format!("{} est soigné", mon.name()),
            GameEvent::Died { mon, how: DeathKind::Deposited } => format!("{} est mort (déposé K.O.)", mon.name()),
            GameEvent::Died { mon, how: DeathKind::Released } => format!("{} est mort (relâché K.O.)", mon.name()),
            GameEvent::Gone { mon } => format!("{} n'est plus dans la sauvegarde", mon.name()),
            GameEvent::Badge { count } => format!("Badge n° {count} obtenu"),
        }
    }

    /// Le Pokémon concerné, s'il y en a un.
    pub fn mon(&self) -> Option<&BriefMon> {
        match self {
            GameEvent::Caught { mon }
            | GameEvent::Hatched { mon }
            | GameEvent::Evolved { mon, .. }
            | GameEvent::LevelUp { mon, .. }
            | GameEvent::Fainted { mon }
            | GameEvent::Revived { mon }
            | GameEvent::Died { mon, .. }
            | GameEvent::Gone { mon } => Some(mon),
            GameEvent::Badge { .. } => None,
        }
    }
}

fn ko(m: &BriefMon) -> bool {
    m.in_party && !m.is_egg && m.hp == Some(0)
}

/// Évènements entre la sauvegarde précédente `old` et la nouvelle `new`.
pub fn diff(old: &Brief, new: &Brief) -> Vec<GameEvent> {
    let mut out = Vec::new();
    let before: HashMap<&str, &BriefMon> = old.mons.iter().map(|m| (m.uid.as_str(), m)).collect();
    // L'uid contient l'espèce : une évolution change l'uid, on retrouve le Pokémon par son PID.
    let before_by_key: HashMap<&str, &BriefMon> = old.mons.iter().map(|m| (m.key.as_str(), m)).collect();
    let mut seen: Vec<&str> = Vec::new();

    for m in &new.mons {
        let prev = before.get(m.uid.as_str()).or_else(|| before_by_key.get(m.key.as_str())).copied();
        let Some(prev) = prev else {
            out.push(GameEvent::Caught { mon: m.clone() });
            continue;
        };
        seen.push(prev.uid.as_str());
        if prev.is_egg && !m.is_egg {
            out.push(GameEvent::Hatched { mon: m.clone() });
            continue;
        }
        if prev.species != m.species && !m.is_egg {
            out.push(GameEvent::Evolved { mon: m.clone(), from: prev.species_name.clone() });
        }
        if m.level > prev.level && !m.is_egg {
            out.push(GameEvent::LevelUp { mon: m.clone(), from: prev.level });
        }
        if ko(prev) && !m.in_party {
            // K.O. dans l'équipe, maintenant en boîte : il est mort.
            out.push(GameEvent::Died { mon: m.clone(), how: DeathKind::Deposited });
        } else if ko(m) && !ko(prev) {
            out.push(GameEvent::Fainted { mon: m.clone() });
        } else if ko(prev) && m.in_party && m.hp.is_some_and(|hp| hp > 0) {
            out.push(GameEvent::Revived { mon: m.clone() });
        }
    }

    for m in &old.mons {
        if seen.contains(&m.uid.as_str()) {
            continue;
        }
        out.push(if ko(m) { GameEvent::Died { mon: m.clone(), how: DeathKind::Released } } else { GameEvent::Gone { mon: m.clone() } });
    }

    if new.badges > old.badges {
        for count in old.badges + 1..=new.badges {
            out.push(GameEvent::Badge { count });
        }
    }
    out
}

/// Évènements entre deux lectures de la mémoire de l'émulateur. Seule l'équipe y est relue (les
/// boîtes restent celles de la dernière sauvegarde) : un Pokémon déposé en boîte y semble
/// disparu. On ne garde donc que ce que l'équipe montre sûrement (K.O., soin, niveau, évolution,
/// éclosion, capture qui rejoint l'équipe) ; morts, départs et badges restent l'affaire de la
/// sauvegarde.
pub fn diff_memory(old: &Brief, new: &Brief) -> Vec<GameEvent> {
    diff(old, new)
        .into_iter()
        .filter(|e| match e {
            GameEvent::Caught { mon } => mon.in_party,
            GameEvent::Fainted { .. } | GameEvent::Revived { .. } | GameEvent::LevelUp { .. } | GameEvent::Evolved { .. } | GameEvent::Hatched { .. } => true,
            GameEvent::Died { .. } | GameEvent::Gone { .. } | GameEvent::Badge { .. } => false,
        })
        .collect()
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
