//! Jeu Pokémon Game Boy Advance (Gen 3) ouvert depuis une ROM `.gba`.
//!
//! Les tables sont trouvées comme l'Universal Pokémon Randomizer (UPR-ZX, GPLv3,
//! `Gen3RomHandler.loadedRom`) : fichier d'offsets par code jeu et version
//! (`data/upr/gen3_offsets.ini`, repris tel quel), pointeurs fixes de l'en-tête du
//! programme pour Émeraude / Rouge Feu / Vert Feuille (0x1BC fiches, 0x1CC attaques…),
//! et recherche par signature de code pour l'ordre du Pokédex, les rencontres sauvages
//! et les cartes.
//!
//! **Non vérifié sur une vraie ROM** : aucune ROM Gen 3 n'était disponible pendant le
//! développement. Les tests construisent des images synthétiques (en-tête valide,
//! signatures et tables aux emplacements attendus). [`GbaGameRom::verified`] vaut donc
//! toujours `false` et le journal du randomizer le signale.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use kaleido_formats::gba::{hex, GbaRom};

use crate::games::Game;
use crate::pokemon::Species;
use crate::rom::RomError;

/// Famille de jeu (champ `Type` du fichier d'offsets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gen3Kind {
    Ruby,
    Sapphire,
    Emerald,
    FireRedLeafGreen,
}

impl Gen3Kind {
    pub fn is_rs(self) -> bool {
        matches!(self, Gen3Kind::Ruby | Gen3Kind::Sapphire)
    }

    pub fn is_frlg(self) -> bool {
        self == Gen3Kind::FireRedLeafGreen
    }
}

/// Rencontre fixe (`StaticPokemon{}`) : emplacements de l'espèce (u16) et des niveaux (u8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticDef {
    pub species: Vec<usize>,
    pub levels: Vec<usize>,
    /// Commentaire de la ligne dans le fichier d'offsets (« Kyogre », « Lileep »…).
    pub label: String,
}

/// Une section du fichier d'offsets (une version d'un jeu dans une langue).
#[derive(Debug, Clone, Default)]
pub struct RomEntry {
    pub name: String,
    pub code: String,
    pub version: u8,
    pub kind: Option<Gen3Kind>,
    values: HashMap<String, i64>,
    arrays: HashMap<String, Vec<i64>>,
    strings: HashMap<String, String>,
    pub statics: Vec<StaticDef>,
    pub roamers: Vec<StaticDef>,
    copy_static: bool,
}

impl RomEntry {
    pub fn value(&self, key: &str) -> Option<usize> {
        self.values.get(key).copied().filter(|&v| v > 0).map(|v| v as usize)
    }

    pub fn array(&self, key: &str) -> Vec<usize> {
        self.arrays.get(key).map(|v| v.iter().map(|&x| x as usize).collect()).unwrap_or_default()
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }
}

/// Entier décimal ou hexadécimal (`0x…`, `&h…`), comme `parseRIInt`.
fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("&h")) {
        i64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// `{Species=[0x1, 0x2], Level=[0x3]} // Commentaire`.
fn parse_static(value: &str) -> StaticDef {
    let (body, label) = match value.split_once("//") {
        Some((b, l)) => (b, l.trim().to_string()),
        None => (value, String::new()),
    };
    let mut def = StaticDef { label, ..Default::default() };
    for part in body.split(']') {
        let Some((key, list)) = part.split_once("=[") else { continue };
        let key = key.trim_matches(|c: char| !c.is_ascii_alphabetic());
        let offsets: Vec<usize> = list.split(',').filter_map(parse_int).map(|v| v as usize).collect();
        match key {
            "Species" => def.species = offsets,
            "Level" => def.levels = offsets,
            _ => {}
        }
    }
    def
}

/// Lit un fichier d'offsets UPR (`gen3_offsets.ini`, même logique que `Gen3RomHandler`).
pub fn parse_offsets(text: &str) -> Vec<RomEntry> {
    let mut roms: Vec<RomEntry> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        // Commentaires en début de ligne seulement : `StaticPokemon{}` en garde un en fin de ligne.
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            roms.push(RomEntry { name: line[1..line.len() - 1].to_string(), ..Default::default() });
            continue;
        }
        let Some(current) = roms.last_mut() else { continue };
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "StaticPokemon{}" => current.statics.push(parse_static(value)),
            "RoamingPokemon{}" => current.roamers.push(parse_static(value)),
            "TMText[]" | "MoveTutorText[]" => {}
            "Game" => current.code = value.to_string(),
            "Version" => current.version = parse_int(value).unwrap_or(0) as u8,
            "Type" => {
                current.strings.insert("Type".into(), value.to_string());
                current.kind = match value.to_ascii_lowercase().as_str() {
                    "ruby" => Some(Gen3Kind::Ruby),
                    "sapp" => Some(Gen3Kind::Sapphire),
                    "em" => Some(Gen3Kind::Emerald),
                    "frlg" => Some(Gen3Kind::FireRedLeafGreen),
                    _ => None,
                }
            }
            "TableFile" | "CRC32" => {
                current.strings.insert(key.to_string(), value.to_string());
            }
            "CopyStaticPokemon" => current.copy_static = parse_int(value).unwrap_or(0) > 0,
            "CopyFrom" => {
                let name = value.to_string();
                let Some(other) = roms.iter().rev().skip(1).find(|r| r.name.eq_ignore_ascii_case(&name)).cloned() else { continue };
                let current = roms.last_mut().unwrap();
                current.values.extend(other.values);
                current.arrays.extend(other.arrays);
                current.strings.extend(other.strings);
                if current.copy_static {
                    current.statics.extend(other.statics);
                    current.roamers.extend(other.roamers);
                    current.values.insert("StaticPokemonSupport".into(), 1);
                } else {
                    current.values.insert("StaticPokemonSupport".into(), 0);
                }
            }
            k if k.ends_with("Tweak") => {}
            k if k.ends_with("Locator") || k.ends_with("Prefix") => {
                current.strings.insert(k.to_string(), value.to_string());
            }
            k => {
                if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    let items: Vec<i64> = list.split(',').filter(|s| !s.trim().is_empty()).filter_map(parse_int).collect();
                    current.arrays.insert(k.to_string(), items);
                } else if let Some(v) = parse_int(value) {
                    current.values.insert(k.to_string(), v);
                }
            }
        }
    }
    roms
}

static GEN3_ENTRIES: LazyLock<Vec<RomEntry>> = LazyLock::new(|| parse_offsets(include_str!("../data/upr/gen3_offsets.ini")));

/// Section du fichier d'offsets pour ce code jeu et cette version.
pub fn gen3_entry(code: &str, version: u8) -> Option<&'static RomEntry> {
    GEN3_ENTRIES.iter().find(|e| e.code == code && e.version == version)
}

// --- Signatures de code (`Gen3Constants`).

pub const WILD_POKEMON_PREFIX: &str = "0348048009E00000FFFF0000";
pub const MAP_BANKS_PREFIX: &str = "80180068890B091808687047";
pub const POKEDEX_ORDER_PREFIX: &str = "0448814208D0481C0004000C05E00000";
pub const RS_POKEMON_NAMES_SUFFIX: &str = "30B50025084CC8F7";
/// Pointeurs fixes d'Émeraude et de Rouge Feu / Vert Feuille.
pub const EFRLG_POKEMON_NAMES_POINTER: usize = 0x144;
pub const EFRLG_MOVE_NAMES_POINTER: usize = 0x148;
pub const EFRLG_ABILITY_NAMES_POINTER: usize = 0x1C0;
pub const EFRLG_ITEM_DATA_POINTER: usize = 0x1C8;
pub const EFRLG_MOVE_DATA_POINTER: usize = 0x1CC;
pub const EFRLG_POKEMON_STATS_POINTER: usize = 0x1BC;

/// Taille d'une fiche d'espèce.
pub const BASE_STATS_SIZE: usize = 0x1C;
/// Taille des évolutions d'une espèce (5 entrées de 8 octets).
pub const EVOLUTIONS_SIZE: usize = 0x28;
/// Nombre de Pokémon de la Gen 3 (n° national).
pub const SPECIES_COUNT: u16 = 386;
/// Nombre de CT et de CS.
pub const TM_COUNT: usize = 50;
pub const HM_COUNT: usize = 8;

/// Emplacements des tables, résolus à l'ouverture.
#[derive(Debug, Clone)]
pub struct Gen3Layout {
    /// Nombre d'espèces internes (411 : 386 + 25 emplacements vides entre Celebi et Arcko).
    pub internal_count: usize,
    pub pokedex_order: usize,
    pub stats: usize,
    pub movesets: usize,
    pub evolutions: usize,
    pub tmhm_compat: usize,
    pub tm_moves: usize,
    pub tm_moves_duplicate: Option<usize>,
    pub trainers: usize,
    pub trainer_count: usize,
    pub trainer_entry_size: usize,
    pub trainer_class_names: Option<usize>,
    pub trainer_class_count: usize,
    pub trainer_class_name_length: usize,
    pub starters: usize,
    pub wild: usize,
    pub map_banks: Option<usize>,
    pub move_data: Option<usize>,
    pub free_space: usize,
}

/// Jeu Gen 3 chargé en mémoire.
#[derive(Debug, Clone)]
pub struct GbaGameRom {
    pub game: Game,
    pub kind: Gen3Kind,
    pub entry: &'static RomEntry,
    pub layout: Gen3Layout,
    rom: GbaRom,
    /// N° national de chaque espèce interne (0 : emplacement vide).
    internal_to_national: Vec<u16>,
    /// Espèce interne de chaque n° national.
    national_to_internal: Vec<u16>,
}

fn layout_err(what: &str) -> RomError {
    RomError::Layout(format!("{what} introuvable (ROM modifiée ou version non reconnue ?)"))
}

impl GbaGameRom {
    pub fn open(path: &Path) -> Result<Self, RomError> {
        Self::from_rom(GbaRom::open(path)?)
    }

    pub fn from_rom(rom: GbaRom) -> Result<Self, RomError> {
        let h = rom.header().clone();
        let game = Game::from_gba_code(&h.game_code).ok_or_else(|| RomError::Unsupported(format!("jeu GBA « {} » non pris en charge", h.game_code)))?;
        let entry = gen3_entry(&h.game_code, h.version).ok_or_else(|| {
            RomError::Unsupported(format!("{} : version {} ({}) inconnue de la table d'offsets", game.name_fr(), h.version, h.game_code))
        })?;
        if entry.string("TableFile") == Some("gba_jpn") {
            return Err(RomError::Unsupported("les ROM japonaises ne sont pas prises en charge".into()));
        }
        let kind = entry.kind.ok_or_else(|| layout_err("type de jeu"))?;
        let need = |k: &str| entry.value(k).ok_or_else(|| layout_err(k));

        // Ordre du Pokédex : 3 occurrences de la signature, la 2e pointe la table (+16).
        let dex_hits = rom.find_all(&hex(POKEDEX_ORDER_PREFIX));
        if dex_hits.len() != 3 {
            return Err(layout_err("ordre du Pokédex"));
        }
        let pokedex_order = rom.pointer(dex_hits[1] + 16).ok_or_else(|| layout_err("ordre du Pokédex"))?;
        let stats = if kind.is_rs() { need("PokemonStats")? } else { rom.pointer(EFRLG_POKEMON_STATS_POINTER).ok_or_else(|| layout_err("fiches des espèces"))? };
        let move_data = if kind.is_rs() { entry.value("MoveData") } else { rom.pointer(EFRLG_MOVE_DATA_POINTER) };
        let wild = rom.find(&hex(WILD_POKEMON_PREFIX)).and_then(|at| rom.pointer(at + 12)).ok_or_else(|| layout_err("rencontres sauvages"))?;
        let map_banks = rom.find(&hex(MAP_BANKS_PREFIX)).and_then(|at| rom.pointer(at + 12));

        let layout = Gen3Layout {
            internal_count: need("PokemonCount")?,
            pokedex_order,
            stats,
            movesets: need("PokemonMovesets")?,
            evolutions: need("PokemonEvolutions")?,
            tmhm_compat: need("PokemonTMHMCompat")?,
            tm_moves: need("TmMoves")?,
            tm_moves_duplicate: entry.value("TmMovesDuplicate"),
            trainers: need("TrainerData")?,
            trainer_count: need("TrainerCount")?,
            trainer_entry_size: need("TrainerEntrySize")?,
            trainer_class_names: entry.value("TrainerClassNames"),
            trainer_class_count: entry.value("TrainerClassCount").unwrap_or(0),
            trainer_class_name_length: entry.value("TrainerClassNameLength").unwrap_or(13),
            starters: need("StarterPokemon")?,
            wild,
            map_banks,
            move_data,
            free_space: need("FreeSpace")?,
        };

        let mut internal_to_national = vec![0u16; layout.internal_count + 1];
        let mut national_to_internal = vec![0u16; SPECIES_COUNT as usize + 1];
        for i in 1..=layout.internal_count {
            let dex = rom.u16(pokedex_order + (i - 1) * 2).ok_or_else(|| layout_err("ordre du Pokédex"))?;
            if dex != 0 && dex <= SPECIES_COUNT {
                internal_to_national[i] = dex;
                if national_to_internal[dex as usize] == 0 {
                    national_to_internal[dex as usize] = i as u16;
                }
            }
        }
        if (1..=SPECIES_COUNT as usize).any(|n| national_to_internal[n] == 0) {
            return Err(layout_err("table de l'ordre du Pokédex complète"));
        }
        let end = stats + (layout.internal_count + 1) * BASE_STATS_SIZE;
        if end > rom.len() {
            return Err(layout_err("fiches des espèces"));
        }
        Ok(Self { game, kind, entry, layout, rom, internal_to_national, national_to_internal })
    }

    /// Emplacements vérifiés sur une vraie ROM : jamais pour l'instant (voir l'en-tête du module).
    pub fn verified(&self) -> bool {
        false
    }

    pub fn rom(&self) -> &GbaRom {
        &self.rom
    }

    pub fn rom_mut(&mut self) -> &mut GbaRom {
        &mut self.rom
    }

    pub fn save(&self, path: &Path) -> Result<(), RomError> {
        Ok(self.rom.save(path)?)
    }

    /// N° national d'une espèce interne (0 si emplacement vide ou hors table).
    pub fn national(&self, internal: u16) -> u16 {
        self.internal_to_national.get(internal as usize).copied().unwrap_or(0)
    }

    /// Espèce interne d'un n° national (0 si inconnu).
    pub fn internal(&self, national: u16) -> u16 {
        self.national_to_internal.get(national as usize).copied().unwrap_or(0)
    }

    /// Fiche brute (28 octets) d'une espèce, par n° national.
    pub fn personal(&self, national: u16) -> Option<&[u8]> {
        let i = self.internal(national) as usize;
        (i != 0).then(|| self.rom.bytes(self.layout.stats + i * BASE_STATS_SIZE, BASE_STATS_SIZE)).flatten()
    }

    pub fn set_personal(&mut self, national: u16, data: &[u8]) -> Result<(), RomError> {
        let i = self.internal(national) as usize;
        if i == 0 || data.len() < BASE_STATS_SIZE {
            return Err(RomError::Layout(format!("fiche de l'espèce n°{national}")));
        }
        self.rom.write(self.layout.stats + i * BASE_STATS_SIZE, &data[..BASE_STATS_SIZE])?;
        Ok(())
    }

    /// Fiches de toutes les espèces, index = n° national (l'entrée 0 est vide).
    pub fn personal_table(&self) -> Vec<Vec<u8>> {
        (0..=SPECIES_COUNT).map(|n| if n == 0 { vec![0; BASE_STATS_SIZE] } else { self.personal(n).map(<[u8]>::to_vec).unwrap_or_else(|| vec![0; BASE_STATS_SIZE]) }).collect()
    }

    /// Pokédex de la ROM (noms français de Kaleido, fiches lues dans la ROM).
    pub fn species(&self) -> Result<Vec<Species>, RomError> {
        let names: Vec<String> = crate::dex::species_names().iter().map(|s| s.to_string()).collect();
        let abilities: Vec<String> = crate::dex::ability_names().iter().map(|s| s.to_string()).collect();
        crate::rom::assemble_species(self.game, SPECIES_COUNT, &names, &abilities, &self.personal_table())
    }
}

#[cfg(test)]
pub(crate) mod synthetic {
    //! ROM Gen 3 synthétique pour les tests : en-tête valide, signatures de code et
    //! tables aux emplacements du fichier d'offsets. Aucune donnée de jeu réelle.
    use super::*;
    use kaleido_formats::gba::{synthetic_header, GbaRom};

    pub const SIZE: usize = 0x0100_0000;

    /// Construit une ROM pour `code` / `version` : chaque espèce interne `i` a pour n°
    /// national celui de PKHeX (ordre Gen 3 : 1..=251, 25 vides, puis Hoenn mélangé
    /// comme dans les vrais jeux n'est pas reproduit — ici l'ordre est 1:1 hors des vides).
    pub fn build(code: &str, version: u8) -> GbaRom {
        let entry = gen3_entry(code, version).expect("entrée connue");
        let kind = entry.kind.unwrap();
        let mut d = vec![0xFFu8; SIZE];
        d[..0xC0].copy_from_slice(&synthetic_header("POKEMON SYNT", code, version));
        // Zone de code factice (zéros) pour les signatures.
        d[0x200..0x1000].fill(0);
        let put = |d: &mut Vec<u8>, at: usize, bytes: &[u8]| d[at..at + bytes.len()].copy_from_slice(bytes);
        let ptr = |off: usize| (0x0800_0000u32 + off as u32).to_le_bytes();

        // Tables dans une zone libre propre (au-dessus des offsets réels les plus hauts).
        let dex_table = 0x00F0_0000;
        let wild_headers = 0x00F1_0000;
        let count = entry.value("PokemonCount").unwrap();
        // Ordre du Pokédex : 3 signatures, la 2e suivie (+16) du pointeur.
        let sig = hex(POKEDEX_ORDER_PREFIX);
        for (i, at) in [0x300usize, 0x400, 0x500].into_iter().enumerate() {
            put(&mut d, at, &sig);
            if i == 1 {
                put(&mut d, at + 16, &ptr(dex_table));
            }
        }
        for i in 1..=count {
            let national = internal_to_national_synthetic(i);
            put(&mut d, dex_table + (i - 1) * 2, &national.to_le_bytes());
        }
        put(&mut d, 0x600, &hex(WILD_POKEMON_PREFIX));
        put(&mut d, 0x600 + 12, &ptr(wild_headers));
        // Fin de la liste des en-têtes de rencontres (aucune zone par défaut).
        put(&mut d, wild_headers, &[0xFF, 0xFF]);

        let stats = if kind.is_rs() { entry.value("PokemonStats").unwrap() } else { 0x00F2_0000 };
        if !kind.is_rs() {
            put(&mut d, EFRLG_POKEMON_STATS_POINTER, &ptr(stats));
        }
        // Fiches : la fiche Gen 3 de PKHeX (ordre national) recopiée à l'index interne.
        let pkhex = include_bytes!("../data/pkhex/personal/personal_e");
        for i in 1..=count {
            let n = internal_to_national_synthetic(i) as usize;
            let rec = &pkhex[n * BASE_STATS_SIZE..(n + 1) * BASE_STATS_SIZE];
            put(&mut d, stats + i * BASE_STATS_SIZE, rec);
        }
        // Attaques par niveau : une liste « Charge niv. 1, Rugissement niv. 5 » par espèce.
        let movesets = entry.value("PokemonMovesets").unwrap();
        let learn_data = 0x00F4_0000;
        for i in 0..=count {
            let at = learn_data + i * 8;
            put(&mut d, at, &[33, 1 << 1, 45, 5 << 1, 0xFF, 0xFF, 0, 0]);
            put(&mut d, movesets + i * 4, &ptr(at));
        }
        // Évolutions : vides sauf Bulbizarre → Herbizarre niv. 16 → Florizarre niv. 32.
        let evos = entry.value("PokemonEvolutions").unwrap();
        d[evos..evos + (count + 1) * EVOLUTIONS_SIZE].fill(0);
        put(&mut d, evos + EVOLUTIONS_SIZE, &[4, 0, 16, 0, 2, 0, 0, 0]);
        put(&mut d, evos + 2 * EVOLUTIONS_SIZE, &[4, 0, 32, 0, 3, 0, 0, 0]);
        // Starters (internes = nationaux ici).
        let st = entry.value("StarterPokemon").unwrap();
        if kind.is_frlg() {
            for (off, s) in [(0, 1u16), (5, 4), (515, 4), (520, 7), (461, 7), (466, 1)] {
                put(&mut d, st + off, &s.to_le_bytes());
            }
        } else {
            for (k, s) in [277u16, 280, 283].into_iter().enumerate() {
                put(&mut d, st + k * 2, &s.to_le_bytes());
            }
        }
        // CT : CT01 = Mitra-Poing (264) … valeurs factices 1..=50.
        let tms = entry.value("TmMoves").unwrap();
        for k in 0..TM_COUNT {
            put(&mut d, tms + k * 2, &((k as u16) + 1).to_le_bytes());
        }
        if let Some(dup) = entry.value("TmMovesDuplicate") {
            for k in 0..TM_COUNT {
                put(&mut d, dup + k * 2, &((k as u16) + 1).to_le_bytes());
            }
        }
        let compat = entry.value("PokemonTMHMCompat").unwrap();
        d[compat..compat + (count + 1) * 8].fill(0);
        // Dresseurs : entrées vides sauf le n°1 (deux Pokémon, format 0).
        let tr = entry.value("TrainerData").unwrap();
        let size = entry.value("TrainerEntrySize").unwrap();
        let tcount = entry.value("TrainerCount").unwrap();
        d[tr..tr + tcount * size].fill(0);
        let party = 0x00F6_0000;
        put(&mut d, tr + size, &[0, 1, 0, 0]);
        put(&mut d, tr + size + 4, &crate::text::gen3::encode("TEST", 12, 0xFF));
        d[tr + size + size - 8] = 2;
        put(&mut d, tr + size + size - 4, &ptr(party));
        put(&mut d, party, &[0, 0, 5, 0, 16, 0, 0, 0, 0, 0, 7, 0, 19, 0, 0, 0]);
        // Rencontres fixes : on écrit l'espèce d'origine (internes) à chaque emplacement.
        for (k, s) in entry.statics.iter().enumerate() {
            let species = 144 + (k as u16 % 3);
            for &at in &s.species {
                put(&mut d, at, &species.to_le_bytes());
            }
            for &at in &s.levels {
                d[at] = 50;
            }
        }
        GbaRom::from_bytes(d).unwrap()
    }

    /// Ordre interne synthétique : 1..=251 identiques, 252..=276 vides, puis 277.. = 252..
    pub fn internal_to_national_synthetic(i: usize) -> u16 {
        match i {
            0 => 0,
            1..=251 => i as u16,
            252..=276 => 0,
            _ if i - 25 <= 386 => (i - 25) as u16,
            _ => 0,
        }
    }

    /// Ajoute une zone de rencontres (herbe) au premier en-tête de la liste.
    pub fn add_grass(rom: &mut GbaRom, bank: u8, map: u8, species: &[u16; 12]) {
        let wild = rom.find(&hex(WILD_POKEMON_PREFIX)).and_then(|at| rom.pointer(at + 12)).unwrap();
        let mut at = wild;
        while rom.u16(at) != Some(0xFFFF) {
            at += 20;
        }
        let area = 0x00F8_0000 + (at - wild) * 8;
        let slots = area + 0x10;
        let mut header = vec![bank, map, 0, 0];
        header.extend((0x0800_0000u32 + area as u32).to_le_bytes());
        header.extend([0u8; 12]);
        rom.write(at, &header).unwrap();
        rom.write(at + 20, &[0xFF, 0xFF]).unwrap();
        rom.write(area, &[20, 0, 0, 0]).unwrap();
        rom.write_pointer(area + 4, slots).unwrap();
        for (k, &s) in species.iter().enumerate() {
            rom.write(slots + k * 4, &[2, 4]).unwrap();
            rom.write_u16(slots + k * 4 + 2, s).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_file() {
        let fr = gen3_entry("BPRF", 0).unwrap();
        assert_eq!(fr.kind, Some(Gen3Kind::FireRedLeafGreen));
        assert_eq!(fr.value("PokemonCount"), Some(411));
        assert!(fr.value("TrainerData").is_some() && fr.value("StarterPokemon").is_some());
        let us = gen3_entry("BPRE", 0).unwrap();
        assert_eq!(us.value("StarterPokemon"), Some(0x169BB5));
        assert_eq!(us.statics.len(), 25);
        assert_eq!(us.statics[0].label, "Eevee in Celadon Condominiums");
        assert_eq!(us.statics[0].species, vec![0x16C472, 0x16C475, 0x16C4B5, 0x16C4E9]);
        assert_eq!(us.statics[0].levels, vec![0x16C477]);
        let em = gen3_entry("BPEF", 0).unwrap();
        assert_eq!(em.kind, Some(Gen3Kind::Emerald));
        assert_eq!(em.value("StarterPokemon"), Some(0x5B63E4));
        // Copié depuis Émeraude (U).
        assert_eq!(em.value("TrainerEntrySize"), Some(40));
        assert!(!em.statics.is_empty());
        for code in ["AXVE", "AXPE", "BPEE", "BPRE", "BPGE", "AXVF", "AXPF", "BPEF", "BPRF", "BPGF"] {
            let e = gen3_entry(code, 0).unwrap_or_else(|| panic!("{code}"));
            assert!(e.kind.is_some() && e.value("TrainerData").is_some() && e.value("TmMoves").is_some(), "{code}");
        }
        assert!(gen3_entry("BPRE", 1).is_some());
        assert!(gen3_entry("ZZZZ", 0).is_none());
    }

    #[test]
    fn open_synthetic_roms() {
        for code in ["BPRF", "BPEF", "AXVF", "BPGE", "AXPE"] {
            let g = GbaGameRom::from_rom(synthetic::build(code, 0)).unwrap_or_else(|e| panic!("{code} : {e}"));
            assert_eq!(g.national(1), 1);
            assert_eq!(g.national(277), 252);
            assert_eq!(g.internal(386), 411);
            let species = g.species().unwrap();
            assert_eq!(species.len(), 386);
            assert_eq!(species[0].base_stats.hp, 45); // Bulbizarre
            assert_eq!(species[251].name, "Arcko");
            assert!(!g.verified());
        }
    }
}
