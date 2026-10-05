//! Tests sur de vrais fichiers tirés des tests de PKHeX (kwsch/PKHeX, GPLv3) :
//! Pokémon `.pk4` à `.pk7` (dossier `Tests/PKHeX.Core.Tests/Legality/Legal`, stockés
//! déchiffrés) et la sauvegarde Soleil/Lune `SM Project 802.main`
//! (`Tests/PKHeX.Core.Tests/TestData`). Copies dans `crates/core/tests/data/pkhex/`.

use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/", $name))
    };
}

/// Valeurs attendues (lues dans PKHeX / déduites du nom du fichier d'origine).
struct Expected {
    file: &'static str,
    format: PkmFormat,
    bytes: &'static [u8],
    species: u16,
    form: u8,
    nickname: &'static str,
    ot: &'static str,
    moves: [u16; 4],
    is_egg: bool,
    shiny: bool,
}

#[allow(clippy::too_many_arguments)]
const fn e(
    file: &'static str,
    format: PkmFormat,
    bytes: &'static [u8],
    species: u16,
    form: u8,
    nickname: &'static str,
    ot: &'static str,
    moves: [u16; 4],
    is_egg: bool,
    shiny: bool,
) -> Expected {
    Expected { file, format, bytes, species, form, nickname, ot, moves, is_egg, shiny }
}

use PkmFormat::{Gen4, Gen5, Gen6, Gen7};

#[rustfmt::skip]
const PKM_FILES: [Expected; 16] = [
    // « 201 - UNOWN - E8FFA68025F9.pk4 »
    e("pk4_201_unown", Gen4, fixture!("pk4_201_unown.pk4"), 201, 0, "UNOWN", "MATT", [237, 0, 0, 0], false, false),
    // « 0218 - Egg - 30859D41D013 hg egg.pk4 »
    e("pk4_218_egg_hg_party", Gen4, fixture!("pk4_218_egg_hg_party.pk4"), 218, 0, "Egg", "N", [281, 123, 0, 0], true, false),
    // « 236 - Oeuf - 5E05C020CB44.pk4 »
    e("pk4_236_oeuf_party", Gen4, fixture!("pk4_236_oeuf_party.pk4"), 236, 0, "Oeuf", "Yvonne", [33, 270, 252, 193], true, false),
    // « 0407 - ROSERADE - E24358625928.pk4 »
    e("pk4_407_roserade_party", Gen4, fixture!("pk4_407_roserade_party.pk4"), 407, 0, "ROSERADE", "TEST", [241, 202, 173, 345], false, false),
    // « 470  - LEAFEON - 65B05FEF8D69.pk4 » (chromatique : TID ^ SID ^ PID = 7, sans ★ dans le nom)
    e("pk4_470_leafeon", Gen4, fixture!("pk4_470_leafeon.pk4"), 470, 0, "LEAFEON", "VGC10", [231, 98, 175, 376], false, true),
    // « 546 - Jersey - C31DDBEA71D8 french accented OT name.pk5 »
    e("pk5_546_jersey", Gen5, fixture!("pk5_546_jersey.pk5"), 546, 0, "Jersey", "Jôsuke", [78, 72, 178, 75], false, false),
    // « 0550-01 - Basculin - C46358B4FA76 wrongAbility.pk5 »
    e("pk5_550_basculin_party", Gen5, fixture!("pk5_550_basculin_party.pk5"), 550, 1, "Basculin", "Hilda", [498, 36, 242, 401], false, false),
    // « 610 - キバゴ - 6F91B110EED1.pk5 »
    e("pk5_610_axew", Gen5, fixture!("pk5_610_axew.pk5"), 610, 0, "キバゴ", "ＲｏＣ", [184, 163, 206, 337], false, false),
    // « 612 ★ - Haxorus - A1AC5BB800F7.pk5 »
    e("pk5_612_haxorus_party", Gen5, fixture!("pk5_612_haxorus_party.pk5"), 612, 0, "Haxorus", "Lego", [269, 406, 14, 12], false, true),
    // « 035 - Clefairy - 2A2C8A94FA80 dexnav.pk6 »
    e("pk6_035_clefairy_party", Gen6, fixture!("pk6_035_clefairy_party.pk6"), 35, 0, "Clefairy", "Matt", [187, 322, 381, 34], false, false),
    // « 276 ★ - Nirondelle - 0F8089C9AA57 flute level1.pk6 »
    e("pk6_276_nirondelle_party", Gen6, fixture!("pk6_276_nirondelle_party.pk6"), 276, 0, "Nirondelle", "Ilann", [64, 45, 0, 0], false, true),
    // « 645-01 - Landorus - 57BA472C4EB8.pk6 »
    e("pk6_645_landorus", Gen6, fixture!("pk6_645_landorus.pk6"), 645, 1, "Landorus", "Rosa", [89, 444, 369, 446], false, false),
    // « 100 - VOLTI - B086FE1296FC FALCçN.pk7 »
    e("pk7_100_volti_party", Gen7, fixture!("pk7_100_volti_party.pk7"), 100, 0, "VOLTI", "FALCçN", [33, 0, 0, 0], false, false),
    // « 151 - ミュウ - 6B9DADB23EB0.pk7 »
    e("pk7_151_mew", Gen7, fixture!("pk7_151_mew.pk7"), 151, 0, "ミュウ", "ゲーフリ", [1, 0, 0, 0], false, false),
    // « 666-07 - Vivillon - 6560815D4F75.pk7 »
    e("pk7_666_vivillon", Gen7, fixture!("pk7_666_vivillon.pk7"), 666, 7, "Vivillon", "RoC", [476, 16, 78, 33], false, false),
    // « 745 - Lycanroc - F73885FF60E2.pk7 »
    e("pk7_745_lycanroc_party", Gen7, fixture!("pk7_745_lycanroc_party.pk7"), 745, 0, "Lycanroc", "PKHeX", [33, 43, 28, 44], false, false),
];

#[test]
fn real_pkm_files_decode() {
    for x in &PKM_FILES {
        let pk = Pokemon::from_bytes(x.format, x.bytes).unwrap();
        let name = x.file;
        assert!(pk.checksum_valid(), "{name} : somme de contrôle");
        assert_eq!(pk.species(), x.species, "{name}");
        assert_eq!(pk.form(), x.form, "{name}");
        assert_eq!(pk.nickname(), x.nickname, "{name}");
        assert_eq!(pk.ot_name(), x.ot, "{name}");
        assert_eq!(pk.moves(), x.moves, "{name}");
        assert_eq!(pk.is_egg(), x.is_egg, "{name}");
        assert_eq!(pk.is_shiny(), x.shiny, "{name}");
        // Champs plausibles : PP non nuls pour chaque attaque, IV ≤ 31, niveau de rencontre ≤ 100.
        for (m, pp) in pk.moves().iter().zip(pk.pp()) {
            assert_eq!(*m != 0, pp != 0, "{name} : PP");
        }
        assert!(pk.ivs().iter().all(|&iv| iv <= 31), "{name}");
        assert!(pk.met_level() <= 100, "{name}");
        assert!((1..=11).contains(&pk.language()), "{name} : langue {}", pk.language());
        assert!(pk.ball() >= 1 && pk.ball() <= 26, "{name} : Ball {}", pk.ball());
    }
}

#[test]
fn real_pkm_files_specific_fields() {
    let get = |file: &str| {
        let x = PKM_FILES.iter().find(|x| x.file == file).unwrap();
        Pokemon::from_bytes(x.format, x.bytes).unwrap()
    };
    // Gen 4 : Cherish Ball, lieu d'événement (3060), version HG (7).
    let leafeon = get("pk4_470_leafeon");
    assert_eq!((leafeon.ball(), leafeon.met_location(), leafeon.version(), leafeon.met_level()), (16, 3060, 7, 50));
    // Œuf de Platine (version 12), français (3) : lieu d'éclosion « Œuf de pension » (2000),
    // rencontre 2002, lus dans les champs Pt/HGSS.
    let egg = get("pk4_236_oeuf_party");
    assert_eq!((egg.version(), egg.language(), egg.egg_location(), egg.met_location()), (12, 3, 2000, 2002));
    // Gen 5 : forme, talent n° 1 pour un Pokémon né en Noire 2/Blanche 2.
    let jersey = get("pk5_546_jersey");
    assert_eq!((jersey.version(), jersey.language(), jersey.ability_number()), (22, 3, 1));
    // Gen 6 : IV parfaits, EV (ordre Kaleido : PV, Att, Déf, Atq Spé, Déf Spé, Vit).
    let landorus = get("pk6_645_landorus");
    assert_eq!(landorus.ivs(), [31; 6]);
    assert_eq!(landorus.evs(), [1, 252, 12, 1, 0, 244]);
    assert_eq!(landorus.ball(), 25);
    // Gen 7 : statistiques d'équipe présentes.
    let lycanroc = get("pk7_745_lycanroc_party");
    assert!(lycanroc.party_stats().is_some());
    assert_eq!((lycanroc.party_level(), lycanroc.met_level()), (Some(100), 11));
}

/// Les fichiers sont stockés déchiffrés : chiffrer puis déchiffrer redonne les mêmes octets.
#[test]
fn real_pkm_files_crypt_roundtrip() {
    for x in &PKM_FILES {
        let name = x.file;
        let len = x.bytes.len();
        let pk = Pokemon::from_bytes(x.format, x.bytes).unwrap();
        assert_eq!(&pk.data()[..len], x.bytes, "{name} : lecture");
        let enc = if len == x.format.party_size() { pk.encrypt_party() } else { pk.encrypt_stored() };
        assert_eq!(enc.len(), len, "{name}");
        assert_ne!(&enc[8..], &x.bytes[8..], "{name} : non chiffré");
        let dec = Pokemon::from_encrypted(x.format, &enc).unwrap();
        assert_eq!(&dec.data()[..len], x.bytes, "{name} : aller-retour");
        // Détection automatique du chiffrement (comme PKHeX).
        assert_eq!(&Pokemon::from_bytes(x.format, &enc).unwrap().data()[..len], x.bytes, "{name} : détection");
    }
}

fn sm_save() -> SaveFile {
    SaveFile::from_bytes(fixture!("sm_project_802.main")).unwrap()
}

/// Sauvegarde Soleil/Lune réelle : CRC (bloc 36 compris) et signature MemeCrypto valides,
/// et réécriture sans modification octet pour octet identique.
#[test]
fn real_sm_save_checksums_and_signature() {
    let raw = fixture!("sm_project_802.main");
    let save = sm_save();
    assert_eq!(save.version(), SaveVersion::SunMoon);
    assert!(save.warnings().is_empty(), "{:?}", save.warnings());
    let checks = save.checksums();
    assert_eq!(checks.len(), 37 + 1, "37 blocs + la signature");
    let bad: Vec<_> = checks.iter().filter(|c| !c.valid).collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(save.to_bytes(), raw.to_vec());
}

#[test]
fn real_sm_save_contents() {
    let save = sm_save();
    let t = save.trainer();
    assert_eq!(t.name, "PPorg");
    assert_eq!((t.tid, t.sid, t.display_id), (36345, 24477, 161017));
    assert_eq!(t.gender, Gender::Male);
    assert_eq!(t.money, 158_165);
    assert_eq!(t.play_time, PlayTime { hours: 27, minutes: 35, seconds: 42 });

    let party = save.party().unwrap();
    assert_eq!(party.len(), 1);
    assert_eq!(party[0].species(), 801);
    assert_eq!(party[0].party_level(), Some(50));
    assert!(party[0].checksum_valid());

    assert_eq!(save.box_count(), 32);
    assert_eq!(save.box_name(0).unwrap(), "Box 1");
    let first = save.box_slot(0, 0).unwrap().unwrap();
    assert_eq!((first.species(), first.nickname().as_str(), first.ot_name().as_str()), (1, "Bulbasaur", "Jyan"));
    let mut count = 0;
    for b in 0..save.box_count() {
        for s in 0..BOX_SLOTS {
            let at = save.box_offset(b, s).unwrap();
            let raw = &save.data[at..at + 232];
            if let Some(pk) = save.box_slot(b, s).unwrap() {
                count += 1;
                assert!(pk.checksum_valid(), "boîte {b} emplacement {s}");
                assert!((1..=802).contains(&pk.species()), "boîte {b} emplacement {s}");
                // Rechiffrer redonne exactement les octets de la sauvegarde.
                assert_eq!(pk.encrypt_stored(), raw, "boîte {b} emplacement {s}");
            }
        }
    }
    assert_eq!(count, 841);
}

#[test]
fn real_sm_save_pokedex() {
    let save = sm_save();
    assert_eq!(save.dex_max_species(), 802);
    let dex = save.pokedex().unwrap();
    assert_eq!(dex.iter().filter(|e| e.caught).count(), 802);
    assert_eq!(dex.iter().filter(|e| e.seen).count(), 796);
}

/// Modifier la sauvegarde réelle : les CRC et la signature restent valides à la relecture.
#[test]
fn real_sm_save_edit_and_resign() {
    let mut save = sm_save();
    let mut mew = save.box_slot(27, 12).unwrap().unwrap();
    assert_eq!(mew.species(), 151);
    mew.set_nickname("Kaleido").unwrap();
    mew.refresh_checksum();
    save.set_box_slot(31, 29, Some(mew)).unwrap();
    save.set_dex_entry(25, false, false).unwrap();
    let out = save.to_bytes();
    assert_ne!(out, fixture!("sm_project_802.main").to_vec());
    let re = SaveFile::from_bytes(&out).unwrap();
    let bad: Vec<_> = re.checksums().into_iter().filter(|c| !c.valid).collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert_eq!(re.box_slot(31, 29).unwrap().unwrap().nickname(), "Kaleido");
    assert!(!re.pokedex().unwrap()[24].caught);
    // Données modifiées sans to_bytes : les CRC des blocs touchés ne correspondent plus
    // (la signature, elle, ne couvre que la table des CRC).
    let bad: Vec<_> = save.checksums().into_iter().filter(|c| !c.valid).map(|c| c.name).collect();
    assert_eq!(bad, ["bloc 6", "bloc 14"]);
}
