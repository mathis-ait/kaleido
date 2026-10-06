//! Randomizer 3DS : parties propres à Pokémon X / Y (le reste est commun avec Rubis
//! Oméga / Saphir Alpha, dans `ctr.rs`). Emplacements tirés de l'Universal Pokémon
//! Randomizer (Gen6RomHandler, gen6_offsets.ini, Gen6Constants) et vérifiés octet par
//! octet sur Pokémon Y (Europe) ; X partage les mêmes valeurs d'après UPR.
//!
//! - **Starters** : `DllField.cro`, table des dons à 0xF805C, 0x18 octets par entrée
//!   (`u16` espèce @0, `u8` forme @4, `u8` niveau @5) : Marisson, Feunnec, Grenousse
//!   (niv. 5), puis Bulbizarre, Salamèche, Carapuce (niv. 10, dons du Professeur Platane,
//!   option `kanto_starters` ; UPR : `StarterIndices=[0,1,2,3,4,5]`). Écran de choix : `DllPoke3Select.cro`, table à (`u16` @0xB8) + 0x10,
//!   pas de 0x54 (`u16` espèce @0, `u8` forme @2) ; textes « Pokémon de type … » : fichier 63.
//! - **Rencontres** : fichiers de zone `ZO` (LZ11), voir `data::encounters::xy_section` ;
//!   pas de copie concaténée comme en ROSA (la dernière entrée de l'archive est la table
//!   des zones, non compressée).
//! - **Rencontres fixées dans `DllField.cro`** : Pokémon qui tombent du plafond / des
//!   arbres (55 entrées à 0xF4270) et buissons qui bougent (7 entrées à 0xF40CC), 0x3C
//!   octets par entrée : `u32` @0, puis 7 × (`u16` espèce, `u16`, `u8` niveau, `u8`
//!   probabilité en %, `u16`). Les probabilités de chaque entrée totalisent 100 : on le
//!   vérifie avant d'écrire (sinon ces entrées sont laissées telles quelles).
//! - **Dresseurs** : fiche de 0x14 octets, voir `data::trainers::xy_trdata_as_oras`.

use std::fmt::Write as _;

use kaleido_formats::lz;

use super::{Ctx, Settings};
use crate::ctr_rom::CtrGameRom;
use crate::data::{encounters, trainers};
use crate::games::Game;
use crate::rom::RomError;

/// Plus grand identifiant de talent de X / Y (Aura Inversée).
pub(super) const MAX_ABILITY: u16 = 188;

pub(super) const ORIGINAL_STARTERS: [u16; 3] = [650, 653, 656];
/// Bulbizarre, Salamèche, Carapuce : dons n° 3 à 5, affichés en 4e à 6e position de l'écran de choix.
pub(super) const KANTO_STARTERS: [u16; 3] = [1, 4, 7];
const FIELD_CRO: &str = "DllField.cro";
const GIFT_TABLE: usize = 0xF805C;
const GIFT_SIZE: usize = 0x18;
const DISPLAY_CRO: &str = "DllPoke3Select.cro";
/// u16 donnant la position (moins 0x10) de la table d'affichage dans DllPoke3Select.cro.
const DISPLAY_POINTER: usize = 0xB8;
const DISPLAY_EXTRA: usize = 0x10;
const DISPLAY_SIZE: usize = 0x54;
/// Textes « Pokémon de type … » de l'écran de choix (lignes 1 à 3).
const STARTER_TEXT_FILE: usize = 63;

/// Rencontres fixées dans `DllField.cro` : (position, nombre d'entrées).
const FIELD_ENCOUNTERS: [(usize, usize); 2] = [(0xF4270, 55), (0xF40CC, 7)];
const FIELD_ENTRY_SIZE: usize = 0x3C;
const FIELD_SLOTS: usize = 7;

pub fn is_xy(game: Game) -> bool {
    matches!(game, Game::X | Game::Y)
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

/// Contenu actuel d'un fichier : version déjà modifiée dans `files`, sinon celle du RomFS.
fn current(game: &CtrGameRom, files: &[(String, Vec<u8>)], path: &str) -> Result<Vec<u8>, RomError> {
    match files.iter().find(|(p, _)| p == path) {
        Some((_, data)) => Ok(data.clone()),
        None => Ok(game.romfs().read(path)?),
    }
}

/// Ajoute un fichier modifié, ou remplace sa version précédente.
fn put(files: &mut Vec<(String, Vec<u8>)>, path: &str, data: Vec<u8>) {
    match files.iter_mut().find(|(p, _)| p == path) {
        Some((_, d)) => *d = data,
        None => files.push((path.to_string(), data)),
    }
}

/// Écrit les starters dans les deux modules `.cro` et le texte de l'écran de choix.
pub(super) fn write_starters(game: &CtrGameRom, ctx: &Ctx, starters: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
    write_trio(game, starters, 0, ORIGINAL_STARTERS, files)?;
    write_starter_text(game, ctx, starters, files)
}

/// Écrit les Pokémon de Kanto du Professeur Platane (dons n° 3 à 5, même écran de choix).
pub(super) fn write_kanto(game: &CtrGameRom, starters: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
    write_trio(game, starters, 3, KANTO_STARTERS, files)
}

/// Remplace trois dons consécutifs (à partir de `first`) dans `DllField.cro` et l'écran de choix,
/// après avoir vérifié qu'on y trouve bien les espèces d'origine.
fn write_trio(game: &CtrGameRom, starters: [u16; 3], first: usize, originals: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
    let bad = || RomError::Layout("emplacement des starters inattendu (révision du jeu différente ?)".into());
    let mut gift = current(game, files, FIELD_CRO)?;
    let mut display = current(game, files, DISPLAY_CRO)?;
    let display_table = u16_at(&display, DISPLAY_POINTER).ok_or_else(bad)? as usize + DISPLAY_EXTRA;

    for (k, &s) in starters.iter().enumerate() {
        let i = first + k;
        let g = GIFT_TABLE + i * GIFT_SIZE;
        let d = display_table + i * DISPLAY_SIZE;
        if u16_at(&gift, g) != Some(originals[k]) || u16_at(&display, d) != Some(originals[k]) {
            return Err(bad());
        }
        gift[g..g + 2].copy_from_slice(&s.to_le_bytes());
        gift[g + 4] = 0; // forme
        display[d..d + 2].copy_from_slice(&s.to_le_bytes());
        display[d + 2] = 0;
    }
    put(files, FIELD_CRO, gift);
    put(files, DISPLAY_CRO, display);
    Ok(())
}

fn write_starter_text(game: &CtrGameRom, ctx: &Ctx, starters: [u16; 3], files: &mut Vec<(String, Vec<u8>)>) -> Result<(), RomError> {
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
        put(files, l.text, garc.to_bytes());
    }
    Ok(())
}

/// Une entrée de rencontre fixée de `DllField.cro` est-elle cohérente ?
/// (espèces valides, probabilités des emplacements occupés totalisant 100 %).
fn field_entry_ok(cro: &[u8], at: usize, count: u16) -> bool {
    let Some(entry) = cro.get(at..at + FIELD_ENTRY_SIZE) else { return false };
    let mut total = 0u32;
    for i in 0..FIELD_SLOTS {
        let species = u16::from_le_bytes([entry[4 + i * 8], entry[5 + i * 8]]);
        if species > count {
            return false;
        }
        if species != 0 {
            total += entry[9 + i * 8] as u32;
        }
    }
    total == 100
}

/// Zone factice au format X / Y contenant les 7 emplacements d'une entrée de
/// `DllField.cro` (niveau minimum = maximum), pour passer par le code commun.
fn field_pseudo_zone(cro: &[u8], at: usize) -> Vec<u8> {
    let len = encounters::XY_RATES + encounters::XY_SLOTS * 4;
    let mut z = vec![0u8; 0x18 + len];
    z[..2].copy_from_slice(b"ZO");
    z[0x10..0x14].copy_from_slice(&0x18u32.to_le_bytes());
    z[0x14..0x18].copy_from_slice(&((0x18 + len) as u32).to_le_bytes());
    for i in 0..FIELD_SLOTS {
        let src = at + 4 + i * 8;
        let dst = 0x18 + encounters::XY_RATES + i * 4;
        z[dst..dst + 2].copy_from_slice(&cro[src..src + 2]);
        z[dst + 2] = cro[src + 4];
        z[dst + 3] = cro[src + 4];
    }
    z
}

/// Recopie espèces et niveaux d'une zone factice dans l'entrée de `DllField.cro`.
fn field_write_back(cro: &mut [u8], at: usize, zone: &[u8]) {
    for i in 0..FIELD_SLOTS {
        let src = 0x18 + encounters::XY_RATES + i * 4;
        let dst = at + 4 + i * 8;
        if u16::from_le_bytes([cro[dst], cro[dst + 1]]) == 0 {
            continue;
        }
        cro[dst..dst + 2].copy_from_slice(&zone[src..src + 2]);
        cro[dst + 4] = zone[src + 2];
    }
}

/// Pokémon sauvages : zones de l'archive des rencontres (LZ11) et rencontres fixées
/// de `DllField.cro`. Renvoie le nombre d'emplacements modifiés.
pub(super) fn randomize_wild(
    game: &CtrGameRom,
    ctx: &Ctx,
    settings: &Settings,
    seed: u64,
    files: &mut Vec<(String, Vec<u8>)>,
    log: &mut String,
) -> Result<usize, RomError> {
    let l = game.layout;
    let mut garc = game.garc(l.encounters)?;
    let mut zone_ids = Vec::new();
    let mut zones = Vec::new();
    for i in 0..garc.len() {
        let Some(raw) = garc.file(i) else { continue };
        if !lz::is_lz11(raw) {
            continue;
        }
        if let Ok(d) = lz::decompress(raw) {
            if encounters::xy_section(&d).is_some() {
                zone_ids.push(i);
                zones.push(d);
            }
        }
    }
    let garc_zones = zones.len();

    // Rencontres fixées : seulement si toutes les entrées ont la forme attendue.
    let mut cro = current(game, files, FIELD_CRO)?;
    let field: Vec<usize> = FIELD_ENCOUNTERS.iter().flat_map(|&(start, n)| (0..n).map(move |k| start + k * FIELD_ENTRY_SIZE)).collect();
    let field_ok = field.iter().all(|&at| field_entry_ok(&cro, at, l.species_count));
    if field_ok {
        zones.extend(field.iter().map(|&at| field_pseudo_zone(&cro, at)));
    }

    let before = zones.clone();
    let total = super::randomize_wild(ctx, settings, seed, &mut zones, log);
    if garc_zones > 0 {
        let _ = writeln!(log, "(zones 0 à {} : archive des rencontres ; suivantes : DllField.cro)\n", garc_zones - 1);
    }
    if !field_ok {
        let _ = writeln!(log, "(rencontres fixées de {FIELD_CRO} non reconnues : laissées telles quelles)\n");
    }

    for (k, &i) in zone_ids.iter().enumerate() {
        if zones[k] != before[k] {
            garc.set_file(i, lz::compress_lz11(&zones[k]))?;
        }
    }
    put(files, l.encounters, garc.to_bytes());
    if field_ok {
        let mut changed = false;
        for (k, &at) in field.iter().enumerate() {
            let z = &zones[garc_zones + k];
            if *z != before[garc_zones + k] {
                field_write_back(&mut cro, at, z);
                changed = true;
            }
        }
        if changed {
            put(files, FIELD_CRO, cro);
        }
    }
    Ok(total)
}

/// Dresseurs : fiches X / Y converties au format ROSA pour le code commun, puis
/// drapeaux et nombre de Pokémon recopiés. Renvoie le nombre de Pokémon modifiés.
pub(super) fn randomize_trainers(
    game: &CtrGameRom,
    ctx: &Ctx,
    settings: &Settings,
    seed: u64,
    files: &mut Vec<(String, Vec<u8>)>,
    log: &mut String,
) -> Result<usize, RomError> {
    let l = game.layout;
    let mut trdata_garc = game.garc(l.trainer_data)?;
    let mut trpoke_garc = game.garc(l.trainer_pokemon)?;
    let mut trdata: Vec<Vec<u8>> = super::ctr::entries(&trdata_garc);
    let mut trpoke: Vec<Vec<u8>> = super::ctr::entries(&trpoke_garc);
    let mut oras: Vec<Vec<u8>> = trdata.iter().map(|d| trainers::xy_trdata_as_oras(d)).collect();
    let moves = game.text_file(l.move_names)?;
    let total = super::randomize_trainers(ctx, settings, seed, &mut oras, &mut trpoke, &moves, log);
    for (d, o) in trdata.iter_mut().zip(&oras) {
        trainers::xy_trdata_update(d, o);
    }
    for (i, (d, p)) in trdata.into_iter().zip(trpoke).enumerate() {
        trdata_garc.set_file(i, d)?;
        trpoke_garc.set_file(i, p)?;
    }
    put(files, l.trainer_data, trdata_garc.to_bytes());
    put(files, l.trainer_pokemon, trpoke_garc.to_bytes());
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_entry_roundtrip() {
        // Buisson qui bouge de la Route 6 (Pokémon Y, DllField.cro @0xF40CC).
        let mut cro = vec![0u8; FIELD_ENTRY_SIZE];
        let slots: [(u16, u8, u8); 6] = [(543, 10, 40), (543, 11, 20), (543, 12, 20), (531, 10, 10), (531, 11, 5), (531, 12, 5)];
        for (i, &(s, lv, rate)) in slots.iter().enumerate() {
            let at = 4 + i * 8;
            cro[at..at + 2].copy_from_slice(&s.to_le_bytes());
            cro[at + 4] = lv;
            cro[at + 5] = rate;
        }
        assert!(field_entry_ok(&cro, 0, 721));
        let mut zone = field_pseudo_zone(&cro, 0);
        let read = encounters::read(6, &zone);
        assert_eq!(read.len(), 6);
        assert_eq!((read[3].species, read[3].min_level, read[3].max_level), (531, 10, 10));
        encounters::set_species(&mut zone, &read[3], 25);
        encounters::scale_levels(&mut zone, &read[3], 2.0);
        field_write_back(&mut cro, 0, &zone);
        assert_eq!((u16::from_le_bytes([cro[28], cro[29]]), cro[32], cro[33]), (25, 20, 10));
        // Probabilités incohérentes : refusé.
        cro[9] = 41;
        assert!(!field_entry_ok(&cro, 0, 721));
    }
}
