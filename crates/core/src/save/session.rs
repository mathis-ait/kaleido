//! Session d'édition d'une sauvegarde pour l'interface : emplacements, noms
//! français, déplacements entre boîtes et équipe, modifications d'un Pokémon,
//! historique annuler / rétablir.

use serde::{Deserialize, Serialize};

use super::edit::TrainerPatch;
use super::pkm::{ExtrasPatch, PokemonExtras};
use super::{
    calc_stats, exp_for_level, Gender, GrowthRate, PkmDate, PkmFormat, Pokemon, PokemonSummary, SaveError, SaveFile, SaveVersion, ShinyMode, Trainer,
    BOX_SLOTS, PARTY_SLOTS,
};
use crate::dex::{self, Game};
use crate::names;

/// Nombre d'états gardés pour « Annuler ».
const HISTORY: usize = 64;

/// Emplacement d'un Pokémon dans la sauvegarde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Slot {
    Party { index: usize },
    Box { r#box: usize, index: usize },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub slot: Slot,
    #[serde(flatten)]
    pub summary: PokemonSummary,
    pub species_name: String,
    pub ability_name: String,
    pub item_name: Option<String>,
    pub move_names: Vec<String>,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit (calculées si le Pokémon est en boîte).
    pub stats: Option<[u16; 6]>,
    pub known_moves: Vec<KnownMove>,
    pub met_location_name: Option<String>,
    pub egg_location_name: Option<String>,
    pub species_data: Option<SpeciesData>,
    #[serde(flatten)]
    pub details: PokemonDetails,
}

/// Champs de la fiche complète, en plus du résumé.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PokemonDetails {
    pub encryption_constant: u32,
    pub ability_number: u8,
    pub pp: [u8; 4],
    pub pp_ups: [u8; 4],
    pub ot_gender: Gender,
    pub met_location: u16,
    pub met_level: u8,
    pub met_date: Option<PkmDate>,
    pub egg_location: u16,
    pub egg_date: Option<PkmDate>,
    pub version: u8,
    pub language: u8,
    pub fateful_encounter: bool,
    pub markings: [u8; 6],
    pub pokerus_strain: u8,
    pub pokerus_days: u8,
    /// Valeur de chromatique (TSV) du dresseur d'origine, et du Pokémon (PSV).
    pub tsv: u16,
    pub psv: u16,
    /// Concours, rubans, caractéristique, soigneur, souvenirs, Super Training…
    pub extras: PokemonExtras,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveView {
    pub game: &'static str,
    pub version: SaveVersion,
    pub generation: u8,
    pub trainer: Trainer,
    pub box_names: Vec<String>,
    /// Nombre de Pokémon par boîte.
    pub box_fill: Vec<usize>,
    pub party: Vec<SlotView>,
    pub warnings: Vec<String>,
    pub needs_resign: bool,
    pub checksums_valid: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub trainer_name_max: usize,
    pub box_name_max: usize,
    pub nickname_max: usize,
}

/// Modifications demandées par l'interface (champs absents = inchangés).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PokemonPatch {
    pub species: Option<u16>,
    pub form: Option<u8>,
    pub nickname: Option<String>,
    /// `false` : le surnom redevient le nom de l'espèce.
    pub is_nicknamed: Option<bool>,
    pub level: Option<u8>,
    pub exp: Option<u32>,
    pub nature: Option<u8>,
    pub ability: Option<u16>,
    pub ability_number: Option<u8>,
    pub gender: Option<Gender>,
    pub shiny: Option<ShinyMode>,
    pub pid: Option<u32>,
    pub encryption_constant: Option<u32>,
    pub held_item: Option<u16>,
    pub language: Option<u8>,
    pub moves: Option<[u16; 4]>,
    pub pp: Option<[u8; 4]>,
    pub pp_ups: Option<[u8; 4]>,
    pub ivs: Option<[u8; 6]>,
    pub evs: Option<[u8; 6]>,
    pub friendship: Option<u8>,
    pub ot_name: Option<String>,
    pub tid: Option<u16>,
    pub sid: Option<u16>,
    pub ot_gender: Option<Gender>,
    pub ball: Option<u8>,
    pub met_location: Option<u16>,
    pub met_level: Option<u8>,
    /// `Some(None)` efface la date.
    pub met_date: Option<Option<PkmDate>>,
    pub egg_location: Option<u16>,
    pub egg_date: Option<Option<PkmDate>>,
    pub version: Option<u8>,
    pub fateful_encounter: Option<bool>,
    pub is_egg: Option<bool>,
    pub markings: Option<[u8; 6]>,
    pub pokerus: Option<(u8, u8)>,
    pub extras: Option<ExtrasPatch>,
}

pub struct SaveSession {
    pub save: SaveFile,
    undo: Vec<SaveFile>,
    redo: Vec<SaveFile>,
}

fn growth(game: Game, species: u16, form: u8) -> Option<GrowthRate> {
    dex::personal(game, species, form).map(|p| p.growth_rate).or_else(|| names::growth_rate(species))
}

fn details(p: &Pokemon) -> PokemonDetails {
    let pid = p.pid();
    let (strain, days) = p.pokerus();
    let shift = if p.format().generation() >= 6 { 4 } else { 3 };
    PokemonDetails {
        encryption_constant: p.encryption_constant(),
        ability_number: p.ability_number(),
        pp: p.pp(),
        pp_ups: p.pp_ups(),
        ot_gender: p.ot_gender(),
        met_location: p.met_location(),
        met_level: p.met_level(),
        met_date: p.met_date(),
        egg_location: p.egg_location(),
        egg_date: p.egg_date(),
        version: p.version(),
        language: p.language(),
        fateful_encounter: p.fateful_encounter(),
        markings: p.markings(),
        pokerus_strain: strain,
        pokerus_days: days,
        tsv: (p.tid() ^ p.sid()) >> shift,
        psv: (((pid >> 16) ^ (pid & 0xFFFF)) >> shift) as u16,
        extras: p.extras(),
    }
}

/// Données de l'espèce dans le jeu de la sauvegarde (fiche « personal » de PKHeX).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeciesData {
    /// Types (identifiants PKHeX : 0 Normal … 17 Fée) ; un seul si les deux sont égaux.
    pub types: Vec<u8>,
    /// PV, Att, Déf, Atq Spé, Déf Spé, Vit.
    pub base_stats: [u8; 6],
    /// Talents 1, 2 et caché (0 = aucun).
    pub abilities: [u16; 3],
    pub ability_names: [String; 3],
    /// 0 = toujours mâle, 254 = toujours femelle, 255 = asexué, sinon seuil sur le PID.
    pub gender_ratio: u8,
    pub form_name: Option<String>,
    pub form_names: Vec<String>,
    pub base_friendship: u8,
}

/// Infos d'une attaque connue (type, PP max avec les PP Plus).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownMove {
    pub id: u16,
    pub name: String,
    pub type_id: u8,
    pub category: &'static str,
    pub power: Option<u8>,
    pub accuracy: Option<u8>,
    pub base_pp: u8,
    pub max_pp: u8,
}

fn species_data(game: Game, species: u16, form: u8) -> Option<SpeciesData> {
    let info = dex::personal(game, species, form)?;
    let b = info.base_stats;
    let types = if info.types[0] == info.types[1] { vec![info.types[0]] } else { info.types.to_vec() };
    Some(SpeciesData {
        types,
        base_stats: [b.hp, b.attack, b.defense, b.sp_attack, b.sp_defense, b.speed],
        abilities: info.abilities,
        ability_names: info.abilities.map(|a| dex::ability_name(a).unwrap_or_default().to_string()),
        gender_ratio: info.gender_ratio,
        form_name: dex::form_name(game, species, form),
        form_names: dex::form_names(game, species),
        base_friendship: info.base_friendship,
    })
}

fn known_moves(game: Game, p: &Pokemon) -> Vec<KnownMove> {
    let ups = p.pp_ups();
    p.moves()
        .iter()
        .enumerate()
        .filter(|(_, &m)| m != 0)
        .map(|(i, &m)| {
            let info = dex::move_info_in(game, m);
            let base_pp = info.as_ref().map_or(0, |i| i.pp);
            KnownMove {
                id: m,
                name: dex::move_name(m).map_or_else(|| format!("n°{m}"), str::to_string),
                type_id: info.as_ref().map_or(0, |i| i.type_id),
                category: info.as_ref().map_or("", |i| i.category.name_fr()),
                power: info.as_ref().and_then(|i| i.power),
                accuracy: info.as_ref().and_then(|i| i.accuracy),
                base_pp,
                max_pp: max_pp(base_pp, ups[i]),
            }
        })
        .collect()
}

/// PP maximum : chaque PP Plus ajoute 20 % des PP de base.
pub fn max_pp(base: u8, ups: u8) -> u8 {
    (base as u16 + base as u16 * ups.min(3) as u16 / 5) as u8
}

pub fn view_of(game: Game, slot: Slot, p: &Pokemon) -> SlotView {
    let summary = p.summary(growth(game, p.species(), p.form()));
    let name_or_id = |n: Option<&str>, id: u16| match (n, id) {
        (Some(n), _) => n.to_string(),
        (None, 0) => "—".to_string(),
        (None, id) => format!("n°{id}"),
    };
    let species = species_data(game, p.species(), p.form());
    let stats = p
        .party_stats()
        .or_else(|| base_stats(game, p.species(), p.form()).map(|b| calc_stats(&b, summary.level, summary.ivs, summary.evs, summary.nature)));
    SlotView {
        slot,
        species_name: name_or_id(dex::species_name(summary.species), summary.species),
        ability_name: name_or_id(dex::ability_name(summary.ability), summary.ability),
        item_name: dex::item_name_in(game, summary.held_item).filter(|_| summary.held_item != 0).map(str::to_string),
        move_names: summary.moves.iter().filter(|&&m| m != 0).map(|&m| name_or_id(dex::move_name(m), m)).collect(),
        stats,
        known_moves: known_moves(game, p),
        met_location_name: dex::location_name(game.generation(), p.met_location()).map(str::to_string),
        egg_location_name: dex::location_name(game.generation(), p.egg_location()).map(str::to_string),
        species_data: species,
        details: details(p),
        summary,
    }
}

fn base_stats(game: Game, species: u16, form: u8) -> Option<crate::pokemon::BaseStats> {
    dex::personal(game, species, form).map(|p| p.base_stats).or_else(|| names::base_stats(species))
}

/// Jeu des données correspondant à une sauvegarde.
pub fn game_of(v: SaveVersion) -> Game {
    match v {
        SaveVersion::DiamondPearl => Game::DP,
        SaveVersion::Platinum => Game::Pt,
        SaveVersion::HeartGoldSoulSilver => Game::HGSS,
        SaveVersion::BlackWhite => Game::BW,
        SaveVersion::Black2White2 => Game::B2W2,
        SaveVersion::XY => Game::XY,
        SaveVersion::OmegaRubyAlphaSapphire => Game::ORAS,
        SaveVersion::SunMoon => Game::SM,
        SaveVersion::UltraSunUltraMoon => Game::USUM,
    }
}

/// Les 4 dernières attaques apprises par niveau jusqu'au niveau donné (comme PKHeX).
pub fn suggested_moves(game: Game, species: u16, form: u8, level: u8) -> [u16; 4] {
    let mut learned: Vec<u16> = Vec::new();
    for &(m, l) in dex::levelup(game, species, form) {
        if l <= level && m != 0 {
            learned.retain(|&x| x != m);
            learned.push(m);
        }
    }
    let start = learned.len().saturating_sub(4);
    let mut out = [0u16; 4];
    for (i, &m) in learned[start..].iter().enumerate() {
        out[i] = m;
    }
    out
}

/// Identifiant de version (`GameVersion` de PKHeX) du premier jeu de la paire.
pub fn default_version_id(v: SaveVersion) -> u8 {
    match v {
        SaveVersion::DiamondPearl => 10,
        SaveVersion::Platinum => 12,
        SaveVersion::HeartGoldSoulSilver => 7,
        SaveVersion::BlackWhite => 21,
        SaveVersion::Black2White2 => 23,
        SaveVersion::XY => 24,
        SaveVersion::OmegaRubyAlphaSapphire => 26,
        SaveVersion::SunMoon => 30,
        SaveVersion::UltraSunUltraMoon => 32,
    }
}

/// Langue française (`LanguageID` de PKHeX).
pub const LANGUAGE_FR: u8 = 3;

fn invalid(e: super::PkmError) -> SaveError {
    SaveError::Invalid(e.to_string())
}

impl SaveSession {
    /// Jeu des données (fiches, attaques, noms) correspondant à la sauvegarde.
    pub fn game(&self) -> Game {
        game_of(self.save.version())
    }

    pub fn open(bytes: &[u8]) -> Result<Self, SaveError> {
        Ok(Self { save: SaveFile::from_bytes(bytes)?, undo: Vec::new(), redo: Vec::new() })
    }

    /// Applique une modification ; en cas d'erreur la sauvegarde est restaurée telle
    /// quelle, sinon l'état précédent rejoint l'historique.
    pub(crate) fn mutate<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, SaveError>) -> Result<T, SaveError> {
        let before = self.save.clone();
        match f(self) {
            Ok(v) => {
                self.undo.push(before);
                if self.undo.len() > HISTORY {
                    self.undo.remove(0);
                }
                self.redo.clear();
                Ok(v)
            }
            Err(e) => {
                self.save = before;
                Err(e)
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.save, prev));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.save, next));
                true
            }
            None => false,
        }
    }

    pub fn view(&self) -> Result<SaveView, SaveError> {
        let s = &self.save;
        let party = s.party()?.iter().enumerate().map(|(i, p)| view_of(self.game(), Slot::Party { index: i }, p)).collect();
        let (trainer_name_max, box_name_max) = s.name_limits();
        Ok(SaveView {
            game: s.version().label(),
            version: s.version(),
            generation: s.generation(),
            trainer: s.trainer(),
            box_names: (0..s.box_count())
                .map(|i| s.box_name(i).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Boîte {}", i + 1)))
                .collect(),
            box_fill: (0..s.box_count()).map(|b| (0..BOX_SLOTS).filter(|&i| matches!(s.box_slot(b, i), Ok(Some(_)))).count()).collect(),
            party,
            warnings: s.warnings().to_vec(),
            needs_resign: s.needs_resign(),
            checksums_valid: s.checksums_valid(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            trainer_name_max,
            box_name_max,
            nickname_max: if s.generation() >= 6 { 12 } else { 10 },
        })
    }

    pub fn box_view(&self, b: usize) -> Result<Vec<Option<SlotView>>, SaveError> {
        (0..BOX_SLOTS)
            .map(|i| {
                let slot = Slot::Box { r#box: b, index: i };
                Ok(self.save.box_slot(b, i)?.filter(|p| !p.is_empty()).map(|p| view_of(self.game(), slot, &p)))
            })
            .collect()
    }

    /// Tous les Pokémon de la sauvegarde (équipe puis boîtes).
    pub fn all(&self) -> Result<Vec<SlotView>, SaveError> {
        let mut out: Vec<SlotView> = self.save.party()?.iter().enumerate().map(|(i, p)| view_of(self.game(), Slot::Party { index: i }, p)).collect();
        for b in 0..self.save.box_count() {
            out.extend(self.box_view(b)?.into_iter().flatten());
        }
        Ok(out)
    }

    pub fn get(&self, slot: Slot) -> Result<Option<Pokemon>, SaveError> {
        let p = match slot {
            Slot::Party { index } => self.save.party_slot(index)?,
            Slot::Box { r#box, index } => self.save.box_slot(r#box, index)?,
        };
        Ok(p.filter(|p| !p.is_empty()))
    }

    fn set(&mut self, slot: Slot, p: Option<Pokemon>) -> Result<(), SaveError> {
        let game = self.game();
        match slot {
            Slot::Party { index } => self.save.set_party_slot(index, p.map(|p| prepare_for_party(game, p))),
            Slot::Box { r#box, index } => self.save.set_box_slot(r#box, index, p),
        }
    }

    /// Écrit un Pokémon dans un emplacement ; dans l'équipe, un emplacement libre
    /// devient la fin de l'équipe. Renvoie l'emplacement réellement utilisé.
    pub(crate) fn put(&mut self, slot: Slot, p: Pokemon) -> Result<Slot, SaveError> {
        let slot = match slot {
            Slot::Party { index } if index >= self.save.party_count() => {
                let count = self.save.party_count();
                if count >= PARTY_SLOTS {
                    return Err(SaveError::Invalid("l'équipe est complète".into()));
                }
                Slot::Party { index: count }
            }
            s => s,
        };
        self.set(slot, Some(p))?;
        Ok(slot)
    }

    /// Échange deux emplacements (ou déplace vers un emplacement vide).
    pub fn move_pokemon(&mut self, from: Slot, to: Slot) -> Result<(), SaveError> {
        if from == to {
            return Ok(());
        }
        self.mutate(|s| s.move_inner(from, to))
    }

    fn move_inner(&mut self, from: Slot, to: Slot) -> Result<(), SaveError> {
        let a = self.get(from)?.ok_or_else(|| SaveError::Invalid("emplacement de départ vide".into()))?;
        let b = self.get(to)?;
        match (to, b) {
            // Ajout à la fin de l'équipe.
            (Slot::Party { index }, None) => {
                let count = self.save.party_count();
                if index >= PARTY_SLOTS || count >= PARTY_SLOTS {
                    return Err(SaveError::Invalid("l'équipe est complète".into()));
                }
                if matches!(from, Slot::Party { .. }) {
                    return Ok(()); // déjà dans l'équipe : l'ordre ne change pas
                }
                self.save.set_party_slot(count, Some(prepare_for_party(self.game(), a)))?;
                self.set(from, None)
            }
            (_, Some(b)) => {
                self.set(from, Some(b))?;
                self.set(to, Some(a))
            }
            (_, None) => {
                if let Slot::Party { .. } = from {
                    if self.save.party_count() <= 1 {
                        return Err(SaveError::Invalid("l'équipe doit garder au moins un Pokémon".into()));
                    }
                }
                self.set(to, Some(a))?;
                self.set(from, None)
            }
        }
    }

    /// Copie un Pokémon sur un autre emplacement (Maj + glisser). La cible est écrasée.
    pub fn clone_pokemon(&mut self, from: Slot, to: Slot) -> Result<Slot, SaveError> {
        if from == to {
            return Ok(to);
        }
        self.mutate(|s| {
            let a = s.get(from)?.ok_or_else(|| SaveError::Invalid("emplacement de départ vide".into()))?;
            s.put(to, a)
        })
    }

    /// Déplace un Pokémon en écrasant la cible (Alt + glisser) : l'ancien occupant est supprimé.
    pub fn overwrite_pokemon(&mut self, from: Slot, to: Slot) -> Result<(), SaveError> {
        if from == to {
            return Ok(());
        }
        self.mutate(|s| {
            if s.get(to)?.is_none() {
                return s.move_inner(from, to);
            }
            let a = s.get(from)?.ok_or_else(|| SaveError::Invalid("emplacement de départ vide".into()))?;
            s.set(to, Some(a))?;
            // Équipe → équipe : la suppression décale les suivants, l'ordre final reste correct.
            s.set(from, None)
        })
    }

    pub fn delete(&mut self, slot: Slot) -> Result<(), SaveError> {
        self.mutate(|s| s.set(slot, None))
    }

    /// Place un Pokémon importé (fichier .pk*) dans un emplacement.
    pub fn import(&mut self, slot: Slot, bytes: &[u8]) -> Result<SlotView, SaveError> {
        let p = Pokemon::from_bytes(self.save.format(), bytes).map_err(|e| SaveError::Invalid(e.to_string()))?;
        if p.is_empty() {
            return Err(SaveError::Invalid("fichier Pokémon vide".into()));
        }
        let slot = self.mutate(|s| s.put(slot, p))?;
        self.view_slot(slot)
    }

    /// Crée un Pokémon neuf, attrapé par le dresseur de la sauvegarde.
    pub fn create(&mut self, slot: Slot, species: u16, level: u8) -> Result<SlotView, SaveError> {
        if dex::species_name(species).is_none() {
            return Err(SaveError::Invalid(format!("espèce n°{species} inconnue")));
        }
        let p = self.new_pokemon(species, level)?;
        let slot = self.mutate(|s| s.put(slot, p))?;
        self.view_slot(slot)
    }

    pub(crate) fn new_pokemon(&self, species: u16, level: u8) -> Result<Pokemon, SaveError> {
        let format = self.save.format();
        let t = self.save.trainer();
        let level = level.clamp(1, 100);
        let mut p = Pokemon::blank(format);
        // Graine tirée de l'horloge : PID et EC différents à chaque création.
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x1234_5678, |d| d.subsec_nanos() ^ d.as_secs() as u32);
        let mut r = seed;
        let mut next = || {
            r = r.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
            r
        };
        p.set_encryption_constant(next());
        if format.generation() >= 6 {
            p.set_pid(next());
        }
        p.set_species(species);
        p.set_tid(t.tid);
        p.set_sid(t.sid);
        p.set_ot_name(&t.name).map_err(invalid)?;
        p.set_ot_gender(t.gender);
        p.set_nickname(dex::species_name(species).unwrap_or("?")).map_err(invalid)?;
        p.set_is_nicknamed(false);
        p.set_language(LANGUAGE_FR);
        p.set_version(default_version_id(self.save.version()));
        p.set_ball(4);
        p.set_met_level(level).map_err(invalid)?;
        let game = self.game();
        if let Some(g) = growth(game, species, 0) {
            p.set_exp(exp_for_level(g, level));
        }
        if format != PkmFormat::Gen4 {
            p.set_nature((next() >> 16) as u8 % 25).map_err(invalid)?;
        }
        let info = dex::personal(game, species, 0);
        // Sexe et talent cohérents avec le PID (règle des Gen 3 à 5, reprise par PKHeX).
        if format.generation() >= 6 {
            p.set_ability_number(1).map_err(invalid)?;
        }
        let slot = (p.ability_number() == 2) as usize;
        match info {
            Some(info) => {
                let ability = match info.abilities[slot] {
                    0 => info.abilities[0],
                    a => a,
                };
                p.set_ability(ability).map_err(invalid)?;
                p.set_gender(gender_from_pid(info.gender_ratio, p.pid()));
                p.set_friendship(info.base_friendship);
            }
            None => {
                if let Some(a) = names::first_ability(species) {
                    p.set_ability(a).map_err(invalid)?;
                }
                p.set_friendship(70);
            }
        }
        let moves = suggested_moves(game, species, 0, level);
        let moves = if moves == [0; 4] { [33, 0, 0, 0] } else { moves };
        p.set_moves(moves);
        p.set_pp(moves.map(|m| dex::move_info_in(game, m).map_or(0, |i| i.pp)));
        let now = today();
        p.set_met_date(Some(now));
        p.refresh_checksum();
        Ok(p)
    }

    pub fn view_slot(&self, slot: Slot) -> Result<SlotView, SaveError> {
        let p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        Ok(view_of(self.game(), slot, &p))
    }

    pub fn patch(&mut self, slot: Slot, patch: &PokemonPatch) -> Result<SlotView, SaveError> {
        self.mutate(|s| {
            let p = s.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
            let p = apply_patch(s.game(), p, patch)?;
            s.set(slot, Some(p))
        })?;
        self.view_slot(slot)
    }

    pub fn set_trainer(&mut self, patch: &TrainerPatch) -> Result<(), SaveError> {
        self.mutate(|s| s.save.set_trainer(patch))
    }

    pub fn set_box_name(&mut self, index: usize, name: &str) -> Result<(), SaveError> {
        self.mutate(|s| s.save.set_box_name(index, name))
    }

    /// Données déchiffrées d'un Pokémon (fichier .pk4 / .pk5 / .pk6 / .pk7).
    pub fn export(&self, slot: Slot) -> Result<(Vec<u8>, String), SaveError> {
        let p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        let ext = format!("pk{}", self.save.generation());
        Ok((p.stored_data().to_vec(), ext))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.save.to_bytes()
    }
}

/// Date du jour (UTC), pour les dates de rencontre.
pub fn today() -> PkmDate {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Algorithme « civil_from_days » de Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = (yoe + era * 400 + (month <= 2) as i64) as u16;
    PkmDate { year, month, day }
}

pub(crate) fn apply_patch(game: Game, mut p: Pokemon, patch: &PokemonPatch) -> Result<Pokemon, SaveError> {
    if let Some(species) = patch.species {
        if dex::species_name(species).is_none() || species > dex::max_species(game) {
            return Err(SaveError::Invalid(format!("l'espèce n°{species} n'existe pas dans ce jeu")));
        }
        let level = p.level(growth(game, p.species(), p.form()));
        let renamed = !p.is_nicknamed();
        p.set_species(species);
        // Forme inexistante pour la nouvelle espèce : forme de base.
        if patch.form.is_none() && dex::form_names(game, species).len() <= p.form() as usize {
            p.set_form(0).map_err(invalid)?;
        }
        // Talent du même emplacement chez la nouvelle espèce.
        if patch.ability.is_none() {
            if let Some(info) = dex::personal(game, species, p.form()) {
                let i = match p.ability_number() {
                    4 => 2,
                    2 => 1,
                    _ => 0,
                };
                let a = if info.abilities[i] == 0 { info.abilities[0] } else { info.abilities[i] };
                p.set_ability(a).map_err(invalid)?;
            }
        }
        // Même niveau avec la courbe de la nouvelle espèce.
        if let (Some(level), Some(g)) = (level, growth(game, species, p.form())) {
            p.set_exp(exp_for_level(g, level));
        }
        if renamed {
            p.set_nickname(dex::species_name(species).unwrap_or("?")).map_err(invalid)?;
            p.set_is_nicknamed(false);
        }
    }
    if let Some(form) = patch.form {
        p.set_form(form).map_err(invalid)?;
    }
    if let Some(name) = &patch.nickname {
        p.set_nickname(name).map_err(invalid)?;
        p.set_is_nicknamed(dex::species_name(p.species()) != Some(name.as_str()));
    }
    if patch.is_nicknamed == Some(false) {
        p.set_nickname(dex::species_name(p.species()).unwrap_or("?")).map_err(invalid)?;
        p.set_is_nicknamed(false);
    }
    if let Some(v) = patch.encryption_constant {
        if p.format().generation() >= 6 {
            p.set_encryption_constant(v);
        }
    }
    if let Some(v) = patch.pid {
        p.set_pid(v);
    }
    if let Some(n) = patch.nature {
        p.set_nature_any(n).map_err(invalid)?;
    }
    if let Some(n) = patch.ability_number {
        p.set_ability_number(n).map_err(invalid)?;
    }
    if let Some(a) = patch.ability {
        p.set_ability(a).map_err(invalid)?;
    }
    if let Some(g) = patch.gender {
        p.set_gender(g);
    }
    if let Some(item) = patch.held_item {
        p.set_held_item(item);
    }
    if let Some(l) = patch.language {
        p.set_language(l);
    }
    if let Some(moves) = patch.moves {
        p.set_moves(moves);
    }
    if let Some(pp) = patch.pp {
        p.set_pp(pp);
    }
    if let Some(ups) = patch.pp_ups {
        p.set_pp_ups(ups.map(|u| u.min(3)));
    }
    if let Some(ivs) = patch.ivs {
        p.set_ivs(ivs.map(|v| v.min(31))).map_err(invalid)?;
    }
    if let Some(evs) = patch.evs {
        let mut evs = evs.map(|v| v.min(252));
        // Total limité à 510 : on réduit les dernières valeurs si besoin.
        let mut excess = evs.iter().map(|&v| v as i32).sum::<i32>() - 510;
        for v in evs.iter_mut().rev() {
            if excess <= 0 {
                break;
            }
            let cut = excess.min(*v as i32);
            *v -= cut as u8;
            excess -= cut;
        }
        p.set_evs(evs);
    }
    if let Some(f) = patch.friendship {
        p.set_friendship(f);
    }
    if let Some(name) = &patch.ot_name {
        p.set_ot_name(name).map_err(invalid)?;
    }
    if let Some(v) = patch.tid {
        p.set_tid(v);
    }
    if let Some(v) = patch.sid {
        p.set_sid(v);
    }
    if let Some(g) = patch.ot_gender {
        p.set_ot_gender(g);
    }
    // Chromatique après TID/SID et nature : il dépend des deux.
    if let Some(mode) = patch.shiny {
        p.set_shiny(mode);
    }
    if let Some(b) = patch.ball {
        p.set_ball(b);
    }
    if let Some(v) = patch.met_location {
        p.set_met_location(v);
    }
    if let Some(v) = patch.met_level {
        p.set_met_level(v.min(100)).map_err(invalid)?;
    }
    if let Some(d) = patch.met_date {
        p.set_met_date(d);
    }
    if let Some(v) = patch.egg_location {
        p.set_egg_location(v);
    }
    if let Some(d) = patch.egg_date {
        p.set_egg_date(d);
    }
    if let Some(v) = patch.version {
        p.set_version(v);
    }
    if let Some(v) = patch.fateful_encounter {
        p.set_fateful_encounter(v);
    }
    if let Some(v) = patch.is_egg {
        p.set_is_egg(v);
    }
    if let Some(m) = patch.markings {
        p.set_markings(m);
    }
    if let Some((strain, days)) = patch.pokerus {
        p.set_pokerus(strain, days);
    }
    if let Some(x) = &patch.extras {
        p.apply_extras(x).map_err(invalid)?;
    }
    if let Some(exp) = patch.exp {
        p.set_exp(exp);
    }
    if let Some(level) = patch.level {
        let g = growth(game, p.species(), p.form()).ok_or_else(|| SaveError::Invalid("courbe d'expérience inconnue pour cette espèce".into()))?;
        p.set_exp(exp_for_level(g, level.clamp(1, 100)));
    }
    if p.party_level().is_some() {
        p = prepare_for_party(game, p);
    }
    p.refresh_checksum();
    Ok(p)
}

/// Recalcule niveau et statistiques de la section équipe (nécessaire en équipe).
fn prepare_for_party(game: Game, mut p: Pokemon) -> Pokemon {
    if let (Some(base), Some(g)) = (base_stats(game, p.species(), p.form()), growth(game, p.species(), p.form())) {
        p.update_party_stats(&base, g);
        p.refresh_checksum();
    }
    p
}

impl SaveSession {
    pub fn inventory(&self) -> Result<Vec<super::Pouch>, SaveError> {
        self.save.inventory()
    }

    pub fn set_inventory(&mut self, pouches: &[super::Pouch]) -> Result<(), SaveError> {
        self.mutate(|s| s.save.set_inventory(pouches))
    }
}

/// Sexe déduit du PID et du taux de femelles de l'espèce (Gen 3 à 5).
pub fn gender_from_pid(ratio: u8, pid: u32) -> Gender {
    match ratio {
        255 => Gender::Genderless,
        254 => Gender::Female,
        0 => Gender::Male,
        r if ((pid & 0xFF) as u8) < r => Gender::Female,
        _ => Gender::Male,
    }
}

impl SaveSession {
    pub fn pokedex(&self) -> Result<Vec<super::pokedex::DexEntry>, SaveError> {
        self.save.pokedex()
    }

    /// Modifie plusieurs entrées du Pokédex en une seule étape d'historique.
    pub fn set_dex(&mut self, entries: &[super::pokedex::DexEntry]) -> Result<(), SaveError> {
        self.mutate(|s| {
            for e in entries {
                s.save.set_dex_entry(e.species, e.seen, e.caught)?;
            }
            Ok(())
        })
    }

    pub fn dex_set_all(&mut self, seen: bool, caught: bool) -> Result<(), SaveError> {
        self.mutate(|s| s.save.dex_set_all(seen, caught))
    }
}

impl SaveSession {
    /// Écrit plusieurs Pokémon en une seule étape d'historique (« Rendre légal »,
    /// « Tout rendre légal », création d'un Pokémon légal). Renvoie les emplacements utilisés.
    pub fn replace_pokemon(&mut self, items: Vec<(Slot, Pokemon)>) -> Result<Vec<Slot>, SaveError> {
        self.mutate(|s| items.into_iter().map(|(slot, p)| s.put(slot, p)).collect())
    }

    /// Premier emplacement libre des boîtes, en commençant par la boîte `from`.
    pub fn first_empty_box_slot(&self, from: usize) -> Option<Slot> {
        let count = self.save.box_count();
        (0..count)
            .map(|i| (from + i) % count.max(1))
            .find_map(|b| (0..BOX_SLOTS).map(|index| Slot::Box { r#box: b, index }).find(|&slot| matches!(self.get(slot), Ok(None))))
    }
}
