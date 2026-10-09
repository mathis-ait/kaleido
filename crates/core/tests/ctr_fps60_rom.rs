//! « 60 fps natif » sur une vraie ROM : le profil reconnaît le `code.bin` de Rubis Oméga
//! EUR v1.0, et le patch se fusionne avec le taux de chromatiques sans chevauchement.
//!
//! Utilise les `.3ds` déchiffrés de `~/Documents/NDS & 3DS` (ou du dossier
//! `KALEIDO_3DS_DIR`) ; ignoré (avec un message) s'il n'y en a aucun.

use std::path::PathBuf;

use kaleido_core::data::fps60_ctr;
use kaleido_core::data::shiny_ctr;
use kaleido_core::CtrGameRom;
use kaleido_formats::ips;

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
fn fps60_profile_and_merge_with_shiny() {
    let mut seen = 0;
    for rom in roms() {
        let Ok(game) = CtrGameRom::open(&rom) else { continue };
        let tid = game.title_id();
        if !fps60_ctr::supports(tid) {
            continue;
        }
        let original = game.code().unwrap().code;
        let Some(profile) = fps60_ctr::profile_for(tid, &original) else {
            eprintln!("{} : version non couverte (SHA {})", rom.display(), fps60_ctr::sha256_hex(&original));
            continue;
        };
        seen += 1;

        // Seul : le programme patché garde sa taille, les crochets sont posés.
        let alone = fps60_ctr::merged_ips(profile, &original, None).unwrap();
        let mut a = original.clone();
        ips::apply(&mut a, &alone).unwrap();
        assert_eq!(a.len(), original.len());
        assert_ne!(a, original);

        // Avec le taux de chromatiques (1/512) : les deux patchs restent appliqués.
        let mut shiny = original.clone();
        shiny_ctr::apply(&mut shiny, shiny_ctr::plan(512)).unwrap();
        let shiny_ips = ips::create(&original, &shiny).unwrap();
        let both = fps60_ctr::merged_ips(profile, &original, Some(&shiny_ips)).unwrap();
        let mut b = original.clone();
        ips::apply(&mut b, &both).unwrap();
        assert_eq!(shiny_ctr::current(&b).unwrap(), shiny_ctr::current(&shiny).unwrap());
        for (at, bytes) in ips::records(profile.ips).unwrap() {
            assert_eq!(&b[at..at + bytes.len()], &bytes[..]);
        }
    }
    if seen == 0 {
        eprintln!("aucune ROM Rubis Oméga EUR v1.0 : test ignoré (définir KALEIDO_3DS_DIR)");
    }
}

/// Le patch installé à la main pendant le développement (ancienne version du runtime,
/// fusionnée avec le taux de chromatiques) se remplace sans conflit et garde le taux.
#[test]
fn fps60_replaces_older_runtime() {
    let old = include_bytes!("../../../ctr-smooth/proto/fps60-phase2-avec-shiny.ips");
    let shiny_only = include_bytes!("../../../ctr-smooth/proto/kaleido-shiny-or-eur-v1.0.ips");
    for rom in roms() {
        let Ok(game) = CtrGameRom::open(&rom) else { continue };
        let original = game.code().unwrap().code;
        let Some(profile) = fps60_ctr::profile_for(game.title_id(), &original) else { continue };
        // fusion de la phase 2 (commit 4f6a0ef) : runtime d'alors + taux de chromatiques
        let rest = fps60_ctr::strip(profile, &original, old).unwrap().expect("le taux de chromatiques reste");
        let merged = fps60_ctr::merged_ips(profile, &original, Some(&rest)).unwrap();
        assert!(fps60_ctr::contains(profile, &original, &merged).unwrap());
        let mut b = original.clone();
        ips::apply(&mut b, &merged).unwrap();
        let mut s = original.clone();
        ips::apply(&mut s, shiny_only).unwrap();
        assert_eq!(shiny_ctr::current(&b).unwrap(), shiny_ctr::current(&s).unwrap());
    }
}
