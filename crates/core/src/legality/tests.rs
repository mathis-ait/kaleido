//! Tests de la légalité : fichiers légaux et illégaux des tests de PKHeX
//! (`Tests/PKHeX.Core.Tests/Legality/{Legal,Illegal}`, copiés dans
//! `tests/data/pkhex/legality/`), base des rencontres, « Rendre légal » et création.

use std::path::{Path, PathBuf};

use super::encounters::{encounters, EncounterKind};
use super::legalize::{generate_legal, legalize, GenerateRequest};
use super::verify::{analyze, Verdict};
use crate::dex::Game;
use crate::save::{Gender, PkmFormat, PlayTime, Pokemon, Trainer};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| matches!(x.to_str(), Some("pk4" | "pk5" | "pk6" | "pk7"))) {
            out.push(p);
        }
    }
    out.sort();
}

fn load(path: &Path) -> (Pokemon, Game) {
    let bytes = std::fs::read(path).unwrap();
    let (format, game) = match path.extension().and_then(|x| x.to_str()) {
        Some("pk4") => (PkmFormat::Gen4, Game::HGSS),
        Some("pk5") => (PkmFormat::Gen5, Game::B2W2),
        Some("pk6") => (PkmFormat::Gen6, Game::ORAS),
        _ => (PkmFormat::Gen7, Game::USUM),
    };
    (Pokemon::from_bytes(format, &bytes).unwrap(), game)
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pkhex/legality")
}

fn summary(p: &Path, pk: &Pokemon, game: Game) -> String {
    let r = analyze(pk, game);
    let issues: Vec<String> =
        r.checks.iter().filter(|c| c.severity != super::Severity::Valid).map(|c| format!("[{:?}] {} — {}", c.severity, c.title, c.detail)).collect();
    format!("{} → {} ({})\n    {}", p.file_name().unwrap().to_string_lossy(), r.verdict_label, r.origin, issues.join("\n    "))
}

/// Taux de réussite sur les fichiers de PKHeX (légaux : pas « Illégal » ; illégaux : « Illégal »).
#[test]
fn pkhex_samples() {
    let mut legal = Vec::new();
    files(&root().join("Legal"), &mut legal);
    let mut illegal = Vec::new();
    files(&root().join("Illegal"), &mut illegal);
    assert!(legal.len() > 80 && illegal.len() > 25, "fichiers de test manquants");

    let mut ok_legal = 0;
    let mut report = String::new();
    for p in &legal {
        let (pk, game) = load(p);
        if analyze(&pk, game).verdict != Verdict::Illegal {
            ok_legal += 1;
        } else {
            report += &format!("FAUX ILLÉGAL : {}\n", summary(p, &pk, game));
        }
    }
    let mut ok_illegal = 0;
    for p in &illegal {
        let (pk, game) = load(p);
        if analyze(&pk, game).verdict == Verdict::Illegal {
            ok_illegal += 1;
        } else {
            report += &format!("NON DÉTECTÉ : {}\n", summary(p, &pk, game));
        }
    }
    println!("{report}");
    println!("Légaux reconnus : {ok_legal}/{} · Illégaux détectés : {ok_illegal}/{}", legal.len(), illegal.len());
    // Seuils minimaux (voir le rapport ci-dessus pour le détail).
    assert!(ok_legal * 100 >= legal.len() * 97, "trop de faux illégaux : {ok_legal}/{}", legal.len());
    assert!(ok_illegal * 100 >= illegal.len() * 70, "trop peu d'illégaux détectés : {ok_illegal}/{}", illegal.len());
}

fn has(game: Game, location: u16, species: u16) -> bool {
    encounters(game).iter().any(|e| e.location == location && e.species == species && e.kind.is_wild())
}

#[test]
fn route_201_and_first_routes() {
    // Platine, Route 201 (lieu 16) : Étourmi (396) et Keunotor (399).
    assert!(has(Game::Pt, 16, 396));
    assert!(has(Game::Pt, 16, 399));
    // Noir/Blanc, Route 1 (lieu 19 ? non : Route 1 = 4… on cherche Ponchiot et Ratentif).
    let bw = encounters(Game::BW);
    let route1: Vec<_> = bw.iter().filter(|e| e.species == 504 && e.kind.is_wild()).map(|e| e.location).collect();
    assert!(!route1.is_empty(), "Ratentif introuvable dans NB");
    assert!(bw.iter().any(|e| e.species == 506 && route1.contains(&e.location)), "Ponchiot et Ratentif partagent une route");
    // Soleil/Lune : Manglouton (734) et Picassaut (731) sur la Route 1.
    assert!(encounters(Game::SM).iter().any(|e| e.species == 734 && e.kind.is_wild()));
    // Dons et légendaires.
    assert!(encounters(Game::Pt).iter().any(|e| e.species == 487 && e.kind == EncounterKind::Static));
    assert!(encounters(Game::XY).iter().any(|e| e.kind == EncounterKind::FriendSafari));
    assert!(encounters(Game::HGSS).iter().any(|e| e.kind == EncounterKind::Pokewalker));
    assert!(encounters(Game::B2W2).iter().any(|e| e.kind == EncounterKind::DreamRadar));
    let trade = encounters(Game::Pt).iter().find(|e| e.kind == EncounterKind::Trade && e.species == 63).expect("échange Abra");
    assert!(trade.trainer.as_ref().is_some_and(|t| t.names.iter().any(|(l, n)| *l == 3 && !n.is_empty())));
}

fn trainer() -> Trainer {
    Trainer {
        name: "Thisma".into(),
        tid: 12345,
        sid: 54321,
        display_id: 12345,
        gender: Gender::Female,
        money: 0,
        play_time: PlayTime { hours: 0, minutes: 0, seconds: 0 },
    }
}

#[test]
fn generate_is_legal_every_generation() {
    let cases = [
        (Game::Pt, PkmFormat::Gen4, 396u16, 5u8),
        (Game::HGSS, PkmFormat::Gen4, 16, 10),
        (Game::BW, PkmFormat::Gen5, 504, 8),
        (Game::B2W2, PkmFormat::Gen5, 532, 20),
        (Game::XY, PkmFormat::Gen6, 661, 10),
        (Game::ORAS, PkmFormat::Gen6, 263, 12),
        (Game::SM, PkmFormat::Gen7, 734, 6),
        (Game::USUM, PkmFormat::Gen7, 731, 30),
        // Légendaire fixe et Pokémon seulement par reproduction.
        (Game::Pt, PkmFormat::Gen4, 487, 50),
        (Game::XY, PkmFormat::Gen6, 1, 5),
    ];
    for (game, format, species, level) in cases {
        let req = GenerateRequest { species, level, ..Default::default() };
        let out = generate_legal(game, format, &trainer(), &req).unwrap();
        let bad: Vec<_> =
            out.report.checks.iter().filter(|c| c.severity == super::Severity::Invalid).map(|c| format!("{} — {}", c.title, c.detail)).collect();
        assert!(out.success, "{game:?} n°{species} : {bad:?}");
        assert_eq!(out.pokemon.species(), species);
    }
}

/// « Créer ce Pokémon » depuis la base, y compris une rencontre d'un jeu plus ancien (transfert).
#[test]
fn generate_from_database_entries() {
    use super::db::{encounter_db, games_for, species_entries, species_index};
    let index = species_index(&games_for(Game::USUM, false));
    assert!(index.len() > 300, "{} espèces", index.len());
    assert!(index.iter().any(|s| s.species == 731 && s.families.contains(&"herbes")));
    // Toutes les jeux transférables vers USUL : Étourmi existe (Platine…).
    let all = games_for(Game::USUM, true);
    let starly = species_entries(&all, 396);
    let pt = starly.iter().find(|e| e.game == Game::Pt && e.family == "herbes").expect("Étourmi dans Platine");
    for (game, format, entry) in [(Game::Pt, PkmFormat::Gen4, pt.clone()), (Game::USUM, PkmFormat::Gen7, pt.clone())] {
        let req = GenerateRequest {
            species: entry.species,
            level: entry.level_min.max(5),
            encounter_index: Some(entry.index),
            encounter_game: Some("pt".into()),
            ..Default::default()
        };
        let out = generate_legal(game, format, &trainer(), &req).unwrap();
        assert!(out.success, "{game:?} : {:?}", out.report.checks);
        assert_eq!(out.pokemon.version(), crate::legality::encounters::PT, "{game:?}");
    }
    // Chaque type de rencontre de chaque jeu produit un Pokémon légal (quelques entrées par type).
    let mut failures = Vec::new();
    let mut total = 0;
    for game in Game::ALL {
        // Gen 3 : génération de Pokémon légaux pas encore prise en charge.
        if game.generation() < 4 {
            continue;
        }
        let format = match game.generation() {
            4 => PkmFormat::Gen4,
            5 => PkmFormat::Gen5,
            6 => PkmFormat::Gen6,
            _ => PkmFormat::Gen7,
        };
        let mut by_kind: std::collections::HashMap<_, Vec<_>> = std::collections::HashMap::new();
        for e in encounter_db(game) {
            let list = by_kind.entry(e.kind).or_default();
            if list.len() < if e.kind == EncounterKind::Event { 30 } else { 6 } {
                list.push(e.clone());
            }
        }
        for (kind, list) in by_kind {
            for e in list {
                total += 1;
                let req = GenerateRequest {
                    species: e.species,
                    form: if e.form >= 30 { 0 } else { e.form },
                    level: e.level_max.max(e.level_min),
                    encounter_index: Some(e.index),
                    ..Default::default()
                };
                let out = generate_legal(game, format, &trainer(), &req).unwrap();
                if !out.success {
                    let bad: Vec<String> = out
                        .report
                        .checks
                        .iter()
                        .filter(|c| c.severity == super::Severity::Invalid)
                        .map(|c| format!("{} — {}", c.title, c.detail))
                        .collect();
                    failures.push(format!("{game:?} {kind:?} n°{} {} : {bad:?}", e.species, e.location_name));
                } else if !e.egg && out.pokemon.met_location() != e.location {
                    failures.push(format!(
                        "{game:?} {kind:?} n°{} : autre rencontre choisie ({} au lieu de {})",
                        e.species,
                        out.pokemon.met_location(),
                        e.location
                    ));
                }
            }
        }
    }
    println!("{}", failures.join("\n"));
    println!("Création depuis la base : {}/{total} légaux", total - failures.len());
    assert!(failures.len() * 20 <= total, "trop d'échecs : {}/{total}", failures.len());
}

#[test]
fn generate_shiny_gen4_keeps_method1() {
    let req = GenerateRequest { species: 396, level: 5, shiny: Some(true), nature: Some(3), ..Default::default() };
    let out = generate_legal(Game::Pt, PkmFormat::Gen4, &trainer(), &req).unwrap();
    assert!(out.success, "{:?}", out.report.checks);
    let pk = &out.pokemon;
    assert_eq!(pk.nature(), 3);
    let pid = pk.pid();
    assert!(((pid >> 16) ^ (pid & 0xFFFF) ^ pk.tid() as u32 ^ pk.sid() as u32) < 8, "chromatique attendu");
    assert_eq!(super::legalize::pid_type(pk), super::rng::PidType::Method1);
}

#[test]
fn legalize_broken_pokemon() {
    // Étourmi de Platine volontairement cassé : attaque impossible, lieu faux, Ball de Maître… puis « Rendre légal ».
    let req = GenerateRequest { species: 396, level: 12, ..Default::default() };
    let good = generate_legal(Game::Pt, PkmFormat::Gen4, &trainer(), &req).unwrap().pokemon;
    let mut broken = good.clone();
    broken.set_moves([94, 33, 0, 0]); // Psyko : impossible pour Étourmi
    broken.set_met_location(200);
    broken.set_ball(16); // Mémoire Ball
    broken.set_fateful_encounter(true);
    broken.set_evs([255, 255, 255, 0, 0, 0]);
    broken.refresh_checksum();
    assert_eq!(analyze(&broken, Game::Pt).verdict, Verdict::Illegal);
    let out = legalize(&broken, Game::Pt, &trainer());
    assert!(out.success, "{:?}", out.report.checks);
    assert!(!out.changes.is_empty());
    assert_ne!(analyze(&out.pokemon, Game::Pt).verdict, Verdict::Illegal);
    assert!(!out.pokemon.moves().contains(&94));

    // Gen 7 : verrou chromatique d'un Tokorico chromatique.
    let req = GenerateRequest { species: 785, level: 60, ..Default::default() };
    let out = generate_legal(Game::SM, PkmFormat::Gen7, &trainer(), &req).unwrap();
    assert!(out.success);
    let mut shiny = out.pokemon.clone();
    shiny.set_pid(((shiny.tid() ^ shiny.sid()) as u32) << 16);
    shiny.refresh_checksum();
    assert_eq!(analyze(&shiny, Game::SM).verdict, Verdict::Illegal);
    let fixed = legalize(&shiny, Game::SM, &trainer());
    assert!(fixed.success, "{:?}", fixed.report.checks);
}

/// Vraie sauvegarde Soleil/Lune des tests de PKHeX : tout vérifier (et vite).
#[test]
fn real_save_check_all() {
    let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pkhex/sm_project_802.main")).unwrap();
    let session = crate::save::session::SaveSession::open(&bytes).unwrap();
    let game = session.game();
    let start = std::time::Instant::now();
    let mut counts = [0usize; 3];
    let mut lines = Vec::new();
    let mut slots = Vec::new();
    for i in 0..session.save.party_count() {
        slots.push(crate::save::session::Slot::Party { index: i });
    }
    for b in 0..session.save.box_count() {
        for index in 0..crate::save::BOX_SLOTS {
            slots.push(crate::save::session::Slot::Box { r#box: b, index });
        }
    }
    for slot in slots {
        let Ok(Some(pk)) = session.get(slot) else { continue };
        let r = analyze(&pk, game);
        counts[r.verdict as usize] += 1;
        if std::env::var("KALEIDO_DUMP").is_ok_and(|f| crate::dex::species_name(pk.species()) == Some(f.as_str())) {
            println!(
                "{} forme {} niv {} rencontre {}@{} œuf {} version {} ball {} pid {:08X} ec {:08X} talent {} n {} fatidique {} dresseur {:?} {}/{} langue {} attaques {:?} {:?}",
                pk.species(),
                pk.form(),
                super::verify::growth_level(&pk, game),
                pk.met_level(),
                pk.met_location(),
                pk.egg_location(),
                pk.version(),
                pk.ball(),
                pk.pid(),
                pk.encryption_constant(),
                pk.ability(),
                pk.ability_number(),
                pk.fateful_encounter(),
                pk.ot_name(),
                pk.tid(),
                pk.sid(),
                pk.language(),
                pk.moves(),
                r.checks.iter().map(|c| format!("{} : {}", c.title, c.detail)).collect::<Vec<_>>()
            );
            for e in super::events::events(4).iter().chain(super::events::events(5)).filter(|e| e.species == pk.species()) {
                println!(
                    "    distribution {:?} niv {} lieu {} {:?}",
                    e.title,
                    e.level_min,
                    e.location,
                    e.trainer.as_ref().map(|t| (t.tid, t.sid, t.names.clone()))
                );
            }
        }
        if r.verdict == Verdict::Illegal {
            let bad: Vec<String> = r.checks.iter().filter(|c| c.severity == super::Severity::Invalid).map(|c| c.title.clone()).collect();
            lines.push(format!("{} : {}", crate::dex::species_name(pk.species()).unwrap_or("?"), bad.join(", ")));
        }
    }
    let elapsed = start.elapsed();
    println!("{}", lines.join("\n"));
    println!("SL : {} légaux, {} douteux, {} illégaux en {elapsed:?}", counts[0], counts[1], counts[2]);
    assert!(counts.iter().sum::<usize>() > 0);
}

/// Détail d'un fichier de test : `KALEIDO_DUMP=motif cargo test debug_dump -- --ignored --nocapture`.
#[test]
#[ignore]
fn debug_dump() {
    let mut all = Vec::new();
    files(&root(), &mut all);
    let filter = std::env::var("KALEIDO_DUMP").unwrap_or_default();
    for p in all.iter().filter(|p| p.to_string_lossy().contains(&filter)) {
        let (pk, game) = load(p);
        println!(
            "{}\n  espèce {} forme {} niv {} rencontre {}@{} œuf {} version {} ball {} pid {:08X} ec {:08X} tid {} sid {} talent {} n {} nature {} sexe {:?} fatidique {} langue {} surnom {:?} dresseur {:?} attaques {:?} réapprendre {:?} iv {:?} pid/iv {:?}",
            p.display(),
            pk.species(),
            pk.form(),
            super::verify::growth_level(&pk, game),
            pk.met_level(),
            pk.met_location(),
            pk.egg_location(),
            pk.version(),
            pk.ball(),
            pk.pid(),
            pk.encryption_constant(),
            pk.tid(),
            pk.sid(),
            pk.ability(),
            pk.ability_number(),
            pk.nature(),
            pk.gender(),
            pk.fateful_encounter(),
            pk.language(),
            pk.nickname(),
            pk.ot_name(),
            pk.moves(),
            super::verify::relearn_moves(&pk),
            pk.ivs(),
            super::legalize::pid_type(&pk)
        );
        println!("  {}", summary(p, &pk, game));
        for e in (4..=7).flat_map(super::events::events).filter(|e| e.species == pk.species()) {
            println!(
                "    distribution {:?} forme {} niv {} lieu {} versions {:?} talent {:?} chromatique {:?} {:?}",
                e.title,
                e.form,
                e.level_min,
                e.location,
                e.versions,
                e.ability,
                e.shiny,
                e.trainer.as_ref().map(|t| (t.tid, t.sid, t.names.clone()))
            );
        }
    }
}

#[test]
fn legal_samples_stay_legal_after_legalize() {
    let mut legal = Vec::new();
    files(&root().join("Legal"), &mut legal);
    for p in legal.iter().take(30) {
        let (pk, game) = load(p);
        if analyze(&pk, game).verdict == Verdict::Illegal {
            continue;
        }
        let out = legalize(&pk, game, &trainer());
        assert!(out.changes.is_empty(), "{} modifié : {:?}", p.display(), out.changes);
    }
}

fn codes(pk: &Pokemon, game: Game) -> Vec<&'static str> {
    analyze(pk, game).checks.iter().filter(|c| c.severity != super::Severity::Valid).map(|c| c.code).collect()
}

fn patched(pk: &Pokemon, patch: crate::save::pkm::ExtrasPatch) -> Pokemon {
    let mut p = pk.clone();
    p.apply_extras(&patch).unwrap();
    p.refresh_checksum();
    p
}

/// Rubans, Hyper Training, pays et souvenirs : règles reprises de PKHeX.
#[test]
fn extras_rules() {
    use crate::save::pkm::{ExtrasPatch, Memory};
    // Métamorph de Soleil/Lune (né en Gen 7), légal tel quel.
    let (pk, game) = load(&root().join("Legal/Generation 7 Static/132 - Ditto - D2606D168C32.pk7"));
    let base = codes(&pk, game);
    assert!(analyze(&pk, game).verdict != Verdict::Illegal, "{base:?}");

    let kalos = patched(&pk, ExtrasPatch { ribbons: Some([("RibbonChampionKalos".to_string(), true)].into()), ..Default::default() });
    assert!(codes(&kalos, game).contains(&"ribbon"), "Maître de Kalos sur un Pokémon de Gen 7");
    let alola = patched(&pk, ExtrasPatch { ribbons: Some([("RibbonChampionAlola".to_string(), true)].into()), ..Default::default() });
    assert!(!codes(&alola, game).contains(&"ribbon"), "Maître d'Alola possible en Gen 7");

    let ht = patched(&pk, ExtrasPatch { hyper_training: Some([true, false, false, false, false, false]), ..Default::default() });
    let ht_codes = codes(&ht, game);
    assert!(ht_codes.contains(&"hyper-level") || ht_codes.contains(&"hyper-perfect"), "{ht_codes:?}");

    let geo = patched(&pk, ExtrasPatch { geo: Some([[0, 0], [1, 49], [0, 0], [0, 0], [0, 0]]), ..Default::default() });
    assert!(codes(&geo, game).contains(&"geo"));

    let mem = patched(&pk, ExtrasPatch { ot_memory: Some(Memory { id: 63, intensity: 7, feeling: 1, variable: 0 }), ..Default::default() });
    assert!(codes(&mem, game).contains(&"memory-ot"), "pas de souvenir de dresseur d'origine en Gen 7");

    let medals = patched(&pk, ExtrasPatch { medals: Some(vec![true]), ..Default::default() });
    assert!(codes(&medals, game).contains(&"super-training"));
}

/// Un Pokémon créé en X/Y reçoit un souvenir avec son dresseur d'origine, comme en jeu.
#[test]
fn generated_gen6_has_ot_memory() {
    let req = GenerateRequest { species: 661, level: 10, ..Default::default() };
    let out = generate_legal(Game::XY, PkmFormat::Gen6, &trainer(), &req).unwrap();
    assert_ne!(out.pokemon.extras().handler.unwrap().ot_memory.id, 0);
    assert!(!codes(&out.pokemon, Game::XY).contains(&"memory-ot-missing"));
}
