//! Pokémon X / Y sur une vraie ROM : formats lus (espèces, starters, rencontres,
//! dresseurs), puis randomisation complète vers LayeredFS + `.3ds` reconstruit et
//! relecture de l'image produite.
//!
//! Nécessite une ROM Y (ou X) déchiffrée : variable `KALEIDO_XY_ROM`, sinon
//! `Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds` dans `KALEIDO_ROMS` ou
//! `~/Documents/NDS & 3DS`. Les tests sont ignorés (avec un message) si elle est absente.

use std::path::PathBuf;

use kaleido_core::data::{encounters, trainers};
use kaleido_core::randomizer::ctr::{self, CtrOutput, LayeredFsTarget};
use kaleido_core::randomizer::{presets, Settings, StarterMode};
use kaleido_core::{CtrGameRom, Game};
use kaleido_formats::ctr_build::verify_image;
use kaleido_formats::garc::Garc;
use kaleido_formats::lz;
use kaleido_formats::romfs::RomFsSource;

fn find_rom() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KALEIDO_XY_ROM").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let dir = std::env::var_os("KALEIDO_ROMS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join("Documents").join("NDS & 3DS"))
    })?;
    let p = dir.join("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds");
    p.is_file().then_some(p)
}

fn open() -> Option<CtrGameRom> {
    let Some(rom) = find_rom() else {
        eprintln!("ROM Pokémon X / Y absente : test ignoré (définir KALEIDO_XY_ROM)");
        return None;
    };
    let game = CtrGameRom::open(&rom).unwrap();
    assert!(matches!(game.game, Game::X | Game::Y));
    Some(game)
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

/// Équipe d'un dresseur X / Y : (espèce, niveau).
fn team(trdata: &Garc, trpoke: &Garc, i: usize) -> Vec<(u16, u16)> {
    let d = trainers::xy_trdata_as_oras(trdata.file(i).unwrap());
    let t = trainers::read_team(6, &d, trpoke.file(i).unwrap()).unwrap();
    t.pokemon.iter().map(|p| (p.species, p.level)).collect()
}

/// Emplacements (espèce) d'une zone de l'archive des rencontres.
fn zone_species(garc: &Garc, i: usize) -> Vec<u16> {
    let zone = lz::decompress(garc.file(i).unwrap()).unwrap();
    assert!(encounters::xy_section(&zone).is_some());
    encounters::read(6, &zone).iter().map(|s| s.species).collect()
}

#[test]
fn xy_formats() {
    let Some(game) = open() else { return };
    let l = game.layout;
    assert!(ctr::supports(game.game) && l.verified);

    // Espèces : noms et fiches cohérents.
    let species = game.species().unwrap();
    let find = |id: u16| species.iter().find(|s| s.id == id).unwrap();
    assert_eq!(find(650).name, "Marisson");
    assert_eq!(find(653).name, "Feunnec");
    assert_eq!(find(656).name, "Grenousse");
    assert_eq!(find(717).name, "Yveltal");

    // Starters : réglages par défaut → starters d'origine ; table des dons de DllField.cro.
    let settings = Settings { starters: StarterMode::Unchanged, ..Settings::default() };
    let starters: Vec<u16> = ctr::preview_starters(&game, &settings, 1).unwrap().iter().map(|p| p.id).collect();
    assert_eq!(starters, [650, 653, 656]);
    let cro = game.romfs().read("DllField.cro").unwrap();
    let gifts: Vec<(u16, u8)> = (0..6).map(|i| (u16_at(&cro, 0xF805C + i * 0x18), cro[0xF805C + i * 0x18 + 5])).collect();
    assert_eq!(gifts, [(650, 5), (653, 5), (656, 5), (1, 10), (4, 10), (7, 10)]);

    // Rencontres : 53 zones (comme l'Universal Pokémon Randomizer) ; Route 2 = zone 259.
    let garc = game.garc(l.encounters).unwrap();
    let zones = (0..garc.len())
        .filter(|&i| garc.file(i).is_some_and(lz::is_lz11))
        .filter(|&i| lz::decompress(garc.file(i).unwrap()).is_ok_and(|z| encounters::xy_section(&z).is_some()))
        .count();
    assert_eq!(zones, 53);
    let route2 = zone_species(&garc, 259);
    for s in [659, 263, 661, 16, 664, 10] {
        assert!(route2.contains(&s), "Route 2 : espèce {s} absente ({route2:?})");
    }

    // Dresseurs : Violette (n°6) et Dianthéa (n°276).
    let (trdata, trpoke) = (game.garc(l.trainer_data).unwrap(), game.garc(l.trainer_pokemon).unwrap());
    assert_eq!(trdata.file(6).unwrap().len(), 0x14);
    assert_eq!(team(&trdata, &trpoke, 6), [(283, 10), (666, 12)]);
    assert_eq!(team(&trdata, &trpoke, 276), [(701, 64), (697, 65), (699, 65), (711, 65), (706, 66), (282, 68)]);
    let names = game.text_file(l.trainer_names).unwrap();
    assert_eq!((names[6].as_str(), names[276].as_str()), ("Violette", "Dianthéa"));
}

/// Les zones de Rubis Oméga / Saphir Alpha ne doivent jamais être prises pour des zones X / Y.
#[test]
fn oras_zones_not_read_as_xy() {
    let dir = std::env::var_os("KALEIDO_ROMS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join("Documents").join("NDS & 3DS"))
    });
    let Some(rom) = dir.map(|d| d.join("Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2).3ds")).filter(|p| p.is_file()) else {
        eprintln!("ROM Rubis Oméga absente : test ignoré");
        return;
    };
    let game = CtrGameRom::open(&rom).unwrap();
    let garc = game.garc(game.layout.encounters).unwrap();
    let mut zones = 0;
    for i in 0..garc.len() {
        let raw = garc.file(i).unwrap();
        let data = if lz::is_lz11(raw) { lz::decompress(raw).unwrap_or_default() } else { raw.to_vec() };
        if data.starts_with(b"ZO") {
            zones += 1;
            assert!(encounters::xy_section(&data).is_none(), "zone {i}");
        }
    }
    assert!(zones > 100);
}

#[test]
fn xy_randomize_layeredfs_and_rom3ds() {
    let Some(game) = open() else { return };
    let l = game.layout;
    let rom = game.romfs().image_path().unwrap().to_path_buf();
    let out = std::env::temp_dir().join(format!("kaleido-xy-rando-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    let result = std::panic::catch_unwind(|| {
        let mut settings = presets().into_iter().find(|p| p.id == "chaos").unwrap().settings;
        settings.random_movesets = true;
        settings.easy_evolutions = true;
        settings.starters = StarterMode::Random;
        settings.ctr_output = CtrOutput::Both;
        let (outcome, written) = ctr::randomize(&game, &settings, 7, &out, LayeredFsTarget::Luma).unwrap();
        // 53 zones de l'archive + 62 rencontres fixées de DllField.cro.
        assert!(outcome.wild_slots > 1000 && outcome.trainer_pokemon > 1000, "{} / {}", outcome.wild_slots, outcome.trainer_pokemon);
        assert!(outcome.log.contains("DllField.cro"));
        assert!(!outcome.log.contains("non reconnues"));
        let starters: Vec<u16> = outcome.starters.iter().map(|p| p.id).collect();
        assert_ne!(starters, [650, 653, 656]);
        for f in [l.personal, l.levelup, l.evolution, l.encounters, l.trainer_data, l.trainer_pokemon, l.text, "DllField.cro", "DllPoke3Select.cro"] {
            assert!(written.files.iter().any(|p| p == f), "{f} non écrit ({:?})", written.files);
        }
        // Un fichier n'est écrit qu'une fois (DllField.cro : starters + rencontres fixées).
        let mut unique = written.files.clone();
        unique.dedup();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), written.files.len());

        // Image reconstruite : hachages valides, fichiers identiques au LayeredFS.
        let (romfs, image) = (written.romfs.unwrap(), written.image.unwrap());
        assert!(romfs.ends_with("luma/titles/0004000000055E00/romfs") || game.game == Game::X);
        assert!(verify_image(&image).unwrap().ok());
        let rebuilt = RomFsSource::open(&image).unwrap();
        for path in &written.files {
            assert!(rebuilt.read(path).unwrap() == std::fs::read(romfs.join(path)).unwrap(), "{path} : différent du LayeredFS");
        }

        // Relecture de la ROM produite.
        let new = CtrGameRom::from_romfs(rebuilt).unwrap();
        assert_eq!(new.game, game.game);
        let cro = new.romfs().read("DllField.cro").unwrap();
        let gifts: Vec<u16> = (0..3).map(|i| u16_at(&cro, 0xF805C + i * 0x18)).collect();
        assert_eq!(gifts, starters);
        let display = new.romfs().read("DllPoke3Select.cro").unwrap();
        let table = u16_at(&display, 0xB8) as usize + 0x10;
        let shown: Vec<u16> = (0..3).map(|i| u16_at(&display, table + i * 0x54)).collect();
        assert_eq!(shown, starters);
        let text = new.text_file(63).unwrap();
        assert!(text[1].starts_with("Pokémon de type "));
        let (old_garc, new_garc) = (game.garc(l.encounters).unwrap(), new.garc(l.encounters).unwrap());
        assert_eq!(zone_species(&new_garc, 259).len(), zone_species(&old_garc, 259).len());
        assert_ne!(zone_species(&new_garc, 259), zone_species(&old_garc, 259));
        let (trdata, trpoke) = (new.garc(l.trainer_data).unwrap(), new.garc(l.trainer_pokemon).unwrap());
        let viola = team(&trdata, &trpoke, 6);
        assert_eq!(viola.len(), 2);
        assert_ne!(viola, [(283, 10), (666, 12)]);
        assert!(new.species().unwrap().len() >= 721);
        let original = RomFsSource::open(&rom).unwrap();
        assert_eq!(new.romfs().files().len(), original.files().len());
    });
    let _ = std::fs::remove_dir_all(&out);
    result.unwrap();
}
