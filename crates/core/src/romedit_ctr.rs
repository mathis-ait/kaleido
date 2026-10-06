//! Éditeur de ROM, jeux 3DS (X / Y, Rubis Oméga / Saphir Alpha, Soleil / Lune,
//! Ultra-Soleil / Ultra-Lune) : mêmes données que sur DS ([`crate::romedit`]) —
//! statistiques, types, talents (talent caché compris), taux de capture et attaques
//! apprises par niveau.
//!
//! Comme le randomizer 3DS ([`crate::randomizer::ctr`]), rien n'est écrit dans la ROM
//! d'origine : la sauvegarde produit un dossier LayeredFS (seules les archives modifiées)
//! et/ou une ROM `.3ds` complète reconstruite.
//!
//! - **Fiches** (`personal`) : 0x40 octets (Gen 6) ou 0x54 (Gen 7), mêmes champs que la
//!   Gen 5 pour ce qui est modifié ; une entrée par espèce puis par forme alternative, la
//!   dernière contient la table complète, mise à jour elle aussi.
//! - **Attaques apprises** (`levelup`) : couples `u16 attaque, u16 niveau`, fin `FFFF FFFF`.
//!   En Gen 7, le niveau 0 désigne une attaque apprise à l'évolution.
//!
//! Seules les espèces (n° national) sont modifiables, comme sur DS : les entrées des
//! formes alternatives restent telles quelles.

use std::path::{Path, PathBuf};

use crate::ctr_rom::CtrGameRom;
use crate::data::learnsets::Learnset;
use crate::randomizer::ctr::{self as rctr, CtrOutput, CtrWritten, LayeredFsTarget};
use crate::rom::RomError;
use crate::romedit::{ability_slots, choices, encode_learnset, read_species, type_choices, EditorData, SpeciesData, MAX_LEVEL};

/// Fichiers d'une archive (une entrée par espèce puis par forme).
type Records = Vec<Vec<u8>>;
/// Fichiers modifiés du RomFS : (chemin, contenu).
pub type RomFsFiles = Vec<(String, Vec<u8>)>;

pub fn supports(game: &CtrGameRom) -> bool {
    rctr::supports(game.game) && game.layout.verified
}

/// Niveau minimal d'une attaque apprise : 0 (à l'évolution) à partir de la Gen 7.
pub fn min_level(generation: u8) -> u8 {
    if generation >= 7 {
        0
    } else {
        1
    }
}

fn check_supported(game: &CtrGameRom) -> Result<(), RomError> {
    if supports(game) {
        Ok(())
    } else {
        Err(RomError::Unsupported(format!("l'édition de {} n'est pas encore prise en charge", game.game.name_fr())))
    }
}

/// Fiches et attaques apprises de toutes les entrées (espèces puis formes).
fn load(game: &CtrGameRom) -> Result<(Records, Records), RomError> {
    let l = game.layout;
    let personal = rctr::entries(&game.garc(l.personal)?);
    let learnsets = rctr::entries(&game.garc(l.levelup)?);
    // La dernière fiche est la table complète : il faut au moins une entrée par espèce en plus.
    if personal.len() <= l.species_count as usize + 1 || learnsets.len() <= l.species_count as usize {
        return Err(RomError::Layout("fiches ou attaques apprises incomplètes".into()));
    }
    Ok((personal, learnsets))
}

/// Lit tout ce que l'éditeur peut modifier.
pub fn read(game: &CtrGameRom) -> Result<EditorData, RomError> {
    check_supported(game)?;
    let gen = game.generation();
    let l = game.layout;
    let (personal, learnsets) = load(game)?;
    let move_names = game.text_file(l.move_names)?;
    let ability_names = game.text_file(l.ability_names)?;
    let species =
        (1..=l.species_count as usize).map(|id| read_species(gen, id as u16, &personal[id], &learnsets[id])).collect::<Result<Vec<_>, _>>()?;
    let mut moves = choices(&move_names, move_names.len().saturating_sub(1));
    if gen >= 7 {
        moves.retain(|m| !rctr::is_z_move(m.id));
    }
    Ok(EditorData {
        hidden_ability: true,
        max_learnset: species.iter().map(|s| s.learnset.len()).max().unwrap_or(0),
        types: type_choices(gen),
        abilities: choices(&ability_names, rctr::max_ability(game.game) as usize),
        moves,
        species,
    })
}

/// Vérifie les espèces modifiées et renvoie les archives à remplacer dans le RomFS
/// (chemin, contenu), avec le nombre d'espèces écrites. Rien n'est renvoyé si aucune
/// espèce ne diffère de l'original.
pub fn edited_files(game: &CtrGameRom, edits: &[SpeciesData]) -> Result<(RomFsFiles, usize), RomError> {
    let original = read(game)?;
    let gen = game.generation();
    let l = game.layout;
    let count = l.species_count;
    let max_ability = rctr::max_ability(game.game);
    let types = original.types.len() as u8;
    let min_level = min_level(gen);

    let mut personal = game.garc(l.personal)?;
    let mut learnset_garc = game.garc(l.levelup)?;
    let last = personal.len() - 1;
    let mut table = personal.file(last).map(<[u8]>::to_vec).unwrap_or_default();
    let mut written = 0;
    for e in edits {
        let bad = |what: &str| RomError::Unsupported(format!("Pokémon n°{} : {what}", e.id));
        if e.id == 0 || e.id > count {
            return Err(RomError::Unsupported(format!("Pokémon n°{} inexistant dans ce jeu", e.id)));
        }
        if original.species[e.id as usize - 1] == *e {
            continue;
        }
        if e.stats.contains(&0) {
            return Err(bad("une statistique de base ne peut pas valoir 0"));
        }
        if e.types.iter().any(|&t| t >= types) {
            return Err(bad("type inconnu"));
        }
        if e.abilities.iter().any(|&a| a > max_ability) || e.abilities[0] == 0 {
            return Err(bad("talent invalide (le premier est obligatoire)"));
        }
        if e.learnset.len() > original.max_learnset {
            return Err(bad(&format!("{} attaques apprises au maximum", original.max_learnset)));
        }
        if e.learnset.iter().any(|m| !original.moves.iter().any(|c| c.id == m.move_id) || m.level < min_level || m.level > MAX_LEVEL) {
            return Err(bad("attaque ou niveau invalide"));
        }

        let id = e.id as usize;
        let mut d = personal.file(id).map(<[u8]>::to_vec).ok_or_else(|| bad("fiche absente"))?;
        let [hp, atk, def, spa, spd, spe] = e.stats;
        d[..6].copy_from_slice(&[hp, atk, def, spe, spa, spd]);
        d[6..8].copy_from_slice(&e.types);
        d[8] = e.catch_rate;
        for (at, &a) in ability_slots(gen).zip(&e.abilities) {
            d[at] = a as u8;
        }
        // Même fiche dans la table complète (dernière entrée).
        let size = d.len();
        if let Some(row) = table.get_mut(id * size..(id + 1) * size) {
            row.copy_from_slice(&d);
        }
        personal.set_file(id, d)?;

        let mut moves: Learnset = e.learnset.iter().map(|m| (m.move_id, m.level)).collect();
        moves.sort_by_key(|&(_, lvl)| lvl);
        learnset_garc.set_file(id, encode_learnset(gen, &moves))?;
        written += 1;
    }
    if written == 0 {
        return Ok((Vec::new(), 0));
    }
    personal.set_file(last, table)?;
    Ok((vec![(l.personal.to_string(), personal.to_bytes()), (l.levelup.to_string(), learnset_garc.to_bytes())], written))
}

/// `<out_dir>/<nom de la ROM> - modifiée.3ds` (`.cxi` si l'entrée n'est pas une CCI).
pub fn image_output_path(input: &Path, out_dir: &Path) -> Result<PathBuf, RomError> {
    let mut r = std::io::BufReader::new(std::fs::File::open(input).map_err(kaleido_formats::FormatError::from)?);
    let img = kaleido_formats::ctr::CtrImage::probe(&mut r)?.ok_or_else(|| RomError::Unsupported("ce n'est pas une ROM 3DS".into()))?;
    let stem = input.file_stem().map_or_else(|| "ROM".into(), |s| s.to_string_lossy().into_owned());
    let ext = kaleido_formats::ctr_build::output_extension(img.container);
    Ok(out_dir.join(format!("{stem} - modifiée.{ext}")))
}

/// Écrit les espèces modifiées sous `out_dir` : dossier LayeredFS et/ou ROM reconstruite
/// selon `output`. Renvoie le nombre d'espèces écrites et les chemins produits.
pub fn save(
    game: &CtrGameRom,
    edits: &[SpeciesData],
    out_dir: &Path,
    output: CtrOutput,
    target: LayeredFsTarget,
) -> Result<(usize, CtrWritten), RomError> {
    // Vérifié avant tout calcul : une ROM complète ne se reconstruit qu'à partir d'une image.
    let image = if output.image() {
        let input = game
            .romfs()
            .image_path()
            .ok_or_else(|| RomError::Unsupported("la sortie .3ds demande une ROM (.3ds, .cxi ou .cia déchiffré), pas un dossier extrait".into()))?;
        Some((input.to_path_buf(), image_output_path(input, out_dir)?))
    } else {
        None
    };
    let (files, count) = edited_files(game, edits)?;
    if count == 0 {
        return Err(RomError::Unsupported("aucune modification à enregistrer".into()));
    }
    let mut written = CtrWritten { files: files.iter().map(|(p, _)| p.clone()).collect(), ..Default::default() };
    if output.layeredfs() {
        written.romfs = Some(rctr::write(out_dir, game.title_id(), target, &files)?);
    }
    if let Some((input, dest)) = image {
        rctr::write_image(&input, &dest, &files)?;
        written.image = Some(dest);
    }
    Ok((count, written))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::learnsets;

    const ROMS: [&str; 4] = [
        "Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds",
        "Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2).3ds",
        "Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds",
        "Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds",
    ];

    #[test]
    fn levels() {
        assert_eq!(min_level(6), 1);
        assert_eq!(min_level(7), 0);
    }

    /// Fichiers d'origine : le réencodage des attaques apprises est identique octet pour
    /// octet, la table complète reprend chaque fiche, le niveau 0 n'existe qu'en Gen 7.
    #[test]
    fn real_roms_formats() {
        for name in ROMS {
            let p = crate::test_rom_path(name);
            if !p.exists() {
                eprintln!("ROM absente, test ignoré : {name}");
                continue;
            }
            let game = CtrGameRom::open(&p).unwrap();
            let gen = game.generation();
            let (personal, learn) = load(&game).unwrap();
            let table = personal.last().unwrap();
            let mut level0 = 0;
            for id in 1..=game.layout.species_count as usize {
                let moves = learnsets::read(gen, &learn[id]);
                assert_eq!(encode_learnset(gen, &moves), learn[id], "{name} n°{id}");
                level0 += moves.iter().filter(|m| m.1 == 0).count();
                let size = personal[id].len();
                assert_eq!(table[id * size..(id + 1) * size], personal[id], "{name} n°{id}");
            }
            assert_eq!(level0 > 0, gen >= 7, "{name}");
        }
    }
}
