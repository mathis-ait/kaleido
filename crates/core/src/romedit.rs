//! Éditeur de ROM (DS) : fiche de chaque espèce (statistiques, types, talents,
//! taux de capture) et attaques apprises par niveau, réécrites dans une copie de la ROM.
//!
//! Mêmes emplacements que le randomizer ([DataPaths]) : seuls les jeux dont les
//! formats ont été vérifiés sur une vraie ROM sont modifiables.

use serde::{Deserialize, Serialize};

use crate::data::learnsets::{self, Learnset};
use crate::data::DataPaths;
use crate::pokemon::{Personal, PokeType, TypeTag};
use crate::rom::{GameRom, RomError};

/// Niveau maximal d'une attaque apprise.
pub const MAX_LEVEL: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelMove {
    pub level: u8,
    #[serde(rename = "move")]
    pub move_id: u16,
}

/// Données modifiables d'une espèce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeciesData {
    /// Numéro du Pokédex national.
    pub id: u16,
    /// PV, Attaque, Défense, Attaque Spé., Défense Spé., Vitesse.
    pub stats: [u8; 6],
    /// Index des deux types tels que stockés dans le jeu (identiques = un seul type).
    pub types: [u8; 2],
    /// Talent 1, talent 2, talent caché (toujours 0 en Gen 4).
    pub abilities: [u16; 3],
    pub catch_rate: u8,
    /// Attaques apprises, triées par niveau.
    pub learnset: Vec<LevelMove>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: u16,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeChoice {
    pub index: u8,
    pub tag: TypeTag,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorData {
    /// Le jeu a un talent caché (Gen 5).
    pub hidden_ability: bool,
    /// Nombre maximal d'attaques apprises par niveau (le plus long des originaux).
    pub max_learnset: usize,
    pub types: Vec<TypeChoice>,
    pub abilities: Vec<Choice>,
    pub moves: Vec<Choice>,
    pub species: Vec<SpeciesData>,
}

pub fn supports(game: &GameRom) -> bool {
    DataPaths::for_game(game.game).is_some()
}

fn paths(game: &GameRom) -> Result<DataPaths, RomError> {
    DataPaths::for_game(game.game)
        .ok_or_else(|| RomError::Unsupported(format!("l'édition de {} n'est pas encore prise en charge", game.game.name_fr())))
}

/// Noms non vides d'un fichier de texte, identifiant compris entre 1 et `max`.
pub(crate) fn choices(names: &[String], max: usize) -> Vec<Choice> {
    names
        .iter()
        .enumerate()
        .take(max + 1)
        .skip(1)
        .filter(|(_, n)| !n.is_empty() && n.as_str() != "-")
        .map(|(id, n)| Choice { id: id as u16, name: n.clone() })
        .collect()
}

pub(crate) fn type_choices(generation: u8) -> Vec<TypeChoice> {
    (0..=u8::MAX).map_while(|i| PokeType::from_index(generation, i).map(|t| TypeChoice { index: i, tag: t.into() })).collect()
}

pub(crate) fn ability_slots(generation: u8) -> std::ops::Range<usize> {
    if generation <= 4 {
        0x16..0x18
    } else {
        0x18..0x1B
    }
}

pub(crate) fn read_species(generation: u8, id: u16, personal: &[u8], learnset: &[u8]) -> Result<SpeciesData, RomError> {
    let p = Personal::new(generation, personal.to_vec()).ok_or_else(|| RomError::Layout(format!("fiche n°{id} trop courte")))?;
    let s = p.base_stats();
    let mut abilities = [0u16; 3];
    for (slot, &a) in abilities.iter_mut().zip(&personal[ability_slots(generation)]) {
        *slot = a as u16;
    }
    Ok(SpeciesData {
        id,
        stats: [s.hp, s.attack, s.defense, s.sp_attack, s.sp_defense, s.speed],
        types: [personal[6], personal[7]],
        abilities,
        catch_rate: p.catch_rate(),
        learnset: learnsets::read(generation, learnset).into_iter().map(|(move_id, level)| LevelMove { level, move_id }).collect(),
    })
}

/// Lit tout ce que l'éditeur peut modifier.
pub fn read(game: &GameRom) -> Result<EditorData, RomError> {
    let paths = paths(game)?;
    let gen = game.generation();
    let count = game.layout.species_count as usize;
    let personal = game.narc(game.layout.personal)?.files;
    let learnsets = game.narc(paths.learnsets)?.files;
    if personal.len() <= count || learnsets.len() <= count {
        return Err(RomError::Layout("fiches ou attaques apprises incomplètes".into()));
    }
    let move_names = game.text_file(paths.move_names)?;
    let ability_names = game.text_file(game.layout.ability_names)?;
    let species = (1..=count).map(|id| read_species(gen, id as u16, &personal[id], &learnsets[id])).collect::<Result<Vec<_>, _>>()?;
    Ok(EditorData {
        hidden_ability: gen >= 5,
        max_learnset: species.iter().map(|s| s.learnset.len()).max().unwrap_or(0),
        types: type_choices(gen),
        abilities: choices(&ability_names, paths.max_ability as usize),
        moves: choices(&move_names, move_names.len().saturating_sub(1)),
        species,
    })
}

/// Encode une liste d'attaques apprises (triée par niveau).
pub fn encode_learnset(generation: u8, learnset: &Learnset) -> Vec<u8> {
    let mut out = Vec::with_capacity(learnset.len() * 4 + 4);
    if generation <= 4 {
        for &(mv, lvl) in learnset {
            out.extend_from_slice(&((mv & 0x1FF) | (lvl as u16) << 9).to_le_bytes());
        }
        out.extend_from_slice(&[0xFF, 0xFF]);
        // Les fichiers d'origine sont alignés sur 4 octets.
        while out.len() % 4 != 0 {
            out.push(0);
        }
    } else {
        for &(mv, lvl) in learnset {
            out.extend_from_slice(&mv.to_le_bytes());
            out.extend_from_slice(&(lvl as u16).to_le_bytes());
        }
        out.extend_from_slice(&[0xFF; 4]);
    }
    out
}

/// Écrit les espèces modifiées dans la ROM (en mémoire). Renvoie le nombre d'espèces écrites.
pub fn apply(game: &mut GameRom, edits: &[SpeciesData]) -> Result<usize, RomError> {
    let paths = paths(game)?;
    let gen = game.generation();
    let count = game.layout.species_count;
    let original = read(game)?;
    let move_max = original.moves.iter().map(|m| m.id).max().unwrap_or(0);
    let types = original.types.len() as u8;

    let mut personal = game.narc(game.layout.personal)?;
    let mut learnset_narc = game.narc(paths.learnsets)?;
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
        let slots = ability_slots(gen).len();
        if e.abilities[..slots].iter().any(|&a| a > paths.max_ability) || e.abilities[0] == 0 {
            return Err(bad("talent invalide (le premier est obligatoire)"));
        }
        if e.learnset.len() > original.max_learnset {
            return Err(bad(&format!("{} attaques apprises au maximum", original.max_learnset)));
        }
        if e.learnset.iter().any(|m| m.move_id == 0 || m.move_id > move_max || m.level == 0 || m.level > MAX_LEVEL) {
            return Err(bad("attaque ou niveau invalide"));
        }

        let d = &mut personal.files[e.id as usize];
        let [hp, atk, def, spa, spd, spe] = e.stats;
        d[..6].copy_from_slice(&[hp, atk, def, spe, spa, spd]);
        d[6..8].copy_from_slice(&e.types);
        d[8] = e.catch_rate;
        for (at, &a) in ability_slots(gen).zip(&e.abilities) {
            d[at] = a as u8;
        }

        let mut moves: Learnset = e.learnset.iter().map(|m| (m.move_id, m.level)).collect();
        moves.sort_by_key(|&(_, lvl)| lvl);
        learnset_narc.files[e.id as usize] = encode_learnset(gen, &moves);
        written += 1;
    }
    if written > 0 {
        game.replace_narc(game.layout.personal, &personal)?;
        game.replace_narc(paths.learnsets, &learnset_narc)?;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learnsets_round_trip() {
        // Bulbizarre (Platine), fichier d'origine aligné sur 4 octets.
        let pt = [0x21, 0x02, 0x2D, 0x06, 0xFF, 0xFF, 0, 0];
        assert_eq!(encode_learnset(4, &learnsets::read(4, &pt)), pt);
        let bw = [0x21, 0, 1, 0, 0x2D, 0, 3, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(encode_learnset(5, &learnsets::read(5, &bw)), bw);
        assert_eq!(encode_learnset(4, &vec![(33, 1)]), [0x21, 0x02, 0xFF, 0xFF]);
    }

    fn rom(name: &str) -> Option<GameRom> {
        let p = crate::test_rom_path(name);
        p.exists().then(|| GameRom::open(&p).unwrap())
    }

    /// Relit une ROM modifiée : seule l'espèce éditée change, les autres sont intactes.
    #[test]
    fn edit_real_roms() {
        for name in [
            "Pokemon - Version Diamant (France) (Rev 5).nds",
            "Pokemon - Version Argent SoulSilver (France).nds",
            "Pokemon - Platinum Version (Europe).nds",
            "Pokemon - Version Blanche (France) (NDSi Enhanced).nds",
            "Pokemon - Version Noire 2 (France) (NDSi Enhanced).nds",
        ] {
            let Some(mut game) = rom(name) else {
                eprintln!("ROM absente, test ignoré : {name}");
                continue;
            };
            let before = read(&game).unwrap();
            let mut bulba = before.species[0].clone();
            assert_eq!(bulba.stats, [45, 49, 49, 65, 65, 45]);
            bulba.stats[5] = 120;
            bulba.types = [before.types.iter().find(|t| t.tag.key == PokeType::Dragon).unwrap().index; 2];
            bulba.catch_rate = 255;
            bulba.learnset.push(LevelMove { level: 2, move_id: 1 });
            assert_eq!(apply(&mut game, &[bulba.clone(), before.species[3].clone()]).unwrap(), 1);

            let bytes = game.rom().to_bytes().unwrap();
            let reread = GameRom::from_rom(kaleido_formats::nds::NdsRom::from_bytes(bytes).unwrap()).unwrap();
            let after = read(&reread).unwrap();
            bulba.learnset.sort_by_key(|m| m.level);
            assert_eq!(after.species[0], bulba);
            assert_eq!(after.species[1..], before.species[1..]);
            assert_eq!(reread.species().unwrap()[0].types.len(), 1);
        }
    }
}
