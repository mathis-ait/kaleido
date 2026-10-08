// Pokémon en combat 3DS : ctr_battle <dump.bin | pid> [gen7]
// Dump : copie brute (ctr_dump) ; pid : FCRAM d'Azahar lue en direct.
fn main() {
    use kaleido_core::live::{DumpSource, MemorySource, ctr};
    use kaleido_core::save::PkmFormat;
    let a: Vec<String> = std::env::args().skip(1).collect();
    let format = if a.get(1).map(String::as_str) == Some("gen7") { PkmFormat::Gen7 } else { PkmFormat::Gen6 };
    let run = |src: &dyn MemorySource, regions: Vec<kaleido_core::live::Region>| {
        let t = std::time::Instant::now();
        let found = ctr::scan(src, &regions, format, &ctr::GEN6_PARAM);
        println!("balayage {:?} : {} Pokémon, {} blocs", t.elapsed(), found.mons.len(), found.params.len());
        for (at, m) in &found.mons {
            println!("  pk {at:#x} esp {} ec {:08x}", m.species(), m.encryption_constant());
        }
        for (at, p) in found.params.iter().filter(|_| std::env::var_os("PARAMS").is_some()) {
            println!("  bloc {at:#x} {p:?}");
        }
        for f in ctr::fighters(src, &found) {
            let p = f.pokemon();
            println!(
                "combattant bloc {:#x} ec {:08x} esp {} N.{:?} {}/{:?} TID {} SID {} chromatique {}",
                f.param_at,
                f.key(),
                p.species(),
                p.party_level(),
                p.current_hp(),
                p.party_stats(),
                p.tid(),
                p.sid(),
                p.is_shiny()
            );
        }
    };
    if let Ok(pid) = a[0].parse::<u32>() {
        #[cfg(windows)]
        {
            let proc = kaleido_core::live::windows::Process::open(pid).unwrap();
            let regions = kaleido_core::live::scan::ctr_candidate_regions(&proc);
            run(&proc, regions);
        }
        let _ = pid;
    } else {
        let src = DumpSource::new().with_zone(0x1000_0000, std::fs::read(&a[0]).unwrap());
        let regions = src.regions();
        run(&src, regions);
    }
}
