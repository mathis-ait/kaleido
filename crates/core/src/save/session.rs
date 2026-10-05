//! Session d'édition d'une sauvegarde pour l'interface : emplacements, noms
//! français, déplacements entre boîtes et équipe, modifications d'un Pokémon.

use serde::{Deserialize, Serialize};

use super::{calc_stats, exp_for_level, GrowthRate, Pokemon, PokemonSummary, SaveError, SaveFile, Trainer, BOX_SLOTS, PARTY_SLOTS};
use crate::names;

/// Emplacement d'un Pokémon dans la sauvegarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Slot {
    Party { index: usize },
    Box { r#box: usize, index: usize },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub slot: Slot,
    #[serde(flatten)]
    pub summary: PokemonSummary,
    pub species_name: String,
    pub ability_name: String,
    pub item_name: Option<String>,
    pub move_names: Vec<String>,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit (calculées si le Pokémon est en boîte).
    pub stats: Option<[u16; 6]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveView {
    pub game: &'static str,
    pub generation: u8,
    pub trainer: Trainer,
    pub box_names: Vec<String>,
    pub party: Vec<SlotView>,
    pub warnings: Vec<String>,
    pub needs_resign: bool,
    pub checksums_valid: bool,
}

/// Modifications demandées par l'interface (champs absents = inchangés).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PokemonPatch {
    pub nickname: Option<String>,
    pub level: Option<u8>,
    pub held_item: Option<u16>,
    pub moves: Option<[u16; 4]>,
    pub ivs: Option<[u8; 6]>,
    pub evs: Option<[u8; 6]>,
    pub friendship: Option<u8>,
}

pub struct SaveSession {
    pub save: SaveFile,
}

fn growth(species: u16) -> Option<GrowthRate> {
    names::growth_rate(species)
}

pub fn view_of(slot: Slot, p: &Pokemon) -> SlotView {
    let summary = p.summary(growth(p.species()));
    let name_or_id = |n: Option<&str>, id: u16| match (n, id) {
        (Some(n), _) => n.to_string(),
        (None, 0) => "—".to_string(),
        (None, id) => format!("n°{id}"),
    };
    let stats = p.party_stats().or_else(|| {
        names::base_stats(p.species()).map(|b| calc_stats(&b, summary.level, summary.ivs, summary.evs, summary.nature))
    });
    SlotView {
        slot,
        species_name: name_or_id(names::species(summary.species), summary.species),
        ability_name: name_or_id(names::ability(summary.ability), summary.ability),
        item_name: names::item(summary.held_item).map(str::to_string),
        move_names: summary.moves.iter().filter(|&&m| m != 0).map(|&m| name_or_id(names::move_name(m), m)).collect(),
        stats,
        summary,
    }
}

impl SaveSession {
    pub fn open(bytes: &[u8]) -> Result<Self, SaveError> {
        Ok(Self { save: SaveFile::from_bytes(bytes)? })
    }

    pub fn view(&self) -> Result<SaveView, SaveError> {
        let s = &self.save;
        let party = s.party()?.iter().enumerate().map(|(i, p)| view_of(Slot::Party { index: i }, p)).collect();
        Ok(SaveView {
            game: s.version().label(),
            generation: s.generation(),
            trainer: s.trainer(),
            box_names: (0..s.box_count())
                .map(|i| s.box_name(i).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Boîte {}", i + 1)))
                .collect(),
            party,
            warnings: s.warnings().to_vec(),
            needs_resign: s.needs_resign(),
            checksums_valid: s.checksums_valid(),
        })
    }

    pub fn box_view(&self, b: usize) -> Result<Vec<Option<SlotView>>, SaveError> {
        (0..BOX_SLOTS)
            .map(|i| {
                let slot = Slot::Box { r#box: b, index: i };
                Ok(self.save.box_slot(b, i)?.filter(|p| !p.is_empty()).map(|p| view_of(slot, &p)))
            })
            .collect()
    }

    pub fn get(&self, slot: Slot) -> Result<Option<Pokemon>, SaveError> {
        let p = match slot {
            Slot::Party { index } => self.save.party_slot(index)?,
            Slot::Box { r#box, index } => self.save.box_slot(r#box, index)?,
        };
        Ok(p.filter(|p| !p.is_empty()))
    }

    fn set(&mut self, slot: Slot, p: Option<Pokemon>) -> Result<(), SaveError> {
        match slot {
            Slot::Party { index } => self.save.set_party_slot(index, p.map(prepare_for_party)),
            Slot::Box { r#box, index } => self.save.set_box_slot(r#box, index, p),
        }
    }

    /// Échange deux emplacements (ou déplace vers un emplacement vide).
    pub fn move_pokemon(&mut self, from: Slot, to: Slot) -> Result<(), SaveError> {
        if from == to {
            return Ok(());
        }
        let a = self.get(from)?.ok_or_else(|| SaveError::Invalid("emplacement de départ vide".into()))?;
        let b = self.get(to)?;
        match (to, b) {
            // Ajout à la fin de l'équipe.
            (Slot::Party { index }, None) => {
                let count = self.save.party_count();
                if index >= PARTY_SLOTS || count >= PARTY_SLOTS {
                    return Err(SaveError::Invalid("l'équipe est complète".into()));
                }
                if matches!(from, Slot::Party { .. }) {
                    return Ok(()); // déjà dans l'équipe : l'ordre ne change pas
                }
                self.save.set_party_slot(count, Some(prepare_for_party(a)))?;
                self.set(from, None)
            }
            (_, Some(b)) => {
                self.set(from, Some(b))?;
                self.set(to, Some(a))
            }
            (_, None) => {
                if let Slot::Party { .. } = from {
                    if self.save.party_count() <= 1 {
                        return Err(SaveError::Invalid("l'équipe doit garder au moins un Pokémon".into()));
                    }
                }
                self.set(to, Some(a))?;
                self.set(from, None)
            }
        }
    }

    pub fn delete(&mut self, slot: Slot) -> Result<(), SaveError> {
        self.set(slot, None)
    }

    /// Place un Pokémon importé (fichier .pk*) dans un emplacement.
    pub fn import(&mut self, slot: Slot, bytes: &[u8]) -> Result<SlotView, SaveError> {
        let p = Pokemon::from_bytes(self.save.format(), bytes).map_err(|e| SaveError::Invalid(e.to_string()))?;
        if p.is_empty() {
            return Err(SaveError::Invalid("fichier Pokémon vide".into()));
        }
        let slot = match slot {
            Slot::Party { index } if index >= self.save.party_count() => Slot::Party { index: self.save.party_count() },
            s => s,
        };
        self.set(slot, Some(p))?;
        self.view_slot(slot)
    }

    pub fn view_slot(&self, slot: Slot) -> Result<SlotView, SaveError> {
        let p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        Ok(view_of(slot, &p))
    }

    pub fn patch(&mut self, slot: Slot, patch: &PokemonPatch) -> Result<SlotView, SaveError> {
        let mut p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        let invalid = |e: super::PkmError| SaveError::Invalid(e.to_string());
        if let Some(name) = &patch.nickname {
            p.set_nickname(name).map_err(invalid)?;
            p.set_is_nicknamed(names::species(p.species()) != Some(name.as_str()));
        }
        if let Some(item) = patch.held_item {
            p.set_held_item(item);
        }
        if let Some(moves) = patch.moves {
            p.set_moves(moves);
        }
        if let Some(ivs) = patch.ivs {
            p.set_ivs(ivs.map(|v| v.min(31))).map_err(invalid)?;
        }
        if let Some(evs) = patch.evs {
            let mut evs = evs.map(|v| v.min(252));
            // Total limité à 510 : on réduit les dernières valeurs si besoin.
            let mut excess = evs.iter().map(|&v| v as i32).sum::<i32>() - 510;
            for v in evs.iter_mut().rev() {
                if excess <= 0 {
                    break;
                }
                let cut = excess.min(*v as i32);
                *v -= cut as u8;
                excess -= cut;
            }
            p.set_evs(evs);
        }
        if let Some(f) = patch.friendship {
            p.set_friendship(f);
        }
        if let Some(level) = patch.level {
            let g = growth(p.species()).ok_or_else(|| SaveError::Invalid("courbe d'expérience inconnue pour cette espèce".into()))?;
            p.set_exp(exp_for_level(g, level.clamp(1, 100)));
        }
        if p.party_level().is_some() {
            p = prepare_for_party(p);
        }
        p.refresh_checksum();
        self.set(slot, Some(p))?;
        self.view_slot(slot)
    }

    /// Données déchiffrées d'un Pokémon (fichier .pk4 / .pk5 / .pk6 / .pk7).
    pub fn export(&self, slot: Slot) -> Result<(Vec<u8>, String), SaveError> {
        let p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        let ext = format!("pk{}", self.save.generation());
        Ok((p.stored_data().to_vec(), ext))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.save.to_bytes()
    }
}

/// Recalcule niveau et statistiques de la section équipe (nécessaire en équipe).
fn prepare_for_party(mut p: Pokemon) -> Pokemon {
    if let (Some(base), Some(g)) = (names::base_stats(p.species()), growth(p.species())) {
        p.update_party_stats(&base, g);
        p.refresh_checksum();
    }
    p
}
