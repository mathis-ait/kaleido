//! Installe un romhack traduit : `romhack_fr <id> <rom_base> <rom_reference> <patch> <sortie>`.
use kaleido_core::romhack;
use kaleido_formats::nds::NdsRom;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let def = romhack::find(&a[0]).expect("hack inconnu");
    let base = std::fs::read(&a[1]).unwrap();
    assert_eq!(romhack::sha1_hex(&base), def.base_sha1, "mauvaise ROM de base");
    let patched = romhack::apply_patch(&base, &std::fs::read(&a[3]).unwrap()).unwrap();
    assert_eq!(romhack::sha1_hex(&patched), def.patched_sha1);
    let t = def.translation.unwrap();
    let mut hack = NdsRom::from_bytes(patched).unwrap();
    let base = NdsRom::from_bytes(base).unwrap();
    let reference = NdsRom::open(a[2].as_ref()).unwrap();
    let pack = romhack::TextPack::parse(t.pack).unwrap();
    let stats = romhack::translate_gen4(&mut hack, &base, &reference, t.msg_path, &pack).unwrap();
    std::fs::write(&a[4], hack.to_bytes().unwrap()).unwrap();
    println!("{stats:?}");
}
