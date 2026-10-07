//! Éditeur de sauvegardes Gen 4 à 7 (moteur), d'après les formats de PKHeX.Core.
//!
//! [`SaveFile::from_bytes`] identifie le jeu ([`crate::saves::identify`], pied
//! DeSmuME `.dsv` compris), lit le dresseur, l'équipe et les boîtes, et
//! [`SaveFile::to_bytes`] rechiffre les Pokémon modifiés et recalcule les sommes de
//! contrôle :
//!
//! - **Gen 4** : CRC16-CCITT dans le pied des blocs « général » et « boîtes » de la
//!   partition active (la plus récente d'après les compteurs, choisie bloc par bloc) ;
//! - **Gen 5** : CRC16-CCITT de chaque bloc modifiable, recopié dans la table des
//!   sommes de contrôle, elle-même protégée par un CRC ;
//! - **Gen 6/7** : table des blocs en fin de fichier (signature « BEEF »), CRC16-CCITT
//!   (Gen 6) ou CRC16 inversé (Gen 7) de **tous** les blocs ;
//! - **Gen 7** : en plus, signature « MemeCrypto » de la table des blocs (SHA-256 +
//!   AES + RSA, clé connue), recalculée comme le fait PKHeX ([`memecrypto`]).
//!
//! Les offsets, écrits à l'origine de mémoire, ont été vérifiés un par un sur le code
//! source de PKHeX (commentaires « Vérifié : PKHeX … »). Seule la Gen 7 a aussi été
//! testée sur une vraie sauvegarde (Soleil/Lune, fournie avec les tests de PKHeX) ;
//! les Pokémon des quatre formats l'ont été sur de vrais fichiers `.pk4` à `.pk7`.
//! Le Pokédex est géré par [`pokedex`].

pub mod checksum;
pub mod convert;
pub mod diff;
pub mod edit;
mod gen4;
mod gen5;
mod gen6;
mod gen7;
mod inventory;
mod memecrypto;
mod origin;
pub mod pkm;
pub mod pokedex;
pub mod session;
pub mod showdown_apply;
pub mod stats;
mod strings;

use std::fmt::Write as _;

use serde::Serialize;

pub use inventory::{InventoryItem, Pouch, PouchKind};
pub use origin::TrainerOrigin;
pub use pkm::{Gender, PkmDate, PkmError, PkmFormat, Pokemon, PokemonSummary, ShinyMode};
pub use stats::{calc_stats, exp_for_level, level_from_exp, nature_name, GrowthRate, NATURES_FR};

use crate::saves::{self, SaveKind, DESMUME_FOOTER, NDS_SAVE_SIZE};

/// Emplacements par boîte, pour toutes les générations prises en charge.
pub const BOX_SLOTS: usize = 30;
/// Taille maximale de l'équipe.
pub const PARTY_SLOTS: usize = 6;

/// Jeu exact d'une sauvegarde (plus précis que [`SaveKind`] pour la Gen 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveVersion {
    DiamondPearl,
    Platinum,
    HeartGoldSoulSilver,
    BlackWhite,
    Black2White2,
    XY,
    OmegaRubyAlphaSapphire,
    SunMoon,
    UltraSunUltraMoon,
}

impl SaveVersion {
    pub fn label(self) -> &'static str {
        match self {
            SaveVersion::DiamondPearl => "Pokémon Diamant / Perle",
            SaveVersion::Platinum => "Pokémon Platine",
            SaveVersion::HeartGoldSoulSilver => "Pokémon Or HeartGold / Argent SoulSilver",
            SaveVersion::BlackWhite => "Pokémon Noire / Blanche",
            SaveVersion::Black2White2 => "Pokémon Noire 2 / Blanche 2",
            SaveVersion::XY => "Pokémon X / Y",
            SaveVersion::OmegaRubyAlphaSapphire => "Pokémon Rubis Oméga / Saphir Alpha",
            SaveVersion::SunMoon => "Pokémon Soleil / Lune",
            SaveVersion::UltraSunUltraMoon => "Pokémon Ultra-Soleil / Ultra-Lune",
        }
    }

    pub fn format(self) -> PkmFormat {
        match self {
            SaveVersion::DiamondPearl | SaveVersion::Platinum | SaveVersion::HeartGoldSoulSilver => PkmFormat::Gen4,
            SaveVersion::BlackWhite | SaveVersion::Black2White2 => PkmFormat::Gen5,
            SaveVersion::XY | SaveVersion::OmegaRubyAlphaSapphire => PkmFormat::Gen6,
            SaveVersion::SunMoon | SaveVersion::UltraSunUltraMoon => PkmFormat::Gen7,
        }
    }

    pub fn generation(self) -> u8 {
        self.format().generation()
    }

    pub fn kind(self) -> SaveKind {
        match self {
            SaveVersion::DiamondPearl => SaveKind::DiamondPearl,
            SaveVersion::Platinum => SaveKind::Platinum,
            SaveVersion::HeartGoldSoulSilver => SaveKind::HeartGoldSoulSilver,
            SaveVersion::BlackWhite | SaveVersion::Black2White2 => SaveKind::Gen5,
            SaveVersion::XY => SaveKind::XY,
            SaveVersion::OmegaRubyAlphaSapphire => SaveKind::OmegaRubyAlphaSapphire,
            SaveVersion::SunMoon => SaveKind::SunMoon,
            SaveVersion::UltraSunUltraMoon => SaveKind::UltraSunUltraMoon,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("fichier de sauvegarde non reconnu")]
    Unrecognized,
    #[error("sauvegarde Gen 5 : table des sommes de contrôle invalide (ni Noire/Blanche, ni Noire 2/Blanche 2)")]
    Gen5Table,
    #[error("sauvegarde 3DS : signature « BEEF » de la table des blocs introuvable")]
    MissingBeef,
    #[error("sauvegarde 3DS : table des blocs vide ou illisible")]
    EmptyBlockTable,
    #[error("sauvegarde tronquée : {0} hors du fichier")]
    Truncated(&'static str),
    #[error("boîte {0} inexistante")]
    BadBox(usize),
    #[error("emplacement {0} inexistant")]
    BadSlot(usize),
    #[error("l'équipe compte {count} Pokémon : impossible d'écrire à l'emplacement {slot}")]
    PartyGap { slot: usize, count: usize },
    #[error("l'équipe doit garder au moins un Pokémon")]
    LastPartyMember,
    #[error("Pokémon au format {found} incompatible avec cette sauvegarde ({expected})")]
    FormatMismatch { expected: PkmFormat, found: PkmFormat },
    #[error("statistiques d'équipe absentes : appelle Pokemon::update_party_stats avant de placer ce Pokémon dans l'équipe")]
    MissingPartyStats,
    #[error(transparent)]
    Pkm(#[from] PkmError),
    /// Opération refusée par l'éditeur (message destiné à l'utilisateur).
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayTime {
    pub hours: u16,
    pub minutes: u8,
    pub seconds: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trainer {
    pub name: String,
    /// Gen 6/7 : région, pays et région de la console du joueur (`MyStatus`), recopiés
    /// dans les Pokémon qu'il obtient.
    #[serde(skip)]
    pub origin: Option<[u8; 3]>,
    pub tid: u16,
    pub sid: u16,
    /// Numéro affiché en jeu : `tid` jusqu'à la Gen 6, nombre à 6 chiffres en Gen 7.
    pub display_id: u32,
    pub gender: Gender,
    pub money: u32,
    pub play_time: PlayTime,
}

/// État d'une somme de contrôle de la sauvegarde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockCheck {
    pub name: String,
    pub offset: usize,
    pub length: usize,
    pub stored: u16,
    pub computed: u16,
    pub valid: bool,
}

/// Offsets (absolus) des champs du dresseur.
#[derive(Debug, Clone)]
struct TrainerLayout {
    name: usize,
    name_max: usize,
    tid: usize,
    sid: usize,
    gender: usize,
    /// Langue de la partie (identifiant `LanguageID` de PKHeX : 1 = japonais, 2 = anglais, 3 = français…).
    language: usize,
    money: usize,
    hours: usize,
    minutes: usize,
    seconds: usize,
}

/// Emplacement des données dans le fichier, calculé à l'ouverture.
#[derive(Debug, Clone)]
struct Layout {
    format: PkmFormat,
    trainer: TrainerLayout,
    /// Premier emplacement de l'équipe.
    party: usize,
    /// Octet du nombre de Pokémon dans l'équipe.
    party_count: usize,
    boxes: usize,
    box_stride: usize,
    box_count: usize,
    box_names: usize,
    box_name_stride: usize,
    box_name_max: usize,
    /// Base du sac (offset absolu des poches, voir [`inventory`]).
    items: usize,
    /// Début des données du Pokédex (voir [`pokedex`]).
    dex: usize,
    /// Numéro (`u16`) de la carte où le joueur a sauvegardé.
    map: usize,
    checks: Checks,
}

#[derive(Debug, Clone)]
enum Checks {
    Gen4(gen4::Gen4Checks),
    Gen5(gen5::NdsChecks),
    Ctr { blocks: Vec<gen6::CtrBlock>, gen7: bool },
}

impl Checks {
    fn fix(&self, data: &mut [u8]) {
        match self {
            Checks::Gen4(blocks) => gen4::fix(data, blocks),
            Checks::Gen5(table) => gen5::fix(data, table),
            Checks::Ctr { blocks, gen7 } => gen6::fix(data, blocks, *gen7),
        }
    }

    fn verify(&self, data: &[u8]) -> Vec<BlockCheck> {
        match self {
            Checks::Gen4(blocks) => gen4::verify(data, blocks),
            Checks::Gen5(table) => gen5::verify(data, table),
            Checks::Ctr { blocks, gen7 } => gen6::verify(data, blocks, *gen7),
        }
    }
}

fn rd_u8(d: &[u8], at: usize) -> u8 {
    d.get(at).copied().unwrap_or(0)
}

fn rd_u16(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
}

fn rd_u32(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn wr_u16(d: &mut [u8], at: usize, v: u16) {
    if let Some(b) = d.get_mut(at..at + 2) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

/// Sauvegarde ouverte et modifiable.
#[derive(Debug, Clone)]
pub struct SaveFile {
    version: SaveVersion,
    data: Vec<u8>,
    /// Pied DeSmuME (`.dsv`) recopié tel quel à l'écriture.
    trailer: Vec<u8>,
    layout: Layout,
    warnings: Vec<String>,
}

impl SaveFile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SaveError> {
        let (body, trailer) = if bytes.len() == NDS_SAVE_SIZE + DESMUME_FOOTER { bytes.split_at(NDS_SAVE_SIZE) } else { (bytes, &[][..]) };
        let version = match saves::identify(body).ok_or(SaveError::Unrecognized)? {
            SaveKind::DiamondPearl => SaveVersion::DiamondPearl,
            SaveKind::Platinum => SaveVersion::Platinum,
            SaveKind::HeartGoldSoulSilver => SaveVersion::HeartGoldSoulSilver,
            SaveKind::Gen5 => gen5::detect(body).ok_or(SaveError::Gen5Table)?,
            SaveKind::XY => SaveVersion::XY,
            SaveKind::OmegaRubyAlphaSapphire => SaveVersion::OmegaRubyAlphaSapphire,
            SaveKind::SunMoon => SaveVersion::SunMoon,
            SaveKind::UltraSunUltraMoon => SaveVersion::UltraSunUltraMoon,
        };
        let (layout, warnings) = match version.generation() {
            4 => gen4::layout(version, body)?,
            5 => gen5::layout(version, body)?,
            6 => gen6::layout(version, body)?,
            _ => gen7::layout(version, body)?,
        };
        Ok(Self { version, data: body.to_vec(), trailer: trailer.to_vec(), layout, warnings })
    }

    /// Fichier complet : Pokémon déjà rechiffrés, sommes de contrôle recalculées,
    /// pied `.dsv` restitué s'il y en avait un.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        self.layout.checks.fix(&mut out);
        out.extend_from_slice(&self.trailer);
        out
    }

    pub fn version(&self) -> SaveVersion {
        self.version
    }

    pub fn kind(&self) -> SaveKind {
        self.version.kind()
    }

    pub fn generation(&self) -> u8 {
        self.version.generation()
    }

    pub fn format(&self) -> PkmFormat {
        self.layout.format
    }

    /// Anomalies relevées à l'ouverture (structure inattendue, signature non gérée…).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Toujours `false` : la signature MemeCrypto de Soleil/Lune et Ultra-Soleil/Ultra-Lune
    /// est recalculée par [`Self::to_bytes`] (portage de PKHeX). Conservé pour l'interface.
    pub fn needs_resign(&self) -> bool {
        false
    }

    /// Sommes de contrôle des données actuelles (avant correction par [`Self::to_bytes`]).
    pub fn checksums(&self) -> Vec<BlockCheck> {
        self.layout.checks.verify(&self.data)
    }

    pub fn checksums_valid(&self) -> bool {
        self.checksums().iter().all(|c| c.valid)
    }

    pub fn trainer(&self) -> Trainer {
        let t = &self.layout.trainer;
        let d = &self.data;
        let name = d.get(t.name..t.name + 2 * (t.name_max + 1)).map(|b| strings::decode(self.format(), b)).unwrap_or_default();
        let (tid, sid) = (rd_u16(d, t.tid), rd_u16(d, t.sid));
        let display_id = if self.generation() >= 7 { ((sid as u32) << 16 | tid as u32) % 1_000_000 } else { tid as u32 };
        Trainer {
            name,
            origin: (self.generation() >= 6).then(|| {
                let o = self.trainer_origin();
                [o.region, o.country, o.console_region]
            }),
            tid,
            sid,
            display_id,
            gender: if rd_u8(d, t.gender) == 0 { Gender::Male } else { Gender::Female },
            money: rd_u32(d, t.money),
            play_time: PlayTime { hours: rd_u16(d, t.hours), minutes: rd_u8(d, t.minutes), seconds: rd_u8(d, t.seconds) },
        }
    }

    /// Badges d'arène obtenus (un bit par badge), Gen 4 à 6.
    /// PKHeX : SAV4 `Badges = General[Trainer1 + 0x1A]` (argent en Trainer1 + 0x14) ;
    /// Misc5 `Badges = Data[0x04]` (argent en 0x00). Pour HGSS, badges de Johto.
    ///
    /// Gen 6 : PKHeX `Misc6XY` / `Misc6AO` `Badges = Data[0xC]` (argent en 0x8).
    /// Gen 7 : pas de badges ; renvoie les îles terminées (bits 0-3), tampons 1 à 4 de
    /// PKHeX `Misc7.Stamps` (`u32` en Misc + 0x8, argent en Misc + 0x4, tampons à partir du bit 4).
    /// Carte (Gen 4-5) ou zone (Gen 6-7) où le joueur se trouvait en sauvegardant :
    /// numéro à traduire en lieu avec les en-têtes de cartes de la ROM.
    pub fn current_map(&self) -> u16 {
        rd_u16(&self.data, self.layout.map)
    }

    pub fn badges(&self) -> Option<u8> {
        if self.generation() == 7 {
            let at = self.layout.trainer.money + 4;
            return self.data.get(at..at + 4).map(|b| ((u32::from_le_bytes([b[0], b[1], b[2], b[3]]) >> 5) & 0xF) as u8);
        }
        self.badges_offset().map(|at| rd_u8(&self.data, at))
    }

    pub fn set_badges(&mut self, bits: u8) {
        if let Some(b) = self.badges_offset().and_then(|at| self.data.get_mut(at)) {
            *b = bits;
        }
    }

    fn badges_offset(&self) -> Option<usize> {
        let money = self.layout.trainer.money;
        match self.generation() {
            4 => Some(money + 6),
            5 | 6 => Some(money + 4),
            _ => None,
        }
    }

    // --- Boîtes.

    pub fn box_count(&self) -> usize {
        self.layout.box_count
    }

    pub fn box_name(&self, index: usize) -> Result<String, SaveError> {
        if index >= self.layout.box_count {
            return Err(SaveError::BadBox(index));
        }
        let at = self.layout.box_names + index * self.layout.box_name_stride;
        let bytes = self.data.get(at..at + 2 * (self.layout.box_name_max + 1)).ok_or(SaveError::Truncated("noms des boîtes"))?;
        Ok(strings::decode(self.format(), bytes))
    }

    fn box_offset(&self, index: usize, slot: usize) -> Result<usize, SaveError> {
        if index >= self.layout.box_count {
            return Err(SaveError::BadBox(index));
        }
        if slot >= BOX_SLOTS {
            return Err(SaveError::BadSlot(slot));
        }
        Ok(self.layout.boxes + index * self.layout.box_stride + slot * self.format().stored_size())
    }

    /// Pokémon d'un emplacement de boîte (`None` si vide).
    pub fn box_slot(&self, index: usize, slot: usize) -> Result<Option<Pokemon>, SaveError> {
        let at = self.box_offset(index, slot)?;
        let raw = self.data.get(at..at + self.format().stored_size()).ok_or(SaveError::Truncated("boîtes"))?;
        let pk = Pokemon::from_bytes(self.format(), raw)?;
        Ok((!pk.is_empty()).then_some(pk))
    }

    /// Écrit (ou vide, avec `None`) un emplacement de boîte.
    pub fn set_box_slot(&mut self, index: usize, slot: usize, pokemon: Option<Pokemon>) -> Result<(), SaveError> {
        let at = self.box_offset(index, slot)?;
        let pk = self.checked(pokemon)?;
        let enc = pk.encrypt_stored();
        self.data.get_mut(at..at + enc.len()).ok_or(SaveError::Truncated("boîtes"))?.copy_from_slice(&enc);
        Ok(())
    }

    fn checked(&self, pokemon: Option<Pokemon>) -> Result<Pokemon, SaveError> {
        let format = self.format();
        match pokemon {
            Some(pk) if pk.format() != format => Err(SaveError::FormatMismatch { expected: format, found: pk.format() }),
            Some(pk) => Ok(pk),
            None => Ok(Pokemon::blank(format)),
        }
    }

    // --- Équipe.

    pub fn party_count(&self) -> usize {
        (rd_u8(&self.data, self.layout.party_count) as usize).min(PARTY_SLOTS)
    }

    fn party_offset(&self, slot: usize) -> usize {
        self.layout.party + slot * self.format().party_size()
    }

    pub fn party_slot(&self, slot: usize) -> Result<Option<Pokemon>, SaveError> {
        if slot >= PARTY_SLOTS {
            return Err(SaveError::BadSlot(slot));
        }
        let at = self.party_offset(slot);
        let raw = self.data.get(at..at + self.format().party_size()).ok_or(SaveError::Truncated("équipe"))?;
        let pk = Pokemon::from_bytes(self.format(), raw)?;
        Ok((!pk.is_empty()).then_some(pk))
    }

    /// Pokémon de l'équipe, dans l'ordre (les emplacements vides sont ignorés).
    pub fn party(&self) -> Result<Vec<Pokemon>, SaveError> {
        let mut out = Vec::with_capacity(PARTY_SLOTS);
        for slot in 0..self.party_count() {
            out.extend(self.party_slot(slot)?);
        }
        Ok(out)
    }

    fn write_party_raw(&mut self, slot: usize, pk: &Pokemon) -> Result<(), SaveError> {
        let at = self.party_offset(slot);
        let enc = pk.encrypt_party();
        self.data.get_mut(at..at + enc.len()).ok_or(SaveError::Truncated("équipe"))?.copy_from_slice(&enc);
        Ok(())
    }

    fn set_party_count(&mut self, count: usize) -> Result<(), SaveError> {
        *self.data.get_mut(self.layout.party_count).ok_or(SaveError::Truncated("équipe"))? = count as u8;
        Ok(())
    }

    /// Remplace un membre de l'équipe, en ajoute un (emplacement = nombre actuel), ou
    /// le retire avec `None` (les suivants remontent). Un Pokémon venant d'une boîte
    /// doit d'abord recevoir ses statistiques ([`Pokemon::update_party_stats`]).
    pub fn set_party_slot(&mut self, slot: usize, pokemon: Option<Pokemon>) -> Result<(), SaveError> {
        if slot >= PARTY_SLOTS {
            return Err(SaveError::BadSlot(slot));
        }
        let count = self.party_count();
        match pokemon {
            Some(pk) => {
                if slot > count {
                    return Err(SaveError::PartyGap { slot, count });
                }
                let pk = self.checked(Some(pk))?;
                if pk.party_stats().is_none() {
                    return Err(SaveError::MissingPartyStats);
                }
                self.write_party_raw(slot, &pk)?;
                if slot == count {
                    self.set_party_count(count + 1)?;
                }
            }
            None => {
                if slot >= count {
                    return Err(SaveError::PartyGap { slot, count });
                }
                if count == 1 {
                    return Err(SaveError::LastPartyMember);
                }
                let size = self.format().party_size();
                let start = self.party_offset(slot);
                let end = self.party_offset(count);
                if end > self.data.len() {
                    return Err(SaveError::Truncated("équipe"));
                }
                self.data.copy_within(start + size..end, start);
                let blank = Pokemon::blank(self.format());
                self.write_party_raw(count - 1, &blank)?;
                self.set_party_count(count - 1)?;
            }
        }
        Ok(())
    }

    /// Repères pour retrouver cette partie dans la mémoire de l'émulateur (compagnon en direct).
    pub fn ram_hints(&self) -> RamHints {
        let t = &self.layout.trainer;
        let d = &self.data;
        let name_len = 2 * (t.name_max + 1);
        let party = self.party().unwrap_or_default();
        RamHints {
            format: self.format(),
            party: self.layout.party,
            party_count: self.layout.party_count,
            trainer_name: t.name,
            trainer_name_bytes: d.get(t.name..t.name + name_len).map(<[u8]>::to_vec).unwrap_or_default(),
            tid: rd_u16(d, t.tid),
            sid: rd_u16(d, t.sid),
            tid_offset: t.tid,
            map: self.layout.map,
            badges: self.badges_offset(),
            hours: t.hours,
            party_keys: party.iter().map(|p| rd_u32(p.data(), 0)).collect(),
        }
    }
}

/// Repères tirés de la sauvegarde pour lire la même partie en mémoire : offsets absolus dans le
/// fichier (la RAM des jeux DS garde les blocs de la sauvegarde dans le même ordre, voir
/// [`crate::live::reader`]), nom et numéro du dresseur, premiers mots chiffrés de l'équipe.
#[derive(Debug, Clone)]
pub struct RamHints {
    pub format: PkmFormat,
    pub party: usize,
    pub party_count: usize,
    pub trainer_name: usize,
    /// Nom du dresseur tel qu'encodé dans la sauvegarde (terminateur et remplissage compris).
    pub trainer_name_bytes: Vec<u8>,
    pub tid: u16,
    pub sid: u16,
    pub tid_offset: usize,
    pub map: usize,
    pub badges: Option<usize>,
    pub hours: usize,
    /// Premier mot (`u32`) de chaque Pokémon de l'équipe : PID en Gen 4-5, constante de
    /// chiffrement en Gen 6-7. Ce mot n'est pas chiffré : il sert de signature en RAM.
    pub party_keys: Vec<u32>,
}

/// Résumé lisible d'une sauvegarde, pour le débogage.
pub fn describe(save: &SaveFile) -> String {
    let mut s = String::new();
    let t = save.trainer();
    let _ = writeln!(s, "{} (Gen {})", save.version().label(), save.generation());
    let _ = writeln!(
        s,
        "Dresseur : {} {} — ID {:05} / SID {:05} (affiché {}), {} ₽, {}:{:02}:{:02}",
        t.name,
        t.gender.symbol(),
        t.tid,
        t.sid,
        t.display_id,
        t.money,
        t.play_time.hours,
        t.play_time.minutes,
        t.play_time.seconds
    );
    match save.party() {
        Ok(party) => {
            let _ = writeln!(s, "Équipe ({}) :", party.len());
            for (i, pk) in party.iter().enumerate() {
                let _ = writeln!(s, "  {}. {}", i + 1, describe_pokemon(pk));
            }
        }
        Err(e) => {
            let _ = writeln!(s, "Équipe illisible : {e}");
        }
    }
    let mut total = 0;
    let mut lines = Vec::new();
    for b in 0..save.box_count() {
        let mons: Vec<Pokemon> = (0..BOX_SLOTS).filter_map(|i| save.box_slot(b, i).ok().flatten()).collect();
        if !mons.is_empty() {
            total += mons.len();
            let name = save.box_name(b).unwrap_or_default();
            lines.push(format!("  Boîte {} « {} » : {} Pokémon", b + 1, name, mons.len()));
            for pk in &mons {
                lines.push(format!("    - {}", describe_pokemon(pk)));
            }
        }
    }
    let _ = writeln!(s, "Boîtes : {} × {} emplacements, {} Pokémon", save.box_count(), BOX_SLOTS, total);
    for l in lines {
        let _ = writeln!(s, "{l}");
    }
    let checks = save.checksums();
    let valid = checks.iter().filter(|c| c.valid).count();
    let _ = writeln!(s, "Sommes de contrôle : {valid}/{} valides", checks.len());
    for c in checks.iter().filter(|c| !c.valid) {
        let _ = writeln!(s, "  ✗ {} @0x{:X} : stockée {:04X}, calculée {:04X}", c.name, c.offset, c.stored, c.computed);
    }
    for w in save.warnings() {
        let _ = writeln!(s, "Attention : {w}");
    }
    s
}

fn describe_pokemon(pk: &Pokemon) -> String {
    let sum = pk.summary(None);
    format!(
        "#{} « {} » N.{}{} {} {}{}{} (DO {} {:05})",
        sum.species,
        sum.nickname,
        if sum.level_estimated { "~" } else { "" },
        sum.level,
        sum.gender.symbol(),
        sum.nature_name,
        if sum.shiny { " ★" } else { "" },
        if sum.is_egg { " (Œuf)" } else { "" },
        sum.ot_name,
        sum.tid
    )
}

/// Sauvegarde Platine **synthétique** remplie de quelques Pokémon, pour essayer
/// l'éditeur sans vraie partie (outil de développement : `kaleido demo-save`).
#[doc(hidden)]
pub fn demo_save() -> Result<Vec<u8>, SaveError> {
    use session::{SaveSession, Slot};
    let mut s = SaveSession::open(&gen4::blank(SaveVersion::Platinum, 0, 0))?;
    let team: [(u16, u8, [u16; 4]); 12] = [
        (445, 62, [337, 89, 200, 14]),
        (448, 55, [396, 94, 245, 182]),
        (392, 48, [53, 7, 183, 370]),
        (25, 40, [85, 98, 86, 21]),
        (6, 70, [53, 19, 337, 89]),
        (149, 80, [200, 245, 57, 63]),
        (94, 58, [247, 94, 85, 109]),
        (133, 15, [33, 39, 28, 98]),
        (143, 45, [34, 156, 214, 89]),
        (130, 50, [127, 242, 349, 89]),
        (448, 30, [396, 98, 182, 245]),
        (493, 100, [449, 63, 105, 248]),
    ];
    for (i, &(species, level, moves)) in team.iter().enumerate() {
        let mut p = Pokemon::blank(PkmFormat::Gen4);
        p.set_species(species);
        // PID aux deux moitiés égales avec TID/SID nuls : chromatique pour le 4ᵉ.
        p.set_pid(if i == 3 { 0x1A2B_1A2B } else { 0x0101_0000u32.wrapping_mul(i as u32 + 7) ^ 0x5A3C });
        let growth = crate::names::growth_rate(species).unwrap_or(GrowthRate::MediumFast);
        p.set_exp(exp_for_level(growth, level));
        p.set_moves(moves);
        p.set_ivs([31, (i as u8 * 7) % 32, 20, 31, 25, (i as u8 * 3) % 32])?;
        p.set_nickname(crate::names::species(species).unwrap_or("?"))?;
        p.set_ot_name("Kaleido")?;
        // Talent : premier talent de l'espèce, d'après les données embarquées (Gen 6).
        if let Some(a) = crate::names::first_ability(species) {
            p.set_ability(a)?;
        }
        p.set_ball(4);
        p.set_language(session::LANGUAGE_FR);
        p.set_friendship(70);
        p.refresh_checksum();
        s.save.set_box_slot(0, i, Some(p))?;
    }
    for i in 0..3 {
        s.move_pokemon(Slot::Box { r#box: 0, index: i }, Slot::Party { index: i })?;
    }
    Ok(s.to_bytes())
}

#[cfg(test)]
mod pkhex_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;
