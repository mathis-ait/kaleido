//! Tests des transferts entre générations, sur de vrais fichiers des tests de PKHeX
//! (kwsch/PKHeX, GPLv3) copiés dans `crates/core/tests/data/pkhex/`.

use super::*;

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/", $name))
    };
}

fn load(f: PkmFormat, b: &[u8]) -> Pokemon {
    Pokemon::from_bytes(f, b).unwrap()
}

fn trainer() -> TransferTrainer {
    TransferTrainer { name: "Thisma".into(), gender: Gender::Female, tid: 12345, sid: 54321 }
}

/// Chiffré puis relu : identique, somme de contrôle valide.
fn assert_roundtrip(p: &Pokemon) {
    assert!(p.checksum_valid(), "{:?} : somme de contrôle", p.format());
    let back = Pokemon::from_bytes(p.format(), &p.encrypt_stored()).unwrap();
    assert_eq!(back.stored_data(), p.stored_data());
    assert!(back.checksum_valid());
}

/// Ce qui ne doit jamais changer d'une génération à l'autre.
fn assert_identity(a: &Pokemon, b: &Pokemon) {
    assert_eq!(a.species(), b.species());
    assert_eq!(a.form(), b.form());
    assert_eq!((a.tid(), a.sid()), (b.tid(), b.sid()));
    assert_eq!(a.exp(), b.exp());
    assert_eq!(a.ivs(), b.ivs());
    assert_eq!(a.evs().map(|v| v.min(252)), b.evs());
    assert_eq!(a.nature(), b.nature());
    assert_eq!(a.is_shiny(), b.is_shiny(), "chromatique préservé");
    assert_eq!(a.ot_name(), b.ot_name());
    assert_eq!(a.ot_gender(), b.ot_gender());
    assert_eq!(a.gender(), b.gender());
    assert_eq!(a.ball(), b.ball());
    assert_eq!(a.language().max(1), b.language());
    assert_eq!(a.version(), b.version());
    assert_eq!(a.is_nicknamed(), b.is_nicknamed());
}

#[test]
fn gen4_to_gen5_follows_poke_transfer() {
    for bytes in [&fixture!("pk4_407_roserade_party.pk4")[..], fixture!("pk4_201_unown.pk4"), fixture!("pk4_470_leafeon.pk4")] {
        let pk4 = load(PkmFormat::Gen4, bytes);
        let pk5 = pk4_to_pk5(&pk4).unwrap();
        assert_eq!(pk5.format(), PkmFormat::Gen5);
        assert_roundtrip(&pk5);
        assert_identity(&pk4, &pk5);
        assert_eq!(pk5.pid(), pk4.pid(), "le PID ne change pas");
        assert_eq!(pk5.nickname(), pk4.nickname());
        assert_eq!(pk5.met_location(), TRANSFER4);
        assert_eq!(pk5.met_level(), current_level(Game::HGSS, &pk4));
        assert_eq!(pk5.friendship(), 70);
        assert_eq!(pk5.met_date(), Some(today()));
        // Lieux Pt / HGSS et Ball HGSS effacés.
        assert!(pk5.data()[0x44..0x48].iter().all(|&b| b == 0));
        assert_eq!(pk5.data()[0x86], 0);
        // Talent : même emplacement (bit 0 du PID pour un Pokémon venu de la Gen 4).
        assert_eq!(pk5.ability_number(), pk4.ability_number());
        assert_eq!(pk5.ability(), pk4.ability());
        // PP au maximum.
        let moves = pk5.moves();
        for (i, pp) in pk5.pp().into_iter().enumerate() {
            if moves[i] != 0 {
                assert!(pp > 0);
            }
        }
    }
}

#[test]
fn gen4_hms_are_forgotten() {
    let mut pk4 = load(PkmFormat::Gen4, fixture!("pk4_407_roserade_party.pk4"));
    pk4.set_moves([SURF, 202, ROCK_SMASH, WHIRLPOOL]);
    pk4.refresh_checksum();
    let pk5 = pk4_to_pk5(&pk4).unwrap();
    // Sans Anti-Brume : liste DPPt (Siphon gardé, comme PKHeX), trous resserrés.
    assert_eq!(pk5.moves(), [202, WHIRLPOOL, 0, 0]);
    assert_eq!(pk5.pp_ups()[2..], [0, 0]);
    pk4.set_moves([DEFOG, WHIRLPOOL, CUT, 202]);
    pk4.refresh_checksum();
    assert_eq!(pk4_to_pk5(&pk4).unwrap().moves(), [DEFOG, 202, 0, 0]);
}

#[test]
fn gen4_event_beasts_get_their_location() {
    let mut pk4 = load(PkmFormat::Gen4, fixture!("pk4_470_leafeon.pk4"));
    pk4.set_species(SUICUNE);
    assert!(pk4.fateful_encounter());
    assert_eq!(transfer_met_location4(&pk4), TRANSFER4_CROWN_UNUSED);
    pk4.set_species(CELEBI);
    assert_eq!(transfer_met_location4(&pk4), TRANSFER4_CELEBI_UNUSED);
    pk4.set_fateful_encounter(false);
    assert_eq!(transfer_met_location4(&pk4), TRANSFER4);
}

#[test]
fn gen4_arceus_loses_its_plate() {
    let mut pk4 = load(PkmFormat::Gen4, fixture!("pk4_470_leafeon.pk4"));
    pk4.set_species(ARCEUS);
    pk4.set_form(5).unwrap();
    pk4.set_held_item(298);
    let pk5 = pk4_to_pk5(&pk4).unwrap();
    assert_eq!((pk5.form(), pk5.held_item()), (0, 0));
}

#[test]
fn gen5_to_gen6_follows_poke_transporter() {
    let t = trainer();
    for bytes in [&fixture!("pk5_612_haxorus_party.pk5")[..], fixture!("pk5_546_jersey.pk5"), fixture!("pk5_610_axew.pk5"), fixture!("pk5_550_basculin_party.pk5")] {
        let pk5 = load(PkmFormat::Gen5, bytes);
        let pk6 = pk5_to_pk6(&pk5, Some(&t)).unwrap();
        assert_roundtrip(&pk6);
        assert_identity(&pk5, &pk6);
        // EC = ancien PID, PID gardé sauf correction du chromatique.
        assert_eq!(pk6.encryption_constant(), pk5.pid());
        assert_eq!(pk6.pid(), transfer_pid(pk5.pid(), pk5.tid(), pk5.sid()));
        assert_eq!(pk6.moves(), pk5.moves());
        assert_eq!(pk6.met_location(), pk5.met_location());
        assert_eq!(pk6.met_level(), pk5.met_level());
        assert_eq!(pk6.met_date(), pk5.met_date());
        assert_eq!(pk6.held_item(), 0, "objet non transféré");
        assert_eq!(pk6.markings(), pk5.markings());
        // Dresseur actuel : celui qui transfère, souvenir d'échange, bonheur de base.
        assert_eq!(handler(&pk6).as_deref(), Some("Thisma"));
        let d = pk6.data();
        assert_eq!((d[0xA4], d[0xA5]), (1, 4));
        assert!(matches!(d[0xA6], 6 | 7));
        let base = dex::personal(Game::ORAS, pk6.species(), pk6.form()).unwrap().base_friendship;
        assert_eq!((d[0xCA], d[0xA2]), (base, base));
        // Talent de la Gen 6 au même emplacement.
        let info = dex::personal(Game::ORAS, pk6.species(), pk6.form()).unwrap();
        let idx = match pk6.ability_number() {
            1 => 0,
            2 => 1,
            4 => 2,
            n => panic!("emplacement {n}"),
        };
        assert_eq!(pk6.ability(), info.abilities[idx]);
    }
}

#[test]
fn gen5_names_are_kept_or_retitled() {
    let jersey = pk5_to_pk6(&load(PkmFormat::Gen5, fixture!("pk5_546_jersey.pk5")), None).unwrap();
    assert_eq!(jersey.nickname(), "Jersey");
    assert_eq!(jersey.ot_name(), "Jôsuke");
    let axew = pk5_to_pk6(&load(PkmFormat::Gen5, fixture!("pk5_610_axew.pk5")), None).unwrap();
    assert_eq!(axew.nickname(), "キバゴ");
    assert_eq!(axew.ot_name(), "ＲｏＣ");
    // Sans dresseur qui transfère : le dresseur d'origine devient aussi dresseur actuel.
    assert_eq!(handler(&axew).as_deref(), Some("ＲｏＣ"));
    // Pokémon anglais sans surnom venu de la Gen 4 : « ROSERADE » → « Roserade ».
    let pk4 = load(PkmFormat::Gen4, fixture!("pk4_407_roserade_party.pk4"));
    let pk6 = pk5_to_pk6(&pk4_to_pk5(&pk4).unwrap(), None).unwrap();
    assert_eq!(pk6.nickname(), "Roserade");
    assert!(!pk6.is_nicknamed());
    // Pokémon français : nom français de l'espèce.
    let mut fr = load(PkmFormat::Gen5, fixture!("pk5_612_haxorus_party.pk5"));
    fr.set_language(3);
    assert_eq!(pk5_to_pk6(&fr, None).unwrap().nickname(), "Tranchodon");
}

#[test]
fn transfer_pid_prevents_fake_shinies() {
    // XOR sur 16 bits = 8 : non chromatique en Gen 5 (seuil 8), le serait en Gen 6 (seuil 16).
    let (tid, sid) = (0, 0);
    let pid = 0x0000_0008;
    let flipped = transfer_pid(pid, tid, sid);
    assert_eq!(flipped, 0x8000_0008);
    let mut pk5 = load(PkmFormat::Gen5, fixture!("pk5_610_axew.pk5"));
    pk5.set_tid(tid);
    pk5.set_sid(sid);
    pk5.set_pid(pid);
    pk5.refresh_checksum();
    assert!(!pk5.is_shiny());
    let pk6 = pk5_to_pk6(&pk5, None).unwrap();
    assert!(!pk6.is_shiny());
    assert_eq!(pk6.encryption_constant(), pid);
    // Un vrai chromatique reste chromatique, PID inchangé.
    let hax = load(PkmFormat::Gen5, fixture!("pk5_612_haxorus_party.pk5"));
    assert!(hax.is_shiny());
    let hax6 = pk5_to_pk6(&hax, None).unwrap();
    assert!(hax6.is_shiny());
    assert_eq!(hax6.pid(), hax.pid());
}

#[test]
fn gen5_ribbons_move_to_their_gen6_place() {
    let mut pk5 = load(PkmFormat::Gen5, fixture!("pk5_610_axew.pk5"));
    let mut d = pk5.data().to_vec();
    d[0x24] = 0b1000_0011; // Maître Sinnoh, Capacité, Alerte
    d[0x3F] = 0b1000_0001; // Effort, Monde
    d[0x60] = 0b0000_0111; // 3 rubans de concours Sinnoh
    d[0x3E] = 0b0110_0000; // Victoire, Gagnant
    pk5 = Pokemon::from_decrypted(PkmFormat::Gen5, &d).unwrap();
    let pk6 = pk5_to_pk6(&pk5, None).unwrap();
    let e = pk6.data();
    assert!(bit(e, 0x30, 2) && bit(e, 0x30, 7) && bit(e, 0x31, 0) && bit(e, 0x33, 1));
    assert_eq!(e[0x38], 3, "rubans de concours comptés");
    assert_eq!(e[0x39], 3, "Capacité + Victoire + Gagnant");
    assert!(bit(e, 0x34, 5) && bit(e, 0x34, 6));
}

#[test]
fn gen6_to_gen7_follows_bank() {
    for bytes in [&fixture!("pk6_035_clefairy_party.pk6")[..], fixture!("pk6_276_nirondelle_party.pk6"), fixture!("pk6_645_landorus.pk6")] {
        let pk6 = load(PkmFormat::Gen6, bytes);
        let pk7 = pk6_to_pk7(&pk6).unwrap();
        assert_roundtrip(&pk7);
        assert_identity(&pk6, &pk7);
        assert_eq!(pk7.encryption_constant(), pk6.encryption_constant());
        assert_eq!(pk7.pid(), pk6.pid());
        assert_eq!(pk7.nickname(), pk6.nickname());
        assert_eq!(pk7.moves(), pk6.moves());
        assert_eq!(pk7.held_item(), pk6.held_item());
        assert_eq!(pk7.met_location(), pk6.met_location());
        assert_eq!(pk7.ability_number(), pk6.ability_number());
        assert_eq!(pk7.markings(), pk6.markings().map(|m| (m != 0) as u8));
        let d = pk7.data();
        assert!(d[0x94..0x9E].iter().all(|&b| b == 0), "souvenirs de voyage effacés");
        assert_eq!((d[0xA4], d[0xA5], d[0xA8]), (1, 4, 0));
        assert_eq!(d[0xDE], 0);
    }
}

#[test]
fn chain_gen4_to_gen7() {
    let t = trainer();
    for bytes in [&fixture!("pk4_407_roserade_party.pk4")[..], fixture!("pk4_470_leafeon.pk4"), fixture!("pk4_201_unown.pk4")] {
        let pk4 = load(PkmFormat::Gen4, bytes);
        let pk7 = convert_for(&pk4, Game::SM, Some(&t), false).unwrap();
        assert_eq!(pk7.format(), PkmFormat::Gen7);
        assert_roundtrip(&pk7);
        assert_eq!(pk7.species(), pk4.species());
        assert_eq!(pk7.ivs(), pk4.ivs());
        assert_eq!(pk7.evs(), pk4.evs().map(|v| v.min(252)));
        assert_eq!(pk7.nature(), pk4.nature());
        assert_eq!(pk7.is_shiny(), pk4.is_shiny());
        assert_eq!(pk7.ot_name(), pk4.ot_name());
        assert_eq!(pk7.met_location(), TRANSFER4);
        assert_eq!(pk7.encryption_constant(), pk4.pid());
        assert_eq!(pk7.party_level(), None, "section équipe vidée");
        // Dresseur actuel : celui de la sauvegarde.
        assert_eq!(handler(&pk7).as_deref(), Some("Thisma"));
    }
}

#[test]
fn downgrade_and_eggs_are_refused() {
    let pk7 = load(PkmFormat::Gen7, fixture!("pk7_151_mew.pk7"));
    let err = convert_for(&pk7, Game::HGSS, None, false).unwrap_err();
    assert!(matches!(err, ConvertError::Downgrade { .. }));
    assert!(err.to_string().contains("reste dans la banque"));
    let c = compatibility(&pk7, Game::Pt);
    assert!(!c.ok && c.blocker.is_some());

    let egg = load(PkmFormat::Gen4, fixture!("pk4_236_oeuf_party.pk4"));
    assert!(egg.is_egg());
    assert!(matches!(convert_for(&egg, Game::BW, None, false), Err(ConvertError::Egg)));
    // Même génération : un œuf peut aller dans un autre jeu de la Gen 4.
    assert!(convert_for(&egg, Game::Pt, None, false).is_ok());
}

#[test]
fn missing_moves_items_and_species() {
    // Attaque de ROSA (Draco Ascension, n°620) : absente de X / Y, retirable.
    let mut pk6 = load(PkmFormat::Gen6, fixture!("pk6_645_landorus.pk6"));
    pk6.set_moves([89, 620, 0, 0]);
    pk6.refresh_checksum();
    let c = compatibility(&pk6, Game::XY);
    assert!(!c.ok && c.fixable && !c.converts);
    assert_eq!(c.problems[0].kind, "move");
    assert!(matches!(convert_for(&pk6, Game::XY, None, false), Err(ConvertError::Incompatible(_))));
    let fixed = convert_for(&pk6, Game::XY, None, true).unwrap();
    assert_eq!(fixed.moves(), [89, 0, 0, 0]);
    assert!(compatibility(&pk6, Game::ORAS).ok);

    // Zeraora (n°807) n'existe pas dans Soleil / Lune.
    let mut pk7 = load(PkmFormat::Gen7, fixture!("pk7_151_mew.pk7"));
    pk7.set_species(807);
    pk7.refresh_checksum();
    let c = compatibility(&pk7, Game::SM);
    assert!(!c.ok && !c.fixable);
    assert_eq!(c.problems[0].kind, "species");
    assert!(compatibility(&pk7, Game::USUM).ok);
}

#[test]
fn compatibility_describes_the_trip() {
    let pk4 = load(PkmFormat::Gen4, fixture!("pk4_407_roserade_party.pk4"));
    let same = compatibility(&pk4, Game::Pt);
    assert!(same.ok && !same.converts && same.changes.is_empty());
    let far = compatibility(&pk4, Game::USUM);
    assert!(far.ok && far.converts);
    assert_eq!((far.from, far.to), (PkmFormat::Gen4, PkmFormat::Gen7));
    assert_eq!(far.changes.len(), 4);
}

#[test]
fn handler_follows_the_receiving_trainer() {
    let pk7 = load(PkmFormat::Gen7, fixture!("pk7_745_lycanroc_party.pk7"));
    let ot = TransferTrainer { name: pk7.ot_name(), gender: pk7.ot_gender(), tid: pk7.tid(), sid: pk7.sid() };
    // Retour chez le dresseur d'origine.
    let home = update_handler(&pk7, &ot).unwrap();
    assert_eq!(handler(&home), None);
    assert!(home.checksum_valid());
    // Chez quelqu'un d'autre : nouveau dresseur actuel, bonheur de base.
    let away = update_handler(&pk7, &trainer()).unwrap();
    assert_eq!(handler(&away).as_deref(), Some("Thisma"));
    assert_eq!(away.data()[0x92], 1);
    assert_eq!(away.friendship(), dex::personal(Game::USUM, 745, 0).unwrap().base_friendship);
    assert!(away.checksum_valid());
}
