//! Éditeur de ROM Gen 1 et 2 (Game Boy) : statistiques, types, taux de capture et attaques
//! apprises, avec la même interface que `romedit` (DS). Pas de talents.
//!
//! Les fiches GB passent par le format Gen 3/4 de Kaleido (`data::gen12`). Les attaques sont
//! réécrites sur place : une espèce ne peut pas apprendre plus d'attaques qu'à l'origine.
//! En Gen 1, le Spécial prend la valeur de l'Attaque Spéciale. Non vérifié sur une vraie ROM.

use crate::data::gen12;
use crate::data::learnsets::Learnset;
use crate::gb_rom::GbGameRom;
use crate::pokemon::PokeType;
use crate::romedit::{encode_learnset, read_species, type_choices, Choice, EditorData, SpeciesData, MAX_LEVEL};
use crate::rom::RomError;

pub fn supports(_game: &GbGameRom) -> bool {
    true
}

fn max_move(g: &GbGameRom) -> u16 {
    if g.generation == 1 {
        165
    } else {
        251
    }
}

/// Lit tout ce que l'éditeur peut modifier.
pub fn read(g: &GbGameRom) -> Result<EditorData, RomError> {
    let mut species = Vec::with_capacity(g.species_count() as usize);
    for id in 1..=g.species_count() {
        let rec = gen12::to_gen4_record(g.generation, g.personal(id).ok_or_else(|| RomError::Layout(format!("fiche n°{id}")))?);
        let mut s = read_species(3, id, &rec, &gen12::learnset_gen4(g, id)?)?;
        s.abilities = [0; 3];
        species.push(s);
    }
    // Types de la génération seulement (ni Acier, ni Ténèbres, ni « ??? » en Gen 1).
    let types = type_choices(3)
        .into_iter()
        .filter(|t| !matches!(t.tag.key, PokeType::Mystery) && (g.generation == 2 || !matches!(t.tag.key, PokeType::Steel | PokeType::Dark)))
        .collect();
    let moves = crate::dex::move_names()
        .iter()
        .enumerate()
        .take(max_move(g) as usize + 1)
        .skip(1)
        .map(|(id, n)| Choice { id: id as u16, name: n.to_string() })
        .collect();
    Ok(EditorData { hidden_ability: false, max_learnset: species.iter().map(|s| s.learnset.len()).max().unwrap_or(0), types, abilities: Vec::new(), moves, species })
}

/// Écrit les espèces modifiées dans la ROM (en mémoire). Renvoie le nombre d'espèces écrites.
pub fn apply(g: &mut GbGameRom, edits: &[SpeciesData]) -> Result<usize, RomError> {
    let original = read(g)?;
    let allowed: Vec<u8> = original.types.iter().map(|t| t.index).collect();
    let mut written = 0;
    for e in edits {
        let bad = |what: &str| RomError::Unsupported(format!("Pokémon n°{} : {what}", e.id));
        if e.id == 0 || e.id > g.species_count() {
            return Err(RomError::Unsupported(format!("Pokémon n°{} inexistant dans ce jeu", e.id)));
        }
        let before = &original.species[e.id as usize - 1];
        if before == e {
            continue;
        }
        if e.stats.contains(&0) {
            return Err(bad("une statistique de base ne peut pas valoir 0"));
        }
        if e.types.iter().any(|t| !allowed.contains(t)) {
            return Err(bad("type absent de cette génération"));
        }
        if e.learnset.len() > before.learnset.len() {
            return Err(bad(&format!("{} attaques apprises au maximum sur Game Boy", before.learnset.len())));
        }
        if e.learnset.iter().any(|m| m.move_id == 0 || m.move_id > max_move(g) || m.level == 0 || m.level > MAX_LEVEL) {
            return Err(bad("attaque ou niveau invalide"));
        }
        let raw = g.personal(e.id).ok_or_else(|| bad("fiche introuvable"))?.to_vec();
        let mut rec = gen12::to_gen4_record(g.generation, &raw);
        let [hp, atk, def, spa, spd, spe] = e.stats;
        rec[..6].copy_from_slice(&[hp, atk, def, spe, spa, spd]);
        rec[6..8].copy_from_slice(&e.types);
        rec[8] = e.catch_rate;
        g.set_personal(e.id, &gen12::from_gen4_record(g.generation, &raw, &rec))?;
        let mut moves: Learnset = e.learnset.iter().map(|m| (m.move_id, m.level)).collect();
        moves.sort_by_key(|&(_, lvl)| lvl);
        gen12::set_learnset_gen4(g, e.id, &encode_learnset(4, &moves))?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gb_rom::synthetic;

    #[test]
    fn edit_and_reread() {
        for section in ["Red (F)", "Gold (F)"] {
            let mut g = GbGameRom::from_rom(synthetic::build(section)).unwrap();
            let before = read(&g).unwrap();
            let mut bulba = before.species[0].clone();
            bulba.stats[5] = 120;
            if g.generation == 1 {
                // Gen 1 : un seul Spécial, pris de l'Attaque Spéciale.
                bulba.stats[4] = bulba.stats[3];
            }
            bulba.types = [before.types.iter().find(|t| t.tag.key == PokeType::Dragon).unwrap().index; 2];
            bulba.catch_rate = 255;
            bulba.learnset[0].move_id = 89;
            assert_eq!(apply(&mut g, &[bulba.clone(), before.species[3].clone()]).unwrap(), 1);
            let reread = GbGameRom::from_rom(kaleido_formats::gb::GbRom::from_bytes(g.rom().to_bytes()).unwrap()).unwrap();
            let after = read(&reread).unwrap();
            bulba.learnset.sort_by_key(|m| m.level);
            assert_eq!(after.species[0], bulba, "{section}");
            assert_eq!(after.species[1..], before.species[1..]);
            // Plus d'attaques qu'à l'origine : refusé.
            let mut more = after.species[3].clone();
            more.learnset.push(crate::romedit::LevelMove { level: 50, move_id: 1 });
            assert!(apply(&mut g, &[more]).is_err());
            if g.generation == 1 {
                let mut steel = after.species[3].clone();
                steel.types = [8, 8];
                assert!(apply(&mut g, &[steel]).is_err());
            }
        }
    }
}
