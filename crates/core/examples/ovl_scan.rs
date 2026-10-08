// Overlays chargés dans des dumps de RAM : ovl_scan <rom.nds> <dump1,dump2,...>
// Pour chaque overlay, « o » si ses 512 premiers octets sont en RAM à son adresse, « . » sinon.
use kaleido_formats::nds::NdsRom;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let rom = NdsRom::open(std::path::Path::new(&a[0])).unwrap();
    let dumps: Vec<(String, Vec<u8>)> = a[1].split(',').map(|f| (f.rsplit('/').next().unwrap().to_string(), std::fs::read(f).unwrap())).collect();
    println!("dumps: {}", dumps.iter().map(|d| d.0.as_str()).collect::<Vec<_>>().join(" "));
    for o in rom.overlays() {
        let Ok(code) = rom.overlay(o.id) else { continue };
        let n = code.len().min(512);
        if n < 64 { continue }
        let ofs = (o.ram_address as usize) & 0x3F_FFFF;
        let marks: String = dumps.iter().map(|(_, d)| if d.get(ofs..ofs + n) == Some(&code[..n]) { 'o' } else { '.' }).collect();
        if marks.contains('o') {
            println!("ovl {:3} @ {:08x} taille {:6}  {marks}", o.id, o.ram_address, o.ram_size);
        }
    }
}
