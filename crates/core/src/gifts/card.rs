//! Lecture des cartes cadeau : PCD / PGT (Gen 4), PGF (Gen 5), WC6 / WC7 (+ « full »).
//!
//! Portage de PKHeX (kwsch/PKHeX, GPLv3, comme Kaleido), dossier `PKHeX.Core/MysteryGifts/` :
//! `PGT.cs`, `PCD.cs`, `PGF.cs`, `WC6.cs`, `WC6Full.cs`, `WC7.cs`, `WC7Full.cs`, `ShinyType6.cs`.
//! Les offsets ci-dessous sont repris tels quels de ces fichiers.
//!
//! | Format | Taille | Contenu |
//! |---|---|---|
//! | PGT | 0x104 | type (u16), emplacement, détail, objet (u32), PK4 d'équipe **chiffré** en 0x08 |
//! | PCD | 0x358 | PGT + titre (0x104, 36 caractères), compatibilité (0x14C), n° de carte (0x150) |
//! | PGF | 0xCC | carte Gen 5 (titre en 0x60, date en 0xAC, n° en 0xB0, type en 0xB3) |
//! | WC6 / WC7 | 0x108 | carte 3DS (n° en 0x00, titre en 0x02, date en 0x4C, type en 0x51) |
//! | WC6full / WC7full | 0x310 | en-tête de distribution (version admise en 0x00, langue en 0x1FF) + carte en 0x208 |

use super::{GiftError, GiftFormat, GiftItem, GiftKind, GiftPokemon, AbilityRule, ShinyRule, RIBBONS};
use crate::save::{Gender, PkmDate, PkmFormat, Pokemon};

pub(super) const PGT_SIZE: usize = 0x104;
pub(super) const PCD_SIZE: usize = 0x358;
pub(super) const PGF_SIZE: usize = 0xCC;
pub(super) const WC_SIZE: usize = 0x108;
pub(super) const FULL_SIZE: usize = 0x310;
/// Début de la carte dans un fichier « full ».
pub(super) const FULL_START: usize = FULL_SIZE - WC_SIZE;

pub(super) fn u16le(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

pub(super) fn u32le(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn words(d: &[u8]) -> impl Iterator<Item = u16> + '_ {
    d.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]))
}

/// Chaîne Gen 4 (table de caractères propre au jeu, terminateur 0xFFFF).
pub(super) fn text4(d: &[u8]) -> String {
    let codes: Vec<u16> = words(d).take_while(|&c| c != 0xFFFF && c != 0).collect();
    crate::text::gen4::decode(&codes)
}

/// Chaîne Gen 5 (UTF-16, terminateur 0xFFFF, parfois 0).
pub(super) fn text5(d: &[u8]) -> String {
    let codes: Vec<u16> = words(d).take_while(|&c| c != 0xFFFF && c != 0).collect();
    crate::text::gen5::decode(&codes)
}

/// Chaîne 3DS (UTF-16, terminateur 0, ♂/♀ en 0xE08E/0xE08F).
pub(super) fn text3ds(d: &[u8]) -> String {
    let units = words(d).take_while(|&c| c != 0).map(|c| match c {
        0xE08E => 0x2642,
        0xE08F => 0x2640,
        c => c,
    });
    char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
}

/// Ordre des statistiques des cartes (PV, Att, Déf, Vit, Atq Spé, Déf Spé) vers l'ordre
/// Kaleido (PV, Att, Déf, Atq Spé, Déf Spé, Vit).
pub(super) fn ivs_to_kaleido(card: [u8; 6]) -> [u8; 6] {
    [card[0], card[1], card[2], card[4], card[5], card[3]]
}

pub(super) fn date_ok(year: u16, month: u8, day: u8) -> Option<PkmDate> {
    (2000..2100).contains(&year).then_some(()).filter(|_| (1..=12).contains(&month) && (1..=31).contains(&day))?;
    Some(PkmDate { year, month, day })
}

/// Rubans d'événement : (octet, bit) dans chaque format de carte, dans l'ordre de [`RIBBONS`].
// Vérifié : PKHeX PGF.cs (RIB0 0x0C : Country, National, Earth, World, Classic, Premier, Event,
// Birthday ; RIB1 0x0D : Special, Souvenir, Wishing, ChampionBattle, ChampionRegional,
// ChampionNational, ChampionWorld), WC6.cs / WC7.cs (RIB0 0x74 : ChampionBattle, ChampionRegional,
// ChampionNational, Country, National, Earth, World, Event ; RIB1 0x75 : ChampionWorld, Birthday,
// Special, Souvenir, Wishing, Classic, Premier).
pub(super) const RIBBON_BITS_PGF: [(usize, u8); 15] =
    [(0x0C, 0), (0x0C, 1), (0x0C, 2), (0x0C, 3), (0x0C, 4), (0x0C, 5), (0x0C, 6), (0x0C, 7), (0x0D, 0), (0x0D, 1), (0x0D, 2), (0x0D, 3), (0x0D, 4), (0x0D, 5), (0x0D, 6)];
pub(super) const RIBBON_BITS_WC: [(usize, u8); 15] =
    [(0x74, 3), (0x74, 4), (0x74, 5), (0x74, 6), (0x75, 5), (0x75, 6), (0x74, 7), (0x75, 1), (0x75, 2), (0x75, 3), (0x75, 4), (0x74, 0), (0x74, 1), (0x74, 2), (0x75, 0)];
/// Mêmes rubans dans un PK4 / PK5.
// Vérifié : PKHeX PK4.cs / PK5.cs (RIB2 0x26 : Event bit 3, ChampionWorld 5, Birthday 6, Special 7 ;
// RIB3 0x27 : Souvenir 0, Wishing 1, Classic 2, Premier 3 ; RIB7 0x3F : ChampionBattle 1,
// ChampionRegional 2, ChampionNational 3, Country 4, National 5, Earth 6, World 7).
pub(super) const RIBBON_BITS_PK45: [(usize, u8); 15] =
    [(0x3F, 4), (0x3F, 5), (0x3F, 6), (0x3F, 7), (0x27, 2), (0x27, 3), (0x26, 3), (0x26, 6), (0x26, 7), (0x27, 0), (0x27, 1), (0x3F, 1), (0x3F, 2), (0x3F, 3), (0x26, 5)];
/// Mêmes rubans dans un PK6 / PK7.
// Vérifié : PKHeX PK6.cs / PK7.cs (RIB2 0x32 : Country bit 6, National 7 ; RIB3 0x33 : Earth 0,
// World 1, Classic 2, Premier 3, Event 4, Birthday 5, Special 6, Souvenir 7 ; RIB4 0x34 :
// Wishing 0, ChampionBattle 1, ChampionRegional 2, ChampionNational 3, ChampionWorld 4).
pub(super) const RIBBON_BITS_PK67: [(usize, u8); 15] =
    [(0x32, 6), (0x32, 7), (0x33, 0), (0x33, 1), (0x33, 2), (0x33, 3), (0x33, 4), (0x33, 5), (0x33, 6), (0x33, 7), (0x34, 0), (0x34, 1), (0x34, 2), (0x34, 3), (0x34, 4)];

pub(super) fn ribbons_from(d: &[u8], bits: &[(usize, u8); 15]) -> Vec<&'static str> {
    RIBBONS.iter().zip(bits).filter(|(_, &(at, bit))| d[at] >> bit & 1 != 0).map(|(k, _)| *k).collect()
}

/// Résultat de la lecture d'une carte.
pub(super) struct Parsed {
    pub card_id: u16,
    pub title: String,
    pub kind: GiftKind,
    pub kind_detail: Option<String>,
    pub date: Option<PkmDate>,
    pub games: Vec<u8>,
    pub pokemon: Option<GiftPokemon>,
    pub items: Vec<GiftItem>,
    pub template4: Option<Pokemon>,
}

// --- Gen 4 ---------------------------------------------------------------------------------

/// Types de cadeau Gen 4 (`GiftType4` de PKHeX).
pub(super) mod gift4 {
    pub const POKEMON: u16 = 1;
    pub const EGG: u16 = 2;
    pub const ITEM: u16 = 3;
    pub const MANAPHY_EGG: u16 = 7;
    pub const MOVIE: u16 = 13;
}

fn kind4_label(kind: u16, sub: u32) -> String {
    match kind {
        4 => "Règles de combat".into(),
        5 => "Goodies (décoration)".into(),
        6 => match sub {
            1 => "Sceau".into(),
            2 => "Accessoire".into(),
            3 => "Fond de Concours".into(),
            _ => "Décoration".into(),
        },
        8 => "Carte Membre".into(),
        9 => "Lettre de Chen".into(),
        10 => "Flûte Azur".into(),
        11 => "Application Pokémontre".into(),
        12 => "Clé Secrète".into(),
        14 => "Parcours Pokéwalker".into(),
        15 => "Photo souvenir".into(),
        n => format!("Type de cadeau n°{n}"),
    }
}

/// Jeux Gen 4 : un cadeau Pokémon est lié à la version de son modèle quand elle est
/// renseignée (comme `PCD.CanBeReceivedByVersion` de PKHeX) ; sinon tous les jeux DS Gen 4.
fn games4(template_version: u8) -> Vec<u8> {
    const ALL: [u8; 5] = [10, 11, 12, 7, 8];
    if ALL.contains(&template_version) {
        vec![template_version]
    } else {
        ALL.to_vec()
    }
}

/// Lit un PGT (0x104 octets) ; `card` contient au moins ces octets.
pub(super) fn parse_pgt(card: &[u8]) -> Result<Parsed, GiftError> {
    let kind = u16le(card, 0);
    let item = u32le(card, 4);
    let gift = &card[8..8 + PkmFormat::Gen4.party_size()];
    let has_pk = gift.iter().any(|&b| b != 0);
    let template = if has_pk && matches!(kind, gift4::POKEMON | gift4::EGG | gift4::MOVIE) {
        Some(Pokemon::from_bytes(PkmFormat::Gen4, gift)?)
    } else {
        None
    };
    let mut parsed = Parsed {
        card_id: 0,
        title: String::new(),
        kind: GiftKind::Other,
        kind_detail: None,
        date: None,
        games: games4(0),
        pokemon: None,
        items: Vec::new(),
        template4: None,
    };
    match kind {
        gift4::MANAPHY_EGG => {
            parsed.kind = GiftKind::Egg;
            parsed.pokemon = Some(GiftPokemon::manaphy_egg());
        }
        gift4::POKEMON | gift4::EGG | gift4::MOVIE => {
            let p = template.ok_or_else(|| GiftError::Invalid("cadeau Pokémon sans Pokémon".into()))?;
            let info = GiftPokemon::from_pk4(&p, kind == gift4::EGG);
            parsed.kind = if info.egg { GiftKind::Egg } else { GiftKind::Pokemon };
            parsed.games = games4(p.version());
            parsed.pokemon = Some(info);
            parsed.template4 = Some(p);
        }
        gift4::ITEM => {
            parsed.kind = GiftKind::Item;
            parsed.items.push(GiftItem { id: item as u16, count: 1 });
        }
        other => {
            parsed.kind_detail = Some(kind4_label(other, item));
        }
    }
    Ok(parsed)
}

pub(super) fn parse_pcd(card: &[u8]) -> Result<Parsed, GiftError> {
    let mut parsed = parse_pgt(&card[..PGT_SIZE])?;
    parsed.title = text4(&card[0x104..0x104 + 0x48]).trim().to_string();
    parsed.card_id = u16le(card, 0x150);
    // Compatibilité de la carte : un bit par version (`1 << GameVersion`), ex. 0x0C00 =
    // Diamant + Perle, 0x1D80 = toute la Gen 4 (relevé sur la base de PKHeX).
    let compat = u16le(card, 0x14C);
    let games: Vec<u8> = [10u8, 11, 12, 7, 8].into_iter().filter(|v| compat >> v & 1 != 0).collect();
    if !games.is_empty() {
        parsed.games = games;
    }
    Ok(parsed)
}

// --- Gen 5 ---------------------------------------------------------------------------------

/// Versions Gen 5 admises (bits : 1 Blanche, 2 Noire, 4 Blanche 2, 8 Noire 2).
pub(super) fn games5(origin: u8, restrict: u8) -> Vec<u8> {
    if origin != 0 {
        return vec![origin];
    }
    (0..4u8).filter(|i| restrict == 0 || restrict >> i & 1 != 0).map(|i| 20 + i).collect()
}

pub(super) fn parse_pgf(c: &[u8], restrict_version: u8) -> Result<Parsed, GiftError> {
    let card_type = c[0xB3];
    let (year, month, day) = (u16le(c, 0xAE), c[0xAD], c[0xAC]);
    let mut parsed = Parsed {
        card_id: u16le(c, 0xB0),
        title: text5(&c[0x60..0x60 + 37 * 2]).trim().to_string(),
        kind: GiftKind::Other,
        kind_detail: None,
        date: date_ok(year, month, day),
        games: games5(c[0x04], restrict_version),
        pokemon: None,
        items: Vec::new(),
        template4: None,
    };
    match card_type {
        1 => {
            let info = GiftPokemon::from_pgf(c);
            parsed.kind = if info.egg { GiftKind::Egg } else { GiftKind::Pokemon };
            parsed.pokemon = Some(info);
        }
        2 => {
            parsed.kind = GiftKind::Item;
            parsed.items.push(GiftItem { id: u16le(c, 0), count: 1 });
        }
        3 => parsed.kind_detail = Some("Pouvoir Pass (Entralink)".into()),
        n => parsed.kind_detail = Some(format!("Type de carte n°{n}")),
    }
    Ok(parsed)
}

// --- Gen 6 / 7 -----------------------------------------------------------------------------

/// Versions 3DS admises (bits : X/Y/Saphir Alpha/Rubis Oméga ou Soleil/Lune/Ultra-Soleil/Ultra-Lune).
pub(super) fn games3ds(gen7: bool, card_id: u16, restrict: u8) -> Vec<u8> {
    let base = if gen7 { 30 } else { 24 };
    // Correction PKHeX (WC7.CanBeReceivedByVersion) : la carte 2046 (Amphinobi-Sacha) n'est
    // valable que dans Soleil et Lune.
    if gen7 && card_id == 2046 {
        return vec![30, 31];
    }
    (0..4u8).filter(|i| restrict == 0 || restrict >> i & 1 != 0).map(|i| base + i).collect()
}

/// Date d'une carte 3DS : `AAAAMMJJ` en Gen 6, `AAMMJJ` (an - 2000) en Gen 7.
pub(super) fn date3ds(c: &[u8], gen7: bool) -> Option<PkmDate> {
    let raw = u32le(c, 0x4C);
    let year = raw / 10000 + if gen7 { 2000 } else { 0 };
    date_ok(year.min(u16::MAX as u32) as u16, (raw % 10000 / 100) as u8, (raw % 100) as u8)
}

pub(super) fn parse_wc(c: &[u8], gen7: bool, restrict_version: u8) -> Result<Parsed, GiftError> {
    let card_id = u16le(c, 0);
    let card_type = c[0x51];
    let mut parsed = Parsed {
        card_id,
        title: text3ds(&c[2..2 + 0x4A]).trim().to_string(),
        kind: GiftKind::Other,
        kind_detail: None,
        date: date3ds(c, gen7),
        games: games3ds(gen7, card_id, restrict_version),
        pokemon: None,
        items: Vec::new(),
        template4: None,
    };
    match card_type {
        0 => {
            let info = GiftPokemon::from_wc(c, gen7);
            parsed.kind = if info.egg { GiftKind::Egg } else { GiftKind::Pokemon };
            parsed.pokemon = Some(info);
        }
        1 => {
            parsed.kind = GiftKind::Item;
            if gen7 {
                // Jusqu'à 6 objets (id, quantité) à partir de 0x68.
                for i in 0..6 {
                    let id = u16le(c, 0x68 + 4 * i);
                    if id == 0 {
                        break;
                    }
                    parsed.items.push(GiftItem { id, count: u16le(c, 0x6A + 4 * i).max(1) });
                }
            } else {
                parsed.items.push(GiftItem { id: u16le(c, 0x68), count: u16le(c, 0x70).max(1) });
            }
        }
        2 if gen7 => parsed.kind_detail = Some(format!("Poké Haricots × {}", u16le(c, 0x68))),
        2 => parsed.kind_detail = Some("O-Aura".into()),
        3 if gen7 => parsed.kind_detail = Some(format!("{} Points Combat", u16le(c, 0x68))),
        n => parsed.kind_detail = Some(format!("Type de carte n°{n}")),
    }
    Ok(parsed)
}

// --- Détails Pokémon -----------------------------------------------------------------------

impl GiftPokemon {
    /// Œuf de Manaphy (Pokémon Ranger) : aucun Pokémon modèle, tout est fixé par le jeu.
    fn manaphy_egg() -> Self {
        GiftPokemon {
            species: 490,
            form: 0,
            level: 1,
            met_level: 1,
            egg: true,
            moves: [294, 145, 346, 0],
            relearn: [0; 4],
            held_item: 0,
            ball: 4,
            ot_name: None,
            ot_gender: None,
            tid: None,
            sid: None,
            shiny: ShinyRule::Random,
            pid: None,
            nature: None,
            gender: Some(Gender::Genderless),
            ability: AbilityRule::OneOrTwo,
            ivs: [None; 6],
            perfect_ivs: 0,
            met_location: 0,
            egg_location: 3001,
            language: 0,
            nickname: None,
            origin_game: 0,
            fateful: true,
            ribbons: Vec::new(),
            evs: [0; 6],
        }
    }

    fn from_pk4(p: &Pokemon, egg_gift: bool) -> Self {
        let pid = p.pid();
        let ot = p.ot_name();
        let fixed_pid = pid > 1;
        let shiny = if egg_gift {
            ShinyRule::Random
        } else if !fixed_pid || !p.is_shiny() {
            ShinyRule::Never
        } else if (p.tid() ^ p.sid()) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF) == 0 {
            ShinyRule::AlwaysSquare
        } else {
            ShinyRule::AlwaysStar
        };
        let has_ivs = p.ivs().iter().any(|&v| v != 0);
        let raw = p.data();
        let egg = egg_gift || p.is_egg();
        // Le modèle range le lieu de rencontre dans le champ « lieu de l'œuf », moins 3000
        // (PKHeX PCD.Location / PGT.SetMetData).
        let stored = p.egg_location();
        GiftPokemon {
            species: p.species(),
            form: p.form(),
            level: p.party_level().unwrap_or_else(|| p.met_level()).max(1),
            met_level: p.met_level(),
            egg,
            moves: p.moves(),
            relearn: [0; 4],
            held_item: p.held_item(),
            ball: p.ball(),
            ot_name: (!ot.is_empty()).then_some(ot.clone()),
            ot_gender: (!ot.is_empty()).then(|| p.ot_gender()),
            tid: (!ot.is_empty()).then(|| p.tid()),
            sid: (!ot.is_empty()).then(|| p.sid()),
            shiny,
            pid: fixed_pid.then_some(pid),
            nature: fixed_pid.then_some((pid % 25) as u8),
            gender: Some(p.gender()),
            ability: AbilityRule::Fixed((pid & 1) as u8),
            ivs: if has_ivs { p.ivs().map(Some) } else { [None; 6] },
            perfect_ivs: 0,
            met_location: if egg { 0 } else { stored + 3000 },
            egg_location: if egg { stored + 3000 } else { 0 },
            language: p.language(),
            nickname: p.is_nicknamed().then(|| p.nickname()),
            origin_game: p.version(),
            fateful: p.fateful_encounter(),
            ribbons: ribbons_from(raw, &RIBBON_BITS_PK45),
            evs: p.evs(),
        }
    }

    fn from_pgf(c: &[u8]) -> Self {
        let egg = c[0x5C] == 1;
        let pid = u32le(c, 0x08);
        let (tid, sid) = (u16le(c, 0x00), u16le(c, 0x02));
        let pid_type = c[0x37];
        let shiny = if egg {
            ShinyRule::Random
        } else if pid != 0 {
            let xor = (tid ^ sid) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF);
            match xor {
                0 => ShinyRule::AlwaysSquare,
                1..=7 => ShinyRule::AlwaysStar,
                _ => ShinyRule::Never,
            }
        } else {
            match pid_type {
                0 => ShinyRule::Never,
                2 => ShinyRule::Always,
                _ => ShinyRule::Random,
            }
        };
        let ivs_card: [u8; 6] = std::array::from_fn(|i| c[0x43 + i]);
        let ot_gender = c[0x5A];
        let nickname = text5(&c[0x1E..0x1E + 22]);
        GiftPokemon {
            species: u16le(c, 0x1A),
            form: c[0x1C],
            level: c[0x5B],
            met_level: c[0x3C],
            egg,
            moves: std::array::from_fn(|i| u16le(c, 0x12 + 2 * i)),
            relearn: [0; 4],
            held_item: u16le(c, 0x10),
            ball: c[0x0E],
            ot_name: (!egg).then(|| text5(&c[0x4A..0x4A + 16])),
            ot_gender: (!egg && ot_gender < 3).then_some(if ot_gender & 1 == 1 { Gender::Female } else { Gender::Male }),
            tid: (!egg).then_some(tid),
            sid: (!egg).then_some(sid),
            shiny,
            pid: (pid != 0).then_some(pid),
            nature: (c[0x34] < 25).then_some(c[0x34]),
            gender: match c[0x35] {
                0 => Some(Gender::Male),
                1 => Some(Gender::Female),
                _ => None,
            },
            ability: AbilityRule::from_type(c[0x36]),
            ivs: card_ivs(ivs_card),
            perfect_ivs: perfect_count(ivs_card),
            met_location: u16le(c, 0x3A),
            egg_location: u16le(c, 0x38),
            language: c[0x1D],
            nickname: (!nickname.is_empty()).then_some(nickname),
            origin_game: c[0x04],
            fateful: true,
            ribbons: ribbons_from(c, &RIBBON_BITS_PGF),
            evs: [0; 6],
        }
    }

    fn from_wc(c: &[u8], gen7: bool) -> Self {
        let egg = c[0xD1] == 1;
        let (tid, sid) = (u16le(c, 0x68), u16le(c, 0x6A));
        let pid = u32le(c, 0xD4);
        let pid_type = c[0xA3];
        let ot_gender = c[0xB5];
        let ot_set = c[0xB6] != 0 || c[0xB7] != 0;
        let player_ids = ot_gender == 3;
        let shiny = if egg {
            ShinyRule::Random
        } else {
            match pid_type {
                // PID fixe : chromatique selon le XOR avec l'ID de la carte (« carré » si nul).
                0 if tid as u32 | (sid as u32) << 16 == 0 => ShinyRule::Never,
                0 => match (tid ^ sid) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF) {
                    0 => ShinyRule::AlwaysSquare,
                    1..=15 => ShinyRule::AlwaysStar,
                    _ => ShinyRule::Never,
                },
                1 => ShinyRule::Random,
                2 => ShinyRule::Always,
                _ => ShinyRule::Never,
            }
        };
        let ivs_card: [u8; 6] = std::array::from_fn(|i| c[0xAF + i]);
        let nickname = text3ds(&c[0x86..0x86 + 0x1A]);
        let level = c[0xD0];
        let met_level = if gen7 && c[0xA8] != 0 { c[0xA8] } else { level };
        GiftPokemon {
            species: u16le(c, 0x82),
            form: c[0x84],
            level,
            met_level,
            egg,
            moves: std::array::from_fn(|i| u16le(c, 0x7A + 2 * i)),
            relearn: std::array::from_fn(|i| u16le(c, 0xD8 + 2 * i)),
            held_item: u16le(c, 0x78),
            ball: c[0x76],
            ot_name: ot_set.then(|| text3ds(&c[0xB6..0xB6 + 0x1A])),
            ot_gender: (!player_ids).then_some(if ot_gender & 1 == 1 { Gender::Female } else { Gender::Male }),
            tid: (!player_ids).then_some(tid),
            sid: (!player_ids).then_some(sid),
            shiny,
            pid: (pid_type == 0).then_some(pid),
            nature: (c[0xA0] < 25).then_some(c[0xA0]),
            gender: match c[0xA1] {
                0 => Some(Gender::Male),
                1 => Some(Gender::Female),
                2 => Some(Gender::Genderless),
                _ => None,
            },
            ability: AbilityRule::from_type(c[0xA2]),
            ivs: card_ivs(ivs_card),
            perfect_ivs: perfect_count(ivs_card),
            met_location: u16le(c, 0xA6),
            egg_location: u16le(c, 0xA4),
            language: c[0x85],
            nickname: (!nickname.is_empty()).then_some(nickname),
            origin_game: c[0x6C],
            // Correction PKHeX (WC6.FatefulEncounter) : les cadeaux « Link » (lieu 30011)
            // ne sont pas des rencontres fatidiques.
            fateful: gen7 || u16le(c, 0xA6) != 30011,
            ribbons: ribbons_from(c, &RIBBON_BITS_WC),
            evs: ivs_to_kaleido(std::array::from_fn(|i| c[0xE5 + i])),
        }
    }
}

/// IV fixés (ordre Kaleido) ; `None` = tiré au hasard.
fn card_ivs(card: [u8; 6]) -> [Option<u8>; 6] {
    ivs_to_kaleido(card).map(|v| (v <= 31).then_some(v))
}

/// Nombre d'IV parfaits garantis : codes 0xFC à 0xFE (1 à 3) d'un IV aléatoire.
fn perfect_count(card: [u8; 6]) -> u8 {
    card.iter().filter(|&&v| (0xFC..=0xFE).contains(&v)).map(|&v| v - 0xFB).max().unwrap_or(0)
}

/// Découpe un fichier selon son format (taille vérifiée par l'appelant).
pub(super) fn parse(format: GiftFormat, file: &[u8], restrict_version: u8) -> Result<Parsed, GiftError> {
    match format {
        GiftFormat::Pgt => parse_pgt(file),
        GiftFormat::Pcd => parse_pcd(file),
        GiftFormat::Pgf => parse_pgf(file, restrict_version),
        GiftFormat::Wc6 => parse_wc(file, false, restrict_version),
        GiftFormat::Wc7 => parse_wc(file, true, restrict_version),
        GiftFormat::Wc6Full => parse_wc(&file[FULL_START..], false, file[0]),
        GiftFormat::Wc7Full => parse_wc(&file[FULL_START..], true, file[0]),
    }
}
