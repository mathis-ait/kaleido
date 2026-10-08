//! Jeu Pokémon Game Boy (Gen 1 : Rouge, Bleu, Jaune ; Gen 2 : Or, Argent, Cristal).
//!
//! Emplacements : fichiers d'offsets de l'Universal Pokémon Randomizer (UPR-ZX, GPLv3,
//! `data/upr/gen1_offsets.ini` et `gen2_offsets.ini`, repris tels quels). Une version est
//! reconnue comme le fait `Gen1RomHandler.checkRomEntry` / `Gen2RomHandler.checkRomEntry` :
//! titre (Gen 1) ou code GBC (Gen 2), version, drapeau « hors Japon », puis la somme
//! globale de l'en-tête quand le fichier la précise (Rouge et Bleu français, allemands…
//! ont le même titre que les versions américaines).
//!
//! **Non vérifié sur une vraie ROM** : aucune ROM Gen 1 ou 2 n'était disponible. Les
//! tests construisent des ROM synthétiques.

use std::path::Path;
use std::sync::LazyLock;

use kaleido_formats::gb::{GbHeader, GbRom};

use crate::gba_rom::{parse_offsets, RomEntry};
use crate::games::Game;
use crate::pokemon::Species;
use crate::rom::RomError;

static GEN1: LazyLock<Vec<RomEntry>> = LazyLock::new(|| parse_offsets(include_str!("../data/upr/gen1_offsets.ini")));
static GEN2: LazyLock<Vec<RomEntry>> = LazyLock::new(|| parse_offsets(include_str!("../data/upr/gen2_offsets.ini")));

/// Section du fichier d'offsets correspondant à l'en-tête.
pub fn entry_for(data: &[u8]) -> Option<(&'static RomEntry, u8)> {
    let h = GbHeader::parse(data)?;
    let title = GbHeader::raw_title(data);
    let crc = u16::from_be_bytes([data[0x14E], data[0x14F]]) as usize;
    let nonjap = h.non_japanese as usize;
    let matches = |e: &&RomEntry, gen: u8| {
        let sig_ok = if gen == 1 { title.starts_with(e.code.as_str()) } else { h.code == e.code };
        sig_ok && e.version == h.version && e.value("NonJapanese").unwrap_or(0) == nonjap
    };
    for (list, gen) in [(&*GEN1, 1u8), (&*GEN2, 2u8)] {
        if let Some(e) = list.iter().find(|e| matches(e, gen) && e.value("CRCInHeader") == Some(crc)) {
            return Some((e, gen));
        }
    }
    for (list, gen) in [(&*GEN1, 1u8), (&*GEN2, 2u8)] {
        if let Some(e) = list.iter().find(|e| matches(e, gen) && e.value("CRCInHeader").is_none()) {
            return Some((e, gen));
        }
    }
    None
}

/// Langue d'après le nom de la section du fichier d'offsets.
pub fn entry_language(e: &RomEntry) -> Option<char> {
    let inner = e.name.split('(').nth(1)?;
    inner.chars().next().map(|c| if c == 'U' { 'E' } else { c })
}

/// Jeu GB chargé en mémoire.
#[derive(Debug, Clone)]
pub struct GbGameRom {
    pub game: Game,
    pub generation: u8,
    pub entry: &'static RomEntry,
    rom: GbRom,
    /// Gen 1 : n° national de chaque espèce interne (index 1 à 190) ; Gen 2 : identité.
    internal_to_national: Vec<u16>,
    national_to_internal: Vec<u16>,
}

fn layout_err(what: &str) -> RomError {
    RomError::Layout(format!("{what} introuvable (ROM modifiée ou version non reconnue ?)"))
}

impl GbGameRom {
    pub fn open(path: &Path) -> Result<Self, RomError> {
        Self::from_rom(GbRom::open(path)?)
    }

    pub fn from_rom(rom: GbRom) -> Result<Self, RomError> {
        let h = rom.header().clone();
        let game = Game::from_gb(&GbHeader::raw_title(rom.data()), &h.code)
            .ok_or_else(|| RomError::Unsupported(format!("jeu Game Boy « {} » non pris en charge", h.title)))?;
        let (entry, generation) = entry_for(rom.data()).ok_or_else(|| RomError::Unsupported(format!("{} : version inconnue de la table d'offsets", game.name_fr())))?;
        if entry.value("NonJapanese").unwrap_or(0) == 0 {
            return Err(RomError::Unsupported("les ROM japonaises ne sont pas prises en charge".into()));
        }
        let count = Self::species_count_for(generation) as usize;
        let mut internal_to_national = vec![0u16; 256];
        let mut national_to_internal = vec![0u16; count + 1];
        if generation == 1 {
            let order = entry.value("PokedexOrder").ok_or_else(|| layout_err("ordre du Pokédex"))?;
            let internal = entry.value("InternalPokemonCount").unwrap_or(190);
            for i in 1..=internal {
                let n = rom.u8(order + i - 1).ok_or_else(|| layout_err("ordre du Pokédex"))? as u16;
                if n != 0 && n as usize <= count {
                    internal_to_national[i] = n;
                    if national_to_internal[n as usize] == 0 {
                        national_to_internal[n as usize] = i as u16;
                    }
                }
            }
            if (1..=count).any(|n| national_to_internal[n] == 0) {
                return Err(layout_err("table de l'ordre du Pokédex complète"));
            }
        } else {
            for n in 1..=count {
                internal_to_national[n] = n as u16;
                national_to_internal[n] = n as u16;
            }
        }
        let g = Self { game, generation, entry, rom, internal_to_national, national_to_internal };
        g.personal_offset(count as u16).filter(|&at| at + g.record_size() <= g.rom.len()).ok_or_else(|| layout_err("fiches des espèces"))?;
        Ok(g)
    }

    fn species_count_for(generation: u8) -> u16 {
        if generation == 1 {
            151
        } else {
            251
        }
    }

    pub fn species_count(&self) -> u16 {
        Self::species_count_for(self.generation)
    }

    pub fn is_yellow(&self) -> bool {
        self.entry.string("Type") == Some("Yellow")
    }

    pub fn is_crystal(&self) -> bool {
        self.entry.string("Type") == Some("Crystal")
    }

    /// Emplacements vérifiés sur une vraie ROM : jamais pour l'instant.
    pub fn verified(&self) -> bool {
        false
    }

    pub fn rom(&self) -> &GbRom {
        &self.rom
    }

    pub fn rom_mut(&mut self) -> &mut GbRom {
        &mut self.rom
    }

    pub fn save(&self, path: &Path) -> Result<(), RomError> {
        Ok(self.rom.save(path)?)
    }

    /// N° national d'une espèce interne (Gen 1) ; identité en Gen 2.
    pub fn national(&self, internal: u8) -> u16 {
        self.internal_to_national.get(internal as usize).copied().unwrap_or(0)
    }

    pub fn internal(&self, national: u16) -> u8 {
        self.national_to_internal.get(national as usize).copied().unwrap_or(0) as u8
    }

    /// Taille d'une fiche dans la ROM (0x1C en Gen 1, 0x20 en Gen 2).
    pub fn record_size(&self) -> usize {
        if self.generation == 1 {
            0x1C
        } else {
            0x20
        }
    }

    /// Offset de la fiche d'une espèce (Mew est à part dans Rouge et Bleu).
    pub fn personal_offset(&self, national: u16) -> Option<usize> {
        if national == 0 || national > self.species_count() {
            return None;
        }
        if national == 151 && self.generation == 1 {
            if let Some(mew) = self.entry.value("MewStatsOffset") {
                return Some(mew);
            }
        }
        Some(self.entry.value("PokemonStatsOffset")? + (national as usize - 1) * self.record_size())
    }

    pub fn personal(&self, national: u16) -> Option<&[u8]> {
        self.rom.bytes(self.personal_offset(national)?, self.record_size())
    }

    pub fn set_personal(&mut self, national: u16, data: &[u8]) -> Result<(), RomError> {
        let at = self.personal_offset(national).ok_or_else(|| layout_err("fiche"))?;
        let size = self.record_size();
        self.rom.write(at, &data[..size])?;
        Ok(())
    }

    /// Pokédex de la ROM (noms français de Kaleido, fiches lues dans la ROM).
    pub fn species(&self) -> Result<Vec<Species>, RomError> {
        let names: Vec<String> = crate::dex::species_names().iter().map(|s| s.to_string()).collect();
        let abilities: Vec<String> = crate::dex::ability_names().iter().map(|s| s.to_string()).collect();
        let table: Vec<Vec<u8>> = (0..=self.species_count())
            .map(|n| if n == 0 { vec![0; 0x2C] } else { self.personal(n).map(|r| crate::data::gen12::to_gen4_record(self.generation, r)).unwrap_or_else(|| vec![0; 0x2C]) })
            .collect();
        crate::rom::assemble_species(self.game, self.species_count(), &names, &abilities, &table)
    }
}

#[cfg(test)]
pub(crate) mod synthetic {
    //! ROM Gen 1 / Gen 2 synthétique pour les tests : en-tête valide et tables aux
    //! emplacements du fichier d'offsets. Aucune donnée de jeu réelle.
    use super::*;
    use kaleido_formats::gb::synthetic as header;

    pub const SIZE: usize = 0x20_0000;

    /// Ordre interne Gen 1 synthétique : l'espèce interne `i` est le n° national `i` (1 à 151).
    pub fn build(section: &str) -> GbRom {
        let (list, gen) = if GEN1.iter().any(|e| e.name == section) { (&*GEN1, 1u8) } else { (&*GEN2, 2u8) };
        let e = list.iter().find(|e| e.name == section).expect("section connue");
        let (title, code) = if gen == 1 { (e.code.clone(), String::new()) } else { ("POKEMON".to_string(), e.code.clone()) };
        let mut d = header(&title, &code, e.version, SIZE);
        if let Some(crc) = e.value("CRCInHeader") {
            d[0x14E..0x150].copy_from_slice(&(crc as u16).to_be_bytes());
        }
        let put = |d: &mut Vec<u8>, at: usize, bytes: &[u8]| d[at..at + bytes.len()].copy_from_slice(bytes);
        let count = if gen == 1 { 151usize } else { 251 };
        if gen == 1 {
            let order = e.value("PokedexOrder").unwrap();
            for i in 1..=190usize {
                d[order + i - 1] = if i <= 151 { i as u8 } else { 0 };
            }
        }
        // Fiches : tables de PKHeX (même format que la ROM).
        let pkhex: &[u8] = if gen == 1 { include_bytes!("../data/pkhex/personal/personal_rb") } else { include_bytes!("../data/pkhex/personal/personal_c") };
        let size = if gen == 1 { 0x1C } else { 0x20 };
        let stats = e.value("PokemonStatsOffset").unwrap();
        for n in 1..=count {
            let at = if n == 151 && gen == 1 { e.value("MewStatsOffset").unwrap_or(stats + 150 * size) } else { stats + (n - 1) * size };
            put(&mut d, at, &pkhex[n * size..(n + 1) * size]);
        }
        // Évolutions et attaques : une table de pointeurs dans la banque, puis des listes
        // « évolutions (0) puis attaques (0) ». Bulbizarre → Herbizarre niv. 16 ; sinon aucune évolution.
        let table = e.value("PokemonMovesetsTableOffset").unwrap();
        let bank = table / 0x4000;
        let data_at = bank * 0x4000 + if gen == 1 { 0x1500 } else { 0x3000 };
        let entries = if gen == 1 { 190 } else { 251 };
        let mut at = data_at;
        for i in 1..=entries {
            let ptr = kaleido_formats::gb::offset_pointer(at);
            put(&mut d, table + (i - 1) * 2, &ptr.to_le_bytes());
            let target = if gen == 1 { 2u8 } else { 2 };
            let list: Vec<u8> = if i == 1 { vec![1, 16, target, 0, 7, 22, 13, 73, 0] } else { vec![0, 5, 33, 9, 45, 0] };
            put(&mut d, at, &list);
            at += list.len();
        }
        // Starters (internes = nationaux).
        for (k, s) in [1u8, 4, 7].into_iter().enumerate() {
            for off in e.array(&format!("StarterOffsets{}", k + 1)) {
                d[off] = s;
            }
        }
        // Rencontres (Gen 1) : une carte avec de l'herbe (10 emplacements) puis la fin 0xFFFF.
        if gen == 1 {
            let wild = e.value("WildPokemonTableOffset").unwrap();
            let wbank = wild / 0x4000;
            let area = wbank * 0x4000 + 0x3F00;
            put(&mut d, wild, &kaleido_formats::gb::offset_pointer(area).to_le_bytes());
            put(&mut d, wild + 2, &[0xFF, 0xFF]);
            let mut a = vec![25u8];
            for k in 0..10u8 {
                a.extend([3 + k % 2, 16 + (k % 3)]);
            }
            a.push(0); // pas d'eau
            put(&mut d, area, &a);
            // Cannes : vieille (espèce, niveau), super (2 emplacements).
            let old = e.value("OldRodOffset").unwrap();
            put(&mut d, old, &[0, 129, 5]);
            let good = e.value("GoodRodOffset").unwrap();
            put(&mut d, good, &[10, 118, 10, 60]);
            let sup = e.value("SuperRodTableOffset").unwrap();
            d[sup] = 0xFF;
        } else {
            let wild = e.value("WildPokemonOffset").unwrap();
            // Johto herbe : une carte (groupe, n°, 3 taux, 3 × 7 emplacements), puis fin ; eau, Kanto… vides.
            let mut a = vec![1u8, 1, 10, 10, 10];
            for _ in 0..3 {
                for k in 0..7u8 {
                    a.extend([2 + k, 16 + k % 3]);
                }
            }
            a.extend([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
            put(&mut d, wild, &a);
        }
        // Dresseurs : un seul dresseur dans la première classe ayant un dresseur.
        let tr = e.value("TrainerDataTableOffset").unwrap();
        let tbank = tr / 0x4000;
        let counts = e.array("TrainerDataClassCounts");
        let classes = if gen == 1 { 47 } else { e.value("TrainerClassAmount").unwrap() };
        let tdata = tbank * 0x4000 + if gen == 1 { 0x3300 } else { 0x3000 };
        let mut cur = tdata;
        let mut first = true;
        for c in 0..classes {
            let idx = if gen == 1 { c + 1 } else { c };
            put(&mut d, tr + c * 2, &kaleido_formats::gb::offset_pointer(cur).to_le_bytes());
            for _ in 0..counts.get(idx).copied().unwrap_or(0) {
                // Le premier dresseur a deux Pokémon, les autres une équipe vide (place comptée).
                let t: Vec<u8> = match (gen, first) {
                    (1, true) => vec![0xFF, 5, 16, 7, 19, 0],
                    (1, false) => vec![5, 0],
                    (_, true) => vec![0x80, 0x50, 0, 5, 16, 7, 19, 0xFF],
                    _ => vec![0x50, 0, 0xFF],
                };
                first = false;
                put(&mut d, cur, &t);
                cur += t.len();
            }
        }
        GbRom::from_bytes(d).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_files() {
        let red_fr = GEN1.iter().find(|e| e.name == "Red (F)").unwrap();
        assert_eq!(red_fr.value("CRCInHeader"), Some(0x7AFC));
        assert_eq!(red_fr.value("PokemonStatsOffset"), Some(0x383DE)); // copié depuis Red (U)
        assert_eq!(entry_language(red_fr), Some('F'));
        let crystal_fr = GEN2.iter().find(|e| e.name == "Crystal (F)").unwrap();
        assert_eq!(crystal_fr.code, "BYTF");
        assert!(crystal_fr.value("TrainerDataTableOffset").is_some());
        assert!(!crystal_fr.array("StarterOffsets1").is_empty());
    }

    #[test]
    fn open_synthetic_roms() {
        for (section, game) in [("Red (F)", Game::Red), ("Blue (U)", Game::Blue), ("Yellow (F)", Game::Yellow), ("Gold (F)", Game::Gold), ("Crystal (F)", Game::Crystal)] {
            let g = GbGameRom::from_rom(synthetic::build(section)).unwrap_or_else(|e| panic!("{section} : {e}"));
            assert_eq!(g.game, game, "{section}");
            assert_eq!(g.entry.name, section);
            let species = g.species().unwrap();
            assert_eq!(species.len(), g.species_count() as usize);
            assert_eq!(species[0].base_stats.hp, 45, "{section}");
            assert_eq!(species[0].types.len(), 2);
            assert!(!g.verified());
        }
    }
}
