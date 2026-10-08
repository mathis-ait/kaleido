// Tous les Pokémon valides (format équipe) dans les grandes zones d'Azahar : mon_scan <pid> <gen6|gen7>
// Encodage : la constante de chiffrement (4 premiers octets) ne doit pas être nulle.
#[cfg(windows)]
fn main() {
    use kaleido_core::live::reader::decode_party_mon;
    use kaleido_core::live::{scan, windows::Process, MemorySource};
    use kaleido_core::save::PkmFormat;
    let a: Vec<String> = std::env::args().skip(1).collect();
    let format = if a[1] == "gen7" { PkmFormat::Gen7 } else { PkmFormat::Gen6 };
    let proc = Process::open(a[0].parse().unwrap()).unwrap();
    let mut buf = vec![0u8; (8 << 20) + 260];
    for r in scan::ctr_candidate_regions(&proc) {
        let mut at = r.base;
        while at < r.end() {
            let len = ((r.end() - at) as usize).min(buf.len());
            if proc.read(at, &mut buf[..len]).is_ok() {
                let mut i = 0;
                while i + 260 <= len {
                    // Filtre rapide : octets 4-5 = 0 (inutilisés) dans les formats 3DS.
                    if buf[i + 4] == 0 && buf[i + 5] == 0 && buf[i..i + 4] != [0, 0, 0, 0] {
                        if let Some(p) = decode_party_mon(format, &buf[i..i + 260]) {
                            println!("{:x} esp {} N.{} PV {} TID {}", at + i as u64, p.species(), p.party_level().unwrap_or(0), p.current_hp(), p.tid());
                        }
                    }
                    i += 4;
                }
            }
            at += (len - 260) as u64;
        }
    }
}
#[cfg(not(windows))]
fn main() {}
