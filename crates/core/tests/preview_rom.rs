//! Aperçu « Nouvelle aventure » sur de vraies ROMs : starters, première route et premier
//! champion lus dans le résultat randomisé, identiques d'un appel à l'autre (même seed),
//! et mêmes starters que la randomisation écrite ensuite.
//!
//! ROMs cherchées dans `KALEIDO_ROMS` ou `~/Documents/NDS & 3DS` ; un jeu absent est ignoré.
//! La durée est affichée : l'objectif du PRD est de rester sous 3 s (build optimisé).

use std::path::PathBuf;

use kaleido_core::randomizer::preview::{preview_ctr, preview_nds, Preview};
use kaleido_core::randomizer::{presets, randomize, Settings};
use kaleido_core::GameRom;

fn rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("KALEIDO_ROMS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join("Documents").join("NDS & 3DS"))
    })?;
    let p = dir.join(name);
    if p.is_file() {
        Some(p)
    } else {
        eprintln!("ignoré : {name} absent");
        None
    }
}

fn settings() -> Settings {
    // « Équilibré » : starters, sauvages et dresseurs randomisés.
    presets().into_iter().find(|p| p.id == "equilibre").unwrap().settings
}

fn check(p: &Preview, label: &str) {
    println!(
        "{label} : {} ms · starters {:?} · {} · {}",
        p.elapsed_ms,
        p.starters.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        p.first_route.as_ref().map_or("—".to_string(), |r| format!("{} {:?}", r.name, r.encounters.iter().map(|e| e.name.as_str()).collect::<Vec<_>>())),
        p.first_leader.as_ref().map_or("—".to_string(), |l| format!("{} {:?}", l.name, l.team.iter().map(|e| (e.name.as_str(), e.min_level)).collect::<Vec<_>>())),
    );
    assert_eq!(p.starters.len(), 3, "{label}");
    assert!(p.first_route.as_ref().is_some_and(|r| !r.encounters.is_empty()), "{label} : première route");
    assert!(p.first_leader.as_ref().is_some_and(|l| !l.team.is_empty()), "{label} : premier champion");
    assert!(p.share_code.starts_with("KLD1-"));
}

#[test]
fn apercu_platine() {
    let Some(path) = rom("Pokemon - Platinum Version (Europe).nds") else { return };
    let a = preview_nds(&path, &settings(), 1234).unwrap();
    check(&a, "Platine");
    let b = preview_nds(&path, &settings(), 1234).unwrap();
    assert_eq!(format!("{:?}", a.first_route), format!("{:?}", b.first_route), "même seed, même aperçu");
    // La ROM écrite ensuite donne exactement les starters de l'aperçu.
    let mut game = GameRom::open(&path).unwrap();
    let written = randomize(&mut game, &settings(), 1234).unwrap();
    assert_eq!(written.starters.iter().map(|s| s.id).collect::<Vec<_>>(), a.starters.iter().map(|s| s.id).collect::<Vec<_>>());
    let other = preview_nds(&path, &settings(), 99).unwrap();
    assert_ne!(format!("{:?}", other.starters), format!("{:?}", a.starters), "« Relancer le tirage » change la partie");
}

fn ctr(name: &str, label: &str) {
    let Some(path) = rom(name) else { return };
    let scratch = std::env::temp_dir().join(format!("kaleido-preview-{}-{label}", std::process::id()));
    let a = preview_ctr(&path, &settings(), 1234, &scratch).unwrap();
    check(&a, label);
    let b = preview_ctr(&path, &settings(), 1234, &scratch).unwrap();
    assert_eq!(format!("{:?}", a.first_leader), format!("{:?}", b.first_leader), "même seed, même aperçu");
    assert!(!scratch.exists(), "le dossier temporaire est supprimé");
}

#[test]
fn apercu_y() {
    ctr("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds", "Y");
}

#[test]
fn apercu_ultra_soleil() {
    ctr("Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds", "Ultra-Soleil");
}

/// Un mod LayeredFS de Kaleido (avec `kaleido-base.txt`) se relit comme un jeu complet :
/// le Nuzlocke et le compagnon voient les rencontres randomisées, comme l'aperçu.
#[test]
fn mod_relu_par_dessus_le_jeu() {
    use kaleido_core::formats::romfs::MOD_BASE_FILE;
    use kaleido_core::nuzlocke::rom_ctr;
    use kaleido_core::randomizer::ctr::{randomize, LayeredFsTarget};
    use kaleido_core::CtrGameRom;
    let Some(path) = rom("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds") else { return };
    let out = std::env::temp_dir().join(format!("kaleido-mod-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let game = CtrGameRom::open(&path).unwrap();
    let (_, written) = randomize(&game, &settings(), 77, &out, LayeredFsTarget::Emulator).unwrap();
    let romfs = written.romfs.unwrap();
    std::fs::write(romfs.parent().unwrap().join(MOD_BASE_FILE), path.display().to_string()).unwrap();
    let info = rom_ctr::read_path(&romfs).unwrap();
    let scratch = out.join("apercu");
    let preview = preview_ctr(&path, &settings(), 77, &scratch).unwrap();
    let mut routes: Vec<_> = info.routes.iter().filter(|r| !r.encounters.is_empty()).collect();
    routes.sort_by_key(|r| r.order);
    let first = preview.first_route.unwrap();
    assert_eq!(routes[0].name, first.name);
    assert_eq!(routes[0].encounters.iter().map(|e| e.species).collect::<Vec<_>>(), first.encounters.iter().map(|e| e.species).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(&out);
}
