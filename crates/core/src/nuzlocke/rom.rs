//! Ce que le mode Nuzlocke lit dans la ROM (éventuellement randomisée par Kaleido) :
//! les zones de rencontre regroupées par lieu, les champions d'arène et le Conseil 4.
//!
//! ## Zones ↔ lieux de rencontre de la sauvegarde
//!
//! Chaque carte du jeu a un en-tête qui donne son fichier de rencontres sauvages et
//! l'identifiant du nom de lieu affiché. Vérifié sur les vraies ROMs :
//!
//! - **Platine** : table de 593 en-têtes de 24 octets dans l'ARM9 (0xE601C sur la
//!   ROM européenne / américaine ; retrouvée par recherche sinon). Rencontres = u16
//!   en +0x0E (0xFFFF = aucune), nom du lieu = u8 en +0x12, index dans le fichier de
//!   textes 433 (« Mystery Zone », « Twinleaf Town »…).
//! - **Noire / Blanche** : NARC `a/0/1/2`, un fichier de 427 en-têtes de 48 octets.
//!   Rencontres = u16 en +0x14 (0xFFFF = aucune), nom du lieu = u8 en +0x1A
//!   (l'octet suivant porte des drapeaux d'affichage), index dans le fichier de textes 89.
//!
//! Dans les deux jeux, le lieu de rencontre enregistré dans un Pokémon (PK4 / PK5)
//! est **ce même index** : les listes de lieux de PKHeX (`met4_00000`, `met5_00000`)
//! suivent ligne à ligne les fichiers de textes 433 et 89 (comparé sur Platine et Blanche).
//! Les zones sont donc regroupées par nom de lieu, et une route garde la liste de
//! tous les identifiants qui portent ce nom (certains noms existent en double en Gen 5).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::data::{encounters, evolutions, starters, trainers, DataPaths};
use crate::dex;
use crate::games::Game;
use crate::rom::{GameRom, RomError};

/// Une espèce rencontrable sur une route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Encounter {
    pub species: u16,
    pub min_level: u8,
    pub max_level: u8,
    /// Herbe, Surf, Canne…
    pub methods: Vec<&'static str>,
}

/// Une « route » au sens Nuzlocke : toutes les zones qui portent le même nom de lieu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Clé stable (le nom), utilisée pour les marques manuelles.
    pub key: String,
    pub name: String,
    /// Identifiants de lieu (= lieu de rencontre dans la sauvegarde).
    pub location_ids: Vec<u16>,
    /// Fichiers de rencontres de la ROM regroupés ici.
    pub zones: Vec<u16>,
    pub encounters: Vec<Encounter>,
    /// Position dans l'ordre de l'histoire (approximatif) ; les lieux hors liste viennent après.
    pub order: u32,
}

/// Rôle d'un dresseur important.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LeaderKind {
    Gym,
    Elite,
    Champion,
}

/// Champion d'arène, membre du Conseil 4 ou maître, lu dans la ROM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leader {
    pub kind: LeaderKind,
    /// « Arène 1 », « Conseil 4 », « Maître »…
    pub label: String,
    pub name: &'static str,
    pub town: &'static str,
    pub trainer_ids: Vec<u16>,
    /// Pokémon le plus fort de l'équipe (dans la ROM, donc après randomisation).
    pub ace_species: u16,
    pub ace_level: u8,
    /// Nom de la classe de dresseur lu dans la ROM (vérification).
    pub class_name: String,
    /// `false` si la classe lue ne correspond pas à celle attendue (ROM modifiée ?).
    pub verified: bool,
}

/// Tout ce que le mode Nuzlocke tire d'une ROM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomInfo {
    pub game: Game,
    pub routes: Vec<Route>,
    pub leaders: Vec<Leader>,
    /// Espèces de départ proposées par la ROM.
    pub starters: Vec<u16>,
    /// Espèce de base de chaque famille d'évolution (index = espèce).
    #[serde(skip)]
    pub family: Vec<u16>,
    /// Seed Kaleido si la ROM a été randomisée par Kaleido.
    pub seed: Option<u64>,
}

impl RomInfo {
    /// Espèce de base de la famille (l'espèce elle-même si inconnue).
    pub fn family_of(&self, species: u16) -> u16 {
        self.family.get(species as usize).copied().filter(|&f| f != 0).unwrap_or(species)
    }

    pub fn supports(game: Game) -> bool {
        matches!(game, Game::Platinum | Game::Black | Game::White)
    }
}

/// Fichier de textes des noms de lieux.
fn place_names_text(game: Game) -> Option<usize> {
    match game {
        Game::Platinum => Some(433),
        Game::Black | Game::White => Some(89),
        _ => None,
    }
}

/// Fichier de textes des classes de dresseurs.
fn trainer_classes_text(game: Game) -> Option<usize> {
    match game {
        Game::Platinum => Some(619),
        Game::Black | Game::White => Some(191),
        _ => None,
    }
}

/// Lit routes, champions et familles d'évolution.
pub fn read(game: &GameRom) -> Result<RomInfo, RomError> {
    let g = game.game;
    let (Some(paths), Some(names_text), true) = (DataPaths::for_game(g), place_names_text(g), RomInfo::supports(g)) else {
        return Err(RomError::Unsupported(format!("le mode Nuzlocke ne gère pas encore {}", g.name_fr())));
    };
    let place_names = game.text_file(names_text)?;
    let enc = game.narc(paths.encounters)?;
    let zones = match g {
        Game::Platinum => platinum_zones(game, enc.files.len(), place_names.len())?,
        _ => bw_zones(game)?,
    };
    let french = game.rom().header().region() == Some('F');
    let routes = group_routes(g, &zones, &enc.files, &place_names, french);

    let leaders = read_leaders(game, &paths)?;
    let starters = starters::read(game, paths.starters).map(|s| s.to_vec()).unwrap_or_default();
    let family = read_families(game, &paths).unwrap_or_default();
    Ok(RomInfo { game: g, routes, leaders, starters, family, seed: None })
}

const PT_HEADER_SIZE: usize = 24;
const PT_HEADER_COUNT: usize = 593;
const PT_HEADER_OFFSET: usize = 0xE601C;

/// Un en-tête de carte Platine plausible : rencontres et nom dans les bornes, musiques
/// de jour et de nuit dans la plage des séquences de terrain (1000 à 1300).
fn platinum_table_ok(arm9: &[u8], at: usize, enc_count: usize, name_count: usize) -> bool {
    let Some(table) = arm9.get(at..at + PT_HEADER_SIZE * PT_HEADER_COUNT) else { return false };
    table.as_chunks::<PT_HEADER_SIZE>().0.iter().all(|e| {
        let u16_at = |i: usize| u16::from_le_bytes([e[i], e[i + 1]]);
        let wild = u16_at(0x0E);
        (wild == 0xFFFF || (wild as usize) < enc_count)
            && (e[0x12] as usize) < name_count
            && (1000..=1300).contains(&u16_at(0x0A))
            && (1000..=1300).contains(&u16_at(0x0C))
    })
}

/// (fichier de rencontres, identifiant de lieu) pour chaque carte de Platine.
fn platinum_zones(game: &GameRom, enc_count: usize, name_count: usize) -> Result<Vec<(u16, u16)>, RomError> {
    let arm9 = game.rom().arm9_decompressed()?;
    let at = if platinum_table_ok(&arm9, PT_HEADER_OFFSET, enc_count, name_count) {
        PT_HEADER_OFFSET
    } else {
        // Autre langue : la table est ailleurs dans l'ARM9, on la cherche.
        (0..arm9.len().saturating_sub(PT_HEADER_SIZE * PT_HEADER_COUNT))
            .step_by(4)
            .find(|&at| platinum_table_ok(&arm9, at, enc_count, name_count))
            .ok_or_else(|| RomError::Layout("table des cartes de Platine introuvable".into()))?
    };
    Ok(arm9[at..at + PT_HEADER_SIZE * PT_HEADER_COUNT]
        .as_chunks::<PT_HEADER_SIZE>()
        .0
        .iter()
        .filter_map(|e| {
            let wild = u16::from_le_bytes([e[0x0E], e[0x0F]]);
            (wild != 0xFFFF).then_some((wild, e[0x12] as u16))
        })
        .collect())
}

const BW_HEADERS: &str = "a/0/1/2";
const BW_HEADER_SIZE: usize = 48;

fn bw_zones(game: &GameRom) -> Result<Vec<(u16, u16)>, RomError> {
    let narc = game.narc(BW_HEADERS)?;
    let d = narc.files.first().ok_or_else(|| RomError::Layout("en-têtes de cartes absents".into()))?;
    Ok(d.as_chunks::<BW_HEADER_SIZE>()
        .0
        .iter()
        .filter_map(|e| {
            let wild = u16::from_le_bytes([e[0x14], e[0x15]]);
            (wild != 0xFFFF).then_some((wild, e[0x1A] as u16))
        })
        .collect())
}

/// Nom français d'un lieu : texte de la ROM si elle est française, sinon liste de
/// PKHeX (mêmes identifiants), sinon texte de la ROM.
fn place_name(game: Game, id: u16, rom_names: &[String], french_rom: bool) -> String {
    let rom = || rom_names.get(id as usize).filter(|n| !n.trim().is_empty()).cloned();
    // Kaleido distingue les noms en double par un suffixe (« Route Victoire (D/P/Pt) ») : inutile ici.
    let pkhex = || {
        dex::location_name(game.generation(), id).map(|n| match n.rsplit_once(" (") {
            Some((base, _)) if n.ends_with(')') => base.to_string(),
            _ => n.to_string(),
        })
    };
    let name = if french_rom { rom().or_else(pkhex) } else { pkhex().or_else(rom) };
    name.unwrap_or_else(|| format!("Lieu n°{id}"))
}

/// Mode de rencontre d'un emplacement, d'après sa position dans le fichier.
fn method(game: Game, offset: usize) -> &'static str {
    if game == Game::Platinum {
        return match offset {
            ..0x64 => "Herbe",
            0x64..0xCC => "Spécial",
            _ => ["Surf", "Éclate-Roc", "Canne", "Super Canne", "Méga Canne"][((offset - 0xCC) / 0x2C).min(4)],
        };
    }
    match ((offset % 232).saturating_sub(8)) / 4 {
        0..12 => "Herbe",
        12..24 => "Herbe sombre",
        24..36 => "Phénomène",
        36..41 => "Surf",
        41..46 => "Surf (remous)",
        46..51 => "Pêche",
        _ => "Pêche (remous)",
    }
}

/// Regroupe les zones par nom de lieu et agrège leurs rencontres.
pub(crate) fn group_routes(game: Game, zones: &[(u16, u16)], files: &[Vec<u8>], rom_names: &[String], french_rom: bool) -> Vec<Route> {
    let order = story_order(game);
    let mut by_name: BTreeMap<String, Route> = BTreeMap::new();
    for &(file, location) in zones {
        let name = place_name(game, location, rom_names, french_rom);
        let route = by_name.entry(name.clone()).or_insert_with(|| Route {
            key: name.clone(),
            name,
            location_ids: Vec::new(),
            zones: Vec::new(),
            encounters: Vec::new(),
            order: u32::MAX,
        });
        if !route.location_ids.contains(&location) {
            route.location_ids.push(location);
        }
        if let Some(pos) = order.iter().position(|&l| l == location) {
            route.order = route.order.min(pos as u32);
        }
        if route.zones.contains(&file) {
            continue;
        }
        route.zones.push(file);
        let Some(data) = files.get(file as usize) else { continue };
        for slot in encounters::read(game.generation(), data) {
            let m = method(game, slot.offset);
            match route.encounters.iter_mut().find(|e| e.species == slot.species) {
                Some(e) => {
                    e.min_level = e.min_level.min(slot.min_level);
                    e.max_level = e.max_level.max(slot.max_level);
                    if !e.methods.contains(&m) {
                        e.methods.push(m);
                    }
                }
                None => route.encounters.push(Encounter {
                    species: slot.species,
                    min_level: slot.min_level,
                    max_level: slot.max_level,
                    methods: vec![m],
                }),
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

/// Lieux dans l'ordre (approximatif) de l'histoire, puis l'après-Ligue.
fn story_order(game: Game) -> &'static [u16] {
    match game {
        // Route 201, Lac Vérité, 202, 203, Entrée Charbourg, Mine, 204, Les Éoliennes, 205,
        // Forêt Vestigion, Vieux Château, Vestigion, 206, Grotte Revêche, 207, Mont Couronné,
        // 208, 209, Ruines Bonville, 210, 215, 214, Rive Courage, Lac Courage, 213, Verchamps,
        // Grand Marais, Jardin Trophée, 212, 211, Célestia, 218, Joliberges, Île de Fer, 216,
        // 217, Rive Savoir, Lac Savoir, Forge Fuego, Chemin Rocheux, Grotte Mania, Tunnel Mania,
        // 222, Rivamar, 223, Route Victoire, Ligue, puis l'après-Ligue.
        Game::Platinum => &[
            16, 76, 17, 18, 59, 46, 19, 47, 20, 48, 70, 9, 21, 65, 22, 50, 23, 24, 53, 25, 30, 29, 73, 77, 28, 11, 52, 68, 27, 26, 5, 33, 7,
            69, 31, 32, 74, 78, 49, 57, 66, 67, 37, 13, 38, 54, 15, 62, 61, 64, 34, 35, 36, 83, 39, 40, 41, 42, 84, 43, 44, 45, 1,
        ],
        // Route 1, 2, Ogoesse, Vestiges du Rêve, Route 3, Grotte Parsemille, Forêt d'Empoigne,
        // Route 4, Désert Délassant, Château Enfoui, Route 5, Pont Yoneuve, Port Yoneuve, Hangar
        // Frigorifique, Route 6, Grotte Électrolithe, Route 7, Tour des Cieux, Antre
        // d'Entraînement, Mont Foré, Flocombe, Tourbière, Route 8, Tour Dragospire, Route 9,
        // Pont de l'Inconnu, Route 10, Route Victoire, Salle Épreuve, puis l'après-Ligue.
        Game::Black | Game::White => &[
            14, 15, 6, 32, 16, 54, 33, 17, 34, 35, 18, 65, 10, 36, 19, 37, 20, 56, 59, 38, 12, 57, 21, 39, 22, 68, 23, 40, 73, 61, 24, 67, 25, 26,
            27, 28, 29, 30, 31, 42, 71, 53, 70, 72, 63, 74,
        ],
        _ => &[],
    }
}

/// Dresseur clé : rôle, nom français, ville, identifiants et classe attendue.
struct LeaderDef {
    kind: LeaderKind,
    name: &'static str,
    town: &'static str,
    /// (identifiant de dresseur, classe attendue).
    members: &'static [(u16, u8)],
}

const fn gym(name: &'static str, town: &'static str, members: &'static [(u16, u8)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Gym, name, town, members }
}

const fn elite(name: &'static str, members: &'static [(u16, u8)]) -> LeaderDef {
    LeaderDef { kind: LeaderKind::Elite, name, town: "Ligue Pokémon", members }
}

/// Champions d'arène dans l'ordre de l'histoire, Conseil 4 puis maître / combat final.
/// Identifiants vérifiés dans les fichiers trdata/trpoke des ROMs (classes et équipes) :
/// Platine (classes « Leader », « Elite Four », « Champion ») et Blanche (« Champion »,
/// « Conseil 4 »). Les noms de dresseurs Gen 4 / 5 sont compressés dans la ROM, d'où les
/// noms français ci-dessous.
const PLATINUM_LEADERS: &[LeaderDef] = &[
    gym("Pierrick", "Charbourg", &[(246, 62)]),
    gym("Flo", "Vestigion", &[(315, 74)]),
    gym("Kiméra", "Unionpolis", &[(318, 77)]),
    gym("Mélina", "Voilaroc", &[(317, 76)]),
    gym("Lovis", "Verchamps", &[(316, 75)]),
    gym("Charles", "Joliberges", &[(250, 64)]),
    gym("Gladys", "Frimapic", &[(319, 78)]),
    gym("Tanguy", "Rivamar", &[(320, 79)]),
    elite("Aaron", &[(261, 65)]),
    elite("Terry", &[(262, 66)]),
    elite("Adrien", &[(263, 67)]),
    elite("Lucio", &[(264, 68)]),
    LeaderDef { kind: LeaderKind::Champion, name: "Cynthia", town: "Ligue Pokémon", members: &[(267, 69)] },
];

/// Noire / Blanche. Arène 1 : l'un des trois frères selon le starter (même niveau) ;
/// arène 8 : Watson (Noire) ou Iris (Blanche), équipes identiques. Après le Conseil 4,
/// le combat final est N puis Ghetis (le maître Goyah est facultatif).
const BW_LEADERS: &[LeaderDef] = &[
    gym("Armando / Rachid / Noa", "Ogoesse", &[(11, 10), (12, 11), (13, 12)]),
    gym("Aloé", "Maillard", &[(21, 19)]),
    gym("Artie", "Volucité", &[(22, 20)]),
    gym("Inezia", "Méanville", &[(23, 21)]),
    gym("Bardane", "Port Yoneuve", &[(24, 22)]),
    gym("Carolina", "Parsemille", &[(25, 23)]),
    gym("Zhu", "Flocombe", &[(131, 54)]),
    gym("Watson / Iris", "Janusia", &[(132, 55), (133, 56)]),
    elite("Anis", &[(228, 78)]),
    elite("Kumaï", &[(229, 79)]),
    elite("Pieris", &[(230, 80)]),
    elite("Percila", &[(231, 81)]),
    LeaderDef { kind: LeaderKind::Champion, name: "N", town: "Château de N", members: &[(586, 101), (587, 101)] },
    LeaderDef { kind: LeaderKind::Champion, name: "Ghetis", town: "Château de N", members: &[(232, 82)] },
];

fn leader_defs(game: Game) -> &'static [LeaderDef] {
    match game {
        Game::Platinum => PLATINUM_LEADERS,
        Game::Black | Game::White => BW_LEADERS,
        _ => &[],
    }
}

fn read_leaders(game: &GameRom, paths: &DataPaths) -> Result<Vec<Leader>, RomError> {
    let trdata = game.narc(paths.trainer_data)?;
    let trpoke = game.narc(paths.trainer_pokemon)?;
    let classes = trainer_classes_text(game.game).map(|t| game.text_file(t)).transpose()?.unwrap_or_default();
    let gen = game.generation();
    let mut gym_n = 0;
    let mut out = Vec::new();
    for def in leader_defs(game.game) {
        let mut ace: Option<(u16, u8)> = None;
        let mut verified = true;
        let mut class_name = String::new();
        // N : en Noire il utilise Zekrom (587), en Blanche Reshiram (586).
        let mut members = def.members.to_vec();
        if game.game == Game::Black && def.name == "N" {
            members.reverse();
        }
        let ids: Vec<u16> = members.iter().map(|m| m.0).collect();
        for &(id, expected) in &members {
            let (Some(td), Some(tp)) = (trdata.files.get(id as usize), trpoke.files.get(id as usize)) else {
                verified = false;
                continue;
            };
            let class = td.get(1).copied().unwrap_or(0);
            verified &= class == expected;
            if class_name.is_empty() {
                class_name = classes.get(class as usize).cloned().unwrap_or_default();
            }
            if let Some(team) = trainers::read_team(gen, td, tp) {
                for p in &team.pokemon {
                    let level = p.level.min(255) as u8;
                    if ace.is_none_or(|(_, l)| level > l) {
                        ace = Some((p.species, level));
                    }
                }
            }
        }
        let label = match def.kind {
            LeaderKind::Gym => {
                gym_n += 1;
                format!("Arène {gym_n}")
            }
            LeaderKind::Elite => "Conseil 4".to_string(),
            LeaderKind::Champion if game.game == Game::Platinum => "Maître".to_string(),
            LeaderKind::Champion => "Combat final".to_string(),
        };
        let (ace_species, ace_level) = ace.unwrap_or((0, 0));
        out.push(Leader { kind: def.kind, label, name: def.name, town: def.town, trainer_ids: ids, ace_species, ace_level, class_name, verified });
    }
    Ok(out)
}

/// Espèce de base de chaque famille, d'après les évolutions de la ROM.
fn read_families(game: &GameRom, paths: &DataPaths) -> Result<Vec<u16>, RomError> {
    let narc = game.narc(paths.evolutions)?;
    let count = game.layout.species_count as usize;
    let mut parent = vec![0u16; count + 1];
    for (species, data) in narc.files.iter().enumerate().take(count + 1) {
        for e in evolutions::read(data) {
            let t = e.target as usize;
            if t <= count && parent[t] == 0 && t != species {
                parent[t] = species as u16;
            }
        }
    }
    Ok((0..=count)
        .map(|s| {
            let mut cur = s;
            // Au plus 3 stades : la limite évite une boucle sur une ROM mal formée.
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

/// Seed Kaleido inscrite dans une ROM DS (sans lire la ROM en entier).
pub fn kaleido_seed(path: &std::path::Path) -> Option<u64> {
    use std::io::Read;
    let mut f = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut header = vec![0u8; kaleido_formats::nds::HEADER_SIZE];
    f.read_exact(&mut header).ok()?;
    let json = kaleido_formats::nds::read_signature(&mut f, &header).ok()??;
    serde_json::from_slice::<crate::randomizer::KaleidoTag>(&json).ok().map(|t| t.seed)
}
