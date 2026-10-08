// Part de chaque overlay présente en RAM (octets égaux / taille) : ovl_match <rom.nds> <dump1,dump2,...>
// Ou extrait un overlay décompressé : ovl_match <rom.nds> - <id> <sortie.bin>
use kaleido_formats::nds::NdsRom;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let rom = NdsRom::open(std::path::Path::new(&a[0])).unwrap();
    if let Some(out) = a.get(3) {
        let id: u32 = a[2].parse().unwrap();
        std::fs::write(out, rom.overlay(id).unwrap()).unwrap();
        return;
    }
    let dumps: Vec<Vec<u8>> = a[1].split(',').map(|f| std::fs::read(f).unwrap()).collect();
    for o in rom.overlays() {
        let Ok(code) = rom.overlay(o.id) else { continue };
        let ofs = (o.ram_address as usize) & 0x3F_FFFF;
        let ratios: Vec<String> = dumps
            .iter()
            .map(|d| {
                let n = code.len().min(d.len().saturating_sub(ofs));
                let same = (0..n).filter(|&i| d[ofs + i] == code[i]).count();
                format!("{:5.1}%", 100.0 * same as f64 / code.len().max(1) as f64)
            })
            .collect();
        // Début de l'overlay retrouvé ailleurs en RAM (chargé à une autre adresse ?).
        let probe = &code[code.len().min(0x40)..code.len().min(0x80)];
        let elsewhere: Vec<String> = dumps
            .iter()
            .map(|d| {
                if probe.len() < 64 {
                    "-".into()
                } else {
                    d.windows(64).step_by(4).position(|w| w == probe).map_or("-".into(), |p| format!("{:08x}", 0x0200_0000 + p * 4 - 0x40))
                }
            })
            .collect();
        println!("ovl {:3} @ {:08x} taille {:6}  {}  ailleurs {}", o.id, o.ram_address, code.len(), ratios.join("  "), elsewhere.join(" "));
    }
}
