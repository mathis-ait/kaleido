// Seuil des chromatiques d'une ROM DS : shiny_rate <rom.nds>
fn main() {
    let path = std::env::args().nth(1).expect("chemin de la ROM");
    match kaleido_core::data::shiny::rom_threshold(std::path::Path::new(&path)) {
        Some(t) if t >= 256 => println!("seuil {t} : toujours chromatique"),
        Some(t) => println!("seuil {t} : 1 sur {}", 65536 / t as u32),
        None => println!("seuil introuvable"),
    }
}
