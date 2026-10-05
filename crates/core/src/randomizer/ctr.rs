//! Randomizer 3DS (Rubis Oméga / Saphir Alpha) : mêmes réglages que sur DS, mais
//! le résultat est un dossier LayeredFS (seuls les fichiers modifiés sont écrits),
//! à utiliser avec Luma3DS sur console ou dans le dossier « mods » d'un émulateur.
//!
//! Les starters ne sont pas modifiés : ils sont définis dans un module `.cro`
//! dont la signature (static.crr) devrait aussi être mise à jour.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kaleido_formats::garc::Garc;
use kaleido_formats::lz;
use serde::{Deserialize, Serialize};

use super::{apply_personal, randomize_trainers, randomize_wild, share_code, Ctx, Outcome, Settings, Sources, TrainerMode, WildMode};
use crate::ctr_rom::CtrGameRom;
use crate::data::encounters;
use crate::games::Game;
use crate::rom::RomError;

/// Plus grand identifiant de talent de Rubis Oméga / Saphir Alpha.
const ORAS_MAX_ABILITY: u16 = 191;

/// Destination du dossier LayeredFS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayeredFsTarget {
    /// `luma/titles/<title ID>/romfs/…` : à copier à la racine de la carte SD.
    #[default]
    Luma,
    /// `<title ID>/romfs/…` : à placer dans le dossier « mods » d'un émulateur (Azahar, Citra…).
    Emulator,
}

pub fn supports(game: Game) -> bool {
    matches!(game, Game::OmegaRuby | Game::AlphaSapphire)
}

fn unsupported() -> RomError {
    RomError::Unsupported("le randomizer 3DS prend en charge Rubis Oméga et Saphir Alpha pour l'instant".into())
}

/// Entrées d'une archive GARC (première sous-entrée de chacune).
fn entries(garc: &Garc) -> Vec<Vec<u8>> {
    (0..garc.len()).map(|i| garc.file(i).map(<[u8]>::to_vec).unwrap_or_default()).collect()
}

fn load(game: &CtrGameRom, settings: &Settings) -> Result<(Ctx, Garc), RomError> {
    let l = game.layout;
    let personal = game.garc(l.personal)?;
    let evolutions = entries(&game.garc(l.evolution)?);
    let learnsets = entries(&game.garc(l.levelup)?);
    let src = Sources {
        gen: game.generation(),
        count: l.species_count,
        names: game.text_file(l.species_names)?,
        personal: entries(&personal),
        evolutions: &evolutions,
        learnsets: &learnsets,
        max_ability: ORAS_MAX_ABILITY,
    };
    Ok((Ctx::new(src, settings)?, personal))
}

/// Randomise et écrit les fichiers modifiés sous `out_dir`. Renvoie le résultat et
/// le dossier `romfs` créé.
pub fn randomize(game: &CtrGameRom, settings: &Settings, seed: u64, out_dir: &Path, target: LayeredFsTarget) -> Result<(Outcome, PathBuf), RomError> {
    if !supports(game.game) {
        return Err(unsupported());
    }
    let l = game.layout;
    let (mut ctx, mut personal) = load(game, settings)?;
    let code = share_code(seed, settings);
    let mut log = String::new();
    let _ = writeln!(log, "Kaleido — journal de randomisation (3DS)");
    let _ = writeln!(log, "Jeu : {} ({:016X})", game.game.name_fr(), game.title_id());
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}");
    let _ = writeln!(log, "Starters : inchangés (pas encore pris en charge sur 3DS)\n");

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    // 1. Fiches des espèces : chaque entrée, plus la table complète en dernière position.
    if apply_personal(&mut ctx, settings, seed, &mut log) {
        let size = ctx.personal.get(1).map_or(0, Vec::len);
        let last = personal.len() - 1;
        let mut table = personal.file(last).map(<[u8]>::to_vec).unwrap_or_default();
        for s in 1..=l.species_count as usize {
            personal.set_file(s, ctx.personal[s].clone())?;
            if size > 0 && (s + 1) * size <= table.len() {
                table[s * size..(s + 1) * size].copy_from_slice(&ctx.personal[s]);
            }
        }
        personal.set_file(last, table)?;
        files.push((l.personal.to_string(), personal.to_bytes()));
    }

    // 2. Pokémon sauvages : fichiers de zone compressés en LZ11 + copie concaténée « EN ».
    let mut wild_slots = 0;
    if settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100 {
        let mut garc = game.garc(l.encounters)?;
        let raw = entries(&garc);
        let decompressed: Vec<Option<Vec<u8>>> = raw.iter().map(|d| if lz::is_lz11(d) { lz::decompress(d).ok() } else { None }).collect();
        let zone_ids: Vec<usize> = (0..raw.len()).filter(|&i| decompressed[i].as_deref().is_some_and(|d| d.starts_with(b"ZO"))).collect();
        let mut zones: Vec<Vec<u8>> = zone_ids.iter().map(|&i| decompressed[i].clone().unwrap()).collect();
        let before: Vec<Vec<u8>> = zones.clone();
        wild_slots = randomize_wild(&ctx, settings, seed, &mut zones, &mut log);

        // Section modifiée de chaque zone, pour mettre à jour la copie concaténée.
        let mut changes: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for (k, &i) in zone_ids.iter().enumerate() {
            if zones[k] != before[k] {
                if let Some(range) = encounters::oras_section(&before[k]) {
                    changes.push((before[k][range.clone()].to_vec(), zones[k][range].to_vec()));
                }
                garc.set_file(i, lz::compress_lz11(&zones[k]))?;
            }
        }
        // La copie concaténée (« EN ») n'est pas forcément compressée.
        for (i, data) in decompressed.iter().enumerate() {
            let compressed = data.is_some();
            let data = data.as_ref().unwrap_or(&raw[i]);
            if data.starts_with(b"ZO") || zone_ids.contains(&i) {
                continue;
            }
            let mut patched = data.clone();
            let mut hits = 0;
            for (old, new) in &changes {
                if let Some(pos) = patched.windows(old.len()).position(|w| w == old.as_slice()) {
                    patched[pos..pos + new.len()].copy_from_slice(new);
                    hits += 1;
                }
            }
            if hits > 0 {
                garc.set_file(i, if compressed { lz::compress_lz11(&patched) } else { patched })?;
                let _ = writeln!(log, "(copie concaténée n°{i} : {hits} zones mises à jour)\n");
            }
        }
        files.push((l.encounters.to_string(), garc.to_bytes()));
    }

    // 3. Dresseurs.
    let mut trainer_pokemon = 0;
    if settings.trainers != TrainerMode::Unchanged || settings.trainer_level_percent != 100 {
        let mut trdata_garc = game.garc(l.trainer_data)?;
        let mut trpoke_garc = game.garc(l.trainer_pokemon)?;
        let mut trdata = entries(&trdata_garc);
        let mut trpoke = entries(&trpoke_garc);
        let moves = game.text_file(l.move_names)?;
        trainer_pokemon = randomize_trainers(&ctx, settings, seed, &mut trdata, &mut trpoke, &moves, &mut log);
        for (i, (d, p)) in trdata.into_iter().zip(trpoke).enumerate() {
            trdata_garc.set_file(i, d)?;
            trpoke_garc.set_file(i, p)?;
        }
        files.push((l.trainer_data.to_string(), trdata_garc.to_bytes()));
        files.push((l.trainer_pokemon.to_string(), trpoke_garc.to_bytes()));
    }

    let romfs = write(out_dir, game.title_id(), target, &files)?;
    let outcome = Outcome { seed, share_code: code, starters: Vec::new(), wild_slots, trainer_pokemon, log };
    Ok((outcome, romfs))
}

/// Écrit les fichiers sous `<out>/luma/titles/<TID>/romfs` ou `<out>/<TID>/romfs`.
fn write(out_dir: &Path, title_id: u64, target: LayeredFsTarget, files: &[(String, Vec<u8>)]) -> Result<PathBuf, RomError> {
    let tid = format!("{title_id:016X}");
    let romfs = match target {
        LayeredFsTarget::Luma => out_dir.join("luma").join("titles").join(&tid).join("romfs"),
        LayeredFsTarget::Emulator => out_dir.join(&tid).join("romfs"),
    };
    for (path, data) in files {
        let dest = romfs.join(path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(kaleido_formats::FormatError::from)?;
        }
        std::fs::write(&dest, data).map_err(kaleido_formats::FormatError::from)?;
    }
    Ok(romfs)
}
