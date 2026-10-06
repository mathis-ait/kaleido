//! Rend une séquence du SDAT d'une ROM DS Pokémon en WAV.
//!
//! ```text
//! cargo run -p kaleido-core --example music_nds -- <rom.nds> <sortie.wav> [secondes] [nom ou index]
//! cargo run -p kaleido-core --example music_nds -- <rom.nds> --list      (durées des séquences TITLE / THEME)
//! cargo run -p kaleido-core --example music_nds -- <rom.nds> --list-all  (durées de toutes les séquences)
//! ```
//!
//! Sans nom ni index, c'est le thème de l'écran titre qui est rendu.

use std::path::Path;
use std::time::Instant;

use kaleido_core::games::Game;
use kaleido_core::music::nds::{find_sdat, render_sequences, sequence_timing, title_sequences, Sdat, SeqTiming};
use kaleido_formats::nds::NdsRom;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage : music_nds <rom.nds> <sortie.wav> [secondes] [nom ou index]\n        music_nds <rom.nds> --list | --list-all");
        std::process::exit(2);
    }
    if let Err(e) = run(&args) {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}

fn describe(timing: SeqTiming) -> String {
    match timing {
        SeqTiming::Loop(a, b) => format!("boucle {a:.2} s -> {b:.2} s ({:.2} s)", b - a),
        SeqTiming::End(e) => format!("sans boucle, fin à {e:.2} s"),
        SeqTiming::Unknown => "?".to_string(),
    }
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let rom = NdsRom::open(Path::new(&args[0]))?;
    let game = Game::from_nds_code(&rom.header().game_code);
    let sdat = Sdat::parse(find_sdat(&rom)?)?;
    let names = sdat.sequence_names();
    let title = game.map(|g| title_sequences(&sdat, g)).unwrap_or_default();

    if args[1] == "--list" || args[1] == "--list-all" {
        let all = args[1] == "--list-all";
        println!("{} séquences ({})", names.len(), game.map_or("jeu inconnu", |g| g.name_fr()));
        for (i, name) in names.iter().enumerate() {
            let Some(info) = sdat.sequence_info(i) else { continue };
            let mark = if title.contains(&i) { "  <- écran titre" } else { "" };
            let timing = match name.as_deref() {
                Some(n) if all || n.contains("TITLE") || n.contains("THEME") => format!("  ({})", describe(sequence_timing(&sdat, i, 600.0)?)),
                _ => String::new(),
            };
            // Empreinte du fichier SSEQ, pour repérer les séquences partagées entre jeux.
            let hash = sdat.sequence_file(i).map_or(0u32, |d| d.iter().fold(0u32, |h, &b| h.wrapping_mul(31).wrapping_add(b as u32)));
            println!(
                "{i:5}  {}  [fichier {}, banque {}, vol {}, lecteur {}, empreinte {hash:08x}]{timing}{mark}",
                name.as_deref().unwrap_or("-"),
                info.file,
                info.bank,
                info.volume,
                info.player
            );
        }
        return Ok(());
    }

    let seconds: f32 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(60.0);
    let indices = match args.get(3) {
        Some(s) => vec![match s.parse::<usize>() {
            Ok(i) => i,
            Err(_) => names.iter().position(|n| n.as_deref() == Some(s.as_str())).ok_or(format!("séquence {s} introuvable"))?,
        }],
        None if title.is_empty() => return Err("séquence de l'écran titre introuvable".into()),
        None => title,
    };
    let start = Instant::now();
    let pcm = render_sequences(&sdat, &indices, seconds)?;
    let elapsed = start.elapsed();
    std::fs::write(&args[1], pcm.to_wav())?;

    for &i in &indices {
        println!("séquence {i} ({}) : {}", names.get(i).cloned().flatten().unwrap_or_default(), describe(sequence_timing(&sdat, i, 600.0)?));
    }
    let peak = pcm.samples.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0);
    let rms = (pcm.samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / pcm.samples.len().max(1) as f64).sqrt();
    let clipped = pcm.samples.iter().filter(|&&s| s == i16::MAX || s == i16::MIN).count();
    println!(
        "{:.2} s à {} Hz, crête {peak}, RMS {rms:.0}, échantillons écrêtés {clipped}, rendu en {:.0} ms",
        pcm.seconds(),
        pcm.sample_rate,
        elapsed.as_secs_f64() * 1000.0
    );
    Ok(())
}
