//! Cherche les polices Gen 4 (en-tête 0x10, table des largeurs) et exporte les largeurs.
//! `cargo run --release -p kaleido-core --example font_scan -- rom.nds sortie.json`
use kaleido_formats::{narc::Narc, nds::NdsRom};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rom = NdsRom::open(args[0].as_ref()).unwrap();
    let mut out = Vec::new();
    for (id, path) in rom.files() {
        let Some(d) = rom.file(id) else { continue };
        if !Narc::is_narc(d) { continue }
        let Ok(narc) = Narc::parse(d) else { continue };
        for (i, f) in narc.files.iter().enumerate() {
            if f.len() < 0x20 { continue }
            let u = |o: usize| u32::from_le_bytes(f[o..o + 4].try_into().unwrap()) as usize;
            let (hdr, wofs, n) = (u(0), u(4), u(8));
            if hdr == 0x10 && n > 100 && n < 2000 && wofs + n == f.len() {
                println!("{path} #{i}: {n} glyphes, cellule {}x{}", f[12], f[13]);
                out.push(format!("\"{path}#{i}\":{:?}", &f[wofs..wofs + n]));
            }
        }
    }
    std::fs::write(&args[1], format!("{{{}}}", out.join(","))).unwrap();
}
