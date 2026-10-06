//! Noire 2 / Blanche 2 sur une vraie ROM (Noire 2 française, DSi Enhanced) : lecture de
//! chaque format pris en charge, puis randomisation complète, reconstruction de la ROM
//! et relecture de tout ce qui a été écrit.
//!
//! Dossier des ROMs : variable `KALEIDO_ROMS`, sinon `~/Documents/NDS & 3DS`. Les tests
//! sont ignorés (avec un message) si la ROM est absente.

use std::path::PathBuf;

use kaleido_core::battle::trainers::{Role, RomTrainers};
use kaleido_core::data::{field_items, machines, shiny, shops, starters, trainers, DataPaths};
use kaleido_core::games::Game;
use kaleido_core::nuzlocke::rom as nuz;
use kaleido_core::randomizer::{self, statics, Settings};
use kaleido_core::{romedit, GameRom};
use kaleido_formats::nds::NdsRom;

const BLACK2: &str = "Pokemon - Version Noire 2 (France) (NDSi Enhanced).nds";

fn open(name: &str) -> Option<GameRom> {
    let dir = std::env::var_os("KALEIDO_ROMS").map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        PathBuf::from(home).join("Documents").join("NDS & 3DS")
    });
    let path = dir.join(name);
    if !path.exists() {
        eprintln!("ROM absente, test ignoré : {}", path.display());
        return None;
    }
    Some(GameRom::open(&path).unwrap())
}

/// Tout ce qui est lu dans la ROM d'origine, octet par octet comparé au jeu.
#[test]
fn black2_formats() {
    let Some(game) = open(BLACK2) else { return };
    assert_eq!(game.game, Game::Black2);
    assert!(game.layout.verified);
    let paths = DataPaths::for_game(game.game).unwrap();

    // Pokédex : noms, talents, fiches.
    let species = game.species().unwrap();
    assert_eq!(species.len(), 649);
    assert_eq!(species[494].name, "Vipélierre");
    assert_eq!(species[0].abilities, vec!["Engrais".to_string()]);
    assert_eq!(game.text_file(paths.move_names).unwrap()[1], "Écras'Face");

    // Starters (script 854) : Vipélierre, Gruikui, Moustillon.
    assert_eq!(starters::read(&game, paths.starters).unwrap(), [495, 498, 501]);

    // Rencontres : Route 19 (Ratentif, Chacripan niv. 2-4) et Route 20.
    let info = nuz::read(&game).unwrap();
    let route = |name: &str| info.routes.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("{name} absente"));
    let r19 = route("Route 19");
    assert_eq!(r19.location_ids, vec![124]);
    assert!(r19.encounters.iter().any(|e| e.species == 504 && e.min_level == 2 && e.max_level == 4));
    assert!(r19.encounters.iter().any(|e| e.species == 509));
    assert!(route("Route 20").encounters.iter().any(|e| e.species == 519)); // Poichigeon
    assert_eq!(info.routes[0].name, "Pavonnay");
    assert_eq!(info.routes[1].name, "Route 19");
    // Noms de la ROM = liste de lieux de PKHeX (mêmes identifiants).
    let names = game.text_file(109).unwrap();
    for r in &info.routes {
        for &id in &r.location_ids {
            assert_eq!(names[id as usize], r.name);
            assert!(kaleido_core::dex::location_name(5, id).unwrap_or_default().starts_with(r.name.as_str()), "lieu n°{id}");
        }
    }
    // Champions, Conseil 4, Maître.
    let levels: Vec<u8> = info.leaders.iter().map(|l| l.ace_level).collect();
    assert_eq!(levels, vec![13, 18, 24, 30, 33, 39, 48, 51, 58, 58, 58, 58, 59]);
    assert!(info.leaders.iter().all(|l| l.verified), "classes de dresseurs inattendues");
    assert_eq!(info.starters, vec![495, 498, 501]);
    assert_eq!(info.family_of(497), 495);

    // Dresseurs : Tcheren (champion n°1) et le rival.
    let trdata = game.narc(paths.trainer_data).unwrap();
    let trpoke = game.narc(paths.trainer_pokemon).unwrap();
    let cheren = trainers::read_team(5, &trdata.files[156], &trpoke.files[156]).unwrap();
    let team: Vec<(u16, u16)> = cheren.pokemon.iter().map(|p| (p.species, p.level)).collect();
    assert_eq!(team, [(504, 11), (506, 13)]);
    let battle = RomTrainers::from_nds(&game).unwrap();
    let t = battle.trainer(156).unwrap();
    assert_eq!((t.name.as_str(), t.role), ("Tcheren", Some(Role::Gym)));
    assert_eq!(battle.trainer(341).unwrap().role, Some(Role::Champion)); // Iris
    assert_eq!(battle.trainer(161).unwrap().role, Some(Role::Rival));
    assert!(battle.team(156).unwrap().iter().all(|c| c.moves.iter().any(|&m| m != 0)));

    // Objets ramassables (scripts 1240 / 1241) et boutiques (a/2/8/2).
    let items = field_items::read(&game).unwrap();
    assert_eq!(items.len(), 613);
    assert_eq!(items.iter().filter(|f| f.kind == field_items::FieldItemKind::Hidden).count(), 213);
    let shop_list = shops::read(&game).unwrap();
    assert_eq!(shop_list.len(), 32);
    assert_eq!(shop_list[0].items, vec![4, 17]); // Poké Ball, Potion
    assert_eq!(shop_list[7].items, vec![331, 335]); // Ogoesse : CT04, CT08
    assert!(shop_list.iter().filter(|s| s.kind == shops::ShopKind::Tm).all(|s| s.items.iter().all(|&i| (328..=427).contains(&i))));

    // CT/CS (ARM9), donneurs de capacités (overlay 36).
    let m = machines::read(game.rom(), 5).unwrap();
    assert_eq!((m.tms[0], m.tms[1], m.hms[0]), (468, 337, 15)); // Aiguisage, Dracogriffe, Coupe
    let spec = machines::MachineSpec::for_generation(5).unwrap();
    assert!(machines::palette_slots(&game.rom().arm9_decompressed().unwrap(), &spec).is_some());
    let ovl = game.rom().overlay(machines::B2W2_TUTOR_OVERLAY).unwrap();
    let tutors = machines::B2w2Tutors::locate(&ovl, 559, &game.rom().header().game_code).unwrap();
    assert_eq!(tutors.offset, 0x5152C);
    assert_eq!(&tutors.moves(&ovl)[..4], &[20, 173, 282, 235]); // Étreinte, Ronflement, Sabotage, Synthèse
    let personal = game.narc(game.layout.personal).unwrap();
    // Dracaufeu : Canicule (n°4) et Dracochoc (n°57), pas Synthèse (n°3).
    assert!(machines::b2w2_tutor_compatible(&personal.files[6], 4));
    assert!(machines::b2w2_tutor_compatible(&personal.files[6], 57));
    assert!(!machines::b2w2_tutor_compatible(&personal.files[6], 3));

    // Chromatiques.
    assert_eq!(shiny::current_threshold(game.rom()).unwrap(), 8);

    // Pokémon fixes et échanges : tous relus sans incohérence.
    let listing = statics::list(&game).unwrap();
    assert_eq!(listing.statics.len(), 43, "{:?}", listing.notes);
    let kyurem: Vec<usize> = listing.statics.iter().filter(|s| s.species == 646).map(|s| s.index).collect();
    assert_eq!(kyurem, vec![5, 6, 7]);
    assert_eq!(listing.statics[29].species, 440); // Ptiravi (œuf)
    assert_eq!(listing.trades.len(), 30);
    let gigalith = listing.trades.iter().find(|t| t.entry == 26).unwrap();
    assert_eq!((gigalith.given, gigalith.requested), (526, 587)); // Gigalithe contre Emolga
    assert!(!listing.notes.iter().any(|n| n.contains("ignorée")), "{:?}", listing.notes);

    // Éditeur de ROM.
    assert!(romedit::supports(&game) && randomizer::supports(&game));
    let edit = romedit::read(&game).unwrap();
    assert_eq!(edit.species[0].stats, [45, 49, 49, 65, 65, 45]);
    assert_eq!(edit.moves.len(), 559);
}

/// Réglages qui touchent à tout ce que le randomizer sait modifier.
fn everything() -> Settings {
    serde_json::from_str(
        r#"{
        "starters": "random", "wild": "area", "wildLevelPercent": 110,
        "trainers": "type_themed", "trainerLevelPercent": 120, "stats": "shuffle",
        "randomTypes": true, "randomAbilities": true, "noLegendaries": false, "catchRate": "doubled",
        "easyEvolutions": true, "randomMovesets": true, "trainerEvolutions": true, "trainerMaxIvs": true,
        "shinyOdds": 512,
        "moves": { "randomTms": true, "randomTutors": true, "noGameBreaking": true, "keepFieldMoves": true,
                   "goodDamagingPercent": 40, "tmCompat": "random_prefer_type", "fullHmCompat": true,
                   "tutorCompat": "random_prefer_type", "followEvolutions": true, "levelupSanity": true },
        "items": { "fieldItems": "random", "banBadFieldItems": true, "shops": "random", "banBadShopItems": true,
                   "guaranteeEvolutionItems": true, "noMasterBall": true },
        "statics": { "mode": "random", "levelModifier": 10, "trades": "given_and_requested",
                     "tradeRandomItems": true, "tradeRandomIvs": true }
    }"#,
    )
    .unwrap()
}

/// Randomisation complète, reconstruction de la ROM (DSi : fichiers relogés), relecture.
#[test]
fn black2_full_randomization() {
    let Some(mut game) = open(BLACK2) else { return };
    let original_tms = machines::read(game.rom(), 5).unwrap();
    let original_shops = shops::read(&game).unwrap();
    let original_items = field_items::read(&game).unwrap();
    let original_statics = statics::list(&game).unwrap();
    let settings = everything();
    let outcome = randomizer::randomize(&mut game, &settings, 2024).unwrap();
    assert!(outcome.wild_slots > 4000 && outcome.trainer_pokemon > 1500, "{} / {}", outcome.wild_slots, outcome.trainer_pokemon);
    assert!(!outcome.log.contains("Non pris en charge"));

    let bytes = game.rom().to_bytes().unwrap();
    let reread = GameRom::from_rom(NdsRom::from_bytes(bytes).unwrap()).unwrap();
    let paths = DataPaths::for_game(reread.game).unwrap();

    // Starters : écran de choix, don et textes.
    let chosen: Vec<u16> = outcome.starters.iter().map(|s| s.id).collect();
    assert_eq!(starters::read(&reread, paths.starters).unwrap().to_vec(), chosen);
    let story = kaleido_formats::narc::Narc::parse(reread.rom().file_by_path("a/0/0/3").unwrap()).unwrap();
    let lines = kaleido_core::text::gen5::MsgFile::parse(&story.files[169]).unwrap().strings();
    assert!(lines[37].ends_with(&outcome.starters[0].name), "{}", lines[37]);
    assert!(lines[35].ends_with(&outcome.starters[2].name), "{}", lines[35]);

    // Pokédex, sauvages, dresseurs, Nuzlocke et page Combat se relisent.
    assert_eq!(reread.species().unwrap().len(), 649);
    let info = nuz::read(&reread).unwrap();
    assert_eq!(info.starters, chosen);
    assert!(info.routes.len() > 50);
    let battle = RomTrainers::from_nds(&reread).unwrap();
    assert!(battle.team(156).unwrap().iter().all(|c| c.level >= 13));

    // CT : attaques changées, CS intactes ; donneurs relus.
    let tms = machines::read(reread.rom(), 5).unwrap();
    assert_ne!(tms.tms, original_tms.tms);
    assert_eq!(tms.hms, original_tms.hms);
    let ovl = reread.rom().overlay(machines::B2W2_TUTOR_OVERLAY).unwrap();
    assert!(machines::B2w2Tutors::locate(&ovl, 559, &reread.rom().header().game_code).is_ok());

    // Objets et boutiques : mêmes emplacements, objets changés ; boutiques principales et de CT intactes.
    let items = field_items::read(&reread).unwrap();
    assert_eq!(items.len(), original_items.len());
    assert!(items.iter().zip(&original_items).filter(|(a, b)| a.item != b.item).count() > 300);
    let shop_list = shops::read(&reread).unwrap();
    for (a, b) in shop_list.iter().zip(&original_shops) {
        assert_eq!(a.items.len(), b.items.len());
        if a.kind != shops::ShopKind::Special {
            assert_eq!(a.items, b.items, "{}", a.name);
        }
    }

    // Chromatiques : 65536 / 512 = 128.
    assert_eq!(shiny::current_threshold(reread.rom()).unwrap(), 128);

    // Pokémon fixes : toujours lisibles (copies concordantes), formes imposées remises à 0.
    let listing = statics::list(&reread).unwrap();
    assert_eq!(listing.statics.len(), original_statics.statics.len(), "{:?}", listing.notes);
    let changed = listing.statics.iter().zip(&original_statics.statics).filter(|(a, b)| a.species != b.species).count();
    assert!(changed > 35);
    let scripts = kaleido_formats::narc::Narc::parse(reread.rom().file_by_path("a/0/5/6").unwrap()).unwrap();
    if listing.statics[6].species != 646 {
        assert_eq!(scripts.files[1208][0xD8D], 0, "forme de Kyurem Noir");
    }
    assert_eq!(listing.trades.len(), 30);

    // Signature Kaleido.
    assert!(reread.rom().header().game_code.starts_with("IRE"));
}
