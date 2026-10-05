//! ROM « DSi Enhanced » presque pleine (Pokémon Blanche) : des fichiers qui grossissent
//! au-delà de la place libre forcent le rangement de tous les fichiers. Le test relit la
//! ROM et vérifie chaque fichier ainsi que les zones qui ne doivent pas bouger.
//! Ignoré (avec un message) si la ROM est absente.

use std::path::PathBuf;

use kaleido_formats::nds::NdsRom;

/// Dossier `KALEIDO_ROMS`, sinon `~/Documents/NDS & 3DS`.
fn rom_path() -> Option<PathBuf> {
    let dir = match std::env::var_os("KALEIDO_ROMS") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?).join("Documents").join("NDS & 3DS"),
    };
    let p = dir.join("Pokemon - Version Blanche (France) (NDSi Enhanced).nds");
    p.exists().then_some(p)
}

#[test]
fn full_dsi_rom_repacks_files() {
    let Some(path) = rom_path() else {
        eprintln!("ROM Blanche absente, test ignoré");
        return;
    };
    let original = std::fs::read(&path).unwrap();
    let mut rom = NdsRom::from_bytes(original.clone()).unwrap();
    let h = rom.header().clone();
    assert_ne!(h.ntr_region_end, 0, "ROM DSi attendue");

    // 40 fichiers nommés grossissent de 3 Kio chacun : bien plus que les ~21 Kio libres en fin de zone.
    let ids: Vec<u16> = rom.files().map(|(id, _)| id).step_by(5).take(40).collect();
    let mut expected = Vec::new();
    for &id in &ids {
        let mut d = rom.file(id).unwrap().to_vec();
        d.extend((0..3072u32).map(|i| (i * 7 + id as u32) as u8));
        expected.push((id, d.clone()));
        rom.replace_file(id, d).unwrap();
    }

    let out = rom.to_bytes().unwrap();
    assert_eq!(out.len(), original.len());
    let reread = NdsRom::from_bytes(out.clone()).unwrap();
    for id in 0..reread.file_count() as u16 {
        let want = expected.iter().find(|(i, _)| *i == id).map(|(_, d)| d.as_slice()).unwrap_or_else(|| rom.file(id).unwrap());
        assert_eq!(reread.file(id).unwrap(), want, "fichier n°{id}");
    }
    for o in reread.overlays() {
        assert!(reread.overlay(o.id).is_ok(), "overlay {}", o.id);
    }

    // Zones fixes : ARM7, FNT, bannière, données hors FAT après les fichiers, zone DSi.
    let same = |s: u32, len: u32| assert_eq!(out[s as usize..(s + len) as usize], original[s as usize..(s + len) as usize], "zone {s:#X}");
    same(h.arm7_offset, h.arm7_size);
    same(h.fnt_offset, h.fnt_size);
    same(h.banner_offset, 0x840);
    // Données hors FAT entre le dernier fichier et la fin déclarée de la ROM (~3,9 Mio sur Blanche).
    let fat = &original[h.fat_offset as usize..(h.fat_offset + h.fat_size) as usize];
    let files_end = fat.as_chunks::<8>().0.iter().map(|c| u32::from_le_bytes([c[4], c[5], c[6], c[7]])).max().unwrap();
    let extra = files_end.next_multiple_of(0x200);
    same(extra, h.used_rom_size - extra);
    assert_eq!(out[h.ntr_region_end as usize..], original[h.ntr_region_end as usize..]);
}
