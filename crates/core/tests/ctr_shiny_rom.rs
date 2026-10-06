//! Taux de chromatiques sur de vraies ROM 3DS (X / Y, ROSA, Soleil / Lune, Ultra) :
//! le `code.ips` écrit pour LayeredFS, appliqué au programme d'origine, doit donner
//! le même programme que celui de la ROM `.3ds` reconstruite, dont tous les hachages
//! (ExHeader, ExeFS, RomFS) doivent rester valides.
//!
//! Utilise les `.3ds` déchiffrés de `~/Documents/NDS & 3DS` (ou du dossier
//! `KALEIDO_3DS_DIR`) ; ignoré (avec un message) s'il n'y en a aucun.

use std::path::PathBuf;

use kaleido_core::data::shiny_ctr::{self, Plan};
use kaleido_core::randomizer::ctr::{self, CtrOutput, LayeredFsTarget};
use kaleido_core::randomizer::Settings;
use kaleido_core::CtrGameRom;
use kaleido_formats::ctr_build::verify_image;

fn roms() -> Vec<PathBuf> {
    let dir = std::env::var_os("KALEIDO_3DS_DIR").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join("Documents").join("NDS & 3DS"))
    });
    let Some(Ok(entries)) = dir.map(std::fs::read_dir) else { return Vec::new() };
    let mut roms: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("3ds")) && !p.to_string_lossy().contains("Kaleido"))
        .collect();
    roms.sort();
    roms
}

#[test]
fn shiny_rate_ips_and_rebuilt_rom_agree() {
    let roms = roms();
    if roms.is_empty() {
        eprintln!("aucune ROM 3DS : test ignoré (définir KALEIDO_3DS_DIR)");
    }
    for rom in roms {
        let game = CtrGameRom::open(&rom).unwrap();
        if !ctr::supports(game.game) {
            continue;
        }
        let original = game.code().unwrap().code;
        assert_eq!(shiny_ctr::current(&original).unwrap(), Plan::Unchanged, "{}", game.game.name_fr());

        let out = std::env::temp_dir().join(format!("kaleido-shiny-{}-{}", game.title_id(), std::process::id()));
        // Seul le taux change : aucun fichier du RomFS n'est modifié.
        let settings = Settings { shiny_odds: 512, ctr_output: CtrOutput::Both, ..Settings::default() };
        let (outcome, written) = ctr::randomize(&game, &settings, 1, &out, LayeredFsTarget::Emulator).unwrap();
        assert!(outcome.log.contains("== Chromatiques =="), "{}", outcome.log);
        assert!(written.files.is_empty(), "{:?}", written.files);

        // LayeredFS : <TID>/code.ips appliqué au programme d'origine.
        let romfs = written.romfs.unwrap();
        let ips = std::fs::read(romfs.parent().unwrap().join("code.ips")).unwrap();
        assert!(ips.len() < 64, "{} octets", ips.len());
        let mut patched = original.clone();
        kaleido_formats::ips::apply(&mut patched, &ips).unwrap();
        let Plan::Rerolls(n) = shiny_ctr::current(&patched).unwrap() else { panic!("{}", game.game.name_fr()) };
        assert!((480..=540).contains(&shiny_ctr::effective_odds(n)), "{n} tirages");

        // ROM complète : même programme, hachages valides.
        let image = written.image.unwrap();
        let check = verify_image(&image).unwrap();
        assert!(check.ok(), "{}: {check:?}", game.game.name_fr());
        let rebuilt = CtrGameRom::open(&image).unwrap();
        assert_eq!(rebuilt.code().unwrap().code, patched, "{}", game.game.name_fr());
        assert_eq!(rebuilt.romfs().files().len(), game.romfs().files().len());
        eprintln!("{} : {n} tirages, ROM {} octets", game.game.name_fr(), std::fs::metadata(&image).unwrap().len());

        // « Tous » : le cas « chromatique forcé » devient inconditionnel.
        let mut always = original.clone();
        assert_eq!(shiny_ctr::apply(&mut always, Plan::Always).unwrap(), 1);
        assert_eq!(shiny_ctr::current(&always).unwrap(), Plan::Always);
        assert_eq!(always.iter().zip(&original).filter(|(a, b)| a != b).count(), 1);
        std::fs::remove_dir_all(&out).unwrap();
    }
}
