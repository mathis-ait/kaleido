//! Éditeur de ROM 3DS sur les vraies ROM (X / Y, Rubis Oméga, Lune, Ultra-Soleil) :
//! lecture, modification d'une espèce, sauvegarde en LayeredFS + `.3ds` reconstruit,
//! puis relecture de l'image produite.
//!
//! Les ROM déchiffrées sont cherchées dans `KALEIDO_ROMS`, sinon `~/Documents/NDS & 3DS` ;
//! chaque jeu absent est ignoré (avec un message). Les fichiers produits sont écrits dans
//! un dossier temporaire, supprimé à la fin.

use std::path::PathBuf;

use kaleido_core::data::learnsets;
use kaleido_core::pokemon::{Personal, PokeType};
use kaleido_core::randomizer::ctr::{CtrOutput, LayeredFsTarget};
use kaleido_core::romedit::LevelMove;
use kaleido_core::{romedit_ctr, CtrGameRom, Game};
use kaleido_formats::ctr_build::verify_image;
use kaleido_formats::garc::Garc;
use kaleido_formats::romfs::RomFsSource;

fn find_rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("KALEIDO_ROMS").map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join("Documents").join("NDS & 3DS"))
    })?;
    let p = dir.join(name);
    p.is_file().then_some(p)
}

/// (ROM, jeu attendu, starter édité, cible LayeredFS).
const CASES: [(&str, Game, u16, LayeredFsTarget); 4] = [
    ("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds", Game::Y, 650, LayeredFsTarget::Luma),
    ("Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2).3ds", Game::OmegaRuby, 252, LayeredFsTarget::Emulator),
    ("Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds", Game::Moon, 722, LayeredFsTarget::Luma),
    ("Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds", Game::UltraSun, 722, LayeredFsTarget::Emulator),
];

#[test]
fn edit_real_3ds_roms() {
    for (name, expected, starter, target) in CASES {
        let Some(rom) = find_rom(name) else {
            eprintln!("ROM absente, test ignoré : {name}");
            continue;
        };
        let game = CtrGameRom::open(&rom).unwrap();
        assert_eq!(game.game, expected);
        assert!(romedit_ctr::supports(&game));
        let gen = game.generation();
        let before = romedit_ctr::read(&game).unwrap();
        assert!(before.hidden_ability);
        assert_eq!(before.species.len(), game.layout.species_count as usize);
        assert_eq!(before.types.len(), 18);

        // Valeurs connues : Bulbizarre, Marisson, puis le starter édité.
        let grass = before.types.iter().find(|t| t.tag.key == PokeType::Grass).unwrap().index;
        let poison = before.types.iter().find(|t| t.tag.key == PokeType::Poison).unwrap().index;
        let bulba = &before.species[0];
        assert_eq!((bulba.stats, bulba.types, bulba.abilities, bulba.catch_rate), ([45, 49, 49, 65, 65, 45], [grass, poison], [65, 65, 34], 45));
        let chespin = &before.species[649];
        assert_eq!((chespin.stats, chespin.abilities), ([56, 61, 65, 48, 45, 38], [65, 65, 171]));
        let mut edit = before.species[starter as usize - 1].clone();
        let stats = match starter {
            650 => [56, 61, 65, 48, 45, 38],
            252 => [40, 45, 35, 65, 55, 70],
            _ => [68, 55, 55, 50, 50, 42],
        };
        assert_eq!(edit.stats, stats);
        assert_eq!(edit.types[0], grass);
        assert!(!edit.learnset.is_empty());
        if gen >= 7 {
            // Brindibou : Plante / Vol, talent caché Longue Portée ; Z-attaques non proposées.
            assert_eq!(edit.abilities, [65, 65, 203]);
            assert!(!before.moves.iter().any(|m| m.id == 622));
        }

        // Modifications.
        let dragon = before.types.iter().find(|t| t.tag.key == PokeType::Dragon).unwrap().index;
        edit.stats = [100, 110, 90, 80, 70, 120];
        edit.types = [dragon, grass];
        edit.abilities = [22, 65, 1];
        edit.catch_rate = 255;
        edit.learnset.push(LevelMove { level: 2, move_id: 1 });
        let mut bad = edit.clone();
        bad.learnset.push(LevelMove { level: 0, move_id: 1 });
        if gen >= 7 {
            edit.learnset.push(LevelMove { level: 0, move_id: 2 });
            bad.learnset.push(LevelMove { level: 5, move_id: 622 });
        }
        assert!(romedit_ctr::edited_files(&game, &[bad]).is_err());

        let out = std::env::temp_dir().join(format!("kaleido-romedit-{}-{}", std::process::id(), starter));
        std::fs::create_dir_all(&out).unwrap();
        let result = std::panic::catch_unwind(|| {
            let unchanged = before.species[3].clone();
            let (count, written) = romedit_ctr::save(&game, &[edit.clone(), unchanged], &out, CtrOutput::Both, target).unwrap();
            assert_eq!(count, 1);
            let l = game.layout;
            assert_eq!(written.files, [l.personal, l.levelup]);
            let (romfs, image) = (written.romfs.unwrap(), written.image.unwrap());
            let tid = format!("{:016X}", game.title_id());
            assert!(romfs.ends_with(PathBuf::from(&tid).join("romfs")));
            assert_eq!(romfs.to_string_lossy().contains("luma"), target == LayeredFsTarget::Luma);
            assert!(image.file_name().unwrap().to_string_lossy().ends_with(" - modifiée.3ds"));

            // LayeredFS : fiche individuelle et table complète, attaques apprises.
            let personal = Garc::parse(&std::fs::read(romfs.join(l.personal)).unwrap()).unwrap();
            let entry = personal.file(starter as usize).unwrap();
            let p = Personal::new(gen, entry.to_vec()).unwrap();
            assert_eq!((p.base_stats().speed, p.catch_rate(), entry[6], entry[7], entry[0x18], entry[0x1A]), (120, 255, dragon, grass, 22, 1));
            let table = personal.file(personal.len() - 1).unwrap();
            assert_eq!(&table[starter as usize * entry.len()..][..entry.len()], entry);
            let learn = Garc::parse(&std::fs::read(romfs.join(l.levelup)).unwrap()).unwrap();
            let moves = learnsets::read(gen, learn.file(starter as usize).unwrap());
            assert_eq!(moves.len(), edit.learnset.len());
            assert!(moves.windows(2).all(|w| w[0].1 <= w[1].1));

            // Image reconstruite : valide, mêmes fichiers que le LayeredFS, relue par l'éditeur.
            assert!(verify_image(&image).unwrap().ok());
            let rebuilt = RomFsSource::open(&image).unwrap();
            for path in &written.files {
                assert!(rebuilt.read(path).unwrap() == std::fs::read(romfs.join(path)).unwrap(), "{path} : différent du LayeredFS");
            }
            let new = CtrGameRom::from_romfs(rebuilt).unwrap();
            assert_eq!(new.game, game.game);
            let after = romedit_ctr::read(&new).unwrap();
            let mut expected = edit.clone();
            expected.learnset.sort_by_key(|m| m.level);
            let i = starter as usize - 1;
            assert_eq!(after.species[i], expected);
            assert_eq!(after.species[..i], before.species[..i]);
            assert_eq!(after.species[i + 1..], before.species[i + 1..]);
            let shown = &new.species().unwrap()[i];
            assert_eq!((shown.types.len(), shown.catch_rate, shown.base_stats.speed), (2, 255, 120));
            assert!(shown.hidden_ability.is_some());
        });
        let _ = std::fs::remove_dir_all(&out);
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }
}
