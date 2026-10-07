//! Instantané en lecture seule d'une sauvegarde, pour le compagnon de partie.
//!
//! Le compagnon relit le fichier à chaque sauvegarde en jeu : il lui faut l'équipe telle
//! que le jeu l'affiche (PV restants, statut, objet, attaques), le contenu des boîtes et la
//! progression, sans rien de ce qui sert à l'édition (historique, fiches complètes).

use serde::Serialize;

use super::{game_of, growth, view_of, SaveSession, Slot};
use crate::dex;
use crate::dex::Game;
use crate::save::{Gender, PlayTime, Pokemon, SaveError, SaveVersion, Trainer, BOX_SLOTS};

/// Problème de statut d'un Pokémon de l'équipe (champ `status` de la section équipe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusCondition {
    None,
    Sleep,
    Poison,
    Burn,
    Freeze,
    Paralysis,
    /// Poison grave (Toxik).
    BadPoison,
}

impl StatusCondition {
    /// Format commun aux Gen 3 à 7 : bits 0-2 = tours de sommeil restants, puis un bit par statut.
    pub fn from_bits(v: u32) -> Self {
        if v & 0x7 != 0 {
            Self::Sleep
        } else if v & 0x80 != 0 {
            Self::BadPoison
        } else if v & 0x08 != 0 {
            Self::Poison
        } else if v & 0x10 != 0 {
            Self::Burn
        } else if v & 0x20 != 0 {
            Self::Freeze
        } else if v & 0x40 != 0 {
            Self::Paralysis
        } else {
            Self::None
        }
    }
}

/// Pokémon de l'équipe, tel que le jeu l'affiche.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMon {
    pub slot: Slot,
    pub species: u16,
    pub form: u8,
    pub species_name: String,
    pub nickname: String,
    pub is_nicknamed: bool,
    pub is_egg: bool,
    pub shiny: bool,
    pub gender: Gender,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
    pub status: StatusCondition,
    pub item_name: Option<String>,
    pub ability_name: String,
    pub nature_name: &'static str,
    pub move_names: Vec<String>,
    pub met_location_name: Option<String>,
    /// Identifiant stable du Pokémon (PID + constante de chiffrement), pour suivre un même
    /// Pokémon d'une sauvegarde à l'autre.
    pub uid: String,
}

/// Pokémon d'une boîte (juste ce qu'il faut pour la grille).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBoxMon {
    pub index: usize,
    pub species: u16,
    pub form: u8,
    pub species_name: String,
    pub nickname: String,
    pub is_egg: bool,
    pub shiny: bool,
    pub level: u8,
    pub uid: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveBox {
    pub name: String,
    pub mons: Vec<LiveBoxMon>,
}

/// État de la partie au moment de la dernière sauvegarde en jeu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSnapshot {
    pub game: &'static str,
    pub version: SaveVersion,
    pub generation: u8,
    pub trainer: Trainer,
    pub play_time: PlayTime,
    /// Badges obtenus (Gen 4-6) ou épreuves des îles terminées (Gen 7).
    pub badges: Option<u8>,
    pub party: Vec<LiveMon>,
    pub boxes: Vec<LiveBox>,
}

/// Identifiant stable d'un Pokémon : le PID seul ne suffit pas (deux Pokémon générés de la
/// même façon peuvent le partager), la constante de chiffrement le complète dès la Gen 6.
fn uid(p: &Pokemon) -> String {
    let s = p.summary(None);
    format!("{:08x}-{:08x}-{}", s.pid, p.encryption_constant(), s.species)
}

fn live_mon(game: Game, slot: Slot, p: &Pokemon) -> LiveMon {
    let v = view_of(game, slot, p);
    let max_hp = p.party_stats().map(|s| s[0]).or(v.stats.map(|s| s[0])).unwrap_or(0);
    LiveMon {
        slot,
        species: v.summary.species,
        form: v.summary.form,
        nickname: v.summary.nickname.clone(),
        is_nicknamed: v.summary.is_nicknamed,
        is_egg: v.summary.is_egg,
        shiny: v.summary.shiny,
        gender: v.summary.gender,
        level: v.summary.level,
        hp: p.current_hp().min(max_hp.max(p.current_hp())),
        max_hp,
        status: StatusCondition::from_bits(p.status_condition()),
        item_name: v.item_name,
        ability_name: v.ability_name,
        nature_name: v.summary.nature_name,
        move_names: v.move_names,
        met_location_name: v.met_location_name,
        uid: uid(p),
        species_name: v.species_name,
    }
}

impl SaveSession {
    /// Instantané pour le compagnon. Échoue si les sommes de contrôle ne sont pas bonnes :
    /// le fichier est sans doute en cours d'écriture par l'émulateur, il faut relire plus tard.
    pub fn live(&self) -> Result<LiveSnapshot, SaveError> {
        let s = &self.save;
        if !s.checksums_valid() {
            return Err(SaveError::Invalid("sommes de contrôle incorrectes (sauvegarde en cours d'écriture ?)".into()));
        }
        let game = game_of(s.version());
        let party = s.party()?.iter().enumerate().map(|(i, p)| live_mon(game, Slot::Party { index: i }, p)).collect();
        let mut boxes = Vec::with_capacity(s.box_count());
        for b in 0..s.box_count() {
            let mut mons = Vec::new();
            for i in 0..BOX_SLOTS {
                if let Some(p) = s.box_slot(b, i)?.filter(|p| !p.is_empty()) {
                    let sum = p.summary(growth(game, p.species(), p.form()));
                    mons.push(LiveBoxMon {
                        index: i,
                        species: sum.species,
                        form: sum.form,
                        species_name: dex::species_name(sum.species).map(str::to_string).unwrap_or_else(|| format!("n°{}", sum.species)),
                        nickname: sum.nickname.clone(),
                        is_egg: sum.is_egg,
                        shiny: sum.shiny,
                        level: sum.level,
                        uid: uid(&p),
                    });
                }
            }
            let name = s.box_name(b).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Boîte {}", b + 1));
            boxes.push(LiveBox { name, mons });
        }
        let trainer = s.trainer();
        Ok(LiveSnapshot {
            game: s.version().label(),
            version: s.version(),
            generation: s.generation(),
            play_time: trainer.play_time,
            trainer,
            badges: s.badges(),
            party,
            boxes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuts() {
        assert_eq!(StatusCondition::from_bits(0), StatusCondition::None);
        assert_eq!(StatusCondition::from_bits(3), StatusCondition::Sleep);
        assert_eq!(StatusCondition::from_bits(0x08), StatusCondition::Poison);
        assert_eq!(StatusCondition::from_bits(0x88), StatusCondition::BadPoison);
        assert_eq!(StatusCondition::from_bits(0x10), StatusCondition::Burn);
        assert_eq!(StatusCondition::from_bits(0x20), StatusCondition::Freeze);
        assert_eq!(StatusCondition::from_bits(0x40), StatusCondition::Paralysis);
    }

    #[test]
    fn instantane_de_la_sauvegarde_de_demo() {
        let bytes = crate::save::demo_save().unwrap();
        let live = SaveSession::open(&bytes).unwrap().live().unwrap();
        assert_eq!(live.generation, 4);
        assert_eq!(live.party.len(), 3);
        let first = &live.party[0];
        assert_eq!(first.species, 445);
        assert_eq!(first.level, 62);
        assert!(first.max_hp > 0 && first.hp == first.max_hp, "PV {} / {}", first.hp, first.max_hp);
        assert_eq!(first.status, StatusCondition::None);
        assert_eq!(first.move_names.len(), 4);
        assert_eq!(live.boxes[0].mons.len(), 9);
        // Identifiants stables et distincts.
        let mut uids: Vec<_> = live.party.iter().map(|m| m.uid.clone()).chain(live.boxes[0].mons.iter().map(|m| m.uid.clone())).collect();
        let n = uids.len();
        uids.sort();
        uids.dedup();
        assert_eq!(uids.len(), n);
    }

    /// Émulateur interrompu en pleine écriture : quelle que soit la zone à moitié écrite, on obtient
    /// soit une erreur (relire plus tard), soit l'état complet d'une des deux copies, jamais un mélange.
    #[test]
    fn ecriture_partielle_jamais_corrompue() {
        let bytes = crate::save::demo_save().unwrap();
        let reference = SaveSession::open(&bytes).unwrap().live().unwrap();
        let same = |l: &LiveSnapshot| {
            l.party.iter().map(|m| (&m.uid, m.level, m.hp)).eq(reference.party.iter().map(|m| (&m.uid, m.level, m.hp)))
                && l.boxes.iter().map(|b| b.mons.len()).eq(reference.boxes.iter().map(|b| b.mons.len()))
        };
        let step = bytes.len() / 64;
        for start in (0..bytes.len()).step_by(step) {
            let mut broken = bytes.clone();
            let end = (start + step).min(broken.len());
            broken[start..end].fill(0xFF);
            if let Ok(live) = SaveSession::open(&broken).and_then(|s| s.live()) {
                // Une copie vide (sauvegarde de démo sans seconde copie) peut aussi être choisie : elle
                // ne doit alors rien contenir plutôt qu'un état mélangé.
                let empty = live.party.is_empty() && live.boxes.iter().all(|b| b.mons.is_empty());
                assert!(same(&live) || empty, "zone {start:#x}..{end:#x} : instantané incohérent");
            }
        }
    }
}
