//! Tests de la base de cadeaux mystère (fichiers `mgdb` de PKHeX) et de leur réception.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::*;
use crate::save::session::{SaveSession, Slot};
use crate::save::{level_from_exp, PkmFormat, SaveFile};

const SM_SAVE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/sm_project_802.main"));

fn rng() -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(0x6B61_6C65)
}

fn trainer(version: SaveVersion, game_version: u8) -> GiftTrainer {
    GiftTrainer {
        name: "Kaleido".into(),
        tid: 12345,
        sid: 54321,
        gender: Gender::Female,
        language: 3,
        version: game_version,
        save_version: version,
        console_region: 2,
        country: 77,
        region: 18,
    }
}

fn trainer_for(generation: u8) -> GiftTrainer {
    match generation {
        4 => trainer(SaveVersion::HeartGoldSoulSilver, 7),
        5 => trainer(SaveVersion::Black2White2, 23),
        6 => trainer(SaveVersion::OmegaRubyAlphaSapphire, 26),
        _ => trainer(SaveVersion::UltraSunUltraMoon, 32),
    }
}

fn find(generation: u8, card_id: u16, species: u16) -> &'static Gift {
    database()
        .iter()
        .find(|g| g.generation() == generation && g.card_id == card_id && g.species() == species)
        .unwrap_or_else(|| panic!("carte Gen {generation} n°{card_id} introuvable"))
}

#[test]
fn database_counts() {
    let db = database();
    let count = |f: GiftFormat| db.iter().filter(|g| g.format == f).count();
    // Toutes les cartes des fichiers de PKHeX sont lues (tailles / taille d'une carte).
    assert_eq!(count(GiftFormat::Pcd), 590);
    assert_eq!(count(GiftFormat::Pgf), 709);
    assert_eq!(count(GiftFormat::Wc6Full), 422);
    assert_eq!(count(GiftFormat::Wc6), 416);
    assert_eq!(count(GiftFormat::Wc7Full), 801);
    assert_eq!(count(GiftFormat::Wc7), 136);
    assert_eq!(db.len(), 590 + 709 + 422 + 416 + 801 + 136);
    let pokemon = db.iter().filter(|g| g.is_pokemon()).count();
    let items = db.iter().filter(|g| g.kind == GiftKind::Item).count();
    assert!(pokemon > 2000, "{pokemon} cartes Pokémon");
    assert!(items > 50, "{items} cartes objet");
    for g in db {
        assert!(!g.games.is_empty(), "carte sans jeu : {}", g.display_title());
        if let Some(p) = &g.pokemon {
            assert!(p.species > 0 && p.species <= 807, "espèce {} pour {}", p.species, g.display_title());
        }
    }
}

#[test]
fn known_cards() {
    // WC6 n°525 : Diancie (novembre 2014).
    let diancie = find(6, 525, 719);
    let p = diancie.pokemon.as_ref().unwrap();
    assert_eq!((p.level, p.ot_name.as_deref(), p.tid), (50, Some("NOV2014"), Some(11064)));
    assert_eq!(p.moves, [591, 585, 115, 216]);
    assert_eq!((p.shiny, p.perfect_ivs), (ShinyRule::Never, 3));
    // WC6 n°557 : Mew niveau 100 de GF.
    let mew = find(6, 557, 151);
    let p = mew.pokemon.as_ref().unwrap();
    assert_eq!((p.level, p.ot_name.as_deref(), p.moves), (100, Some("GF"), [1, 0, 0, 0]));
    assert!(mew.games.contains(&24) && mew.games.contains(&27));
    // WC7 n°272 : Marshadow (« MT. Tensei »).
    let marshadow = find(7, 272, 802);
    let p = marshadow.pokemon.as_ref().unwrap();
    assert_eq!((p.level, p.ot_name.as_deref(), p.tid, p.sid), (50, Some("MT. Tensei"), Some(60981), Some(4151)));
    assert_eq!(p.moves, [712, 370, 395, 247]);
    assert_eq!(marshadow.title, "Mythical Pokémon Marshadow");
    // Gen 4 n°19 : Arceus, Gen 5 n°129 : Meloetta.
    let arceus = find(4, 19, 493);
    assert_eq!(arceus.pokemon.as_ref().unwrap().level, 100);
    assert_eq!(arceus.games, vec![10, 11, 12]);
    let meloetta = find(5, 129, 648);
    assert_eq!(meloetta.games, vec![20, 21, 22, 23]);
    // Titre français et détails.
    let fr = database().iter().find(|g| g.title == "Diancie, le Pokémon fabuleux !").unwrap();
    let d = fr.details(0, Some(SaveVersion::XY));
    assert_eq!(d.compatible, Some(true));
    assert_eq!(d.summary.species_name, Some("Diancie"));
    assert_eq!(d.move_names.len(), 4);
    assert_eq!(d.ivs_label, "Aléatoires, dont 3 à 31");
    let d = fr.details(0, Some(SaveVersion::SunMoon));
    assert_eq!(d.compatible, Some(false));
}

/// Chaque carte Pokémon se convertit pour sa génération et respecte la carte.
#[test]
fn convert_every_pokemon_card() {
    let mut rng = rng();
    for (i, g) in database().iter().enumerate().filter(|(_, g)| g.is_pokemon()) {
        let tr = trainer_for(g.generation());
        let info = g.pokemon.as_ref().unwrap();
        let p = convert(g, &tr, &mut rng).unwrap_or_else(|e| panic!("carte {i} « {} » : {e}", g.display_title()));
        let what = format!("carte {i} {} n°{} « {} »", g.format.label(), g.card_id, g.display_title());
        assert!(p.checksum_valid(), "{what}");
        assert_eq!(p.species(), info.species, "{what}");
        assert_eq!(p.form(), info.form, "{what}");
        assert_eq!(p.is_egg(), info.egg, "{what}");
        if info.moves[0] != 0 {
            assert_eq!(p.moves(), info.moves, "{what}");
        }
        if g.generation() >= 5 && info.level > 0 {
            let growth = dex::personal(tr_game(&tr), info.species, info.form).map(|x| x.growth_rate).unwrap();
            assert_eq!(level_from_exp(growth, p.exp()), info.level, "{what}");
        }
        if !info.egg {
            if let Some(ot) = &info.ot_name {
                assert_eq!(&p.ot_name(), ot, "{what}");
            }
            if let (Some(tid), Some(sid)) = (info.tid, info.sid) {
                assert_eq!((p.tid(), p.sid()), (tid, sid), "{what}");
            }
            match info.shiny {
                ShinyRule::Never => assert!(!p.is_shiny(), "{what} : chromatique interdit"),
                ShinyRule::Always | ShinyRule::AlwaysStar | ShinyRule::AlwaysSquare => assert!(p.is_shiny(), "{what} : chromatique attendu"),
                ShinyRule::Random => {}
            }
        } else if g.generation() >= 5 {
            assert_eq!((p.tid(), p.ot_name().as_str()), (tr.tid, "Kaleido"), "{what}");
        }
        if let Some(pid) = info.pid {
            assert_eq!(p.pid(), pid, "{what}");
        }
        if let Some(n) = info.nature {
            assert_eq!(p.nature(), n, "{what}");
        }
        let ivs = p.ivs();
        for (k, iv) in info.ivs.iter().enumerate() {
            if let Some(v) = iv {
                assert_eq!(ivs[k], *v, "{what} IV {k}");
            }
        }
        assert!(ivs.iter().filter(|&&v| v == 31).count() >= info.perfect_ivs as usize, "{what} : IV parfaits");
        if g.generation() >= 6 {
            assert_eq!(p.ball(), info.ball, "{what}");
            assert!(p.met_date().is_some(), "{what}");
        }
        // Rechiffrement puis relecture identique.
        let back = Pokemon::from_bytes(p.format(), &p.encrypt_stored()).unwrap();
        assert_eq!(back.stored_data(), p.stored_data(), "{what}");
    }
}

fn tr_game(tr: &GiftTrainer) -> dex::Game {
    crate::save::session::game_of(tr.save_version)
}

#[test]
fn shiny_rules_and_perfect_ivs() {
    let mut rng = rng();
    // Carte « toujours chromatique » au PID aléatoire : chromatique pour notre dresseur.
    let always = database().iter().find(|g| g.generation() == 7 && g.pokemon.as_ref().is_some_and(|p| p.shiny == ShinyRule::Always && !p.egg));
    if let Some(g) = always {
        let tr = trainer_for(7);
        for _ in 0..20 {
            assert!(convert(g, &tr, &mut rng).unwrap().is_shiny());
        }
    }
    // Marshadow : 3 IV parfaits garantis, jamais chromatique, sur 50 tirages.
    let marshadow = find(7, 272, 802);
    let tr = trainer_for(7);
    for _ in 0..50 {
        let p = convert(marshadow, &tr, &mut rng).unwrap();
        assert!(!p.is_shiny());
        assert!(p.ivs().iter().filter(|&&v| v == 31).count() >= 3);
        // Dresseur de la carte, le joueur en dresseur actuel.
        assert_eq!(p.ot_name(), "MT. Tensei");
        assert_eq!(p.language(), 3);
        assert_eq!(p.version(), 32);
        assert_eq!(p.met_level(), 50);
        assert!(p.fateful_encounter());
        assert_eq!(p.data()[0x93], 1, "dresseur actuel = le joueur");
    }
}

#[test]
fn file_roundtrip_and_detection() {
    let db = database();
    let mut wrong = Vec::new();
    for g in db {
        // Avec l'extension : relecture identique.
        let again = Gift::from_file(g.file(), Some(g.format.extension())).unwrap();
        assert_eq!(again.card_id, g.card_id);
        assert_eq!(again.species(), g.species());
        // Sans extension : déduite de la taille (WC6 / WC7 départagés par la date ou l'espèce).
        let guessed = Gift::from_file(g.file(), None).unwrap();
        if guessed.format != g.format {
            wrong.push(format!("{:?} → {:?} n°{} {}", g.format, guessed.format, g.card_id, g.display_title()));
        }
    }
    assert!(wrong.is_empty(), "formats mal devinés : {wrong:#?}");
    assert!(matches!(Gift::from_file(&[0; 100], None), Err(GiftError::UnknownFormat(100))));
    assert!(matches!(Gift::from_file(&[0; 100], Some("wc7")), Err(GiftError::BadSize { .. })));
}

#[test]
fn search_filters() {
    let db = database();
    let refs: Vec<&Gift> = db.iter().collect();
    let q = GiftQuery { species: Some(802), ..Default::default() };
    let hits = search(&refs, &q);
    assert!(!hits.is_empty() && hits.iter().all(|&i| refs[i].species() == 802));
    let q = GiftQuery { text: "diancie".into(), generations: vec![6], ..Default::default() };
    assert!(search(&refs, &q).iter().all(|&i| refs[i].species() == 719));
    let q = GiftQuery { shiny: Some(true), ..Default::default() };
    assert!(search(&refs, &q).iter().all(|&i| refs[i].pokemon.as_ref().unwrap().shiny.is_shiny()));
    let q = GiftQuery { egg: Some(true), ..Default::default() };
    assert!(!search(&refs, &q).is_empty());
    let q = GiftQuery { versions: versions_of(SaveVersion::SunMoon).to_vec(), ..Default::default() };
    let sm = search(&refs, &q);
    assert!(sm.iter().all(|&i| refs[i].generation() == 7));
    let q = GiftQuery { kinds: vec![GiftKind::Item], sort: GiftSort::Title, ..Default::default() };
    assert!(search(&refs, &q).iter().all(|&i| refs[i].kind == GiftKind::Item));
    let q = GiftQuery { sort: GiftSort::Species, ..Default::default() };
    let sorted = search(&refs, &q);
    assert_eq!(sorted.len(), db.len());
}

#[test]
fn add_marshadow_to_sun_moon_save() {
    let mut s = SaveSession::open(SM_SAVE).unwrap();
    let marshadow = find(7, 272, 802);
    let out = add_to_save(&mut s, marshadow, None, &mut rng()).unwrap();
    let slot = out.slot.unwrap();
    assert!(matches!(slot, Slot::Box { .. }));
    assert!(out.message.contains("Marshadow"), "{}", out.message);
    // Une seule étape d'historique.
    assert!(s.undo());
    assert!(s.get(slot).unwrap().is_none());
    assert!(s.redo());
    // Relecture du fichier écrit : sommes de contrôle et signature valides, Pokémon intact.
    let bytes = s.to_bytes();
    let reopened = SaveFile::from_bytes(&bytes).unwrap();
    assert!(reopened.checksums_valid());
    let Slot::Box { r#box, index } = slot else { unreachable!() };
    let p = reopened.box_slot(r#box, index).unwrap().unwrap();
    assert!(p.checksum_valid());
    assert_eq!((p.species(), p.ot_name().as_str(), p.tid(), p.sid()), (802, "MT. Tensei", 60981, 4151));
    assert_eq!(p.moves(), [712, 370, 395, 247]);
    assert!(!p.is_shiny());
    // Dresseur actuel = celui de la sauvegarde ; langue de la sauvegarde.
    let tr = s.save.trainer();
    let mut ht = Pokemon::blank(PkmFormat::Gen7);
    ht.set_ot_name(&tr.name).unwrap();
    assert_eq!(&p.data()[0x78..0x92], &ht.data()[0xB0..0xCA]);
    assert_eq!(p.language(), s.save.trainer_origin().language);
}

#[test]
fn add_player_ot_and_item_gifts_to_save() {
    let mut s = SaveSession::open(SM_SAVE).unwrap();
    let tr = s.save.trainer();
    // Carte au dresseur « joueur » (sexe du DO = 3) : ID et nom de la sauvegarde.
    let player = database().iter().find(|g| g.generation() == 7 && g.pokemon.as_ref().is_some_and(|p| p.tid.is_none() && !p.egg)).unwrap();
    let out = add_to_save(&mut s, player, Some(Slot::Box { r#box: 31, index: 29 }), &mut rng()).unwrap();
    let p = s.get(out.slot.unwrap()).unwrap().unwrap();
    assert_eq!((p.tid(), p.sid()), (tr.tid, tr.sid));
    // Objets : ajoutés au sac en une étape.
    let item = find(7, 1605, 0);
    let before = s.inventory().unwrap();
    let count = |pouches: &[crate::save::Pouch], id: u16| pouches.iter().flat_map(|p| &p.items).filter(|i| i.id == id).map(|i| i.count).sum::<u16>();
    let out = add_to_save(&mut s, item, None, &mut rng()).unwrap();
    assert!(out.message.starts_with("Ajouté au sac"), "{}", out.message);
    let after = s.inventory().unwrap();
    for it in &item.items {
        assert!(count(&after, it.id) >= count(&before, it.id).max(1), "objet {}", it.id);
    }
    let reopened = SaveFile::from_bytes(&s.to_bytes()).unwrap();
    assert!(reopened.checksums_valid());
}

#[test]
fn gen4_gift_into_platinum_save() {
    let bytes = crate::save::demo_save().unwrap();
    let mut s = SaveSession::open(&bytes).unwrap();
    let arceus = find(4, 19, 493);
    let out = add_to_save(&mut s, arceus, None, &mut rng()).unwrap();
    let reopened = SaveFile::from_bytes(&s.to_bytes()).unwrap();
    assert!(reopened.checksums_valid());
    let Some(Slot::Box { r#box, index }) = out.slot else { panic!() };
    let p = reopened.box_slot(r#box, index).unwrap().unwrap();
    assert!(p.checksum_valid() && !p.is_shiny());
    assert_eq!(p.species(), 493);
    assert_eq!(p.met_date(), Some(crate::save::session::today()));
    // Œuf de Manaphy (cadeau de Pokémon Ranger) s'il est dans la base.
    if let Some(manaphy) = database().iter().find(|g| g.generation() == 4 && g.kind == GiftKind::Egg && g.species() == 490) {
        let out = add_to_save(&mut s, manaphy, None, &mut rng()).unwrap();
        let p = s.get(out.slot.unwrap()).unwrap().unwrap();
        assert!(p.is_egg() && !p.is_shiny() && p.checksum_valid());
    }
}

#[test]
fn other_generations_are_refused() {
    let mut s = SaveSession::open(SM_SAVE).unwrap();
    let diancie = find(6, 525, 719);
    let err = add_to_save(&mut s, diancie, None, &mut rng()).unwrap_err().to_string();
    assert!(err.contains("même génération"), "{err}");
    let bytes = crate::save::demo_save().unwrap();
    let mut s4 = SaveSession::open(&bytes).unwrap();
    let marshadow = find(7, 272, 802);
    let err = add_to_save(&mut s4, marshadow, None, &mut rng()).unwrap_err().to_string();
    assert!(err.contains("ne peut pas le recevoir"), "{err}");
    // Une carte d'un autre type (Pouvoir Pass, O-Aura…) est refusée avec une explication.
    if let Some(other) = database().iter().find(|g| g.generation() == 7 && g.kind == GiftKind::Other) {
        assert!(add_to_save(&mut s, other, None, &mut rng()).is_err());
    }
}
