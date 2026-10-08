// Modules CRO d'une ROM 3DS : cro_list <rom.3ds>
use kaleido_formats::romfs::RomFsSource;
fn main() {
    let fs = RomFsSource::open(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap();
    let names: Vec<String> = fs.files().iter().filter(|f| f.path.ends_with(".cro")).map(|f| f.path.trim_start_matches('/').trim_end_matches(".cro").to_string()).collect();
    println!("{} modules : {}", names.len(), names.iter().filter(|n| n.contains("Battle") || n.contains("Field")).cloned().collect::<Vec<_>>().join(", "));
}
