//! Éditeur de ROM Gen 3 (GBA) : mêmes données et même interface que `romedit` (DS).
//!
//! Fiches de 28 octets (même début que la Gen 4 : statistiques, types, taux de capture,
//! talents en 0x16-0x17) et attaques apprises (même codage que la Gen 4). Une liste
//! d'attaques plus longue que l'originale est déplacée dans la zone libre de la ROM.
//! Non vérifié sur une vraie ROM (voir `gba_rom`).

use crate::data::gen3;
use crate::data::learnsets::{self, Learnset};
use crate::gba_rom::{GbaGameRom, SPECIES_COUNT};
use crate::romedit::{encode_learnset, read_species, type_choices, Choice, EditorData, SpeciesData, MAX_LEVEL};
use crate::rom::RomError;

/// Plus grand talent et plus grande attaque de la Gen 3.
const MAX_ABILITY: u16 = 77;
const MAX_MOVE: u16 = 354;
/// Les listes peuvent grandir (déplacées dans la zone libre) jusqu'à cette taille.
const MAX_LEARNSET: usize = 24;

pub fn supports(_game: &GbaGameRom) -> bool {
    true
}

fn names(list: &[&str], max: u16) -> Vec<Choice> {
    list.iter()
        .enumerate()
        .take(max as usize + 1)
        .skip(1)
        .filter(|(_, n)| !n.is_empty() && **n != "-")
        .map(|(id, n)| Choice { id: id as u16, name: n.to_string() })
        .collect()
}

/// Lit tout ce que l'éditeur peut modifier.
pub fn read(game: &GbaGameRom) -> Result<EditorData, RomError> {
    let mut species = Vec::with_capacity(SPECIES_COUNT as usize);
    for id in 1..=SPECIES_COUNT {
        let personal = game.personal(id).ok_or_else(|| RomError::Layout(format!("fiche n°{id}")))?;
        species.push(read_species(3, id, personal, &gen3::learnset(game, id)?)?);
    }
    Ok(EditorData {
        hidden_ability: false,
        max_learnset: species.iter().map(|s| s.learnset.len()).max().unwrap_or(0).max(MAX_LEARNSET),
        types: type_choices(3),
        abilities: names(crate::dex::ability_names(), MAX_ABILITY),
        moves: names(crate::dex::move_names(), MAX_MOVE),
        species,
    })
}

/// Écrit les espèces modifiées dans la ROM (en mémoire). Renvoie le nombre d'espèces écrites.
pub fn apply(game: &mut GbaGameRom, edits: &[SpeciesData]) -> Result<usize, RomError> {
    let original = read(game)?;
    let types = original.types.len() as u8;
    let mut written = 0;
    for e in edits {
        let bad = |what: &str| RomError::Unsupported(format!("Pokémon n°{} : {what}", e.id));
        if e.id == 0 || e.id > SPECIES_COUNT {
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
        if e.abilities[..2].iter().any(|&a| a > MAX_ABILITY) || e.abilities[0] == 0 {
            return Err(bad("talent invalide (le premier est obligatoire)"));
        }
        if e.learnset.len() > original.max_learnset {
            return Err(bad(&format!("{} attaques apprises au maximum", original.max_learnset)));
        }
        if e.learnset.iter().any(|m| m.move_id == 0 || m.move_id > MAX_MOVE || m.level == 0 || m.level > MAX_LEVEL) {
            return Err(bad("attaque ou niveau invalide"));
        }
        let mut d = game.personal(e.id).ok_or_else(|| bad("fiche introuvable"))?.to_vec();
        let [hp, atk, def, spa, spd, spe] = e.stats;
        d[..6].copy_from_slice(&[hp, atk, def, spe, spa, spd]);
        d[6..8].copy_from_slice(&e.types);
        d[8] = e.catch_rate;
        d[0x16] = e.abilities[0] as u8;
        // 0 = un seul talent : le jeu ne tire alors jamais le second (`CreateBoxMon`).
        d[0x17] = e.abilities[1] as u8;
        game.set_personal(e.id, &d)?;

        let mut moves: Learnset = e.learnset.iter().map(|m| (m.move_id, m.level)).collect();
        moves.sort_by_key(|&(_, lvl)| lvl);
        let mut raw = encode_learnset(3, &moves);
        // Pas d'alignement en Gen 3 : la liste se termine au 0xFFFF.
        while raw.len() >= 2 && !raw.ends_with(&[0xFF, 0xFF]) {
            raw.pop();
        }
        debug_assert_eq!(learnsets::read(3, &raw), moves);
        gen3::set_learnset(game, e.id, &raw)?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gba_rom::synthetic;
    use crate::pokemon::PokeType;
    use crate::romedit::LevelMove;

    /// ROM synthétique : seule l'espèce éditée change, les autres sont intactes à la relecture.
    #[test]
    fn edit_and_reread() {
        for code in ["BPEF", "BPRF", "AXPF"] {
            let mut g = GbaGameRom::from_rom(synthetic::build(code, 0)).unwrap();
            let before = read(&g).unwrap();
            let mut bulba = before.species[0].clone();
            assert_eq!(bulba.stats, [45, 49, 49, 65, 65, 45]);
            bulba.stats[5] = 120;
            bulba.types = [before.types.iter().find(|t| t.tag.key == PokeType::Dragon).unwrap().index; 2];
            bulba.catch_rate = 255;
            bulba.learnset.push(LevelMove { level: 2, move_id: 1 });
            bulba.learnset.push(LevelMove { level: 9, move_id: 354 });
            assert_eq!(apply(&mut g, &[bulba.clone(), before.species[3].clone()]).unwrap(), 1);
            let reread = GbaGameRom::from_rom(kaleido_formats::gba::GbaRom::from_bytes(g.rom().to_bytes()).unwrap()).unwrap();
            let after = read(&reread).unwrap();
            bulba.learnset.sort_by_key(|m| m.level);
            assert_eq!(after.species[0], bulba);
            assert_eq!(after.species[1..], before.species[1..]);
            assert_eq!(reread.species().unwrap()[0].types.len(), 1);
        }
    }
}
