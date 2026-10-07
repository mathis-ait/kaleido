//! Fiches des espèces (`personal_*`) et attaques apprises (`lvlmove_*`, `eggmove_*`) de PKHeX.

use std::sync::LazyLock;

use serde::Serialize;

use super::{max_species, Game};
use crate::pokemon::BaseStats;
use crate::save::stats::GrowthRate;

/// Fiche d'une espèce (ou d'une forme) dans un jeu donné.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalInfo {
    pub base_stats: BaseStats,
    /// Types (identifiants du jeu, voir [`super::type_name`]) ; identiques si mono-type.
    pub types: [u8; 2],
    pub catch_rate: u8,
    /// Expérience de base donnée en battant l'espèce.
    pub base_exp: u16,
    /// Points d'effort rapportés (0 à 3 par statistique).
    pub ev_yield: BaseStats,
    /// Objets tenus à l'état sauvage (commun, rare, très rare en Gen 5+) ; 0 = aucun.
    pub held_items: [u16; 3],
    /// Taux de femelles sur 254 ; 0 = toujours mâle, 254 = toujours femelle, 255 = asexué.
    pub gender_ratio: u8,
    pub hatch_cycles: u8,
    pub base_friendship: u8,
    pub growth_rate: GrowthRate,
    pub egg_groups: [u8; 2],
    /// Talents 1, 2 et caché (0 : pas de talent caché en Gen 4).
    pub abilities: [u16; 3],
    pub escape_rate: u8,
    /// Nombre de formes (1 si aucune forme alternative).
    pub form_count: u8,
    /// Indice de la fiche de la forme 1 dans la table (0 si aucune).
    pub form_stats_index: u16,
    pub color: u8,
    /// Taille (dm) et poids (hg), connus à partir de la Gen 5.
    pub height: u16,
    pub weight: u16,
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

impl PersonalInfo {
    fn parse(game: Game, d: &[u8]) -> Self {
        let ev = u16_at(d, 0x0A);
        let ev_at = |shift: u16| ((ev >> shift) & 3) as u8;
        let base_stats = BaseStats { hp: d[0], attack: d[1], defense: d[2], speed: d[3], sp_attack: d[4], sp_defense: d[5] };
        let ev_yield = BaseStats { hp: ev_at(0), attack: ev_at(2), defense: ev_at(4), speed: ev_at(6), sp_attack: ev_at(8), sp_defense: ev_at(10) };
        if game.generation() <= 2 {
            return Self::parse_gb(game.generation(), d);
        }
        if game.generation() == 3 {
            // Fiche Gen 3 (0x1C, `PersonalInfo3.cs`) : même début que la Gen 4, sans formes.
            return PersonalInfo {
                base_stats,
                types: [d[6], d[7]],
                catch_rate: d[8],
                base_exp: d[9] as u16,
                ev_yield,
                held_items: [u16_at(d, 0x0C), u16_at(d, 0x0E), 0],
                gender_ratio: d[0x10],
                hatch_cycles: d[0x11],
                base_friendship: d[0x12],
                growth_rate: GrowthRate::from_index(d[0x13]).unwrap_or(GrowthRate::MediumFast),
                egg_groups: [d[0x14], d[0x15]],
                abilities: [d[0x16] as u16, d[0x17] as u16, 0],
                escape_rate: d[0x18],
                color: d[0x19] & 0x7F,
                form_count: 1,
                form_stats_index: 0,
                height: 0,
                weight: 0,
            };
        }
        if game.generation() == 4 {
            // Fiche Gen 4 (0x2C) ; nombre de formes et indice ajoutés par PKHeX en 0x29-0x2B.
            return PersonalInfo {
                base_stats,
                types: [d[6], d[7]],
                catch_rate: d[8],
                base_exp: d[9] as u16,
                ev_yield,
                held_items: [u16_at(d, 0x0C), u16_at(d, 0x0E), 0],
                gender_ratio: d[0x10],
                hatch_cycles: d[0x11],
                base_friendship: d[0x12],
                growth_rate: GrowthRate::from_index(d[0x13]).unwrap_or(GrowthRate::MediumFast),
                egg_groups: [d[0x14], d[0x15]],
                abilities: [d[0x16] as u16, d[0x17] as u16, 0],
                escape_rate: d[0x18],
                color: d[0x19] & 0x7F,
                form_count: d[0x29].max(1),
                form_stats_index: u16_at(d, 0x2A),
                height: 0,
                weight: 0,
            };
        }
        // Gen 5 à 7 : les 0x28 premiers octets ont la même disposition.
        PersonalInfo {
            base_stats,
            types: [d[6], d[7]],
            catch_rate: d[8],
            base_exp: u16_at(d, 0x22),
            ev_yield,
            held_items: [u16_at(d, 0x0C), u16_at(d, 0x0E), u16_at(d, 0x10)],
            gender_ratio: d[0x12],
            hatch_cycles: d[0x13],
            base_friendship: d[0x14],
            growth_rate: GrowthRate::from_index(d[0x15]).unwrap_or(GrowthRate::MediumFast),
            egg_groups: [d[0x16], d[0x17]],
            abilities: [d[0x18] as u16, d[0x19] as u16, d[0x1A] as u16],
            escape_rate: d[0x1B],
            form_stats_index: u16_at(d, 0x1C),
            form_count: d[0x20].max(1),
            color: d[0x21] & 0x3F,
            height: u16_at(d, 0x24),
            weight: u16_at(d, 0x26),
        }
    }

    /// Fiches Gen 1 (0x1C : n°, PV, Att, Déf, Vit, Spécial, types, capture, expérience…, courbe
    /// en 0x13) et Gen 2 (0x20 : n°, 6 statistiques, types, capture, expérience, objets, sexe en
    /// 0x0D, éclosion en 0x0F, courbe en 0x16, groupes d'œufs en 0x17), types en codes GB
    /// (`PersonalInfo1.cs`, `PersonalInfo2.cs` de PKHeX). Les types sont ramenés aux
    /// identifiants Gen 3/4 de Kaleido (voir [`crate::pokemon::gb_type_to_gen4`]).
    fn parse_gb(generation: u8, d: &[u8]) -> Self {
        let t = |b: u8| crate::pokemon::gb_type_to_gen4(b).unwrap_or(0);
        let (stats, types, catch, exp) = if generation == 1 {
            (BaseStats { hp: d[1], attack: d[2], defense: d[3], speed: d[4], sp_attack: d[5], sp_defense: d[5] }, [t(d[6]), t(d[7])], d[8], d[9])
        } else {
            (BaseStats { hp: d[1], attack: d[2], defense: d[3], speed: d[4], sp_attack: d[5], sp_defense: d[6] }, [t(d[7]), t(d[8])], d[9], d[10])
        };
        let growth = if generation == 1 { d[0x13] } else { d[0x16] };
        PersonalInfo {
            base_stats: stats,
            types,
            catch_rate: catch,
            base_exp: exp as u16,
            ev_yield: BaseStats { hp: 0, attack: 0, defense: 0, speed: 0, sp_attack: 0, sp_defense: 0 },
            held_items: if generation == 2 { [d[0x0B] as u16, d[0x0C] as u16, 0] } else { [0; 3] },
            gender_ratio: if generation == 2 { d[0x0D] } else { 127 },
            hatch_cycles: if generation == 2 { d[0x0F] } else { 0 },
            base_friendship: 70,
            growth_rate: GrowthRate::from_index(growth).unwrap_or(GrowthRate::MediumFast),
            egg_groups: if generation == 2 { [d[0x17] & 0xF, d[0x17] >> 4] } else { [0; 2] },
            abilities: [0; 3],
            escape_rate: 0,
            form_count: 1,
            form_stats_index: 0,
            color: 0,
            height: 0,
            weight: 0,
        }
    }

    /// Indice de la fiche de la forme `form` (règle `PersonalInfo.FormIndex` de PKHeX).
    fn form_index(&self, species: u16, form: u8) -> usize {
        if form == 0 || self.form_stats_index == 0 || form >= self.form_count {
            species as usize
        } else {
            self.form_stats_index as usize + form as usize - 1
        }
    }

    /// Lit une fiche brute d'une ROM du jeu (même disposition que les tables de PKHeX ;
    /// en Gen 4, les types restent dans l'ordre de la ROM, avec « ??? » en 9).
    pub fn from_rom(game: Game, data: &[u8]) -> Option<Self> {
        if data.len() < if game.generation() <= 3 { 0x1C } else { 0x28 } {
            return None;
        }
        let mut d = data.to_vec();
        d.resize(d.len().max(0x2C), 0);
        Some(Self::parse(game, &d))
    }

    /// Indice de la fiche d'une forme dans la table complète (voir `form_index`).
    pub fn record_index(&self, species: u16, form: u8) -> usize {
        self.form_index(species, form)
    }
}

struct Table {
    data: &'static [u8],
    size: usize,
}

const PERSONAL: [Table; 16] = [
    Table { data: include_bytes!("../../data/pkhex/personal/personal_dp"), size: 0x2C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_pt"), size: 0x2C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_hgss"), size: 0x2C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_bw"), size: 0x3C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_b2w2"), size: 0x4C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_xy"), size: 0x40 },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_ao"), size: 0x50 },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_sm"), size: 0x54 },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_uu"), size: 0x54 },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_rs"), size: 0x1C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_e"), size: 0x1C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_fr"), size: 0x1C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_rb"), size: 0x1C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_y"), size: 0x1C },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_gs"), size: 0x20 },
    Table { data: include_bytes!("../../data/pkhex/personal/personal_c"), size: 0x20 },
];

fn entry(game: Game, index: usize) -> Option<PersonalInfo> {
    let table = &PERSONAL[game.index()];
    let raw = table.data.get(index * table.size..(index + 1) * table.size)?;
    Some(PersonalInfo::parse(game, raw))
}

/// Indice dans les tables du jeu (fiches, attaques par niveau) de l'espèce et de sa forme.
fn table_index(game: Game, species: u16, form: u8) -> Option<usize> {
    if species == 0 || species > max_species(game) {
        return None;
    }
    Some(entry(game, species as usize)?.form_index(species, form))
}

/// Fiche de l'espèce `species` sous la forme `form` dans le jeu ; la forme 0 sert de
/// repli quand la forme n'a pas de fiche propre (Zarbi, Vivaldaim…).
pub fn personal(game: Game, species: u16, form: u8) -> Option<PersonalInfo> {
    entry(game, table_index(game, species, form)?)
}

/// Octets bruts de la fiche (bits des CT/CS et des donneurs de capacités, lus par la
/// vérification de légalité), et son indice dans la table.
pub(crate) fn personal_raw(game: Game, species: u16, form: u8) -> Option<(usize, &'static [u8])> {
    let index = table_index(game, species, form)?;
    let table = &PERSONAL[game.index()];
    Some((index, table.data.get(index * table.size..(index + 1) * table.size)?))
}

/// Tables « BinLinker16 » de PKHeX : identifiant sur 2 octets, nombre d'entrées sur 2 octets,
/// puis les décalages (u16) de début de chaque entrée, la suivante en marquant la fin.
fn binlinker16(data: &[u8]) -> Vec<&[u8]> {
    let count = u16_at(data, 2) as usize;
    (0..count)
        .map(|i| {
            let start = u16_at(data, 4 + i * 2) as usize;
            let end = u16_at(data, 6 + i * 2) as usize;
            data.get(start..end).unwrap_or(&[])
        })
        .collect()
}

/// Entrée `lvlmove` : n attaques (u16) puis n niveaux (u8).
fn parse_levelup(data: &'static [u8]) -> Vec<Vec<(u16, u8)>> {
    binlinker16(data)
        .into_iter()
        .map(|e| {
            let n = e.len() / 3;
            (0..n).map(|i| (u16_at(e, i * 2), e[n * 2 + i])).collect()
        })
        .collect()
}

fn parse_eggmoves(data: &'static [u8]) -> Vec<Vec<u16>> {
    binlinker16(data).into_iter().map(|e| e.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect()).collect()
}

type Levelup = LazyLock<Vec<Vec<(u16, u8)>>>;
type EggMoves = LazyLock<Vec<Vec<u16>>>;

static LEVELUP: [Levelup; 16] = [
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_dp.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_pt.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_hgss.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_bw.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_b2w2.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_xy.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_ao.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_sm.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_uu.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_rs.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_e.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_fr.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_rb.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_y.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_gs.pkl"))),
    LazyLock::new(|| parse_levelup(include_bytes!("../../data/pkhex/levelup/lvlmove_c.pkl"))),
];

static EGG_GS: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_gs.pkl")));
static EGG_C: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_c.pkl")));
static EGG_RS: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_rs.pkl")));
static EGG_DPPT: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_dppt.pkl")));
static EGG_HGSS: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_hgss.pkl")));
static EGG_BW: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_bw.pkl")));
static EGG_XY: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_xy.pkl")));
static EGG_AO: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_ao.pkl")));
static EGG_SM: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_sm.pkl")));
static EGG_UU: EggMoves = LazyLock::new(|| parse_eggmoves(include_bytes!("../../data/pkhex/eggmove/eggmove_uu.pkl")));

/// Attaques apprises par niveau : couples (attaque, niveau) dans l'ordre du jeu ;
/// le niveau 0 désigne une attaque apprise à l'évolution (Gen 7).
pub fn levelup(game: Game, species: u16, form: u8) -> &'static [(u16, u8)] {
    table_index(game, species, form).and_then(|i| LEVELUP[game.index()].get(i)).map_or(&[], Vec::as_slice)
}

/// Capacités Œuf de l'espèce (à demander pour la forme de base de la lignée).
pub fn egg_moves(game: Game, species: u16, form: u8) -> &'static [u16] {
    if species == 0 || species > max_species(game) {
        return &[];
    }
    let (table, index): (&EggMoves, Option<usize>) = match game {
        // Pas d'œufs en Gen 1.
        Game::RB | Game::Y => return &[],
        Game::GS => (&EGG_GS, Some(species as usize)),
        Game::C => (&EGG_C, Some(species as usize)),
        Game::RS | Game::E | Game::FRLG => (&EGG_RS, Some(species as usize)),
        Game::DP | Game::Pt => (&EGG_DPPT, Some(species as usize)),
        Game::HGSS => (&EGG_HGSS, Some(species as usize)),
        Game::BW | Game::B2W2 => (&EGG_BW, Some(species as usize)),
        Game::XY => (&EGG_XY, Some(species as usize)),
        Game::ORAS => (&EGG_AO, Some(species as usize)),
        // En Gen 7, les capacités Œuf dépendent de la forme (Miaouss d'Alola…).
        Game::SM => (&EGG_SM, table_index(game, species, form)),
        Game::USUM => (&EGG_UU, table_index(game, species, form)),
    };
    index.and_then(|i| table.get(i)).map_or(&[], Vec::as_slice)
}
