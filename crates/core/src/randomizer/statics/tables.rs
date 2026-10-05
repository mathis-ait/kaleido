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
    /// Fichier n° `.0` de l'archive des cartes (Noire/Blanche : `a/1/2/5`).
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
pub const BW_TRADE_TEXT: usize = 35;
