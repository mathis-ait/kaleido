//! Gen 1 et 2 : PK1 / PK2 réels des tests de PKHeX (`Tests/PKHeX.Core.Tests/Legality/Legal`,
//! copiés dans `tests/data/pkhex/`) et sauvegardes **synthétiques** (aucune vraie sauvegarde
//! Game Boy n'était disponible) construites par `gen12::blank`.

use super::convert::{convert_chain as convert, pk12_to_pk7, TRANSFER1, TRANSFER2};
use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/", $name))
    };
}

const GB: [(&[u8], PkmFormat, u16); 5] = [
    (fixture!("pk1_004_charmander.pk1"), PkmFormat::Gen1, 4),
    (fixture!("pk1_025_pikachu.pk1"), PkmFormat::Gen1, 25),
    (fixture!("pk2_025_pikachu.pk2"), PkmFormat::Gen2, 25),
    (fixture!("pk2_230_kingdra.pk2"), PkmFormat::Gen2, 230),
    (fixture!("pk2_251_celebi.pk2"), PkmFormat::Gen2, 251),
];

#[test]
fn real_pk1_pk2_files_round_trip() {
    for (bytes, format, species) in GB {
        let pk = Pokemon::from_bytes(format, bytes).unwrap();
        assert_eq!(pk.species(), species);
        assert!(!pk.ot_name().is_empty(), "{species}");
        // Relu puis réécrit sans changement : octets identiques.
        assert_eq!(&pk.stored_data()[..], bytes, "{species}");
        let back = Pokemon::from_bytes(format, &pk.stored_data()).unwrap();
        assert_eq!(&back.stored_data()[..], bytes);
        // Valeurs génétiques : les DV (0 à 15) tels quels ; Atq Spé = Déf Spé (Spécial) ; PV déduits des bits faibles.
        let iv = pk.ivs();
        assert!(iv.iter().all(|&v| v <= 15) && iv[3] == iv[4], "{species} {iv:?}");
        assert_eq!(iv[0], (iv[1] & 1) << 3 | (iv[2] & 1) << 2 | (iv[5] & 1) << 1 | (iv[3] & 1));
        assert_eq!(pk.nature(), (pk.exp() % 25) as u8);
    }
}

#[test]
fn gb_edits_are_written_back() {
    let mut pk = Pokemon::from_bytes(PkmFormat::Gen2, GB[2].0).unwrap();
    pk.set_nickname("PIKA").unwrap();
    pk.set_moves([84, 85, 0, 0]);
    pk.set_ivs([15, 10, 10, 10, 10, 10]).unwrap();
    pk.set_held_item(crate::save::pk12::item2_exposed(0x52)); // Baie Mystère ou équivalent
    let back = Pokemon::from_bytes(PkmFormat::Gen2, &pk.stored_data()).unwrap();
    assert_eq!(back.nickname(), "PIKA");
    assert_eq!(back.moves(), [84, 85, 0, 0]);
    assert_eq!(&back.ivs()[1..], &[10, 10, 10, 10, 10]);
    assert_eq!(back.held_item(), pk.held_item());
    // Chromatique Gen 2 : forcé puis retiré.
    let mut s = back.clone();
    s.set_shiny(ShinyMode::Star);
    assert!(s.is_shiny());
    s.set_shiny(ShinyMode::None);
    assert!(!s.is_shiny());
}

#[test]
fn virtual_console_transfer() {
    for (bytes, format, species) in GB {
        let pk = Pokemon::from_bytes(format, bytes).unwrap();
        let pk7 = pk12_to_pk7(&pk).unwrap();
        assert_eq!(pk7.format(), PkmFormat::Gen7);
        assert_eq!(pk7.species(), species);
        assert!(pk7.checksum_valid());
        assert_eq!(pk7.met_location(), if format == PkmFormat::Gen1 { TRANSFER1 } else { TRANSFER2 });
        assert_eq!(pk7.tid(), pk.tid());
        assert_eq!(pk7.is_shiny(), pk.is_shiny(), "{species}");
        assert_eq!(pk7.ivs().iter().filter(|&&v| v == 31).count() >= if matches!(species, 151 | 251) { 5 } else { 3 }, true);
        // Déterministe.
        assert_eq!(pk12_to_pk7(&pk).unwrap().stored_data(), pk7.stored_data());
        // Chaîne générique.
        assert_eq!(convert(&pk, PkmFormat::Gen7, None).unwrap().species(), species);
    }
}

#[test]
fn synthetic_gb_saves_are_identified_and_round_trip() {
    for (version, boxes) in [(SaveVersion::RedBlue, 12), (SaveVersion::Yellow, 12), (SaveVersion::GoldSilver, 14), (SaveVersion::Crystal, 14)] {
        let blank = gen12::blank(version);
        assert_eq!(saves::identify(&blank).map(|k| k.generation()), Some(version.format().generation()));
        let save = SaveFile::from_bytes(&blank).unwrap();
        assert_eq!(save.version(), version);
        assert!(save.checksums_valid(), "{version:?}");
        assert_eq!(save.box_count(), boxes);
        assert_eq!(save.trainer().name, "THISMA");
        assert_eq!(save.party_count(), 0);
        assert_eq!(save.to_bytes(), blank, "{version:?}");
        // Pied d'horloge conservé.
        let mut rtc = blank.clone();
        rtc.extend([3u8; 0x30]);
        assert_eq!(SaveFile::from_bytes(&rtc).unwrap().to_bytes(), rtc);
    }
}

#[test]
fn edit_gb_save_and_reread() {
    for version in [SaveVersion::RedBlue, SaveVersion::Yellow, SaveVersion::GoldSilver, SaveVersion::Crystal] {
        let format = version.format();
        let (bytes, species) = if format == PkmFormat::Gen1 { (GB[1].0, 25) } else { (GB[3].0, 230) };
        let pk = Pokemon::from_bytes(format, bytes).unwrap();
        let mut save = SaveFile::from_bytes(&gen12::blank(version)).unwrap();
        save.set_party_slot(0, Some(pk.clone())).unwrap();
        save.set_party_slot(1, Some(pk.clone())).unwrap();
        save.set_box_slot(0, 0, Some(pk.clone())).unwrap();
        save.set_box_slot(0, 5, Some(pk.clone())).unwrap(); // liste compacte : va en 2e place
        save.set_box_slot(7, 0, Some(pk.clone())).unwrap(); // banque 3
        save.set_trainer(&edit::TrainerPatch { name: Some("Red".into()), money: Some(123_456), tid: Some(4242), hours: Some(12), ..Default::default() }).unwrap();
        save.set_badges(0b1000_0001);
        save.set_dex_entry(species, true, true).unwrap();
        if format == PkmFormat::Gen2 {
            save.set_box_name(2, "Kaleido").unwrap();
        } else {
            assert!(save.set_box_name(2, "X").is_err());
        }

        let out = save.to_bytes();
        assert_eq!(out.len(), gen12::SIZE);
        let back = SaveFile::from_bytes(&out).unwrap();
        assert_eq!(back.version(), version);
        assert!(back.checksums_valid(), "{version:?}");
        assert_eq!(back.party_count(), 2);
        assert_eq!(back.party_slot(1).unwrap().unwrap().species(), species);
        assert_eq!(back.box_slot(0, 1).unwrap().unwrap().species(), species);
        assert!(back.box_slot(0, 2).unwrap().is_none());
        let boxed = back.box_slot(7, 0).unwrap().unwrap();
        assert_eq!((boxed.species(), boxed.ot_name()), (species, pk.ot_name()));
        let t = back.trainer();
        assert_eq!((t.name.as_str(), t.money, t.tid, t.play_time.hours), ("RED", 123_456, 4242, 12));
        assert_eq!(back.badges(), Some(0b1000_0001));
        let dex = back.pokedex().unwrap();
        assert_eq!(dex.len(), if format == PkmFormat::Gen1 { 151 } else { 251 });
        assert!(dex[species as usize - 1].caught);
        if format == PkmFormat::Gen2 {
            assert_eq!(back.box_name(2).unwrap(), "KALEIDO");
        }
        // Retrait : les suivants remontent.
        let mut save = back;
        save.set_box_slot(0, 0, None).unwrap();
        assert!(save.box_slot(0, 1).unwrap().is_none());
        assert!(save.set_party_slot(0, None).is_ok());
        assert!(save.set_party_slot(0, None).is_err()); // dernier membre
        // Un PK3 est refusé.
        let pk3 = Pokemon::blank(PkmFormat::Gen3);
        assert!(save.set_box_slot(0, 0, Some(pk3)).is_err());
    }
}
