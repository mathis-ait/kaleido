//! Tests de la session d'édition, sur une sauvegarde Platine synthétique.

use super::session::{PokemonPatch, SaveSession, Slot};
use super::*;

fn pokemon(species: u16, exp: u32) -> Pokemon {
    let mut p = Pokemon::blank(PkmFormat::Gen4);
    p.set_species(species);
    p.set_exp(exp);
    p.set_pid(0x1234_5678);
    p.refresh_checksum();
    p
}

#[test]
fn move_patch_and_reload() {
    let mut s = SaveSession::open(&gen4::blank(SaveVersion::Platinum, 0, 0)).unwrap();
    // Une sauvegarde vierge n'a pas d'équipe : on y place d'abord un Pokémon.
    s.save.set_box_slot(1, 5, Some(pokemon(1, 100))).unwrap();
    s.move_pokemon(Slot::Box { r#box: 1, index: 5 }, Slot::Party { index: 0 }).unwrap();
    // Carchacrok a une courbe « lente » : 1 250 000 points = niveau 100.
    s.save.set_box_slot(0, 0, Some(pokemon(445, 1_250_000))).unwrap();
    s.save.set_box_slot(0, 1, Some(pokemon(25, 1000))).unwrap();

    assert_eq!(s.save.party_count(), 1);
    assert!(s.view_slot(Slot::Party { index: 0 }).unwrap().stats.is_some());

    // Boîte → fin de l'équipe : le Pokémon reçoit ses statistiques d'équipe.
    s.move_pokemon(Slot::Box { r#box: 0, index: 0 }, Slot::Party { index: 1 }).unwrap();
    let garchomp = s.view_slot(Slot::Party { index: 1 }).unwrap();
    assert_eq!(garchomp.species_name, "Carchacrok");
    assert_eq!(garchomp.summary.level, 100);
    assert!(s.get(Slot::Box { r#box: 0, index: 0 }).unwrap().is_none());

    // Échange équipe ↔ boîte.
    s.move_pokemon(Slot::Box { r#box: 0, index: 1 }, Slot::Party { index: 1 }).unwrap();
    assert_eq!(s.view_slot(Slot::Party { index: 1 }).unwrap().summary.species, 25);
    assert_eq!(s.view_slot(Slot::Box { r#box: 0, index: 1 }).unwrap().summary.species, 445);

    // Équipe → boîte vide, puis le dernier Pokémon de l'équipe ne peut plus partir.
    s.move_pokemon(Slot::Party { index: 1 }, Slot::Box { r#box: 2, index: 0 }).unwrap();
    assert_eq!(s.save.party_count(), 1);
    assert!(s.move_pokemon(Slot::Party { index: 0 }, Slot::Box { r#box: 2, index: 1 }).is_err());

    // Modification : niveau 50, objet, EV plafonnés à 510.
    let patch = PokemonPatch { level: Some(50), held_item: Some(4), evs: Some([252, 252, 252, 0, 0, 0]), ..Default::default() };
    let v = s.patch(Slot::Party { index: 0 }, &patch).unwrap();
    assert_eq!(v.summary.level, 50);
    assert_eq!(v.item_name.as_deref(), Some("Poké Ball"));
    assert_eq!(v.summary.evs.iter().map(|&e| e as u32).sum::<u32>(), 510);

    let reloaded = SaveSession::open(&s.to_bytes()).unwrap();
    assert_eq!(reloaded.view_slot(Slot::Party { index: 0 }).unwrap().summary.level, 50);
    assert_eq!(reloaded.view_slot(Slot::Box { r#box: 2, index: 0 }).unwrap().species_name, "Pikachu");
}
