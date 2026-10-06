//! Emplacements des Pokémon fixes et des échanges, repris des fichiers
//! `gen4_offsets.ini` / `gen5_offsets.ini` de l'Universal Pokémon Randomizer (FVX).
//!
//! Les entrées `StaticPokemon{}` de Platine (U) sont copiées telles quelles par
//! Platine (E) et (U Rev 1) (`CopyStaticPokemon=1`) ; celles de Noire (U) par
//! Blanche (U) et par toutes les versions traduites de Noire/Blanche (dont
//! Blanche (F), IRAF). Chaque valeur est relue et vérifiée avant toute écriture.

/// Emplacement d'une valeur dans la ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loc {
    /// Fichier n° `.0` de l'archive des scripts, décalage `.1`.
    Script(u16, u32),
    /// Fichier n° `.0` de l'archive des cartes (Noire/Blanche : `a/1/2/5` ; Noire 2 / Blanche 2 : `a/1/2/6`).
    Map(u16, u32),
    /// Code ARM9 (décompressé).
    Arm9(u32),
    /// Overlay `.0` (décompressé), décalage `.1`.
    Overlay(u32, u32),
}

/// Nature d'une rencontre fixe (pour l'affichage et les règles de choix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Combat ou don scripté.
    Static,
    /// Œuf offert (pas de niveau).
    Egg,
    /// Fossile ressuscité.
    Fossil,
    /// Pokémon vagabond.
    Roamer,
    /// Faux objet au sol (Noire/Blanche : Trompignon et Gaulet déguisés en Poké Ball).
    FakeItem,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Static => "fixe",
            Kind::Egg => "œuf",
            Kind::Fossil => "fossile",
            Kind::Roamer => "vagabond",
            Kind::FakeItem => "faux objet",
        }
    }
}

/// Une rencontre : toutes les copies de l'espèce (u16) et tous les niveaux (u8).
#[derive(Debug, Clone)]
pub struct Def {
    pub species: Vec<Loc>,
    pub levels: Vec<Loc>,
    pub kind: Kind,
}

fn script(kind: Kind, species: &[(u16, u32)], levels: &[(u16, u32)]) -> Def {
    Def { species: species.iter().map(|&(f, o)| Loc::Script(f, o)).collect(), levels: levels.iter().map(|&(f, o)| Loc::Script(f, o)).collect(), kind }
}

/// Platine : `StaticPokemon{}` de `[Platinum (U)]` (ordre identique à l'UPR).
pub fn platinum() -> Vec<Def> {
    use Kind::{Egg, Static};
    let mut out = vec![
        script(Static, &[(291, 0x43), (291, 0x52), (389, 0xCC), (389, 0xDD)], &[(291, 0x54), (389, 0xDF)]), // Giratina
        script(Static, &[(361, 0x39), (361, 0x48)], &[(361, 0x4A)]),                                        // Créhelf
        script(Static, &[(357, 0x81), (357, 0x90)], &[(357, 0x92)]),                                        // Créfadet
        script(Static, &[(239, 0xAB), (239, 0xB8)], &[(239, 0xBA)]),                                        // Dialga
        script(Static, &[(240, 0xAB), (240, 0xB8)], &[(240, 0xBA)]),                                        // Palkia
        script(Static, &[(286, 0x103), (286, 0x112)], &[(286, 0x114)]),                                     // Heatran
        script(Static, &[(317, 0x88), (317, 0x94)], &[(317, 0x96)]),                                        // Regigigas
        script(Static, &[(392, 0xB0), (392, 0xBD)], &[(392, 0xBF)]),                                        // Registeel
        script(Static, &[(394, 0xB0), (394, 0xBD)], &[(394, 0xBF)]),                                        // Regice
        script(Static, &[(396, 0xB0), (396, 0xBD)], &[(396, 0xBF)]),                                        // Regirock
        script(Static, &[(363, 0x8E)], &[(363, 0x90)]),                                                     // Darkrai
        script(Static, &[(310, 0x87), (310, 0x96)], &[(310, 0x98)]),                                        // Shaymin
        script(Static, &[(238, 0x6C), (238, 0x7A), (238, 0x89)], &[(238, 0x8B)]),                           // Arceus
        script(Egg, &[(71, 0xE5B)], &[]),                                                                   // Togepi (œuf)
        script(Static, &[(117, 0x79)], &[(117, 0x7B)]),                                                     // Évoli
        script(Static, &[(153, 0x7D)], &[(153, 0x7F)]),                                                     // Porygon
        script(Egg, &[(329, 0x338)], &[]),                                                                  // Riolu (œuf)
        script(Static, &[(216, 0x1C9), (216, 0x1DA)], &[(216, 0x1DC)]),                                     // Baudrive
        script(Static, &[(337, 0x51), (337, 0x5D)], &[(337, 0x5F)]),                                        // Motisma
        script(Static, &[(441, 0x153), (441, 0x160)], &[(441, 0x162)]),                                     // Spiritomb
    ];
    // Fossiles : table de l'ARM9 (`FossilTableOffset`), 7 × (u16 objet, u16 espèce) ;
    // niveau commun dans le script 65 (`FossilLevelOffset`).
    for f in 0..7 {
        out.push(Def { species: vec![Loc::Arm9(PT_FOSSIL_TABLE + 2 + f * 4)], levels: vec![Loc::Script(65, 0x426)], kind: Kind::Fossil });
    }
    out
}

const PT_FOSSIL_TABLE: u32 = 0xEBFFC;

/// Diamant / Perle : `StaticPokemon{}` de `[Diamond (U)]` (UPR-ZX), copiées par toutes
/// les versions traduites (`CopyStaticPokemon=1`) ; vérifiées sur Diamant (ADAF).
/// Archive des scripts : `scr_seq_release.narc`.
pub fn diamond_pearl(code: &str) -> Vec<Def> {
    use Kind::{Egg, Static};
    let mut out = vec![
        script(Static, &[(342, 0x261), (342, 0x2BE)], &[(342, 0x2C0)]), // Étourmi (Lac Vérité)
        script(Static, &[(230, 0x4AE), (230, 0xE9A), (230, 0xECE), (230, 0x1201), (230, 0x1235)], &[(230, 0xEE4), (230, 0x124B)]), // Dialga
        script(Static, &[(230, 0x4B4), (230, 0xEA0), (230, 0xED4), (230, 0x1207), (230, 0x123B)], &[(230, 0xEE4), (230, 0x124B)]), // Palkia
        script(Static, &[(352, 0x39), (352, 0x48)], &[(352, 0x4A)]),    // Créhelf
        script(Static, &[(348, 0x81), (348, 0x90)], &[(348, 0x92)]),    // Créfadet
        script(Static, &[(278, 0x16F), (278, 0x17E)], &[(278, 0x180)]), // Heatran
        script(Static, &[(309, 0x88), (309, 0x94)], &[(309, 0x96)]),    // Regigigas
        script(Static, &[(283, 0x50), (283, 0x5F)], &[(283, 0x61)]),    // Giratina
        script(Static, &[(354, 0x40)], &[(354, 0x42)]),                 // Darkrai
        script(Static, &[(302, 0x39), (302, 0x48)], &[(302, 0x4A)]),    // Shaymin
        script(Static, &[(232, 0x45), (232, 0x53), (232, 0x62)], &[(232, 0x64)]), // Arceus
        script(Static, &[(112, 0xB5)], &[(112, 0xB7)]),                 // Évoli
        script(Egg, &[(90, 0x568)], &[]),                               // Ptiravi (œuf)
        script(Egg, &[(321, 0x332)], &[]),                              // Riolu (œuf)
        script(Static, &[(210, 0x1C5), (210, 0x1D6)], &[(210, 0x1D8)]), // Baudrive
        script(Static, &[(329, 0x74), (329, 0x80)], &[(329, 0x82)]),    // Motisma
        script(Static, &[(406, 0x153), (406, 0x160)], &[(406, 0x162)]), // Spiritomb
    ];
    // Fossiles : table de l'ARM9 (`FossilTableOffset`, selon la langue) ; niveau dans le script 63.
    let table = match code {
        "ADAE" | "APAE" => Some(0xF450C),
        "ADAF" | "APAF" => Some(0xF4550),
        "ADAD" | "APAD" => Some(0xF4520),
        "ADAS" | "APAS" => Some(0xF455C),
        "ADAI" | "APAI" => Some(0xF44C4),
        "ADAJ" => Some(0xF6330),
        "APAJ" => Some(0xF6334),
        "ADAK" | "APAK" => Some(0xEFB5C),
        _ => None,
    };
    if let Some(table) = table {
        for f in 0..7 {
            out.push(Def { species: vec![Loc::Arm9(table + 2 + f * 4)], levels: vec![Loc::Script(63, 0x41A)], kind: Kind::Fossil });
        }
    }
    out
}

/// HeartGold / SoulSilver : `StaticPokemon{}` de `[HeartGold (U)]` (UPR-ZX), copiées par
/// les versions traduites ; toutes relues sur SoulSilver (IPGF). Archive des scripts : `a/0/1/2`.
/// Ignorés : Giratina Originel (forme), lots du Casino (textes du menu), Shuckie et
/// Kenya (échanges), œuf mystère et vagabonds (code de l'overlay / de l'ARM9).
pub fn heartgold_soulsilver() -> Vec<Def> {
    use Kind::{Egg, Static};
    let mut out = vec![
        script(Static, &[(104, 0x108)], &[(104, 0x138), (104, 0x12C)]), // Lugia
        script(Static, &[(21, 0xD1)], &[(21, 0xF5), (21, 0x101)]),      // Ho-Oh
        script(
            Static,
            &[(216, 0x58F), (216, 0x6E8), (216, 0x708), (24, 0x67), (24, 0xB4), (24, 0x314), (24, 0x320), (24, 0xD4)],
            &[(216, 0x70A), (24, 0x322)],
        ), // Suicune
        script(Static, &[(14, 0x2F), (14, 0x3B)], &[(14, 0x3D)]),       // Artikodin
        script(Static, &[(191, 0x26B), (191, 0x277)], &[(191, 0x279)]), // Électhor
        script(Static, &[(106, 0x2F), (106, 0x3B)], &[(106, 0x3D)]),    // Sulfura
        script(Static, &[(11, 0x2F), (11, 0x3B)], &[(11, 0x3D)]),       // Mewtwo
        script(Static, &[(134, 0xA3), (134, 0xB4)], &[(134, 0xB6)]),    // Kyogre
        script(Static, &[(133, 0xA3), (133, 0xB4)], &[(133, 0xB6)]),    // Groudon
        script(Static, &[(135, 0xDA), (135, 0xEB), (135, 0x62), (135, 0x98)], &[(135, 0xED)]), // Rayquaza
        script(Static, &[(131, 0x43A), (131, 0x67C), (131, 0x872), (131, 0x8E4), (131, 0x958), (131, 0x963)], &[(131, 0x965)]), // Dialga
        script(Static, &[(131, 0x4A2), (131, 0x695), (131, 0x88D), (131, 0x8FA), (131, 0x97F), (131, 0x98A)], &[(131, 0x98C)]), // Palkia
        script(Static, &[(750, 0x4CC)], &[(750, 0x4E3)]),               // Latias
        script(Static, &[(750, 0x4B7)], &[(750, 0x4E3)]),               // Latios
        script(Static, &[(243, 0x2FD), (243, 0x14B)], &[(243, 0x2FF), (243, 0x14D)]), // Simularbre
        script(Static, &[(58, 0x61), (58, 0x6D)], &[(58, 0x6F)]),       // Lokhlass
        script(Static, &[(938, 0x3CD), (938, 0x3DE)], &[(938, 0x3E0)]), // Léviator rouge
        script(Static, &[(197, 0x6C), (197, 0x7D), (199, 0x26A), (199, 0x27B)], &[(197, 0x7F), (199, 0x27D)]), // Ronflex
        script(
            Static,
            &[(89, 0xF3D), (89, 0x1078), (89, 0x10A5), (89, 0x112C), (89, 0x11B3)],
            &[(89, 0xF3F), (89, 0x107A), (89, 0x10A7), (89, 0x112E), (89, 0x11B5)],
        ), // Smogo (QG Rocket)
        script(
            Static,
            &[(89, 0xF6A), (89, 0xFC4), (89, 0x101E), (89, 0x104B), (89, 0x1159), (89, 0x1186)],
            &[(89, 0xF6C), (89, 0xFC6), (89, 0x1020), (89, 0x104D), (89, 0x115B), (89, 0x1188)],
        ), // Voltorbe (QG Rocket)
        script(
            Static,
            &[(89, 0xF97), (89, 0xFF1), (89, 0x10D2), (89, 0x10FF), (89, 0x11E0)],
            &[(89, 0xF99), (89, 0xFF3), (89, 0x10D4), (89, 0x1101), (89, 0x11E2)],
        ), // Racaillou (QG Rocket)
        script(Static, &[(90, 0x784)], &[(90, 0x786)]),                 // Électrode (QG Rocket) 1
        script(Static, &[(90, 0x7E8)], &[(90, 0x7EA)]),                 // Électrode 2
        script(Static, &[(90, 0x84C)], &[(90, 0x84E)]),                 // Électrode 3
        script(Static, &[(892, 0x61)], &[(892, 0x63)]),                 // Évoli
        script(Static, &[(98, 0x71)], &[(98, 0x73)]),                   // Debugant
        script(Static, &[(112, 0x4D1)], &[(112, 0x4D3)]),               // Minidraco
        script(Static, &[(740, 0x66F), (740, 0x675), (740, 0x695), (740, 0x818), (740, 0x8BC)], &[(740, 0x86D)]), // Bulbizarre
        script(Static, &[(740, 0x71D), (740, 0x723), (740, 0x743), (740, 0x833), (740, 0x8D7)], &[(740, 0x86D)]), // Carapuce
        script(Static, &[(740, 0x7CB), (740, 0x7D1), (740, 0x7F1)], &[(740, 0x86D)]), // Salamèche
        script(Static, &[(837, 0x28F)], &[(837, 0x2D1)]),               // Arcko
        script(Static, &[(837, 0x2A8)], &[(837, 0x2D1)]),               // Poussifeu
        script(Static, &[(837, 0x2B4)], &[(837, 0x2D1)]),               // Gobou
        script(Egg, &[(860, 0x146), (860, 0x14D)], &[]),                // Œuf de Wattouat (Primo)
        script(Egg, &[(860, 0x180), (860, 0x187)], &[]),                // Œuf d'Axoloto (Primo)
        script(Egg, &[(860, 0x1BA), (860, 0x1C1)], &[]),                // Œuf de Limagma (Primo)
        script(Static, &[(878, 0x90)], &[(878, 0x92)]),                 // Tentacool secret
    ];
    // Fossiles : table de l'overlay 21 (`FossilTableOvlNumber`), niveau dans le script 755.
    for f in 0..7 {
        out.push(Def {
            species: vec![Loc::Overlay(HGSS_FOSSIL_OVERLAY, 0x130 + 2 + f * 4)],
            levels: vec![Loc::Script(755, 0x58D)],
            kind: Kind::Fossil,
        });
    }
    out
}

pub const HGSS_FOSSIL_OVERLAY: u32 = 21;

/// Noire/Blanche : `StaticPokemon{}` puis `StaticPokemonFakeBall{}` de `[Black (U)]`.
pub fn black_white() -> Vec<Def> {
    use Kind::{Egg, Fossil, Static};
    let mut out = vec![
        script(Static, &[(304, 0x121), (304, 0x1C9), (304, 0x299)], &[(304, 0x29D)]), // Feuillajou
        script(Static, &[(304, 0x131), (304, 0x1DE), (304, 0x2B7)], &[(304, 0x2BB)]), // Flamajou
        script(Static, &[(304, 0xFE), (304, 0x1A1), (304, 0x268)], &[(304, 0x26C)]),  // Flotajou
        script(Static, &[(526, 0x758)], &[(526, 0x75C)]),                             // Magicarpe
        script(Static, &[(94, 0x810), (94, 0x64), (94, 0xB4), (94, 0x44B), (94, 0x7AB), (94, 0x7D0), (94, 0x7DC)], &[(94, 0x44F)]), // Zorua
        script(Egg, &[(776, 0x85), (776, 0xB2)], &[]),                                // Pyronille (œuf)
        script(Static, &[(316, 0x369)], &[(316, 0x36B)]),                             // Darumacho 1
        script(Static, &[(316, 0x437)], &[(316, 0x439)]),                             // Darumacho 2
        script(Static, &[(316, 0x505)], &[(316, 0x507)]),                             // Darumacho 3
        script(Static, &[(316, 0x5D3)], &[(316, 0x5D5)]),                             // Darumacho 4
        script(Static, &[(316, 0x6A1)], &[(316, 0x6A3)]),                             // Darumacho 5
        script(Static, &[(306, 0x65), (306, 0x8F)], &[(306, 0x91)]),                  // Mushana
        script(Static, &[(770, 0x2F8), (770, 0x353)], &[(770, 0x355)]),               // Zoroark
        script(Static, &[(364, 0xE), (364, 0x1F)], &[(364, 0x21)]),                   // Pyrax
        script(Static, &[(474, 0x1CE), (474, 0x20A)], &[(474, 0x20C)]),               // Victini
        script(Static, &[(426, 0x133), (426, 0x15B), (556, 0x1841), (556, 0xCFC), (556, 0x1878), (556, 0x18EA)], &[(426, 0x15D), (556, 0xCFE)]), // Reshiram
        script(Static, &[(426, 0x127), (426, 0x174), (556, 0x184D), (556, 0x186C), (556, 0xD15), (556, 0x18DE)], &[(426, 0x176), (556, 0xD17)]), // Zekrom
        script(Static, &[(670, 0x415), (670, 0x426), (692, 0x1E2)], &[(670, 0x428)]), // Cobaltium
        script(Static, &[(458, 0x10), (458, 0x21), (692, 0x203)], &[(458, 0x23)]),    // Terrakium
        script(Static, &[(312, 0x10), (312, 0x21), (692, 0x224)], &[(312, 0x23)]),    // Viridium
        script(Static, &[(752, 0x66D), (752, 0x6CC), (752, 0x6DD)], &[(752, 0x6DF)]), // Démétéros
        script(Static, &[(464, 0x10), (468, 0x4F), (468, 0x60)], &[(468, 0x62)]),     // Kyurem
    ];
    // Fossiles (script 877, niveau commun).
    for off in [0x601, 0x620, 0x63F, 0x65E, 0x67D, 0x69C, 0x6BB, 0x6DA, 0x6F9] {
        out.push(script(Fossil, &[(877, off)], &[(877, 0x3F7)]));
    }
    // Faux objets (Trompignon, Gaulet) : espèce dans le script 897, niveaux dans les cartes.
    out.push(Def {
        species: vec![Loc::Script(897, 0x45)],
        levels: [(331, 0x2AA), (331, 0x2CE), (355, 0x196), (355, 0x2DA)].iter().map(|&(f, o)| Loc::Map(f, o)).collect(),
        kind: Kind::FakeItem,
    });
    out.push(Def {
        species: vec![Loc::Script(897, 0xC1)],
        levels: [(355, 0x24A), (355, 0x26E)].iter().map(|&(f, o)| Loc::Map(f, o)).collect(),
        kind: Kind::FakeItem,
    });
    out
}

/// Noire 2 / Blanche 2 : `StaticPokemon{}` puis `StaticPokemonFakeBall{}` de `[Black 2 (U)]`,
/// copiés tels quels par Blanche 2 et les versions traduites (`CopyStaticPokemon=1`).
/// Scripts `a/0/5/6`, cartes `a/1/2/6`. Vérifié sur Noire 2 (IREF).
pub fn black2_white2() -> Vec<Def> {
    use Kind::{Egg, Fossil, Static};
    let mut out = vec![
        script(Static, &[(662, 0x1DE), (662, 0x240), (740, 0xCD), (740, 0xFC), (740, 0x12C), (740, 0x14C)], &[(740, 0x12E), (740, 0x14E)]), // Cobaltium
        script(Static, &[(730, 0x13A), (730, 0x15F), (730, 0x19B), (730, 0x1BB)], &[(730, 0x19D), (730, 0x1BD)]), // Viridium
        script(Static, &[(948, 0x45D), (948, 0x48D), (948, 0x4AD)], &[(948, 0x48F), (948, 0x4AF)]),               // Terrakium
        script(
            Static,
            &[(426, 0x38A), (426, 0x39B), (556, 0x367), (556, 0x568), (556, 0x5E6), (556, 0x6E1), (1208, 0x3A4), (1208, 0xA6A), (1208, 0x717)],
            &[(426, 0x39D)],
        ), // Reshiram
        script(
            Static,
            &[(426, 0x36B), (426, 0x37C), (556, 0x350), (556, 0x551), (556, 0x5C7), (556, 0x6C3), (1208, 0x38D), (1208, 0xA53), (1208, 0x706)],
            &[(426, 0x37E)],
        ), // Zekrom
        script(
            Static,
            &[
                (1112, 0x133),
                (1122, 0x2BA),
                (1122, 0x311),
                (1128, 0x37A),
                (1128, 0x3D1),
                (1208, 0x1B7),
                (1208, 0x1F8),
                (1208, 0x723),
                (1208, 0xF3D),
                (1208, 0xF4E),
            ],
            &[(1208, 0xF50)],
        ), // Kyurem
        script(Static, &[(1208, 0xD8B), (1208, 0xD97)], &[(1208, 0xD99)]),                                        // Kyurem Noir (forme : B2W2_FORMS)
        script(Static, &[(1208, 0xDB6), (1208, 0xDC2)], &[(1208, 0xDC4)]),                                        // Kyurem Blanc
        script(
            Static,
            &[
                (304, 0xCC),
                (304, 0x14B),
                (304, 0x1BC),
                (304, 0x237),
                (304, 0x327),
                (304, 0x3E6),
                (304, 0x4A1),
                (304, 0x54A),
                (304, 0x5BD),
                (304, 0x5CE),
            ],
            &[(304, 0x5D0)],
        ), // Latias
        script(
            Static,
            &[
                (304, 0xB5),
                (304, 0x134),
                (304, 0x1A5),
                (304, 0x220),
                (304, 0x310),
                (304, 0x3CF),
                (304, 0x48A),
                (304, 0x533),
                (304, 0x59E),
                (304, 0x5AF),
            ],
            &[(304, 0x5B1)],
        ), // Latios
        script(Static, &[(32, 0x247), (32, 0x2B0), (32, 0x2C1), (1034, 0x12A)], &[(32, 0x2C3)]),                  // Créhelf
        script(Static, &[(684, 0x136), (684, 0x1C2), (684, 0x1D3), (1034, 0x169)], &[(684, 0x1D5)]),              // Créfollet
        script(Static, &[(950, 0xA1), (950, 0x10A), (950, 0x11B), (1034, 0x1BE)], &[(950, 0x11D)]),               // Créfadet
        script(Static, &[(1222, 0x134), (1222, 0x145), (1018, 0x32)], &[(1222, 0x147)]),                          // Regirock
        script(Static, &[(1224, 0x134), (1224, 0x145), (1018, 0x2C)], &[(1224, 0x147)]),                          // Regice
        script(Static, &[(1226, 0x134), (1226, 0x145), (1018, 0x38)], &[(1226, 0x147)]),                          // Registeel
        script(Static, &[(1018, 0x97), (1018, 0xA8)], &[(1018, 0xAA)]),                                           // Regigigas
        script(Static, &[(526, 0x48D), (526, 0x512), (526, 0x523)], &[(526, 0x525)]),                             // Cresselia
        script(Static, &[(1068, 0x193), (1068, 0x1D6), (1068, 0x1E7), (1080, 0x193), (1080, 0x1D6), (1080, 0x1E7)], &[(1068, 0x1E9), (1080, 0x1E9)]), // Heatran
        script(Static, &[(652, 0x5C6), (652, 0x5E9)], &[(652, 0x5EB)]),    // Vaututrice
        script(Static, &[(1102, 0x592), (1102, 0x5B5)], &[(1102, 0x5B7)]), // Gueriaigle
        script(Static, &[(364, 0xE), (364, 0x32), (364, 0x40)], &[(364, 0x34), (364, 0x42)]), // Pyrax
        script(Static, &[(1030, 0x290), (1030, 0x2A1)], &[(1030, 0x2A3)]), // Crabaraque
        script(Static, &[(480, 0xE1), (480, 0x10A), (480, 0x131), (480, 0x15A)], &[(480, 0x10C), (480, 0x15C)]), // Moyade
        script(Static, &[(1168, 0x2C), (1168, 0x4F)], &[(1168, 0x51)]),    // Tranchodon chromatique
        script(Static, &[(988, 0x382)], &[(988, 0x386)]),                  // Évoli
        script(Static, &[(664, 0x3B5), (664, 0x3E2), (664, 0x40F), (664, 0x43C)], &[(664, 0x3B9), (664, 0x3E6), (664, 0x413), (664, 0x440)]), // Vivaldaim (quatre saisons)
        script(Static, &[(880, 0xAB4), (880, 0xAC7)], &[(880, 0xAB8)]), // Griknot chromatique
        script(Static, &[(880, 0xAD3), (880, 0xAE6)], &[(880, 0xAD7)]), // Minidraco chromatique
        script(Egg, &[(54, 0xDD)], &[]),                                // Ptiravi (œuf)
        script(Static, &[(526, 0x27E)], &[(526, 0x282)]),               // Magicarpe
    ];
    // Fossiles (script 1253, niveau commun).
    for off in [0x5E0, 0x5FF, 0x61E, 0x63D, 0x65C, 0x67B, 0x69A, 0x6B9, 0x6D8] {
        out.push(script(Fossil, &[(1253, off)], &[(1253, 0x3D6)]));
    }
    out.push(script(Static, &[(208, 0x5A6)], &[(208, 0x5A8)])); // Zorua
                                                                // Faux objets (Trompignon, Gaulet) : espèce dans le script 1273, niveaux dans les cartes.
    out.push(Def {
        species: vec![Loc::Script(1273, 0x45)],
        levels: [(500, 0x46E), (500, 0x492), (500, 0x4B6), (506, 0x42A), (506, 0x44E)].iter().map(|&(f, o)| Loc::Map(f, o)).collect(),
        kind: Kind::FakeItem,
    });
    out.push(Def {
        species: vec![Loc::Script(1273, 0xC7)],
        levels: [(534, 0x2F2), (534, 0x316), (562, 0x3FE), (562, 0x422), (563, 0x742), (563, 0x766), (563, 0x78A)]
            .iter()
            .map(|&(f, o)| Loc::Map(f, o))
            .collect(),
        kind: Kind::FakeItem,
    });
    out
}

/// Noire 2 / Blanche 2 : formes imposées (`Forme=[…]` de l'UPR) des rencontres
/// Kyurem Noir (6), Kyurem Blanc (7) et Vivaldaim (26). Remises à 0 quand l'espèce change.
pub const B2W2_FORMS: [(usize, &[(u16, u32)]); 3] =
    [(6, &[(1208, 0xD8D), (1208, 0xD9B)]), (7, &[(1208, 0xDB8), (1208, 0xDC6)]), (26, &[(664, 0x3B7), (664, 0x3E4), (664, 0x411), (664, 0x43E)])];

/// Échanges de Noire 2 / Blanche 2 (`a/1/6/3`) : entrée inutilisée (`TradesUnused`).
pub const BLACK2_TRADES_UNUSED: [usize; 1] = [25];
pub const WHITE2_TRADES_UNUSED: [usize; 1] = [24];
/// Textes des échanges (surnoms et dresseurs d'origine).
pub const B2W2_TRADE_TEXT: usize = 37;
/// Dialogues des personnes qui proposent les échanges : (entrée, fichier des textes de
/// l'histoire `a/0/0/3`), d'après `IngameTradePersonTextOffsets` de l'UPR (Noire 2 :
/// 529, 555, 193, 594, 628, 628 ; Blanche 2 : 537 pour son échange exclusif). Vérifié
/// sur Noire 2 : Chlorobule/Doudouvet (529), Gigalithe/Emolga (555), Bouldeneu/Démanta
/// (193), Motisma/Métamorph (594), Capidextre/Minotaupe et Alakazam/Hippodocus (628).
pub const B2W2_TRADE_PERSON_TEXTS: [(usize, usize); 7] = [(24, 529), (25, 537), (26, 555), (27, 193), (28, 594), (29, 628), (30, 628)];
pub const B2W2_STORY_TEXT: &str = "a/0/0/3";

/// Index du légendaire de la boîte (`BoxLegendaryOffset`) : Reshiram (Noire), Zekrom (Blanche).
pub const BW_BOX_LEGENDARY_BLACK: usize = 15;
pub const BW_BOX_LEGENDARY_WHITE: usize = 16;

/// Vagabonds de Noire/Blanche (overlay 10) : emplacements de Noire (U) / Blanche (U).
/// Les versions traduites décalent le code : on retrouve la fonction
/// `GetRoamerFlagOffset` par sa signature et on applique le même décalage.
pub const BW_ROAMER_OVERLAY: u32 = 10;
pub const BW_ROAMER_FUNCTION_US: u32 = 0x95C4;
/// Début de la fonction d'origine (Tornadus en constante, Fulguris = Tornadus + 1).
pub const BW_ROAMER_FUNCTION: [u8; 24] =
    [0x05, 0x49, 0x88, 0x42, 0x04, 0xD0, 0x49, 0x1C, 0x88, 0x42, 0x03, 0xD1, 0x00, 0x20, 0x70, 0x47, 0x01, 0x20, 0x70, 0x47, 0x02, 0x20, 0x70, 0x47];
/// (espèce : constantes de l'overlay (US), décalages dans le script 674), pour Fulguris puis Boréas.
pub const BW_ROAMERS: [(u16, [u32; 2], [u32; 2]); 2] = [(642, [0x95D8, 0x940C], [0x57E, 0x5F1]), (641, [0x95DC, 0x9410], [0x572, 0x5DC])];
pub const BW_ROAMER_LEVEL_US: u32 = 0x930E;
pub const BW_ROAMER_SCRIPT: u16 = 674;

/// Correctif du légendaire de la boîte (Gen5Constants).
pub const BLACK_BOX_PREFIX1: [u8; 10] = [0x79, 0xF6, 0xBA, 0xEF, 0x07, 0xB0, 0xF0, 0xBD, 0xC0, 0x46];
pub const BLACK_BOX_PREFIX2: [u8; 12] = [0xDE, 0xDB, 0x00, 0x20, 0xC0, 0x43, 0x02, 0xB0, 0xF8, 0xBD, 0xC0, 0x46];
pub const WHITE_BOX_PREFIX1: [u8; 8] = [0x00, 0xF0, 0xFE, 0xF8, 0x00, 0x20, 0x70, 0xBD];
pub const WHITE_BOX_PREFIX2: [u8; 8] = [0x64, 0xF6, 0x2E, 0xF9, 0x70, 0xBD, 0x00, 0x00];
pub const BW_FIELD_OVERLAY: u32 = 21;

/// Platine : correctif du sol du Monde Distorsion (Gen4Constants.distortionWorldGroundCheckPrefix).
pub const PT_FIELD_OVERLAY: u32 = 5;
pub const PT_DISTORTION_PREFIX: [u8; 14] = [0x23, 0xD8, 0x49, 0x18, 0x79, 0x44, 0xC9, 0x88, 0x09, 0x04, 0x09, 0x14, 0x8F, 0x44];

/// Échanges de Noire/Blanche (`a/1/6/5`) : entrées inutilisées (`TradesUnused`).
pub const BLACK_TRADES_UNUSED: [usize; 8] = [1, 3, 7, 8, 9, 10, 11, 12];
pub const WHITE_TRADES_UNUSED: [usize; 8] = [0, 2, 7, 8, 9, 10, 11, 12];
/// `TradeScript[]` : affichage des Pokémon échangés (script, [(demandé, donné)]), dans
/// l'ordre des échanges utilisés. Les deux versions partagent ces scripts.
pub const BW_TRADE_SCRIPTS: [(u16, &[(u32, u32)]); 5] = [
    (46, &[(0x81, 0x86), (0x97, 0x9C)]),                  // Doudouvet / Chlorobule
    (202, &[(0x224, 0x21F)]),                             // Chinchidou / Bargantua
    (686, &[(0x76, 0x71)]),                               // Géolithe / Emolga
    (830, &[(0xB3, 0xAE), (0xEA, 0xE5), (0x114, 0x10F)]), // Pashmilla / Goinfrex
    (764, &[(0x43, 0x3E)]),                               // Métamorph / Motisma
];
/// Textes des échanges : surnoms et dresseurs d'origine.
pub const PT_TRADE_TEXT: usize = 370;
/// Dialogues des personnes qui proposent les échanges (Platine).
pub const PT_TRADE_PERSON_TEXTS: [usize; 4] = [74, 97, 180, 643];
/// Diamant / Perle : textes des échanges et dialogues (`IngameTradesTextOffset`,
/// `IngameTradePersonTextOffsets` ; vérifiés sur Diamant ADAF).
pub const DP_TRADE_TEXT: usize = 326;
pub const DP_TRADE_PERSON_TEXTS: [usize; 4] = [67, 89, 171, 584];
/// HeartGold / SoulSilver (vérifié sur SoulSilver IPGF) : 0 = pas de dialogue ; entrées 6 et 7
/// (Shuckie, Kenya) traitées à part par l'UPR et ignorées ici.
pub const HGSS_TRADE_TEXT: usize = 200;
pub const HGSS_TRADE_PERSON_TEXTS: [usize; 11] = [562, 596, 608, 634, 0, 0, 344, 463, 535, 47, 537];
pub const HGSS_TRADES_UNUSED: [usize; 2] = [6, 7];
pub const BW_TRADE_TEXT: usize = 35;
