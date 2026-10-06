//! Randomizer 3DS (Rubis Oméga / Saphir Alpha, X / Y) : mêmes réglages que sur DS. Le
//! résultat est un dossier LayeredFS (seuls les fichiers modifiés sont écrits),
//! à utiliser avec Luma3DS sur console ou dans le dossier « mods » d'un émulateur,
//! et/ou une ROM `.3ds` déchiffrée complète, reconstruite avec ces fichiers
//! (`kaleido_formats::ctr_build`), à ouvrir directement dans un émulateur.
//!
//! Starters : comme l'Universal Pokémon Randomizer (Gen6RomHandler.setStarters), on
//! modifie la table des dons de `DllField.cro` et l'écran de choix de
//! `DllPoke3Select.cro`, sans toucher à `static.crr` : Luma3DS (patch des jeux activé)
//! et les émulateurs ne vérifient pas ces signatures.
//!
//! X / Y : starters, rencontres et dresseurs ont leurs propres formats (`ctr_xy.rs`).
//! Soleil / Lune et Ultra-Soleil / Ultra-Lune ont leurs propres formats : voir `ctr_gen7`
//! (même sortie, mêmes réglages ; [`randomize`] et [`preview_starters`] y renvoient).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kaleido_formats::garc::Garc;
use kaleido_formats::lz;
use serde::{Deserialize, Serialize};

use super::{apply_personal, ctr_xy, randomize_trainers, randomize_wild, share_code, Ctx, Outcome, Settings, Sources, WildMode};
use crate::ctr_rom::CtrGameRom;
use crate::data::encounters;
use crate::games::Game;
use crate::rom::RomError;

/// Plus grand identifiant de talent de Rubis Oméga / Saphir Alpha.
const ORAS_MAX_ABILITY: u16 = 191;

/// Starters de Rubis Oméga / Saphir Alpha (valeurs de l'Universal Pokémon Randomizer).
const GIFT_CRO: &str = "DllField.cro";
const GIFT_TABLE: usize = 0xF906C;
const GIFT_SIZE: usize = 0x24;
const DISPLAY_CRO: &str = "DllPoke3Select.cro";
/// u16 donnant la position de la table d'affichage dans DllPoke3Select.cro.
const DISPLAY_POINTER: usize = 0xB8;
const DISPLAY_SIZE: usize = 0x54;
const ORIGINAL_STARTERS: [u16; 3] = [252, 255, 258];
/// Textes « Pokémon de type … » de l'écran de choix (lignes 1 à 3).
const STARTER_TEXT_FILE: usize = 77;

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

/// Aperçu des starters, sans rien écrire.
pub fn preview_starters(game: &CtrGameRom, settings: &Settings, seed: u64) -> Result<Vec<super::PokemonRef>, RomError> {
    if !supports(game.game) {
        return Err(unsupported());
    }
    if super::ctr_gen7::supports(game.game) {
        return super::ctr_gen7::preview_starters(game, settings, seed);
    }
    let (mut ctx, _) = load(game, settings)?;
    apply_personal(&mut ctx, settings, seed, &mut String::new());
    let chosen = super::choose_starters(&ctx, settings, seed, original_starters(game.game));
    Ok(chosen.iter().map(|&id| super::PokemonRef { id, name: ctx.name(id).to_string() }).collect())
}

/// Écrit les starters dans les deux modules `.cro` et le texte de l'écran de choix.
fn write_starters(game: &CtrGameRom, ctx: &Ctx, starters: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
    let bad = || RomError::Layout("emplacement des starters inattendu (révision du jeu différente ?)".into());
    let mut gift = game.romfs().read(GIFT_CRO)?;
    let mut display = game.romfs().read(DISPLAY_CRO)?;
    let display_table = u16_at(&display, DISPLAY_POINTER).ok_or_else(bad)? as usize;

    for (i, &s) in starters.iter().enumerate() {
        let g = GIFT_TABLE + i * GIFT_SIZE;
        let d = display_table + i * DISPLAY_SIZE;
        if u16_at(&gift, g) != Some(ORIGINAL_STARTERS[i]) || u16_at(&display, d) != Some(ORIGINAL_STARTERS[i]) {
            return Err(bad());
        }
        gift[g..g + 2].copy_from_slice(&s.to_le_bytes());
        gift[g + 4] = 0; // forme
        display[d..d + 2].copy_from_slice(&s.to_le_bytes());
        display[d + 2] = 0;
    }
    files.push((GIFT_CRO.to_string(), gift));
    files.push((DISPLAY_CRO.to_string(), display));

    // Texte en français : « Pokémon de type {type} » puis le nom (variable du jeu).
    let l = game.layout;
    let mut garc = game.garc(l.text)?;
    if let Some(data) = garc.file(STARTER_TEXT_FILE).map(<[u8]>::to_vec) {
        use crate::text::gen5::{MsgFile, Variant};
        let mut msg = MsgFile::parse_with(&data, Variant::Gen6)?;
        let mut lines = msg.strings();
        for (i, &s) in starters.iter().enumerate() {
            let type_name = ctx.types(s).first().map_or("Normal", |t| t.name_fr());
            if let Some(line) = lines.get_mut(i + 1) {
                *line = format!("Pokémon de type {type_name}\n{{VAR:0101,0000}}");
            }
        }
        msg.set_strings(&lines)?;
        garc.set_file(STARTER_TEXT_FILE, msg.to_bytes())?;
        files.push((l.text.to_string(), garc.to_bytes()));
    }
    Ok(())
}

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

/// Ce que produit le randomizer 3DS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CtrOutput {
    /// Dossier LayeredFS : seuls les fichiers modifiés (léger, la ROM reste intacte).
    #[default]
    LayeredFs,
    /// ROM complète déchiffrée (`.3ds`, ou `.cxi` pour une entrée `.cxi` / `.cia`).
    Rom3ds,
    /// Les deux.
    Both,
}

impl CtrOutput {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn layeredfs(self) -> bool {
        self != Self::Rom3ds
    }

    pub fn image(self) -> bool {
        self != Self::LayeredFs
    }
}

/// Fichiers produits par [`randomize`].
#[derive(Debug, Clone, Default)]
pub struct CtrWritten {
    /// Dossier `romfs` du LayeredFS.
    pub romfs: Option<PathBuf>,
    /// ROM reconstruite.
    pub image: Option<PathBuf>,
    /// Chemins (dans le RomFS) des fichiers modifiés.
    pub files: Vec<String>,
}

pub fn supports(game: Game) -> bool {
    matches!(game, Game::OmegaRuby | Game::AlphaSapphire) || ctr_xy::is_xy(game) || super::ctr_gen7::supports(game)
}

fn unsupported() -> RomError {
    RomError::Unsupported("le randomizer 3DS prend en charge X, Y, Rubis Oméga, Saphir Alpha, Soleil, Lune, Ultra-Soleil et Ultra-Lune".into())
}

/// Capacités Z (Gen 7) : jamais apprises par niveau.
pub(crate) use super::ctr_gen7::is_z_move;

/// Plus grand identifiant de talent du jeu (3DS).
pub(crate) fn max_ability(game: Game) -> u16 {
    if super::ctr_gen7::supports(game) {
        super::ctr_gen7::max_ability(game)
    } else if ctr_xy::is_xy(game) {
        ctr_xy::MAX_ABILITY
    } else {
        ORAS_MAX_ABILITY
    }
}

/// Starters d'origine du jeu.
fn original_starters(game: Game) -> [u16; 3] {
    if ctr_xy::is_xy(game) {
        ctr_xy::ORIGINAL_STARTERS
    } else {
        ORIGINAL_STARTERS
    }
}

/// Entrées d'une archive GARC (première sous-entrée de chacune).
pub(crate) fn entries(garc: &Garc) -> Vec<Vec<u8>> {
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
        max_ability: max_ability(game.game),
    };
    Ok((Ctx::new(src, settings)?, personal))
}

/// Randomise et écrit sous `out_dir` le dossier LayeredFS et/ou la ROM reconstruite
/// (`settings.ctr_output`). Renvoie le résultat et les chemins écrits.
pub fn randomize(
    game: &CtrGameRom,
    settings: &Settings,
    seed: u64,
    out_dir: &Path,
    target: LayeredFsTarget,
) -> Result<(Outcome, CtrWritten), RomError> {
    if !supports(game.game) {
        return Err(unsupported());
    }
    if super::ctr_gen7::supports(game.game) {
        return super::ctr_gen7::randomize(game, settings, seed, out_dir, target);
    }
    let output = settings.ctr_output;
    // Vérifié avant tout calcul : une ROM complète ne se reconstruit qu'à partir d'une image.
    let image_out = if output.image() {
        let input = game
            .romfs()
            .image_path()
            .ok_or_else(|| RomError::Unsupported("la sortie .3ds demande une ROM (.3ds, .cxi ou .cia déchiffré), pas un dossier extrait".into()))?;
        Some(image_output_path(input, out_dir, seed)?)
    } else {
        None
    };
    let l = game.layout;
    let (mut ctx, mut personal) = load(game, settings)?;
    let code = share_code(seed, settings);
    let mut log = String::new();
    let _ = writeln!(log, "Kaleido — journal de randomisation (3DS)");
    let _ = writeln!(log, "Jeu : {} ({:016X})", game.game.name_fr(), game.title_id());
    let _ = writeln!(log, "Seed : {seed}");
    let _ = writeln!(log, "Code de partage : {code}\n");

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

    // 2. Starters.
    let xy = ctr_xy::is_xy(game.game);
    let originals = original_starters(game.game);
    let starters = super::choose_starters(&ctx, settings, seed, originals);
    if starters != originals {
        if xy {
            ctr_xy::write_starters(game, &ctx, starters, &mut files)?;
        } else {
            write_starters(game, &ctx, starters, &mut files)?;
        }
        let _ = writeln!(log, "== Starters ==");
        for (old, new) in originals.iter().zip(starters) {
            let _ = writeln!(log, "{} → {}", ctx.name(*old), ctx.name(new));
        }
        let _ = writeln!(log);
    }
    // X / Y : Bulbizarre, Salamèche et Carapuce du Professeur Platane.
    if xy && settings.kanto_starters != super::StarterMode::Unchanged {
        let kanto = super::choose_second_trio(&ctx, settings, seed, ctr_xy::KANTO_STARTERS, &starters);
        if kanto != ctr_xy::KANTO_STARTERS {
            ctr_xy::write_kanto(game, kanto, &mut files)?;
            let _ = writeln!(log, "== Pokémon de Kanto (Professeur Platane) ==");
            for (old, new) in ctr_xy::KANTO_STARTERS.iter().zip(kanto) {
                let _ = writeln!(log, "{} → {}", ctx.name(*old), ctx.name(new));
            }
            let _ = writeln!(log);
        }
    }

    // Évolutions et attaques apprises (avant les dresseurs, qui s'en servent).
    if settings.easy_evolutions {
        let mut garc = game.garc(l.evolution)?;
        let mut evo_files = entries(&garc);
        super::extras::easy_evolutions(&mut ctx, &mut evo_files, &mut log);
        for (i, f) in evo_files.into_iter().enumerate() {
            garc.set_file(i, f)?;
        }
        files.push((l.evolution.to_string(), garc.to_bytes()));
    }
    if settings.random_movesets {
        let max_move = game.text_file(l.move_names)?.len().saturating_sub(1) as u16;
        let mut garc = game.garc(l.levelup)?;
        let mut learn_files = entries(&garc);
        super::extras::random_movesets(&mut ctx, &mut learn_files, max_move, seed, &mut log);
        for (i, f) in learn_files.into_iter().enumerate() {
            garc.set_file(i, f)?;
        }
        files.push((l.levelup.to_string(), garc.to_bytes()));
    }

    // 3. Pokémon sauvages : fichiers de zone compressés en LZ11 + copie concaténée « EN ».
    let mut wild_slots = 0;
    if xy && (settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100) {
        wild_slots = ctr_xy::randomize_wild(game, &ctx, settings, seed, &mut files, &mut log)?;
    }
    if !xy && (settings.wild != WildMode::Unchanged || settings.wild_level_percent != 100) {
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

    // 4. Dresseurs.
    let mut trainer_pokemon = 0;
    if xy && super::trainers_changed(settings) {
        trainer_pokemon = ctr_xy::randomize_trainers(game, &ctx, settings, seed, &mut files, &mut log)?;
    }
    if !xy && super::trainers_changed(settings) {
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

    let mut written = CtrWritten { files: files.iter().map(|(p, _)| p.clone()).collect(), ..Default::default() };
    if output.layeredfs() {
        written.romfs = Some(write(out_dir, game.title_id(), target, &files)?);
    }
    if let (Some(dest), Some(input)) = (image_out, game.romfs().image_path()) {
        let report = write_image(input, &dest, &files)?;
        let _ = writeln!(log, "== ROM complète ==");
        let _ = writeln!(log, "{} ({} octets, {} fichiers remplacés dans le RomFS)", dest.display(), report.size, report.replaced);
        written.image = Some(dest);
    }
    let starters = starters.iter().map(|&id| super::PokemonRef { id, name: ctx.name(id).to_string() }).collect();
    let outcome = Outcome { seed, share_code: code, starters, wild_slots, trainer_pokemon, log };
    Ok((outcome, written))
}

/// `<out_dir>/<nom de la ROM> - Kaleido <seed>.3ds` (`.cxi` si l'entrée n'est pas une CCI).
pub(super) fn image_output_path(input: &Path, out_dir: &Path, seed: u64) -> Result<PathBuf, RomError> {
    let mut r = std::io::BufReader::new(std::fs::File::open(input).map_err(kaleido_formats::FormatError::from)?);
    let img = kaleido_formats::ctr::CtrImage::probe(&mut r)?.ok_or_else(|| RomError::Unsupported("ce n'est pas une ROM 3DS".into()))?;
    let stem = input.file_stem().map_or_else(|| "ROM".into(), |s| s.to_string_lossy().into_owned());
    let ext = kaleido_formats::ctr_build::output_extension(img.container);
    Ok(out_dir.join(format!("{stem} - Kaleido {seed}.{ext}")))
}

/// Reconstruit la ROM avec les fichiers modifiés, puis relit chacun d'eux dans
/// l'image écrite. En cas d'échec, le fichier incomplet est supprimé.
pub(crate) fn write_image(input: &Path, dest: &Path, files: &[(String, Vec<u8>)]) -> Result<kaleido_formats::ctr_build::RebuildReport, RomError> {
    use kaleido_formats::romfs::RomFsSource;
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(p, d)| (p.as_str(), d.as_slice())).collect();
    let result = kaleido_formats::ctr_build::rebuild_image(input, dest, &refs).map_err(RomError::from).and_then(|report| {
        let rebuilt = RomFsSource::open(dest)?;
        for (path, data) in files {
            if rebuilt.read(path)? != *data {
                return Err(RomError::Layout(format!("relecture de {path} dans la ROM reconstruite : contenu différent")));
            }
        }
        Ok(report)
    });
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

/// Écrit les fichiers sous `<out>/luma/titles/<TID>/romfs` ou `<out>/<TID>/romfs`.
pub(crate) fn write(out_dir: &Path, title_id: u64, target: LayeredFsTarget, files: &[(String, Vec<u8>)]) -> Result<PathBuf, RomError> {
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
