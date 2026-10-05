//! Reconstruction d'une vraie image 3DS sans modification : l'image écrite doit être
//! identique à l'original (jusqu'à la fin des données) et tous ses hachages valides.
//!
//! Nécessite une ROM déchiffrée : variable `KALEIDO_3DS_ROM`, sinon le premier `.3ds`
//! de `~/Documents/NDS & 3DS`. Le test est ignoré (avec un message) si elle est absente.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::time::Instant;

use kaleido_formats::ctr_build::{rebuild_image, verify_image};

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

/// Compare `a` (entier) au début de `b`, par blocs.
fn same_prefix(a: &PathBuf, b: &PathBuf) -> Option<u64> {
    let (mut fa, mut fb) = (BufReader::with_capacity(1 << 20, File::open(a).unwrap()), BufReader::with_capacity(1 << 20, File::open(b).unwrap()));
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    let mut pos = 0u64;
    loop {
        let n = fa.read(&mut ba).unwrap();
        if n == 0 {
            return None;
        }
        fb.read_exact(&mut bb[..n]).unwrap();
        if ba[..n] != bb[..n] {
            let i = ba[..n].iter().zip(&bb[..n]).position(|(x, y)| x != y).unwrap();
            return Some(pos + i as u64);
        }
        pos += n as u64;
    }
}

#[test]
fn rebuild_unchanged_is_identical() {
    let Some(rom) = find_rom() else {
        eprintln!("ROM 3DS absente : test ignoré (définir KALEIDO_3DS_ROM)");
        return;
    };
    let out = std::env::temp_dir().join(format!("kaleido-rebuild-same-{}.3ds", std::process::id()));
    let t = Instant::now();
    let report = rebuild_image(&rom, &out, &[]).unwrap();
    eprintln!("reconstruction : {} octets en {:.1?}", report.size, t.elapsed());

    let result = std::panic::catch_unwind(|| {
        let t = Instant::now();
        let diff = same_prefix(&out, &rom);
        eprintln!("comparaison : {:.1?}", t.elapsed());
        assert_eq!(diff, None, "première différence avec l'original");
        let t = Instant::now();
        let check = verify_image(&out).unwrap();
        eprintln!("vérification : {:.1?} — {check:?}", t.elapsed());
        assert!(check.ok());
        assert!(check.partitions[0].ivfc_blocks[2] > 0);
    });
    let _ = std::fs::remove_file(&out);
    result.unwrap();
}
