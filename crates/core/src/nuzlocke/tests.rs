//! Tests du mode Nuzlocke : lecture des vraies ROMs (ignorés si absentes) et bilan
//! sur une sauvegarde Platine synthétique.

use std::path::Path;

use super::rom::{self, Encounter, Leader, LeaderKind, Route};
use super::*;
use crate::save::session::Slot;
use crate::save::{exp_for_level, GrowthRate, PkmFormat, Pokemon};
use crate::GameRom;

const ROMS: &str = "C:/Users/Thisma/Documents/NDS & 3DS";

fn open_rom(file: &str) -> Option<GameRom> {
    let path = Path::new(ROMS).join(file);
    if !path.exists() {
        eprintln!("ROM absente, test ignoré : {}", path.display());
        return None;
    }
    Some(GameRom::open(&path).unwrap())
}

fn route<'a>(info: &'a RomInfo, name: &str) -> &'a Route {
    info.routes.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("route « {name} » absente"))
}

fn leader<'a>(info: &'a RomInfo, name: &str) -> &'a Leader {
    info.leaders.iter().find(|l| l.name == name).unwrap()
}

#[test]
fn platinum_routes_and_caps() {
    let Some(game) = open_rom("Pokemon - Platinum Version (Europe).nds") else { return };
    let info = rom::read(&game).unwrap();

    // Route 201 en premier, lieu n°16 (même identifiant que dans les Pokémon capturés).
    let r201 = &info.routes[0];
    assert_eq!(r201.name, "Route 201");
    assert_eq!(r201.location_ids, vec![16]);
    let species: Vec<u16> = r201.encounters.iter().map(|e| e.species).collect();
    assert!(species.contains(&396) && species.contains(&399), "Étourmi et Keunotor : {species:?}");
    // Le texte de la ROM (anglais) porte le même index que la liste française de PKHeX.
    let names = game.text_file(433).unwrap();
    assert_eq!(names[16], "Route 201");
    assert_eq!(names[46], "Oreburgh Mine");
    let mine = route(&info, "Mine Charbourg");
    assert_eq!(mine.location_ids, vec![46]);
    assert_eq!(mine.zones.len(), 2);
    assert!(mine.encounters.iter().any(|e| e.species == 74 && e.methods.contains(&"Herbe")));
    assert!(route(&info, "Mont Couronné").zones.len() > 10);
    // Lac Vérité vient juste après la Route 201 dans l'ordre de l'histoire.
    assert_eq!(info.routes[1].location_ids, vec![76]);

    // Champions : classe « Leader » et Pokémon le plus fort lus dans la ROM.
    let caps: Vec<(&str, u16, u8)> = info.leaders.iter().map(|l| (l.name, l.ace_species, l.ace_level)).collect();
    assert_eq!(caps[0], ("Pierrick", 408, 14)); // Kranidos
    assert_eq!(leader(&info, "Flo").ace_level, 22);
    assert_eq!(leader(&info, "Kiméra").ace_level, 26);
    assert_eq!(leader(&info, "Mélina").ace_level, 32);
    assert_eq!(leader(&info, "Lovis").ace_level, 37);
    assert_eq!(leader(&info, "Charles").ace_level, 41);
    assert_eq!(leader(&info, "Gladys").ace_level, 44);
    assert_eq!((leader(&info, "Tanguy").ace_species, leader(&info, "Tanguy").ace_level), (466, 50)); // Élekable
    assert_eq!(leader(&info, "Lucio").ace_level, 59);
    assert_eq!((leader(&info, "Cynthia").ace_species, leader(&info, "Cynthia").ace_level), (445, 62)); // Carchacrok
    for l in &info.leaders {
        assert!(l.verified, "{} : classe inattendue ({})", l.name, l.class_name);
    }
    assert_eq!(leader(&info, "Pierrick").class_name, "Leader");
    assert_eq!(leader(&info, "Aaron").class_name, "Elite Four");
    assert_eq!(leader(&info, "Cynthia").class_name, "Champion");
    assert_eq!(info.leaders.iter().filter(|l| l.kind == LeaderKind::Gym).count(), 8);

    // Familles : Étoupic → Étourmi, Carchacrok → Griknot ; starters de Platine.
    assert_eq!(info.family_of(398), 396);
    assert_eq!(info.family_of(445), 443);
    assert_eq!(info.starters, vec![387, 390, 393]);

    // Bilan complet avec les vraies données de la ROM.
    let r = report(&info, &scenario(), &RunState::default()).unwrap();
    assert_eq!(r.routes[0].capture.as_ref().unwrap().nickname, "Piou");
    assert_eq!(r.stats.level_cap, Some(14));
    assert_eq!(r.caps[0].leader.name, "Pierrick");
    assert!(r.caps[0].current);
}

#[test]
fn white_routes_and_caps() {
    let Some(game) = open_rom("Pokemon - Version Blanche (France) (NDSi Enhanced).nds") else { return };
    let info = rom::read(&game).unwrap();

    assert_eq!(info.routes[0].name, "Route 1");
    assert_eq!(info.routes[0].location_ids, vec![14]);
    assert_eq!(info.routes[1].name, "Route 2");
    // ROM française : noms tirés de la ROM, et même index que la liste de PKHeX
    // (sauf le n°36, fusionné par PKHeX avec le PWT de Noire 2 / Blanche 2).
    let names = game.text_file(89).unwrap();
    for r in &info.routes {
        for &id in &r.location_ids {
            assert_eq!(names[id as usize], r.name, "lieu n°{id}");
            if id != 36 {
                // Kaleido ajoute parfois un suffixe pour distinguer les doublons : « (N/B) ».
                let pkhex = crate::dex::location_name(5, id).unwrap_or_default();
                assert!(pkhex.starts_with(r.name.as_str()), "lieu n°{id} : {pkhex} ≠ {}", r.name);
            }
        }
    }
    assert_eq!(route(&info, "Hangar Frigorifique").location_ids, vec![36]);
    let castle = route(&info, "Château Enfoui");
    assert!(castle.zones.len() > 20);
    // Les saisons sont fusionnées : chaque espèce n'apparaît qu'une fois par route.
    for r in &info.routes {
        let mut s: Vec<u16> = r.encounters.iter().map(|e| e.species).collect();
        let n = s.len();
        s.dedup();
        assert_eq!(s.len(), n);
    }
    assert!(route(&info, "Route 1").encounters.iter().any(|e| e.species == 504)); // Ratentif

    let levels: Vec<u8> = info.leaders.iter().map(|l| l.ace_level).collect();
    assert_eq!(levels, vec![14, 20, 23, 27, 31, 35, 39, 43, 50, 50, 50, 50, 52, 54]);
    for l in &info.leaders {
        assert!(l.verified, "{} : classe inattendue ({})", l.name, l.class_name);
    }
    assert_eq!(leader(&info, "Inezia").class_name, "Champion");
    assert_eq!(leader(&info, "Anis").class_name, "Conseil 4");
    assert_eq!(leader(&info, "Inezia").ace_species, 523); // Zéblitz
    // Blanche : N utilise Reshiram.
    assert_eq!(leader(&info, "N").ace_species, 643);
    assert_eq!(leader(&info, "Ghetis").ace_species, 635); // Trioxhydre
}

// --- Bilan sur une sauvegarde synthétique.

fn enc(species: u16) -> Encounter {
    Encounter { species, min_level: 2, max_level: 4, methods: vec!["Herbe"] }
}

fn test_route(name: &str, id: u16, order: u32, species: &[u16]) -> Route {
    Route {
        key: name.into(),
        name: name.into(),
        location_ids: vec![id],
        zones: vec![],
        encounters: species.iter().map(|&s| enc(s)).collect(),
        order,
    }
}

fn test_leader(kind: LeaderKind, name: &'static str, level: u8) -> Leader {
    Leader {
        kind,
        label: String::new(),
        name,
        town: "",
        trainer_ids: vec![],
        ace_species: 1,
        ace_level: level,
        class_name: String::new(),
        verified: true,
    }
}

fn test_rom() -> RomInfo {
    let mut family: Vec<u16> = (0..=493).collect();
    family[388] = 387;
    family[389] = 387;
    family[397] = 396;
    family[398] = 396;
    RomInfo {
        game: Game::Platinum,
        routes: vec![
            test_route("Route 201", 16, 0, &[396, 399]),
            test_route("Route 202", 17, 2, &[396, 403]),
            test_route("Route 203", 18, 3, &[63]),
            test_route("Mine Charbourg", 46, 5, &[74, 95]),
        ],
        leaders: vec![
            test_leader(LeaderKind::Gym, "Pierrick", 14),
            test_leader(LeaderKind::Gym, "Flo", 22),
            test_leader(LeaderKind::Elite, "Aaron", 53),
            test_leader(LeaderKind::Elite, "Lucio", 59),
            test_leader(LeaderKind::Champion, "Cynthia", 62),
        ],
        starters: vec![387, 390, 393],
        family,
        seed: Some(42),
    }
}

/// Pokémon capturé par le dresseur de la sauvegarde (TID/SID nuls), non chromatique.
fn caught(species: u16, pid: u32, location: u16, level: u8, day: u8, nickname: Option<&str>) -> Pokemon {
    let mut p = Pokemon::blank(PkmFormat::Gen4);
    p.set_species(species);
    p.set_pid(pid);
    let growth = crate::names::growth_rate(species).unwrap_or(GrowthRate::MediumFast);
    p.set_exp(exp_for_level(growth, level));
    p.set_met_location(location);
    p.set_met_level(level).unwrap();
    p.set_met_date(Some(PkmDate { year: 2026, month: 10, day }));
    match nickname {
        Some(n) => {
            p.set_nickname(n).unwrap();
            p.set_is_nicknamed(true);
        }
        None => {
            p.set_nickname(crate::names::species(species).unwrap_or("?")).unwrap();
            p.set_is_nicknamed(false);
        }
    }
    p.refresh_checksum();
    p
}

const STARTER: &str = "12345601";

/// Scénario : starter K.O. dans l'équipe, une capture et une capture de trop sur la
/// Route 201, un doublon puis une vraie capture sur la Route 202, une capture rangée
/// au cimetière (boîte « RIP »).
fn scenario() -> SaveSession {
    let mut s = SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
    for i in 0..12 {
        s.save.set_box_slot(0, i, None).unwrap();
    }
    s.save.set_box_slot(0, 0, Some(caught(387, 0x1234_5601, 16, 5, 1, Some("Tortue")))).unwrap(); // starter
    s.save.set_box_slot(0, 1, Some(caught(397, 0x1234_5602, 17, 20, 3, Some("Plume")))).unwrap(); // doublon
    s.save.set_box_slot(0, 2, Some(caught(403, 0x1234_5603, 17, 10, 5, Some("Lixy")))).unwrap();
    for i in 0..3 {
        s.move_pokemon(Slot::Box { r#box: 0, index: i }, Slot::Party { index: i }).unwrap();
    }
    // Le starter tombe K.O.
    let mut starter = s.save.party_slot(0).unwrap().unwrap();
    starter.set_current_hp(0);
    starter.refresh_checksum();
    s.save.set_party_slot(0, Some(starter)).unwrap();

    s.save.set_box_slot(0, 5, Some(caught(396, 0x1234_5604, 16, 3, 2, Some("Piou")))).unwrap();
    s.save.set_box_slot(0, 6, Some(caught(399, 0x1234_5605, 16, 4, 4, None))).unwrap(); // capture de trop
    s.set_box_name(1, "RIP").unwrap();
    s.save.set_box_slot(1, 0, Some(caught(74, 0x1234_5606, 46, 8, 6, None))).unwrap();
    s
}

#[test]
fn report_on_synthetic_save() {
    let s = scenario();
    let mut state = RunState::default();
    state.missed.insert("Route 203".into());
    let r = report(&test_rom(), &s, &state).unwrap();

    let route = |k: &str| r.routes.iter().find(|x| x.key == k).unwrap();
    let r201 = route("Route 201");
    assert_eq!(r201.status, RouteStatus::Caught);
    assert_eq!(r201.capture.as_ref().unwrap().nickname, "Piou");
    assert_eq!(r201.others.len(), 1);
    assert_eq!(r201.others[0].catch, Some(CatchKind::Extra));
    // Le starter (rencontré au niveau 5 sur la Route 201) ne compte pas comme capture.
    let starter = r.party.iter().find(|m| m.key == STARTER).unwrap();
    assert_eq!(starter.origin, Origin::Starter);
    assert!(starter.dead);
    assert_eq!(starter.death_cause.as_deref(), Some("K.O. dans l'équipe"));

    // Route 202 : Étourvol est un doublon (famille d'Étourmi), Lixy compte ensuite.
    let r202 = route("Route 202");
    assert_eq!(r202.status, RouteStatus::Caught);
    assert_eq!(r202.capture.as_ref().unwrap().species, 403);
    assert_eq!(r202.others[0].catch, Some(CatchKind::Dupe));
    assert!(r202.encounters.iter().find(|e| e.encounter.species == 396).unwrap().owned);

    assert_eq!(route("Route 203").status, RouteStatus::Missed);
    let mine = route("Mine Charbourg");
    assert_eq!(mine.status, RouteStatus::Caught);
    assert!(mine.capture.as_ref().unwrap().dead);
    assert_eq!(r.graveyard_box.as_ref().map(|g| (g.index, g.auto)), Some((1, true)));

    assert_eq!(r.stats.captures, 3);
    assert_eq!(r.stats.dead, 2);
    assert_eq!(r.stats.routes_caught, 3);
    assert_eq!(r.stats.routes_missed, 1);
    assert_eq!(r.stats.badges, 0);
    assert_eq!(r.stats.level_cap, Some(14));
    assert_eq!(r.seed, Some(42));
    assert_eq!(r.graveyard.len(), 2);

    let rules: Vec<(&str, Option<&str>)> = r.violations.iter().map(|v| (v.rule, v.mon.as_deref())).collect();
    assert!(rules.contains(&("levelCap", Some("12345602"))), "{rules:?}"); // Étourvol N. 20 > 14
    assert!(!rules.contains(&("levelCap", Some("12345603")))); // Lixy N. 10
    assert!(rules.contains(&("dead", Some(STARTER))));
    assert!(rules.contains(&("onePerRoute", Some("12345605"))));
    assert!(rules.contains(&("nickname", Some("12345606")))); // Racaillou sans surnom
    assert!(rules.contains(&("dupes", Some("12345602"))));
    assert_eq!(r.violations[0].severity, Severity::Error);
}

#[test]
fn rules_badges_and_manual_marks() {
    let mut s = scenario();
    // Un badge : le niveau maximum devient celui de Flo.
    s.save.set_badges(0b1);
    let mut state = RunState::default();
    let r = report(&test_rom(), &s, &state).unwrap();
    assert_eq!((r.stats.badges, r.stats.level_cap), (1, Some(22)));
    assert!(r.caps[0].beaten && r.caps[1].current);
    assert!(!r.violations.iter().any(|v| v.rule == "levelCap"));

    // Badges forcés à 8 : niveau maximum du Conseil 4.
    state.badges = Some(8);
    let r = report(&test_rom(), &s, &state).unwrap();
    assert_eq!(r.stats.level_cap, Some(59));
    assert!(!r.stats.badges_from_save);

    // Sans clause doublons, Étourvol (capturé avant Lixy) compte pour la Route 202.
    state.rules.dupes_clause = false;
    // Starter déclaré vivant, Lixy déclaré mort, cimetière désactivé (boîte 3 choisie).
    state.alive.insert(STARTER.into());
    state.dead.insert("12345603".into());
    state.graveyard_box = Some(3);
    let r = report(&test_rom(), &s, &state).unwrap();
    let r202 = r.routes.iter().find(|x| x.key == "Route 202").unwrap();
    assert_eq!(r202.capture.as_ref().unwrap().species, 397);
    assert_eq!(r202.others[0].catch, Some(CatchKind::Extra));
    assert!(!r.party.iter().find(|m| m.key == STARTER).unwrap().dead);
    let lixy = r.party.iter().find(|m| m.key == "12345603").unwrap();
    assert_eq!(lixy.death_cause.as_deref(), Some("Marqué mort à la main"));
    assert!(r.violations.iter().any(|v| v.rule == "dead" && v.severity == Severity::Error));
    assert!(!r.routes.iter().find(|x| x.key == "Mine Charbourg").unwrap().capture.as_ref().unwrap().dead);
    assert_eq!(r.graveyard_box.as_ref().map(|g| g.auto), Some(false));
}

#[test]
fn shiny_clause_and_mismatch() {
    let mut s = scenario();
    // Chromatique (TID/SID nuls, moitiés du PID égales) en plus sur la Route 201.
    s.save.set_box_slot(0, 7, Some(caught(399, 0x0BAD_0BAD, 16, 4, 9, Some("Brille")))).unwrap();
    let state = RunState::default();
    let r = report(&test_rom(), &s, &state).unwrap();
    let shiny = r.routes[0].others.iter().find(|m| m.key == "0BAD0BAD").unwrap();
    assert_eq!(shiny.catch, Some(CatchKind::ShinyBonus));
    assert!(!r.violations.iter().any(|v| v.mon.as_deref() == Some("0BAD0BAD")));

    let mut bw = test_rom();
    bw.game = Game::White;
    assert!(matches!(report(&bw, &s, &state), Err(NuzlockeError::Mismatch(_))));
}

#[test]
fn state_file_roundtrip() {
    let dir = std::env::temp_dir().join(format!("kaleido-nuz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("partie.sav");
    assert_eq!(state_path(&save), dir.join("partie.sav.nuzlocke.json"));
    assert_eq!(load_state(&save), RunState::default());
    let mut state = RunState { rom_path: Some("C:/rom.nds".into()), graveyard_box: Some(17), ..Default::default() };
    state.rules.set_mode = true;
    state.missed.insert("Route 1".into());
    store_state(&save, &state).unwrap();
    assert_eq!(load_state(&save), state);
    // Champs absents : valeurs par défaut (fichier d'une version précédente).
    std::fs::write(state_path(&save), br#"{"badges": 3}"#).unwrap();
    let old = load_state(&save);
    assert_eq!(old.badges, Some(3));
    assert!(old.rules.dupes_clause);
    std::fs::remove_dir_all(&dir).unwrap();
}
