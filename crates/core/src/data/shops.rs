//! Boutiques (Gen 4 Diamant / Perle / Platine / HGSS, Gen 5 Noire/Blanche).
//!
//! Portage de `getShops` / `setShops` de l'Universal Pokémon Randomizer :
//!
//! - Platine (ARM9) : la boutique « progressive » des Centres Pokémon (`u16` objet,
//!   `u16` badges requis ; taille sur un octet, pointeur vers la liste), puis une table
//!   de 20 pointeurs vers les boutiques secondaires (listes de `u16` terminées par `FFFF`) ;
//! - Noire/Blanche (overlay 21) : 26 boutiques, une table de pointeurs et une table de
//!   tailles (un octet par boutique), dont les positions dépendent de la langue ;
//! - Noire 2 / Blanche 2 (archive `a/2/8/2`) : un fichier par boutique, liste de `u16`
//!   (32 boutiques ; vérifié sur Noire 2, IREF).
//! - Diamant / Perle et HeartGold / SoulSilver (ARM9) : comme UPR-ZX (`getShopItems`),
//!   listes de `u16` terminées par `FFFF`, à la suite les unes des autres juste après le
//!   motif `ShopDataPrefix` (dépend de la langue). Les boutiques de décorations et de
//!   sceaux (`SkipShops`) sont parcourues sans être lues ; celles de CT sont lues mais
//!   jamais modifiées. Vérifié sur Diamant (ADAF) et SoulSilver (IPGF).
//!
//! Contrairement à l'UPR, les boutiques gardent leur taille : les objets sont réécrits
//! sur place, sans déplacer les listes.

use serde::Serialize;

use crate::games::Game;
use crate::rom::{GameRom, RomError};

/// Adresse de chargement de l'ARM9 dans tous les jeux Pokémon DS.
const ARM9_RAM: u32 = 0x0200_0000;
const BW_SHOP_OVERLAY: u32 = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShopKind {
    /// Comptoir principal des Centres Pokémon (Poké Balls, Potions…) : jamais modifié.
    Regular,
    /// Boutique de CT : garde ses CT.
    Tm,
    /// Boutique secondaire, randomisable.
    Special,
}

impl ShopKind {
    pub fn name_fr(self) -> &'static str {
        match self {
            ShopKind::Regular => "principale",
            ShopKind::Tm => "CT",
            ShopKind::Special => "secondaire",
        }
    }
}

/// Où sont stockés les objets d'une boutique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopSource {
    Arm9,
    Overlay(u32),
    /// Fichier n° `.0` de l'archive des boutiques (Noire 2 / Blanche 2).
    NarcFile(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shop {
    pub index: usize,
    pub name: &'static str,
    pub kind: ShopKind,
    /// Boutique accessible pendant l'aventure principale (avant la Ligue).
    pub main_game: bool,
    pub items: Vec<u16>,
    pub source: ShopSource,
    /// Position de chaque identifiant d'objet.
    pub positions: Vec<usize>,
}

const PT_SHOP_NAMES: [&str; 21] = [
    "Boutique principale (selon les badges)",
    "Littorella (comptoir secondaire)",
    "Charbourg (comptoir secondaire)",
    "Floraville (comptoir secondaire)",
    "Vestigion (comptoir secondaire)",
    "Vestigion (herboriste)",
    "Unionpolis (comptoir secondaire)",
    "Bonville (comptoir secondaire)",
    "Verchamps (comptoir secondaire)",
    "Centre Commercial de Voilaroc 1er (droite)",
    "Centre Commercial de Voilaroc 1er (gauche)",
    "Centre Commercial de Voilaroc 2e (haut)",
    "Centre Commercial de Voilaroc 2e (milieu)",
    "Centre Commercial de Voilaroc 3e (haut)",
    "Centre Commercial de Voilaroc 3e (bas)",
    "Célestia (comptoir secondaire)",
    "Frimapic (comptoir secondaire)",
    "Joliberges (comptoir secondaire)",
    "Rivamar (comptoir secondaire)",
    "Ligue Pokémon (comptoir secondaire)",
    "Centre Commercial de Voilaroc sous-sol (Baies)",
];
/// Index (boutique progressive = 0) des boutiques de CT de Platine (`TMShops` de l'UPR).
const PT_TM_SHOPS: [usize; 2] = [13, 14];

const BW_SHOP_NAMES: [&str; 26] = [
    "Boutique principale (0 badge)",
    "Boutique principale (2 badges)",
    "Boutique principale (3 badges)",
    "Boutique principale (5 badges)",
    "Boutique principale (7 badges)",
    "Boutique principale (8 badges)",
    "Arabelle (comptoir secondaire)",
    "Ogoesse (comptoir secondaire)",
    "Maillard (comptoir secondaire)",
    "Volucité (comptoir secondaire)",
    "Méanville (CT)",
    "Port Yoneuve (comptoir secondaire)",
    "Parsemille (CT)",
    "Flocombe (CT)",
    "Janusia (comptoir secondaire)",
    "Ligue Pokémon (comptoir secondaire)",
    "Mozheim (comptoir secondaire)",
    "Renouet (comptoir secondaire)",
    "Ville Noire / Forêt Blanche (comptoir secondaire)",
    "Maillard / Centre commercial (objets X)",
    "Port Yoneuve (herboriste)",
    "Port Yoneuve (encens)",
    "Centre commercial Route 9 (1)",
    "Centre commercial Route 9 (CT)",
    "Centre commercial Route 9 (2)",
    "Centre commercial Route 9 (3, gauche)",
];
const BW_TM_SHOPS: [usize; 4] = [10, 12, 13, 23];
const BW_REGULAR_SHOPS: [usize; 6] = [0, 1, 2, 3, 4, 5];
const BW_MAIN_GAME_SHOPS: [usize; 12] = [3, 5, 6, 8, 9, 12, 14, 17, 18, 19, 21, 22];

/// Noire 2 / Blanche 2 : une boutique par fichier de `a/2/8/2` (liste de `u16`),
/// 32 boutiques utilisées (`ShopCount`, `bw2ShopNames` de l'UPR).
const B2W2_SHOPS: &str = "a/2/8/2";
const B2W2_SHOP_NAMES: [&str; 32] = [
    "Boutique principale (0 badge)",
    "Boutique principale (1 badge)",
    "Boutique principale (3 badges)",
    "Boutique principale (5 badges)",
    "Boutique principale (7 badges)",
    "Boutique principale (8 badges)",
    "Arabelle (comptoir secondaire)",
    "Ogoesse (CT)",
    "Maillard (comptoir secondaire)",
    "Volucité (comptoir secondaire)",
    "Méanville (CT)",
    "Port Yoneuve (comptoir secondaire)",
    "Parsemille (CT)",
    "Flocombe (comptoir secondaire)",
    "Janusia (comptoir secondaire)",
    "Route Victoire (comptoir secondaire)",
    "Ligue Pokémon (comptoir secondaire)",
    "Entrelasque (CT)",
    "Vaguelone (comptoir secondaire)",
    "Ville Noire / Forêt Blanche (comptoir secondaire)",
    "Maillard / Centre commercial (objets X)",
    "Port Yoneuve (herboriste)",
    "Port Yoneuve (encens)",
    "Centre commercial Route 9 (1)",
    "Centre commercial Route 9 (CT)",
    "Centre commercial Route 9 (2)",
    "Centre commercial Route 9 (3, gauche)",
    "Pavonnay (comptoir secondaire)",
    "Ondes-sur-Mer (comptoir secondaire)",
    "Papeloa (comptoir secondaire)",
    "Amaillide (comptoir secondaire)",
    "Arpentières (comptoir secondaire)",
];
const B2W2_TM_SHOPS: [usize; 5] = [7, 10, 12, 17, 24];
const B2W2_REGULAR_SHOPS: [usize; 6] = [0, 1, 2, 3, 4, 5];
const B2W2_MAIN_GAME_SHOPS: [usize; 17] = [9, 11, 14, 15, 16, 18, 20, 21, 22, 23, 25, 26, 27, 28, 29, 30, 31];

/// Boutique d'une liste « à la suite » (Diamant / Perle, HGSS) : nom, nature
/// (`None` = boutique ignorée : décorations, sceaux…), aventure principale.
type SeqShop = (&'static str, Option<ShopKind>, bool);

const SP: Option<ShopKind> = Some(ShopKind::Special);
const TM: Option<ShopKind> = Some(ShopKind::Tm);

/// Diamant / Perle : `dpShopNames`, `SkipShops` et `MainGameShops` d'UPR-ZX (28 boutiques lues).
const DP_SHOPS: [SeqShop; 28] = [
    ("Rivamar (comptoir secondaire)", SP, true),
    ("Féli-Cité (comptoir secondaire)", SP, true),
    ("Floraville (comptoir secondaire)", SP, true),
    ("Charbourg (comptoir secondaire)", SP, true),
    ("Vestigion (comptoir secondaire)", SP, true),
    ("Vestigion (herboriste)", SP, true),
    ("Frimapic (comptoir secondaire)", SP, true),
    ("Bonville (comptoir secondaire)", SP, true),
    ("Verchamps (comptoir secondaire)", SP, true),
    ("Célestia (comptoir secondaire)", SP, true),
    ("Unionpolis (comptoir secondaire)", SP, true),
    ("Joliberges (comptoir secondaire)", SP, true),
    ("Centre Commercial de Voilaroc (décorations 1)", None, true),
    ("Centre Commercial de Voilaroc (décorations 2)", None, true),
    ("Centre Commercial de Voilaroc (vitamines)", SP, true),
    ("Centre Commercial de Voilaroc (CT 1)", TM, true),
    ("Marché de Rivamar (sceaux 1)", None, false),
    ("Marché de Rivamar (sceaux 2)", None, false),
    ("Marché de Rivamar (sceaux 3)", None, false),
    ("Marché de Rivamar (sceaux 4)", None, false),
    ("Centre Commercial de Voilaroc (CT 2)", TM, true),
    ("Marché de Rivamar (sceaux 5)", None, false),
    ("Marché de Rivamar (sceaux 6)", None, false),
    ("Marché de Rivamar (sceaux 7)", None, false),
    ("Ligue Pokémon (comptoir secondaire)", SP, true),
    ("Centre Commercial de Voilaroc (objets X)", SP, true),
    ("Centre Commercial de Voilaroc (soins)", SP, true),
    ("Centre Commercial de Voilaroc (Poké Balls…)", SP, true),
];

/// HeartGold / SoulSilver : `hgssShopNames`, `SkipShops` et `MainGameShops` d'UPR-ZX (40 boutiques lues).
const HGSS_SHOPS: [SeqShop; 40] = [
    ("Ville Griotte (comptoir secondaire)", SP, true),
    ("Azuria (comptoir secondaire)", SP, false),
    ("Rosalia (comptoir secondaire)", SP, true),
    ("Centre Commercial de Céladopole (lettres)", SP, false),
    ("Safrania (comptoir secondaire)", SP, false),
    ("Mauville (comptoir secondaire)", SP, true),
    ("Ebenelle (comptoir secondaire)", SP, true),
    ("Oliville (comptoir secondaire)", SP, true),
    ("Parmanie (comptoir secondaire)", SP, false),
    ("Lavanville (comptoir secondaire)", SP, false),
    ("Argenta (comptoir secondaire)", SP, false),
    ("Jadielle (comptoir secondaire)", SP, false),
    ("Écorcia (comptoir secondaire)", SP, true),
    ("Acajou (avant la planque)", SP, false),
    ("Entrée du Parc Safari (sud-ouest)", SP, true),
    ("Doublonville (herboriste)", SP, false),
    ("Irisia (pharmacie)", SP, true),
    ("Décorations 1", None, false),
    ("Décorations 2", None, false),
    ("Centre Commercial de Doublonville (vitamines)", SP, true),
    ("Centre Commercial de Céladopole (vitamines)", SP, false),
    ("Place du Mont Sélénite", SP, false),
    ("Sceaux 1", None, false),
    ("Sceaux 2", None, false),
    ("Sceaux 3", None, false),
    ("Sceaux 4", None, false),
    ("Sceaux 5", None, false),
    ("Sceaux 6", None, false),
    ("Boutique inutilisée", None, false),
    ("Sceaux 7", None, false),
    ("Dôme Pokéathlon (cartes de données 25-27)", None, false),
    ("Centre Commercial de Doublonville (objets X)", SP, true),
    ("Centre Commercial de Céladopole (objets X)", SP, false),
    ("Acajou (après la planque)", SP, true),
    ("Centre Commercial de Doublonville (soins)", SP, true),
    ("Centre Commercial de Céladopole (soins)", SP, false),
    ("Centre Commercial de Doublonville (Poké Balls…)", SP, true),
    ("Centre Commercial de Doublonville (CT)", TM, false),
    ("Centre Commercial de Céladopole (Poké Balls…)", SP, false),
    ("Céladopole (CT)", TM, false),
];

/// `ShopDataPrefix` d'UPR-ZX selon le code du jeu.
fn sequential_prefix(code: &str) -> Option<[u8; 16]> {
    let hex = match code {
        "ADAE" | "APAE" => "391104027511040285AF0302A5AF0302",
        "ADAF" | "APAF" | "ADAD" | "APAD" | "ADAS" | "APAS" | "ADAI" | "APAI" => "BD110402CDAF0302EDAF0302DD110402",
        "ADAJ" | "APAJ" => "F11A040249BD030219BD0302FDBC0302",
        "ADAK" | "APAK" => "E1150402F1B3030211B4030201160402",
        "IPKE" | "IPGE" | "IPKF" | "IPGF" | "IPKD" | "IPGD" | "IPGS" | "IPKI" | "IPGI" => "298E0402618E0402998E0402B98E0402",
        "IPKS" => "218E0402598E0402918E0402B18E0402",
        "IPKJ" | "IPGJ" => "E57C040235770402CD1C200251320402",
        "IPKK" => "858F04028D8F0402958F0402B58F0402",
        "IPGK" => "7D8F0402858F04028D8F0402AD8F0402",
        _ => return None,
    };
    let mut out = [0u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[derive(Debug, Clone, Copy)]
enum Layout {
    Platinum {
        size: usize,
        pointer: usize,
        special_table: usize,
        special_count: usize,
    },
    BlackWhite {
        pointers: usize,
        sizes: usize,
        count: usize,
    },
    /// Listes à la suite après un motif de l'ARM9 (Diamant / Perle, HGSS).
    Sequential {
        prefix: [u8; 16],
        shops: &'static [SeqShop],
    },
    Black2White2,
}

/// Emplacements repris de `gen4_offsets.ini` / `gen5_offsets.ini` de l'UPR.
fn layout(game: &GameRom) -> Option<Layout> {
    let code = game.rom().header().game_code.as_str();
    let pt = |size, pointer, special_table| Layout::Platinum { size, pointer, special_table, special_count: 20 };
    let bw = |pointers, sizes| Layout::BlackWhite { pointers, sizes, count: 26 };
    Some(match (game.game, code) {
        (Game::Platinum, "CPUE") => pt(0x46B74, 0x46B94, 0x46BF0),
        (Game::Platinum, "CPUD" | "CPUF" | "CPUS" | "CPUI") => pt(0x46C18, 0x46C38, 0x46C94),
        (Game::Platinum, "CPUJ") => pt(0x466E0, 0x46700, 0x4675C),
        (Game::Platinum, "CPUK") => pt(0x47068, 0x47088, 0x470E4),
        (Game::Black, "IRBO") => bw(0x562A8, 0x516DE),
        (Game::Black, "IRBF" | "IRBD") => bw(0x562A8, 0x516D6),
        (Game::Black, "IRBS") => bw(0x562A8, 0x516E2),
        (Game::Black, "IRBI") => bw(0x56288, 0x516CA),
        (Game::Black, "IRBJ") => bw(0x55FA8, 0x513EA),
        (Game::Black, "IRBK") => bw(0x56288, 0x516CE),
        (Game::White, "IRAO") => bw(0x562A8, 0x516D6),
        (Game::White, "IRAF" | "IRAD") => bw(0x56288, 0x516CE),
        (Game::White, "IRAS") => bw(0x562A8, 0x516DA),
        (Game::White, "IRAI") => bw(0x56288, 0x516C2),
        (Game::White, "IRAJ") => bw(0x55FA8, 0x513E2),
        (Game::White, "IRAK") => bw(0x56288, 0x516C6),
        // Archive des boutiques : même emplacement dans toutes les langues (UPR `File<ShopItems>`).
        (Game::Black2 | Game::White2, _) => Layout::Black2White2,
        (Game::Diamond | Game::Pearl, _) => Layout::Sequential { prefix: sequential_prefix(code)?, shops: &DP_SHOPS },
        (Game::HeartGold | Game::SoulSilver, _) => Layout::Sequential { prefix: sequential_prefix(code)?, shops: &HGSS_SHOPS },
        _ => return None,
    })
}

fn bad() -> RomError {
    RomError::Layout("boutiques introuvables (ROM modifiée ?)".into())
}

fn rd16(d: &[u8], at: usize) -> Result<u16, RomError> {
    Ok(u16::from_le_bytes(d.get(at..at + 2).ok_or_else(bad)?.try_into().map_err(|_| bad())?))
}

fn rd32(d: &[u8], at: usize) -> Result<u32, RomError> {
    Ok(u32::from_le_bytes(d.get(at..at + 4).ok_or_else(bad)?.try_into().map_err(|_| bad())?))
}

/// Pointeur mémoire → position dans un bloc chargé à `base`.
fn resolve(pointer: u32, base: u32, len: usize) -> Result<usize, RomError> {
    pointer.checked_sub(base).map(|p| p as usize).filter(|&p| p < len).ok_or_else(bad)
}

/// Lit toutes les boutiques du jeu.
pub fn read(game: &GameRom) -> Result<Vec<Shop>, RomError> {
    let layout = layout(game).ok_or_else(super::field_items::unsupported)?;
    let shops = match layout {
        Layout::Platinum { size, pointer, special_table, special_count } => {
            let arm9 = game.rom().arm9_decompressed()?;
            let mut shops = Vec::new();
            let count = *arm9.get(size).ok_or_else(bad)? as usize;
            let start = resolve(rd32(&arm9, pointer)?, ARM9_RAM, arm9.len())?;
            let positions: Vec<usize> = (0..count).map(|i| start + i * 4).collect();
            let items = positions.iter().map(|&p| rd16(&arm9, p)).collect::<Result<Vec<_>, _>>()?;
            shops.push(Shop {
                index: 0,
                name: PT_SHOP_NAMES[0],
                kind: ShopKind::Regular,
                main_game: true,
                items,
                source: ShopSource::Arm9,
                positions,
            });
            let table = resolve(rd32(&arm9, special_table)?, ARM9_RAM, arm9.len())?;
            for i in 0..special_count {
                let mut at = resolve(rd32(&arm9, table + 4 * i)?, ARM9_RAM, arm9.len())?;
                let (mut items, mut positions) = (Vec::new(), Vec::new());
                loop {
                    let id = rd16(&arm9, at)?;
                    if id == 0xFFFF {
                        break;
                    }
                    if items.len() > 64 {
                        return Err(bad());
                    }
                    items.push(id);
                    positions.push(at);
                    at += 2;
                }
                let index = i + 1;
                let kind = if PT_TM_SHOPS.contains(&index) { ShopKind::Tm } else { ShopKind::Special };
                shops.push(Shop { index, name: PT_SHOP_NAMES[index], kind, main_game: true, items, source: ShopSource::Arm9, positions });
            }
            shops
        }
        Layout::BlackWhite { pointers, sizes, count } => {
            let ovl = game.rom().overlay(BW_SHOP_OVERLAY)?;
            let base = game.rom().overlays().iter().find(|o| o.id == BW_SHOP_OVERLAY).ok_or_else(bad)?.ram_address;
            let mut shops = Vec::new();
            for (i, &name) in BW_SHOP_NAMES.iter().enumerate().take(count) {
                let size = *ovl.get(sizes + i).ok_or_else(bad)? as usize;
                let start = resolve(rd32(&ovl, pointers + 4 * i)?, base, ovl.len())?;
                let positions: Vec<usize> = (0..size).map(|j| start + j * 2).collect();
                let items = positions.iter().map(|&p| rd16(&ovl, p)).collect::<Result<Vec<_>, _>>()?;
                let kind = if BW_TM_SHOPS.contains(&i) {
                    ShopKind::Tm
                } else if BW_REGULAR_SHOPS.contains(&i) {
                    ShopKind::Regular
                } else {
                    ShopKind::Special
                };
                shops.push(Shop {
                    index: i,
                    name,
                    kind,
                    main_game: BW_MAIN_GAME_SHOPS.contains(&i),
                    items,
                    source: ShopSource::Overlay(BW_SHOP_OVERLAY),
                    positions,
                });
            }
            shops
        }
        Layout::Black2White2 => {
            let narc = game.narc(B2W2_SHOPS)?;
            let mut shops = Vec::new();
            for (i, &name) in B2W2_SHOP_NAMES.iter().enumerate() {
                let file = narc.files.get(i).ok_or_else(bad)?;
                let positions: Vec<usize> = (0..file.len() / 2).map(|j| j * 2).collect();
                let items = positions.iter().map(|&p| rd16(file, p)).collect::<Result<Vec<_>, _>>()?;
                let kind = if B2W2_TM_SHOPS.contains(&i) {
                    ShopKind::Tm
                } else if B2W2_REGULAR_SHOPS.contains(&i) {
                    ShopKind::Regular
                } else {
                    ShopKind::Special
                };
                shops.push(Shop {
                    index: i,
                    name,
                    kind,
                    main_game: B2W2_MAIN_GAME_SHOPS.contains(&i),
                    items,
                    source: ShopSource::NarcFile(i),
                    positions,
                });
            }
            shops
        }
        Layout::Sequential { prefix, shops: defs } => {
            let arm9 = game.rom().arm9_decompressed()?;
            let mut hits = arm9.windows(prefix.len()).enumerate().filter(|(_, w)| *w == prefix).map(|(i, _)| i);
            let (Some(found), None) = (hits.next(), hits.next()) else { return Err(bad()) };
            let mut at = found + prefix.len();
            let mut shops = Vec::new();
            for (i, &(name, kind, main_game)) in defs.iter().enumerate() {
                let (mut items, mut positions) = (Vec::new(), Vec::new());
                loop {
                    let id = rd16(&arm9, at)?;
                    if id == 0xFFFF {
                        break;
                    }
                    if positions.len() > 64 {
                        return Err(bad());
                    }
                    // Les 0 sont ignorés (comme l'UPR) et restent en place.
                    if id != 0 {
                        items.push(id);
                        positions.push(at);
                    }
                    at += 2;
                }
                at += 2;
                if let Some(kind) = kind {
                    shops.push(Shop { index: i, name, kind, main_game, items, source: ShopSource::Arm9, positions });
                }
            }
            shops
        }
    };
    if shops.iter().any(|s| s.items.is_empty() || s.items.iter().any(|&i| i == 0 || i > 1000)) {
        return Err(bad());
    }
    Ok(shops)
}

/// Réécrit les objets des boutiques à leur place (chaque boutique garde sa taille).
pub fn write(game: &mut GameRom, shops: &[Shop]) -> Result<(), RomError> {
    let mut arm9: Option<Vec<u8>> = None;
    let mut overlay: Option<Vec<u8>> = None;
    let mut narc: Option<kaleido_formats::narc::Narc> = None;
    for shop in shops {
        if shop.items.len() != shop.positions.len() {
            return Err(RomError::Layout(format!("la boutique « {} » doit garder sa taille", shop.name)));
        }
        let data = match shop.source {
            ShopSource::NarcFile(file) => {
                if narc.is_none() {
                    narc = Some(game.narc(B2W2_SHOPS)?);
                }
                narc.as_mut().and_then(|n| n.files.get_mut(file))
            }
            ShopSource::Arm9 => {
                if arm9.is_none() {
                    arm9 = Some(game.rom().arm9_decompressed()?);
                }
                arm9.as_mut()
            }
            ShopSource::Overlay(id) => {
                if overlay.is_none() {
                    overlay = Some(game.rom().overlay(id)?);
                }
                overlay.as_mut()
            }
        };
        let Some(data) = data else { continue };
        for (&item, &at) in shop.items.iter().zip(&shop.positions) {
            data.get_mut(at..at + 2).ok_or_else(bad)?.copy_from_slice(&item.to_le_bytes());
        }
    }
    if let Some(code) = arm9 {
        if code != game.rom().arm9_decompressed()? {
            game.rom_mut().replace_arm9(&code)?;
        }
    }
    if let Some(data) = overlay {
        if data != game.rom().overlay(BW_SHOP_OVERLAY)? {
            // Recompressé : non compressé, l'overlay 21 ne tient plus dans la ROM (DSi Enhanced).
            game.rom_mut().replace_overlay_recompressed(BW_SHOP_OVERLAY, data)?;
        }
    }
    if let Some(n) = narc {
        game.replace_narc(B2W2_SHOPS, &n)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_pointers() {
        assert_eq!(resolve(0x0200_0010, ARM9_RAM, 0x20).unwrap(), 0x10);
        assert!(resolve(0x0200_0030, ARM9_RAM, 0x20).is_err());
        assert!(resolve(0x0100_0000, ARM9_RAM, 0x20).is_err());
        assert!(rd16(&[1], 0).is_err());
        assert_eq!(rd32(&[1, 0, 0, 0], 0).unwrap(), 1);
    }

    #[test]
    fn names_match_counts() {
        assert_eq!(PT_SHOP_NAMES.len(), 21);
        assert!(PT_TM_SHOPS.iter().all(|&i| i <= 20));
        assert!(BW_TM_SHOPS.iter().chain(&BW_MAIN_GAME_SHOPS).all(|&i| i < BW_SHOP_NAMES.len()));
        assert!(B2W2_TM_SHOPS.iter().chain(&B2W2_MAIN_GAME_SHOPS).all(|&i| i < B2W2_SHOP_NAMES.len()));
        // SkipShops d'UPR-ZX : décorations, sceaux (et CT, gardées mais jamais modifiées).
        let skipped = |defs: &[SeqShop]| defs.iter().enumerate().filter(|(_, d)| d.1 != SP).map(|(i, _)| i).collect::<Vec<_>>();
        assert_eq!(skipped(&DP_SHOPS), vec![12, 13, 15, 16, 17, 18, 19, 20, 21, 22, 23]);
        assert_eq!(skipped(&HGSS_SHOPS), vec![17, 18, 22, 23, 24, 25, 26, 27, 28, 29, 30, 37, 39]);
        assert_eq!(sequential_prefix("ADAF").map(|p| p[0]), Some(0xBD));
        assert_eq!(sequential_prefix("IPGK").map(|p| p[12]), Some(0xAD));
        assert!(sequential_prefix("CPUF").is_none());
    }
}
