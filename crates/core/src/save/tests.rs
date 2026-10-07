//! Tests aller-retour sur des sauvegardes synthétiques, pour chaque génération.

use super::pkm::tests::sample;
use super::*;

/// Écrit nom, TID/SID, argent et temps de jeu directement dans les données.
fn poke_trainer(save: &mut SaveFile, name: &str) {
    let t = save.layout.trainer.clone();
    let format = save.format();
    let bytes = strings::encode(format, name, t.name_max).unwrap();
    save.data[t.name..t.name + bytes.len()].copy_from_slice(&bytes);
    save.data[t.tid..t.tid + 2].copy_from_slice(&12345u16.to_le_bytes());
    save.data[t.sid..t.sid + 2].copy_from_slice(&54321u16.to_le_bytes());
    save.data[t.gender] = 1;
    save.data[t.money..t.money + 4].copy_from_slice(&987_654u32.to_le_bytes());
    save.data[t.hours..t.hours + 2].copy_from_slice(&123u16.to_le_bytes());
    save.data[t.minutes] = 45;
    save.data[t.seconds] = 6;
}

/// Scénario commun : dresseur, deux Pokémon en boîte, deux dans l'équipe, écriture,
/// relecture, puis retrait d'un membre de l'équipe.
fn roundtrip(bytes: Vec<u8>, version: SaveVersion) -> SaveFile {
    let mut save = SaveFile::from_bytes(&bytes).unwrap();
    assert_eq!(save.version(), version);
    let format = version.format();
    assert_eq!(save.format(), format);
    assert_eq!(save.party_count(), 0);
    assert!(save.party().unwrap().is_empty());
    let last_box = save.box_count() - 1;
    for b in [0, last_box] {
        for s in [0, BOX_SLOTS - 1] {
            assert_eq!(save.box_slot(b, s).unwrap(), None);
        }
    }

    poke_trainer(&mut save, "Thisma");
    let a = sample(format, 0x0000_A000);
    let mut b = sample(format, 0x7FFF_3FFF);
    b.set_species(25);
    b.set_nickname("Pikachu").unwrap();
    b.refresh_checksum();
    save.set_box_slot(0, 0, Some(a.clone())).unwrap();
    save.set_box_slot(last_box, BOX_SLOTS - 1, Some(b.clone())).unwrap();
    save.set_party_slot(0, Some(a.clone())).unwrap();
    save.set_party_slot(1, Some(b.clone())).unwrap();

    let out = save.to_bytes();
    assert_eq!(out.len(), bytes.len());
    let mut re = SaveFile::from_bytes(&out).unwrap();
    assert_eq!(re.version(), version);
    let bad: Vec<_> = re.checksums().into_iter().filter(|c| !c.valid).collect();
    assert!(bad.is_empty(), "{version:?} : {bad:?}");

    let t = re.trainer();
    assert_eq!(t.name, "Thisma");
    assert_eq!((t.tid, t.sid, t.money), (12345, 54321, 987_654));
    assert_eq!(t.gender, Gender::Female);
    assert_eq!(t.play_time, PlayTime { hours: 123, minutes: 45, seconds: 6 });
    let expected_id = if version.generation() == 7 { (54321u32 << 16 | 12345) % 1_000_000 } else { 12345 };
    assert_eq!(t.display_id, expected_id);

    let boxed_a = re.box_slot(0, 0).unwrap().unwrap();
    assert_eq!(boxed_a.stored_data(), a.stored_data());
    let boxed_b = re.box_slot(last_box, BOX_SLOTS - 1).unwrap().unwrap();
    assert_eq!(boxed_b.stored_data(), b.stored_data());
    assert_eq!(boxed_b.nickname(), "Pikachu");
    assert_eq!(re.box_slot(0, 1).unwrap(), None);
    assert_eq!(re.party().unwrap(), vec![a.clone(), b.clone()]);

    // Retrait du premier membre : le second remonte.
    re.set_party_slot(0, None).unwrap();
    assert_eq!(re.party().unwrap(), vec![b.clone()]);
    assert!(matches!(re.set_party_slot(0, None), Err(SaveError::LastPartyMember)));
    assert!(matches!(re.set_party_slot(3, Some(a.clone())), Err(SaveError::PartyGap { slot: 3, count: 1 })));
    re.set_box_slot(0, 0, None).unwrap();
    assert_eq!(re.box_slot(0, 0).unwrap(), None);

    let again = SaveFile::from_bytes(&re.to_bytes()).unwrap();
    assert!(again.checksums_valid(), "{version:?}");
    assert_eq!(again.party().unwrap(), vec![b]);
    assert_eq!(again.box_slot(0, 0).unwrap(), None);

    // Erreurs d'index et de format.
    assert!(matches!(again.box_slot(again.box_count(), 0), Err(SaveError::BadBox(_))));
    assert!(matches!(again.box_slot(0, BOX_SLOTS), Err(SaveError::BadSlot(_))));
    let other = if format == PkmFormat::Gen4 { PkmFormat::Gen5 } else { PkmFormat::Gen4 };
    let mut again = again;
    assert!(matches!(again.set_box_slot(0, 0, Some(sample(other, 1))), Err(SaveError::FormatMismatch { .. })));
    let boxed_only = Pokemon::from_decrypted(format, a.stored_data()).unwrap();
    assert!(matches!(again.set_party_slot(1, Some(boxed_only)), Err(SaveError::MissingPartyStats)));

    let text = describe(&again);
    assert!(text.contains("Thisma"), "{text}");
    assert!(text.contains("Pikachu"), "{text}");
    again
}

#[test]
fn gen4_diamond_pearl() {
    let save = roundtrip(gen4::blank(SaveVersion::DiamondPearl, 0, 0), SaveVersion::DiamondPearl);
    assert_eq!(save.box_count(), 18);
}

#[test]
fn gen4_platinum_uses_newest_partitions() {
    let v = SaveVersion::Platinum;
    let c = gen4::consts(v);
    // Bloc général le plus récent dans la 2e partition, boîtes dans la 1re.
    let blank = gen4::blank(v, 1, 0);
    let mut save = SaveFile::from_bytes(&blank).unwrap();
    save.set_box_slot(0, 0, Some(sample(PkmFormat::Gen4, 0x1234))).unwrap();
    save.set_party_slot(0, Some(sample(PkmFormat::Gen4, 0x5678))).unwrap();
    let out = save.to_bytes();
    // Les écritures ont eu lieu dans la bonne partition, l'autre copie est intacte.
    assert_eq!(out[0xA0 - 4], 0);
    assert_eq!(out[gen4::PARTITION + 0xA0 - 4], 1);
    assert_ne!(&out[c.storage_start + 4..c.storage_start + 4 + 136], &[0u8; 136][..]);
    let other = gen4::PARTITION + c.storage_start + 4;
    assert_eq!(&out[other..other + 136], &[0u8; 136][..]);
    // Le CRC est à la fin du bloc (pied de 0x14 octets).
    let end = gen4::PARTITION + c.general_size;
    let crc = checksum::crc16_ccitt(&out[gen4::PARTITION..end - 0x14]);
    assert_eq!(u16::from_le_bytes([out[end - 2], out[end - 1]]), crc);

    roundtrip(blank, v);
}

#[test]
fn gen4_hgss_with_desmume_footer() {
    let v = SaveVersion::HeartGoldSoulSilver;
    let mut bytes = gen4::blank(v, 0, 1);
    let footer: Vec<u8> = (0..DESMUME_FOOTER as u8).collect();
    bytes.extend_from_slice(&footer);
    let save = roundtrip(bytes, v);
    let out = save.to_bytes();
    assert_eq!(out.len(), NDS_SAVE_SIZE + DESMUME_FOOTER);
    assert_eq!(&out[NDS_SAVE_SIZE..], &footer[..]);
}

#[test]
fn gen4_box_names() {
    let v = SaveVersion::DiamondPearl;
    let mut save = SaveFile::from_bytes(&gen4::blank(v, 0, 0)).unwrap();
    let at = save.layout.box_names + 2 * save.layout.box_name_stride;
    let name = strings::encode(PkmFormat::Gen4, "BOÎTE 3", 19).unwrap();
    save.data[at..at + name.len()].copy_from_slice(&name);
    assert_eq!(save.box_name(2).unwrap(), "BOÎTE 3");
    assert_eq!(save.box_name(0).unwrap(), "");
    assert!(matches!(save.box_name(18), Err(SaveError::BadBox(18))));
}

#[test]
fn gen5_black_white() {
    let save = roundtrip(gen5::blank(SaveVersion::BlackWhite), SaveVersion::BlackWhite);
    assert_eq!(save.box_count(), 24);
}

#[test]
fn gen5_black2_white2() {
    roundtrip(gen5::blank(SaveVersion::Black2White2), SaveVersion::Black2White2);
}

#[test]
fn gen5_unknown_table_is_rejected() {
    // 512 Kio sans pied Gen 4 ni table Gen 5 valide.
    assert!(matches!(SaveFile::from_bytes(&vec![0; NDS_SAVE_SIZE]), Err(SaveError::Gen5Table)));
}

#[test]
fn gen6_xy() {
    let (bytes, _) = gen6::blank(0x65600, &gen6::synthetic_lengths(SaveVersion::XY));
    let save = roundtrip(bytes, SaveVersion::XY);
    assert_eq!(save.box_count(), 31);
    assert!(save.warnings().is_empty(), "{:?}", save.warnings());
    assert!(!save.needs_resign());
}

#[test]
fn gen6_oras() {
    let (bytes, _) = gen6::blank(0x76000, &gen6::synthetic_lengths(SaveVersion::OmegaRubyAlphaSapphire));
    let save = roundtrip(bytes, SaveVersion::OmegaRubyAlphaSapphire);
    assert!(save.warnings().is_empty(), "{:?}", save.warnings());
}

#[test]
fn gen6_missing_beef() {
    assert!(matches!(SaveFile::from_bytes(&vec![0; 0x65600]), Err(SaveError::MissingBeef)));
}

#[test]
fn gen6_unexpected_structure_warns() {
    // Un seul petit bloc : aucune zone utile n'est couverte.
    let (bytes, _) = gen6::blank(0x65600, &[0x100]);
    let save = SaveFile::from_bytes(&bytes).unwrap();
    assert!(save.warnings().len() >= 5);
}

#[test]
fn gen7_sun_moon() {
    let (bytes, _) = gen6::blank(0x6BE00, &gen7::synthetic_lengths(SaveVersion::SunMoon));
    let save = roundtrip(bytes, SaveVersion::SunMoon);
    assert_eq!(save.box_count(), 32);
    // La signature MemeCrypto est recalculée : plus besoin de PKHeX.
    assert!(!save.needs_resign());
    assert!(save.warnings().is_empty(), "{:?}", save.warnings());
}

#[test]
fn gen7_ultra_sun_moon() {
    let (bytes, _) = gen6::blank(0x6CC00, &gen7::synthetic_lengths(SaveVersion::UltraSunUltraMoon));
    let save = roundtrip(bytes, SaveVersion::UltraSunUltraMoon);
    assert!(save.warnings().is_empty(), "{:?}", save.warnings());
}

#[test]
fn ctr_checksum_algorithms_differ() {
    // Même contenu : le CRC Gen 6 et le CRC Gen 7 ne coïncident pas.
    let lengths = gen7::synthetic_lengths(SaveVersion::SunMoon);
    let (bytes, offsets) = gen6::blank(0x6BE00, &lengths);
    let out = SaveFile::from_bytes(&bytes).unwrap().to_bytes();
    let info = gen6::table_offset(out.len()).unwrap();
    let block = &out[offsets[4]..offsets[4] + lengths[4]];
    let stored = u16::from_le_bytes([out[info + 0x14 + 8 * 4 + 6], out[info + 0x14 + 8 * 4 + 7]]);
    assert_eq!(stored, checksum::crc16_invert(block));
    assert_ne!(stored, checksum::crc16_ccitt(block));
}

#[test]
fn unrecognized_file() {
    assert!(matches!(SaveFile::from_bytes(&[0; 1234]), Err(SaveError::Unrecognized)));
}

#[test]
fn summary_serializes_camel_case() {
    let pk = sample(PkmFormat::Gen6, 42);
    let json = serde_json::to_value(pk.summary(None)).unwrap();
    assert_eq!(json["natureName"], "Rigide");
    assert_eq!(json["levelEstimated"], false);
    assert_eq!(json["gender"], "female");
    assert_eq!(json["ivs"], serde_json::json!([31, 30, 29, 28, 27, 26]));
    let trainer = Trainer {
        name: "A".into(),
        origin: None,
        tid: 1,
        sid: 2,
        display_id: 1,
        gender: Gender::Male,
        money: 0,
        play_time: PlayTime { hours: 1, minutes: 2, seconds: 3 },
    };
    let json = serde_json::to_value(&trainer).unwrap();
    assert_eq!(json["playTime"]["hours"], 1);
    assert_eq!(json["displayId"], 1);
    assert_eq!(serde_json::to_value(SaveVersion::Black2White2).unwrap(), "black2_white2");
}
