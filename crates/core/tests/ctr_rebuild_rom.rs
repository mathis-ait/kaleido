//! Randomisation d'une vraie ROM 3DS vers LayeredFS + `.3ds` reconstruit : chaque
//! fichier modifié relu dans la nouvelle image doit être identique, à l'octet près, à
//! celui du dossier LayeredFS, les autres identiques à l'original, et tous les
//! hachages (ExHeader, ExeFS, superbloc, IVFC niveau par niveau) valides.
//!
//! Nécessite une ROM Rubis Oméga / Saphir Alpha déchiffrée : variable
//! `KALEIDO_3DS_ROM`, sinon le premier `.3ds` de `~/Documents/NDS & 3DS`. Le test est
//! ignoré (avec un message) si elle est absente.

use std::path::PathBuf;
use std::time::Instant;

use kaleido_core::randomizer::ctr::{self, CtrOutput, LayeredFsTarget};
use kaleido_core::randomizer::presets;
use kaleido_core::CtrGameRom;
use kaleido_formats::ctr_build::verify_image;
use kaleido_formats::romfs::RomFsSource;

fn find_rom() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KALEIDO_3DS_ROM").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let dir = PathBuf::from(home).join("Documents").join("NDS & 3DS");
    let mut roms: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("3ds"))).collect();
    roms.sort();
    roms.into_iter().next()
}

#[test]
fn randomized_rom3ds_matches_layeredfs() {
    let Some(rom) = find_rom() else {
        eprintln!("ROM 3DS absente : test ignoré (définir KALEIDO_3DS_ROM)");
        return;
    };
    let game = CtrGameRom::open(&rom).unwrap();
    if !ctr::supports(game.game) {
        eprintln!("{} : pas pris en charge par le randomizer 3DS, test ignoré", game.game.name_fr());
        return;
    }
    let out = std::env::temp_dir().join(format!("kaleido-rebuild-rando-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    let result = std::panic::catch_unwind(|| {
        // « Chaos total » + attaques et évolutions : un maximum de fichiers modifiés.
        let mut settings = presets().into_iter().find(|p| p.id == "chaos").unwrap().settings;
        settings.random_movesets = true;
        settings.easy_evolutions = true;
        settings.ctr_output = CtrOutput::Both;
        let t = Instant::now();
        let (outcome, written) = ctr::randomize(&game, &settings, 42, &out, LayeredFsTarget::Emulator).unwrap();
        eprintln!("randomisation + reconstruction : {:.1?}", t.elapsed());
        assert!(outcome.wild_slots > 0 && outcome.trainer_pokemon > 0);
        let (romfs, image) = (written.romfs.unwrap(), written.image.unwrap());
        assert!(image.extension().is_some_and(|e| e == "3ds"));
        eprintln!("{} ({} octets), {} fichiers modifiés : {:?}", image.display(), std::fs::metadata(&image).unwrap().len(), written.files.len(), written.files);
        assert!(written.files.len() >= 5);

        let t = Instant::now();
        let check = verify_image(&image).unwrap();
        eprintln!("vérification : {:.1?} — {check:?}", t.elapsed());
        assert!(check.ok());

        // Fichiers modifiés : identiques au LayeredFS ; les autres : identiques à l'original.
        let rebuilt = RomFsSource::open(&image).unwrap();
        let original = RomFsSource::open(&rom).unwrap();
        assert_eq!(rebuilt.title_id(), original.title_id());
        let paths = |s: &RomFsSource| s.files().iter().map(|f| f.path.clone()).collect::<Vec<_>>();
        assert_eq!(paths(&rebuilt), paths(&original));
        let mut differ = 0;
        for path in &written.files {
            let layered = std::fs::read(romfs.join(path)).unwrap();
            assert!(rebuilt.read(path).unwrap() == layered, "{path} : différent du LayeredFS");
            // Certains fichiers réécrits peuvent rester identiques (ex. `trdata`).
            differ += usize::from(original.read(path).unwrap() != layered);
        }
        eprintln!("{differ} fichiers réellement différents de l'original");
        assert!(differ >= 4);
        let t = Instant::now();
        let mut unchanged = 0;
        for f in original.files().iter().filter(|f| !written.files.contains(&f.path) && f.size < 64 << 20) {
            assert!(rebuilt.read(&f.path).unwrap() == original.read(&f.path).unwrap(), "{} : différent de l'original", f.path);
            unchanged += 1;
        }
        eprintln!("{unchanged} fichiers inchangés relus : {:.1?}", t.elapsed());
    });
    let _ = std::fs::remove_dir_all(&out);
    result.unwrap();
}
