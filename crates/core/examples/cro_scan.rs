// Modules CRO présents dans la mémoire d'Azahar : cro_scan <pid>
// En-tête CRO : « CRO0 » à +0x80, offset/adresse du nom à +0x84 (3dbrew).
#[cfg(windows)]
fn main() {
    use kaleido_core::live::{scan, windows::Process, MemorySource};
    let pid: u32 = std::env::args().nth(1).unwrap().parse().unwrap();
    let proc = Process::open(pid).unwrap();
    let regions = if std::env::var_os("ALL").is_some() { proc.regions() } else { scan::ctr_candidate_regions(&proc) };
    let mut buf = vec![0u8; 1 << 20];
    for r in &regions {
        let mut at = r.base;
        while at < r.end() {
            let len = ((r.end() - at) as usize).min(buf.len());
            if proc.read(at, &mut buf[..len]).is_ok() {
                let mut i = 0;
                while i + 0x100 <= len {
                    if &buf[i + 0x80..i + 0x84] == b"CRO0" {
                        let name_ofs = u32::from_le_bytes(buf[i + 0x84..i + 0x88].try_into().unwrap());
                        let next = u32::from_le_bytes(buf[i + 0x88..i + 0x8C].try_into().unwrap());
                        let prev = u32::from_le_bytes(buf[i + 0x8C..i + 0x90].try_into().unwrap());
                        // Nom : offset depuis le début (non chargé) ou adresse virtuelle (chargé).
                        let rel = if (name_ofs as usize) < 0x100000 { name_ofs as usize } else { (name_ofs & 0xFFF) as usize + 0 };
                        let mut name = String::new();
                        let nb = proc.read_vec(at + i as u64 + rel as u64, 32).unwrap_or_default();
                        for &c in nb.iter().take_while(|&&c| c != 0) { name.push(c as char); }
                        println!("{:x} nom@{:08x} next {:08x} prev {:08x} {name}", at + i as u64, name_ofs, next, prev);
                    }
                    i += 4;
                }
            }
            at += len as u64;
        }
    }
}
#[cfg(not(windows))]
fn main() {}
