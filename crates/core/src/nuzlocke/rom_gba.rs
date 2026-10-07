//! Mode Nuzlocke et compagnon pour les jeux Gen 3 : routes, champions et familles.
//!
//! Chaque en-tête de rencontres donne la carte (banque, numéro) ; l'en-tête de la carte
//! donne sa section (`mapsec`, octet +0x14), qui est aussi le lieu de rencontre enregistré
//! dans les Pokémon (PK3). Les noms viennent de la liste de lieux Gen 3 de PKHeX
//! (`met3_00000`), en français quelle que soit la langue de la ROM.
//!
//! Indices des champions et du Conseil 4 : décompilations pokeruby / pokeemerald /
//! pokefirered (`opponents.h`) et `EliteFourIndices` du fichier d'offsets de l'UPR.
//! Non vérifiés sur une vraie ROM : `verified` vaut `false` si l'équipe lue est vide.

use std::collections::BTreeMap;

use super::rom::{Encounter, Leader, LeaderKind, RomInfo, Route};
use crate::data::{evolutions, gen3};
use crate::dex;
use crate::gba_rom::{GbaGameRom, SPECIES_COUNT};
use crate::games::Game;
use crate::rom::RomError;

/// Carte d'une sauvegarde Gen 3 : `banque << 8 | numéro` (voir `SaveFile::current_map`).
pub fn map_key(bank: u8, map: u8) -> u16 {
    (bank as u16) << 8 | map as u16
}

/// Ordre approximatif de l'histoire (sections de carte).
fn story_order(game: Game) -> Vec<u16> {
    match game {
        // Route 101, 102, 103, 104, Bois Clémenti, 116, Tunnel Mérazon, Grotte Granite, 105-109,
        // 110, 117, 111, 112, Chemin Ardent, 113, 114, Mont Chimnée, 118, 119, 120, 121…
        Game::Ruby | Game::Sapphire | Game::Emerald => {
            let mut v = vec![16, 17, 18, 19, 59, 31, 60, 55, 20, 21, 22, 23, 24, 25, 32, 26, 27, 74, 28, 29, 56, 65, 76, 33, 34, 35, 36, 57, 37, 38];
            v.extend(39..=49);
            v.extend([67, 68, 61, 63, 62, 70, 72, 71, 73, 80]);
            v
        }
        // Route 1, 22, 2, Forêt de Jade, 3, Mont Sélénite, 4, 24, 25, 5, 6, 11, Cave Taupiqueur,
        // 9, 10, Grotte, Tour Pokémon, 8, 7, 12-18, Parc Safari, 19-21, Îles Écume, Manoir, 23,
        // Route Victoire, puis les Îles Sevii.
        _ => {
            let mut v = vec![101, 122, 102, 126, 103, 127, 104, 124, 125, 105, 106, 111, 131, 109, 110, 138, 140, 108, 107];
            v.extend(112..=118);
            v.extend([136, 119, 120, 121, 139, 135, 123, 132, 141, 142]);
            v.extend(143..=195);
            v
        }
    }
}

struct LeaderDef {
    kind: LeaderKind,
    name: &'static str,
    town: &'static str,
    trainers: &'static [u16],
}

const fn gym(name: &'static str, town: &'static str, trainers: &'static [u16]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Gym, name, town, trainers }
}

const fn elite(name: &'static str, trainers: &'static [u16]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Elite, name, town: "Ligue Pokémon", trainers }
}

fn leader_defs(game: Game) -> Vec<LeaderDef> {
    match game {
        Game::FireRed | Game::LeafGreen => vec![
            gym("Pierre", "Argenta", &[414]),
            gym("Ondine", "Azuria", &[415]),
            gym("Major Bob", "Carmin sur Mer", &[416]),
            gym("Érika", "Céladopole", &[417]),
            gym("Koga", "Parmanie", &[418]),
            gym("Morgane", "Safrania", &[420]),
            gym("Auguste", "Cramois'Île", &[419]),
            gym("Giovanni", "Jadielle", &[350]),
            elite("Olga", &[410]),
            elite("Aldo", &[411]),
            elite("Agatha", &[412]),
            elite("Peter", &[413]),
            LeaderDef { kind: LeaderKind::Champion, name: "Blue", town: "Ligue Pokémon", trainers: &[438, 439, 440] },
        ],
        _ => {
            let eighth = if game == Game::Emerald { "Juan" } else { "Marc" };
            let champion = if game == Game::Emerald { "Marc" } else { "Pierre Rochard" };
            vec![
                gym("Roxanne", "Mérouville", &[265]),
                gym("Bastien", "Village Myokara", &[266]),
                gym("Voltère", "Lavandia", &[267]),
                gym("Adriane", "Vermilava", &[268]),
                gym("Norman", "Clémenti-Ville", &[269]),
                gym("Alizée", "Cimetronelle", &[270]),
                gym("Lévy et Tatia", "Algatia", &[271]),
                gym(eighth, "Atalanopolis", &[272]),
                elite("Damien", &[261]),
                elite("Spectra", &[262]),
                elite("Glacia", &[263]),
                elite("Aragon", &[264]),
                LeaderDef { kind: LeaderKind::Champion, name: champion, town: "Éternara", trainers: &[335] },
            ]
        }
    }
}

fn read_leaders(g: &GbaGameRom) -> Vec<Leader> {
    let mut gym_n = 0;
    leader_defs(g.game)
        .into_iter()
        .map(|def| {
            let mut ace: Option<(u16, u8)> = None;
            let mut class_name = String::new();
            let mut verified = true;
            for &id in def.trainers {
                match gen3::trainer(g, id as usize) {
                    Ok(t) if !t.party.is_empty() => {
                        if class_name.is_empty() {
                            class_name = gen3::trainer_class_name(g, t.class).unwrap_or_default();
                        }
                        for m in &t.party {
                            let level = m.level.min(255) as u8;
                            if ace.is_none_or(|(_, l)| level > l) {
                                ace = Some((m.species, level));
                            }
                        }
                    }
                    _ => verified = false,
                }
            }
            let label = match def.kind {
                LeaderKind::Gym => {
                    gym_n += 1;
                    format!("Arène {gym_n}")
                }
                LeaderKind::Elite => "Conseil 4".into(),
                LeaderKind::Champion => "Maître".into(),
            };
            let (ace_species, ace_level) = ace.unwrap_or((0, 0));
            Leader { kind: def.kind, label, name: def.name, town: def.town, trainer_ids: def.trainers.to_vec(), ace_species, ace_level, class_name, verified }
        })
        .collect()
}

fn families(g: &GbaGameRom) -> Vec<u16> {
    let count = SPECIES_COUNT as usize;
    let mut parent = vec![0u16; count + 1];
    for s in 1..=count {
        let Ok(raw) = gen3::evolutions(g, s as u16) else { continue };
        for e in evolutions::read(&raw) {
            let t = e.target as usize;
            if t <= count && parent[t] == 0 && t != s {
                parent[t] = s as u16;
            }
        }
    }
    (0..=count)
        .map(|s| {
            let mut cur = s;
            for _ in 0..4 {
                match parent[cur] {
                    0 => break,
                    p => cur = p as usize,
                }
            }
            cur as u16
        })
        .collect()
}

/// Lit routes, dresseurs clés, starters et familles d'évolution d'un jeu Gen 3.
pub fn read(g: &GbaGameRom) -> Result<RomInfo, RomError> {
    let order = story_order(g.game);
    let mut by_name: BTreeMap<String, Route> = BTreeMap::new();
    let mut map_locations = vec![u16::MAX; 0x10000];
    for area in gen3::wild_areas(g)? {
        let section = gen3::map_section(g, area.bank, area.map).map_or(u16::MAX, u16::from);
        map_locations[map_key(area.bank, area.map) as usize] = section;
        let name = dex::location_name(3, section).map_or_else(|| format!("Carte {}.{}", area.bank, area.map), str::to_string);
        let route = by_name.entry(name.clone()).or_insert_with(|| Route {
            key: name.clone(),
            name,
            location_ids: Vec::new(),
            zones: Vec::new(),
            encounters: Vec::new(),
            order: u32::MAX,
        });
        if section != u16::MAX && !route.location_ids.contains(&section) {
            route.location_ids.push(section);
        }
        if let Some(pos) = order.iter().position(|&l| l == section) {
            route.order = route.order.min(pos as u32);
        }
        route.zones.push(map_key(area.bank, area.map));
        let method = area.kind.label();
        for slot in &area.slots {
            match route.encounters.iter_mut().find(|e| e.species == slot.species) {
                Some(e) => {
                    e.min_level = e.min_level.min(slot.min_level);
                    e.max_level = e.max_level.max(slot.max_level);
                    if !e.methods.contains(&method) {
                        e.methods.push(method);
                    }
                }
                None => route.encounters.push(Encounter { species: slot.species, min_level: slot.min_level, max_level: slot.max_level, methods: vec![method] }),
            }
        }
    }
    // Lieux des cartes sans rencontre (villes…) pour le compagnon : lus à la demande.
    if let Some(banks) = g.layout.map_banks {
        for bank in 0..64u8 {
            let Some(bank_at) = g.rom().pointer(banks + bank as usize * 4) else { break };
            for map in 0..=255u8 {
                let Some(header) = g.rom().pointer(bank_at + map as usize * 4) else { break };
                if let Some(section) = g.rom().u8(header + 0x14) {
                    map_locations[map_key(bank, map) as usize] = section as u16;
                }
            }
        }
    }
    let mut routes: Vec<Route> = by_name.into_values().filter(|r| !r.encounters.is_empty()).collect();
    let base = order.len() as u32;
    for r in &mut routes {
        r.location_ids.sort_unstable();
        if r.order == u32::MAX {
            r.order = base + r.location_ids.first().copied().unwrap_or(999) as u32;
        }
    }
    routes.sort_by_key(|r| r.order);
    let seed = g.rom().signature().and_then(|j| serde_json::from_slice::<crate::randomizer::KaleidoTag>(j).ok()).map(|t| t.seed);
    Ok(RomInfo {
        game: g.game,
        routes,
        leaders: read_leaders(g),
        starters: gen3::starters(g).map(|s| s.to_vec()).unwrap_or_default(),
        family: families(g),
        seed,
        map_locations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gba_rom::synthetic;

    #[test]
    fn routes_and_leaders_from_synthetic_rom() {
        let mut rom = synthetic::build("BPRF", 0);
        synthetic::add_grass(&mut rom, 3, 19, &[16, 16, 19, 19, 16, 19, 16, 19, 10, 10, 13, 13]);
        let g = GbaGameRom::from_rom(rom).unwrap();
        let info = read(&g).unwrap();
        assert_eq!(info.routes.len(), 1);
        // Pas de table des cartes dans la ROM synthétique : nom de repli.
        assert_eq!(info.routes[0].name, "Carte 3.19");
        assert_eq!(info.routes[0].encounters.len(), 4);
        assert_eq!(info.starters, vec![1, 4, 7]);
        assert_eq!(info.family_of(3), 1);
        assert_eq!(info.leaders.len(), 13);
        assert_eq!(info.leaders[0].name, "Pierre");
        assert!(!info.leaders[0].verified);
    }
}
