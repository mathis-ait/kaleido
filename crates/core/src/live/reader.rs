//! Lecture de la partie dans la RAM émulée : équipe, carte, badges, équipe adverse en combat.
//!
//! **Équipe.** Les Pokémon restent chiffrés en RAM, au même format que dans la sauvegarde (PK4
//! à PK7). Leur premier mot (PID en Gen 4-5, constante de chiffrement en Gen 6-7) n'est pas
//! chiffré : on cherche en RAM les premiers mots de l'équipe lue dans la sauvegarde, puis on
//! valide chaque emplacement (déchiffrement, somme de contrôle, niveau et PV plausibles). Aucune
//! adresse propre à une version ou une langue n'est nécessaire.
//!
//! Le jeu peut garder plusieurs copies de l'équipe (sauvegarde en RAM, copie de combat). Elles
//! sont toutes suivies ; celle qui a changé le plus récemment est la bonne (PV en combat).
//!
//! **Bloc de sauvegarde.** Sur DS, la RAM garde le bloc « général » de la sauvegarde tel qu'il est
//! écrit dans le fichier : si le nom et le numéro du dresseur sont bien à la même distance de
//! l'équipe que dans le fichier, la carte et les badges se lisent au même endroit. Sinon (3DS,
//! dont la RAM range les blocs autrement), seuls l'équipe et le combat sont lus.
//!
//! **Combat (DS).** Une équipe de jeu DS en RAM est précédée de deux mots : capacité (6) et
//! nombre de Pokémon. Une telle équipe dont aucun Pokémon n'est à nous est une équipe adverse ;
//! un PID jamais vu signale une nouvelle rencontre. Sauvage si le Pokémon porte déjà nos
//! identifiants de dresseur (le jeu les lui donne pour calculer s'il est chromatique).
//!
//! **Lecture au milieu d'une écriture.** Un Pokémon lu à moitié a une somme de contrôle fausse :
//! l'instantané entier est rejeté et relu au tick suivant.

use std::collections::HashSet;

use super::scan::{self, DsRam};
use super::{LiveError, MemorySource, Region};
use crate::save::{PkmFormat, Pokemon, RamHints};

/// Espèce la plus haute connue (Gen 7).
const MAX_SPECIES: u16 = 807;
const PARTY_SLOTS: usize = 6;

/// Console émulée et où se trouve sa RAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Console {
    Ds(DsRam),
    /// 3DS : FCRAM, zones de 64 Mio ou plus de l'émulateur.
    Ctr,
}

/// Décode un Pokémon d'équipe lu en RAM : `None` si l'emplacement est vide ou incohérent.
pub fn decode_party_mon(format: PkmFormat, bytes: &[u8]) -> Option<Pokemon> {
    let plausible = |p: &Pokemon| {
        let species = p.species();
        let Some(stats) = p.party_stats() else { return false };
        p.checksum_valid() && (1..=MAX_SPECIES).contains(&species) && p.current_hp() <= stats[0] && p.party_level().is_some_and(|l| l <= 100)
    };
    if let Ok(p) = Pokemon::from_encrypted(format, bytes) {
        if plausible(&p) {
            return Some(p);
        }
    }
    // Le jeu déchiffre parfois un Pokémon sur place le temps de le lire.
    Pokemon::from_decrypted(format, bytes).ok().filter(plausible)
}

/// Emplacement vide : tout à zéro, ou espèce nulle une fois déchiffré.
fn empty_slot(format: PkmFormat, bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0) || Pokemon::from_encrypted(format, bytes).map(|p| p.species() == 0 && p.checksum_valid()).unwrap_or(false)
}

/// Nombre de Pokémon annoncé par l'en-tête de l'équipe, s'il est lisible et plausible.
fn header_count(src: &dyn MemorySource, format: PkmFormat, slot0: u64) -> Option<usize> {
    let n = match format {
        PkmFormat::Gen4 | PkmFormat::Gen5 => src.read_u32(slot0.checked_sub(4)?).ok()? as usize,
        PkmFormat::Gen6 | PkmFormat::Gen7 => {
            let mut b = [0u8];
            src.read(slot0 + (PARTY_SLOTS * format.party_size()) as u64, &mut b).ok()?;
            b[0] as usize
        }
        // Pas de lecture en mémoire pour les jeux GB/GBA (aucune carte).
        PkmFormat::Gen1 | PkmFormat::Gen2 | PkmFormat::Gen3 => return None,
    };
    (1..=PARTY_SLOTS).contains(&n).then_some(n)
}

/// Équipe lue à `slot0`.
#[derive(Debug, Clone)]
pub struct PartyAt {
    pub mons: Vec<Pokemon>,
    /// Octets bruts (chiffrés) des emplacements occupés : pour savoir si la copie a changé.
    pub raw: Vec<u8>,
}

/// Lit l'équipe à `slot0`. `Ok(None)` : pas (ou plus) une équipe, ou lue pendant une écriture.
pub fn read_party_at(src: &dyn MemorySource, format: PkmFormat, slot0: u64) -> Result<Option<PartyAt>, LiveError> {
    let size = format.party_size();
    let raw = src.read_vec(slot0, size * PARTY_SLOTS)?;
    let count = header_count(src, format, slot0);
    let mut mons = Vec::new();
    for chunk in raw.chunks_exact(size) {
        match decode_party_mon(format, chunk) {
            Some(p) => mons.push(p),
            None if empty_slot(format, chunk) => break,
            // Ni valide ni vide : écriture en cours (ou ce n'est pas une équipe).
            None => {
                if count.is_some_and(|n| mons.len() >= n) {
                    break;
                }
                return Ok(None);
            }
        }
    }
    if mons.is_empty() {
        return Ok(None);
    }
    // L'en-tête, quand il existe, fait foi : les emplacements suivants peuvent garder un ancien Pokémon.
    if let Some(n) = count {
        if n > mons.len() {
            return Ok(None);
        }
        mons.truncate(n);
    }
    let used = mons.len() * size;
    Ok(Some(PartyAt { mons, raw: raw[..used].to_vec() }))
}

/// Combat en cours ou dernier combat vu.
#[derive(Debug, Clone)]
pub struct Battle {
    /// Équipe adverse, dans l'ordre du jeu.
    pub enemies: Vec<Pokemon>,
    /// Sauvage (les Pokémon portent les identifiants du joueur) ou dresseur.
    pub wild: bool,
    /// Premier PID adverse jamais vu jusque-là : début d'une rencontre.
    pub new: bool,
}

/// Ce qu'un tick a pu lire.
#[derive(Debug, Clone)]
pub struct LiveRead {
    pub party: Vec<Pokemon>,
    /// Carte actuelle (Gen 4-5, bloc de sauvegarde retrouvé en RAM).
    pub map: Option<u16>,
    pub badges: Option<u8>,
    pub battle: Option<Battle>,
}

#[derive(Debug, Clone)]
struct Copy {
    addr: u64,
    raw: Vec<u8>,
    changed_at: u64,
    /// Copie qui appartient au bloc de sauvegarde (dresseur à la bonne distance).
    save_block: bool,
}

/// Suivi d'une partie dans la RAM d'un émulateur, d'un tick à l'autre.
pub struct LiveReader {
    console: Console,
    hints: RamHints,
    copies: Vec<Copy>,
    tick: u64,
    /// Dernier tick où l'équipe a été recherchée.
    searched_at: Option<u64>,
    /// PID / constantes de chiffrement déjà vus chez l'adversaire.
    seen_enemies: HashSet<u32>,
    enemy_at: Option<u64>,
    /// Ticks consécutifs sans équipe valide.
    misses: u32,
    /// Recherche de l'équipe adverse activée (cartes mémoire : `battle`).
    battle: bool,
}

/// Re-recherche de l'équipe perdue : au plus tous les N ticks (la recherche 3DS lit la FCRAM).
const RESEARCH_DS: u64 = 5;
const RESEARCH_CTR: u64 = 150;
/// Recherche de l'équipe adverse (DS) : tous les N ticks.
const BATTLE_SCAN: u64 = 2;

impl LiveReader {
    pub fn new(console: Console, hints: RamHints) -> Self {
        LiveReader {
            console,
            hints,
            copies: Vec::new(),
            tick: 0,
            searched_at: None,
            seen_enemies: HashSet::new(),
            enemy_at: None,
            misses: 0,
            battle: true,
        }
    }

    /// Active ou coupe la recherche de l'équipe adverse.
    pub fn set_battle(&mut self, on: bool) {
        self.battle = on;
    }

    pub fn console(&self) -> &Console {
        &self.console
    }

    /// Nouveaux repères (après une sauvegarde en jeu) : nouvelle équipe à chercher si besoin.
    pub fn set_hints(&mut self, hints: RamHints) {
        self.hints = hints;
    }

    /// Adresses des copies de l'équipe trouvées (diagnostic).
    pub fn party_addresses(&self) -> Vec<u64> {
        self.copies.iter().map(|c| c.addr).collect()
    }

    /// Adresse de la dernière équipe adverse trouvée (diagnostic).
    pub fn enemy_address(&self) -> Option<u64> {
        self.enemy_at
    }

    fn search_regions(&self, src: &dyn MemorySource) -> Vec<Region> {
        match &self.console {
            Console::Ds(ram) => vec![Region { base: ram.base, size: ram.size, allocation: ram.base }],
            Console::Ctr => scan::ctr_candidate_regions(src),
        }
    }

    /// Le bloc de sauvegarde est-il autour de l'équipe à `slot0`, comme dans le fichier ?
    fn is_save_block(&self, src: &dyn MemorySource, slot0: u64) -> bool {
        let h = &self.hints;
        if h.trainer_name_bytes.is_empty() {
            return false;
        }
        let name_at = beside(slot0, h.party, h.trainer_name);
        let tid_at = beside(slot0, h.party, h.tid_offset);
        let name = src.read_vec(name_at, h.trainer_name_bytes.len());
        let ids = src.read_vec(tid_at, 4);
        let want: Vec<u8> = h.tid.to_le_bytes().into_iter().chain(h.sid.to_le_bytes()).collect();
        matches!((name, ids), (Ok(n), Ok(i)) if n == h.trainer_name_bytes && i == want)
    }

    /// Cherche toutes les copies de l'équipe en RAM.
    fn search_party(&mut self, src: &dyn MemorySource) {
        self.searched_at = Some(self.tick);
        let format = self.hints.format;
        let size = format.party_size() as u64;
        let regions = self.search_regions(src);
        let mut found: Vec<u64> = Vec::new();
        let keys = self.hints.party_keys.clone();
        for (hit, key) in scan::find_u32s(src, &regions, &keys, 256) {
            let j = keys.iter().position(|&k| k == key).unwrap_or(0);
            // L'équipe a pu être réordonnée depuis la sauvegarde : on essaie chaque position.
            let order = std::iter::once(j).chain((0..PARTY_SLOTS).filter(|&k| k != j));
            for k in order {
                let Some(slot0) = hit.checked_sub(k as u64 * size) else { continue };
                if found.contains(&slot0) {
                    break;
                }
                let Ok(Some(p)) = read_party_at(src, format, slot0) else { continue };
                if p.mons.get(k).is_some_and(|m| m.data()[..4] == key.to_le_bytes()) {
                    found.push(slot0);
                    break;
                }
            }
        }
        // Équipe vide dans la sauvegarde (nouvelle partie, starter pas encore reçu) ou équipe
        // introuvable par ses Pokémon : on retrouve le bloc de sauvegarde par le numéro du
        // dresseur, l'équipe est alors à la même distance que dans le fichier.
        if found.is_empty() && !self.hints.trainer_name_bytes.is_empty() {
            let h = &self.hints;
            let ids = u32::from(h.tid) | u32::from(h.sid) << 16;
            for (hit, _) in scan::find_u32s(src, &regions, &[ids], 4096) {
                let slot0 = beside(hit, h.tid_offset, h.party);
                if !found.contains(&slot0) && self.is_save_block(src, slot0) {
                    found.push(slot0);
                }
            }
        }
        let mut copies: Vec<Copy> = Vec::new();
        for addr in found {
            let save_block = self.is_save_block(src, addr);
            let raw = read_party_at(src, format, addr).ok().flatten().map(|p| p.raw).unwrap_or_default();
            copies.push(Copy { addr, raw, changed_at: 0, save_block });
        }
        // Le bloc de sauvegarde d'abord : c'est la copie choisie tant qu'aucune autre ne bouge.
        copies.sort_by_key(|c| !c.save_block);
        self.copies = copies;
    }

    /// Lecture d'un tick. `Ok(None)` : rien de valide ce tick-ci (équipe pas encore trouvée,
    /// ou lue pendant une écriture) ; l'appelant garde l'instantané précédent.
    pub fn tick(&mut self, src: &dyn MemorySource) -> Result<Option<LiveRead>, LiveError> {
        self.tick += 1;
        if !src.alive() {
            return Err(LiveError::Gone);
        }
        let research = match self.console {
            Console::Ds(_) => RESEARCH_DS,
            Console::Ctr => RESEARCH_CTR,
        };
        if self.copies.is_empty() && self.searched_at.is_none_or(|t| self.tick - t >= research) {
            self.search_party(src);
        }
        let format = self.hints.format;
        let mut best: Option<(u64, bool, PartyAt)> = None;
        let mut torn = false;
        let tick = self.tick;
        for c in &mut self.copies {
            match read_party_at(src, format, c.addr)? {
                Some(p) => {
                    if p.raw != c.raw {
                        c.raw = p.raw.clone();
                        c.changed_at = tick;
                    }
                    if best.as_ref().is_none_or(|(at, _, _)| c.changed_at > *at) {
                        best = Some((c.changed_at, c.save_block, p));
                    }
                }
                // Bloc de sauvegarde sans aucun Pokémon : équipe vide, lue telle quelle.
                None if c.save_block && empty_party(src, &self.hints, c.addr) => {
                    if best.is_none() {
                        best = Some((c.changed_at, true, PartyAt { mons: Vec::new(), raw: Vec::new() }));
                    }
                }
                None => torn |= c.save_block,
            }
        }
        let Some((_, _, party)) = best else {
            self.misses += 1;
            if self.misses >= 5 {
                // Équipe disparue (partie rechargée, Pokémon échangés) : nouvelle recherche.
                self.copies.clear();
                self.misses = 0;
            }
            return Ok(None);
        };
        self.misses = 0;
        if torn {
            // Le bloc de sauvegarde est en cours d'écriture : on attend le tick suivant.
            return Ok(None);
        }
        let (map, badges) = self.save_block_fields(src);
        let battle =
            if self.battle && matches!(self.console, Console::Ds(_)) && tick % BATTLE_SCAN == 0 { self.scan_battle(src, &party.mons) } else { None };
        Ok(Some(LiveRead { party: party.mons, map, badges, battle }))
    }

    fn save_block_fields(&self, src: &dyn MemorySource) -> (Option<u16>, Option<u8>) {
        let Some(c) = self.copies.iter().find(|c| c.save_block) else { return (None, None) };
        let h = &self.hints;
        let at = |ofs: usize| Some(beside(c.addr, h.party, ofs));
        let map = at(h.map).and_then(|a| src.read_vec(a, 2).ok()).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let badges = h.badges.and_then(at).and_then(|a| src.read_vec(a, 1).ok()).map(|b| b[0]);
        (map, badges)
    }

    /// Équipe adverse (DS) : en-tête `[6][n]` suivi de Pokémon qui ne sont pas à nous.
    fn scan_battle(&mut self, src: &dyn MemorySource, ours: &[Pokemon]) -> Option<Battle> {
        let Console::Ds(ram) = &self.console else { return None };
        let format = self.hints.format;
        let mine: HashSet<[u8; 4]> = ours.iter().map(|p| key_of(p)).collect();
        let copies: HashSet<u64> = self.copies.iter().map(|c| c.addr).collect();
        let read = |slot0: u64| -> Option<Vec<Pokemon>> {
            let p = read_party_at(src, format, slot0).ok().flatten()?;
            p.mons.iter().all(|m| !mine.contains(&key_of(m))).then_some(p.mons)
        };
        // Adresse connue d'abord (pas besoin de relire 4 Mio si le combat continue).
        let mut enemies = self.enemy_at.and_then(read);
        if enemies.is_none() {
            self.enemy_at = None;
            let ram_bytes = src.read_vec(ram.base, ram.size as usize).ok()?;
            let words = ram_bytes.as_chunks::<4>().0;
            for i in 0..words.len().saturating_sub(2) {
                if u32::from_le_bytes(words[i]) != PARTY_SLOTS as u32 {
                    continue;
                }
                let n = u32::from_le_bytes(words[i + 1]);
                if !(1..=PARTY_SLOTS as u32).contains(&n) {
                    continue;
                }
                let slot0 = ram.base + (i as u64 + 2) * 4;
                if copies.contains(&slot0) {
                    continue;
                }
                if let Some(e) = read(slot0) {
                    // Plusieurs équipes adverses en RAM (ancien combat pas effacé) : on garde
                    // celle qui contient un Pokémon jamais vu, sinon la première trouvée.
                    let fresh = e.iter().any(|m| !self.seen_enemies.contains(&u32::from_le_bytes(key_of(m))));
                    if enemies.is_none() || fresh {
                        self.enemy_at = Some(slot0);
                        enemies = Some(e);
                        if fresh {
                            break;
                        }
                    }
                }
            }
        }
        let enemies = enemies?;
        let new = enemies.iter().fold(false, |acc, m| self.seen_enemies.insert(u32::from_le_bytes(key_of(m))) | acc);
        let (tid, sid) = (self.hints.tid, self.hints.sid);
        let wild = enemies.len() <= 2 && enemies.iter().all(|m| m.tid() == tid && m.sid() == sid);
        Some(Battle { enemies, wild, new })
    }

    /// Marque comme déjà vus les adversaires présents en RAM au moment de l'attache : un ancien
    /// combat resté en mémoire ne doit pas passer pour une nouvelle rencontre.
    pub fn prime_battle(&mut self, src: &dyn MemorySource) {
        let ours = self.copies.first().and_then(|c| read_party_at(src, self.hints.format, c.addr).ok().flatten()).map(|p| p.mons).unwrap_or_default();
        if let Some(b) = self.scan_battle(src, &ours) {
            for m in &b.enemies {
                self.seen_enemies.insert(u32::from_le_bytes(key_of(m)));
            }
        }
    }
}

/// Équipe vide : compteur à 0 et premier emplacement vide.
fn empty_party(src: &dyn MemorySource, h: &RamHints, slot0: u64) -> bool {
    let count = src.read_vec(beside(slot0, h.party, h.party_count), 1).ok().map(|b| b[0]);
    let first = src.read_vec(slot0, h.format.party_size()).ok();
    count == Some(0) && first.is_some_and(|b| empty_slot(h.format, &b))
}

fn key_of(p: &Pokemon) -> [u8; 4] {
    let d = p.data();
    [d[0], d[1], d[2], d[3]]
}

/// Adresse en RAM du champ à l'offset `ofs` du fichier, sachant que l'équipe (offset `party`
/// dans le fichier) est à `slot0` : même distance qu'entre les deux dans la sauvegarde.
fn beside(slot0: u64, party: usize, ofs: usize) -> u64 {
    slot0.wrapping_add((ofs as i64 - party as i64) as u64)
}
