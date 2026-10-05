//! Formule de dégâts Gen 4 à 7, portée de Pokémon Showdown (smogon/damage-calc, MIT) :
//! `calculateDPP` (gen4.ts), `calculateBWXY` (gen56.ts) et `calculateSMSSSV` (gen789.ts).
//!
//! Gen 4 : les modificateurs sont des multiplications successives arrondies à
//! l'entier inférieur. Gen 5+ : ils sont « chaînés » sur une base 4096
//! (`chainMods`) puis appliqués avec l'arrondi du jeu (`pokeRound` : x,5 vers le bas).

use serde::Serialize;

use super::ids::{ability as ab, item as it, moves as mv};
use super::types::{self, Eff, FIGHTING, FIRE, FLYING, GHOST, GROUND, ICE, NORMAL, ROCK, STEEL, WATER};
use super::{Combatant, Field, SideState, Status, Weather};
use crate::dex::{self, MoveCategory};

/// Dégâts d'une utilisation de l'attaque.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Damage {
    /// Pas de dégâts (immunité, attaque de statut…) et pourquoi.
    None { reason: String },
    /// Dégâts fixes (Frappe Atlas, Draco-Rage…).
    Fixed { amount: u16 },
    /// Les 16 jets possibles (aléatoire 85 à 100 %), pour chaque coup.
    Rolls { hits: Vec<[u16; 16]> },
}

/// Résultat du calcul d'une attaque.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveCalc {
    pub move_id: u16,
    pub name: String,
    /// Type final (après Ball'Météo, Peau Féérique…).
    pub type_id: u8,
    pub category: MoveCategory,
    /// Puissance de base utilisée (après puissance variable : Balayage, Gyroballe…).
    pub power: Option<u16>,
    pub priority: i8,
    pub hits: u8,
    /// Multiplicateur de type total (0, 0,25 … 4).
    pub effectiveness: f32,
    pub critical: bool,
    pub damage: Damage,
    /// Modificateurs appliqués, en clair (« STAB ×1,5 », « Bandeau Choix ×1,5 »…).
    pub modifiers: Vec<String>,
}

// --- Arrondis du jeu (util.ts) ---

/// `pokeRound(n / d)` : arrondi au plus proche, x,5 vers le bas.
fn round_div(n: u64, d: u64) -> u64 {
    let (q, r) = (n / d, n % d);
    if 2 * r > d {
        q + 1
    } else {
        q
    }
}

/// `chainMods` : enchaîne des multiplicateurs exprimés sur 4096.
fn chain(mods: &[u64], lower: u64, upper: u64) -> u64 {
    let mut m = 4096u64;
    for &x in mods {
        if x != 4096 {
            m = (m * x + 2048) >> 12;
        }
    }
    m.clamp(lower, upper)
}

/// `getModifiedStat` : niveaux −6 à +6 (×2/8 … ×8/2).
fn modified_stat(stat: u64, stage: i8) -> u64 {
    let s = stage.clamp(-6, 6);
    if s >= 0 {
        stat * (2 + s as u64) / 2
    } else {
        stat * 2 / (2 + (-s) as u64)
    }
}

/// Simple (Gen 4) : les niveaux comptent double.
fn simple_stat(stat: u64, stage: i8) -> u64 {
    modified_stat(stat, (stage as i16 * 2).clamp(-6, 6) as i8)
}

// --- Noms pour les libellés ---

fn ability_label(id: u16) -> String {
    dex::ability_name(id).unwrap_or("talent").to_string()
}

fn item_label(id: u16) -> String {
    dex::item_name(id).unwrap_or("objet").to_string()
}

fn fr(x: f32) -> String {
    let s = format!("{x}");
    s.replace('.', ",")
}

/// Un combattant pendant le calcul.
struct Mon<'a> {
    c: &'a Combatant,
    side: &'a SideState,
    ability: u16,
    boosts: [i8; 6],
    max_hp: u64,
    cur_hp: u64,
    speed: u64,
}

impl Mon<'_> {
    fn has(&self, a: u16) -> bool {
        self.ability == a
    }
    fn has_any(&self, list: &[u16]) -> bool {
        list.contains(&self.ability)
    }
    fn item(&self) -> u16 {
        self.c.item
    }
    fn status(&self) -> Status {
        self.side.status
    }
    fn has_status(&self) -> bool {
        !matches!(self.side.status, Status::None)
    }
    fn has_type(&self, t: u8) -> bool {
        self.c.types.contains(&t)
    }
    /// PV ≤ 1/3 (Brasier, Torrent, Engrais, Essaim).
    fn pinch(&self) -> bool {
        self.cur_hp * 3 <= self.max_hp
    }
}

fn new_mon<'a>(c: &'a Combatant, side: &'a SideState) -> Mon<'a> {
    let max_hp = c.stats[0].max(1) as u64;
    let pct = side.hp_percent.clamp(1, 100) as u64;
    let cur_hp = (max_hp * pct).div_ceil(100).clamp(1, max_hp);
    Mon { c, side, ability: c.ability, boosts: side.boosts, max_hp, cur_hp, speed: 0 }
}

/// Météo effective : Ciel Gris et Air Lock l'annulent.
pub fn effective_weather(field: &Field, a: &Combatant, b: &Combatant) -> Weather {
    let negates = |c: &Combatant| c.ability == ab::CLOUD_NINE || c.ability == ab::AIR_LOCK;
    if negates(a) || negates(b) {
        Weather::None
    } else {
        field.weather
    }
}

/// Vitesse finale (`getFinalSpeed`) : niveaux, talents de météo, Mouchoir Choix, paralysie.
pub fn speed(generation: u8, c: &Combatant, side: &SideState, weather: Weather) -> u32 {
    let mut s = modified_stat(c.stats[5] as u64, side.boosts[5]);
    let mut mods = Vec::new();
    let status = !matches!(side.status, Status::None);
    if (c.ability == ab::CHLOROPHYLL && weather == Weather::Sun)
        || (c.ability == ab::SWIFT_SWIM && weather == Weather::Rain)
        || (generation >= 5 && c.ability == ab::SAND_RUSH && weather == Weather::Sand)
        || (generation >= 7 && c.ability == ab::SLUSH_RUSH && weather == Weather::Hail)
    {
        mods.push(8192);
    } else if c.ability == ab::QUICK_FEET && status {
        mods.push(6144);
    }
    if c.item == it::CHOICE_SCARF {
        mods.push(6144);
    } else if c.item == it::IRON_BALL || c.item == it::MACHO_BRACE || it::POWER_ITEMS.contains(&c.item) {
        mods.push(2048);
    }
    s = round_div(s * chain(&mods, 410, 131172), 4096);
    if side.status == Status::Paralysis && c.ability != ab::QUICK_FEET {
        s = s * if generation >= 7 { 50 } else { 25 } / 100;
    }
    s.min(10000) as u32
}

/// Puissance et type de Puissance Cachée (IV dans l'ordre PV, Att, Déf, Atq Spé, Déf Spé, Vit).
pub fn hidden_power(generation: u8, ivs: [u8; 6]) -> (u8, u16) {
    // Ordre des bits : PV, Att, Déf, Vit, Atq Spé, Déf Spé.
    let order = [0, 1, 2, 5, 3, 4];
    let (mut t, mut p) = (0u32, 0u32);
    for (bit, &i) in order.iter().enumerate() {
        t += ((ivs[i] & 1) as u32) << bit;
        p += (((ivs[i] >> 1) & 1) as u32) << bit;
    }
    let type_id = (t * 15 / 63) as u8 + 1; // Combat (1) … Ténèbres (16)
    let power = if generation <= 5 { (p * 40 / 63 + 30) as u16 } else { 60 };
    (type_id, power)
}

/// Rang des types pour l'ordre d'application en Gen 4 (gen4.ts, `typeEffectivenessPrecedenceRules`).
fn gen4_precedence(t: u8) -> usize {
    use super::types::*;
    [NORMAL, FIRE, WATER, ELECTRIC, GRASS, ICE, FIGHTING, POISON, GROUND, FLYING, PSYCHIC, BUG, ROCK, GHOST, DRAGON, DARK, STEEL]
        .iter()
        .position(|&x| x == t)
        .unwrap_or(99)
}

/// Talents du défenseur ignorés par Brise Moule / Turbo Brasier / Téra-Voltage.
const BREAKABLE: [u16; 24] = [
    ab::BATTLE_ARMOR,
    ab::SHELL_ARMOR,
    ab::STURDY,
    ab::VOLT_ABSORB,
    ab::WATER_ABSORB,
    ab::FLASH_FIRE,
    ab::WONDER_GUARD,
    ab::LEVITATE,
    ab::LIGHTNING_ROD,
    ab::SOUNDPROOF,
    ab::THICK_FAT,
    ab::MARVEL_SCALE,
    ab::MOTOR_DRIVE,
    ab::HEATPROOF,
    ab::SIMPLE,
    ab::DRY_SKIN,
    ab::UNAWARE,
    ab::FILTER,
    ab::STORM_DRAIN,
    ab::SOLID_ROCK,
    ab::MULTISCALE,
    ab::SAP_SIPPER,
    ab::FUR_COAT,
    ab::FLUFFY,
];

const PINCH: [(u16, u8); 4] = [(ab::OVERGROW, types::GRASS), (ab::BLAZE, FIRE), (ab::TORRENT, WATER), (ab::SWARM, types::BUG)];

/// Calcule les dégâts de l'attaque `move_id` de `att` sur `def`.
/// `None` si l'attaque n'existe pas dans ce jeu.
#[allow(clippy::too_many_arguments)]
pub fn calculate(
    game: dex::Game,
    att: &Combatant,
    att_side: &SideState,
    def: &Combatant,
    def_side: &SideState,
    move_id: u16,
    field: &Field,
    crit: bool,
) -> Option<MoveCalc> {
    let generation = game.generation();
    let info = dex::move_info_in(game, move_id)?;
    let mut out = MoveCalc {
        move_id,
        name: dex::move_name(move_id).unwrap_or("?").to_string(),
        type_id: info.type_id,
        category: info.category,
        power: info.power.map(u16::from),
        priority: info.priority,
        hits: 1,
        effectiveness: 1.0,
        critical: false,
        damage: Damage::None { reason: String::new() },
        modifiers: Vec::new(),
    };
    let none = |mut out: MoveCalc, reason: &str| {
        out.damage = Damage::None { reason: reason.to_string() };
        Some(out)
    };
    if info.category == MoveCategory::Status {
        return none(out, "Attaque de statut : pas de dégâts directs.");
    }

    let weather = effective_weather(field, att, def);
    let mut a = new_mon(att, att_side);
    let mut d = new_mon(def, def_side);

    // Intimidation (util.ts, checkIntimidate), dans les deux sens.
    if field.intimidate {
        apply_intimidate(&d, &mut a, &mut out.modifiers, true);
        apply_intimidate(&a, &mut d, &mut out.modifiers, false);
    }
    a.speed = speed(generation, att, &SideState { boosts: a.boosts, ..att_side.clone() }, weather) as u64;
    d.speed = speed(generation, def, &SideState { boosts: d.boosts, ..def_side.clone() }, weather) as u64;

    // Brise Moule et apparentés.
    let breaker = [ab::MOLD_BREAKER, ab::TERAVOLT, ab::TURBOBLAZE];
    if a.has_any(&breaker) && BREAKABLE.contains(&d.ability) {
        out.modifiers.push(format!("{} ignore {}", ability_label(a.ability), ability_label(d.ability)));
        d.ability = 0;
    }

    let is_crit = crit && !d.has_any(&[ab::BATTLE_ARMOR, ab::SHELL_ARMOR]);
    out.critical = is_crit;

    // --- Type de l'attaque ---
    let mut move_type = info.type_id;
    let mut base_power = info.power.map(u16::from);
    let mut ate_boost = false;
    if move_id == mv::WEATHER_BALL {
        move_type = match weather {
            Weather::Sun => FIRE,
            Weather::Rain => WATER,
            Weather::Sand => ROCK,
            Weather::Hail => ICE,
            Weather::None => NORMAL,
        };
    } else if move_id == mv::JUDGMENT {
        if let Some(t) = it::plate_type(a.item()) {
            move_type = t;
        }
    } else if move_id == mv::HIDDEN_POWER {
        let (t, p) = hidden_power(generation, att.ivs);
        move_type = t;
        base_power = Some(p);
    }
    let no_type_change = [mv::JUDGMENT, mv::WEATHER_BALL, mv::STRUGGLE].contains(&move_id);
    if generation <= 4 {
        if a.has(ab::NORMALIZE) && move_id != mv::STRUGGLE {
            move_type = NORMAL;
        }
    } else if !no_type_change {
        let normal = move_type == NORMAL;
        let ate = match a.ability {
            ab::AERILATE if normal => Some(FLYING),
            ab::PIXILATE if normal => Some(types::FAIRY),
            ab::REFRIGERATE if normal => Some(ICE),
            ab::GALVANIZE if normal && generation >= 7 => Some(types::ELECTRIC),
            _ => None,
        };
        if let Some(t) = ate {
            move_type = t;
            ate_boost = true;
            out.modifiers.push(format!("{} : type {}", ability_label(a.ability), types::name(t)));
        } else if a.has(ab::NORMALIZE) {
            move_type = NORMAL;
            ate_boost = generation >= 7;
        }
    }
    out.type_id = move_type;

    // --- Efficacité ---
    let mut def_types = types::distinct(&def.types);
    if generation <= 4 && def_types.len() == 2 && gen4_precedence(def_types[0]) > gen4_precedence(def_types[1]) {
        def_types.swap(0, 1);
    }
    let ghost_revealed = a.has(ab::SCRAPPY);
    let mut effs: Vec<Eff> = def_types
        .iter()
        .map(|&t| {
            if ghost_revealed && t == GHOST && (move_type == NORMAL || move_type == FIGHTING) {
                types::NEUTRAL
            } else if move_id == mv::FREEZE_DRY && t == WATER {
                types::SUPER
            } else {
                let e = types::effectiveness(generation, move_type, t);
                if move_id == mv::FLYING_PRESS {
                    e * types::effectiveness(generation, FLYING, t) / 4
                } else {
                    e
                }
            }
        })
        .collect();
    if move_type == GROUND && d.item() == it::IRON_BALL && effs.contains(&types::IMMUNE) {
        // Balle Fer : le Pokémon touche le sol.
        if let Some(e) = effs.iter_mut().find(|e| **e == types::IMMUNE) {
            *e = types::NEUTRAL;
        }
    }
    let eff_num: u64 = effs.iter().map(|&e| e as u64).product();
    let eff_den: u64 = 4u64.pow(effs.len() as u32);
    out.effectiveness = eff_num as f32 / eff_den as f32;
    if eff_num == 0 {
        return none(out, "Aucun effet : immunité de type.");
    }
    let super_eff = eff_num > eff_den;
    let not_very = eff_num < eff_den;

    // --- Talents et objets qui annulent l'attaque ---
    let absorbed = (d.has(ab::WONDER_GUARD) && !super_eff && !(generation <= 4 && move_id == 424))
        || (move_type == FIRE && d.has(ab::FLASH_FIRE))
        || (move_type == WATER && (d.has(ab::WATER_ABSORB) || d.has(ab::DRY_SKIN) || (generation >= 5 && d.has(ab::STORM_DRAIN))))
        || (move_type == types::ELECTRIC
            && (d.has(ab::VOLT_ABSORB) || d.has(ab::MOTOR_DRIVE) || (generation >= 5 && d.has(ab::LIGHTNING_ROD))))
        || (move_type == types::GRASS && generation >= 5 && d.has(ab::SAP_SIPPER))
        || (move_type == GROUND && d.has(ab::LEVITATE) && d.item() != it::IRON_BALL)
        || (mv::SOUND.contains(&move_id) && d.has(ab::SOUNDPROOF))
        || (generation >= 6 && mv::BULLET.contains(&move_id) && d.has(ab::BULLETPROOF));
    if absorbed {
        return none(out, &format!("Aucun effet : talent {}.", ability_label(d.ability)));
    }
    if generation >= 5 && move_type == GROUND && d.item() == it::AIR_BALLOON {
        return none(out, "Aucun effet : Ballon (le Pokémon flotte).");
    }

    // --- Dégâts fixes ---
    let fixed = match move_id {
        mv::SEISMIC_TOSS | mv::NIGHT_SHADE => Some(att.level as u64),
        mv::DRAGON_RAGE => Some(40),
        mv::SONIC_BOOM => Some(20),
        mv::SUPER_FANG | mv::NATURES_MADNESS => Some((d.cur_hp / 2).max(1)),
        mv::ENDEAVOR => Some(d.cur_hp.saturating_sub(a.cur_hp)),
        mv::FINAL_GAMBIT => Some(a.cur_hp),
        _ if mv::OHKO.contains(&move_id) => {
            if att.level < def.level || (generation >= 5 && d.has(ab::STURDY)) {
                return none(out, "Échoue : la cible est de plus haut niveau (ou a Fermeté).");
            }
            out.modifiers.push("K.O. en un coup s'il touche".into());
            Some(d.cur_hp)
        }
        _ => None,
    };
    if let Some(amount) = fixed {
        out.power = None;
        out.damage = Damage::Fixed { amount: amount.min(u16::MAX as u64) as u16 };
        return Some(out);
    }
    if move_id == mv::PSYWAVE {
        return none(out, "Vague Psy : dégâts aléatoires (½ à 1,5 × le niveau), non calculés.");
    }

    // --- Nombre de coups ---
    let hits: u8 = if mv::TWO_HITS.contains(&move_id) {
        2
    } else if move_id == mv::TRIPLE_KICK {
        3
    } else if mv::MULTI_HITS.contains(&move_id) {
        if a.has(ab::SKILL_LINK) {
            out.modifiers.push(format!("{} : 5 coups", ability_label(a.ability)));
            5
        } else {
            out.modifiers.push("2 à 5 coups : 3 comptés".into());
            3
        }
    } else {
        1
    };
    out.hits = hits;

    let physical = info.category == MoveCategory::Physical;
    let ctx = Ctx { generation, a: &a, d: &d, move_id, move_type, physical, crit: is_crit, weather, super_eff, not_very };

    let mut all_hits = Vec::with_capacity(hits as usize);
    for h in 0..hits {
        let mut labels = Vec::new();
        let Some(bp) = ctx.base_power(base_power, h, &mut labels) else {
            return none(out, "Puissance variable non calculée pour cette attaque.");
        };
        if h == 0 {
            out.power = Some(bp as u16);
        }
        let rolls = if generation <= 4 {
            ctx.gen4(bp, &effs, h, &mut labels)
        } else {
            ctx.gen5plus(bp, eff_num, eff_den, ate_boost, h, &mut labels)
        };
        if h == 0 {
            out.modifiers.extend(labels);
        }
        all_hits.push(rolls);
    }
    if super_eff {
        out.modifiers.push(format!("Super efficace ×{}", fr(out.effectiveness)));
    } else if not_very {
        out.modifiers.push(format!("Pas très efficace ×{}", fr(out.effectiveness)));
    }
    out.damage = Damage::Rolls { hits: all_hits };
    Some(out)
}

/// Intimidation de `source` sur `target` (util.ts, checkIntimidate).
fn apply_intimidate(source: &Mon, target: &mut Mon, labels: &mut Vec<String>, label: bool) {
    if source.ability != ab::INTIMIDATE {
        return;
    }
    if target.has_any(&[ab::CLEAR_BODY, ab::WHITE_SMOKE, ab::HYPER_CUTTER, ab::FULL_METAL_BODY]) {
        return;
    }
    let atk = target.boosts[1];
    target.boosts[1] = if target.has_any(&[ab::CONTRARY, ab::DEFIANT]) {
        (atk + 1).min(6)
    } else if target.has(ab::SIMPLE) {
        (atk - 2).max(-6)
    } else {
        (atk - 1).max(-6)
    };
    if target.has(ab::COMPETITIVE) {
        target.boosts[3] = (target.boosts[3] + 2).min(6);
    }
    if label {
        labels.push("Intimidation adverse : Attaque −1".into());
    }
}

struct Ctx<'a> {
    generation: u8,
    a: &'a Mon<'a>,
    d: &'a Mon<'a>,
    move_id: u16,
    move_type: u8,
    physical: bool,
    crit: bool,
    weather: Weather,
    super_eff: bool,
    not_very: bool,
}

impl Ctx<'_> {
    /// Puissance de base, avant les modificateurs (`calculateBasePower*`).
    fn base_power(&self, listed: Option<u16>, hit: u8, labels: &mut Vec<String>) -> Option<u64> {
        let (a, d, gen5) = (self.a, self.d, self.generation >= 5);
        let first = a.speed > d.speed;
        let bp = listed.map(u64::from);
        let weight_power = |w: u64| match w {
            2000.. => 120,
            1000.. => 100,
            500.. => 80,
            250.. => 60,
            100.. => 40,
            _ => 20,
        };
        Some(match self.move_id {
            mv::LOW_KICK | mv::GRASS_KNOT => {
                if d.c.weight == 0 {
                    return None;
                }
                weight_power(d.c.weight as u64)
            }
            mv::HEAVY_SLAM | mv::HEAT_CRASH => {
                let (wa, wd) = (a.c.weight as u64, d.c.weight.max(1) as u64);
                if a.c.weight == 0 || d.c.weight == 0 {
                    return None;
                }
                match () {
                    _ if wa >= 5 * wd => 120,
                    _ if wa >= 4 * wd => 100,
                    _ if wa >= 3 * wd => 80,
                    _ if wa >= 2 * wd => 60,
                    _ => 40,
                }
            }
            mv::GYRO_BALL => {
                let p = 25 * d.speed / a.speed.max(1) + if gen5 { 1 } else { 0 };
                p.min(150)
            }
            mv::ELECTRO_BALL => match a.speed / d.speed.max(1) {
                4.. => 150,
                3 => 120,
                2 => 80,
                1 => 60,
                _ => 40,
            },
            mv::RETURN => (a.c.friendship as u64 * 10 / 25).max(1),
            mv::FRUSTRATION => ((255 - a.c.friendship as u64) * 10 / 25).max(1),
            mv::FLAIL | mv::REVERSAL => {
                let scale: u64 = if gen5 { 48 } else { 64 };
                let p = scale * a.cur_hp / a.max_hp;
                let t: [u64; 5] = if gen5 { [1, 4, 9, 16, 32] } else { [1, 5, 12, 21, 42] };
                match p {
                    _ if p <= t[0] => 200,
                    _ if p <= t[1] => 150,
                    _ if p <= t[2] => 100,
                    _ if p <= t[3] => 80,
                    _ if p <= t[4] => 40,
                    _ => 20,
                }
            }
            mv::ERUPTION | mv::WATER_SPOUT => (150 * a.cur_hp / a.max_hp).max(1),
            mv::CRUSH_GRIP | mv::WRING_OUT => {
                if gen5 {
                    let p = 100 * (d.cur_hp * 4096 / d.max_hp);
                    ((120 * p + 2047) / 4096 / 100).max(1)
                } else {
                    d.cur_hp * 120 / d.max_hp + 1
                }
            }
            mv::PUNISHMENT => {
                let n: u64 = d.boosts[1..].iter().filter(|&&b| b > 0).map(|&b| b as u64).sum();
                (60 + 20 * n).min(200)
            }
            mv::STORED_POWER | mv::POWER_TRIP => {
                let n: u64 = a.boosts[1..].iter().filter(|&&b| b > 0).map(|&b| b as u64).sum();
                20 + 20 * n
            }
            mv::TRIPLE_KICK => 10 * (hit as u64 + 1),
            mv::WEATHER_BALL => bp? * if self.weather == Weather::None { 1 } else { 2 },
            mv::ACROBATICS if gen5 => bp? * if a.item() == 0 { 2 } else { 1 },
            mv::HEX if gen5 => bp? * if d.has_status() { 2 } else { 1 },
            mv::PAYBACK => {
                let doubled = !first;
                if doubled {
                    labels.push("Représailles : la cible agit avant ×2".into());
                }
                bp? * if doubled { 2 } else { 1 }
            }
            // Gen 4 : Façade et Saumure doublent la puissance de base.
            mv::FACADE if !gen5 => bp? * if matches!(a.status(), Status::Burn | Status::Paralysis | Status::Poison) { 2 } else { 1 },
            mv::BRINE if !gen5 => bp? * if d.cur_hp * 2 <= d.max_hp { 2 } else { 1 },
            _ => bp?,
        })
    }

    // ----- Génération 4 (gen4.ts) -----

    fn gen4(&self, mut bp: u64, effs: &[Eff], hit: u8, labels: &mut Vec<String>) -> [u16; 16] {
        let (a, d) = (self.a, self.d);
        let mt = self.move_type;

        // calculateBPModsDPP
        if a.has(ab::TECHNICIAN) && bp <= 60 {
            bp = bp * 3 / 2;
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        }
        if (a.item() == it::MUSCLE_BAND && self.physical) || (a.item() == it::WISE_GLASSES && !self.physical) {
            bp = bp * 11 / 10;
            labels.push(format!("{} ×1,1", item_label(a.item())));
        } else if it::boosted_type(a.item()) == Some(mt) || self.orb_boost() {
            bp = bp * 6 / 5;
            labels.push(format!("{} ×1,2", item_label(a.item())));
        }
        if (a.has(ab::RECKLESS) && mv::RECKLESS.contains(&self.move_id)) || (a.has(ab::IRON_FIST) && mv::PUNCH.contains(&self.move_id)) {
            bp = bp * 6 / 5;
            labels.push(format!("{} ×1,2", ability_label(a.ability)));
        } else if a.pinch() && PINCH.iter().any(|&(ab, t)| a.has(ab) && mt == t) {
            bp = bp * 3 / 2;
            labels.push(format!("{} (PV ≤ 1/3) ×1,5", ability_label(a.ability)));
        }
        if (d.has(ab::HEATPROOF) && mt == FIRE) || (d.has(ab::THICK_FAT) && (mt == FIRE || mt == ICE)) {
            bp /= 2;
            labels.push(format!("{} adverse ×0,5", ability_label(d.ability)));
        } else if d.has(ab::DRY_SKIN) && mt == FIRE {
            bp = bp * 5 / 4;
            labels.push(format!("{} adverse ×1,25", ability_label(d.ability)));
        }

        // calculateAttackDPP
        let raw_atk = a.c.stats[if self.physical { 1 } else { 3 }] as u64;
        let atk_boost = a.boosts[if self.physical { 1 } else { 3 }];
        let mut attack = if d.has(ab::UNAWARE) {
            raw_atk
        } else if a.has(ab::SIMPLE) {
            simple_stat(raw_atk, atk_boost)
        } else if atk_boost > 0 || (!self.crit && atk_boost < 0) {
            modified_stat(raw_atk, atk_boost)
        } else {
            raw_atk
        };
        if self.physical && (a.has(ab::PURE_POWER) || a.has(ab::HUGE_POWER)) {
            attack *= 2;
            labels.push(format!("{} ×2", ability_label(a.ability)));
        } else if (self.weather == Weather::Sun && !self.physical && a.has(ab::SOLAR_POWER))
            || (self.physical && (a.has(ab::HUSTLE) || (a.has(ab::GUTS) && a.has_status())))
        {
            attack = attack * 3 / 2;
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        }
        let species = a.c.species;
        if (self.physical && a.item() == it::CHOICE_BAND)
            || (!self.physical && a.item() == it::CHOICE_SPECS)
            || (!self.physical && a.item() == it::SOUL_DEW && (species == 380 || species == 381))
        {
            attack = attack * 3 / 2;
            labels.push(format!("{} ×1,5", item_label(a.item())));
        } else if (a.item() == it::LIGHT_BALL && species == 25)
            || (a.item() == it::THICK_CLUB && (species == 104 || species == 105) && self.physical)
            || (a.item() == it::DEEP_SEA_TOOTH && species == 366 && !self.physical)
        {
            attack *= 2;
            labels.push(format!("{} ×2", item_label(a.item())));
        }

        // calculateDefenseDPP
        let raw_def = d.c.stats[if self.physical { 2 } else { 4 }] as u64;
        let def_boost = d.boosts[if self.physical { 2 } else { 4 }];
        let mut defense = if a.has(ab::UNAWARE) {
            raw_def
        } else if d.has(ab::SIMPLE) {
            simple_stat(raw_def, def_boost)
        } else if def_boost < 0 || (!self.crit && def_boost > 0) {
            modified_stat(raw_def, def_boost)
        } else {
            raw_def
        };
        if d.has(ab::MARVEL_SCALE) && d.has_status() && self.physical {
            defense = defense * 3 / 2;
            labels.push(format!("{} adverse ×1,5", ability_label(d.ability)));
        }
        let dspecies = d.c.species;
        if d.item() == it::SOUL_DEW && (dspecies == 380 || dspecies == 381) && !self.physical {
            defense = defense * 3 / 2;
        } else if (d.item() == it::DEEP_SEA_SCALE && dspecies == 366 && !self.physical)
            || (d.item() == it::METAL_POWDER && dspecies == 132 && self.physical)
        {
            defense *= 2;
        }
        if self.weather == Weather::Sand && d.has_type(ROCK) && !self.physical {
            defense = defense * 3 / 2;
            labels.push("Tempête de sable : Déf. Spé. Roche ×1,5".into());
        }
        if self.move_id == mv::EXPLOSION || self.move_id == mv::SELF_DESTRUCT {
            defense /= 2;
        }
        let defense = defense.max(1);

        let level = a.c.level as u64;
        let mut base = (2 * level / 5 + 2) * bp * attack / 50 / defense;
        if a.status() == Status::Burn && self.physical && !a.has(ab::GUTS) {
            base /= 2;
            labels.push("Brûlure ×0,5".into());
        }

        // calculateFinalModsDPP
        let mt = self.move_type;
        if (self.weather == Weather::Sun && mt == FIRE) || (self.weather == Weather::Rain && mt == WATER) {
            base = base * 3 / 2;
            labels.push("Météo ×1,5".into());
        } else if (self.weather == Weather::Sun && mt == WATER)
            || (self.weather == Weather::Rain && mt == FIRE)
            || (self.move_id == mv::SOLAR_BEAM && matches!(self.weather, Weather::Rain | Weather::Sand | Weather::Hail))
        {
            base /= 2;
            labels.push("Météo ×0,5".into());
        }
        base += 2;
        if self.crit {
            if a.has(ab::SNIPER) {
                base *= 3;
                labels.push("Coup critique + Sniper ×3".into());
            } else {
                base *= 2;
                labels.push("Coup critique ×2".into());
            }
        }
        if a.item() == it::LIFE_ORB {
            base = base * 13 / 10;
            labels.push(format!("{} ×1,3", item_label(a.item())));
        }

        let stab = if a.has_type(mt) {
            if a.has(ab::ADAPTABILITY) {
                labels.push("STAB + Adaptabilité ×2".into());
                (2, 1)
            } else {
                labels.push("STAB ×1,5".into());
                (3, 2)
            }
        } else {
            (1, 1)
        };
        let filter = (d.has(ab::FILTER) || d.has(ab::SOLID_ROCK)) && self.super_eff;
        let ebelt = a.item() == it::EXPERT_BELT && self.super_eff;
        let tinted = a.has(ab::TINTED_LENS) && self.not_very;
        let berry = hit == 0 && it::resist_berry_type(d.item()) == Some(mt) && (self.super_eff || mt == NORMAL);
        if filter {
            labels.push(format!("{} adverse ×0,75", ability_label(d.ability)));
        }
        if ebelt {
            labels.push(format!("{} ×1,2", item_label(a.item())));
        }
        if tinted {
            labels.push(format!("{} ×2", ability_label(a.ability)));
        }
        if berry {
            labels.push(format!("{} adverse ×0,5", item_label(d.item())));
        }

        let mut rolls = [0u16; 16];
        for (i, r) in rolls.iter_mut().enumerate() {
            let mut x = base * (85 + i as u64) / 100;
            x = x * stab.0 / stab.1;
            for &e in effs {
                x = x * e as u64 / 4;
            }
            if filter {
                x = x * 3 / 4;
            }
            if ebelt {
                x = x * 6 / 5;
            }
            if tinted {
                x *= 2;
            }
            if berry {
                x /= 2;
            }
            *r = x.clamp(1, u16::MAX as u64) as u16;
        }
        rolls
    }

    fn orb_boost(&self) -> bool {
        let (s, i, t) = (self.a.c.species, self.a.item(), self.move_type);
        (i == it::ADAMANT_ORB && s == 483 && (t == STEEL || t == types::DRAGON))
            || (i == it::LUSTROUS_ORB && s == 484 && (t == WATER || t == types::DRAGON))
            || (i == it::GRISEOUS_ORB && s == 487 && (t == GHOST || t == types::DRAGON))
    }

    // ----- Générations 5 à 7 (gen56.ts, gen789.ts) -----

    fn gen5plus(&self, bp: u64, eff_num: u64, eff_den: u64, ate_boost: bool, hit: u8, labels: &mut Vec<String>) -> [u16; 16] {
        let (a, d, generation) = (self.a, self.d, self.generation);
        let mt = self.move_type;
        let special = !self.physical;

        // calculateBPModsBWXY
        let mut bp_mods: Vec<u64> = Vec::new();
        let slower = a.speed <= d.speed;
        if a.has(ab::TECHNICIAN) && bp <= 60 {
            bp_mods.push(6144);
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        } else if (a.has(ab::ANALYTIC) && slower)
            || (a.has(ab::SAND_FORCE) && self.weather == Weather::Sand && (mt == ROCK || mt == GROUND || mt == STEEL))
        {
            bp_mods.push(5325);
            labels.push(format!("{} ×1,3", ability_label(a.ability)));
        } else if (a.has(ab::RECKLESS) && mv::RECKLESS.contains(&self.move_id))
            || (a.has(ab::IRON_FIST) && mv::PUNCH.contains(&self.move_id))
        {
            bp_mods.push(4915);
            labels.push(format!("{} ×1,2", ability_label(a.ability)));
        }
        if d.has(ab::HEATPROOF) && mt == FIRE {
            bp_mods.push(2048);
            labels.push(format!("{} adverse ×0,5", ability_label(d.ability)));
        } else if d.has(ab::DRY_SKIN) && mt == FIRE {
            bp_mods.push(5120);
            labels.push(format!("{} adverse ×1,25", ability_label(d.ability)));
        }
        if a.item() != 0 && it::boosted_type(a.item()) == Some(mt) {
            bp_mods.push(4915);
            labels.push(format!("{} ×1,2", item_label(a.item())));
        } else if (a.item() == it::MUSCLE_BAND && self.physical) || (a.item() == it::WISE_GLASSES && special) {
            bp_mods.push(4505);
            labels.push(format!("{} ×1,1", item_label(a.item())));
        } else if self.orb_boost() {
            bp_mods.push(4915);
            labels.push(format!("{} ×1,2", item_label(a.item())));
        } else if it::gem_type(a.item()) == Some(mt) && hit == 0 {
            bp_mods.push(if generation > 5 { 5325 } else { 6144 });
            labels.push(format!("{} ×{}", item_label(a.item()), if generation > 5 { "1,3" } else { "1,5" }));
        }
        if (self.move_id == mv::FACADE && matches!(a.status(), Status::Burn | Status::Paralysis | Status::Poison))
            || (self.move_id == mv::BRINE && d.cur_hp * 2 <= d.max_hp)
        {
            bp_mods.push(8192);
            labels.push("Puissance ×2".into());
        } else if generation > 5 && self.move_id == mv::KNOCK_OFF && d.item() != 0 && hit == 0 {
            bp_mods.push(6144);
            labels.push("Sabotage (cible avec objet) ×1,5".into());
        } else if self.move_id == mv::SOLAR_BEAM && matches!(self.weather, Weather::Rain | Weather::Sand | Weather::Hail) {
            bp_mods.push(2048);
            labels.push("Lance-Soleil sans soleil ×0,5".into());
        }
        if ate_boost {
            let m = if generation >= 7 { 4915 } else { 5325 };
            bp_mods.push(m);
            labels.push(format!("{} ×{}", ability_label(a.ability), if generation >= 7 { "1,2" } else { "1,3" }));
        } else if (a.has(ab::MEGA_LAUNCHER) && mv::PULSE.contains(&self.move_id))
            || (a.has(ab::STRONG_JAW) && mv::BITE.contains(&self.move_id))
        {
            bp_mods.push(6144);
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        }
        let bp = round_div(bp * chain(&bp_mods, 41, 2_097_152), 4096).max(1) % 65536;

        // calculateAttackBWXY
        let source = if self.move_id == mv::FOUL_PLAY { d } else { a };
        let stat_i = if special { 3 } else { 1 };
        let boost = source.boosts[stat_i];
        let raw = source.c.stats[stat_i] as u64;
        let mut attack = if boost == 0 || (self.crit && boost < 0) || d.has(ab::UNAWARE) { raw } else { modified_stat(raw, boost) };
        if a.has(ab::HUSTLE) && self.physical {
            attack = round_div(attack * 3, 2);
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        }
        let mut at_mods: Vec<u64> = Vec::new();
        if d.has(ab::THICK_FAT) && (mt == FIRE || mt == ICE) {
            at_mods.push(2048);
            labels.push(format!("{} adverse ×0,5", ability_label(d.ability)));
        }
        if (a.has(ab::GUTS) && a.has_status() && self.physical) || (a.pinch() && PINCH.iter().any(|&(x, t)| a.has(x) && mt == t)) {
            at_mods.push(6144);
            let cond = if a.has(ab::GUTS) { "" } else { " (PV ≤ 1/3)" };
            labels.push(format!("{}{cond} ×1,5", ability_label(a.ability)));
        } else if a.has(ab::SOLAR_POWER) && self.weather == Weather::Sun && special {
            at_mods.push(6144);
            labels.push(format!("{} ×1,5", ability_label(a.ability)));
        } else if a.has(ab::DEFEATIST) && a.cur_hp * 2 <= a.max_hp {
            at_mods.push(2048);
            labels.push(format!("{} ×0,5", ability_label(a.ability)));
        } else if (a.has(ab::HUGE_POWER) || a.has(ab::PURE_POWER)) && self.physical {
            at_mods.push(8192);
            labels.push(format!("{} ×2", ability_label(a.ability)));
        }
        let species = a.c.species;
        if (a.item() == it::THICK_CLUB && (species == 104 || species == 105) && self.physical)
            || (a.item() == it::DEEP_SEA_TOOTH && species == 366 && special)
            || (a.item() == it::LIGHT_BALL && species == 25)
        {
            at_mods.push(8192);
            labels.push(format!("{} ×2", item_label(a.item())));
        } else if (generation <= 6 && a.item() == it::SOUL_DEW && (species == 380 || species == 381) && special)
            || (a.item() == it::CHOICE_BAND && self.physical)
            || (a.item() == it::CHOICE_SPECS && special)
        {
            at_mods.push(6144);
            labels.push(format!("{} ×1,5", item_label(a.item())));
        }
        let attack = round_div(attack * chain(&at_mods, 410, 131_072), 4096).max(1) % 65536;

        // calculateDefenseBWXY
        let hits_physical = self.physical || [mv::PSYSHOCK, mv::PSYSTRIKE, mv::SECRET_SWORD].contains(&self.move_id);
        let def_i = if hits_physical { 2 } else { 4 };
        let dboost = d.boosts[def_i];
        let raw = d.c.stats[def_i] as u64;
        let mut defense = if dboost == 0 || (self.crit && dboost > 0) || a.has(ab::UNAWARE) { raw } else { modified_stat(raw, dboost) };
        if self.weather == Weather::Sand && d.has_type(ROCK) && !hits_physical {
            defense = round_div(defense * 3, 2);
            labels.push("Tempête de sable : Déf. Spé. Roche ×1,5".into());
        }
        let mut df_mods: Vec<u64> = Vec::new();
        if d.has(ab::MARVEL_SCALE) && d.has_status() && hits_physical {
            df_mods.push(6144);
            labels.push(format!("{} adverse ×1,5", ability_label(d.ability)));
        }
        let dspecies = d.c.species;
        if (!hits_physical && generation <= 6 && d.item() == it::SOUL_DEW && (dspecies == 380 || dspecies == 381))
            || (!hits_physical && d.item() == it::ASSAULT_VEST)
        {
            df_mods.push(6144);
            labels.push(format!("{} adverse ×1,5", item_label(d.item())));
        }
        if (d.item() == it::METAL_POWDER && dspecies == 132 && hits_physical)
            || (d.item() == it::DEEP_SEA_SCALE && dspecies == 366 && !hits_physical)
        {
            df_mods.push(8192);
        }
        if d.has(ab::FUR_COAT) && hits_physical {
            df_mods.push(8192);
            labels.push(format!("{} adverse ×2", ability_label(d.ability)));
        }
        let defense = round_div(defense * chain(&df_mods, 410, 131_072), 4096).max(1) % 65536;

        // calculateBaseDamageBWXY / SMSSSV
        let level = a.c.level as u64;
        let mut base = (2 * level / 5 + 2) * bp * attack / defense / 50 + 2;
        if (self.weather == Weather::Sun && mt == FIRE) || (self.weather == Weather::Rain && mt == WATER) {
            base = round_div(base * 6144, 4096);
            labels.push("Météo ×1,5".into());
        } else if (self.weather == Weather::Sun && mt == WATER) || (self.weather == Weather::Rain && mt == FIRE) {
            base = round_div(base * 2048, 4096);
            labels.push("Météo ×0,5".into());
        }
        if self.crit {
            if generation > 5 {
                base = base * 3 / 2;
                labels.push("Coup critique ×1,5".into());
            } else {
                base *= 2;
                labels.push("Coup critique ×2".into());
            }
        }

        // getStabMod
        let mut stab = 4096u64;
        if a.has_type(mt) {
            stab += 2048;
            if a.has(ab::ADAPTABILITY) {
                stab += 2048;
                labels.push("STAB + Adaptabilité ×2".into());
            } else {
                labels.push("STAB ×1,5".into());
            }
        }
        let burn = a.status() == Status::Burn && self.physical && !a.has(ab::GUTS) && !(self.move_id == mv::FACADE && generation >= 6);
        if burn {
            labels.push("Brûlure ×0,5".into());
        }

        // calculateFinalModsBWXY / SMSSSV
        let mut final_mods: Vec<u64> = Vec::new();
        let full_hp = d.cur_hp == d.max_hp;
        let multiscale = hit == 0 && full_hp && (d.has(ab::MULTISCALE) || (generation >= 7 && d.has(ab::SHADOW_SHIELD)));
        let filter = self.super_eff && (d.has(ab::FILTER) || d.has(ab::SOLID_ROCK) || (generation >= 7 && d.has(ab::PRISM_ARMOR)));
        let sniper = a.has(ab::SNIPER) && self.crit;
        let tinted = a.has(ab::TINTED_LENS) && self.not_very;
        if generation >= 7 {
            if a.has(ab::NEUROFORCE) && self.super_eff {
                final_mods.push(5120);
                labels.push(format!("{} ×1,25", ability_label(a.ability)));
            } else if sniper {
                final_mods.push(6144);
                labels.push(format!("{} ×1,5", ability_label(a.ability)));
            } else if tinted {
                final_mods.push(8192);
                labels.push(format!("{} ×2", ability_label(a.ability)));
            }
            if multiscale {
                final_mods.push(2048);
                labels.push(format!("{} adverse ×0,5", ability_label(d.ability)));
            }
            if filter {
                final_mods.push(3072);
                labels.push(format!("{} adverse ×0,75", ability_label(d.ability)));
            }
            if d.has(ab::FLUFFY) && mt == FIRE {
                final_mods.push(8192);
                labels.push(format!("{} adverse ×2", ability_label(d.ability)));
            }
        } else {
            if multiscale {
                final_mods.push(2048);
                labels.push(format!("{} adverse ×0,5", ability_label(d.ability)));
            }
            if tinted {
                final_mods.push(8192);
                labels.push(format!("{} ×2", ability_label(a.ability)));
            }
            if sniper {
                final_mods.push(6144);
                labels.push(format!("{} ×1,5", ability_label(a.ability)));
            }
            if filter {
                final_mods.push(3072);
                labels.push(format!("{} adverse ×0,75", ability_label(d.ability)));
            }
        }
        if a.item() == it::EXPERT_BELT && self.super_eff {
            final_mods.push(4915);
            labels.push(format!("{} ×1,2", item_label(a.item())));
        } else if a.item() == it::LIFE_ORB {
            final_mods.push(5324);
            labels.push(format!("{} ×1,3", item_label(a.item())));
        }
        if hit == 0 && it::resist_berry_type(d.item()) == Some(mt) && (self.super_eff || mt == NORMAL) {
            final_mods.push(2048);
            labels.push(format!("{} adverse ×0,5", item_label(d.item())));
        }
        let final_mod = chain(&final_mods, 41, 131_072);

        // getFinalDamage
        let mut rolls = [0u16; 16];
        for (i, r) in rolls.iter_mut().enumerate() {
            let mut x = base * (85 + i as u64) / 100;
            if stab != 4096 {
                x = round_div(x * stab, 4096);
            }
            x = x * eff_num / eff_den;
            if burn {
                x /= 2;
            }
            let scaled = x * final_mod;
            let y = if scaled < 4096 { 1 } else { round_div(scaled, 4096) };
            *r = (y % 65536).max(1) as u16;
        }
        rolls
    }
}
