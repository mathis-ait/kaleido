// Plus gros overlays d'une ROM DS : ovl_list <rom.nds>
use kaleido_formats::nds::NdsRom;
fn main() {
    let rom = NdsRom::open(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap();
    let mut v: Vec<_> = rom.overlays().to_vec();
    v.sort_by_key(|o| std::cmp::Reverse(o.ram_size));
    for o in v.iter().take(8) {
        println!("ovl {:3} @ {:08x}..{:08x} taille {:6}", o.id, o.ram_address, o.ram_address + o.ram_size, o.ram_size);
    }
}
