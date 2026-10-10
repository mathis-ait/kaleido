//! Living Dex : les Pokémon réellement possédés, lus dans les sauvegardes et la banque.
//!
//! Chaque Pokémon devient un [`Specimen`] : espèce, forme, sexe, chromatique, Ball, dresseur
//! d'origine… et une clé stable qui reconnaît le même Pokémon d'une sauvegarde à l'autre (une
//! copie de sauvegarde, ou un Pokémon passé par la banque, ne compte qu'une fois). La répartition
//! dans les cases de la Living Dex (règles des formes) est faite par l'interface, avec les données
//! de `app/public/livedex/`.

use serde::Serialize;

use crate::legality::{self, Verdict};
use crate::save::session::{SaveSession, Slot, SlotView};
use crate::save::{Gender, PkmDate, SaveError};

/// Un Pokémon possédé.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Specimen {
    /// Identité du Pokémon, la même dans toutes les sauvegardes où il se trouve.
    pub key: String,
    pub species: u16,
    pub form: u8,
    pub gender: Gender,
    pub shiny: bool,
    /// Surnom, s'il en a un.
    pub nickname: Option<String>,
    pub level: u8,
    pub ball: u8,
    pub ot_name: String,
    pub tid: u16,
    pub sid: u16,
    /// Jeu d'origine (`GameVersion` de PKHeX), 0 si la sauvegarde ne le précise pas (Gen 1 et 2).
    pub version: u8,
    pub met_location: Option<String>,
    pub met_level: u8,
    pub met_date: Option<PkmDate>,
    /// Où il est rangé : « Équipe », « Boîte 3 »…
    pub place: String,
    pub slot: Option<Slot>,
    /// Verdict du moteur de légalité (absent pour la banque).
    pub legality: Option<Verdict>,
}

/// Clé d'identité : à partir de la Gen 3, constante de chiffrement (ou PID), PID, ID et nom du
/// dresseur ; les Gen 1 et 2 n'ont pas de PID, on y prend les DV. L'espèce en fait aussi partie :
/// les Living Dex générées (PKHeX) donnent la même constante à toute une famille d'évolution.
pub fn specimen_key(v: &SlotView, generation: u8) -> String {
    let s = &v.summary;
    if generation <= 2 {
        let dv: String = s.ivs.iter().map(|d| format!("{d:x}")).collect();
        format!("g12-{}-{dv}-{:05}-{}", s.species, s.tid, s.ot_name)
    } else {
        let ec = if v.details.encryption_constant != 0 { v.details.encryption_constant } else { s.pid };
        format!("{}-{}-{ec:08x}-{:08x}-{:05}-{:05}-{}", s.species, s.form, s.pid, s.tid, s.sid, s.ot_name)
    }
}

/// Convertit la fiche d'un Pokémon ; `None` pour un Œuf ou un emplacement vide.
pub fn specimen(v: &SlotView, generation: u8, place: String, legality: Option<Verdict>) -> Option<Specimen> {
    let s = &v.summary;
    if s.species == 0 || s.is_egg {
        return None;
    }
    Some(Specimen {
        key: specimen_key(v, generation),
        species: s.species,
        form: s.form,
        gender: s.gender,
        shiny: s.shiny,
        nickname: s.is_nicknamed.then(|| s.nickname.clone()),
        level: s.level,
        ball: s.ball,
        ot_name: s.ot_name.clone(),
        tid: s.tid,
        sid: s.sid,
        version: v.details.version,
        met_location: v.met_location_name.clone().filter(|n| !n.is_empty()),
        met_level: v.details.met_level,
        met_date: v.details.met_date,
        place,
        slot: Some(v.slot),
        legality,
    })
}

/// Tous les Pokémon d'une sauvegarde (équipe puis boîtes), Œufs exclus.
pub fn from_session(session: &SaveSession) -> Result<Vec<Specimen>, SaveError> {
    let generation = session.save.format().generation();
    let game = session.game();
    let box_names = session.view()?.box_names;
    let mut out = Vec::new();
    for v in session.all()? {
        let place = match v.slot {
            Slot::Party { .. } => "Équipe".to_string(),
            Slot::Box { r#box, .. } => {
                box_names.get(r#box).filter(|n| !n.trim().is_empty()).cloned().unwrap_or_else(|| format!("Boîte {}", r#box + 1))
            }
        };
        let verdict = session.get(v.slot)?.map(|pk| legality::analyze(&pk, game).verdict);
        out.extend(specimen(&v, generation, place, verdict));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(name: &str) -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/").to_string() + name).unwrap()
    }

    #[test]
    fn reads_every_pokemon_of_a_real_save() {
        let session = SaveSession::open(&read("sm_project_802.main")).unwrap();
        let list = from_session(&session).unwrap();
        assert!(!list.is_empty());
        assert!(list.iter().all(|s| s.species > 0 && s.species <= 809));
        // Deux Pokémon différents ne se confondent pas (la sauvegarde contient un seul doublon :
        // deux copies exactes d'un même Pokémon, comptées une fois).
        let keys: std::collections::HashSet<_> = list.iter().map(|s| &s.key).collect();
        assert_eq!(keys.len(), list.len() - 1);
        assert!(list.iter().all(|s| s.legality.is_some()));
    }
}
