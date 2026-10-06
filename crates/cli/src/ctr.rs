//! Commandes 3DS : RomFS, archives GARC, textes et Pokédex Gen 6/7.

use std::path::Path;

use kaleido_formats::garc::Garc;
use kaleido_formats::lz;
use kaleido_formats::romfs::RomFsSource;

use crate::{report, CliResult};

pub fn open(path: &str) -> RomFsSource {
    RomFsSource::open(Path::new(path)).unwrap_or_else(|e| {
        eprintln!("impossible d'ouvrir {path} : {e}");
        std::process::exit(1);
    })
}

/// Randomise un jeu 3DS vers un dossier LayeredFS, puis relit les fichiers écrits.
pub fn randomize(path: &str, preset: &str, seed: u64, out: &str) -> CliResult {
    use kaleido_core::data::{encounters, trainers};
    use kaleido_core::randomizer::{self, ctr::LayeredFsTarget};

    let settings = crate::load_settings(preset)?;
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    let (outcome, written) = randomizer::ctr::randomize(&game, &settings, seed, Path::new(out), LayeredFsTarget::Luma)?;
    println!("{} emplacements sauvages, {} Pokémon de dresseurs", outcome.wild_slots, outcome.trainer_pokemon);
    std::fs::write(format!("{out}/journal.txt"), &outcome.log)?;
    if let Some(image) = &written.image {
        println!("ROM complète → {}", image.display());
    }
    // Sortie « .3ds » seule (réglage `ctrOutput` d'un fichier JSON) : pas de dossier à relire.
    let Some(romfs) = written.romfs else { return Ok(()) };
    println!("LayeredFS → {}", romfs.display());

    // Relecture : une zone et un dresseur, depuis les fichiers écrits.
    let names = game.text_file(game.layout.species_names)?;
    let l = game.layout;
    if game.generation() == 7 {
        if let Ok(data) = std::fs::read(romfs.join(gen7_gifts(&game))) {
            let gifts = Garc::parse(&data)?;
            println!("Starters relus :");
            print_gen7_gifts(&names, gifts.file(0).unwrap_or_default(), 3);
        }
        // Textes de l'écran de choix (fichier 41 en Soleil/Lune, 39 en Ultra) : lignes modifiées.
        if let Ok(data) = std::fs::read(romfs.join(l.story_text)) {
            use kaleido_core::text::gen5::{MsgFile, Variant};
            let n = if gen7_gifts(&game) == "a/1/5/9" { 39 } else { 41 };
            let lines = |g: &Garc| g.file(n).and_then(|d| MsgFile::parse_with(d, Variant::Gen6).ok()).map(|m| m.strings()).unwrap_or_default();
            let (before, after) = (lines(&game.garc(l.story_text)?), lines(&Garc::parse(&data)?));
            for (i, (b, a)) in before.iter().zip(&after).enumerate().filter(|(_, (b, a))| b != a) {
                println!("  texte {n}.{i} : {b:?} → {a:?}");
            }
        }
        if let Ok(data) = std::fs::read(romfs.join(l.encounters)) {
            let garc = Garc::parse(&data)?;
            println!("Zone 0 relue :");
            print_gen7_area(&names, &lz::decompress(garc.file(9).ok_or("zone 0 absente")?)?);
        }
        if let Ok(data) = std::fs::read(romfs.join(l.trainer_pokemon)) {
            let (d, p) = (game.garc(l.trainer_data)?, Garc::parse(&data)?);
            for i in [1, 149] {
                println!("Dresseur {i} relu : {}", gen7_team(&names, d.file(i).unwrap_or_default(), p.file(i).unwrap_or_default()));
            }
        }
        return Ok(());
    }
    if let Ok(data) = std::fs::read(romfs.join(l.encounters)) {
        let garc = Garc::parse(&data)?;
        let zone = lz::decompress(garc.file(100).ok_or("zone 100 absente")?)?;
        let slots: Vec<_> =
            encounters::read(6, &zone).iter().map(|s| format!("{} {}-{}", names[s.species as usize], s.min_level, s.max_level)).collect();
        println!("Zone 100 relue : {}", slots.join(", "));
    }
    if let (Ok(d), Ok(p)) = (std::fs::read(romfs.join(l.trainer_data)), std::fs::read(romfs.join(l.trainer_pokemon))) {
        let (d, p) = (Garc::parse(&d)?, Garc::parse(&p)?);
        let team = trainers::read_team(6, d.file(561).unwrap_or_default(), p.file(561).unwrap_or_default()).ok_or("dresseur 561 illisible")?;
        let list: Vec<_> = team.pokemon.iter().map(|t| format!("{} niv. {} {:?}", names[t.species as usize], t.level, t.moves)).collect();
        println!("Dresseur 561 relu : {}", list.join(" ; "));
    }
    Ok(())
}

/// Archive des Pokémon offerts en Gen 7 (starters en tête de l'entrée 0).
fn gen7_gifts(game: &kaleido_core::CtrGameRom) -> &'static str {
    use kaleido_core::Game;
    if matches!(game.game, Game::UltraSun | Game::UltraMoon) {
        "a/1/5/9"
    } else {
        "a/1/5/5"
    }
}

fn u16_at(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn species_name(names: &[String], raw: u16) -> String {
    let (s, form) = (raw & 0x7FF, raw >> 11);
    let name = names.get(s as usize).map_or("?", String::as_str);
    if form > 0 {
        format!("{name} (forme {form})")
    } else {
        name.to_string()
    }
}

/// Dons Gen 7 : 0x14 octets (`u16` espèce, `u8` forme, `u8` niveau).
fn print_gen7_gifts(names: &[String], gifts: &[u8], count: usize) {
    for (i, g) in gifts.as_chunks::<0x14>().0.iter().take(count).enumerate() {
        let form = if g[2] > 0 { format!(" (forme {})", g[2]) } else { String::new() };
        println!("  don {i:2} : {}{form} niv. {}", names.get(u16_at(g, 0) as usize).map_or("?", String::as_str), g[3]);
    }
}

/// Tables d'une zone Gen 7 (archive `EA` décompressée).
fn print_gen7_area(names: &[String], d: &[u8]) {
    use kaleido_core::data::encounters::{alola_tables, ALOLA_HALF};
    for (t, table) in alola_tables(d).into_iter().enumerate() {
        for (half, label) in ["jour", "nuit"].iter().enumerate() {
            let base = table + 4 + half * ALOLA_HALF;
            let row = |r: usize| -> Vec<String> {
                (0..10)
                    .filter(|&i| u16_at(d, base + 0x0C + r * 40 + i * 4) != 0)
                    .map(|i| species_name(names, u16_at(d, base + 0x0C + r * 40 + i * 4)))
                    .collect()
            };
            let normal = row(0);
            if normal.is_empty() {
                continue;
            }
            let rates: Vec<String> = d[base + 2..base + 12].iter().map(|r| r.to_string()).collect();
            println!("  table {t} ({label}, niv. {}-{}, taux {}) : {}", d[base], d[base + 1], rates.join("/"), normal.join(", "));
            let mut sos: Vec<String> = (1..8).flat_map(row).collect();
            sos.extend((0..6).map(|i| u16_at(d, base + 0x14C + i * 4)).filter(|&s| s != 0).map(|s| format!("{} (météo)", species_name(names, s))));
            let mut seen = std::collections::HashSet::new();
            sos.retain(|s| seen.insert(s.clone()));
            if !sos.is_empty() {
                println!("    SOS : {}", sos.join(", "));
            }
        }
    }
}

/// Équipe d'un dresseur Gen 7 (voir `kaleido_core::randomizer` : trdata 0x14, trpoke 0x20).
fn gen7_team(names: &[String], trdata: &[u8], trpoke: &[u8]) -> String {
    let count = trdata.get(3).copied().unwrap_or(0) as usize;
    trpoke
        .as_chunks::<0x20>()
        .0
        .iter()
        .take(count)
        .map(|p| {
            let moves: Vec<String> = (0..4).map(|m| u16_at(p, 0x18 + m * 2)).filter(|&m| m != 0).map(|m| m.to_string()).collect();
            let item = u16_at(p, 0x14);
            let mut s = format!("{} niv. {}", species_name(names, u16_at(p, 0x10) | (u16_at(p, 0x12) << 11)), u16_at(p, 0x0E));
            if item != 0 {
                s.push_str(&format!(" @{item}"));
            }
            if !moves.is_empty() {
                s.push_str(&format!(" [{}]", moves.join(",")));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" ; ")
}

pub fn gen7_starters(path: &str) -> CliResult {
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    let names = game.text_file(game.layout.species_names)?;
    let gifts = game.garc(gen7_gifts(&game))?;
    print_gen7_gifts(&names, gifts.file(0).unwrap_or_default(), usize::MAX);
    Ok(())
}

pub fn gen7_wild(path: &str, zone: usize) -> CliResult {
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    let names = game.text_file(game.layout.species_names)?;
    let garc = game.garc(game.layout.encounters)?;
    let entry = garc.file(9 + 11 * zone).ok_or("zone absente")?;
    print_gen7_area(&names, &lz::decompress(entry)?);
    Ok(())
}

pub fn gen7_trainers(path: &str, filter: &str) -> CliResult {
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    let l = game.layout;
    let names = game.text_file(l.species_names)?;
    let trainer_names = game.text_file(l.trainer_names)?;
    let classes = game.text_file(l.trainer_classes)?;
    let (d, p) = (game.garc(l.trainer_data)?, game.garc(l.trainer_pokemon)?);
    for i in 1..d.len() {
        let td = d.file(i).unwrap_or_default();
        let class = td.first().and_then(|&c| classes.get(c as usize)).map_or("?", String::as_str);
        let line =
            format!("{i:4} {class} {} : {}", trainer_names.get(i).map_or("?", String::as_str), gen7_team(&names, td, p.file(i).unwrap_or_default()));
        if line.contains(filter) {
            println!("{line}");
        }
    }
    Ok(())
}

/// Exporte les noms français et les données de base des espèces (Gen 6) vers un
/// JSON embarqué par `kaleido_core::names` (utile pour afficher une sauvegarde sans ROM).
pub fn export_names(path: &str, out: &str) -> CliResult {
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    let l = game.layout;
    let count = l.species_count as usize;
    let species: Vec<String> = game.text_file(l.species_names)?.into_iter().take(count + 1).collect();
    let personal = game.garc(l.personal)?;
    let record = |i: usize| personal.file(i).map(<[u8]>::to_vec).unwrap_or_default();
    let base_stats: Vec<[u8; 6]> = (0..=count).map(|i| record(i).get(..6).and_then(|s| s.try_into().ok()).unwrap_or([0; 6])).collect();
    let growth: Vec<u8> = (0..=count).map(|i| record(i).get(0x15).copied().unwrap_or(0)).collect();
    let ability_ids: Vec<u8> = (0..=count).map(|i| record(i).get(0x18).copied().unwrap_or(0)).collect();
    let json = format!(
        "{{\"source\":{},\"species\":{},\"moves\":{},\"abilities\":{},\"items\":{},\"growth\":{:?},\"baseStats\":{:?},\"abilityIds\":{:?}}}",
        json_str(game.game.name_fr()),
        json_list(&species),
        json_list(&game.text_file(l.move_names)?),
        json_list(&game.text_file(l.ability_names)?),
        json_list(&game.text_file(l.item_names)?),
        growth,
        base_stats,
        ability_ids
    );
    std::fs::write(out, json)?;
    println!("{} espèces exportées vers {out}", species.len() - 1);
    Ok(())
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_list(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(","))
}

pub fn info(rom: &RomFsSource) -> CliResult {
    match rom.title_id() {
        Some(id) => println!("Title ID   : {id:016X}"),
        None => println!("Title ID   : inconnu"),
    }
    if let Some(game) = rom.title_id().and_then(kaleido_core::Game::from_title_id) {
        println!("Jeu        : {}", game.name_fr());
    }
    let files = rom.files();
    let total: u64 = files.iter().map(|f| f.size).sum();
    println!("RomFS      : {} fichiers, {} octets", files.len(), total);
    let garcs = files.iter().filter(|f| f.path.starts_with("a/")).count();
    println!("Archives   : {garcs} fichiers a/x/y/z");
    Ok(())
}

pub fn ls(rom: &RomFsSource, filter: &str) -> CliResult {
    for f in rom.files().iter().filter(|f| f.path.contains(filter)) {
        let head = if f.size >= 4 { rom.read(&f.path).ok() } else { None };
        match head.as_deref().filter(|d| Garc::is_garc(d)).map(Garc::parse) {
            Some(Ok(g)) => {
                let multi = g.entries.iter().filter(|e| e.subfiles.len() > 1).count();
                let lz = g.entries.iter().filter(|e| e.data().is_some_and(lz::is_lz11)).count();
                println!(
                    "{:>10}  {}  [GARC v{:X}, {} entrées{}{}]",
                    f.size,
                    f.path,
                    g.version >> 8,
                    g.len(),
                    if multi > 0 { format!(", {multi} multiples") } else { String::new() },
                    if lz > 0 { format!(", {lz} LZ11") } else { String::new() }
                );
            }
            Some(Err(e)) => println!("{:>10}  {}  [GARC illisible : {e}]", f.size, f.path),
            None => println!("{:>10}  {}", f.size, f.path),
        }
    }
    Ok(())
}

pub fn check(rom: &RomFsSource) -> CliResult {
    let mut failures = 0;
    let (mut garcs, mut identical, mut bad) = (0, 0, Vec::new());
    let (mut entries, mut subfiles, mut lz_total, mut lz_ok, mut lz_bad) = (0, 0, 0, 0, Vec::new());
    let mut versions = std::collections::BTreeMap::<u16, usize>::new();
    let mut lz_sizes = (0usize, 0usize);

    for f in rom.files().iter().filter(|f| f.size >= 4) {
        let data = rom.read(&f.path)?;
        if !Garc::is_garc(&data) {
            continue;
        }
        garcs += 1;
        let garc = match Garc::parse(&data) {
            Ok(g) => g,
            Err(e) => {
                bad.push(format!("{} : {e}", f.path));
                continue;
            }
        };
        *versions.entry(garc.version).or_default() += 1;
        let rebuilt = garc.to_bytes();
        if rebuilt == data {
            identical += 1;
        } else {
            let at = rebuilt.iter().zip(&data).position(|(a, b)| a != b).unwrap_or(rebuilt.len().min(data.len()));
            bad.push(format!("{} : différence à {at:#X} ({} → {} octets)", f.path, data.len(), rebuilt.len()));
        }
        for e in &garc.entries {
            entries += 1;
            for (_, sub) in &e.subfiles {
                subfiles += 1;
                if !lz::is_lz11(sub) {
                    continue;
                }
                lz_total += 1;
                // Un fichier non compressé peut commencer par 0x11 : seuls ceux qui se décompressent comptent.
                let Ok(d) = lz::decompress_lz11(sub) else { continue };
                lz_ok += 1;
                let packed = lz::compress_lz11(&d);
                if lz::decompress_lz11(&packed).ok().as_ref() != Some(&d) {
                    lz_bad.push(format!("{} : recompression LZ11 incorrecte", f.path));
                } else {
                    lz_sizes.0 += sub.len();
                    lz_sizes.1 += packed.len();
                }
            }
        }
    }

    let versions: Vec<String> = versions.iter().map(|(v, n)| format!("v{:X} × {n}", v >> 8)).collect();
    println!("       {garcs} GARC ({}), {entries} entrées, {subfiles} sous-fichiers", versions.join(", "));
    println!("       {identical}/{garcs} GARC reconstruits à l'octet près");
    report(&format!("Reconstruction des {garcs} GARC"), bad.is_empty(), &mut failures);
    for b in bad.iter().take(15) {
        println!("       ✗ {b}");
    }
    println!("       {lz_ok}/{lz_total} sous-fichiers commençant par 0x11 décompressés (LZ11)");
    println!("       recompression : {} → {} octets", lz_sizes.0, lz_sizes.1);
    report(&format!("Recompression LZ11 des {lz_ok} sous-fichiers"), lz_bad.is_empty(), &mut failures);
    for b in lz_bad.iter().take(10) {
        println!("       ✗ {b}");
    }
    if failures > 0 {
        return Err(format!("{failures} vérification(s) en échec").into());
    }
    Ok(())
}

/// Outil de mise au point : vidage hexadécimal d'un fichier du RomFS, ou d'une
/// entrée de GARC (décompressée si elle est en LZ11).
pub fn hex(rom: &RomFsSource, path: &str, entry: Option<usize>, offset: usize, len: usize) -> CliResult {
    let mut data = rom.read(path)?;
    if let Some(n) = entry {
        let garc = Garc::parse(&data)?;
        let e = garc.entries.get(n).ok_or("entrée hors de l'archive")?;
        let bits: Vec<_> = e.subfiles.iter().map(|(b, d)| format!("bit {b} : {} octets", d.len())).collect();
        println!("entrée {n} : {}", bits.join(", "));
        data = e.data().unwrap_or_default().to_vec();
        if lz::is_lz11(&data) {
            if let Ok(d) = lz::decompress_lz11(&data) {
                println!("LZ11 : {} → {} octets", data.len(), d.len());
                data = d;
            }
        }
    }
    println!("{} octets", data.len());
    let end = offset.saturating_add(len).min(data.len());
    for (i, row) in data.get(offset..end).unwrap_or_default().chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02X}")).collect();
        let text: String = row.iter().map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect();
        println!("{:08X}  {:<48} {text}", offset + i * 16, hex.join(" "));
    }
    Ok(())
}

pub fn species(path: &str) -> CliResult {
    let game = kaleido_core::CtrGameRom::open(Path::new(path))?;
    if !game.layout.verified {
        eprintln!("attention : emplacements non vérifiés pour {}", game.game.name_fr());
    }
    crate::print_species(&game.species()?);
    Ok(())
}

/// Outil de mise au point : cherche une suite d'octets (hexadécimal) dans tous les
/// fichiers du RomFS et dans les entrées de GARC (décompressées si besoin).
pub fn search_bytes(rom: &RomFsSource, pattern: &str, filter: &str) -> CliResult {
    let needle = (0..pattern.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(pattern.get(i..i + 2).unwrap_or("?"), 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| "motif hexadécimal invalide")?;
    if needle.is_empty() {
        return Err("motif vide".into());
    }
    let find_all = |hay: &[u8]| hay.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle.as_slice()).map(|(i, _)| i).collect::<Vec<_>>();
    for f in rom.files().iter().filter(|f| f.path.contains(filter) && f.size < 0x1000_0000) {
        let data = rom.read(&f.path)?;
        if let Ok(garc) = Garc::parse(&data) {
            for (n, e) in garc.entries.iter().enumerate() {
                let Some(sub) = e.data() else { continue };
                let unpacked = if lz::is_lz11(sub) { lz::decompress_lz11(sub).ok() } else { None };
                let hits = find_all(unpacked.as_deref().unwrap_or(sub));
                if !hits.is_empty() {
                    let at: Vec<String> = hits.iter().take(8).map(|h| format!("{h:#X}")).collect();
                    println!("{} entrée {n}{} : {}", f.path, if unpacked.is_some() { " (LZ11)" } else { "" }, at.join(", "));
                }
            }
        } else {
            let hits = find_all(&data);
            if !hits.is_empty() {
                let at: Vec<String> = hits.iter().take(8).map(|h| format!("{h:#X}")).collect();
                println!("{} : {}", f.path, at.join(", "));
            }
        }
    }
    Ok(())
}
