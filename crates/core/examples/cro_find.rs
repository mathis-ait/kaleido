// Cherche en mémoire d'Azahar le début d'un module CRO de la ROM : cro_find <pid> <rom.3ds> <nom>
#[cfg(windows)]
fn main() {
    use kaleido_core::live::{windows::Process, MemorySource};
    use kaleido_formats::romfs::RomFsSource;
    let a: Vec<String> = std::env::args().skip(1).collect();
    let fs = RomFsSource::open(std::path::Path::new(&a[1])).unwrap();
    let path = fs.files().iter().map(|f| f.path.clone()).find(|p| p.ends_with(&format!("{}.cro", a[2]))).expect("CRO absent");
    let cro = fs.read(&path).unwrap();
    println!("{path} {} octets", cro.len());
    let proc = Process::open(a[0].parse().unwrap()).unwrap();
    // Plusieurs morceaux : empreintes (0x00), code (0x1000, 0x8000).
    for ofs in [0usize, 0x1000, 0x8000] {
        let needle = &cro[ofs..ofs + 32];
        let mut hits = Vec::new();
        let mut buf = vec![0u8; 1 << 20];
        for r in proc.regions() {
            let mut at = r.base;
            while at < r.end() {
                let len = ((r.end() - at) as usize).min(buf.len());
                if proc.read(at, &mut buf[..len]).is_ok() {
                    for i in (0..len.saturating_sub(32)).step_by(4) {
                        if &buf[i..i + 32] == needle { hits.push(at + i as u64); }
                    }
                }
                at += len as u64;
            }
        }
        println!("+{ofs:#x}: {} occurrence(s) {:x?}", hits.len(), hits.iter().take(4).collect::<Vec<_>>());
    }
}
#[cfg(not(windows))]
fn main() {}
