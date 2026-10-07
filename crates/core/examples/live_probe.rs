//! Diagnostic du compagnon en direct (lecture seule).
//!
//! ```text
//! cargo run -p kaleido-core --example live_probe -- list
//! cargo run -p kaleido-core --example live_probe -- dump <pid> <sortie.bin>
//! cargo run -p kaleido-core --example live_probe -- watch <pid> <sauvegarde> [secondes]
//! ```
#[cfg(windows)]
fn main() {
    use kaleido_core::live::reader::{Console, LiveReader};
    use kaleido_core::live::windows::{processes, Process};
    use kaleido_core::live::{scan, MemorySource};
    use kaleido_core::save::session::SaveSession;
    use std::time::{Duration, Instant};

    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    match arg(0) {
        "list" => {
            for p in processes() {
                let e = p.exe.to_lowercase();
                if ["melonds", "desmume", "azahar", "citra", "lime3ds"].iter().any(|n| e.contains(n)) {
                    println!("{} {}", p.pid, p.exe);
                }
            }
        }
        "dump" => {
            let proc = Process::open(arg(1).parse().unwrap()).unwrap();
            let t = Instant::now();
            let regions = proc.regions();
            println!("{} zones lisibles en {:?}", regions.len(), t.elapsed());
            let t = Instant::now();
            let ram = scan::find_ds_ram(&proc).expect("RAM DS introuvable");
            println!("RAM DS : {ram:x?} trouvée en {:?}", t.elapsed());
            let bytes = proc.read_vec(ram.base, ram.size as usize).unwrap();
            std::fs::write(arg(2), bytes).unwrap();
            println!("copie écrite : {}", arg(2));
        }
        "watch" => {
            let proc = Process::open(arg(1).parse().unwrap()).unwrap();
            let save = SaveSession::open(&std::fs::read(arg(2)).unwrap()).unwrap();
            let mut hints = save.save.ram_hints();
            // KALEIDO_NO_KEYS=1 : simule une sauvegarde sans Pokémon (recherche par le dresseur).
            if std::env::var_os("KALEIDO_NO_KEYS").is_some() {
                hints.party_keys.clear();
            }
            println!("dresseur {} {:?} ; équipe {:08x?}", save.save.trainer().name, hints.trainer_name_bytes.len(), hints.party_keys);
            let t = Instant::now();
            let console = match scan::find_ds_ram(&proc) {
                Some(r) => Console::Ds(r),
                None => Console::Ctr,
            };
            println!("console {console:x?} en {:?}", t.elapsed());
            let mut reader = LiveReader::new(console, hints);
            let t = Instant::now();
            let first = reader.tick(&proc);
            println!(
                "premier tick en {:?} ({}), copies {:x?}",
                t.elapsed(),
                first.as_ref().map(|r| r.is_some()).unwrap_or(false),
                reader.party_addresses()
            );
            reader.prime_battle(&proc);
            let secs: u64 = arg(3).parse().unwrap_or(10);
            let end = Instant::now() + Duration::from_secs(secs);
            let mut last = String::new();
            let mut worst = Duration::ZERO;
            let mut ticks = 0;
            while Instant::now() < end {
                let t = Instant::now();
                let r = reader.tick(&proc);
                worst = worst.max(t.elapsed());
                ticks += 1;
                let line = match r {
                    Ok(Some(r)) => {
                        let party: Vec<String> = r
                            .party
                            .iter()
                            .map(|p| {
                                format!(
                                    "{} N.{} {}/{}",
                                    p.species(),
                                    p.party_level().unwrap_or(0),
                                    p.current_hp(),
                                    p.party_stats().map_or(0, |s| s[0])
                                )
                            })
                            .collect();
                        let battle = r.battle.map(|b| {
                            let e: Vec<String> = b
                                .enemies
                                .iter()
                                .map(|p| {
                                    format!(
                                        "{} N.{} {}/{}",
                                        p.species(),
                                        p.party_level().unwrap_or(0),
                                        p.current_hp(),
                                        p.party_stats().map_or(0, |s| s[0])
                                    )
                                })
                                .collect();
                            format!(" | combat {} {}{:?}", if b.wild { "sauvage" } else { "dresseur" }, if b.new { "NOUVEAU " } else { "" }, e)
                        });
                        format!("carte {:?} badges {:?} équipe {:?}{}", r.map, r.badges, party, battle.unwrap_or_default())
                    }
                    Ok(None) => "rien ce tick".into(),
                    Err(e) => format!("erreur : {e}"),
                };
                if line != last {
                    println!("[{:>6} ms] {line}", (secs * 1000) as i64 - (end - Instant::now()).as_millis() as i64);
                    last = line;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            println!("{ticks} ticks, le plus long {worst:?}, copies {:x?}", reader.party_addresses());
        }
        "save" => {
            let save = SaveSession::open(&std::fs::read(arg(1)).unwrap()).unwrap();
            let live = save.live().unwrap();
            println!("{:x?}", save.save.ram_hints());
            println!("{} {} badges {:?} carte {} temps {:?}", live.game, live.trainer.name, live.badges, live.map, live.play_time);
            for m in &live.party {
                println!("  {} N.{} {}/{}", m.species_name, m.level, m.hp, m.max_hp);
            }
        }
        "find" => {
            let proc = Process::open(arg(1).parse().unwrap()).unwrap();
            let ram = scan::find_ds_ram(&proc).expect("RAM DS introuvable");
            let hex = arg(2);
            let needle: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
            let region = kaleido_core::live::Region { base: ram.base, size: ram.size, allocation: ram.base };
            for a in scan::find_all(&proc, &[region], &needle, 1, 100) {
                println!("{:#08x}", 0x0200_0000 + (a - ram.base));
            }
        }
        "fixture" => {
            let proc = Process::open(arg(1).parse().unwrap()).unwrap();
            let save = SaveSession::open(&std::fs::read(arg(2)).unwrap()).unwrap();
            let ram = scan::find_ds_ram(&proc).expect("RAM DS introuvable");
            let mut reader = LiveReader::new(Console::Ds(ram.clone()), save.save.ram_hints());
            for _ in 0..4 {
                let _ = reader.tick(&proc);
            }
            let mut keep = vec![(scan::DS_HEADER_OFFSET as u32, 0x200)];
            for a in reader.party_addresses().into_iter().chain(reader.enemy_address()) {
                let ofs = (a - ram.base) as u32;
                keep.push((ofs.saturating_sub(0x400), 0x1C00));
            }
            keep.sort();
            println!("morceaux {keep:x?}");
            let bytes = kaleido_core::live::DumpSource::to_sparse(&proc, ram.base, ram.size as u32, &keep).unwrap();
            std::fs::write(arg(3), bytes).unwrap();
        }
        // Rejoue un dump brut (sortie de « dump ») : replay <dump.bin> <sauvegarde> [prime]
        // Rejoue des dumps bruts (sortie de « dump ») dans l'ordre, avec le même lecteur :
        // replay <dump1.bin,dump2.bin,…> <sauvegarde> [prime]
        "replay" => {
            let save = SaveSession::open(&std::fs::read(arg(2)).unwrap()).unwrap();
            let mut reader: Option<LiveReader> = None;
            for (i, file) in arg(1).split(',').enumerate() {
                let src = kaleido_core::live::DumpSource::new().with_zone(0x1000_0000, std::fs::read(file).unwrap());
                let ram = scan::find_ds_ram(&src).expect("RAM DS introuvable");
                let reader = reader.get_or_insert_with(|| LiveReader::new(Console::Ds(ram.clone()), save.save.ram_hints()));
                if i == 0 && arg(3) == "prime" {
                    let _ = reader.tick(&src);
                    reader.prime_battle(&src);
                }
                println!("-- {file}");
                for _ in 0..4 {
                    match reader.tick(&src) {
                        Ok(Some(r)) => println!(
                            "équipe {:?} carte {:?} combat {:?}",
                            r.party.iter().map(|p| (p.species(), p.shiny_xor(), p.tid(), p.sid())).collect::<Vec<_>>(),
                            r.map,
                            r.battle.map(|b| (b.wild, b.new, b.enemies.iter().map(|p| p.species()).collect::<Vec<_>>()))
                        ),
                        other => println!("{other:?}"),
                    }
                }
            }
        }
        _ => eprintln!("usage : list | dump <pid> <sortie> | watch <pid> <sauvegarde> [s]"),
    }
}

#[cfg(not(windows))]
fn main() {}
