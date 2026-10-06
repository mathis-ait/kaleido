//! Musique d'un jeu Pokémon 3DS vers un fichier WAV.
//!
//! ```text
//! cargo run -p kaleido-core --example music_ctr -- <jeu.3ds ou dossier> <sortie.wav> [secondes] [nom/index]
//! cargo run -p kaleido-core --example music_ctr -- <jeu.3ds ou dossier> --list
//! ```
//!
//! Sans `nom/index` : thème de l'écran titre. Sinon : un flux de `--list`, par son
//! index ou par une partie de son chemin (ex. `bgm_nj_pokecen`).

use std::path::Path;
use std::time::Instant;

use kaleido_core::music::ctr;
use kaleido_core::Game;
use kaleido_formats::romfs::RomFsSource;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage : music_ctr <jeu.3ds ou dossier> <sortie.wav | --list> [secondes] [nom/index]");
        std::process::exit(2);
    }
    let rom = Path::new(&args[0]);
    let streams = ctr::list_streams(rom).expect("RomFS illisible");
    if args[1] == "--list" {
        for (i, s) in streams.iter().enumerate() {
            println!("{i:4}  {s}");
        }
        return;
    }
    let seconds: f32 = args.get(2).map_or(60.0, |s| s.parse().expect("durée invalide"));
    let t = Instant::now();
    let pcm = match args.get(3) {
        None => {
            let game = RomFsSource::open(rom).ok().and_then(|s| s.title_id()).and_then(Game::from_title_id).expect("jeu 3DS non reconnu");
            println!("{} : thème de l'écran titre ({})", game.name_fr(), ctr::title_files(game).join(", "));
            ctr::title_theme(rom, game, seconds)
        }
        Some(sel) => {
            let file = sel
                .parse::<usize>()
                .ok()
                .and_then(|i| streams.get(i))
                .or_else(|| streams.iter().find(|s| s.contains(sel.as_str())))
                .expect("flux introuvable (voir --list)");
            println!("{file}");
            ctr::stream(rom, file, seconds)
        }
    }
    .unwrap_or_else(|e| panic!("{e}"));
    let elapsed = t.elapsed();
    let peak = pcm.samples.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);
    let rms = (pcm.samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / pcm.samples.len().max(1) as f64).sqrt();
    println!("{} Hz, {:.2} s, crête {peak}, RMS {rms:.0}, extrait et décodé en {elapsed:?}", pcm.sample_rate, pcm.seconds());
    std::fs::write(&args[1], pcm.to_wav()).expect("écriture du WAV");
}
