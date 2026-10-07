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
        origin: Some([2, 77, 2]),
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
    // Platine, hautes herbes : chromatique du Poké Radar (PID bâti à partir des IV) ou méthode 1.
    let radar = super::rng::chain_shiny(pid, super::verify::iv32(pk), pk.tid(), pk.sid());
    assert!(radar || super::legalize::pid_type(pk) == super::rng::PidType::Method1);

    // HeartGold : pas de Poké Radar, méthode K avec le bon slot.
    let req = GenerateRequest { species: 16, level: 4, shiny: Some(true), nature: Some(10), ..Default::default() };
    let out = generate_legal(Game::HGSS, PkmFormat::Gen4, &trainer(), &req).unwrap();
    assert!(out.success, "{:?}", out.report.checks);
    assert_eq!(super::legalize::pid_type(&out.pokemon), super::rng::PidType::Method1);
    assert!(out.pokemon.is_shiny());
}

/// Les Pokémon sauvages créés en Gen 4 suivent les tirages des méthodes J et K (slot, niveau, nature).
#[test]
fn generated_gen4_wild_follow_method_jk() {
    use super::rng::{frame4, Lead4, Method4};
    let mut checked = 0;
    for (game, species, level) in [(Game::Pt, 396u16, 4u8), (Game::DP, 399, 4), (Game::HGSS, 16, 4), (Game::HGSS, 129, 20)] {
        let req = GenerateRequest { species, level, nature: Some(7), ..Default::default() };
        let out = generate_legal(game, PkmFormat::Gen4, &trainer(), &req).unwrap();
        assert!(out.success, "{game:?} n°{species} : {:?}", out.report.checks);
        let pk = &out.pokemon;
        let e = super::encounters::encounters(game)
            .iter()
            .find(|e| e.location == pk.met_location() && e.species == species && super::verify::area4(e.kind).is_some() && !e.slots.is_empty());
        let Some(e) = e else { continue };
        let slots: Vec<_> = e.slots.iter().map(|&slot| super::rng::Slot4 { area: super::verify::area4(e.kind).unwrap(), slot, level_min: e.level_min, level_max: e.level_max }).collect();
        let lead = frame4(Method4::of_version(pk.version()), &slots, Some(pk.met_level()), pk.pid(), super::verify::iv32(pk));
        assert_eq!(lead, Some(Lead4::None), "{game:?} n°{species} : {:?}", out.changes);
        assert!(out.changes.iter().any(|c| c.term == "methodJK"), "{:?}", out.changes);
        checked += 1;
    }
    assert!(checked >= 3, "{checked} vérifiés");
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
    // Mesure du PRD : tout le corpus légal reste légal (et intact) après « Rendre légal ».
    let mut kept = 0;
    let mut total = 0;
    for p in &legal {
        let (pk, game) = load(p);
        total += 1;
        let out = legalize(&pk, game, &trainer());
        if analyze(&pk, game).verdict != Verdict::Illegal {
            assert!(out.changes.is_empty(), "{} modifié : {:?}", p.display(), out.changes);
        }
        assert_ne!(analyze(&out.pokemon, game).verdict, Verdict::Illegal, "{} encore illégal", p.display());
        kept += 1;
    }
    println!("Corpus légal après « Rendre légal » : {kept}/{total} légaux");
}

/// Mesure du PRD : « Rendre légal » sur une équipe de 6 en moins de 2 s (mode release ;
/// la limite est relâchée en debug, où le code n'est pas optimisé).
#[test]
fn prd_team_of_six_is_fast() {
    let cases = [
        (Game::Pt, PkmFormat::Gen4, 396u16, 20u8),
        (Game::HGSS, PkmFormat::Gen4, 16, 25),
        (Game::B2W2, PkmFormat::Gen5, 532, 30),
        (Game::XY, PkmFormat::Gen6, 661, 30),
        (Game::USUM, PkmFormat::Gen7, 731, 40),
        (Game::Pt, PkmFormat::Gen4, 443, 40),
    ];
    let mut team = Vec::new();
    for (game, format, species, level) in cases {
        let req = GenerateRequest { species, level, ..Default::default() };
        let mut pk = generate_legal(game, format, &trainer(), &req).unwrap().pokemon;
        pk.set_moves([94, 0, 0, 0]);
        pk.set_ball(1);
        pk.set_met_location(1);
        pk.refresh_checksum();
        team.push((pk, game));
    }
    let start = std::time::Instant::now();
    let mut legal = 0;
    for (pk, game) in &team {
        let out = super::legalize::legalize_with(pk, *game, &trainer(), None, true);
        legal += out.success as usize;
        println!("  n°{} : {} modification(s), {} rencontre(s) possibles, {:?}", pk.species(), out.changes.len(), out.options.len(), out.changes.iter().map(|c| c.text.as_str()).collect::<Vec<_>>());
    }
    let elapsed = start.elapsed();
    println!("Équipe de 6 (aperçu complet) : {legal}/6 légaux en {elapsed:?}");
    let limit = if cfg!(debug_assertions) { 20.0 } else { 2.0 };
    assert!(elapsed.as_secs_f64() < limit, "{elapsed:?}");
    assert_eq!(legal, 6);
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

/// Mesure du PRD : part des Pokémon illégaux de PKHeX rendus légaux par « Rendre légal ».
#[test]
fn prd_illegal_corpus_legalized() {
    let mut illegal = Vec::new();
    files(&root().join("Illegal"), &mut illegal);
    let mut fixed = 0;
    let (mut detected, mut detected_fixed) = (0, 0);
    let mut lines = Vec::new();
    for p in &illegal {
        let (pk, game) = load(p);
        let seen = analyze(&pk, game).verdict == Verdict::Illegal;
        detected += seen as usize;
        let out = legalize(&pk, game, &trainer());
        if out.success && analyze(&out.pokemon, game).verdict != Verdict::Illegal {
            fixed += 1;
            detected_fixed += seen as usize;
        } else {
            lines.push(format!("ÉCHEC : {}", summary(p, &out.pokemon, game)));
        }
    }
    println!("{}", lines.join("\n"));
    println!("Illégaux rendus légaux : {fixed}/{} (dont {detected_fixed}/{detected} parmi ceux que Kaleido détecte)", illegal.len());
    assert!(fixed * 100 >= illegal.len() * 90, "cible du PRD : 90 % ({fixed}/{})", illegal.len());
}

/// Gen 5 à 7 : constante de chiffrement et PID du transfert, pays de la console, dates.
#[test]
fn gen67_transfer_geo_and_dates() {
    use crate::save::pkm::ExtrasPatch;
    use crate::save::PkmDate;
    // Étourmi de Platine transféré dans Ultra-Soleil : EC = PID d'origine, PID recalculé.
    let req = GenerateRequest { species: 396, level: 10, encounter_index: None, ..Default::default() };
    let pt = generate_legal(Game::Pt, PkmFormat::Gen4, &trainer(), &req).unwrap().pokemon;
    let mut moved = generate_legal(Game::USUM, PkmFormat::Gen7, &trainer(), &GenerateRequest { species: 396, level: 10, ..Default::default() }).unwrap();
    if moved.pokemon.version() == crate::legality::encounters::PT {
        let p = &moved.pokemon;
        assert_eq!(p.pid(), super::rng::transfer_pid(p.encryption_constant(), p.tid(), p.sid()));
    }
    let h = moved.pokemon.extras().handler.unwrap();
    assert_eq!((h.region, h.country, h.console_region), (2, 77, 2), "pays de la console du joueur");
    assert!(moved.pokemon.met_date().is_some());
    let _ = pt;

    // Pays incohérent et date avant la sortie du jeu : corrigés sur place.
    let mut p = moved.pokemon.clone();
    p.apply_extras(&ExtrasPatch { country: Some(1), console_region: Some(2), ..Default::default() }).unwrap();
    p.set_met_date(Some(PkmDate { year: 2001, month: 1, day: 1 }));
    p.refresh_checksum();
    assert!(codes(&p, Game::USUM).contains(&"geo-console"));
    assert!(codes(&p, Game::USUM).contains(&"met-date-early"));
    let out = legalize(&p, Game::USUM, &trainer());
    assert!(out.success, "{:?}", out.report.checks);
    let h = out.pokemon.extras().handler.unwrap();
    assert!(super::verify::extras::console_country_valid(h.console_region, h.country));
    let d = out.pokemon.met_date().unwrap();
    assert!(d.year >= 2017, "{d:?}");
    assert!(out.changes.iter().any(|c| c.term == "country"), "{:?}", out.changes);
    moved.pokemon = out.pokemon;
}

/// Set Smogon reposant sur une capacité Œuf : Azumarill Cognobidon (X/Y) reste légal, par l'œuf.
#[test]
fn egg_move_sets_are_kept() {
    let req = GenerateRequest {
        species: 184,
        level: 50,
        nature: Some(3),
        ability_number: Some(4),
        moves: Some([187, 453, 583, 276]), // Cognobidon, Aqua-Jet, Câlinerie, Surpuissance
        ivs: Some([31; 6]),
        ..Default::default()
    };
    let out = generate_legal(Game::XY, PkmFormat::Gen6, &trainer(), &req).unwrap();
    assert!(out.success, "{:?}", out.report.checks);
    assert!(out.pokemon.moves().contains(&187), "Cognobidon gardé : {:?}", out.adjustments);
    assert!(out.pokemon.egg_location() != 0, "obtenu par l'œuf");
    assert!(out.changes.iter().any(|c| c.term == "eggMoves"), "{:?}", out.changes);
    assert_eq!(out.pokemon.ivs(), [31; 6]);
}
