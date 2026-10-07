//! Mode Nuzlocke, jeux 3DS : X / Y, Rubis Oméga / Saphir Alpha, Soleil / Lune et
//! Ultra-Soleil / Ultra-Lune. Même résultat que [`super::rom::read`] pour les jeux DS :
//! routes (zones de rencontre regroupées par nom de lieu), dresseurs clés, starters et
//! familles d'évolution.
//!
//! ## Zones ↔ lieux de rencontre de la sauvegarde
//!
//! - **X / Y, ROSA** (UPR-ZX `Gen6RomHandler.loadWildMapNames`, pk3DS `ZoneData`) : l'archive
//!   des rencontres contient un fichier « ZO » (LZ11) par zone, puis la table des zones, non
//!   compressée : 0x38 octets par zone, nom du lieu = 10 bits bas du `u16` en +0x1C (Y : 360
//!   zones, table en entrée 360 ; Rubis Oméga : 536 zones). Vérifié sur Y et Rubis Oméga.
//! - **Soleil / Lune, Ultra** (pk3DS `Area7` / `ZoneData7`, UPR-ZX `Gen7RomHandler.getAreaData`) :
//!   `a/0/7/7` entrée 0 = zones de 0x54 octets (nom du lieu = `u32` en +0x1C), entrée 1 =
//!   monde de chaque zone (`u16`) ; `a/0/9/1` = mondes (paquets « WD », dont le premier fichier
//!   donne en `u32` @8 le début d'une liste de couples `u16` zone, `u16` aire). Les rencontres
//!   de l'aire `n` sont l'entrée `9 + 11 × n` de l'archive des rencontres ; toutes ses tables
//!   valent pour chacune de ses zones (comme le fait PKHeX, `Gen7SlotDumper`). Vérifié sur
//!   Lune et Ultra-Soleil.
//!
//! Le nom de lieu est l'index dans le fichier de textes des lieux (Y : 72, ROSA : 90,
//! Soleil / Lune : 67, Ultra : 72) ; c'est aussi le lieu de rencontre enregistré dans les
//! Pokémon (PK6 / PK7 : listes `met6_00000` / `met7_00000` de PKHeX, mêmes lignes). Les
//! lignes impaires sont des sous-lieux (« Sentier Croquis ») : la route garde le nom
//! principal, de sorte que tous les morceaux d'une même route (Route 1 et ses abords en
//! Alola) n'en font qu'une.

use std::collections::BTreeMap;
use std::path::Path;

use kaleido_formats::lz;

use super::rom::{Encounter, Leader, LeaderKind, RomInfo, Route};
use crate::ctr_rom::CtrGameRom;
use crate::data::encounters::{self, Format, SlotKind};
use crate::data::{trainers, u16_at};
use crate::dex;
use crate::games::Game;
use crate::rom::{GameRom, RomError};

/// Jeux 3DS pris en charge par le mode Nuzlocke.
pub fn supports(game: Game) -> bool {
    game.generation() >= 6 && crate::ctr_rom::CtrLayout::for_game(game).is_some()
}

/// Ouvre une ROM DS (`.nds`) ou 3DS (image ou dossier extrait) et lit ce qu'il faut au mode Nuzlocke.
pub fn read_path(path: &Path) -> Result<RomInfo, RomError> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds")) {
        let mut info = super::rom::read(&GameRom::open(path)?)?;
        info.seed = super::rom::kaleido_seed(path);
        Ok(info)
    } else {
        read(&CtrGameRom::open(path)?)
    }
}

/// Fichier de textes des noms de lieux (UPR-ZX `MapNamesTextOffset`, vérifié).
fn place_names_text(game: Game) -> usize {
    match game {
        Game::X | Game::Y | Game::UltraSun | Game::UltraMoon => 72,
        Game::OmegaRuby | Game::AlphaSapphire => 90,
        _ => 67,
    }
}

/// Lit routes, dresseurs clés, starters et familles d'évolution d'un jeu 3DS.
pub fn read(game: &CtrGameRom) -> Result<RomInfo, RomError> {
    let g = game.game;
    if !supports(g) {
        return Err(RomError::Unsupported(format!("le mode Nuzlocke ne gère pas encore {}", g.name_fr())));
    }
    let names = game.text_file(place_names_text(g))?;
    let (zones, map_locations) = if g.generation() == 7 { alola_zones(game)? } else { gen6_zones(game)? };
    let routes = group_routes(g, &zones, &names);
    let leaders = read_leaders(game)?;
    let starters = read_starters(game).unwrap_or_default();
    let family = read_families(game).unwrap_or_default();
    Ok(RomInfo { game: g, routes, leaders, starters, family, seed: None, map_locations })
}

/// Une zone : identifiant du lieu et rencontres (fichier décompressé).
struct Zone {
    /// Numéro du fichier de rencontres (zone en Gen 6, aire en Gen 7).
    file: u16,
    location: u16,
    data: std::rc::Rc<Vec<u8>>,
}

fn decompressed(d: &[u8]) -> Vec<u8> {
    if lz::is_lz11(d) {
        lz::decompress(d).unwrap_or_default()
    } else {
        Vec::new()
    }
}

const GEN6_ZONE_SIZE: usize = 0x38;

/// X / Y et ROSA : un fichier par zone, puis la table des zones (entrée `n` de `n × 0x38` octets).
/// Renvoie aussi le lieu de chaque zone (index = numéro de zone de la sauvegarde).
fn gen6_zones(game: &CtrGameRom) -> Result<(Vec<Zone>, Vec<u16>), RomError> {
    let garc = game.garc(game.layout.encounters)?;
    let table_index = (1..garc.len())
        .rev()
        .find(|&i| garc.file(i).is_some_and(|d| d.len() == i * GEN6_ZONE_SIZE))
        .ok_or_else(|| RomError::Layout("table des zones introuvable dans l'archive des rencontres".into()))?;
    let table = garc.file(table_index).unwrap_or_default();
    let zones: Vec<Zone> = table
        .as_chunks::<GEN6_ZONE_SIZE>()
        .0
        .iter()
        .enumerate()
        .map(|(i, z)| Zone {
            file: i as u16,
            location: u16_at(z, 0x1C) & 0x3FF,
            data: std::rc::Rc::new(decompressed(garc.file(i).unwrap_or_default())),
        })
        .collect();
    let locations = zones.iter().map(|z| z.location).collect();
    Ok((zones, locations))
}

/// Fichiers d'un paquet « mini » (`EA`, `WD`…) : `u16` nombre @2, puis `nombre + 1` positions `u32`.
fn mini_unpack<'a>(d: &'a [u8], magic: &[u8; 2]) -> Vec<&'a [u8]> {
    if d.len() < 4 || &d[..2] != magic {
        return Vec::new();
    }
    let count = u16_at(d, 2) as usize;
    let offset = |i: usize| d.get(4 + i * 4..8 + i * 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    (0..count).filter_map(|i| d.get(offset(i)?..offset(i + 1)?)).collect()
}

const ZONE_DATA_7: &str = "a/0/7/7";
const WORLD_DATA_7: &str = "a/0/9/1";
const ZONE7_SIZE: usize = 0x54;
const AREA_FIRST: usize = 9;
const AREA_STRIDE: usize = 11;

/// Soleil / Lune, Ultra : chaque zone rattachée à son aire de rencontres via son monde.
/// Renvoie aussi le lieu de chaque zone (index = numéro de zone de la sauvegarde).
fn alola_zones(game: &CtrGameRom) -> Result<(Vec<Zone>, Vec<u16>), RomError> {
    // Zones et mondes sont compressés (LZ11), sauf le monde de chaque zone.
    let raw = |d: Option<&[u8]>| -> Vec<u8> {
        let d = d.unwrap_or_default();
        if lz::is_lz11(d) {
            lz::decompress(d).unwrap_or_default()
        } else {
            d.to_vec()
        }
    };
    let zd = game.garc(ZONE_DATA_7)?;
    let wd = game.garc(WORLD_DATA_7)?;
    let enc = game.garc(game.layout.encounters)?;
    let (zone_data, zone_world) = (raw(zd.file(0)), raw(zd.file(1)));
    let world_files: Vec<Vec<u8>> = (0..wd.len()).map(|i| raw(wd.file(i))).collect();
    let worlds: Vec<&[u8]> = world_files.iter().map(|f| mini_unpack(f, b"WD").first().copied().unwrap_or_default()).collect();
    let locations = zone_data
        .as_chunks::<ZONE7_SIZE>()
        .0
        .iter()
        .map(|z| u32::from_le_bytes([z[0x1C], z[0x1D], z[0x1E], z[0x1F]]).min(u16::MAX as u32) as u16)
        .collect();
    let mut areas: BTreeMap<usize, std::rc::Rc<Vec<u8>>> = BTreeMap::new();
    let mut out = Vec::new();
    for (i, z) in zone_data.as_chunks::<ZONE7_SIZE>().0.iter().enumerate() {
        let Some(world) = zone_world.get(i * 2..i * 2 + 2).and_then(|w| worlds.get(u16_at(w, 0) as usize)) else { continue };
        let Some(start) = world.get(8..12).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else { continue };
        // Liste (zone, aire) du monde : une zone absente n'a pas de rencontres.
        let area = world
            .get(start..)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .find(|e| u16_at(&e[..], 0) as usize == i)
            .map(|e| u16_at(&e[..], 2) as usize);
        let Some(area) = area else { continue };
        let file = AREA_FIRST + AREA_STRIDE * area;
        if file >= enc.len() {
            continue;
        }
        let data = areas.entry(file).or_insert_with(|| std::rc::Rc::new(decompressed(enc.file(file).unwrap_or_default()))).clone();
        let location = u32::from_le_bytes([z[0x1C], z[0x1D], z[0x1E], z[0x1F]]);
        out.push(Zone { file: file as u16, location: location.min(u16::MAX as u32) as u16, data });
    }
    Ok((out, locations))
}

/// Nom principal d'un lieu : texte de la ROM, sinon liste de PKHeX (sans le sous-lieu).
fn place_name(game: Game, id: u16, rom_names: &[String]) -> String {
    let rom = rom_names.get(id as usize).map(|n| n.trim()).filter(|n| !n.is_empty() && !n.starts_with('{')).map(str::to_string);
    let pkhex = || {
        dex::location_name(game.generation(), id).map(|n| match n.split_once(" (") {
            Some((base, _)) if n.ends_with(')') => base.to_string(),
            _ => n.to_string(),
        })
    };
    rom.or_else(pkhex).unwrap_or_else(|| format!("Lieu n°{id}"))
}

/// Mode de rencontre d'un emplacement (position dans le fichier décompressé).
fn method(game: Game, data: &[u8], slot: &encounters::Slot) -> &'static str {
    match game.generation() {
        7 => {
            let SlotKind::TableLevels(base) = slot.kind else { return "Herbe" };
            let night = !encounters::alola_tables(data).contains(&(base - 4));
            if slot.offset >= base + 0x14C {
                "Appel à l'aide (météo)"
            } else if slot.offset >= base + 0x0C + 40 {
                if night {
                    "Appel à l'aide (nuit)"
                } else {
                    "Appel à l'aide (jour)"
                }
            } else if night {
                "Sauvage (nuit)"
            } else {
                "Sauvage (jour)"
            }
        }
        _ => {
            if let Some(section) = encounters::xy_section(data) {
                return match (slot.offset - section.start - encounters::XY_RATES) / 4 {
                    0..12 => "Herbe",
                    12..24 => "Fleurs jaunes",
                    24..36 => "Fleurs violettes",
                    36..48 => "Fleurs rouges",
                    48..60 => "Hautes herbes",
                    60..65 => "Surf",
                    65..70 => "Éclate-Roc",
                    70..73 => "Canne",
                    73..76 => "Super Canne",
                    76..79 => "Méga Canne",
                    _ => "Horde",
                };
            }
            let start = encounters::oras_section(data).map_or(0, |s| s.start);
            match (slot.offset.saturating_sub(start + encounters::ORAS_RATES)) / 4 {
                0..12 => "Herbe",
                12..24 => "Hautes herbes",
                // UPR-ZX : « DexNav Foreign Encounter ».
                24..27 => "Navi-Dex",
                27..32 => "Surf",
                32..37 => "Éclate-Roc",
                37..40 => "Canne",
                40..43 => "Super Canne",
                43..46 => "Méga Canne",
                _ => "Horde",
            }
        }
    }
}

/// Gen 7 : moitiés de tables (jour ou nuit) dont la rencontre normale ne propose que des
/// Picassaut — tables de remplissage (niveaux 2-3, comme les « Pikipek Table » que PKHeX
/// écarte dans `Gen7SlotDumper`), vues par exemple aux Ruines de l'Essor en Lune.
fn alola_placeholders(data: &[u8]) -> Vec<usize> {
    const PIKIPEK: u16 = 731;
    encounters::alola_tables(data)
        .into_iter()
        .flat_map(|t| [t + 4, t + 4 + encounters::ALOLA_HALF])
        .filter(|&base| {
            let row: Vec<u16> = (0..10)
                .filter_map(|i| data.get(base + 0x0C + i * 4..base + 0x0E + i * 4))
                .map(|b| u16_at(b, 0) & 0x7FF)
                .filter(|&s| s != 0)
                .collect();
            !row.is_empty() && row.iter().all(|&s| s == PIKIPEK)
        })
        .collect()
}

/// Regroupe les zones par nom de lieu et agrège leurs rencontres.
fn group_routes(game: Game, zones: &[Zone], rom_names: &[String]) -> Vec<Route> {
    let order = story_order(game);
    let format = Format::for_game(game);
    let mut by_name: BTreeMap<String, Route> = BTreeMap::new();
    for z in zones {
        if z.data.is_empty() {
            continue;
        }
        let name = place_name(game, z.location, rom_names);
        let route = by_name.entry(name.clone()).or_insert_with(|| Route {
            key: name.clone(),
            name,
            location_ids: Vec::new(),
            zones: Vec::new(),
            encounters: Vec::new(),
            order: u32::MAX,
        });
        if !route.location_ids.contains(&z.location) {
            route.location_ids.push(z.location);
        }
        if let Some(pos) = order.iter().position(|&l| l == z.location) {
            route.order = route.order.min(pos as u32);
        }
        if route.zones.contains(&z.file) {
            continue;
        }
        route.zones.push(z.file);
        let placeholders = alola_placeholders(&z.data);
        for slot in encounters::read_format(format, &z.data) {
            if matches!(slot.kind, SlotKind::TableLevels(base) if placeholders.contains(&base)) {
                continue;
            }
            let m = method(game, &z.data, &slot);
            match route.encounters.iter_mut().find(|e| e.species == slot.species) {
                Some(e) => {
                    e.min_level = e.min_level.min(slot.min_level);
                    e.max_level = e.max_level.max(slot.max_level);
                    if !e.methods.contains(&m) {
                        e.methods.push(m);
                    }
                }
                None => {
                    route.encounters.push(Encounter { species: slot.species, min_level: slot.min_level, max_level: slot.max_level, methods: vec![m] })
                }
            }
        }
    }
    let mut routes: Vec<Route> = by_name.into_values().filter(|r| !r.encounters.is_empty()).collect();
    // Hors liste : après l'histoire, par identifiant.
    let base = order.len() as u32;
    for r in &mut routes {
        r.location_ids.sort_unstable();
        if r.order == u32::MAX {
            r.order = base + r.location_ids[0] as u32;
        }
    }
    routes.sort_by_key(|r| r.order);
    routes
}

/// Lieux dans l'ordre (approximatif) de l'histoire ; identifiants des fichiers de textes des lieux.
fn story_order(game: Game) -> &'static [u16] {
    match game {
        Game::X | Game::Y => XY_ORDER,
        Game::OmegaRuby | Game::AlphaSapphire => ORAS_ORDER,
        Game::Sun | Game::Moon => SM_ORDER,
        _ => USUM_ORDER,
    }
}

/// X / Y (texte 72) : Route 2, Forêt de Neuvartault, Route 3, Route 22, Route 4, Route 5,
/// Route 6, Palais Chaydeuvre, Route 7, Cave Connecterre, Route 8, Relifac-le-Haut,
/// Roche-sur-Gliffe, Route 9, Grotte Étincelante, Route 10, Route 11, Grotte Miroitante,
/// Yantreizh, Route 12, Baie Azur, Port Tempères, Route 13, Route 14, Romant-sous-Bois,
/// Route 15, Hôtel Désolation, Route 16, La Frescale, Caverne Gelée, Route 17, Flusselles,
/// Route 18, Mozheim, Route 19, Route 20, Village Pokémon, Route 21, Route Victoire, puis
/// l'après-Ligue (Grotte Coda…).
const XY_ORDER: &[u16] = &[
    12, 14, 16, 102, 20, 28, 34, 36, 38, 134, 42, 40, 44, 46, 132, 50, 54, 56, 58, 62, 112, 64, 66, 68, 70, 74, 142, 78, 76, 82, 84, 86, 88, 90, 92,
    96, 98, 100, 104,
];

/// Rubis Oméga / Saphir Alpha (texte 90) : Routes 101, 103, 102, Clémenti-Ville, Route 104,
/// Bois Clémenti, Mérouville, Route 116, Tunnel Mérazon, Myokara, Chenal 106, Grotte Granite,
/// Chenal 109, Poivressel, Route 110, Lavandia, Routes 117, 111, 112, Chemin Ardent, Routes 113,
/// 114, Site Météore, Route 115, Sentier Sinuroc, Vermilava, Chenaux 105, 107, 108, Routes 118,
/// 119, Cimetronelle, Routes 120, 121, Parc Safari, Chenal 122, Mont Mémoria, Route 123,
/// Nénucrique, Planques Magma et Aqua, Chenal 124, Algatia, Chenal 125, Grotte Tréfonds,
/// Chenaux 126 à 128, Caverne Fondmer, Atalanopolis, Grotte Origine, Chenaux 129 à 131,
/// Pacifiville, Chenaux 132 à 134, Pilier Céleste, Éternara, Route Victoire, puis l'après-Ligue.
const ORAS_ORDER: &[u16] = &[
    204, 208, 206, 184, 210, 282, 190, 234, 274, 174, 214, 280, 220, 186, 222, 188, 236, 224, 226, 288, 228, 230, 272, 232, 286, 176, 212, 216, 218,
    238, 240, 192, 242, 244, 324, 246, 290, 248, 194, 314, 292, 250, 196, 252, 300, 254, 256, 258, 294, 198, 296, 260, 262, 264, 182, 266, 268, 270,
    316, 200, 298,
];

/// Soleil / Lune (texte 67). Mele-Mele : Route 1, Ekaeka, Route 2, Champ de Baies, Cimetière
/// d'Ekaeka, Colline Dicarat, Grotte Verdoyante, Route 3, Jardin de Mele-Mele, Grotte Verlamer,
/// Baie de Kala'e, Mer de Mele-Mele, Ruines du Conflit. Akala : Route 4, Ohana, Ranch Ohana,
/// Route 5, Colline Clapotis, Routes 6 et 7, Parc Volcanique, Route 8, Jungle Sombrefeuille,
/// Tunnel Taupiqueur, Route 9, Club et Plage Hano-Hano, Colline Memento, Côte Reculée d'Akala,
/// Ruines de l'Éveil, Jardin d'Akala, Konikoni. Ula-Ula : Malié, Parc de Malié, Route 10, Mont
/// Hokulani, Routes 11 et 12, Côte Sauvage, Route 13, Désert Haina, Village Toko, Route 14,
/// Bradley Prix, Routes 15 et 16, Jardin d'Ula-Ula, Route 17, Mont Ardent, Ruines de l'Essor,
/// Lacs du Halo. Poni : Village Flottant, Prairie de Poni, Vieille Route, Récif de Poni, Ruines
/// de l'Au-Delà, Île Noadkoko, Grand Canyon de Poni, Autels, Mont Lanakila, Ligue, puis
/// l'après-Ligue (Forêt, Plaine et Côte de Poni, Chemin du Défi, Jardin de Poni, Caverne Coda).
const SM_ORDER: &[u16] = &[
    6, 8, 18, 20, 22, 12, 44, 38, 34, 36, 46, 48, 10, 40, 42, 14, 16, 30, 32, 50, 68, 78, 52, 86, 88, 54, 56, 82, 84, 58, 90, 100, 60, 62, 64, 76,
    94, 92, 66, 72, 132, 192, 134, 106, 136, 108, 122, 110, 112, 124, 114, 126, 150, 116, 118, 128, 120, 138, 140, 142, 144, 172, 158, 160, 162, 180,
    184, 174, 176, 178, 146, 154, 164, 166, 168, 170, 156, 182,
];

/// Ultra-Soleil / Ultra-Lune (texte 72) : mêmes lieux que Soleil / Lune, plus la Grotte de la
/// Plage (200, Mele-Mele), le Tunnel Perce-Mont (232, Akala) et un troisième morceau de la
/// Route 1 (230).
const USUM_ORDER: &[u16] = &[
    6, 8, 230, 18, 20, 22, 12, 200, 44, 38, 34, 36, 46, 48, 10, 40, 42, 14, 16, 30, 32, 50, 68, 78, 52, 86, 88, 54, 56, 82, 84, 232, 58, 90, 100, 60,
    62, 64, 76, 94, 92, 66, 72, 132, 192, 134, 106, 136, 108, 122, 110, 112, 124, 114, 126, 150, 116, 118, 128, 120, 138, 140, 142, 144, 172, 158,
    160, 162, 180, 184, 174, 176, 178, 146, 154, 164, 166, 168, 170, 156, 182,
];

/// Dresseur clé : rôle, libellé, nom français, lieu, identifiants et classe attendue.
struct LeaderDef {
    kind: LeaderKind,
    label: &'static str,
    name: &'static str,
    town: &'static str,
    /// (identifiant de dresseur, classe attendue) ; plusieurs selon le starter choisi.
    members: &'static [(u16, u16)],
}

const fn gym(name: &'static str, town: &'static str, members: &'static [(u16, u16)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Gym, label: "", name, town, members }
}

const fn elite(name: &'static str, members: &'static [(u16, u16)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Elite, label: "Conseil 4", name, town: "Ligue Pokémon", members }
}

const fn champion(name: &'static str, members: &'static [(u16, u16)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Champion, label: "Maître", name, town: "Ligue Pokémon", members }
}

const fn trial(label: &'static str, name: &'static str, town: &'static str, members: &'static [(u16, u16)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Gym, label, name, town, members }
}

/// X / Y (vérifié sur Y : noms, classes et équipes lus dans la ROM, texte 20 des classes).
const XY_LEADERS: &[LeaderDef] = &[
    gym("Violette", "Neuvartault", &[(6, 4)]),
    gym("Lino", "Relifac-le-Haut", &[(76, 39)]),
    gym("Cornélia", "Yantreizh", &[(21, 41)]),
    gym("Amaro", "Port Tempères", &[(22, 42)]),
    gym("Lem", "Illumis", &[(23, 44)]),
    gym("Valériane", "Romant-sous-Bois", &[(24, 45)]),
    gym("Astera", "Flusselles", &[(25, 40)]),
    gym("Urup", "Auffrac-les-Congères", &[(26, 43)]),
    elite("Malva", &[(269, 37)]),
    elite("Narcisse", &[(271, 38)]),
    elite("Thyméo", &[(187, 36)]),
    elite("Dracéna", &[(270, 35)]),
    champion("Dianthéa", &[(276, 53)]),
];

/// Rubis Oméga / Saphir Alpha (vérifié sur Rubis Oméga, texte 21 des classes).
const ORAS_LEADERS: &[LeaderDef] = &[
    gym("Roxanne", "Mérouville", &[(561, 200)]),
    gym("Bastien", "Myokara", &[(563, 201)]),
    gym("Voltère", "Lavandia", &[(567, 202)]),
    gym("Adriane", "Vermilava", &[(569, 203)]),
    gym("Norman", "Clémenti-Ville", &[(570, 204)]),
    gym("Alizée", "Cimetronelle", &[(571, 205)]),
    gym("Lévy & Tatia", "Algatia", &[(552, 206)]),
    gym("Marc", "Atalanopolis", &[(572, 207)]),
    elite("Damien", &[(553, 194)]),
    elite("Spectra", &[(554, 195)]),
    elite("Glacia", &[(555, 196)]),
    elite("Aragon", &[(556, 197)]),
    champion("Pierre Rochard", &[(557, 198)]),
];

/// Soleil / Lune (vérifié sur Lune, texte 106 des classes). Althéo et Euphorbe : un
/// dresseur par starter. Les épreuves des capitaines se terminent contre un Pokémon dominant
/// (rencontre fixe, pas un dresseur de la ROM) : seuls le combat contre Althéo et les Grands
/// Duels servent de paliers.
const SM_LEADERS: &[LeaderDef] = &[
    trial("Capitaine", "Althéo", "Mele-Mele", &[(52, 38), (215, 38), (216, 38)]),
    trial("Grand Duel 1", "Pectorius", "Mele-Mele", &[(23, 31)]),
    trial("Grand Duel 2", "Alyxia", "Akala", &[(90, 49)]),
    trial("Grand Duel 3", "Danh", "Ula-Ula", &[(154, 141)]),
    trial("Grand Duel 4", "Paulie", "Poni", &[(155, 51)]),
    elite("Pectorius", &[(152, 109)]),
    elite("Alyxia", &[(153, 110)]),
    elite("Margie", &[(149, 107)]),
    elite("Kahili", &[(156, 80)]),
    champion("Euphorbe", &[(129, 111), (413, 111), (414, 111)]),
];

/// Ultra-Soleil / Ultra-Lune (vérifié sur Ultra-Soleil, texte 111 des classes) : Molène
/// remplace Pectorius au Conseil 4, Tili défend le titre de Maître.
const USUM_LEADERS: &[LeaderDef] = &[
    trial("Capitaine", "Althéo", "Mele-Mele", &[(52, 38), (215, 38), (216, 38)]),
    trial("Grand Duel 1", "Pectorius", "Mele-Mele", &[(23, 31)]),
    trial("Grand Duel 2", "Alyxia", "Akala", &[(90, 49)]),
    trial("Grand Duel 3", "Danh", "Ula-Ula", &[(154, 50)]),
    trial("Grand Duel 4", "Paulie", "Poni", &[(497, 51)]),
    elite("Molène", &[(489, 191)]),
    elite("Alyxia", &[(153, 110)]),
    elite("Margie", &[(149, 107)]),
    elite("Kahili", &[(156, 80)]),
    champion("Tili", &[(494, 194), (495, 194), (496, 194)]),
];

fn leader_defs(game: Game) -> &'static [LeaderDef] {
    match game {
        Game::X | Game::Y => XY_LEADERS,
        Game::OmegaRuby | Game::AlphaSapphire => ORAS_LEADERS,
        Game::Sun | Game::Moon => SM_LEADERS,
        Game::UltraSun | Game::UltraMoon => USUM_LEADERS,
        _ => &[],
    }
}

/// Classe et équipe (espèce, niveau) d'un dresseur 3DS.
fn trainer_team(game: Game, trdata: &[u8], trpoke: &[u8]) -> Option<(u16, Vec<(u16, u16)>)> {
    match game.generation() {
        7 => {
            let count = *trdata.get(3)? as usize;
            let team = trpoke.get(..count * 0x20)?.as_chunks::<0x20>().0.iter().map(|p| (u16_at(p, 0x10), u16_at(p, 0x0E))).collect();
            Some((u16_at(trdata, 0), team))
        }
        _ => {
            let xy = matches!(game, Game::X | Game::Y);
            let oras = if xy { trainers::xy_trdata_as_oras(trdata) } else { trdata.to_vec() };
            let team = trainers::read_team(6, &oras, trpoke)?;
            Some((u16_at(&oras, 2), team.pokemon.iter().map(|p| (p.species, p.level)).collect()))
        }
    }
}

fn read_leaders(game: &CtrGameRom) -> Result<Vec<Leader>, RomError> {
    let l = game.layout;
    let trdata = game.garc(l.trainer_data)?;
    let trpoke = game.garc(l.trainer_pokemon)?;
    let classes = game.text_file(l.trainer_classes)?;
    let mut gym_n = 0;
    let mut out = Vec::new();
    for def in leader_defs(game.game) {
        let mut ace: Option<(u16, u8)> = None;
        let mut verified = true;
        let mut class_name = String::new();
        for &(id, expected) in def.members {
            let (Some(td), Some(tp)) = (trdata.file(id as usize), trpoke.file(id as usize)) else {
                verified = false;
                continue;
            };
            let Some((class, team)) = trainer_team(game.game, td, tp) else {
                verified = false;
                continue;
            };
            verified &= class == expected;
            if class_name.is_empty() {
                class_name = classes.get(class as usize).cloned().unwrap_or_default();
            }
            for (species, level) in team {
                let level = level.min(255) as u8;
                if ace.is_none_or(|(_, l)| level > l) {
                    ace = Some((species, level));
                }
            }
        }
        let label = match def.kind {
            LeaderKind::Gym if def.label.is_empty() => {
                gym_n += 1;
                format!("Arène {gym_n}")
            }
            _ => def.label.to_string(),
        };
        let (ace_species, ace_level) = ace.unwrap_or((0, 0));
        let trainer_ids = def.members.iter().map(|m| m.0).collect();
        out.push(Leader { kind: def.kind, label, name: def.name, town: def.town, trainer_ids, ace_species, ace_level, class_name, verified });
    }
    Ok(out)
}

/// Gen 7 : la sauvegarde ne compte pas des badges mais les îles terminées (tampons
/// « Grand Duel » de PKHeX `Misc7.Stamps`). Renvoie le nombre de dresseurs de type arène
/// battus après `islands` Grands Duels (capitaines compris), sous forme de masque de bits
/// comme les badges des autres jeux.
pub fn alola_badges(rom: &RomInfo, islands: u8) -> u8 {
    let islands = islands.count_ones() as usize;
    let gyms: Vec<&Leader> = rom.leaders.iter().filter(|l| l.kind == LeaderKind::Gym).collect();
    let beaten = match islands {
        0 => 0,
        n => gyms.iter().enumerate().filter(|(_, l)| l.label.starts_with("Grand Duel")).nth(n - 1).map_or(gyms.len(), |(i, _)| i + 1),
    };
    ((1u16 << beaten.min(8)) - 1) as u8
}

/// Starters proposés par le jeu : table des dons de `DllField.cro` (X / Y, ROSA ; positions
/// de l'UPR, vérifiées sur Y et Rubis Oméga Rev 2) ou entrée 0 de l'archive des dons (Gen 7).
fn read_starters(game: &CtrGameRom) -> Result<Vec<u16>, RomError> {
    let (data, table, stride) = match game.game {
        Game::X | Game::Y => (game.romfs().read("DllField.cro")?, 0xF805C, 0x18),
        Game::OmegaRuby | Game::AlphaSapphire => (game.romfs().read("DllField.cro")?, 0xF906C, 0x24),
        g => {
            let gifts = if matches!(g, Game::UltraSun | Game::UltraMoon) { "a/1/5/9" } else { "a/1/5/5" };
            (game.garc(gifts)?.file(0).unwrap_or_default().to_vec(), 0, 0x14)
        }
    };
    let count = game.layout.species_count;
    let starters: Vec<u16> = (0..3).filter_map(|i| data.get(table + i * stride..table + i * stride + 2).map(|b| u16_at(b, 0))).collect();
    // Autre révision du jeu : table ailleurs, on ne devine pas.
    if starters.len() != 3 || starters.iter().any(|&s| s == 0 || s > count) {
        return Err(RomError::Layout("table des starters inattendue".into()));
    }
    Ok(starters)
}

/// Espèce de base de chaque famille, d'après les évolutions de la ROM (Gen 6 : 8 × 6 octets,
/// espèce en +4 ; Gen 7 : 8 × 8 octets, espèce en +4).
fn read_families(game: &CtrGameRom) -> Result<Vec<u16>, RomError> {
    let garc = game.garc(game.layout.evolution)?;
    let count = game.layout.species_count as usize;
    let entry = if game.generation() == 7 { 8 } else { 6 };
    let mut parent = vec![0u16; count + 1];
    for species in 0..=count.min(garc.len().saturating_sub(1)) {
        let d = garc.file(species).unwrap_or_default();
        for e in d.chunks_exact(entry).take(8) {
            let (method, target) = (u16_at(e, 0), u16_at(e, 4) as usize);
            if method != 0 && target != 0 && target <= count && parent[target] == 0 && target != species {
                parent[target] = species as u16;
            }
        }
    }
    Ok((0..=count)
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
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mini_pack() {
        let mut d = b"WD\x02\x00".to_vec();
        for o in [16u32, 18, 21] {
            d.extend_from_slice(&o.to_le_bytes());
        }
        d.extend_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(mini_unpack(&d, b"WD"), vec![&[1u8, 2][..], &[3, 4, 5][..]]);
        assert!(mini_unpack(&d, b"EA").is_empty());
    }

    #[test]
    fn island_stamps() {
        let leader = |kind, label: &str| Leader {
            kind,
            label: label.to_string(),
            name: "",
            town: "",
            trainer_ids: Vec::new(),
            ace_species: 0,
            ace_level: 0,
            class_name: String::new(),
            verified: true,
        };
        let rom = RomInfo {
            game: Game::Moon,
            routes: Vec::new(),
            leaders: vec![
                leader(LeaderKind::Gym, "Capitaine"),
                leader(LeaderKind::Gym, "Grand Duel 1"),
                leader(LeaderKind::Gym, "Grand Duel 2"),
                leader(LeaderKind::Elite, "Conseil 4"),
            ],
            starters: Vec::new(),
            family: Vec::new(),
            seed: None,
            map_locations: Vec::new(),
        };
        assert_eq!(alola_badges(&rom, 0), 0);
        assert_eq!(alola_badges(&rom, 0b0001), 0b11); // Althéo et Pectorius battus
        assert_eq!(alola_badges(&rom, 0b0011), 0b111);
        assert_eq!(alola_badges(&rom, 0b1111), 0b111);
    }

    // ---------- ROM réelles (ignorées si absentes) ----------

    fn open(file: &str) -> Option<RomInfo> {
        let path = crate::test_rom_path(file);
        if !path.exists() {
            eprintln!("ROM absente, test ignoré : {}", path.display());
            return None;
        }
        Some(read_path(&path).unwrap())
    }

    fn route<'a>(info: &'a RomInfo, name: &str) -> &'a Route {
        info.routes.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("route « {name} » absente"))
    }

    fn leader<'a>(info: &'a RomInfo, name: &str) -> &'a Leader {
        info.leaders.iter().find(|l| l.name == name).unwrap()
    }

    /// Contrôles communs : identifiants = lieux de rencontre de PKHeX, dresseurs vérifiés,
    /// une espèce par route.
    fn check_common(info: &RomInfo) {
        let generation = info.game.generation();
        for r in &info.routes {
            for &id in &r.location_ids {
                // Liste de PKHeX tirée d'Ultra : « Côte Sauvage » de Soleil / Lune y devient « Plage d'Ula-Ula ».
                if id == 110 && matches!(info.game, Game::Sun | Game::Moon) {
                    continue;
                }
                let pkhex = dex::location_name(generation, id).unwrap_or_default();
                assert!(pkhex.starts_with(r.name.as_str()), "lieu n°{id} : {pkhex} ≠ {}", r.name);
            }
            let mut s: Vec<u16> = r.encounters.iter().map(|e| e.species).collect();
            let n = s.len();
            s.sort_unstable();
            s.dedup();
            assert_eq!(s.len(), n, "{}", r.name);
        }
        for l in &info.leaders {
            assert!(l.verified, "{} : classe inattendue ({})", l.name, l.class_name);
            assert!(l.ace_level > 0, "{}", l.name);
        }
        let mut orders: Vec<u32> = info.routes.iter().map(|r| r.order).collect();
        orders.dedup();
        assert_eq!(orders.len(), info.routes.len(), "ordre de l'histoire en double");
    }

    #[test]
    fn pokemon_y() {
        let Some(info) = open("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds") else { return };
        assert_eq!(info.game, Game::Y);
        check_common(&info);
        let names: Vec<&str> = info.routes.iter().take(5).map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Route 2", "Forêt de Neuvartault", "Route 3", "Route 22", "Route 4"]);
        assert_eq!(info.routes[0].location_ids, vec![12]);
        let r4 = route(&info, "Route 4");
        assert!(r4.encounters.iter().any(|e| e.species == 669 && e.methods.contains(&"Fleurs jaunes"))); // Flabébé
        assert!(route(&info, "Route Victoire").zones.len() > 5);
        let gyms: Vec<(&str, u8)> = info.leaders.iter().filter(|l| l.kind == LeaderKind::Gym).map(|l| (l.name, l.ace_level)).collect();
        assert_eq!(gyms[0], ("Violette", 12));
        assert_eq!(gyms[7], ("Urup", 59));
        assert_eq!(leader(&info, "Dianthéa").ace_level, 68);
        assert_eq!(leader(&info, "Violette").class_name, "Championne");
        assert_eq!(info.starters, vec![650, 653, 656]);
        assert_eq!((info.family_of(652), info.family_of(25)), (650, 172));
    }

    #[test]
    fn omega_ruby() {
        let Some(info) = open("Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2).3ds") else { return };
        check_common(&info);
        assert_eq!((info.routes[0].name.as_str(), info.routes[1].name.as_str()), ("Route 101", "Route 103"));
        assert!(route(&info, "Route 101").encounters.iter().any(|e| e.species == 263)); // Zigzaton
        assert!(route(&info, "Route 104").encounters.iter().any(|e| e.methods.contains(&"Horde")));
        let levels: Vec<u8> = info.leaders.iter().map(|l| l.ace_level).collect();
        assert_eq!(levels, vec![14, 16, 21, 28, 30, 35, 45, 46, 52, 53, 54, 55, 59]);
        assert_eq!(leader(&info, "Roxanne").town, "Mérouville");
        assert_eq!(info.starters, vec![252, 255, 258]);
        assert_eq!(info.family_of(254), 252);
    }

    #[test]
    fn moon() {
        let Some(info) = open("Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else { return };
        check_common(&info);
        let r1 = &info.routes[0];
        assert_eq!((r1.name.as_str(), r1.location_ids.as_slice()), ("Route 1", &[6, 8][..]));
        assert!(r1.encounters.iter().any(|e| e.species == 734 && e.methods.contains(&"Sauvage (jour)"))); // Manglouton
        assert!(r1.encounters.iter().any(|e| e.species == 731)); // Picassaut
        assert!(r1.encounters.iter().any(|e| e.methods.iter().any(|m| m.starts_with("Appel à l'aide"))));
        // Tables de remplissage (Picassaut niv. 2-3) écartées.
        assert!(!info.routes.iter().any(|r| r.name == "Ruines de l’Essor"));
        assert!(route(&info, "Grotte Verdoyante").encounters.iter().all(|e| e.min_level >= 8));
        let caps: Vec<(&str, &str, u8)> = info.leaders.iter().map(|l| (l.label.as_str(), l.name, l.ace_level)).collect();
        assert_eq!(caps[0], ("Capitaine", "Althéo", 10));
        assert_eq!(caps[1], ("Grand Duel 1", "Pectorius", 15));
        assert_eq!(caps[4], ("Grand Duel 4", "Paulie", 48));
        assert_eq!(caps[9], ("Maître", "Euphorbe", 58));
        assert_eq!(info.starters, vec![722, 725, 728]);
        assert_eq!((info.family_of(724), info.family_of(791)), (722, 789)); // Archéduc, Solgaleo
    }

    #[test]
    fn ultra_sun() {
        let Some(info) = open("Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else { return };
        check_common(&info);
        assert_eq!(info.routes[0].location_ids, vec![6, 8, 230]);
        assert!(route(&info, "Tunnel Perce-Mont").order < route(&info, "Route 8").order);
        let caps: Vec<(&str, u8)> = info.leaders.iter().map(|l| (l.name, l.ace_level)).collect();
        assert_eq!(caps[0], ("Althéo", 11));
        assert_eq!(caps[4], ("Paulie", 54));
        assert_eq!(caps[5], ("Molène", 57));
        assert_eq!(caps[9], ("Tili", 60));
        assert_eq!(info.starters, vec![722, 725, 728]);
    }

    /// Sauvegarde Soleil / Lune (PKHeX) liée à la ROM Lune : les Ultra-Chimères capturées
    /// tombent sur leurs routes, les îles terminées donnent le niveau maximum du Conseil 4.
    #[test]
    fn sun_moon_save_report() {
        let Some(info) = open("Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else { return };
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pkhex/sm_project_802.main")).unwrap();
        let session = crate::save::session::SaveSession::open(&bytes).unwrap();
        assert!(super::super::compatible(info.game, session.save.version()));
        assert_eq!(session.save.badges(), Some(0b1111));
        let r = super::super::report(&info, &session, &super::super::RunState::default()).unwrap();
        assert_eq!((r.stats.badges, r.stats.level_cap), (5, Some(55)));
        let caught: Vec<(&str, u16)> = r.routes.iter().filter_map(|rv| rv.capture.as_ref().map(|c| (rv.name.as_str(), c.species))).collect();
        assert!(caught.contains(&("Colline Dicarat", 800)), "{caught:?}"); // Necrozma
        assert!(caught.contains(&("Caverne Coda", 799)), "{caught:?}"); // Engloutyran
    }
}
