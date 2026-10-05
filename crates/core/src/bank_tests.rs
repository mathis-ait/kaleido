//! Tests de la banque : index sur disque, rangement, recherche, et échanges avec la
//! sauvegarde Soleil/Lune réelle des tests de PKHeX et une Platine synthétique.

use super::*;
use crate::save::{SaveFile, SaveVersion};

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/", $name))
    };
}

/// Dossier temporaire effacé à la fin du test.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("kaleido-bank-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pk4() -> Pokemon {
    Pokemon::from_bytes(PkmFormat::Gen4, fixture!("pk4_407_roserade_party.pk4")).unwrap()
}

fn pk6() -> Pokemon {
    Pokemon::from_bytes(PkmFormat::Gen6, fixture!("pk6_276_nirondelle_party.pk6")).unwrap()
}

fn origin() -> Origin {
    Origin { game: Some("Pokémon Platine".into()), save: Some("platine.sav".into()) }
}

/// Première case vide des boîtes de la sauvegarde.
fn empty_box_slot(s: &SaveSession) -> Slot {
    for b in 0..s.save.box_count() {
        for i in 0..BOX_SLOTS {
            let slot = Slot::Box { r#box: b, index: i };
            if s.get(slot).unwrap().is_none() {
                return slot;
            }
        }
    }
    panic!("aucune case vide");
}

#[test]
fn index_roundtrip_on_disk() {
    let dir = TempDir::new("index");
    let mut bank = Bank::open(&dir.0).unwrap();
    assert_eq!(bank.boxes().len(), 1);
    assert_eq!(bank.count(), 0);
    let a = bank.deposit(&pk4(), None, &origin()).unwrap();
    assert_eq!(a, BankSlot { r#box: 0, index: 0 });
    let b = bank.deposit(&pk6(), Some(BankSlot { r#box: 0, index: 7 }), &Origin::default()).unwrap();
    assert!(bank.deposit(&pk6(), Some(b), &Origin::default()).is_err(), "case occupée");
    let nb = bank.add_box(Some("Légendaires".into())).unwrap();
    bank.rename_box(0, "Mes favoris").unwrap();
    bank.move_slot(b, BankSlot { r#box: nb, index: 3 }).unwrap();

    // Fichiers natifs déchiffrés : relus tels quels par un autre outil.
    let files: Vec<_> = std::fs::read_dir(dir.0.join(POKEMON_DIR)).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|f| f.ends_with(".pk4")) && files.iter().any(|f| f.ends_with(".pk6")));

    // Réouverture : même contenu.
    let bank = Bank::open(&dir.0).unwrap();
    let boxes = bank.boxes();
    assert_eq!(boxes.len(), 2);
    assert_eq!((boxes[0].name.as_str(), boxes[0].count), ("Mes favoris", 1));
    assert_eq!((boxes[1].name.as_str(), boxes[1].count), ("Légendaires", 1));
    let first = bank.box_view(0).unwrap()[0].clone().unwrap();
    assert_eq!(first.species_name, "Roserade");
    assert_eq!(first.format, "PK4");
    assert_eq!(first.entry.origin_save.as_deref(), Some("platine.sav"));
    assert_eq!(bank.read(a).unwrap().stored_data(), pk4().stored_data());
    let moved = bank.read(BankSlot { r#box: 1, index: 3 }).unwrap();
    assert_eq!(moved.stored_data(), pk6().stored_data());
    assert!(moved.checksum_valid());
}

#[test]
fn search_filters_and_boxes() {
    let dir = TempDir::new("search");
    let mut bank = Bank::open(&dir.0).unwrap();
    bank.deposit(&pk4(), None, &origin()).unwrap();
    bank.deposit(&pk6(), None, &origin()).unwrap();
    let hax = Pokemon::from_bytes(PkmFormat::Gen5, fixture!("pk5_612_haxorus_party.pk5")).unwrap();
    bank.deposit(&hax, None, &origin()).unwrap();
    assert_eq!(bank.search(&BankQuery::default()).len(), 3);
    // Nom français de l'espèce, sans tenir compte des accents ni de la casse.
    let r = bank.search(&BankQuery { text: "TRANCHO".into(), ..Default::default() });
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].entry.species, 612);
    assert_eq!(bank.search(&BankQuery { text: "ROSÉRA".into(), ..Default::default() }).len(), 1);
    assert_eq!(bank.search(&BankQuery { shiny: Some(true), ..Default::default() }).len(), 2);
    assert_eq!(bank.search(&BankQuery { generation: Some(6), ..Default::default() }).len(), 1);
    // Boîtes : on ne supprime ni une boîte pleine ni la dernière.
    assert!(bank.delete_box(0).is_err());
    let nb = bank.add_box(None).unwrap();
    assert_eq!(bank.boxes()[nb].name, "Boîte 2");
    bank.delete_box(nb).unwrap();
    assert_eq!(bank.boxes().len(), 1);
    // Suppression : fichier gardé dans la corbeille.
    bank.remove(BankSlot { r#box: 0, index: 0 }).unwrap();
    assert_eq!(bank.count(), 2);
    assert_eq!(std::fs::read_dir(dir.0.join(TRASH_DIR)).unwrap().count(), 1);
}

#[test]
fn import_export_files() {
    let dir = TempDir::new("import");
    let mut bank = Bank::open(&dir.0).unwrap();
    let s = bank.import_bytes(fixture!("pk7_151_mew.pk7"), Some("pk7"), None, &Origin::default()).unwrap();
    // Sans extension : 232 octets d'un Pokémon de Soleil / Lune → PK7.
    let t = bank.import_bytes(fixture!("pk7_666_vivillon.pk7"), None, None, &Origin::default()).unwrap();
    assert_eq!(bank.box_view(0).unwrap()[t.index].as_ref().unwrap().format, "PK7");
    let (bytes, ext) = bank.export(s).unwrap();
    assert_eq!(ext, "pk7");
    assert_eq!(bytes, fixture!("pk7_151_mew.pk7")[..232].to_vec());
    assert!(bank.import_bytes(&[0; 10], None, None, &Origin::default()).is_err());
}

#[test]
fn first_free_grows_the_bank() {
    let dir = TempDir::new("grow");
    let mut bank = Bank::open(&dir.0).unwrap();
    for _ in 0..BOX_SLOTS {
        bank.deposit(&pk4(), None, &Origin::default()).unwrap();
    }
    let s = bank.deposit(&pk4(), None, &Origin::default()).unwrap();
    assert_eq!(s, BankSlot { r#box: 1, index: 0 });
    assert_eq!(bank.count(), 31);
}

#[test]
fn withdraw_into_sun_moon_and_reopen() {
    let dir = TempDir::new("withdraw");
    let mut bank = Bank::open(&dir.0).unwrap();
    let from = bank.deposit(&pk4(), None, &origin()).unwrap();
    let mut s = SaveSession::open(fixture!("sm_project_802.main")).unwrap();
    let detail = bank.detail(from, Some(s.game())).unwrap();
    let c = detail.compatibility.unwrap();
    assert!(c.ok && c.converts);
    let to = empty_box_slot(&s);
    let view = withdraw_to_save(&mut bank, &mut s, from, to, false, false).unwrap();
    assert_eq!(view.summary.species, 407);
    assert!(view.summary.checksum_valid);
    assert_eq!(bank.count(), 0, "déplacé : il quitte la banque");
    // Annulable dans la sauvegarde.
    assert!(s.view().unwrap().can_undo);

    // Réécrite puis relue : CRC et signature valides, Pokémon PK7 lisible.
    let out = s.to_bytes();
    let re = SaveFile::from_bytes(&out).unwrap();
    assert_eq!(re.version(), SaveVersion::SunMoon);
    let bad: Vec<_> = re.checksums().into_iter().filter(|c| !c.valid).collect();
    assert!(bad.is_empty(), "{bad:?}");
    let Slot::Box { r#box, index } = to else { unreachable!() };
    let p = re.box_slot(r#box, index).unwrap().unwrap();
    assert_eq!(p.format(), PkmFormat::Gen7);
    assert_eq!(p.species(), 407);
    assert_eq!(p.ivs(), pk4().ivs());
    assert_eq!(p.met_location(), convert::TRANSFER4);
    assert!(p.checksum_valid());
    assert_eq!(convert::handler(&p).as_deref(), Some("PPorg"));
}

#[test]
fn withdraw_refuses_older_games_and_occupied_slots() {
    let dir = TempDir::new("refuse");
    let mut bank = Bank::open(&dir.0).unwrap();
    let from = bank.deposit(&pk6(), None, &Origin::default()).unwrap();
    let mut s = SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
    let free = empty_box_slot(&s);
    let err = withdraw_to_save(&mut bank, &mut s, from, free, false, false).unwrap_err();
    assert!(err.to_string().contains("plus récente"), "{err}");
    assert_eq!(bank.count(), 1, "il reste dans la banque");
    let occupied = Slot::Party { index: 0 };
    let from4 = bank.deposit(&pk4(), None, &Origin::default()).unwrap();
    assert!(withdraw_to_save(&mut bank, &mut s, from4, occupied, true, false).is_err());
    // Copie dans la même génération : la banque garde son exemplaire.
    let v = withdraw_to_save(&mut bank, &mut s, from4, free, true, false).unwrap();
    assert_eq!(v.summary.species, 407);
    assert_eq!(bank.count(), 2);
}

#[test]
fn deposit_move_removes_from_save_in_one_step() {
    let dir = TempDir::new("deposit");
    let mut bank = Bank::open(&dir.0).unwrap();
    let mut s = SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
    let slot = Slot::Party { index: 1 };
    let before = s.get(slot).unwrap().unwrap();
    let count = s.save.party_count();
    let b = deposit_from_save(&mut bank, &mut s, slot, None, false, Some("demo.sav".into())).unwrap();
    assert_eq!(s.save.party_count(), count - 1);
    assert_eq!(bank.read(b).unwrap().stored_data(), before.stored_data());
    assert_eq!(bank.box_view(0).unwrap()[0].as_ref().unwrap().entry.origin_game.as_deref(), Some("Pokémon Platine"));
    // Une seule étape d'annulation remet le Pokémon dans l'équipe.
    assert!(s.undo());
    assert_eq!(s.save.party_count(), count);
    assert!(!s.undo(), "le dépôt n'a fait qu'une étape");

    // Copie : la sauvegarde ne change pas.
    deposit_from_save(&mut bank, &mut s, slot, None, true, None).unwrap();
    assert_eq!(s.save.party_count(), count);
    assert_eq!(bank.count(), 2);
}

#[test]
fn last_party_member_stays() {
    let dir = TempDir::new("last");
    let mut bank = Bank::open(&dir.0).unwrap();
    let mut s = SaveSession::open(fixture!("sm_project_802.main")).unwrap();
    assert_eq!(s.save.party_count(), 1);
    assert!(deposit_from_save(&mut bank, &mut s, Slot::Party { index: 0 }, None, false, None).is_err());
    assert_eq!(bank.count(), 0);
    // En copie, c'est permis.
    deposit_from_save(&mut bank, &mut s, Slot::Party { index: 0 }, None, true, None).unwrap();
    assert_eq!(bank.count(), 1);
}
