//! Corrélations PID / IV des Gen 3 à 5 (d'après `MethodFinder.cs`, `LCRNG.cs`,
//! `PokewalkerRNG.cs`, `CuteCharm4.cs` et `MonochromeRNG.cs` de PKHeX).
//!
//! **Méthode 1** (Gen 3/4) : quatre appels successifs au LCRNG `0x41C64E6D / 0x6073`
//! donnent la moitié basse puis haute du PID, puis les deux mots d'IV (15 bits chacun).
//! Les Méthodes 2 et 4 (Gen 3) sautent un appel avant ou entre les IV.

use serde::Serialize;

const MUL: u32 = 0x41C6_4E6D;
const ADD: u32 = 0x6073;
const MUL_INV: u32 = 0xEEB9_EB65;
const ADD_INV: u32 = 0x0A3561A1;

pub fn next(s: u32) -> u32 {
    s.wrapping_mul(MUL).wrapping_add(ADD)
}

pub fn prev(s: u32) -> u32 {
    s.wrapping_mul(MUL_INV).wrapping_add(ADD_INV)
}

/// Type de corrélation trouvé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PidType {
    Method1,
    Method2,
    Method4,
    /// PID chromatique en chaîne (Poké Radar).
    ChainShiny,
    /// Talent Joli Sourire du meneur.
    CuteCharm,
    Pokewalker,
    None,
}

impl PidType {
    pub fn label(self) -> &'static str {
        match self {
            PidType::Method1 => "Méthode 1",
            PidType::Method2 => "Méthode 2",
            PidType::Method4 => "Méthode 4",
            PidType::ChainShiny => "Poké Radar (chaîne chromatique)",
            PidType::CuteCharm => "Joli Sourire",
            PidType::Pokewalker => "Pokéwalker",
            PidType::None => "aucune",
        }
    }
}

/// IV (ordre du jeu : PV, Att, Déf, Vit, Atq Spé, Déf Spé) en deux mots de 15 bits.
fn iv_words(iv32: u32) -> (u32, u32) {
    (iv32 & 0x7FFF, (iv32 >> 15) & 0x7FFF)
}

/// Graines dont les deux premiers appels donnent `first` puis `second` (16 bits hauts).
fn seeds_for(first: u32, second: u32) -> impl Iterator<Item = u32> {
    (0..=0xFFFFu32).filter_map(move |low| {
        let s1 = (first << 16) | low;
        (next(s1) >> 16 == second).then_some(prev(s1))
    })
}

/// Méthode 1, 2 ou 4 : PID (bas puis haut), IV ensuite.
pub fn method_1_2_4(pid: u32, iv32: u32) -> Option<(PidType, u32)> {
    let (iv1, iv2) = iv_words(iv32);
    for seed in seeds_for(pid & 0xFFFF, pid >> 16) {
        let s = next(next(seed));
        let a = next(s);
        let b = next(a);
        let c = next(b);
        if (a >> 16) & 0x7FFF == iv1 {
            if (b >> 16) & 0x7FFF == iv2 {
                return Some((PidType::Method1, seed));
            }
            if (c >> 16) & 0x7FFF == iv2 {
                return Some((PidType::Method4, seed));
            }
        } else if (b >> 16) & 0x7FFF == iv1 && (c >> 16) & 0x7FFF == iv2 {
            return Some((PidType::Method2, seed));
        }
    }
    None
}

/// Poké Radar (DPPt) : PID chromatique bâti bit à bit à partir des IV (`IsChainShinyValid`).
pub fn chain_shiny(pid: u32, iv32: u32, tid: u16, sid: u16) -> bool {
    let (iv1, iv2) = iv_words(iv32);
    // Graines dont les deux appels donnent les IV (bit 15 libre).
    for hi in 0..2u32 {
        for low in 0..=0xFFFFu32 {
            let s1 = ((iv1 | (hi << 15)) << 16) | low;
            let s2 = next(s1);
            if (s2 >> 16) & 0x7FFF != iv2 {
                continue;
            }
            // `seed` = état qui a donné les IV ; on remonte les bits du PID.
            let mut s = prev(s1);
            let mut ok = true;
            for i in (3..=15).rev() {
                if (s >> 16) & 1 != (pid >> i) & 1 {
                    ok = false;
                    break;
                }
                s = prev(s);
            }
            if !ok {
                continue;
            }
            let upper = s;
            if (upper >> 16) & 7 != (pid >> 16) & 7 {
                continue;
            }
            let lower = prev(upper);
            if (lower >> 16) & 7 != pid & 7 {
                continue;
            }
            let upid = (((pid & 0xFFFF) ^ tid as u32 ^ sid as u32) & 0xFFF8) | ((upper >> 16) & 7);
            if upid == pid >> 16 {
                return true;
            }
        }
    }
    false
}

/// Joli Sourire (Gen 4) : PID ≤ 0xFF choisi pour imposer le sexe opposé au meneur.
pub fn cute_charm(pid: u32) -> bool {
    pid <= 0xFF
}

/// PID du Pokéwalker (`PokewalkerRNG.GetPID`).
pub fn pokewalker_pid(tid: u16, sid: u16, nature: u32, gender: u8, ratio: u8) -> u32 {
    let nature = if nature >= 24 { 0 } else { nature };
    let mut pid: u32 = ((((tid ^ sid) as u32 >> 8) ^ 0xFF) << 24) & 0xFF00_0000;
    pid = pid.wrapping_add(nature).wrapping_sub(pid % 25);
    if ratio == 0 || ratio >= 0xFE {
        return pid;
    }
    let pid_gender = if (pid & 0xFF) < ratio as u32 { 1 } else { 0 };
    if gender == pid_gender {
        return pid;
    }
    if gender == 0 {
        pid = pid.wrapping_add((((ratio as u32).wrapping_sub(pid & 0xFF)) / 25 + 1) * 25);
        if (nature & 1) != (pid & 1) {
            pid = pid.wrapping_add(25);
        }
    } else {
        pid = pid.wrapping_sub((((pid & 0xFF).wrapping_sub(ratio as u32)) / 25 + 1) * 25);
        if (nature & 1) != (pid & 1) {
            pid = pid.wrapping_sub(25);
        }
    }
    pid
}

pub fn is_pokewalker(pid: u32, tid: u16, sid: u16, gender: u8, ratio: u8) -> bool {
    let mid = pid & 0x00FF_FF00;
    if mid != 0 && mid != 0x00FF_FF00 {
        return false;
    }
    let nature = pid % 25;
    // Azurill femelle devenue Marill mâle : PID calculé avec le taux d'Azurill (`IsAzurillEdgeCaseM`).
    nature != 24 && (pokewalker_pid(tid, sid, nature, gender, ratio) == pid || (gender == 0 && pokewalker_pid(tid, sid, nature, 1, 0xBF) == pid))
}

/// Analyse Gen 3/4 : première corrélation trouvée.
pub fn analyze_gen34(pid: u32, iv32: u32, tid: u16, sid: u16, shiny: bool, gender: u8, ratio: u8) -> PidType {
    if let Some((t, _)) = method_1_2_4(pid, iv32) {
        return t;
    }
    if shiny && chain_shiny(pid, iv32, tid, sid) {
        return PidType::ChainShiny;
    }
    if cute_charm(pid) {
        return PidType::CuteCharm;
    }
    if is_pokewalker(pid, tid, sid, gender, ratio) {
        return PidType::Pokewalker;
    }
    PidType::None
}

/// Gen 5, rencontres sauvages : le bit 31 du PID corrige le « XOR » des ID
/// (`MonochromeRNG.GetBitXor` doit donner 0).
pub fn gen5_xor_ok(pid: u32, tid: u16, sid: u16) -> bool {
    ((tid ^ sid) as u32 & 1) ^ (pid & 1) ^ (pid >> 31) == 0
}

/// Corrige le bit 31 d'un PID Gen 5 sauvage.
pub fn gen5_fix_xor(pid: u32, tid: u16, sid: u16) -> u32 {
    if gen5_xor_ok(pid, tid, sid) {
        pid
    } else {
        pid ^ 0x8000_0000
    }
}

/// PID attendu après un transfert vers la Gen 6 (`PK5.GetTransferPID`) : égal à la
/// constante de chiffrement, bit 31 inversé si le Pokémon devenait chromatique.
pub fn transfer_pid(ec: u32, tid: u16, sid: u16) -> u32 {
    let tmp = ec ^ (tid as u32 | (sid as u32) << 16);
    let xor = (tmp ^ (tmp >> 16)) & 0xFFFF;
    if xor & 0xFFF8 == 8 {
        ec ^ 0x8000_0000
    } else {
        ec
    }
}

/// Petit générateur pour la création (graine → suite pseudo-aléatoire).
pub struct Rand(u64);

impl Rand {
    pub fn new(seed: u64) -> Self {
        Rand(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn from_clock() -> Self {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Rand::new(t)
    }

    pub fn next_u32(&mut self) -> u32 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next_u32() % n
        }
    }
}

/// Contraintes pour générer un PID / des IV.
#[derive(Debug, Clone, Copy, Default)]
pub struct PidWish {
    pub nature: Option<u8>,
    pub shiny: Option<bool>,
    /// Bit du talent (0 = talent 1, 1 = talent 2).
    pub ability_bit: Option<u32>,
    /// Sexe voulu (0 mâle, 1 femelle) et taux de femelles de l'espèce.
    pub gender: Option<(u8, u8)>,
}

fn gender_of(pid: u32, ratio: u8) -> u8 {
    match ratio {
        255 => 2,
        254 => 1,
        0 => 0,
        r => ((pid & 0xFF) < r as u32) as u8,
    }
}

fn is_shiny(pid: u32, tid: u16, sid: u16, threshold: u32) -> bool {
    ((pid >> 16) ^ (pid & 0xFFFF) ^ tid as u32 ^ sid as u32) < threshold
}

/// PID + IV de Méthode 1 respectant les souhaits (essais aléatoires, chromatique compris
/// en cherchant un PID dont le XOR est < 8).
pub fn generate_method1(rand: &mut Rand, tid: u16, sid: u16, wish: PidWish) -> Option<(u32, u32)> {
    let want_shiny = wish.shiny == Some(true);
    let tries: u32 = if want_shiny { 40_000_000 } else { 2_000_000 };
    let mut seed = rand.next_u32();
    for _ in 0..tries {
        seed = next(seed);
        let a = next(seed);
        let b = next(a);
        let pid = (b >> 16) << 16 | (a >> 16);
        if let Some(n) = wish.nature {
            if pid % 25 != n as u32 {
                continue;
            }
        }
        if let Some(bit) = wish.ability_bit {
            if pid & 1 != bit {
                continue;
            }
        }
        if let Some((g, ratio)) = wish.gender {
            if g < 2 && gender_of(pid, ratio) != g {
                continue;
            }
        }
        if let Some(s) = wish.shiny {
            if is_shiny(pid, tid, sid, 8) != s {
                continue;
            }
        }
        let c = next(b);
        let d = next(c);
        let iv32 = ((c >> 16) & 0x7FFF) | (((d >> 16) & 0x7FFF) << 15);
        return Some((pid, iv32));
    }
    None
}

/// PID libre (Gen 3 à 5 hors Méthode 1) : sexe et talent liés au PID, chromatique au choix.
pub fn generate_pid(rand: &mut Rand, tid: u16, sid: u16, threshold: u32, wish: PidWish, gen5_wild: bool, gen5_ability_high: bool) -> u32 {
    for _ in 0..5_000_000 {
        let mut pid = rand.next_u32();
        if wish.shiny == Some(true) {
            // XOR < seuil : on recalcule la moitié haute.
            let low = pid & 0xFFFF;
            let x = rand.below(threshold);
            pid = ((tid as u32 ^ sid as u32 ^ low ^ x) & 0xFFFF) << 16 | low;
        }
        if let Some(bit) = wish.ability_bit {
            if gen5_ability_high {
                pid = (pid & !0x1_0000) | (bit << 16);
            } else if pid & 1 != bit {
                continue;
            }
        }
        if gen5_wild {
            pid = gen5_fix_xor(pid, tid, sid);
        }
        if let Some(n) = wish.nature {
            if !gen5_ability_high && pid % 25 != n as u32 {
                continue;
            }
        }
        if let Some((g, ratio)) = wish.gender {
            if g < 2 && gender_of(pid, ratio) != g {
                continue;
            }
        }
        let shiny = is_shiny(pid, tid, sid, threshold);
        if let Some(s) = wish.shiny {
            if shiny != s {
                continue;
            }
        }
        return pid;
    }
    rand.next_u32()
}

// --- Gen 4 sauvage : méthodes J (DPPt) et K (HGSS), Poké Radar.
//
// D'après `MethodJ.cs`, `MethodK.cs`, `SlotMethodJ.cs`, `SlotMethodK.cs` et
// `SlotMethodH.cs` de PKHeX. Avant le PID, le jeu tire l'emplacement (slot), parfois le
// niveau, puis la nature ; il retire ensuite des PID (bas, haut) jusqu'à obtenir cette
// nature, et enfin les IV. Un talent Synchronisation en tête remplace le tirage de la
// nature par un test (réussi : nature du meneur) ; les autres talents de tête ajoutent un
// tirage entre le slot et la nature.

/// Méthode de tirage des rencontres sauvages de la Gen 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method4 {
    /// Diamant, Perle, Platine.
    J,
    /// HeartGold, SoulSilver.
    K,
}

impl Method4 {
    pub fn of_version(version: u8) -> Self {
        if matches!(version, 7 | 8) {
            Method4::K
        } else {
            Method4::J
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Method4::J => "méthode J",
            Method4::K => "méthode K",
        }
    }

    fn nature(self, r: u32) -> u32 {
        match self {
            Method4::J => r / 0xA3E,
            Method4::K => r % 25,
        }
    }

    fn sync_pass(self, r: u32) -> bool {
        match self {
            Method4::J => r >> 15 == 0,
            Method4::K => r & 1 == 0,
        }
    }
}

/// Type de zone Gen 4 utile aux tirages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area4 {
    Grass,
    Surf,
    OldRod,
    GoodRod,
    SuperRod,
}

/// Un emplacement de rencontre : numéro de slot et niveaux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot4 {
    pub area: Area4,
    pub slot: u8,
    pub level_min: u8,
    pub level_max: u8,
}

fn slot_regular(roll: u32) -> u8 {
    match roll {
        0..=19 => 0,
        20..=39 => 1,
        40..=49 => 2,
        50..=59 => 3,
        60..=69 => 4,
        70..=79 => 5,
        80..=84 => 6,
        85..=89 => 7,
        90..=93 => 8,
        94..=97 => 9,
        98 => 10,
        99 => 11,
        _ => u8::MAX,
    }
}

fn slot_surf(roll: u32) -> u8 {
    match roll {
        0..=59 => 0,
        60..=89 => 1,
        90..=94 => 2,
        95..=98 => 3,
        99 => 4,
        _ => u8::MAX,
    }
}

/// Slot tiré par `r` (16 bits hauts du générateur).
fn slot_of(method: Method4, area: Area4, r: u32) -> u8 {
    match method {
        Method4::J => {
            let esv = r / 656;
            match area {
                Area4::Grass => slot_regular(esv),
                Area4::Surf | Area4::OldRod => slot_surf(esv),
                Area4::GoodRod | Area4::SuperRod => match esv {
                    0..=39 => 0,
                    40..=79 => 1,
                    80..=94 => 2,
                    95..=98 => 3,
                    99 => 4,
                    _ => u8::MAX,
                },
            }
        }
        Method4::K => {
            let roll = r % 100;
            match area {
                Area4::Grass => slot_regular(roll),
                Area4::Surf => slot_surf(roll),
                _ => match roll {
                    0..=39 => 0,
                    40..=69 => 1,
                    70..=84 => 2,
                    85..=94 => 3,
                    _ => 4,
                },
            }
        }
    }
}

/// Le niveau est-il tiré au hasard (sinon : fixé par le slot) ?
fn level_rand(area: Area4) -> bool {
    area != Area4::Grass
}

fn random_level(s: &Slot4, r: u32) -> u8 {
    let span = 1 + s.level_max.saturating_sub(s.level_min) as u32;
    (r % span) as u8 + s.level_min
}

/// La canne peut-elle ferrer avec ce tirage (`IsFishPossible`) ?
fn rod_bites(method: Method4, area: Area4, r: u32) -> bool {
    let rate = match area {
        Area4::OldRod => 25,
        Area4::GoodRod => 50,
        Area4::SuperRod => 75,
        _ => return true,
    };
    match method {
        Method4::J => r / 656 < rate,
        // HGSS : le Pokémon qui suit le dresseur peut ajouter 50 (cas le plus favorable).
        Method4::K => r % 100 < rate + 50,
    }
}

/// Toutes les graines de méthode 1 (état juste avant la moitié basse du PID).
pub fn method1_seeds(pid: u32, iv32: u32) -> Vec<u32> {
    let (iv1, iv2) = iv_words(iv32);
    seeds_for(pid & 0xFFFF, pid >> 16)
        .filter(|&seed| {
            let a = next(next(next(seed)));
            let b = next(a);
            (a >> 16) & 0x7FFF == iv1 && (b >> 16) & 0x7FFF == iv2
        })
        .collect()
}

/// Nombre de paires de PID rejetées possibles avant le PID retenu (`GetReversalWindow`).
fn reversal_window(seed: u32, nature: u32) -> usize {
    let mut s = seed;
    let mut b = s >> 16;
    let mut count = 0;
    while count < 10_000 {
        s = prev(s);
        let a = s >> 16;
        if ((b << 16) | a) % 25 == nature {
            break;
        }
        s = prev(s);
        b = s >> 16;
        count += 1;
    }
    count
}

/// Tête de l'équipe déduite des tirages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lead4 {
    /// Aucun talent de tête.
    None,
    /// Talent de tête sans effet (Synchronisation ratée, Joli Sourire raté…).
    Fail,
    /// Synchronisation réussie (nature du meneur).
    Synchronize,
    /// Poké Radar (DPPt, hautes herbes) : tirages libres.
    Radar,
}

/// Le slot (et le niveau) se lisent à partir de l'état `before` (appel juste avant le
/// tirage de la nature ou de la Synchronisation), en sautant `skip` appels.
fn slot_matches(method: Method4, s: &Slot4, before: u32, skip: usize, met_level: Option<u8>) -> bool {
    let mut st = before;
    for _ in 0..skip {
        st = prev(st);
    }
    if level_rand(s.area) {
        if let Some(l) = met_level {
            if random_level(s, st >> 16) != l {
                return false;
            }
        }
        st = prev(st);
    }
    slot_of(method, s.area, st >> 16) == s.slot
}

/// Une rencontre sauvage Gen 4 (méthode J ou K) peut-elle produire ce PID et ces IV ?
/// `met_level` : niveau de rencontre à retrouver (Pokémon jamais transféré), sinon libre.
pub fn frame4(method: Method4, slots: &[Slot4], met_level: Option<u8>, pid: u32, iv32: u32) -> Option<Lead4> {
    let nature = pid % 25;
    let mut lead: Option<Lead4> = None;
    for seed in method1_seeds(pid, iv32) {
        let window = reversal_window(seed, nature);
        let mut cur = seed;
        for _ in 0..=window {
            let p0 = cur >> 16;
            let before = prev(cur);
            for s in slots {
                if method.nature(p0) == nature {
                    if slot_matches(method, s, before, 0, met_level) {
                        return Some(Lead4::None);
                    }
                    if slot_matches(method, s, before, 1, met_level) {
                        lead = Some(Lead4::Fail);
                    }
                }
                if lead.is_none() && method.sync_pass(p0) && slot_matches(method, s, before, 0, met_level) {
                    lead = Some(Lead4::Synchronize);
                }
            }
            cur = prev(prev(cur));
        }
    }
    if lead.is_none() && method == Method4::J && slots.iter().any(|s| s.area == Area4::Grass) {
        lead = Some(Lead4::Radar);
    }
    lead
}

fn wish_ok(pid: u32, tid: u16, sid: u16, wish: &PidWish) -> bool {
    if let Some(bit) = wish.ability_bit {
        if pid & 1 != bit {
            return false;
        }
    }
    if let Some((g, ratio)) = wish.gender {
        if g < 2 && gender_of(pid, ratio) != g {
            return false;
        }
    }
    if let Some(s) = wish.shiny {
        if is_shiny(pid, tid, sid, 8) != s {
            return false;
        }
    }
    true
}

/// PID et IV tirés par une rencontre sauvage Gen 4 sans talent de tête, pour un slot et
/// un niveau donnés, en respectant les souhaits (nature, sexe, talent ; pour un
/// chromatique, voir [`generate_chain_shiny`] et [`generate_frame4_shiny`]).
pub fn generate_frame4(rand: &mut Rand, method: Method4, slot: &Slot4, met_level: Option<u8>, tid: u16, sid: u16, wish: PidWish) -> Option<(u32, u32)> {
    for _ in 0..400_000 {
        let mut s = rand.next_u32();
        if matches!(slot.area, Area4::OldRod | Area4::GoodRod | Area4::SuperRod) {
            s = next(s);
            if !rod_bites(method, slot.area, s >> 16) {
                continue;
            }
        }
        s = next(s);
        if slot_of(method, slot.area, s >> 16) != slot.slot {
            continue;
        }
        if level_rand(slot.area) {
            s = next(s);
            if met_level.is_some_and(|l| random_level(slot, s >> 16) != l) {
                continue;
            }
        }
        s = next(s);
        let nature = method.nature(s >> 16);
        if nature >= 25 || wish.nature.is_some_and(|n| n as u32 != nature) {
            continue;
        }
        let mut pid = 0;
        let mut found = false;
        for _ in 0..1000 {
            let a = next(s);
            let b = next(a);
            s = b;
            pid = (b >> 16) << 16 | (a >> 16);
            if pid % 25 == nature {
                found = true;
                break;
            }
        }
        if !found || !wish_ok(pid, tid, sid, &wish) {
            continue;
        }
        let c = next(s);
        let d = next(c);
        return Some((pid, ((c >> 16) & 0x7FFF) | (((d >> 16) & 0x7FFF) << 15)));
    }
    None
}

/// Chromatique sauvage hors Poké Radar : PID de méthode 1 chromatique dont les tirages
/// précédents correspondent au slot (avec ou sans talent de tête).
pub fn generate_frame4_shiny(rand: &mut Rand, method: Method4, slot: &Slot4, met_level: Option<u8>, tid: u16, sid: u16, wish: PidWish) -> Option<(u32, u32)> {
    for _ in 0..48 {
        let (pid, iv32) = generate_method1(rand, tid, sid, wish)?;
        if frame4(method, std::slice::from_ref(slot), met_level, pid, iv32).is_some_and(|l| l != Lead4::Radar) {
            return Some((pid, iv32));
        }
    }
    None
}

/// Chromatique du Poké Radar (DPPt) : PID bâti bit à bit à partir des tirages, puis les IV
/// (inverse de [`chain_shiny`]).
pub fn generate_chain_shiny(rand: &mut Rand, tid: u16, sid: u16, wish: PidWish) -> Option<(u32, u32)> {
    for _ in 0..2_000_000 {
        let mut s = rand.next_u32();
        s = next(s);
        let lower = (s >> 16) & 7;
        s = next(s);
        let upper = (s >> 16) & 7;
        let mut low = lower;
        for i in 3..=15 {
            s = next(s);
            low |= ((s >> 16) & 1) << i;
        }
        let high = ((low ^ tid as u32 ^ sid as u32) & 0xFFF8) | upper;
        let pid = high << 16 | low;
        if wish.nature.is_some_and(|n| pid % 25 != n as u32) || !wish_ok(pid, tid, sid, &PidWish { shiny: None, ..wish }) {
            continue;
        }
        let a = next(s);
        let b = next(a);
        let iv32 = ((a >> 16) & 0x7FFF) | (((b >> 16) & 0x7FFF) << 15);
        return Some((pid, iv32));
    }
    None
}

#[cfg(test)]
mod tests4 {
    use super::*;

    #[test]
    fn frame4_round_trip() {
        let mut rand = Rand::new(42);
        for method in [Method4::J, Method4::K] {
            for (area, slot, min, max, met) in [
                (Area4::Grass, 3u8, 7u8, 7u8, None),
                (Area4::Surf, 1, 20, 40, Some(33)),
                (Area4::SuperRod, 2, 30, 50, Some(41)),
            ] {
                let s = Slot4 { area, slot, level_min: min, level_max: max };
                let wish = PidWish { nature: Some(17), shiny: Some(false), ability_bit: Some(1), gender: Some((1, 127)) };
                let (pid, iv32) = generate_frame4(&mut rand, method, &s, met, 1234, 5678, wish).expect("tirage trouvé");
                assert_eq!(pid % 25, 17);
                assert_eq!(pid & 1, 1);
                assert_eq!(frame4(method, &[s], met, pid, iv32), Some(Lead4::None), "{method:?} {area:?}");
                assert!(method_1_2_4(pid, iv32).is_some_and(|(t, _)| t == PidType::Method1));
            }
        }
        // Un PID de méthode 1 quelconque n'a presque jamais le bon slot sans talent de tête.
        let s = Slot4 { area: Area4::Surf, slot: 4, level_min: 20, level_max: 40 };
        let mut none = 0;
        for _ in 0..40 {
            let (pid, iv32) = generate_method1(&mut rand, 1, 2, PidWish::default()).unwrap();
            none += (frame4(Method4::K, &[s], Some(39), pid, iv32) == Some(Lead4::None)) as u32;
        }
        assert!(none < 10, "{none}");
        let wish = PidWish { nature: Some(5), shiny: Some(true), ability_bit: None, gender: None };
        let (pid, iv32) = generate_chain_shiny(&mut rand, 1234, 5678, wish).unwrap();
        assert_eq!(pid % 25, 5);
        assert!(is_shiny(pid, 1234, 5678, 8));
        assert!(chain_shiny(pid, iv32, 1234, 5678));
    }
}
