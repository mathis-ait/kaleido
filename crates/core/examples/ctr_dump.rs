// Zones mémoire d'Azahar et copie brute : ctr_dump <pid> [adresse_hex sortie.bin]
// Sans adresse : liste des zones candidates (FCRAM). Avec : copie de l'allocation qui contient l'adresse.
#[cfg(windows)]
fn main() {
    use kaleido_core::live::{MemorySource, scan, windows::Process};
    let a: Vec<String> = std::env::args().skip(1).collect();
    let proc = Process::open(a[0].parse().unwrap()).unwrap();
    let regions = if std::env::var_os("ALL").is_some() { proc.regions() } else { scan::ctr_candidate_regions(&proc) };
    if a.len() == 2 {
        let addr = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
        let b = proc.read_vec(addr, 32).unwrap();
        println!("{:02x?}", b);
        return;
    }
    if a.len() < 3 {
        let mut total = 0;
        for r in &regions {
            total += r.size;
            println!("{:x} {:>6} Mio (alloc {:x})", r.base, r.size >> 20, r.allocation);
        }
        println!("{} zones, {} Mio", regions.len(), total >> 20);
        return;
    }
    let addr = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let r = regions.iter().find(|r| r.contains(addr, 1)).expect("adresse hors des zones");
    let mut out = Vec::with_capacity(r.size as usize);
    let mut buf = vec![0u8; 4 << 20];
    let mut at = r.base;
    while at < r.end() {
        let len = ((r.end() - at) as usize).min(buf.len());
        if proc.read(at, &mut buf[..len]).is_err() {
            buf[..len].fill(0);
        }
        out.extend_from_slice(&buf[..len]);
        at += len as u64;
    }
    std::fs::write(&a[2], &out).unwrap();
    println!("{:x} + {:x} -> {}", r.base, r.size, a[2]);
}
#[cfg(not(windows))]
fn main() {}
