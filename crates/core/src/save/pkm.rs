//! Pokémon des Gen 4 à 7 : formats PK4, PK5, PK6 et PK7 (d'après PKHeX.Core).
//!
//! Structure commune : un en-tête de 8 octets (PID ou constante de chiffrement,
//! drapeaux, somme de contrôle), quatre blocs A/B/C/D mélangés puis chiffrés, et,
//! pour les Pokémon de l'équipe seulement, une section de statistiques chiffrée à part.
//!
//! | Format | Bloc | Stocké (boîte) | Équipe |
//! |---|---|---|---|
//! | PK4 | 32 | 136 | 236 |
//! | PK5 | 32 | 136 | 220 |
//! | PK6 / PK7 | 56 | 232 | 260 |
//!
//! - **Gen 4/5** : ordre des blocs = `((PID >> 13) & 31) % 24` ; les blocs sont chiffrés
//!   par le LCRNG `0x41C64E6D / 0x6073` initialisé avec la somme de contrôle, la section
//!   d'équipe avec le PID.
//! - **Gen 6/7** : la constante de chiffrement (EC, offset 0) joue les deux rôles.
//!
//! La somme de contrôle est la somme des mots de 16 bits des quatre blocs déchiffrés.
//!
//! Offsets vérifiés sur le code de PKHeX (`PK4.cs` à `PK7.cs`, `G4PKM.cs`, `G6PKM.cs`,
//! `PokeCrypto.cs`) et contrôlés sur de vrais fichiers `.pk4` à `.pk7` ainsi que sur la
//! sauvegarde Soleil/Lune des tests de PKHeX (voir `pkhex_tests.rs`).

use serde::Serialize;

use super::stats::{self, GrowthRate};
use super::strings;
use crate::pokemon::BaseStats;
use crate::text::TextError;

const LCRNG_MUL: u32 = 0x41C6_4E6D;
const LCRNG_ADD: u32 = 0x6073;
const HEADER: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum PkmError {
    #[error("données de Pokémon invalides : {got} octets pour le format {format} (attendu {stored} ou {party})")]
    BadSize { format: PkmFormat, got: usize, stored: usize, party: usize },
    #[error("texte trop long : {len} caractères (maximum {max})")]
    TooLong { len: usize, max: usize },
    #[error(transparent)]
    Text(#[from] TextError),
    #[error("en Gen 4, la nature découle du PID : modifie le PID à la place")]
    NatureFromPid,
    #[error("valeur hors limites pour « {field} » : {value}")]
    OutOfRange { field: &'static str, value: u32 },
    #[error("{0}")]
    Invalid(String),
}

/// Format de Pokémon (lié à la génération de la sauvegarde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PkmFormat {
    Gen4,
    Gen5,
    Gen6,
    Gen7,
}

impl PkmFormat {
    pub fn generation(self) -> u8 {
        match self {
            PkmFormat::Gen4 => 4,
            PkmFormat::Gen5 => 5,
            PkmFormat::Gen6 => 6,
            PkmFormat::Gen7 => 7,
        }
    }

    /// Taille d'un Pokémon en boîte.
    pub fn stored_size(self) -> usize {
        if self.is_ds() {
            136
        } else {
            232
        }
    }

    /// Taille d'un Pokémon d'équipe (données stockées + statistiques).
    pub fn party_size(self) -> usize {
        match self {
            PkmFormat::Gen4 => 236,
            PkmFormat::Gen5 => 220,
            PkmFormat::Gen6 | PkmFormat::Gen7 => 260,
        }
    }

    /// Taille de chacun des quatre blocs mélangés.
    pub fn block_size(self) -> usize {
        if self.is_ds() {
            32
        } else {
            56
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PkmFormat::Gen4 => "PK4",
            PkmFormat::Gen5 => "PK5",
            PkmFormat::Gen6 => "PK6",
            PkmFormat::Gen7 => "PK7",
        }
    }

    fn is_ds(self) -> bool {
        matches!(self, PkmFormat::Gen4 | PkmFormat::Gen5)
    }

    fn shiny_threshold(self) -> u32 {
        if self.is_ds() {
            8
        } else {
            16
        }
    }

    fn ofs(self) -> &'static Ofs {
        if self.is_ds() {
            &OFS_DS
        } else {
            &OFS_3DS
        }
    }
}

impl std::fmt::Display for PkmFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
    Genderless,
}

impl Gender {
    fn from_bits(bits: u8) -> Self {
        match bits & 3 {
            0 => Gender::Male,
            1 => Gender::Female,
            _ => Gender::Genderless,
        }
    }

    fn bits(self) -> u8 {
        match self {
            Gender::Male => 0,
            Gender::Female => 1,
            Gender::Genderless => 2,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Gender::Male => "♂",
            Gender::Female => "♀",
            Gender::Genderless => "",
        }
    }
}

/// Offsets des champs qui changent entre DS (PK4/PK5) et 3DS (PK6/PK7).
struct Ofs {
    ability: usize,
    /// Rencontre fatidique (bit 0), sexe (bits 1-2), forme (bits 3-7).
    flags: usize,
    evs: usize,
    moves: usize,
    pp: usize,
    pp_ups: usize,
    iv32: usize,
    nickname: usize,
    nickname_max: usize,
    ot: usize,
    ot_max: usize,
    egg_location: usize,
    met_location: usize,
    ball: usize,
    /// Niveau de rencontre (bits 0-6) et sexe du dresseur d'origine (bit 7).
    met_level: usize,
    version: usize,
    language: usize,
    status: usize,
    level: usize,
    hp: usize,
    stats: usize,
}

// Vérifié : PKHeX PKM/PK4.cs, PK5.cs, Shared/G4PKM.cs (Ability 0x15, octet 0x40 = rencontre fatidique /
// sexe / forme, EV 0x18, attaques 0x28, PP 0x30, PP Plus 0x34, IV32 0x38, surnom 0x48 sur 22 octets,
// OT 0x68 sur 16 octets, EggLocationDP 0x7E, MetLocationDP 0x80, BallDPPt 0x83, MetLevel / sexe du DO
// 0x84, Version 0x5F, Language 0x17, Status_Condition 0x88, Stat_Level 0x8C, PV 0x8E, stats 0x90) ;
// contrôlé sur de vrais .pk4 / .pk5 des tests de PKHeX.
const OFS_DS: Ofs = Ofs {
    ability: 0x15,
    flags: 0x40,
    evs: 0x18,
    moves: 0x28,
    pp: 0x30,
    pp_ups: 0x34,
    iv32: 0x38,
    nickname: 0x48,
    nickname_max: 10,
    ot: 0x68,
    ot_max: 7,
    egg_location: 0x7E,
    met_location: 0x80,
    ball: 0x83,
    met_level: 0x84,
    version: 0x5F,
    language: 0x17,
    status: 0x88,
    level: 0x8C,
    hp: 0x8E,
    stats: 0x90,
};

// Vérifié : PKHeX PKM/PK6.cs, PK7.cs, Shared/G6PKM.cs (Ability 0x14, octet 0x1D, EV 0x1E, surnom
// 0x40 sur 26 octets, attaques 0x5A, PP 0x62, PP Plus 0x66, IV32 0x74, OT 0xB0 sur 26 octets,
// EggLocation 0xD8, MetLocation 0xDA, Ball 0xDC, MetLevel / sexe du DO 0xDD, Version 0xDF,
// Language 0xE3, Status_Condition 0xE8, Stat_Level 0xEC, PV 0xF0, stats 0xF2) ; contrôlé sur de
// vrais .pk6 / .pk7 et sur les boîtes de la sauvegarde SL des tests de PKHeX.
const OFS_3DS: Ofs = Ofs {
    ability: 0x14,
    flags: 0x1D,
    evs: 0x1E,
    moves: 0x5A,
    pp: 0x62,
    pp_ups: 0x66,
    iv32: 0x74,
    nickname: 0x40,
    nickname_max: 12,
    ot: 0xB0,
    ot_max: 12,
    egg_location: 0xD8,
    met_location: 0xDA,
    ball: 0xDC,
    met_level: 0xDD,
    version: 0xDF,
    language: 0xE3,
    status: 0xE8,
    level: 0xEC,
    hp: 0xF0,
    stats: 0xF2,
};

// Champs propres à une génération.
// Vérifié : PKHeX PK4.cs (EggLocationExtended 0x44, MetLocationExtended 0x46, BallHGSS 0x86),
// PK5.cs (Nature 0x41, HiddenAbility bit 0 de 0x42), PK6.cs / PK7.cs (PID 0x18, Nature 0x1C,
// AbilityNumber 0x15, CurrentHandler 0x93, HandlingTrainerFriendship 0xA2,
// OriginalTrainerFriendship 0xCA), PK4.cs / PK5.cs (OriginalTrainerFriendship 0x14).
const G4_EGG_LOCATION_PT: usize = 0x44;
const G4_MET_LOCATION_PT: usize = 0x46;
const G4_BALL_HGSS: usize = 0x86;
const G5_NATURE: usize = 0x41;
const G5_HIDDEN_ABILITY: usize = 0x42;
const G67_PID: usize = 0x18;
const G67_NATURE: usize = 0x1C;
const G67_ABILITY_NUMBER: usize = 0x15;
const G67_HT_FRIENDSHIP: usize = 0xA2;
const G67_CURRENT_HANDLER: usize = 0x93;
const G67_OT_FRIENDSHIP: usize = 0xCA;
const G45_FRIENDSHIP: usize = 0x14;

// Vérifié : PKHeX PK4.cs etc. (EV et statistiques : PV, Att, Déf, Vit, Atq Spé, Déf Spé).
/// Ordre des statistiques dans les jeux : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
/// `KALEIDO_FROM_GAME[i]` = index jeu de la statistique Kaleido `i`.
const KALEIDO_FROM_GAME: [usize; 6] = [0, 1, 2, 4, 5, 3];

fn to_kaleido_order<T: Copy>(game: [T; 6]) -> [T; 6] {
    std::array::from_fn(|i| game[KALEIDO_FROM_GAME[i]])
}

fn to_game_order<T: Copy + Default>(kaleido: [T; 6]) -> [T; 6] {
    let mut game = [T::default(); 6];
    for (i, &v) in kaleido.iter().enumerate() {
        game[KALEIDO_FROM_GAME[i]] = v;
    }
    game
}

/// Position chiffrée de chaque bloc A, B, C, D pour les 24 valeurs de mélange
/// (`BlockPosition` de PKHeX) : bloc clair `k` = bloc chiffré `BLOCK_POSITION[sv][k]`.
// Vérifié : PKHeX PKM/Util/PokeCrypto.cs (BlockPosition ; sv = (clé >> 13) & 31, les valeurs
// 24 à 31 y sont des copies de 0 à 7, d'où le `% 24`).
pub const BLOCK_POSITION: [[u8; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 3, 1, 2],
    [0, 2, 3, 1],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [2, 0, 1, 3],
    [3, 0, 1, 2],
    [2, 0, 3, 1],
    [3, 0, 2, 1],
    [1, 2, 0, 3],
    [1, 3, 0, 2],
    [2, 1, 0, 3],
    [3, 1, 0, 2],
    [2, 3, 0, 1],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
    [1, 3, 2, 0],
    [2, 1, 3, 0],
    [3, 1, 2, 0],
    [2, 3, 1, 0],
    [3, 2, 1, 0],
];

/// Valeur de mélange déduite du PID (Gen 4/5) ou de l'EC (Gen 6/7).
pub fn shuffle_value(key: u32) -> usize {
    ((key >> 13) & 31) as usize % 24
}

/// Ordre des blocs dans les données chiffrées, ex. `"ACDB"` pour la valeur 3.
pub fn encrypted_block_order(sv: usize) -> String {
    let pos = BLOCK_POSITION[sv % 24];
    let mut order = [' '; 4];
    for (k, &p) in pos.iter().enumerate() {
        order[p as usize] = (b'A' + k as u8) as char;
    }
    order.iter().collect()
}

// Vérifié : PKHeX PokeCrypto.cs (CryptArray : LCRNG 0x41C64E6D / 0x6073, mot ^= graine >> 16 ;
// Gen 4/5 : blocs avec la somme de contrôle, statistiques avec le PID ; Gen 6/7 : tout avec l'EC).
fn crypt(data: &mut [u8], mut seed: u32) {
    for w in data.as_chunks_mut::<2>().0 {
        seed = seed.wrapping_mul(LCRNG_MUL).wrapping_add(LCRNG_ADD);
        let v = u16::from_le_bytes([w[0], w[1]]) ^ (seed >> 16) as u16;
        w.copy_from_slice(&v.to_le_bytes());
    }
}

fn unshuffle(blocks: &mut [u8], sv: usize, bs: usize) {
    let src = blocks.to_vec();
    for (k, &p) in BLOCK_POSITION[sv].iter().enumerate() {
        let p = p as usize;
        blocks[k * bs..(k + 1) * bs].copy_from_slice(&src[p * bs..(p + 1) * bs]);
    }
}

fn shuffle(blocks: &mut [u8], sv: usize, bs: usize) {
    let src = blocks.to_vec();
    for (k, &p) in BLOCK_POSITION[sv].iter().enumerate() {
        let p = p as usize;
        blocks[p * bs..(p + 1) * bs].copy_from_slice(&src[k * bs..(k + 1) * bs]);
    }
}

fn le_u16(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn le_u32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Un Pokémon déchiffré. Le tampon a toujours la taille « équipe » ; pour un
/// Pokémon lu dans une boîte, la section de statistiques est à zéro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pokemon {
    format: PkmFormat,
    data: Vec<u8>,
}

/// Résumé sérialisable pour l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PokemonSummary {
    pub species: u16,
    pub form: u8,
    pub nickname: String,
    pub is_nicknamed: bool,
    pub is_egg: bool,
    pub level: u8,
    /// `true` si le niveau est estimé (Pokémon en boîte sans courbe d'expérience connue).
    pub level_estimated: bool,
    pub exp: u32,
    pub shiny: bool,
    pub gender: Gender,
    pub nature: u8,
    pub nature_name: &'static str,
    pub ability: u16,
    pub held_item: u16,
    pub moves: [u16; 4],
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit.
    pub ivs: [u8; 6],
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit.
    pub evs: [u8; 6],
    pub ot_name: String,
    pub tid: u16,
    pub sid: u16,
    pub ball: u8,
    pub friendship: u8,
    pub pid: u32,
    pub checksum_valid: bool,
}

impl Pokemon {
    /// Emplacement vide (espèce 0).
    pub fn blank(format: PkmFormat) -> Self {
        Self { format, data: vec![0; format.party_size()] }
    }

    fn check_size(format: PkmFormat, len: usize) -> Result<(), PkmError> {
        if len == format.stored_size() || len == format.party_size() {
            Ok(())
        } else {
            Err(PkmError::BadSize { format, got: len, stored: format.stored_size(), party: format.party_size() })
        }
    }

    /// Données déjà déchiffrées (taille boîte ou équipe).
    pub fn from_decrypted(format: PkmFormat, bytes: &[u8]) -> Result<Self, PkmError> {
        Self::check_size(format, bytes.len())?;
        let mut data = vec![0; format.party_size()];
        data[..bytes.len()].copy_from_slice(bytes);
        Ok(Self { format, data })
    }

    /// Données chiffrées, telles que stockées dans une sauvegarde.
    pub fn from_encrypted(format: PkmFormat, bytes: &[u8]) -> Result<Self, PkmError> {
        Self::check_size(format, bytes.len())?;
        let mut buf = bytes.to_vec();
        let bs = format.block_size();
        let end = HEADER + 4 * bs;
        let key = le_u32(&buf, 0);
        let block_seed = if format.is_ds() { le_u16(&buf, 6) as u32 } else { key };
        crypt(&mut buf[HEADER..end], block_seed);
        if buf.len() > end {
            crypt(&mut buf[end..], key);
        }
        unshuffle(&mut buf[HEADER..end], shuffle_value(key), bs);
        Self::from_decrypted(format, &buf)
    }

    /// Données chiffrées ou non : détection comme PKHeX, d'après des champs
    /// toujours nuls une fois déchiffrés (0x64 en Gen 4/5, terminateurs du surnom
    /// et du nom du dresseur en Gen 6/7).
    pub fn from_bytes(format: PkmFormat, bytes: &[u8]) -> Result<Self, PkmError> {
        Self::check_size(format, bytes.len())?;
        let encrypted = if format.is_ds() { le_u32(bytes, 0x64) != 0 } else { le_u16(bytes, 0x58) != 0 || le_u16(bytes, 0xC8) != 0 };
        if encrypted {
            Self::from_encrypted(format, bytes)
        } else {
            Self::from_decrypted(format, bytes)
        }
    }

    pub fn format(&self) -> PkmFormat {
        self.format
    }

    /// Données déchiffrées, taille équipe.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Données déchiffrées, taille boîte.
    pub fn stored_data(&self) -> &[u8] {
        &self.data[..self.format.stored_size()]
    }

    pub fn is_empty(&self) -> bool {
        self.species() == 0
    }

    /// Données chiffrées pour une boîte (somme de contrôle recalculée).
    pub fn encrypt_stored(&self) -> Vec<u8> {
        let mut out = self.encrypt_party();
        out.truncate(self.format.stored_size());
        out
    }

    /// Données chiffrées pour l'équipe (somme de contrôle recalculée).
    pub fn encrypt_party(&self) -> Vec<u8> {
        let mut buf = self.data.clone();
        let chk = self.calc_checksum();
        buf[6..8].copy_from_slice(&chk.to_le_bytes());
        let bs = self.format.block_size();
        let end = HEADER + 4 * bs;
        let key = le_u32(&buf, 0);
        shuffle(&mut buf[HEADER..end], shuffle_value(key), bs);
        let block_seed = if self.format.is_ds() { chk as u32 } else { key };
        crypt(&mut buf[HEADER..end], block_seed);
        crypt(&mut buf[end..], key);
        buf
    }

    pub fn calc_checksum(&self) -> u16 {
        let end = HEADER + 4 * self.format.block_size();
        self.data[HEADER..end].as_chunks::<2>().0.iter().fold(0u16, |acc, w| acc.wrapping_add(u16::from_le_bytes([w[0], w[1]])))
    }

    pub fn checksum(&self) -> u16 {
        self.u16(6)
    }

    pub fn checksum_valid(&self) -> bool {
        self.checksum() == self.calc_checksum()
    }

    pub fn refresh_checksum(&mut self) {
        let chk = self.calc_checksum();
        self.put_u16(6, chk);
    }

    // --- Accès bruts (le tampon a toujours la taille équipe : pas de débordement possible).

    fn u8(&self, at: usize) -> u8 {
        self.data[at]
    }

    fn u16(&self, at: usize) -> u16 {
        le_u16(&self.data, at)
    }

    fn u32(&self, at: usize) -> u32 {
        le_u32(&self.data, at)
    }

    fn put_u8(&mut self, at: usize, v: u8) {
        self.data[at] = v;
    }

    fn put_u16(&mut self, at: usize, v: u16) {
        self.data[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn put_u32(&mut self, at: usize, v: u32) {
        self.data[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    // --- Identité.

    /// Clé de chiffrement : PID en Gen 4/5, constante de chiffrement (EC) en Gen 6/7.
    pub fn encryption_constant(&self) -> u32 {
        self.u32(0)
    }

    pub fn set_encryption_constant(&mut self, v: u32) {
        self.put_u32(0, v);
    }

    pub fn pid(&self) -> u32 {
        self.u32(if self.format.is_ds() { 0 } else { G67_PID })
    }

    /// En Gen 4/5, changer le PID change aussi l'ordre des blocs (géré au chiffrement)
    /// et, en Gen 4, la nature.
    pub fn set_pid(&mut self, v: u32) {
        self.put_u32(if self.format.is_ds() { 0 } else { G67_PID }, v);
    }

    pub fn species(&self) -> u16 {
        self.u16(0x08)
    }

    pub fn set_species(&mut self, v: u16) {
        self.put_u16(0x08, v);
    }

    pub fn held_item(&self) -> u16 {
        self.u16(0x0A)
    }

    pub fn set_held_item(&mut self, v: u16) {
        self.put_u16(0x0A, v);
    }

    pub fn tid(&self) -> u16 {
        self.u16(0x0C)
    }

    pub fn set_tid(&mut self, v: u16) {
        self.put_u16(0x0C, v);
    }

    pub fn sid(&self) -> u16 {
        self.u16(0x0E)
    }

    pub fn set_sid(&mut self, v: u16) {
        self.put_u16(0x0E, v);
    }

    pub fn exp(&self) -> u32 {
        self.u32(0x10)
    }

    pub fn set_exp(&mut self, v: u32) {
        self.put_u32(0x10, v);
    }

    pub fn is_shiny(&self) -> bool {
        let pid = self.pid();
        let xor = (self.tid() ^ self.sid()) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF);
        xor < self.format.shiny_threshold()
    }

    pub fn ability(&self) -> u16 {
        self.u8(self.format.ofs().ability) as u16
    }

    pub fn set_ability(&mut self, v: u16) -> Result<(), PkmError> {
        let v = u8::try_from(v).map_err(|_| PkmError::OutOfRange { field: "talent", value: v as u32 })?;
        self.put_u8(self.format.ofs().ability, v);
        Ok(())
    }

    /// Emplacement du talent : 1, 2 ou 4 (caché). En Gen 4/5, déduit du PID.
    pub fn ability_number(&self) -> u8 {
        match self.format {
            PkmFormat::Gen4 => 1 << (self.pid() & 1),
            PkmFormat::Gen5 if self.u8(G5_HIDDEN_ABILITY) & 1 != 0 => 4,
            // Correction (PKHeX PKM.PIDAbility) : le bit 16 du PID ne compte que pour un Pokémon
            // né en Gen 5 (Blanche 20, Noire 21, Blanche 2 22, Noire 2 23) ; un Pokémon
            // transféré depuis la Gen 3/4 garde le bit 0.
            PkmFormat::Gen5 if (20..=23).contains(&self.version()) => 1 << ((self.pid() >> 16) & 1),
            PkmFormat::Gen5 => 1 << (self.pid() & 1),
            PkmFormat::Gen6 => self.u8(G67_ABILITY_NUMBER),
            // Correction (PKHeX PK7.AbilityNumber) : seuls les 3 bits de poids faible comptent.
            PkmFormat::Gen7 => self.u8(G67_ABILITY_NUMBER) & 7,
        }
    }

    /// Nature (0 = Hardi … 24 = Bizarre). En Gen 4, `PID % 25`.
    pub fn nature(&self) -> u8 {
        match self.format {
            PkmFormat::Gen4 => (self.pid() % 25) as u8,
            PkmFormat::Gen5 => self.u8(G5_NATURE),
            PkmFormat::Gen6 | PkmFormat::Gen7 => self.u8(G67_NATURE),
        }
    }

    pub fn set_nature(&mut self, v: u8) -> Result<(), PkmError> {
        if v >= 25 {
            return Err(PkmError::OutOfRange { field: "nature", value: v as u32 });
        }
        match self.format {
            PkmFormat::Gen4 => return Err(PkmError::NatureFromPid),
            PkmFormat::Gen5 => self.put_u8(G5_NATURE, v),
            PkmFormat::Gen6 | PkmFormat::Gen7 => self.put_u8(G67_NATURE, v),
        }
        Ok(())
    }

    pub fn fateful_encounter(&self) -> bool {
        self.u8(self.format.ofs().flags) & 1 != 0
    }

    pub fn set_fateful_encounter(&mut self, v: bool) {
        let at = self.format.ofs().flags;
        self.put_u8(at, (self.u8(at) & !1) | v as u8);
    }

    /// Sexe stocké. En Gen 4/5, le jeu le recalcule aussi depuis le PID : le modifier
    /// seul peut créer une incohérence.
    pub fn gender(&self) -> Gender {
        Gender::from_bits(self.u8(self.format.ofs().flags) >> 1)
    }

    pub fn set_gender(&mut self, g: Gender) {
        let at = self.format.ofs().flags;
        self.put_u8(at, (self.u8(at) & !0b110) | (g.bits() << 1));
    }

    pub fn form(&self) -> u8 {
        self.u8(self.format.ofs().flags) >> 3
    }

    pub fn set_form(&mut self, form: u8) -> Result<(), PkmError> {
        if form > 31 {
            return Err(PkmError::OutOfRange { field: "forme", value: form as u32 });
        }
        let at = self.format.ofs().flags;
        self.put_u8(at, (self.u8(at) & 0b111) | (form << 3));
        Ok(())
    }

    // --- Attaques.

    pub fn moves(&self) -> [u16; 4] {
        let at = self.format.ofs().moves;
        std::array::from_fn(|i| self.u16(at + 2 * i))
    }

    pub fn set_moves(&mut self, moves: [u16; 4]) {
        let at = self.format.ofs().moves;
        for (i, m) in moves.into_iter().enumerate() {
            self.put_u16(at + 2 * i, m);
        }
    }

    pub fn pp(&self) -> [u8; 4] {
        let at = self.format.ofs().pp;
        std::array::from_fn(|i| self.u8(at + i))
    }

    pub fn set_pp(&mut self, pp: [u8; 4]) {
        let at = self.format.ofs().pp;
        self.data[at..at + 4].copy_from_slice(&pp);
    }

    pub fn pp_ups(&self) -> [u8; 4] {
        let at = self.format.ofs().pp_ups;
        std::array::from_fn(|i| self.u8(at + i))
    }

    pub fn set_pp_ups(&mut self, ups: [u8; 4]) {
        let at = self.format.ofs().pp_ups;
        self.data[at..at + 4].copy_from_slice(&ups);
    }

    // --- EV / IV (ordre Kaleido : PV, Att, Déf, Atq Spé, Déf Spé, Vit).

    pub fn evs(&self) -> [u8; 6] {
        let at = self.format.ofs().evs;
        to_kaleido_order(std::array::from_fn(|i| self.u8(at + i)))
    }

    pub fn set_evs(&mut self, evs: [u8; 6]) {
        let at = self.format.ofs().evs;
        self.data[at..at + 6].copy_from_slice(&to_game_order(evs));
    }

    fn iv32(&self) -> u32 {
        self.u32(self.format.ofs().iv32)
    }

    fn set_iv32(&mut self, v: u32) {
        self.put_u32(self.format.ofs().iv32, v);
    }

    pub fn ivs(&self) -> [u8; 6] {
        let iv32 = self.iv32();
        to_kaleido_order(std::array::from_fn(|i| ((iv32 >> (5 * i)) & 31) as u8))
    }

    pub fn set_ivs(&mut self, ivs: [u8; 6]) -> Result<(), PkmError> {
        if let Some(&bad) = ivs.iter().find(|&&iv| iv > 31) {
            return Err(PkmError::OutOfRange { field: "IV", value: bad as u32 });
        }
        let packed = to_game_order(ivs).iter().enumerate().fold(0u32, |acc, (i, &iv)| acc | (iv as u32) << (5 * i));
        self.set_iv32((self.iv32() & 0xC000_0000) | packed);
        Ok(())
    }

    pub fn is_egg(&self) -> bool {
        self.iv32() >> 30 & 1 != 0
    }

    pub fn set_is_egg(&mut self, v: bool) {
        self.set_iv32((self.iv32() & !(1 << 30)) | (v as u32) << 30);
    }

    pub fn is_nicknamed(&self) -> bool {
        self.iv32() >> 31 != 0
    }

    pub fn set_is_nicknamed(&mut self, v: bool) {
        self.set_iv32((self.iv32() & !(1 << 31)) | (v as u32) << 31);
    }

    // --- Textes.

    pub fn nickname(&self) -> String {
        let o = self.format.ofs();
        strings::decode(self.format, &self.data[o.nickname..o.nickname + 2 * (o.nickname_max + 1)])
    }

    /// Écrit le surnom et active le drapeau « surnommé ».
    pub fn set_nickname(&mut self, name: &str) -> Result<(), PkmError> {
        let o = self.format.ofs();
        let bytes = strings::encode(self.format, name, o.nickname_max)?;
        self.data[o.nickname..o.nickname + bytes.len()].copy_from_slice(&bytes);
        self.set_is_nicknamed(true);
        Ok(())
    }

    pub fn ot_name(&self) -> String {
        let o = self.format.ofs();
        strings::decode(self.format, &self.data[o.ot..o.ot + 2 * (o.ot_max + 1)])
    }

    pub fn set_ot_name(&mut self, name: &str) -> Result<(), PkmError> {
        let o = self.format.ofs();
        let bytes = strings::encode(self.format, name, o.ot_max)?;
        self.data[o.ot..o.ot + bytes.len()].copy_from_slice(&bytes);
        Ok(())
    }

    /// Sexe du dresseur d'origine (bit 7 de l'octet du niveau de rencontre).
    pub fn ot_gender(&self) -> Gender {
        Gender::from_bits(self.u8(self.format.ofs().met_level) >> 7)
    }

    pub fn set_ot_gender(&mut self, g: Gender) {
        let at = self.format.ofs().met_level;
        let female = (g == Gender::Female) as u8;
        self.put_u8(at, (self.u8(at) & 0x7F) | female << 7);
    }

    // --- Rencontre.

    /// Poké Ball. Gen 4 : la plus grande des Balls HGSS (0x86) et DP/Pt (0x83).
    pub fn ball(&self) -> u8 {
        let main = self.u8(self.format.ofs().ball);
        if self.format == PkmFormat::Gen4 {
            // Correction (PKHeX G4PKM.Ball : Math.Max(BallHGSS, BallDPPt)) : on prenait la Ball
            // HGSS dès qu'elle était non nulle, même plus petite.
            main.max(self.u8(G4_BALL_HGSS))
        } else {
            main
        }
    }

    /// Gen 4 : les Balls propres à HGSS (≥ 17) sont affichées comme une Poké Ball dans DP/Pt.
    pub fn set_ball(&mut self, ball: u8) {
        let at = self.format.ofs().ball;
        if self.format == PkmFormat::Gen4 {
            const CHERISH: u8 = 16;
            const SPORT: u8 = 24;
            const POKE: u8 = 4;
            self.put_u8(at, if ball <= CHERISH { ball } else { POKE });
            // Correction (PKHeX G4PKM.Ball) : un Pokémon né dans HGSS (hors cadeau sans œuf)
            // a toujours sa Ball dans le champ HGSS, même une Ball de DP/Pt ; les autres y ont 0.
            // Écart : une Ball HGSS (> Cherish Ball) choisie pour un Pokémon d'un autre jeu est
            // gardée dans le champ HGSS au lieu d'être perdue.
            let born_in_hgss = self.is_hgss_origin() && (!self.fateful_encounter() || self.egg_location() != 0);
            let hgss = if born_in_hgss || ball > CHERISH {
                if ball <= SPORT {
                    ball
                } else {
                    POKE
                }
            } else {
                0
            };
            self.put_u8(G4_BALL_HGSS, hgss);
        } else {
            self.put_u8(at, ball);
        }
    }

    /// Gen 4 : version d'origine HeartGold (7) ou SoulSilver (8).
    fn is_hgss_origin(&self) -> bool {
        matches!(self.version(), 7 | 8)
    }

    /// Gen 4 : écrit un lieu (rencontre ou éclosion) comme `G4PKM.MetLocation` /
    /// `EggLocation` de PKHeX : un lieu propre à Platine/HGSS (112 à 1999, 2011 à 2999)
    /// met « Lieu lointain » (3002) dans le champ DP ; sinon le champ Pt/HGSS ne reçoit
    /// la valeur que pour un Pokémon venant de Platine ou HGSS.
    fn set_location_gen4(&mut self, dp_at: usize, ext_at: usize, v: u16) {
        const FARAWAY: u16 = 3002;
        let pt_hgss_only = (112..2000).contains(&v) || (2011..3000).contains(&v);
        let (dp, ext) = if v == 0 {
            (0, 0)
        } else if pt_hgss_only {
            (FARAWAY, v)
        } else if matches!(self.version(), 7 | 8 | 12) {
            (v, v)
        } else {
            (v, 0)
        };
        self.put_u16(dp_at, dp);
        self.put_u16(ext_at, ext);
    }

    /// Lieu de rencontre. Gen 4 : le champ Platine/HGSS (0x46) prime sur celui de DP (0x80).
    pub fn met_location(&self) -> u16 {
        let main = self.u16(self.format.ofs().met_location);
        if self.format == PkmFormat::Gen4 {
            match self.u16(G4_MET_LOCATION_PT) {
                0 => main,
                ext => ext,
            }
        } else {
            main
        }
    }

    /// Gen 4 : voir `set_location_gen4`.
    // Correction (PKHeX G4PKM.MetLocation) : on écrivait la même valeur dans les deux champs,
    // ce qui donnait un lieu inconnu de DP dans le champ DP. (Cas Gen 3 / Parc des Amis de
    // PKHeX non repris : le champ Pt/HGSS n'est alors rempli que pour Pt/HGSS.)
    pub fn set_met_location(&mut self, v: u16) {
        if self.format == PkmFormat::Gen4 {
            self.set_location_gen4(self.format.ofs().met_location, G4_MET_LOCATION_PT, v);
        } else {
            self.put_u16(self.format.ofs().met_location, v);
        }
    }

    pub fn egg_location(&self) -> u16 {
        let main = self.u16(self.format.ofs().egg_location);
        if self.format == PkmFormat::Gen4 {
            match self.u16(G4_EGG_LOCATION_PT) {
                0 => main,
                ext => ext,
            }
        } else {
            main
        }
    }

    // Correction (PKHeX G4PKM.EggLocation) : même règle que le lieu de rencontre.
    pub fn set_egg_location(&mut self, v: u16) {
        if self.format == PkmFormat::Gen4 {
            self.set_location_gen4(self.format.ofs().egg_location, G4_EGG_LOCATION_PT, v);
        } else {
            self.put_u16(self.format.ofs().egg_location, v);
        }
    }

    pub fn met_level(&self) -> u8 {
        self.u8(self.format.ofs().met_level) & 0x7F
    }

    pub fn set_met_level(&mut self, level: u8) -> Result<(), PkmError> {
        if level > stats::MAX_LEVEL {
            return Err(PkmError::OutOfRange { field: "niveau de rencontre", value: level as u32 });
        }
        let at = self.format.ofs().met_level;
        self.put_u8(at, (self.u8(at) & 0x80) | level);
        Ok(())
    }

    pub fn version(&self) -> u8 {
        self.u8(self.format.ofs().version)
    }

    pub fn set_version(&mut self, v: u8) {
        self.put_u8(self.format.ofs().version, v);
    }

    pub fn language(&self) -> u8 {
        self.u8(self.format.ofs().language)
    }

    pub fn set_language(&mut self, v: u8) {
        self.put_u8(self.format.ofs().language, v);
    }

    /// Bonheur actuel. Gen 6/7 : celui du dresseur actuel (d'origine ou non).
    pub fn friendship(&self) -> u8 {
        self.u8(self.friendship_offset())
    }

    pub fn set_friendship(&mut self, v: u8) {
        self.put_u8(self.friendship_offset(), v);
    }

    fn friendship_offset(&self) -> usize {
        if self.format.is_ds() {
            G45_FRIENDSHIP
        } else if self.u8(G67_CURRENT_HANDLER) == 0 {
            G67_OT_FRIENDSHIP
        } else {
            G67_HT_FRIENDSHIP
        }
    }

    // --- Section équipe.

    /// Niveau stocké dans la section équipe (`None` pour un Pokémon de boîte).
    pub fn party_level(&self) -> Option<u8> {
        Some(self.u8(self.format.ofs().level)).filter(|&l| l != 0)
    }

    /// Statistiques de la section équipe (ordre Kaleido), `None` si absentes.
    pub fn party_stats(&self) -> Option<[u16; 6]> {
        let at = self.format.ofs().stats;
        let stats: [u16; 6] = to_kaleido_order(std::array::from_fn(|i| self.u16(at + 2 * i)));
        (self.party_level().is_some() && stats[0] != 0).then_some(stats)
    }

    pub fn current_hp(&self) -> u16 {
        self.u16(self.format.ofs().hp)
    }

    pub fn set_current_hp(&mut self, hp: u16) {
        self.put_u16(self.format.ofs().hp, hp);
    }

    pub fn status_condition(&self) -> u32 {
        self.u32(self.format.ofs().status)
    }

    /// Écrit niveau et statistiques (ordre Kaleido) ; les PV actuels sont remis au maximum.
    pub fn set_party_stats(&mut self, level: u8, stats: [u16; 6]) {
        let o = self.format.ofs();
        self.put_u8(o.level, level.clamp(1, stats::MAX_LEVEL));
        for (i, s) in to_game_order(stats).into_iter().enumerate() {
            self.put_u16(o.stats + 2 * i, s);
        }
        self.put_u16(o.hp, stats[0]);
        self.put_u32(o.status, 0);
    }

    /// Recalcule la section équipe à partir de l'espèce (statistiques de base et courbe).
    pub fn update_party_stats(&mut self, base: &BaseStats, growth: GrowthRate) {
        let level = stats::level_from_exp(growth, self.exp());
        let stats = stats::calc_stats(base, level, self.ivs(), self.evs(), self.nature());
        self.set_party_stats(level, stats);
    }

    /// Niveau : celui de la section équipe, sinon déduit de l'expérience si la courbe est connue.
    pub fn level(&self, growth: Option<GrowthRate>) -> Option<u8> {
        self.party_level().or_else(|| growth.map(|g| stats::level_from_exp(g, self.exp())))
    }

    pub fn summary(&self, growth: Option<GrowthRate>) -> PokemonSummary {
        let (level, level_estimated) = match self.level(growth) {
            Some(l) => (l, false),
            None => (stats::level_from_exp(GrowthRate::MediumFast, self.exp()), true),
        };
        let nature = self.nature();
        PokemonSummary {
            species: self.species(),
            form: self.form(),
            nickname: self.nickname(),
            is_nicknamed: self.is_nicknamed(),
            is_egg: self.is_egg(),
            level,
            level_estimated,
            exp: self.exp(),
            shiny: self.is_shiny(),
            gender: self.gender(),
            nature,
            nature_name: stats::nature_name(nature),
            ability: self.ability(),
            held_item: self.held_item(),
            moves: self.moves(),
            ivs: self.ivs(),
            evs: self.evs(),
            ot_name: self.ot_name(),
            tid: self.tid(),
            sid: self.sid(),
            ball: self.ball(),
            friendship: self.friendship(),
            pid: self.pid(),
            checksum_valid: self.checksum_valid(),
        }
    }
}

// --- Champs de la fiche complète (offsets vérifiés : PKHeX PK4.cs, PK5.cs, PK6.cs, PK7.cs).

const G45_MARKINGS: usize = 0x16;
const G45_EGG_DATE: usize = 0x78;
const G45_MET_DATE: usize = 0x7B;
const G45_POKERUS: usize = 0x82;
const G6_MARKINGS: usize = 0x2A;
const G7_MARKINGS: usize = 0x16;
const G67_POKERUS: usize = 0x2B;
const G67_EGG_DATE: usize = 0xD1;
const G67_MET_DATE: usize = 0xD4;

/// Façon de rendre un Pokémon chromatique (raccourcis de l'éditeur de PKHeX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShinyMode {
    /// Retire le chromatique (nouveau PID).
    None,
    /// Chromatique « étoile » (nouveau PID, nature, sexe et talent conservés).
    Star,
    /// Chromatique « carré » : XOR nul.
    Square,
    /// Garde le PID et change l'ID secret : préserve la corrélation PID/IV des Gen 3 à 5.
    KeepPid,
}

/// Date de rencontre ou d'éclosion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct PkmDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

/// Générateur pseudo-aléatoire déterministe pour chercher un PID.
fn next_seed(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(LCRNG_MUL).wrapping_add(LCRNG_ADD);
    *seed
}

impl Pokemon {
    /// Marquages (cercle, triangle, carré, cœur, étoile, losange) : 0 = aucun,
    /// 1 = bleu (ou présent avant la Gen 7), 2 = rouge (Gen 7).
    pub fn markings(&self) -> [u8; 6] {
        match self.format {
            PkmFormat::Gen7 => {
                let v = self.u16(G7_MARKINGS);
                std::array::from_fn(|i| ((v >> (2 * i)) & 3) as u8)
            }
            f => {
                let v = self.u8(if f == PkmFormat::Gen6 { G6_MARKINGS } else { G45_MARKINGS });
                std::array::from_fn(|i| (v >> i) & 1)
            }
        }
    }

    pub fn set_markings(&mut self, marks: [u8; 6]) {
        match self.format {
            PkmFormat::Gen7 => {
                let v = marks.iter().enumerate().fold(0u16, |acc, (i, &m)| acc | ((m.min(2) as u16) << (2 * i)));
                self.put_u16(G7_MARKINGS, v);
            }
            f => {
                let v = marks.iter().enumerate().fold(0u8, |acc, (i, &m)| acc | (((m != 0) as u8) << i));
                self.put_u8(if f == PkmFormat::Gen6 { G6_MARKINGS } else { G45_MARKINGS }, v);
            }
        }
    }

    fn pokerus_offset(&self) -> usize {
        if self.format.is_ds() {
            G45_POKERUS
        } else {
            G67_POKERUS
        }
    }

    /// Pokérus : (souche 0-15, jours restants 0-15). Souche non nulle et 0 jour = guéri.
    pub fn pokerus(&self) -> (u8, u8) {
        let v = self.u8(self.pokerus_offset());
        (v >> 4, v & 0xF)
    }

    pub fn set_pokerus(&mut self, strain: u8, days: u8) {
        self.put_u8(self.pokerus_offset(), (strain & 0xF) << 4 | (days & 0xF));
    }

    fn read_date(&self, at: usize) -> Option<PkmDate> {
        let (y, m, d) = (self.u8(at), self.u8(at + 1), self.u8(at + 2));
        (m != 0 && d != 0).then_some(PkmDate { year: 2000 + y as u16, month: m, day: d })
    }

    fn write_date(&mut self, at: usize, date: Option<PkmDate>) {
        let (y, m, d) = date.map_or((0, 0, 0), |d| (d.year.saturating_sub(2000).min(255) as u8, d.month, d.day));
        self.put_u8(at, y);
        self.put_u8(at + 1, m);
        self.put_u8(at + 2, d);
    }

    pub fn met_date(&self) -> Option<PkmDate> {
        self.read_date(if self.format.is_ds() { G45_MET_DATE } else { G67_MET_DATE })
    }

    pub fn set_met_date(&mut self, date: Option<PkmDate>) {
        self.write_date(if self.format.is_ds() { G45_MET_DATE } else { G67_MET_DATE }, date);
    }

    pub fn egg_date(&self) -> Option<PkmDate> {
        self.read_date(if self.format.is_ds() { G45_EGG_DATE } else { G67_EGG_DATE })
    }

    pub fn set_egg_date(&mut self, date: Option<PkmDate>) {
        self.write_date(if self.format.is_ds() { G45_EGG_DATE } else { G67_EGG_DATE }, date);
    }

    /// Choisit l'emplacement de talent (1, 2 ou 4 = caché). Le numéro de talent
    /// lui-même doit être écrit à part ([`Self::set_ability`]).
    /// Gen 4 : pas de talent caché, l'emplacement vient du bit 0 du PID (PID recalculé).
    /// Gen 5 : drapeau « talent caché », sinon bit 16 du PID (PID recalculé).
    pub fn set_ability_number(&mut self, n: u8) -> Result<(), PkmError> {
        if !matches!(n, 1 | 2 | 4) || (n == 4 && self.format == PkmFormat::Gen4) {
            return Err(PkmError::OutOfRange { field: "emplacement de talent", value: n as u32 });
        }
        match self.format {
            PkmFormat::Gen6 => self.put_u8(G67_ABILITY_NUMBER, n),
            PkmFormat::Gen7 => {
                let v = self.u8(G67_ABILITY_NUMBER);
                self.put_u8(G67_ABILITY_NUMBER, (v & !7) | n);
            }
            PkmFormat::Gen5 => {
                let v = self.u8(G5_HIDDEN_ABILITY);
                self.put_u8(G5_HIDDEN_ABILITY, (v & !1) | (n == 4) as u8);
                if n != 4 && self.ability_number() != n {
                    self.reroll_pid(Some(self.is_shiny()), None, Some(n - 1));
                }
            }
            PkmFormat::Gen4 => {
                if self.ability_number() != n {
                    self.reroll_pid(Some(self.is_shiny()), Some(self.nature()), Some(n - 1));
                }
            }
        }
        Ok(())
    }

    /// Gen 4 : la nature vient du PID, qui est recalculé (chromatique, sexe et talent conservés).
    /// Autres générations : octet de nature.
    pub fn set_nature_any(&mut self, nature: u8) -> Result<(), PkmError> {
        if nature >= 25 {
            return Err(PkmError::OutOfRange { field: "nature", value: nature as u32 });
        }
        if self.format == PkmFormat::Gen4 {
            if self.nature() != nature {
                let slot = (self.ability_number() == 2) as u8;
                self.reroll_pid(Some(self.is_shiny()), Some(nature), Some(slot));
            }
            Ok(())
        } else {
            self.set_nature(nature)
        }
    }

    /// Rend le Pokémon chromatique ou non (voir [`ShinyMode`]).
    pub fn set_shiny(&mut self, mode: ShinyMode) {
        let gen4_nature = (self.format == PkmFormat::Gen4).then(|| self.nature());
        let slot = match self.format {
            PkmFormat::Gen4 | PkmFormat::Gen5 if self.ability_number() != 4 => Some((self.ability_number() == 2) as u8),
            _ => None,
        };
        match mode {
            ShinyMode::KeepPid => {
                let pid = self.pid();
                let sid = self.tid() ^ (pid >> 16) as u16 ^ (pid & 0xFFFF) as u16;
                self.set_sid(sid);
            }
            ShinyMode::Square => {
                // XOR nul impossible avec ces contraintes (Gen 5) : chromatique ordinaire.
                if !self.reroll_pid_inner(true, Some(0), gen4_nature, slot) {
                    self.reroll_pid_inner(true, None, gen4_nature, slot);
                }
            }
            ShinyMode::Star => {
                if !self.is_shiny() {
                    self.reroll_pid(Some(true), gen4_nature, slot);
                }
            }
            ShinyMode::None => {
                if self.is_shiny() {
                    self.reroll_pid(Some(false), gen4_nature, slot);
                }
            }
        }
    }

    fn reroll_pid(&mut self, shiny: Option<bool>, nature: Option<u8>, ability_slot: Option<u8>) {
        self.reroll_pid_inner(shiny == Some(true), None, nature, ability_slot);
    }

    /// Cherche un nouveau PID qui garde l'octet bas (sexe en Gen 3 à 5), et respecte
    /// chromatique / nature (Gen 4) / emplacement de talent (bit 0 en Gen 4, bit 16 en Gen 5).
    fn reroll_pid_inner(&mut self, shiny: bool, xor: Option<u32>, nature: Option<u8>, ability_slot: Option<u8>) -> bool {
        let old = self.pid();
        let tsv = (self.tid() ^ self.sid()) as u32;
        let threshold = self.format.shiny_threshold();
        let mut low_byte = old & 0xFF;
        // Emplacement du talent : bit 16 du PID pour un Pokémon né en Gen 5, bit 0 sinon.
        let high_bit = self.format == PkmFormat::Gen5 && (20..=23).contains(&self.version());
        if let (false, Some(slot)) = (high_bit, ability_slot) {
            low_byte = (low_byte & !1) | slot as u32;
        }
        let mut seed = old ^ 0x5EED_CAFE;
        for _ in 0..1 << 22 {
            let r = next_seed(&mut seed);
            let low = ((r >> 16) & 0xFF00) | low_byte;
            let high = if shiny {
                let x = xor.unwrap_or((r >> 8) % threshold);
                tsv ^ low ^ x
            } else {
                next_seed(&mut seed) >> 16
            };
            let pid = high << 16 | low;
            if !shiny && (tsv ^ high ^ low) < threshold {
                continue;
            }
            if let Some(n) = nature {
                if (pid % 25) as u8 != n {
                    continue;
                }
            }
            if let (true, Some(slot)) = (high_bit, ability_slot) {
                if (pid >> 16) & 1 != slot as u32 {
                    continue;
                }
            }
            self.set_pid(pid);
            return true;
        }
        false
    }
}

pub use stats::calc_stats;

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::save) const FORMATS: [PkmFormat; 4] = [PkmFormat::Gen4, PkmFormat::Gen5, PkmFormat::Gen6, PkmFormat::Gen7];

    /// Carchacrok de test, avec section équipe.
    pub(in crate::save) fn sample(format: PkmFormat, key: u32) -> Pokemon {
        let mut pk = Pokemon::blank(format);
        pk.set_encryption_constant(key);
        if format.generation() >= 6 {
            pk.set_pid(0xDEAD_BEEF);
        }
        pk.set_species(445);
        pk.set_held_item(234);
        pk.set_tid(12345);
        pk.set_sid(54321);
        pk.set_exp(1_250_000);
        pk.set_moves([200, 89, 53, 14]);
        pk.set_pp([15, 10, 15, 20]);
        pk.set_pp_ups([3, 0, 0, 1]);
        pk.set_ivs([31, 30, 29, 28, 27, 26]).unwrap();
        pk.set_evs([4, 252, 0, 0, 0, 252]);
        if format != PkmFormat::Gen4 {
            pk.set_nature(3).unwrap();
        }
        pk.set_ability(24).unwrap();
        pk.set_nickname("Carchacrok").unwrap();
        pk.set_ot_name("Thisma").unwrap();
        pk.set_ot_gender(Gender::Female);
        pk.set_gender(Gender::Female);
        pk.set_form(0).unwrap();
        pk.set_ball(4);
        pk.set_met_location(123);
        pk.set_met_level(5).unwrap();
        pk.set_friendship(255);
        pk.set_language(5);
        pk.set_version(10);
        pk.set_party_stats(100, [357, 394, 226, 176, 206, 303]);
        pk.refresh_checksum();
        pk
    }

    #[test]
    fn sizes() {
        assert_eq!((PkmFormat::Gen4.stored_size(), PkmFormat::Gen4.party_size()), (136, 236));
        assert_eq!((PkmFormat::Gen5.stored_size(), PkmFormat::Gen5.party_size()), (136, 220));
        assert_eq!((PkmFormat::Gen6.stored_size(), PkmFormat::Gen6.party_size()), (232, 260));
        assert_eq!((PkmFormat::Gen7.stored_size(), PkmFormat::Gen7.party_size()), (232, 260));
        for f in FORMATS {
            assert_eq!(HEADER + 4 * f.block_size(), f.stored_size());
        }
    }

    #[test]
    fn shuffle_orders_match_official_list() {
        // Liste officielle (Bulbapedia) : ordre des blocs dans les données chiffrées.
        const ORDERS: [&str; 24] = [
            "ABCD", "ABDC", "ACBD", "ACDB", "ADBC", "ADCB", "BACD", "BADC", "BCAD", "BCDA", "BDAC", "BDCA", "CABD", "CADB", "CBAD", "CBDA", "CDAB",
            "CDBA", "DABC", "DACB", "DBAC", "DBCA", "DCAB", "DCBA",
        ];
        for (sv, expected) in ORDERS.iter().enumerate() {
            assert_eq!(encrypted_block_order(sv), *expected, "sv = {sv}");
        }
        // Table inverse de PKHeX (`BlockPositionInvert`) : la permutation inverse de sv est inv[sv].
        const INVERT: [usize; 24] = [0, 1, 2, 4, 3, 5, 6, 7, 12, 18, 13, 19, 8, 10, 14, 20, 16, 22, 9, 11, 15, 21, 17, 23];
        for sv in 0..24 {
            let p = BLOCK_POSITION[sv];
            let q = BLOCK_POSITION[INVERT[sv]];
            for k in 0..4 {
                assert_eq!(q[p[k] as usize] as usize, k, "sv = {sv}");
            }
        }
        assert_eq!(shuffle_value(31 << 13), 7);
        assert_eq!(shuffle_value(24 << 13), 0);
    }

    #[test]
    fn shuffle_places_blocks() {
        // sv = 3 (« ACDB ») : le bloc B se retrouve en dernière position.
        let mut blocks: Vec<u8> = (0..4u8).flat_map(|b| [b; 2]).collect();
        shuffle(&mut blocks, 3, 2);
        assert_eq!(blocks, [0, 0, 2, 2, 3, 3, 1, 1]);
        unshuffle(&mut blocks, 3, 2);
        assert_eq!(blocks, [0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn lcrng_stream() {
        // Premières clés du LCRNG à partir de 0 : 0x6073 >> 16 = 0, puis 0xE97E.
        let mut d = [0u8; 4];
        crypt(&mut d, 0);
        let second = 0x6073u32.wrapping_mul(LCRNG_MUL).wrapping_add(LCRNG_ADD) >> 16;
        assert_eq!(u16::from_le_bytes([d[0], d[1]]), 0);
        assert_eq!(u16::from_le_bytes([d[2], d[3]]) as u32, second);
        assert_eq!(second, 0xE97E);
    }

    #[test]
    fn crypt_roundtrip_every_shuffle() {
        for format in FORMATS {
            for sv in 0..32u32 {
                let pk = sample(format, (0x1A2B_0005 & !(31 << 13)) | (sv << 13));
                let enc = pk.encrypt_party();
                assert_eq!(enc.len(), format.party_size());
                assert_ne!(&enc[8..40], &pk.data()[8..40], "{format:?} sv={sv}");
                let dec = Pokemon::from_encrypted(format, &enc).unwrap();
                assert_eq!(dec, pk, "{format:?} sv={sv}");
                assert!(dec.checksum_valid());
                // Auto-détection dans les deux sens.
                assert_eq!(Pokemon::from_bytes(format, &enc).unwrap(), pk);
                assert_eq!(Pokemon::from_bytes(format, pk.data()).unwrap(), pk);
                // Version boîte : la section équipe disparaît.
                let boxed = Pokemon::from_bytes(format, &pk.encrypt_stored()).unwrap();
                assert_eq!(boxed.stored_data(), pk.stored_data());
                assert_eq!(boxed.party_level(), None);
            }
        }
    }

    #[test]
    fn blank_slots_stay_empty() {
        for format in FORMATS {
            let enc = Pokemon::blank(format).encrypt_stored();
            assert!(Pokemon::from_bytes(format, &enc).unwrap().is_empty());
            assert!(Pokemon::from_bytes(format, &vec![0; format.stored_size()]).unwrap().is_empty());
        }
        assert!(matches!(Pokemon::from_bytes(PkmFormat::Gen6, &[0; 100]), Err(PkmError::BadSize { got: 100, .. })));
    }

    #[test]
    fn accessors() {
        for format in FORMATS {
            let pk = sample(format, 0x0102_0304);
            assert_eq!(pk.species(), 445);
            assert_eq!(pk.held_item(), 234);
            assert_eq!((pk.tid(), pk.sid()), (12345, 54321));
            assert_eq!(pk.exp(), 1_250_000);
            assert_eq!(pk.moves(), [200, 89, 53, 14]);
            assert_eq!(pk.pp(), [15, 10, 15, 20]);
            assert_eq!(pk.pp_ups(), [3, 0, 0, 1]);
            assert_eq!(pk.ivs(), [31, 30, 29, 28, 27, 26]);
            assert_eq!(pk.evs(), [4, 252, 0, 0, 0, 252]);
            assert_eq!(pk.ability(), 24);
            assert_eq!(pk.nickname(), "Carchacrok");
            assert!(pk.is_nicknamed());
            assert!(!pk.is_egg());
            assert_eq!(pk.ot_name(), "Thisma");
            assert_eq!(pk.ot_gender(), Gender::Female);
            assert_eq!(pk.gender(), Gender::Female);
            assert_eq!(pk.ball(), 4);
            assert_eq!(pk.met_location(), 123);
            assert_eq!(pk.met_level(), 5);
            assert_eq!(pk.friendship(), 255);
            assert_eq!(pk.language(), 5);
            assert_eq!(pk.version(), 10);
            assert_eq!(pk.party_level(), Some(100));
            assert_eq!(pk.party_stats(), Some([357, 394, 226, 176, 206, 303]));
            assert_eq!(pk.current_hp(), 357);
            let s = pk.summary(None);
            assert_eq!((s.level, s.level_estimated), (100, false));
            if format != PkmFormat::Gen4 {
                assert_eq!(s.nature_name, "Rigide");
            }
        }
    }

    #[test]
    fn iv_flags_are_independent() {
        let mut pk = Pokemon::blank(PkmFormat::Gen5);
        pk.set_is_egg(true);
        pk.set_ivs([31; 6]).unwrap();
        assert!(pk.is_egg());
        assert!(!pk.is_nicknamed());
        assert_eq!(pk.iv32(), 0x4000_0000 | 0x3FFF_FFFF);
        assert!(pk.set_ivs([32, 0, 0, 0, 0, 0]).is_err());
        // Ordre de stockage : la Vitesse (index Kaleido 5) occupe les bits 15-19.
        pk.set_ivs([0, 0, 0, 0, 0, 31]).unwrap();
        assert_eq!(pk.iv32() & 0x3FFF_FFFF, 31 << 15);
    }

    #[test]
    fn gen4_nature_from_pid() {
        let mut pk = Pokemon::blank(PkmFormat::Gen4);
        pk.set_pid(28);
        assert_eq!(pk.nature(), 3);
        assert!(matches!(pk.set_nature(1), Err(PkmError::NatureFromPid)));
    }

    #[test]
    fn shiny_thresholds() {
        let mut g4 = Pokemon::blank(PkmFormat::Gen4);
        let mut g6 = Pokemon::blank(PkmFormat::Gen6);
        g4.set_pid(7);
        g6.set_pid(7);
        assert!(g4.is_shiny() && g6.is_shiny());
        g4.set_pid(15);
        g6.set_pid(15);
        assert!(!g4.is_shiny() && g6.is_shiny());
        g6.set_tid(0x1234);
        g6.set_sid(0x4321);
        g6.set_pid(((0x1234 ^ 0x4321) << 16) | 3);
        assert!(g6.is_shiny());
    }

    #[test]
    fn gender_form_bits() {
        for format in FORMATS {
            let mut pk = Pokemon::blank(format);
            pk.set_fateful_encounter(true);
            pk.set_form(5).unwrap();
            pk.set_gender(Gender::Genderless);
            assert!(pk.fateful_encounter());
            assert_eq!(pk.form(), 5);
            assert_eq!(pk.gender(), Gender::Genderless);
            assert_eq!(pk.u8(format.ofs().flags), 1 | 2 << 1 | 5 << 3);
        }
    }

    #[test]
    fn gen4_hgss_ball() {
        let mut pk = Pokemon::blank(PkmFormat::Gen4);
        pk.set_ball(18);
        assert_eq!(pk.ball(), 18);
        assert_eq!(pk.u8(0x83), 4);
        pk.set_ball(1);
        assert_eq!(pk.ball(), 1);
    }

    #[test]
    fn party_stats_from_base() {
        let base = BaseStats { hp: 108, attack: 130, defense: 95, sp_attack: 80, sp_defense: 85, speed: 102 };
        let mut pk = Pokemon::blank(PkmFormat::Gen6);
        pk.set_species(445);
        pk.set_exp(1_250_000);
        pk.set_nature(3).unwrap();
        pk.set_ivs([31; 6]).unwrap();
        pk.set_evs([0, 252, 0, 0, 0, 0]);
        pk.update_party_stats(&base, GrowthRate::Slow);
        assert_eq!(pk.party_level(), Some(100));
        assert_eq!(pk.party_stats().unwrap()[1], 394);
        assert_eq!(pk.level(None), Some(100));
        let boxed = Pokemon::from_decrypted(PkmFormat::Gen6, pk.stored_data()).unwrap();
        assert_eq!(boxed.level(None), None);
        assert_eq!(boxed.level(Some(GrowthRate::Slow)), Some(100));
        assert!(boxed.summary(None).level_estimated);
    }
}

#[path = "pkm_extra.rs"]
mod extra;
pub use extra::{ExtrasPatch, Handler, Memory, PokemonExtras, RibbonState, SuperTraining};
