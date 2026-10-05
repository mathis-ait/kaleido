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

#[test]
fn undo_redo_clone_and_overwrite() {
    let mut s = SaveSession::open(&gen4::blank(SaveVersion::Platinum, 0, 0)).unwrap();
    s.save.set_box_slot(0, 0, Some(pokemon(445, 1_250_000))).unwrap();
    s.save.set_box_slot(0, 1, Some(pokemon(25, 1000))).unwrap();
    let a = Slot::Box { r#box: 0, index: 0 };
    let b = Slot::Box { r#box: 0, index: 1 };
    let c = Slot::Box { r#box: 0, index: 2 };

    s.clone_pokemon(a, c).unwrap();
    assert_eq!(s.get(c).unwrap().unwrap().species(), 445);
    assert_eq!(s.get(a).unwrap().unwrap().species(), 445);
    assert!(s.view().unwrap().can_undo);
    assert!(s.undo());
    assert!(s.get(c).unwrap().is_none());
    assert!(s.redo());
    assert_eq!(s.get(c).unwrap().unwrap().species(), 445);

    // Écraser : Pikachu remplace le clone, son emplacement d'origine est vidé.
    s.overwrite_pokemon(b, c).unwrap();
    assert_eq!(s.get(c).unwrap().unwrap().species(), 25);
    assert!(s.get(b).unwrap().is_none());

    // Une erreur ne laisse aucune trace dans l'historique.
    s.undo();
    s.undo();
    assert!(!s.view().unwrap().can_undo);
    assert!(s.patch(a, &PokemonPatch { nickname: Some("Beaucoup trop long".into()), ..Default::default() }).is_err());
    assert!(!s.view().unwrap().can_undo);
}

#[test]
fn shiny_nature_and_ability_keep_other_traits() {
    for version in [SaveVersion::Platinum, SaveVersion::BlackWhite] {
        let bytes = if version == SaveVersion::Platinum { gen4::blank(version, 0, 0) } else { gen5::blank(version) };
        let mut s = SaveSession::open(&bytes).unwrap();
        let slot = Slot::Box { r#box: 0, index: 0 };
        let mut p = Pokemon::blank(s.save.format());
        p.set_species(445);
        p.set_exp(1_250_000);
        p.set_pid(0x1234_5678);
        p.set_tid(12345);
        p.set_sid(54321);
        if version != SaveVersion::Platinum {
            p.set_nature(7).unwrap();
        }
        p.refresh_checksum();
        s.save.set_box_slot(0, 0, Some(p)).unwrap();
        let before = s.view_slot(slot).unwrap();
        let low = before.summary.pid & 0xFF;

        for mode in [ShinyMode::Star, ShinyMode::Square, ShinyMode::None] {
            let v = s.patch(slot, &PokemonPatch { shiny: Some(mode), ..Default::default() }).unwrap();
            assert_eq!(v.summary.shiny, mode != ShinyMode::None, "{version:?} {mode:?}");
            assert_eq!(v.summary.nature, before.summary.nature);
            assert_eq!(v.details.ability_number, before.details.ability_number);
            assert_eq!(v.summary.pid & 0xFF, low, "sexe conservé");
        }

        let v = s.patch(slot, &PokemonPatch { shiny: Some(ShinyMode::KeepPid), ..Default::default() }).unwrap();
        assert!(v.summary.shiny);
        assert_ne!(v.summary.sid, 54321);

        let v = s.patch(slot, &PokemonPatch { nature: Some(13), ability_number: Some(2), ..Default::default() }).unwrap();
        assert_eq!(v.summary.nature, 13);
        assert_eq!(v.details.ability_number, 2);
        assert!(v.summary.shiny, "le chromatique survit au changement de nature");
    }
}

#[test]
fn trainer_box_names_and_create() {
    let mut s = SaveSession::open(&gen4::blank(SaveVersion::Platinum, 0, 0)).unwrap();
    let patch =
        crate::save::edit::TrainerPatch { name: Some("Aurore".into()), tid: Some(4242), money: Some(123_456), hours: Some(12), ..Default::default() };
    s.set_trainer(&patch).unwrap();
    s.set_box_name(3, "Légendes").unwrap();
    let v = s.view().unwrap();
    assert_eq!(v.trainer.name, "Aurore");
    assert_eq!(v.trainer.tid, 4242);
    assert_eq!(v.trainer.money, 123_456);
    assert_eq!(v.trainer.play_time.hours, 12);
    assert_eq!(v.box_names[3], "Légendes");

    let created = s.create(Slot::Box { r#box: 1, index: 4 }, 25, 30).unwrap();
    assert_eq!(created.species_name, "Pikachu");
    assert_eq!(created.summary.level, 30);
    assert_eq!(created.summary.ot_name, "Aurore");
    assert_eq!(created.summary.tid, 4242);
    assert!(created.details.met_date.is_some());

    let reloaded = SaveSession::open(&s.to_bytes()).unwrap();
    assert!(reloaded.save.checksums_valid());
    assert_eq!(reloaded.view().unwrap().trainer.name, "Aurore");
    assert_eq!(reloaded.view_slot(Slot::Box { r#box: 1, index: 4 }).unwrap().summary.species, 25);
}
