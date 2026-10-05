//! `kaleido` : inspection et vérification des ROMs en ligne de commande.

mod ctr;

use std::path::Path;
use std::process::ExitCode;

use std::collections::BTreeMap;

use kaleido_core::text::gen5::Variant;
use kaleido_core::text::{gen4, gen5};
use kaleido_core::FileKind;
use kaleido_formats::garc::Garc;
use kaleido_formats::narc::Narc;
use kaleido_formats::nds::NdsRom;

type CliResult = Result<(), Box<dyn std::error::Error>>;

const USAGE: &str = "\
Usage : kaleido <commande> <rom> [arguments]

Commandes DS (.nds) :
  info      <rom>                       En-tête, nombre de fichiers et d'overlays
  ls        <rom> [filtre]              Liste des fichiers (taille, nombre d'entrées NARC)
  check     <rom>                       Vérifie la relecture / reconstruction de toute la ROM
  species   <rom>                       Pokédex : types, statistiques, talents

Commandes 3DS (.3ds / .cxi / .cia déchiffrés, ou dossier extrait avec romfs/) :
  info3ds    <rom>                      Title ID, jeu, contenu du RomFS
  ls3ds      <rom> [filtre]             Fichiers du RomFS (GARC : entrées, LZ11)
  check3ds   <rom>                      Reconstruit tous les GARC, vérifie le LZ11
  species3ds <rom>                      Pokédex Gen 6/7
  hex3ds     <rom> <chemin> [entrée|-] [début] [longueur]   Vidage hexadécimal

Textes (DS : archive NARC ; 3DS : archive GARC, ex. a/0/7/4) :
  text      <rom> <archive> <n°>        Affiche un fichier de texte
  find      <rom> <archive> <texte>     Cherche un texte dans toute une archive
  textcheck <rom> <archive>             Vérifie déchiffrement, décodage et réencodage
  (alias 3DS : text3ds, find3ds, textcheck3ds)

Outils DS :
  hex       <rom> <archive> <n°>        Vidage hexadécimal d'une entrée NARC
  search    <rom> <v1,v2,…>             Cherche une suite de valeurs (u16/u32)
  randomize <rom> <préréglage> <seed> <sortie.nds>   Randomise (equilibre, nuzlocke, chaos, defi)
  starters  <rom>                       Starters actuels";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["info", rom] => info(&open(rom)),
        ["ls", rom] => ls(&open(rom), ""),
        ["ls", rom, filter] => ls(&open(rom), filter),
        ["check", rom] => check(&open(rom)),
        ["text" | "text3ds", rom, archive, n] => n.parse().map_err(Into::into).and_then(|n| text(rom, archive, n)),
        ["find" | "find3ds", rom, archive, needle] => find(rom, archive, needle),
        ["textcheck" | "textcheck3ds", rom, archive] => text_check(rom, archive),
        ["species", rom] => species(rom),
        ["randomize", rom, preset, seed, out] => seed.parse().map_err(Into::into).and_then(|s| randomize(rom, preset, s, out)),
        ["starters", rom] => starters(rom),
        ["search", rom, values] => search(&open(rom), values),
        ["shinyscan", rom] => shiny_scan(&open(rom)),
        ["shiny", rom, mult, out] => mult.parse().map_err(Into::into).and_then(|m: u16| {
            use kaleido_core::data::shiny;
            let mut r = open(rom);
            let before = r.arm9_decompressed()?;
            println!("ARM9 compressé : {}, {} octets, constante en {:#X}", r.arm9_compressed(), r.arm9().len(), shiny::locate(&r)?);
            let t = std::time::Instant::now();
            let odds = shiny::set_multiplier(&mut r, m)?;
            println!("taux : 1/{odds} ({} ms), ARM9 : {} octets", t.elapsed().as_millis(), r.arm9().len());
            std::fs::write(out, r.to_bytes()?)?;
            let check = NdsRom::open(Path::new(out))?;
            let after = check.arm9_decompressed()?;
            let diff: Vec<usize> = (0..before.len().min(after.len())).filter(|&i| before[i] != after[i]).collect();
            println!(
                "relu : seuil {}, {} octet(s) différent(s) {:X?}, tailles {} / {}, CRC en-tête {}",
                shiny::current_threshold(&check)?,
                diff.len(),
                diff.iter().take(4).collect::<Vec<_>>(),
                before.len(),
                after.len(),
                check.header().header_crc_ok
            );
            Ok(())
        }),
        ["hex", rom, path, n] => n.parse().map_err(Into::into).and_then(|n| hex_entry(&open(rom), path, n)),
        ["info3ds", rom] => ctr::info(&ctr::open(rom)),
        ["ls3ds", rom] => ctr::ls(&ctr::open(rom), ""),
        ["ls3ds", rom, filter] => ctr::ls(&ctr::open(rom), filter),
        ["check3ds", rom] => ctr::check(&ctr::open(rom)),
        ["hex3ds", rom, path, rest @ ..] => hex3ds(rom, path, rest),
        ["bytes3ds", rom, pattern] => ctr::search_bytes(&ctr::open(rom), pattern, ""),
        ["bytes3ds", rom, pattern, filter] => ctr::search_bytes(&ctr::open(rom), pattern, filter),
        ["species3ds", rom] => ctr::species(rom),
        ["export-names", rom, out] => ctr::export_names(rom, out),
        ["demo-save", out] => kaleido_core::save::demo_save().map_err(Into::into).and_then(|b| std::fs::write(out, b).map_err(Into::into)),
        ["save", path] => std::fs::read(path)
            .map_err(Into::into)
            .and_then(|b| kaleido_core::save::SaveFile::from_bytes(&b).map_err(Into::into))
            .map(|s| println!("{}", kaleido_core::save::describe(&s))),
        ["randomize3ds", rom, preset, seed, out] => seed.parse().map_err(Into::into).and_then(|s| ctr::randomize(rom, preset, s, out)),
        ["textdiff", rom, archive, n] => n.parse().map_err(Into::into).and_then(|n| text_diff(rom, archive, n)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur : {e}");
            ExitCode::FAILURE
        }
    }
}

fn open(path: &str) -> NdsRom {
    NdsRom::open(Path::new(path)).unwrap_or_else(|e| {
        eprintln!("impossible d'ouvrir {path} : {e}");
        std::process::exit(1);
    })
}

fn info(rom: &NdsRom) -> CliResult {
    let h = rom.header();
    println!("Titre      : {}", h.title);
    println!("Code       : {}", h.game_code);
    println!("Fichiers   : {} ({} nommés)", rom.file_count(), rom.files().count());
    println!("Overlays   : {}", rom.overlays().len());
    println!("Taille     : {} octets utilisés", h.used_rom_size);
    if h.ntr_region_end != 0 {
        println!("DSi        : zone DS jusqu'à {:#X}", h.ntr_region_end);
    }
    Ok(())
}

fn ls(rom: &NdsRom, filter: &str) -> CliResult {
    for (id, path) in rom.files().filter(|(_, p)| p.contains(filter)) {
        let data = rom.file(id).unwrap_or_default();
        let narc = Narc::is_narc(data).then(|| Narc::parse(data).map(|n| n.files.len()));
        match narc {
            Some(Ok(n)) => println!("{id:5}  {:>9}  {path}  [NARC, {n} fichiers]", data.len()),
            Some(Err(e)) => println!("{id:5}  {:>9}  {path}  [NARC illisible : {e}]", data.len()),
            None => println!("{id:5}  {:>9}  {path}", data.len()),
        }
    }
    Ok(())
}

fn check(rom: &NdsRom) -> CliResult {
    let mut failures = 0;

    let original = rom.to_bytes()?;
    let mut copy = NdsRom::from_bytes(original.clone())?;
    for id in 0..rom.file_count() as u16 {
        copy.replace_file(id, rom.file(id).unwrap_or_default().to_vec())?;
    }
    let rebuilt = copy.to_bytes()?;
    report("Reconstruction ROM identique (tous fichiers réécrits)", rebuilt == original, &mut failures);

    let (mut narcs, mut narc_identical, mut narc_bad) = (0, 0, Vec::new());
    for (id, path) in rom.files() {
        let data = rom.file(id).unwrap_or_default();
        if !Narc::is_narc(data) {
            continue;
        }
        narcs += 1;
        match Narc::parse(data) {
            Ok(n) if n.to_bytes() == data => narc_identical += 1,
            Ok(n) if Narc::parse(&n.to_bytes()).is_ok_and(|m| m == n) => {}
            _ => narc_bad.push(path.to_string()),
        }
    }
    println!("       {narc_identical}/{narcs} NARC reconstruits à l'octet près");
    report(&format!("Contenu des {narcs} NARC préservé"), narc_bad.is_empty(), &mut failures);
    for p in narc_bad.iter().take(10) {
        println!("       ✗ {p}");
    }

    let (mut compressed, mut bad) = (0, Vec::new());
    for ovl in rom.overlays().iter().filter(|o| o.is_compressed()) {
        compressed += 1;
        match rom.overlay(ovl.id) {
            Ok(d) if d.len() as u32 == ovl.ram_size => {}
            Ok(d) => bad.push(format!("overlay {} : {} octets, {} attendus", ovl.id, d.len(), ovl.ram_size)),
            Err(e) => bad.push(format!("overlay {} : {e}", ovl.id)),
        }
    }
    report(&format!("Décompression BLZ des {compressed} overlays compressés"), bad.is_empty(), &mut failures);
    for b in bad.iter().take(10) {
        println!("       ✗ {b}");
    }

    if failures > 0 {
        return Err(format!("{failures} vérification(s) en échec").into());
    }
    Ok(())
}

/// Fichiers d'une archive de textes (NARC sur DS, GARC sur 3DS) et génération du jeu.
struct TextArchive {
    gen: u8,
    files: Vec<Vec<u8>>,
}

fn text_archive(rom: &str, archive: &str) -> Result<TextArchive, Box<dyn std::error::Error>> {
    let detection = kaleido_core::detect_path(Path::new(rom))?;
    let gen = detection.generation.ok_or("jeu non reconnu")?;
    let files = match detection.kind {
        FileKind::NdsRom => Narc::parse(open(rom).file_by_path(archive)?)?.files,
        FileKind::CtrRom | FileKind::CtrDump => {
            let garc = Garc::parse(&ctr::open(rom).read(archive)?)?;
            garc.entries.iter().map(|e| e.data().unwrap_or_default().to_vec()).collect()
        }
        _ => return Err("ce n'est pas une ROM DS ou 3DS".into()),
    };
    Ok(TextArchive { gen, files })
}

fn parse_gen5(gen: u8, data: &[u8]) -> Result<gen5::MsgFile, kaleido_core::text::TextError> {
    gen5::MsgFile::parse_with(data, Variant::for_generation(gen))
}

fn strings(gen: u8, data: &[u8]) -> Result<Vec<String>, kaleido_core::text::TextError> {
    if gen == 4 {
        Ok(gen4::MsgFile::parse(data)?.strings())
    } else {
        Ok(parse_gen5(gen, data)?.strings())
    }
}

fn printable(s: &str) -> String {
    s.replace('\n', "⏎").replace('\r', "⇣").replace('\u{c}', "¶")
}

fn text(rom: &str, archive: &str, n: usize) -> CliResult {
    let TextArchive { gen, files } = text_archive(rom, archive)?;
    let file = files.get(n).ok_or("numéro de fichier hors de l'archive")?;
    for (i, s) in strings(gen, file)?.iter().enumerate() {
        println!("{i:4}: {}", printable(s));
    }
    Ok(())
}

fn find(rom: &str, archive: &str, needle: &str) -> CliResult {
    let TextArchive { gen, files } = text_archive(rom, archive)?;
    let needle = needle.to_lowercase();
    for (n, file) in files.iter().enumerate() {
        let Ok(list) = strings(gen, file) else { continue };
        for (i, s) in list.iter().enumerate().filter(|(_, s)| s.to_lowercase().contains(&needle)) {
            let s: String = printable(s).chars().take(90).collect();
            println!("fichier {n:4} ({:4} entrées)  #{i:<4} {s}", list.len());
        }
    }
    Ok(())
}

fn text_check(rom: &str, archive: &str) -> CliResult {
    let TextArchive { gen, files } = text_archive(rom, archive)?;
    let variant = Variant::for_generation(gen);
    let (mut files_ok, mut strings_total, mut strings_ok, mut roundtrip_ok, mut single_block) = (0, 0, 0, 0, 0);
    let mut unknown: BTreeMap<String, usize> = BTreeMap::new();
    let mut mismatches = Vec::new();

    for (n, file) in files.iter().enumerate() {
        // (codes bruts, réencodage) pour chaque chaîne du fichier.
        let (rewritten, pairs): (Vec<u8>, Vec<(Vec<u16>, Result<Vec<u16>, _>, String)>) = if gen == 4 {
            let m = gen4::MsgFile::parse(file)?;
            let pairs = m.entries.iter().map(|c| {
                let s = gen4::decode(c);
                (c.clone(), gen4::encode(&s), s)
            });
            (m.to_bytes(), pairs.collect())
        } else {
            let m = parse_gen5(gen, file)?;
            // Aller-retour complet par les chaînes (fichiers à un seul bloc : toutes les langues sauf le japonais).
            if m.blocks.len() == 1 {
                single_block += 1;
                let mut copy = m.clone();
                copy.set_strings(&m.strings())?;
                if copy.to_bytes() == *file {
                    roundtrip_ok += 1;
                } else if mismatches.len() < 5 {
                    mismatches.push(format!("fichier {n} : aller-retour par les chaînes différent"));
                }
            }
            let pairs = m.blocks.iter().flatten().map(|e| {
                let s = gen5::decode_with(&e.codes, variant);
                (e.codes.clone(), gen5::encode_with(&s, variant), s)
            });
            (m.to_bytes(), pairs.collect())
        };
        if rewritten == *file {
            files_ok += 1;
        } else if mismatches.len() < 5 {
            mismatches.push(format!("fichier {n} : réécriture différente ({} → {} octets)", file.len(), rewritten.len()));
        }

        for (codes, reencoded, s) in pairs {
            strings_total += 1;
            // Les chaînes compressées (0xF100) sont réencodées sans compression : on compare le texte.
            let same = match &reencoded {
                Ok(r) if *r == codes => true,
                Ok(r) if gen == 4 && codes.first() == Some(&0xF100) => gen4::decode(r) == s,
                // Lignes de taille fixe : la suite n'est faite que de terminateurs.
                Ok(r) if gen >= 5 && codes.starts_with(r) && codes[r.len()..].iter().all(|&c| Some(&c) == r.last()) => true,
                _ => false,
            };
            if same {
                strings_ok += 1;
            } else if mismatches.len() < 15 {
                mismatches.push(format!("fichier {n} : « {} » non réencodable à l'identique", printable(&s)));
            }
            let mut rest = s.as_str();
            while let Some(at) = rest.find("{X:") {
                *unknown.entry(rest[at..at + 8].to_string()).or_default() += 1;
                rest = &rest[at + 8..];
            }
        }
    }

    println!("Fichiers réécrits à l'octet près : {files_ok}/{}", files.len());
    println!("Chaînes réencodées à l'identique : {strings_ok}/{strings_total}");
    if gen >= 5 {
        println!("Fichiers identiques après lecture et réécriture des chaînes : {roundtrip_ok}/{single_block}");
    }
    let mut unknown: Vec<_> = unknown.into_iter().collect();
    unknown.sort_by(|a, b| b.1.cmp(&a.1));
    println!("Caractères inconnus : {} distincts", unknown.len());
    for (code, count) in unknown.iter().take(40) {
        println!("    {code} × {count}");
    }
    for m in &mismatches {
        println!("  ✗ {m}");
    }
    Ok(())
}

fn species(path: &str) -> CliResult {
    let game = kaleido_core::GameRom::open(Path::new(path))?;
    print_species(&game.species()?);
    Ok(())
}

fn print_species(species: &[kaleido_core::pokemon::Species]) {
    for s in species {
        let types: Vec<_> = s.types.iter().map(|t| t.name).collect();
        let b = s.base_stats;
        let mut abilities = s.abilities.join(" / ");
        if let Some(h) = &s.hidden_ability {
            abilities.push_str(&format!(" (caché : {h})"));
        }
        println!(
            "{:03} {:<12} {:<17} {:3} {:3} {:3} {:3} {:3} {:3} = {:3}  {abilities}",
            s.id,
            s.name,
            types.join("/"),
            b.hp,
            b.attack,
            b.defense,
            b.sp_attack,
            b.sp_defense,
            b.speed,
            s.total
        );
    }
}

/// `hex3ds <rom> <chemin> [entrée|-] [début] [longueur]` (valeurs décimales ou 0x…).
fn hex3ds(rom: &str, path: &str, rest: &[&str]) -> CliResult {
    let num = |s: &str| match s.strip_prefix("0x") {
        Some(h) => usize::from_str_radix(h, 16),
        None => s.parse(),
    };
    let entry = rest.first().filter(|s| **s != "-").map(|s| num(s)).transpose()?;
    let offset = rest.get(1).map(|s| num(s)).transpose()?.unwrap_or(0);
    let len = rest.get(2).map(|s| num(s)).transpose()?.unwrap_or(0x100);
    ctr::hex(&ctr::open(rom), path, entry, offset, len)
}

fn randomize(path: &str, preset: &str, seed: u64, out: &str) -> CliResult {
    use kaleido_core::randomizer;
    let settings = load_settings(preset)?;
    let mut game = kaleido_core::GameRom::open(Path::new(path))?;
    let outcome = randomizer::randomize(&mut game, &settings, seed)?;
    game.save(Path::new(out))?;
    let names: Vec<_> = outcome.starters.iter().map(|s| s.name.as_str()).collect();
    println!("Starters : {}", names.join(", "));
    println!("{} emplacements sauvages, {} Pokémon de dresseurs", outcome.wild_slots, outcome.trainer_pokemon);
    println!("Code : {}", outcome.share_code);
    std::fs::write(format!("{out}.log.txt"), &outcome.log)?;
    Ok(())
}

/// Réglages : nom d'un préréglage, ou fichier `.json` (mêmes champs que l'interface).
pub(crate) fn load_settings(arg: &str) -> Result<kaleido_core::randomizer::Settings, Box<dyn std::error::Error>> {
    use kaleido_core::randomizer;
    if arg.ends_with(".json") {
        return Ok(serde_json::from_str(&std::fs::read_to_string(arg)?)?);
    }
    Ok(randomizer::presets().into_iter().find(|p| p.id == arg).ok_or("préréglage inconnu")?.settings)
}

fn starters(path: &str) -> CliResult {
    let game = kaleido_core::GameRom::open(Path::new(path))?;
    let loc = kaleido_core::data::DataPaths::for_game(game.game).ok_or("jeu non pris en charge")?.starters;
    let ids = kaleido_core::data::starters::read(&game, loc)?;
    let names = game.text_file(game.layout.species_names)?;
    for id in ids {
        println!("{id:3} {}", names[id as usize]);
    }
    if loc == kaleido_core::data::starters::StarterLocation::BlackWhite {
        let gifts = kaleido_core::data::starters::read_bw_gifts(&game)?;
        let list: Vec<_> = gifts.iter().map(|&g| names.get(g as usize).cloned().unwrap_or_else(|| g.to_string())).collect();
        println!("Donnés par le script 782 : {}", list.join(", "));
    }
    Ok(())
}

/// Outil de recherche : instructions Thumb `cmp rX, #8` précédées de XOR (calcul shiny).
fn shiny_scan(rom: &NdsRom) -> CliResult {
    let arm9 = kaleido_formats::lz::decompress_blz(rom.arm9()).unwrap_or_else(|_| rom.arm9().to_vec());
    println!("arm9 : {} octets ({} décompressé)", rom.arm9().len(), arm9.len());
    let mut sources: Vec<(String, Vec<u8>)> = vec![("arm9".into(), arm9)];
    for o in rom.overlays() {
        sources.push((format!("overlay {}", o.id), rom.overlay(o.id)?));
    }
    let half = |d: &[u8], at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
    for (name, d) in &sources {
        for at in (24..d.len().saturating_sub(4)).step_by(2) {
            let ins = half(d, at);
            if ins & 0xF8FF != 0x2808 {
                continue; // cmp rX, #8
            }
            let eors = (1..=12).filter(|k| (0x4040..=0x407F).contains(&half(d, at - 2 * k))).count();
            let next = half(d, at + 2);
            if eors >= 2 && matches!(next >> 8, 0xD2 | 0xD3) {
                let ctx: Vec<String> = (at - 24..at + 6).step_by(2).map(|i| format!("{:04X}", half(d, i))).collect();
                println!("{name} @ {at:#X} : eor×{eors} | {}", ctx.join(" "));
            }
        }
        // Variante ARM 32 bits : `cmp rX, #8` (E35X0008) précédé d'EOR (E02…).
        let word = |at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap());
        for at in (32..d.len().saturating_sub(8)).step_by(4) {
            if word(at) & 0xFFF0_FFFF != 0xE350_0008 {
                continue;
            }
            let eors = (1..=8).filter(|k| word(at - 4 * k) & 0x0FE0_0000 == 0x0020_0000).count();
            if eors >= 2 {
                println!("{name} @ {at:#X} (ARM) : eor×{eors} | {:08X} {:08X}", word(at), word(at + 4));
            }
        }
        // Variante « (x >> 3) == 0 » : lsr rD, rS, #3 juste après des EOR.
        for at in (24..d.len().saturating_sub(4)).step_by(2) {
            let ins = half(d, at);
            if ins & 0xFFC0 != 0x08C0 {
                continue;
            }
            let eors = (1..=6).filter(|k| (0x4040..=0x407F).contains(&half(d, at - 2 * k))).count();
            if eors >= 2 {
                let ctx: Vec<String> = (at - 24..at + 8).step_by(2).map(|i| format!("{:04X}", half(d, i))).collect();
                println!("{name} @ {at:#X} (lsr #3) : eor×{eors} | {}", ctx.join(" "));
            }
        }
        // Variante Thumb avec seuil dans un registre ou comparaison « 7 » (≤ 7).
        for at in (24..d.len().saturating_sub(4)).step_by(2) {
            let ins = half(d, at);
            if ins & 0xF8FF != 0x2807 {
                continue;
            }
            let eors = (1..=12).filter(|k| (0x4040..=0x407F).contains(&half(d, at - 2 * k))).count();
            if eors >= 2 {
                let ctx: Vec<String> = (at - 24..at + 6).step_by(2).map(|i| format!("{:04X}", half(d, i))).collect();
                println!("{name} @ {at:#X} (≤7) : eor×{eors} | {}", ctx.join(" "));
            }
        }
    }
    Ok(())
}

/// Cherche une suite de valeurs (u16 consécutifs, puis u32 consécutifs, puis
/// u16 proches à moins de 64 octets) dans l'ARM9, les overlays et les NARC.
fn search(rom: &NdsRom, values: &str) -> CliResult {
    let wanted: Vec<u32> = values.split(',').map(str::parse).collect::<Result<_, _>>()?;
    let as_u16: Vec<u8> = wanted.iter().flat_map(|v| (*v as u16).to_le_bytes()).collect();
    let as_u32: Vec<u8> = wanted.iter().flat_map(|v| v.to_le_bytes()).collect();

    let mut sources: Vec<(String, Vec<u8>)> = vec![("arm9".into(), rom.arm9().to_vec())];
    for o in rom.overlays() {
        sources.push((format!("overlay {}", o.id), rom.overlay(o.id)?));
    }
    for (id, path) in rom.files() {
        let data = rom.file(id).unwrap_or_default();
        if Narc::is_narc(data) {
            if let Ok(n) = Narc::parse(data) {
                sources.extend(n.files.into_iter().enumerate().map(|(i, f)| (format!("{path}[{i}]"), f)));
            }
        }
    }

    for (name, data) in &sources {
        for (label, needle) in [("u16", &as_u16), ("u32", &as_u32)] {
            for at in data.windows(needle.len()).enumerate().filter(|(_, w)| w == needle).map(|(i, _)| i) {
                println!("{name} @ {at:#X} ({label} consécutifs)");
            }
        }
        // Valeurs dispersées (ex. arguments de commandes de script).
        let first = (wanted[0] as u16).to_le_bytes();
        for at in data.windows(2).enumerate().filter(|(_, w)| *w == first).map(|(i, _)| i) {
            let window = &data[at..(at + 64).min(data.len())];
            let all = wanted[1..].iter().all(|v| window.windows(2).any(|w| w == (*v as u16).to_le_bytes()));
            if all && !data[at..].starts_with(&as_u16) {
                println!("{name} @ {at:#X} (proches)");
            }
        }
    }
    Ok(())
}

fn hex_entry(rom: &NdsRom, path: &str, n: usize) -> CliResult {
    let narc = Narc::parse(rom.file_by_path(path)?)?;
    let data = narc.files.get(n).ok_or("numéro hors de l'archive")?;
    println!("{} fichiers, entrée {n} : {} octets", narc.files.len(), data.len());
    for (i, row) in data.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02X}")).collect();
        let words: Vec<String> = row.chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_le_bytes([c[0], c[1]]).to_string()).collect();
        println!("{:04X}  {:<48} | {}", i * 16, hex.join(" "), words.join(" "));
    }
    Ok(())
}

/// Outil de mise au point : premiers octets qui diffèrent après réécriture.
fn text_diff(rom: &str, archive: &str, n: usize) -> CliResult {
    let TextArchive { gen, files } = text_archive(rom, archive)?;
    let file = files.get(n).ok_or("numéro de fichier hors de l'archive")?;
    let rewritten = if gen == 4 { gen4::MsgFile::parse(file)?.to_bytes() } else { parse_gen5(gen, file)?.to_bytes() };
    let hex = |d: &[u8], at: usize| d[at.saturating_sub(4)..(at + 12).min(d.len())].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
    println!("en-tête original : {}", hex(file, 4));
    println!("en-tête réécrit  : {}", hex(&rewritten, 4));
    for at in (0..file.len().min(rewritten.len())).filter(|&i| file[i] != rewritten[i]).take(8) {
        println!("@{at:#06X}  {}  |  {}", hex(file, at), hex(&rewritten, at));
    }
    Ok(())
}

fn report(label: &str, ok: bool, failures: &mut u32) {
    println!("  {}  {label}", if ok { "✓" } else { "✗" });
    if !ok {
        *failures += 1;
    }
}
