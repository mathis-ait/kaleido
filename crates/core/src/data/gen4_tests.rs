//! Diamant et SoulSilver (vraies ROMs françaises, tests ignorés si absentes) : lecture de
//! chaque format octet par octet, puis randomisation complète relue après reconstruction.

use kaleido_formats::nds::NdsRom;

use super::{encounters, field_items, machines, shiny, shops, starters, trainers, DataPaths};
use crate::battle::trainers::{Role, RomTrainers};
use crate::games::Game;
use crate::nuzlocke::rom as nuzlocke;
use crate::randomizer::{self, statics};
use crate::rom::GameRom;

const DIAMOND: &str = "Pokemon - Version Diamant (France) (Rev 5).nds";

fn open(name: &str) -> Option<GameRom> {
    let path = crate::test_rom_path(name);
    if !path.exists() {
        eprintln!("ROM absente, test ignoré : {}", path.display());
        return None;
    }
    Some(GameRom::open(&path).unwrap())
}

/// Relit une ROM modifiée en mémoire comme si elle avait été enregistrée puis rouverte.
fn reopen(game: &GameRom) -> GameRom {
    GameRom::from_rom(NdsRom::from_bytes(game.rom().to_bytes().unwrap()).unwrap()).unwrap()
}

#[test]
fn diamond_formats() {
    let Some(game) = open(DIAMOND) else { return };
    assert_eq!(game.game, Game::Diamond);
    assert!(game.layout.verified);
    let species = game.species().unwrap();
    assert_eq!(species.len(), 493);
    assert_eq!((species[386].name.as_str(), species[389].name.as_str(), species[392].name.as_str()), ("TORTIPOUSS", "OUISTICRAM", "TIPLOUF"));
    assert_eq!(species[0].base_stats.total(), 318);

    let paths = DataPaths::for_game(game.game).unwrap();
    assert_eq!(starters::read(&game, paths.starters).unwrap(), [387, 390, 393]);
    assert_eq!(game.text_file(paths.move_names).unwrap()[33], "Charge");

    // Évolutions et attaques apprises : Tortipouss → Boskara au niveau 18, Charge au niveau 1.
    let evo = super::evolutions::read(&game.narc(paths.evolutions).unwrap().files[387]);
    assert_eq!((evo[0].method, evo[0].param, evo[0].target), (super::evolutions::METHOD_LEVEL, 18, 388));
    let learn = super::learnsets::read(4, &game.narc(paths.learnsets).unwrap().files[387]);
    assert_eq!(learn[0], (33, 1));

    // Rencontres : 183 zones au format Platine ; les espèces sont toutes valides.
    let enc = game.narc(paths.encounters).unwrap();
    assert_eq!(enc.files.len(), 183);
    let all: Vec<encounters::Slot> = enc.files.iter().flat_map(|f| encounters::read_format(encounters::Format::for_game(game.game), f)).collect();
    assert!(all.len() > 5000);
    assert!(all.iter().all(|s| (1..=493).contains(&s.species) && (1..=100).contains(&s.min_level)));

    // Dresseurs : entrées de 6 octets (sans sceau de Ball) ; toutes les équipes se lisent.
    let trdata = game.narc(paths.trainer_data).unwrap().files;
    let trpoke = game.narc(paths.trainer_pokemon).unwrap().files;
    let fmt = trainers::TeamFormat::for_game(game.game);
    let teams: Vec<_> = (1..trdata.len()).filter_map(|i| trainers::read_team_with(fmt, &trdata[i], &trpoke[i])).collect();
    assert!(teams.len() > 800);
    assert!(teams.iter().flat_map(|t| &t.pokemon).all(|p| (1..=493).contains(&p.species) && (1..=100).contains(&p.level)));
    let roark = trainers::read_team_with(fmt, &trdata[246], &trpoke[246]).unwrap();
    assert_eq!(roark.pokemon.iter().map(|p| (p.species, p.level)).collect::<Vec<_>>(), [(74, 12), (95, 12), (408, 14)]);

    // Objets, boutiques, CT, taux de chromatiques.
    let items = field_items::read(&game).unwrap();
    assert_eq!(items.iter().filter(|f| f.kind == field_items::FieldItemKind::Hidden).count(), 229);
    assert_eq!(items[0].item, 365); // CT38
    let shops = shops::read(&game).unwrap();
    assert_eq!(shops.len(), 19);
    assert_eq!(shops.iter().filter(|s| s.kind == shops::ShopKind::Tm).count(), 2);
    let tms = machines::read_game(game.rom(), game.game).unwrap();
    assert_eq!((tms.tms[0], tms.tms[91], tms.hms[0], tms.hms[7]), (264, 433, 15, 431));
    let spec = machines::MachineSpec::for_game(game.game).unwrap();
    assert!(machines::palette_slots(&game.rom().arm9_decompressed().unwrap(), &spec).is_some());
    assert_eq!(shiny::current_threshold(game.rom()).unwrap(), shiny::DEFAULT_THRESHOLD);

    // Pokémon fixes et échanges : tout est relu sans remarque autre que les vagabonds.
    let listing = statics::list(&game).unwrap();
    assert_eq!(listing.statics.len(), 17 + 7);
    assert_eq!(listing.statics[1].species, 483); // Dialga
    assert_eq!(listing.statics[17].species, 142); // Ptéra (fossile)
    assert_eq!(listing.trades.len(), 4);
    assert_eq!((listing.trades[0].given, listing.trades[0].requested, listing.trades[0].nickname.as_str()), (63, 66, "Kazou"));
    assert_eq!(listing.notes.len(), 1, "{:?}", listing.notes);
}

#[test]
fn diamond_nuzlocke_and_battle() {
    let Some(game) = open(DIAMOND) else { return };
    let info = nuzlocke::read(&game).unwrap();
    let r201 = &info.routes[0];
    assert_eq!((r201.name.as_str(), r201.location_ids.as_slice()), ("Route 201", &[16][..]));
    let species: Vec<u16> = r201.encounters.iter().map(|e| e.species).collect();
    assert!(species.contains(&396) && species.contains(&399), "Étourmi et Keunotor : {species:?}");
    assert!(info.routes.iter().any(|r| r.name == "Mine Charbourg" && r.encounters.iter().any(|e| e.species == 74)));
    assert_eq!(info.starters, vec![387, 390, 393]);
    assert_eq!(info.leaders.len(), 13);
    assert!(info.leaders.iter().all(|l| l.verified), "classes des champions");
    assert_eq!((info.leaders[0].name, info.leaders[0].ace_species, info.leaders[0].ace_level), ("Pierrick", 408, 14));
    assert_eq!(info.leaders[12].class_name, "Maître");
    assert_eq!(info.family_of(389), 387);

    let r = RomTrainers::from_nds(&game).unwrap();
    assert!(r.verified);
    let roark = r.trainer(246).unwrap();
    assert_eq!((roark.name.as_str(), roark.role), ("Pierrick", Some(Role::Gym)));
    let built = r.team(246).unwrap();
    assert!(built.iter().all(|c| c.stats[0] > 0 && c.moves.iter().any(|&m| m != 0)));
    assert!(r.trainers.iter().any(|t| t.name == "Cynthia" && t.role == Some(Role::Champion)));
    // Combats en duo (jumelles : classe 8) lus comme sur Platine (type de combat en 0x10).
    let doubles: Vec<u16> = r.trainers.iter().filter(|t| t.double).map(|t| t.class_id).collect();
    assert!(doubles.contains(&8), "{doubles:?}");
}

/// Randomisation complète de Diamant, puis relecture de la ROM reconstruite.
#[test]
fn diamond_full_randomization() {
    let Some(mut game) = open(DIAMOND) else { return };
    let original = reopen(&game);
    let settings: randomizer::Settings = serde_json::from_str(
        r#"{"starters":"triangle","wild":"area","wildLevelPercent":120,"trainers":"type_themed","trainerLevelPercent":110,
            "stats":"shuffle","randomTypes":true,"randomAbilities":true,"catchRate":"doubled","easyEvolutions":true,
            "randomMovesets":true,"trainerEvolutions":true,"trainerMaxIvs":true,"shinyOdds":512,
            "moves":{"randomTms":true,"tmCompat":"random_prefer_type","fullHmCompat":true,"levelupSanity":true},
            "items":{"fieldItems":"random_even","shops":"random","guaranteeEvolutionItems":true},
            "statics":{"mode":"similar_strength","levelModifier":10,"trades":"given_and_requested","tradeRandomIvs":true}}"#,
    )
    .unwrap();
    let outcome = randomizer::randomize(&mut game, &settings, 2024).unwrap();
    assert!(outcome.wild_slots > 5000 && outcome.trainer_pokemon > 1000);
    let after = reopen(&game);

    // Starters : tableau, écran de choix et scripts du rival.
    let chosen: Vec<u16> = outcome.starters.iter().map(|s| s.id).collect();
    let paths = DataPaths::for_game(Game::Diamond).unwrap();
    assert_eq!(starters::read(&after, paths.starters).unwrap().to_vec(), chosen);
    let screen = after.text_file(320).unwrap();
    let names = after.text_file(after.layout.species_names).unwrap();
    for (i, &s) in chosen.iter().enumerate() {
        assert!(screen[i + 1].contains(&names[s as usize]), "{}", screen[i + 1]);
    }
    let scripts = |g: &GameRom| g.narc("fielddata/script/scr_seq_release.narc").unwrap().files;
    let (before_scripts, after_scripts) = (scripts(&original), scripts(&after));
    let patched = [34, 90, 118, 180, 195, 394, 2, 131, 230].iter().filter(|&&f| before_scripts[f] != after_scripts[f]).count();
    assert!(patched >= 6, "scripts du rival corrigés : {patched}");

    // Tout reste lisible, et a bien changé.
    let fmt = trainers::TeamFormat::for_game(Game::Diamond);
    let trdata = after.narc(paths.trainer_data).unwrap().files;
    let trpoke = after.narc(paths.trainer_pokemon).unwrap().files;
    let roark = trainers::read_team_with(fmt, &trdata[246], &trpoke[246]).unwrap();
    assert_eq!(roark.pokemon.len(), 3);
    assert!(roark.pokemon.iter().all(|p| (1..=493).contains(&p.species)));
    assert!(roark.pokemon.iter().map(|p| p.species).ne([74, 95, 408]));
    let enc = after.narc(paths.encounters).unwrap();
    let slots: usize = enc.files.iter().map(|f| encounters::read(4, f).len()).sum();
    assert_eq!(slots, outcome.wild_slots);
    assert_eq!(field_items::read(&after).unwrap().len(), field_items::read(&original).unwrap().len());
    assert_ne!(shops::read(&after).unwrap(), shops::read(&original).unwrap());
    assert_ne!(machines::read_game(after.rom(), Game::Diamond).unwrap().tms, machines::read_game(original.rom(), Game::Diamond).unwrap().tms);
    assert_eq!(shiny::current_threshold(after.rom()).unwrap(), 128);
    let listing = statics::list(&after).unwrap();
    assert_eq!(listing.statics.len(), 24);
    assert_ne!(
        listing.statics.iter().map(|s| s.species).collect::<Vec<_>>(),
        statics::list(&original).unwrap().statics.iter().map(|s| s.species).collect::<Vec<_>>()
    );
    assert!(nuzlocke::read(&after).is_ok());
    assert!(RomTrainers::from_nds(&after).is_ok());
    assert!(crate::romedit::read(&after).is_ok());
}

// ---------- SoulSilver (ROM française IPGF) ----------

const SOULSILVER: &str = "Pokemon - Version Argent SoulSilver (France).nds";

#[test]
fn soulsilver_formats() {
    let Some(game) = open(SOULSILVER) else { return };
    assert_eq!((game.game, game.rom().header().game_code.as_str()), (Game::SoulSilver, "IPGF"));
    assert!(game.layout.verified);
    let species = game.species().unwrap();
    assert_eq!((species[151].name.as_str(), species[154].name.as_str(), species[157].name.as_str()), ("GERMIGNON", "HERICENDRE", "KAIMINUS"));
    let paths = DataPaths::for_game(game.game).unwrap();
    assert_eq!(starters::read(&game, paths.starters).unwrap(), [152, 155, 158]);
    assert_eq!(game.text_file(paths.move_names).unwrap()[33], "Charge");
    assert_eq!(game.text_file(190).unwrap()[1], "Orme: Préfères-tu {VAR:FF00,0003}GERMIGNON{VAR:FF00,0000},\nle Pokémon Plante?");

    // Évolutions (Germignon → Macronium au niveau 16) et attaques apprises (Charge au niveau 1).
    let evo = super::evolutions::read(&game.narc(paths.evolutions).unwrap().files[152]);
    assert_eq!((evo[0].param, evo[0].target), (16, 153));
    assert_eq!(super::learnsets::read(4, &game.narc(paths.learnsets).unwrap().files[152])[0], (33, 1));

    // Rencontres au format Johto : toutes les espèces et tous les niveaux sont valides.
    let enc = game.narc(paths.encounters).unwrap();
    let fmt = encounters::Format::for_game(game.game);
    let all: Vec<encounters::Slot> = enc.files.iter().flat_map(|f| encounters::read_format(fmt, f)).collect();
    assert!(all.len() > 5000);
    assert!(all.iter().all(|s| (1..=493).contains(&s.species) && (1..=100).contains(&s.min_level) && s.min_level <= s.max_level));

    // Dresseurs (avec sceau de Ball, comme Platine) : Albert, Roucool 9 et Roucoups 13.
    let trdata = game.narc(paths.trainer_data).unwrap().files;
    let trpoke = game.narc(paths.trainer_pokemon).unwrap().files;
    let tfmt = trainers::TeamFormat::for_game(game.game);
    let teams: Vec<_> = (1..trdata.len()).filter_map(|i| trainers::read_team_with(tfmt, &trdata[i], &trpoke[i])).collect();
    assert!(teams.len() > 700);
    assert!(teams.iter().flat_map(|t| &t.pokemon).all(|p| (1..=493).contains(&p.species) && (1..=100).contains(&p.level)));
    let falkner = trainers::read_team_with(tfmt, &trdata[20], &trpoke[20]).unwrap();
    assert_eq!(falkner.pokemon.iter().map(|p| (p.species, p.level)).collect::<Vec<_>>(), [(16, 9), (17, 13)]);

    // Objets (table 0xFA53C), boutiques, CT/CS (CS05 = Siphon), chromatiques.
    let items = field_items::read(&game).unwrap();
    assert_eq!(items.len(), 485);
    assert_eq!(items.iter().filter(|f| f.kind == field_items::FieldItemKind::Hidden).count(), 231);
    let shops = shops::read(&game).unwrap();
    assert_eq!(shops.len(), 29);
    let item_names = game.text_file(222).unwrap();
    let griotte: Vec<&str> = shops[0].items.iter().map(|&i| item_names[i as usize].as_str()).collect();
    assert_eq!(griotte, ["Lettre Avion", "Soin Ball"]); // Ville Griotte
    let tms = machines::read_game(game.rom(), game.game).unwrap();
    assert_eq!((tms.tms[0], tms.hms[4]), (264, 250));
    let spec = machines::MachineSpec::for_game(game.game).unwrap();
    assert!(machines::palette_slots(&game.rom().arm9_decompressed().unwrap(), &spec).is_some());
    assert_eq!(shiny::current_threshold(game.rom()).unwrap(), shiny::DEFAULT_THRESHOLD);

    // Pokémon fixes, fossiles (overlay 21) et échanges.
    let listing = statics::list(&game).unwrap();
    assert_eq!(listing.statics.len(), 37 + 7);
    let first: Vec<u16> = listing.statics.iter().take(3).map(|s| s.species).collect();
    assert_eq!(first, [249, 250, 245]); // Lugia, Ho-Oh, Suicune
    assert_eq!(listing.statics[37].species, 142); // Ptéra
    assert_eq!(listing.trades.len(), 11);
    assert_eq!((listing.trades[0].given, listing.trades[0].requested, listing.trades[0].nickname.as_str()), (95, 69, "Lombric")); // Onix contre Chétiflor
    assert_eq!(listing.notes.len(), 1, "{:?}", listing.notes);
}

#[test]
fn soulsilver_nuzlocke_and_battle() {
    let Some(game) = open(SOULSILVER) else { return };
    let info = nuzlocke::read(&game).unwrap();
    let r29 = &info.routes[0];
    assert_eq!((r29.name.as_str(), r29.location_ids.as_slice()), ("Route 29", &[177][..]));
    let method = |s: u16| r29.encounters.iter().find(|e| e.species == s).map(|e| e.methods.clone()).unwrap_or_default();
    assert!(method(16).contains(&"Herbe (matin)") && method(161).contains(&"Herbe (jour)"));
    assert_eq!(method(163), ["Herbe (nuit)"], "Hoothoot seulement la nuit");
    assert!(info
        .routes
        .iter()
        .any(|r| r.name == "Mauville" && r.encounters.iter().any(|e| e.species == 340 && e.methods.contains(&"Essaim (pêche)"))));
    assert!(info.routes.iter().all(|r| r.encounters.iter().all(|e| e.min_level >= 1 && e.min_level <= e.max_level)));
    assert_eq!(info.starters, vec![152, 155, 158]);
    assert_eq!(info.leaders.len(), 21);
    assert!(info.leaders.iter().all(|l| l.verified), "classes des champions");
    let caps: Vec<(&str, u16, u8)> = info.leaders.iter().map(|l| (l.name, l.ace_species, l.ace_level)).collect();
    assert_eq!(caps[0], ("Albert", 17, 13));
    assert_eq!(caps[2], ("Blanche", 241, 19)); // Écrémeuh
    assert_eq!(caps[12], ("Peter", 149, 50));
    assert_eq!(caps[20], ("Blue", 18, 60));

    let r = RomTrainers::from_nds(&game).unwrap();
    assert!(r.verified);
    let role = |id: u16| r.trainer(id).and_then(|t| t.role);
    assert_eq!(
        (role(20), role(30), role(418), role(244), role(1)),
        (Some(Role::Gym), Some(Role::Gym), Some(Role::EliteFour), Some(Role::Champion), Some(Role::Rival))
    );
    assert_eq!(role(492), None, "Pokéfan (ancien index « rival » de l'UPR)");
    assert!(r.trainers.iter().any(|t| t.double), "combats en duo");
    let whitney = r.team(30).unwrap();
    assert!(whitney.iter().all(|c| c.stats[0] > 0 && c.moves.iter().any(|&m| m != 0)));
}

/// Randomisation complète de SoulSilver, puis relecture de la ROM reconstruite.
#[test]
fn soulsilver_full_randomization() {
    let Some(mut game) = open(SOULSILVER) else { return };
    let original = reopen(&game);
    let settings: randomizer::Settings = serde_json::from_str(
        r#"{"starters":"triangle","wild":"area","wildLevelPercent":120,"trainers":"type_themed","trainerLevelPercent":110,
            "stats":"shuffle","randomTypes":true,"randomAbilities":true,"catchRate":"doubled","easyEvolutions":true,
            "randomMovesets":true,"trainerEvolutions":true,"trainerMaxIvs":true,"shinyOdds":512,
            "moves":{"randomTms":true,"tmCompat":"random_prefer_type","fullHmCompat":true,"levelupSanity":true},
            "items":{"fieldItems":"random_even","shops":"random","guaranteeEvolutionItems":true},
            "statics":{"mode":"similar_strength","levelModifier":10,"trades":"given_and_requested","tradeRandomIvs":true}}"#,
    )
    .unwrap();
    let outcome = randomizer::randomize(&mut game, &settings, 4242).unwrap();
    assert!(outcome.wild_slots > 5000 && outcome.trainer_pokemon > 1000);
    assert!(!outcome.log.contains("non vérifiés"));
    let after = reopen(&game);

    // Starters : ARM9, textes du Prof. Orme, scripts du rival (7 fichiers), cris (overlay 61).
    let chosen: Vec<u16> = outcome.starters.iter().map(|s| s.id).collect();
    let paths = DataPaths::for_game(Game::SoulSilver).unwrap();
    assert_eq!(starters::read(&after, paths.starters).unwrap().to_vec(), chosen);
    let names = after.text_file(after.layout.species_names).unwrap();
    let screen = after.text_file(190).unwrap();
    for (i, &s) in chosen.iter().enumerate() {
        assert!(screen[i + 1].contains(&names[s as usize]) && screen[i + 4].contains(&names[s as usize]));
    }
    let scripts = |g: &GameRom| g.narc("a/0/1/2").unwrap().files;
    let (before_scripts, after_scripts) = (scripts(&original), scripts(&after));
    let patched = [7, 23, 96, 110, 819, 850, 866].iter().filter(|&&f| before_scripts[f] != after_scripts[f]).count();
    assert_eq!(patched, 7, "scripts du rival corrigés");
    assert_ne!(after.rom().overlay(61).unwrap(), original.rom().overlay(61).unwrap(), "cris de l'écran de choix");

    // Tout reste lisible, et a bien changé.
    let tfmt = trainers::TeamFormat::for_game(Game::SoulSilver);
    let trdata = after.narc(paths.trainer_data).unwrap().files;
    let trpoke = after.narc(paths.trainer_pokemon).unwrap().files;
    let falkner = trainers::read_team_with(tfmt, &trdata[20], &trpoke[20]).unwrap();
    assert!(falkner.pokemon.iter().all(|p| (1..=493).contains(&p.species)));
    assert!(falkner.pokemon.iter().map(|p| p.species).ne([16, 17]));
    let fmt = encounters::Format::for_game(Game::SoulSilver);
    let slots: usize = after.narc(paths.encounters).unwrap().files.iter().map(|f| encounters::read_format(fmt, f).len()).sum();
    assert_eq!(slots, outcome.wild_slots);
    assert_eq!(field_items::read(&after).unwrap().len(), 485);
    assert_ne!(shops::read(&after).unwrap(), shops::read(&original).unwrap());
    assert_ne!(machines::read_game(after.rom(), Game::SoulSilver).unwrap().tms, machines::read_game(original.rom(), Game::SoulSilver).unwrap().tms);
    assert_eq!(shiny::current_threshold(after.rom()).unwrap(), 128);
    let listing = statics::list(&after).unwrap();
    assert_eq!(listing.statics.len(), 44);
    assert_eq!(listing.trades.len(), 11);
    assert!(nuzlocke::read(&after).is_ok());
    assert!(RomTrainers::from_nds(&after).is_ok());
    assert!(crate::romedit::read(&after).is_ok());
}
