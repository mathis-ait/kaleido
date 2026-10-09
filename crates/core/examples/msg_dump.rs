//! Exporte les textes Gen 4 ou Gen 5 d'une ROM : un fichier par banque, une ligne par message.
//! `cargo run --release -p kaleido-core --example msg_dump -- rom.nds dossier [chemin_narc] [gen5]`
use kaleido_formats::{narc::Narc, nds::NdsRom};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rom = NdsRom::open(args[0].as_ref()).unwrap();
    let dir = std::path::Path::new(&args[1]);
    let path = args.get(2).map(String::as_str).unwrap_or("a/0/2/7");
    let gen5 = args.get(3).is_some_and(|a| a == "gen5");
    std::fs::create_dir_all(dir).unwrap();
    let narc = Narc::parse(rom.file_by_path(path).unwrap()).unwrap();
    for (i, f) in narc.files.iter().enumerate() {
        let strings = if gen5 {
            kaleido_core::text::gen5::MsgFile::parse(f).map(|m| m.strings()).map_err(|e| e.to_string())
        } else {
            kaleido_core::text::gen4::MsgFile::parse(f).map(|m| m.strings()).map_err(|e| e.to_string())
        };
        let lines: Vec<String> = match strings {
            Ok(s) => s.iter().map(|s| s.replace('\n', "\\n").replace('\r', "\\r").replace('\u{c}', "\\f")).collect(),
            Err(e) => vec![format!("!! {e}")],
        };
        std::fs::write(dir.join(format!("{i:04}.txt")), lines.join("\n")).unwrap();
    }
    println!("{} banques", narc.files.len());
}
