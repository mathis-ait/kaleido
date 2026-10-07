//! Gen 3 : PK3 réels des tests de PKHeX (`Tests/PKHeX.Core.Tests/Legality/Legal`, stockés
//! déchiffrés, copiés dans `tests/data/pkhex/`) et sauvegardes **synthétiques** (aucune
//! vraie sauvegarde Gen 3 n'était disponible) construites par `gen3::blank`.

use super::convert::{convert_chain as convert, pk3_to_pk4, TRANSFER3};
use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/", $name))
    };
}

const PK3: [(&[u8], u16, &str, &str); 7] = [
    (fixture!("pk3_054_psyduck.pk3"), 54, "PSYDUCK", "TERRA"),
    (fixture!("pk3_124_zynx.pk3"), 124, "ZYNX", ""),
    (fixture!("pk3_144_articuno.pk3"), 144, "", ""),
    (fixture!("pk3_195_quagsire.pk3"), 195, "", ""),
    (fixture!("pk3_292_shedinja.pk3"), 292, "SHEDINJA", ""),
    (fixture!("pk3_297_makit.pk3"), 297, "MAKIT", ""),
    (fixture!("pk3_327_spinda.pk3"), 327, "", ""),
];

#[test]
fn real_pk3_files_round_trip() {
    for (bytes, species, nickname, ot) in PK3 {
        let pk = Pokemon::from_bytes(PkmFormat::Gen3, bytes).unwrap();
        assert_eq!(pk.species(), species);
        assert!(pk.checksum_valid(), "{species}");
        if !nickname.is_empty() {
            assert_eq!(pk.nickname(), nickname);
        }
        if !ot.is_empty() {
            assert_eq!(pk.ot_name(), ot);
        }
        // Relu puis réécrit sans changement : octets identiques.
        assert_eq!(&pk.stored_data()[..], &bytes[..80], "{species}");
        // Chiffré puis déchiffré.
        let enc = pk.encrypt_party();
        assert_eq!(enc.len(), 100);
        let back = Pokemon::from_bytes(PkmFormat::Gen3, &enc).unwrap();
        assert_eq!(back.species(), species);
        assert_eq!(&back.stored_data()[..], &bytes[..80]);
    }
    // Psykokwak : attaque Lance-Boue (300), Poké Ball, Rubis (version 2).
    let psy = Pokemon::from_bytes(PkmFormat::Gen3, PK3[0].0).unwrap();
    assert_eq!(psy.moves(), [300, 0, 0, 0]);
    assert_eq!(psy.ball(), 4);
    assert_eq!(psy.version(), 2);
    assert_eq!(psy.nature(), (psy.pid() % 25) as u8);
}

#[test]
fn pk3_edits_are_written_back() {
    let mut pk = Pokemon::from_bytes(PkmFormat::Gen3, PK3[0].0).unwrap();
    pk.set_nickname("Kwak").unwrap();
    pk.set_moves([300, 55, 0, 0]);
    pk.set_held_item(17); // Potion (Gen 4) = 13 en Gen 3
    pk.set_ivs([31, 30, 29, 28, 27, 26]).unwrap();
    pk.set_evs([4, 252, 0, 0, 0, 252]);
    let raw = pk.stored_data().to_vec();
    let back = Pokemon::from_bytes(PkmFormat::Gen3, &raw).unwrap();
    assert_eq!(back.nickname(), "Kwak");
    assert_eq!(back.moves(), [300, 55, 0, 0]);
    assert_eq!(back.held_item(), 17);
    assert_eq!(u16::from_le_bytes([raw[0x22], raw[0x23]]), 13);
    assert_eq!(back.ivs(), [31, 30, 29, 28, 27, 26]);
    assert_eq!(back.evs(), [4, 252, 0, 0, 0, 252]);
    assert!(back.checksum_valid());
}

#[test]
fn pal_park_conversion() {
    let pk3 = Pokemon::from_bytes(PkmFormat::Gen3, PK3[0].0).unwrap();
    let pk4 = pk3_to_pk4(&pk3).unwrap();
    assert_eq!(pk4.format(), PkmFormat::Gen4);
    assert_eq!(pk4.species(), 54);
    assert_eq!(pk4.pid(), pk3.pid());
    assert_eq!(pk4.met_location(), TRANSFER3);
    assert_eq!(pk4.friendship(), 70);
    assert_eq!(pk4.version(), 2);
    assert_eq!(pk4.moves(), pk3.moves());
    assert_eq!(pk4.nickname(), "PSYDUCK");
    assert!(pk4.checksum_valid());
    // Enchaînement jusqu'à la Gen 7.
    let pk7 = convert(&pk3, PkmFormat::Gen7, None).unwrap();
    assert_eq!((pk7.format(), pk7.species()), (PkmFormat::Gen7, 54));
    // Pas de retour en arrière.
    assert!(convert(&pk4, PkmFormat::Gen3, None).is_err());
}

#[test]
fn synthetic_saves_are_identified_and_round_trip() {
    for version in [SaveVersion::RubySapphire, SaveVersion::Emerald, SaveVersion::FireRedLeafGreen] {
        let blank = gen3::blank(version);
        assert_eq!(saves::identify(&blank).map(|k| k.generation()), Some(3));
        let save = SaveFile::from_bytes(&blank).unwrap();
        assert_eq!(save.version(), version);
        assert_eq!(save.format(), PkmFormat::Gen3);
        assert!(save.checksums_valid(), "{version:?}");
        assert_eq!(save.box_count(), 14);
        assert_eq!(save.trainer().name, "THISMA");
        // Rien de modifié : fichier identique.
        assert_eq!(save.to_bytes(), blank, "{version:?}");
        // Pied d'horloge de mGBA conservé.
        let mut with_rtc = blank.clone();
        with_rtc.extend([7u8; gen3::RTC_FOOTER]);
        let s = SaveFile::from_bytes(&with_rtc).unwrap();
        assert_eq!(s.to_bytes(), with_rtc);
    }
}

#[test]
fn edit_gen3_save_and_reread() {
    for version in [SaveVersion::RubySapphire, SaveVersion::Emerald, SaveVersion::FireRedLeafGreen] {
        let mut save = SaveFile::from_bytes(&gen3::blank(version)).unwrap();
        // Boîtes : un emplacement à cheval sur deux secteurs (boîte 1, emplacement 21 : 0x4D80 + 4 + 51 × 80 > 0x5D00).
        let psy = Pokemon::from_bytes(PkmFormat::Gen3, PK3[0].0).unwrap();
        save.set_box_slot(1, 21, Some(psy.clone())).unwrap();
        save.set_box_slot(13, 29, Some(psy.clone())).unwrap();
        // Équipe : statistiques calculées.
        let mut party = psy.clone();
        let base = crate::dex::personal(crate::dex::Game::E, 54, 0).unwrap();
        party.update_party_stats(&base.base_stats, base.growth_rate);
        save.set_party_slot(0, Some(party)).unwrap();
        save.set_trainer(&edit::TrainerPatch { name: Some("Kaleido".into()), money: Some(123_456), ..Default::default() }).unwrap();
        save.set_box_name(0, "Equipe A").unwrap();
        save.set_badges(0b1010_0101);

        let bytes = save.to_bytes();
        assert_eq!(bytes.len(), gen3::FULL_SIZE);
        let back = SaveFile::from_bytes(&bytes).unwrap();
        assert!(back.checksums_valid(), "{version:?}");
        assert_eq!(back.box_slot(1, 21).unwrap().unwrap().stored_data(), psy.stored_data());
        assert_eq!(back.box_slot(13, 29).unwrap().unwrap().species(), 54);
        assert_eq!(back.party_count(), 1);
        assert!(back.party_slot(0).unwrap().unwrap().party_stats().is_some());
        let t = back.trainer();
        assert_eq!((t.name.as_str(), t.money), ("Kaleido", 123_456));
        assert_eq!(back.box_name(0).unwrap(), "Equipe A");
        assert_eq!(back.badges(), Some(0b1010_0101));
        // Clé de sécurité : l'argent est stocké chiffré en Émeraude.
        let key = gen3::security_key(version, &back.data);
        assert_eq!(rd_u32(&back.data, back.layout.trainer.money), 123_456 ^ key);
        if version == SaveVersion::Emerald {
            assert_ne!(key, 0);
        }
    }
}

#[test]
fn gen3_bag_and_dex() {
    let mut save = SaveFile::from_bytes(&gen3::blank(SaveVersion::Emerald)).unwrap();
    let mut bag = save.inventory().unwrap();
    let items = bag.iter_mut().find(|p| p.kind == PouchKind::Items).unwrap();
    // Potion (17 en Gen 4) et une Clé (objet rare sans équivalent, exposé avec le drapeau Gen 3).
    items.items = vec![InventoryItem { id: 17, count: 5, is_new: false, is_favorite: false }];
    let keys = bag.iter_mut().find(|p| p.kind == PouchKind::KeyItems).unwrap();
    let bike = crate::dex::GEN3_ITEM_FLAG | 259;
    assert!(keys.allowed.contains(&bike));
    keys.items = vec![InventoryItem { id: bike, count: 1, is_new: false, is_favorite: false }];
    save.set_inventory(&bag).unwrap();
    let back = SaveFile::from_bytes(&save.to_bytes()).unwrap();
    let inv = back.inventory().unwrap();
    assert_eq!(inv.iter().find(|p| p.kind == PouchKind::Items).unwrap().items[0].count, 5);
    assert_eq!(inv.iter().find(|p| p.kind == PouchKind::KeyItems).unwrap().items[0].id, bike);
    assert!(crate::dex::item_name(bike).is_some());

    let mut save = back;
    save.set_dex_entry(386, true, true).unwrap();
    let back = SaveFile::from_bytes(&save.to_bytes()).unwrap();
    let dex = back.pokedex().unwrap();
    assert_eq!(dex.len(), 386);
    assert!(dex[385].seen && dex[385].caught);
    assert!(!dex[0].seen);
}

#[test]
fn session_view_and_new_pokemon() {
    let mut s = session::SaveSession::open(&gen3::blank(SaveVersion::FireRedLeafGreen)).unwrap();
    let p = s.new_pokemon(4, 12).unwrap();
    assert_eq!(p.format(), PkmFormat::Gen3);
    assert_eq!(p.species(), 4);
    assert_eq!(p.ability(), 66); // Brasier
    s.save.set_box_slot(0, 0, Some(p)).unwrap();
    let view = s.view().unwrap();
    assert_eq!(view.generation, 3);
    let back = SaveFile::from_bytes(&s.to_bytes()).unwrap();
    let p = back.box_slot(0, 0).unwrap().unwrap();
    assert_eq!((p.species(), p.ability(), p.checksum_valid()), (4, 66, true));
    assert!(session::SaveSession::open(&s.to_bytes()).unwrap().live().is_ok());
}

#[test]
fn gen3_legality_basics() {
    // Fichiers « légaux » de PKHeX : jamais déclarés illégaux (douteux si la rencontre est un
    // Pokémon fixe, un don ou un œuf, absents de la base Gen 3 de Kaleido).
    for (bytes, species, _, _) in PK3 {
        let pk = Pokemon::from_bytes(PkmFormat::Gen3, bytes).unwrap();
        let report = crate::legality::verify::analyze(&pk, crate::dex::Game::E);
        let errors: Vec<_> = report.checks.iter().filter(|c| c.severity == crate::legality::verify::Severity::Invalid).map(|c| c.title.clone()).collect();
        assert!(errors.is_empty(), "{species} : {errors:?}");
    }
}

