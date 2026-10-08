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

use super::ctr::{self, Fighter, ParamLayout};
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
    /// Notre équipe telle que le combat la voit (PV à jour).
    pub ours: Vec<Pokemon>,
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
    /// Combat en cours d'après le jeu (overlay ou module de combat chargé), quand c'est connu.
    /// Sur 3DS, seul ce signal existe : l'équipe adverse n'est pas encore lue.
    pub in_battle: Option<bool>,
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
    pub(crate) tick: u64,
    /// Dernier tick où l'équipe a été recherchée.
    searched_at: Option<u64>,
    /// PID / constantes de chiffrement déjà vus chez l'adversaire.
    seen_enemies: HashSet<u32>,
    enemy_at: Option<u64>,
    /// Copie de combat de notre équipe.
    ours_at: Option<u64>,
    fight: Option<Fight>,
    /// Indicateur « en combat » du jeu (adresse hôte, valeur en combat), quand il est connu.
    flag: Option<(u64, u32)>,
    /// Début du code de l'overlay de combat (adresse hôte, octets attendus).
    code: Option<(u64, Vec<u8>)>,
    /// 3DS : modules de combat et de carte, et l'emplacement mémoire qu'ils se partagent.
    module: Option<CtrModules>,
    /// Ticks consécutifs sans équipe valide.
    misses: u32,
    /// Recherche de l'équipe adverse activée (cartes mémoire : `battle`).
    battle: bool,
    /// 3DS : Pokémon en combat suivis, et où les chercher.
    ctr: CtrBattle,
    /// 3DS : équipe vivante (la copie au format équipe n'est que l'image de la sauvegarde).
    live_party: Option<ctr::LiveParty>,
    /// Lectures ratées de suite de l'équipe vivante.
    live_misses: u32,
    live_searched: Option<u64>,
    /// Autres tableaux possibles (sauvegarde sans Pokémon).
    live_alts: Vec<ctr::LiveParty>,
}

/// 3DS : Pokémon en combat (blocs de combat reliés à leurs données PK6), zone où ils ont été vus
/// la dernière fois (le jeu réutilise les mêmes adresses d'un combat à l'autre), prochaine recherche.
#[derive(Debug, Clone, Default)]
struct CtrBattle {
    fighters: Vec<Fighter>,
    window: Option<Region>,
    next_scan: u64,
    attempts: u32,
    /// En combat au tick précédent (module de combat chargé).
    active: bool,
}

/// 3DS : recherche des Pokémon en combat toutes les secondes au début du combat, puis toutes les 5 s.
const CTR_FIGHT_SCAN: u64 = 5;
const CTR_FIGHT_RESCAN: u64 = 25;
/// Marge autour des Pokémon en combat vus la dernière fois : la recherche suivante commence là.
const CTR_WINDOW: u64 = 4 << 20;

/// Re-recherche de l'équipe perdue : au plus tous les N ticks (la recherche 3DS lit la FCRAM).
const RESEARCH_DS: u64 = 5;
const RESEARCH_CTR: u64 = 150;
/// Recherche de l'équipe adverse (DS) : tous les N ticks.
const BATTLE_SCAN: u64 = 2;
/// 3DS : recherche de l'emplacement des modules tant qu'il est inconnu, tous les N ticks (5 s),
/// puis seulement si son contenu devient inattendu (partie relancée), toutes les 60 s.
const CTR_MODULE_SEARCH: u64 = 25;
const CTR_MODULE_RESEARCH: u64 = 300;

/// 3DS : en-têtes des modules de combat et de carte, emplacement partagé, dernière recherche.
#[derive(Debug, Clone)]
struct CtrModules {
    battle: Vec<u8>,
    field: Option<Vec<u8>>,
    slot: Option<u64>,
    searched: Option<u64>,
}

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
            ours_at: None,
            fight: None,
            flag: None,
            code: None,
            module: None,
            misses: 0,
            battle: true,
            ctr: CtrBattle::default(),
            live_party: None,
            live_misses: 0,
            live_searched: None,
            live_alts: Vec::new(),
        }
    }

    /// Indicateur « en combat » (adresse DS 0x02xxxxxx, valeur en combat), d'après les cartes mémoire.
    /// Sans lui, la fin d'un combat n'est vue que si le jeu réécrit l'équipe ou change de carte.
    pub fn set_battle_flag(&mut self, ds_address: u32, value: u32) {
        if let Console::Ds(ram) = &self.console {
            self.flag = Some((ram.host(ds_address), value));
        }
    }

    /// Code de l'overlay de combat (adresse DS, premiers octets lus dans la ROM) : prioritaire sur
    /// l'indicateur, car il ne dépend que de la ROM.
    pub fn set_battle_code(&mut self, ds_address: u32, bytes: Vec<u8>) {
        if let Console::Ds(ram) = &self.console {
            self.code = Some((ram.host(ds_address), bytes));
        }
    }

    /// 3DS : en-tête du module CRO de combat (lu dans la ROM), cherché dans la FCRAM.
    pub fn set_battle_module(&mut self, head: Vec<u8>) {
        self.module = Some(CtrModules { battle: head, field: None, slot: None, searched: None });
    }

    /// 3DS : en-tête du module de la carte, chargé au même emplacement que celui du combat
    /// (observé sur Rubis Oméga) : il permet de trouver cet emplacement avant le premier combat.
    pub fn set_field_module(&mut self, head: Vec<u8>) {
        if let Some(m) = &mut self.module {
            m.field = Some(head);
        }
    }

    /// 3DS : combat en cours d'après le module de combat chargé. L'adresse trouvée est relue à
    /// chaque passage ; sans elle, nouvelle recherche toutes les 5 s (la FCRAM fait plusieurs centaines de Mo).
    pub(crate) fn ctr_in_battle(&mut self, src: &dyn MemorySource) -> Option<bool> {
        let tick = self.tick;
        let m = self.module.as_ref()?;
        let n = m.battle.len();
        // Emplacement connu : une seule lecture de 32 octets par passage.
        if let Some(a) = m.slot {
            match src.read_vec(a, n) {
                Ok(b) if b == m.battle => return Some(true),
                Ok(b) if m.field.as_ref() == Some(&b) => return Some(false),
                _ => {}
            }
        }
        // Emplacement inconnu ou contenu inattendu : nouvelle recherche dans la FCRAM, espacée.
        let wait = if m.slot.is_some() { CTR_MODULE_RESEARCH } else { CTR_MODULE_SEARCH };
        if m.searched.is_some_and(|t| tick - t < wait) {
            return Some(false);
        }
        // Liste des zones mémoire lue seulement pour une recherche (elle parcourt tout l'espace du processus).
        let regions = self.fcram_regions(src);
        let m = self.module.as_mut()?;
        m.searched = Some(tick);
        let first = |h: &[u8]| u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
        let mut keys = vec![first(&m.battle)];
        if let Some(f) = &m.field {
            keys.push(first(f));
        }
        for (hit, _) in scan::find_u32s(src, &regions, &keys, 64) {
            let Ok(b) = src.read_vec(hit, n) else { continue };
            if b == m.battle || m.field.as_ref() == Some(&b) {
                m.slot = Some(hit);
                return Some(b == m.battle);
            }
        }
        Some(false)
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
        // Autre équipe dans la sauvegarde (premier Pokémon, capture…) : on relance la recherche,
        // la copie suivie a pu être trouvée par le seul numéro du dresseur.
        if hints.party_keys != self.hints.party_keys {
            self.copies.clear();
            self.searched_at = None;
        }
        self.hints = hints;
    }

    /// Adresses des copies de l'équipe trouvées (diagnostic).
    pub fn party_addresses(&self) -> Vec<u64> {
        self.copies.iter().map(|c| c.addr).collect()
    }

    /// 3DS : adresse du tableau de l'équipe vivante, s'il est trouvé (diagnostic).
    pub fn live_party_address(&self) -> Option<u64> {
        self.live_party.as_ref().map(|l| l.at)
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
        let all = self.search_regions(src);
        let mut found: Vec<u64> = Vec::new();
        let keys = self.hints.party_keys.clone();
        // 3DS : zone par zone, la plus grande (FCRAM) d'abord, jusqu'à trouver l'équipe.
        let sets: Vec<Vec<Region>> = match self.console {
            Console::Ctr => all.iter().map(|r| vec![*r]).collect(),
            Console::Ds(_) => vec![all.clone()],
        };
        let mut regions = all.clone();
        for set in sets {
            let hits = scan::find_u32s(src, &set, &keys, 256);
            if hits.is_empty() {
                continue;
            }
            regions = set;
            for (hit, key) in hits {
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
            if !found.is_empty() {
                break;
            }
        }
        if found.is_empty() {
            regions = all;
        }
        // Équipe vide dans la sauvegarde (nouvelle partie, starter pas encore reçu) ou équipe
        // introuvable par ses Pokémon : on retrouve le bloc de sauvegarde par le numéro du
        // dresseur, l'équipe est alors à la même distance que dans le fichier.
        if found.is_empty() && !self.hints.trainer_name_bytes.is_empty() {
            let h = &self.hints;
            let ids = u32::from(h.tid) | u32::from(h.sid) << 16;
            for (hit, _) in scan::find_u32s(src, &regions, &[ids], 4096) {
                let slot0 = beside(hit, h.tid_offset, h.party);
                // Le dresseur apparaît dans plusieurs blocs (SoulSilver : trois) : on garde le
                // premier dont l'équipe se lit (Pokémon valides ou équipe vide).
                let usable = read_party_at(src, h.format, slot0).ok().flatten().is_some() || empty_party(src, h, slot0);
                if usable && self.is_save_block(src, slot0) {
                    found.push(slot0);
                    break;
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
        // 3DS : l'équipe vivante d'abord ; la copie au format équipe (image de la sauvegarde) ne
        // sert que si elle reste introuvable. Nouvel essai de temps en temps.
        let ctr = matches!(self.console, Console::Ctr);
        if ctr && self.live_party.is_none() && self.live_searched.is_none_or(|t| self.tick - t >= RESEARCH_CTR) {
            self.search_live_party(src);
        }
        if (!ctr || self.live_party.is_none()) && self.copies.is_empty() && self.searched_at.is_none_or(|t| self.tick - t >= research) {
            self.search_party(src);
        }
        let tick = self.tick;
        let party = if self.live_party.is_some() {
            match self.read_live_party(src) {
                Some(mons) => PartyAt { mons, raw: Vec::new() },
                // Lu pendant une écriture : on garde l'instantané précédent.
                None => return Ok(None),
            }
        } else {
            match self.read_copies(src)? {
                Some(p) => p,
                None => return Ok(None),
            }
        };
        let (map, badges) = self.save_block_fields(src);
        let in_battle = match (&self.console, &self.code) {
            (Console::Ctr, _) => self.ctr_in_battle(src),
            (_, Some((at, bytes))) => Some(src.read_vec(*at, bytes.len()).is_ok_and(|b| b == *bytes)),
            (_, None) => self.flag.map(|(at, v)| src.read_u32(at).is_ok_and(|x| x == v)),
        };
        if matches!(self.console, Console::Ctr) {
            let now = in_battle == Some(true);
            // Fin de combat : le tableau trouvé a pu être celui du combat (libéré, nombre remis à
            // zéro) ; on le cherche de nouveau, la copie du combat n'a plus de Pokémon.
            if self.ctr.active && !now {
                self.live_party = None;
                self.live_searched = None;
            }
            self.ctr.active = now;
        }
        let battle = if !self.battle {
            None
        } else if matches!(self.console, Console::Ctr) {
            if in_battle == Some(true) {
                self.ctr_battle(src, &party.mons, map)
            } else {
                self.ctr_battle_over();
                None
            }
        } else if in_battle == Some(false) {
            // Le jeu dit « hors combat » : les équipes restées en RAM ne comptent plus.
            if let Some(f) = &mut self.fight {
                f.over = true;
            }
            None
        } else {
            self.scan_battle(src, &party.mons, map, tick.is_multiple_of(BATTLE_SCAN) || in_battle == Some(true))
        };
        // En combat, la copie de combat porte les PV à jour (le bloc de sauvegarde attend la fin).
        let party = battle.as_ref().map_or(party.mons, |b| b.ours.clone());
        Ok(Some(LiveRead { party, map, badges, battle, in_battle }))
    }

    /// 3DS : équipe vivante, si son tableau est connu. Après 10 lectures ratées de suite (partie
    /// relancée), le tableau est oublié et recherché de nouveau avec l'équipe.
    fn read_live_party(&mut self, src: &dyn MemorySource) -> Option<Vec<Pokemon>> {
        let lp = self.live_party.as_ref()?;
        // Équipe vide alors que la sauvegarde a des Pokémon : tableau abandonné par le jeu.
        let mut read = lp.read(src, self.hints.format).filter(|m| !m.is_empty() || self.hints.party_keys.is_empty());
        // Sauvegarde sans Pokémon : plusieurs tableaux vides possibles, le premier qui se remplit gagne.
        if read.as_ref().is_some_and(|m| m.is_empty()) {
            let format = self.hints.format;
            if let Some(i) = self.live_alts.iter().position(|a| a.read(src, format).is_some_and(|m| !m.is_empty())) {
                let alt = self.live_alts.remove(i);
                read = alt.read(src, format);
                if let Some(old) = self.live_party.replace(alt) {
                    self.live_alts.push(old);
                }
            }
        }
        match read {
            Some(mons) => {
                self.live_misses = 0;
                Some(mons)
            }
            None => {
                self.live_misses += 1;
                if self.live_misses >= 10 {
                    self.live_party = None;
                    self.live_misses = 0;
                    self.copies.clear();
                    self.searched_at = None;
                }
                None
            }
        }
    }

    /// 3DS : tableau de l'équipe vivante, retrouvé depuis un Pokémon de la sauvegarde (données
    /// PK6 au format boîte tenues par un objet dont la section équipe se déchiffre).
    /// Sans Pokémon dans la sauvegarde (début de partie), les tableaux sont reconnus à leur forme :
    /// le premier qui se remplit devient l'équipe.
    fn search_live_party(&mut self, src: &dyn MemorySource) {
        self.live_searched = Some(self.tick);
        self.live_alts.clear();
        let regions = self.fcram_regions(src);
        for (hit, key) in scan::find_u32s(src, &regions, &self.hints.party_keys, 256) {
            let Ok(data) = src.read_vec(hit, 232) else { continue };
            if !ctr::encrypted_box_mon(&data) {
                continue;
            }
            let Some((obj, vaddr, off)) = ctr::owner_of(src, hit, key) else { continue };
            if let Some(lp) = ctr::find_live_party(src, &regions, vaddr, off, obj.vtable) {
                self.live_party = Some(lp);
                self.live_misses = 0;
                return;
            }
        }
        if self.hints.party_keys.is_empty() {
            let mut found = ctr::find_party_arrays(src, &regions).into_iter();
            self.live_party = found.next();
            self.live_alts = found.collect();
            self.live_misses = 0;
        }
    }

    /// Copies de l'équipe au format équipe (DS ; 3DS tant que l'équipe vivante n'est pas trouvée).
    fn read_copies(&mut self, src: &dyn MemorySource) -> Result<Option<PartyAt>, LiveError> {
        let format = self.hints.format;
        let mut best: Option<(u64, bool, PartyAt)> = None;
        let mut torn = false;
        let tick = self.tick;
        // Nos Pokémon : ceux de la sauvegarde et du bloc de sauvegarde en RAM. Une autre copie
        // n'est crue que si elle ne contient qu'eux : SoulSilver réutilise l'emplacement de sa
        // seconde copie pour l'équipe adverse pendant un combat.
        let mut trusted: HashSet<u32> = self.hints.party_keys.iter().copied().collect();
        if let Some(c) = self.copies.iter().find(|c| c.save_block) {
            if let Ok(Some(p)) = read_party_at(src, format, c.addr) {
                trusted.extend(p.mons.iter().map(|m| u32::from_le_bytes(key_of(m))));
            }
        }
        for c in &mut self.copies {
            match read_party_at(src, format, c.addr)? {
                Some(p) if !c.save_block && !p.mons.iter().all(|m| trusted.contains(&u32::from_le_bytes(key_of(m)))) => {}
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
        Ok(Some(party))
    }

    fn save_block_fields(&self, src: &dyn MemorySource) -> (Option<u16>, Option<u8>) {
        let Some(c) = self.copies.iter().find(|c| c.save_block) else { return (None, None) };
        let h = &self.hints;
        let at = |ofs: usize| Some(beside(c.addr, h.party, ofs));
        let map = at(h.map).and_then(|a| src.read_vec(a, 2).ok()).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let badges = h.badges.and_then(at).and_then(|a| src.read_vec(a, 1).ok()).map(|b| b[0]);
        (map, badges)
    }

    /// Combat (DS) : une copie de combat de notre équipe (en-tête `[6][n]`, hors bloc de
    /// sauvegarde) suivie, dans les 4 Kio, de l'équipe adverse. Les équipes d'anciens combats
    /// restent en RAM : seule celle qui suit notre copie de combat compte. Le combat est fini dès
    /// que le jeu réécrit notre équipe dans le bloc de sauvegarde (expérience, PV) ou que la
    /// carte change. Observé dans melonDS sur Blanche : copie à +0x8, adversaire à +0x560.
    fn scan_battle(&mut self, src: &dyn MemorySource, ours: &[Pokemon], map: Option<u16>, full: bool) -> Option<Battle> {
        let Console::Ds(_) = &self.console else { return None };
        let format = self.hints.format;
        let mine: HashSet<u32> = ours.iter().map(|p| u32::from_le_bytes(key_of(p))).collect();
        let is_ours = |p: &PartyAt| p.mons.iter().all(|m| mine.contains(&u32::from_le_bytes(key_of(m))));
        let is_foe = |p: &PartyAt| p.mons.iter().all(|m| !mine.contains(&u32::from_le_bytes(key_of(m))));
        let save_raw = self.copies.iter().find(|c| c.save_block).map(|c| c.raw.clone()).unwrap_or_default();

        // Combat suivi : on relit les deux adresses connues.
        let mut found: Option<(u64, Vec<Pokemon>, u64, Vec<Pokemon>)> = None;
        if let (Some(o), Some(e)) = (self.ours_at, self.enemy_at) {
            let po = read_party_at(src, format, o).ok().flatten().filter(|p| is_ours(p));
            let pe = read_party_at(src, format, e).ok().flatten().filter(|p| is_foe(p));
            if let (Some(po), Some(pe)) = (po, pe) {
                found = Some((o, po.mons, e, pe.mons));
            }
        }
        if found.is_none() {
            if !full {
                return None;
            }
            // Plusieurs combats peuvent rester en RAM : un adversaire jamais vu d'abord.
            let pairs = self.battle_pairs(src, &mine);
            let fresh = |e: &Vec<Pokemon>| e.iter().any(|m| !self.seen_enemies.contains(&u32::from_le_bytes(key_of(m))));
            let pick = pairs.iter().position(|p| fresh(&p.3)).or(if pairs.is_empty() { None } else { Some(0) });
            found = pick.map(|i| pairs[i].clone());
        }
        let Some((o, ours_now, e, enemies)) = found else {
            self.ours_at = None;
            self.enemy_at = None;
            self.fight = None;
            return None;
        };
        self.ours_at = Some(o);
        self.enemy_at = Some(e);
        let mut keys: Vec<u32> = enemies.iter().map(|m| u32::from_le_bytes(key_of(m))).collect();
        keys.sort_unstable();
        let new = match &mut self.fight {
            Some(f) if f.keys == keys => {
                // Sans indicateur du jeu : fin devinée (équipe réécrite, autre carte).
                if !f.over && self.flag.is_none() && self.code.is_none() && (f.map != map || f.save_raw != save_raw) {
                    f.over = true;
                }
                if f.over {
                    return None;
                }
                false
            }
            _ => {
                // Équipe déjà vue à l'attache : ancien combat resté en mémoire.
                let stale = keys.iter().all(|k| self.seen_enemies.contains(k));
                self.fight = Some(Fight { keys: keys.clone(), map, save_raw, over: stale });
                if stale {
                    return None;
                }
                true
            }
        };
        self.seen_enemies.extend(keys);
        let (tid, sid) = (self.hints.tid, self.hints.sid);
        let wild = enemies.len() <= 2 && enemies.iter().all(|m| m.tid() == tid && m.sid() == sid);
        Some(Battle { enemies, ours: ours_now, wild, new })
    }

    /// 3DS : fin du combat (module de combat déchargé).
    fn ctr_battle_over(&mut self) {
        self.ctr.fighters.clear();
        self.ctr.next_scan = 0;
        self.ctr.attempts = 0;
        if let Some(f) = &mut self.fight {
            f.over = true;
        }
    }

    /// Disposition des blocs de combat selon la génération.
    fn ctr_layout(&self) -> ParamLayout {
        if self.hints.format == PkmFormat::Gen7 { ctr::GEN7_PARAM } else { ctr::GEN6_PARAM }
    }

    /// 3DS, en combat : Pokémon en combat relus à chaque passage ; recherchés dans la FCRAM au début
    /// du combat (d'abord autour de ceux du combat précédent, puis dans toute la FCRAM).
    fn ctr_battle(&mut self, src: &dyn MemorySource, ours: &[Pokemon], map: Option<u16>) -> Option<Battle> {
        let layout = self.ctr_layout();
        if !self.ctr.fighters.is_empty() {
            let again: Option<Vec<Fighter>> = self.ctr.fighters.iter().map(|f| ctr::reread(src, &layout, f)).collect();
            self.ctr.fighters = again.unwrap_or_default();
        }
        let mine: HashSet<u32> = ours.iter().map(|p| u32::from_le_bytes(key_of(p))).collect();
        let has_foe = |fs: &[Fighter]| fs.iter().any(|f| !mine.contains(&f.key()));
        if !has_foe(&self.ctr.fighters) {
            if self.tick < self.ctr.next_scan {
                return None;
            }
            self.ctr.attempts += 1;
            self.ctr.next_scan = self.tick + if self.ctr.attempts < 5 { CTR_FIGHT_SCAN } else { CTR_FIGHT_RESCAN };
            let format = self.hints.format;
            let mut fs = Vec::new();
            if let Some(w) = self.ctr.window {
                fs = ctr::fighters(src, &ctr::scan(src, &[w], format, &layout));
            }
            if !has_foe(&fs) {
                let regions = self.fcram_regions(src);
                fs = ctr::fighters(src, &ctr::scan(src, &regions, format, &layout));
            }
            if !has_foe(&fs) {
                return None;
            }
            let lo = fs.iter().map(|f| f.param_at).min().unwrap_or(0);
            let hi = fs.iter().map(|f| f.param_at).max().unwrap_or(0);
            let base = lo.saturating_sub(CTR_WINDOW);
            self.ctr.window = Some(Region { base, size: hi + CTR_WINDOW - base, allocation: base });
            self.ctr.fighters = fs;
        }
        let fighters = &self.ctr.fighters;
        let enemies: Vec<Pokemon> = fighters.iter().filter(|f| !mine.contains(&f.key())).map(Fighter::pokemon).collect();
        // Notre équipe avec les PV du combat.
        let ours_now: Vec<Pokemon> = ours
            .iter()
            .map(|p| {
                let key = u32::from_le_bytes(key_of(p));
                match fighters.iter().find(|f| f.key() == key) {
                    Some(f) => {
                        let mut q = p.clone();
                        q.set_current_hp(f.param.hp.min(p.party_stats().map_or(u16::MAX, |s| s[0])));
                        q
                    }
                    None => p.clone(),
                }
            })
            .collect();
        let mut keys: Vec<u32> = enemies.iter().map(|m| u32::from_le_bytes(key_of(m))).collect();
        keys.sort_unstable();
        let new = match &self.fight {
            Some(f) if f.keys == keys && !f.over => false,
            _ => {
                let new = keys.iter().any(|k| !self.seen_enemies.contains(k));
                self.fight = Some(Fight { keys: keys.clone(), map, save_raw: Vec::new(), over: false });
                new
            }
        };
        self.seen_enemies.extend(keys);
        let (tid, sid) = (self.hints.tid, self.hints.sid);
        // Un Pokémon sauvage porte nos identifiants (calcul du chromatique) ; ceux d'un dresseur, jamais.
        let wild = enemies.iter().all(|m| m.tid() == tid && m.sid() == sid);
        Some(Battle { enemies, ours: ours_now, wild, new })
    }

    /// 3DS : la zone de la FCRAM qui contient notre équipe (les autres grandes zones de l'émulateur
    /// sont son propre code), ou toutes les zones candidates.
    fn fcram_regions(&self, src: &dyn MemorySource) -> Vec<Region> {
        let all = scan::ctr_candidate_regions(src);
        let at = self.copies.first().map(|c| c.addr).or(self.live_party.as_ref().map(|l| l.at));
        match at.and_then(|a| all.iter().find(|r| r.contains(a, 1))) {
            Some(r) => vec![*r],
            None => all,
        }
    }

    /// Toutes les paires (copie de combat de notre équipe, équipe adverse qui la suit) en RAM.
    fn battle_pairs(&self, src: &dyn MemorySource, mine: &HashSet<u32>) -> Vec<(u64, Vec<Pokemon>, u64, Vec<Pokemon>)> {
        let Console::Ds(ram) = &self.console else { return Vec::new() };
        let format = self.hints.format;
        let psize = format.party_size();
        let key = |m: &Pokemon| u32::from_le_bytes(key_of(m));
        let save_at = self.copies.iter().find(|c| c.save_block).map(|c| c.addr);
        let Ok(bytes) = src.read_vec(ram.base, ram.size as usize) else { return Vec::new() };
        let words = bytes.as_chunks::<4>().0;
        let header =
            |i: usize| u32::from_le_bytes(words[i]) == PARTY_SLOTS as u32 && (1..=PARTY_SLOTS as u32).contains(&u32::from_le_bytes(words[i + 1]));
        let mut out = Vec::new();
        for i in 0..words.len().saturating_sub(3) {
            if !header(i) || !mine.contains(&u32::from_le_bytes(words[i + 2])) {
                continue;
            }
            let o = ram.base + (i as u64 + 2) * 4;
            if Some(o) == save_at {
                continue;
            }
            let Some(po) = read_party_at(src, format, o).ok().flatten().filter(|p| p.mons.iter().all(|m| mine.contains(&key(m)))) else { continue };
            // Après nos Pokémon (l'en-tête dit combien) : la place réservée aux 6 n'est pas garantie.
            let start = i + 2 + po.mons.len() * psize / 4;
            let end = (i + 0x1000 / 4).min(words.len().saturating_sub(2));
            for j in start..end {
                if !header(j) {
                    continue;
                }
                let e = ram.base + (j as u64 + 2) * 4;
                if let Some(pe) = read_party_at(src, format, e).ok().flatten().filter(|p| p.mons.iter().all(|m| !mine.contains(&key(m)))) {
                    out.push((o, po.mons.clone(), e, pe.mons));
                    break;
                }
            }
        }
        out
    }

    /// Marque comme terminés tous les combats présents en RAM au moment de l'attache : d'anciens
    /// combats restés en mémoire ne doivent pas passer pour une nouvelle rencontre.
    pub fn prime_battle(&mut self, src: &dyn MemorySource) {
        let ours = self.copies.first().and_then(|c| read_party_at(src, self.hints.format, c.addr).ok().flatten()).map(|p| p.mons).unwrap_or_default();
        let mine: HashSet<u32> = ours.iter().map(|p| u32::from_le_bytes(key_of(p))).collect();
        for (_, _, _, enemies) in self.battle_pairs(src, &mine) {
            self.seen_enemies.extend(enemies.iter().map(|m| u32::from_le_bytes(key_of(m))));
        }
        let (map, _) = self.save_block_fields(src);
        let _ = self.scan_battle(src, &ours, map, true);
        if let Some(f) = &mut self.fight {
            f.over = true;
        }
    }
}

/// Combat suivi d'un tick à l'autre.
#[derive(Debug, Clone)]
struct Fight {
    /// Premiers mots des Pokémon adverses, triés.
    keys: Vec<u32>,
    /// Carte et équipe du bloc de sauvegarde au début du combat.
    map: Option<u16>,
    save_raw: Vec<u8>,
    over: bool,
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
