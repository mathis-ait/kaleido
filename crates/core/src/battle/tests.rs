//! Tests du calculateur. Les valeurs attendues sont recalculées à la main en
//! suivant pas à pas les formules de Pokémon Showdown (gen4.ts, gen56.ts,
//! gen789.ts) : chaque test détaille son calcul.

use super::ids::{ability as ab, item as it};
use super::types::*;
use super::*;
use crate::dex::Game;

/// Pokémon de test : statistiques données directement (PV, Att, Déf, Atq Spé, Déf Spé, Vit).
fn mon(level: u8, types: &[u8], stats: [u16; 6]) -> Combatant {
    Combatant { species: 1, name: "Test".into(), level, types: types.to_vec(), stats, friendship: 255, ..Default::default() }
}

fn rolls(game: Game, a: &Combatant, d: &Combatant, mv: u16) -> (u16, u16) {
    rolls_with(game, a, d, mv, &Field::default(), false)
}

fn rolls_with(game: Game, a: &Combatant, d: &Combatant, mv: u16, field: &Field, crit: bool) -> (u16, u16) {
    let side = SideState::default();
    let c = calculate(game, a, &side, d, &side, mv, field, crit).unwrap();
    match c.damage {
        Damage::Rolls { hits } => (hits[0][0], hits[0][15]),
        Damage::Fixed { amount } => (amount, amount),
        Damage::None { reason } => panic!("pas de dégâts : {reason}"),
    }
}

const EARTHQUAKE: u16 = 89;
const BODY_SLAM: u16 = 34;
const THUNDERBOLT: u16 = 85;
const BULLET_PUNCH: u16 = 418;
const ICE_BEAM: u16 = 58;
const HYPER_VOICE: u16 = 304;
const SHADOW_BALL: u16 = 247;
const MOONBLAST: u16 = 585;

// ---------- Génération 4 (Platine) ----------

/// Carchacrok niv. 50 (Att 182), Séisme (100, Sol, STAB) sur Élektek (Électrik, Déf 95).
/// base = ⌊⌊⌊2×50/5+2⌋ × 100 × 182 / 50⌋ / 95⌋ = ⌊⌊22×100×182/50⌋/95⌋ = ⌊8008/95⌋ = 84 ; +2 = 86.
/// min : ⌊86×85/100⌋ = 73 → STAB ⌊73×1,5⌋ = 109 → ×2 = 218. max : 86 → 129 → 258.
#[test]
fn gen4_garchomp_earthquake_electabuzz() {
    let garchomp = mon(50, &[DRAGON, GROUND], [183, 182, 115, 100, 105, 122]);
    let electabuzz = mon(50, &[ELECTRIC], [140, 100, 95, 115, 105, 125]);
    assert_eq!(rolls(Game::Pt, &garchomp, &electabuzz, EARTHQUAKE), (218, 258));
    // Coup critique Gen 4 : ×2 après le +2 → base 172 ; min ⌊172×0,85⌋ = 146 → 219 → 438 ; max 172 → 258 → 516.
    assert_eq!(rolls_with(Game::Pt, &garchomp, &electabuzz, EARTHQUAKE, &Field::default(), true), (438, 516));
}

/// Bandeau Choix : Att 300 → ⌊300×1,5⌋ = 450. Plaquage (85, Normal, STAB) niv. 100 sur Roche (Déf 200).
/// base = ⌊⌊42×85×450/50⌋/200⌋ = ⌊32130/200⌋ = 160 ; +2 = 162.
/// min : ⌊162×0,85⌋ = 137 → STAB 205 → ×0,5 = 102. max : 162 → 243 → 121.
#[test]
fn gen4_choice_band_resisted() {
    let mut a = mon(100, &[NORMAL], [300, 300, 200, 100, 100, 100]);
    a.item = it::CHOICE_BAND;
    let d = mon(100, &[ROCK], [300, 100, 200, 100, 100, 100]);
    assert_eq!(rolls(Game::Pt, &a, &d, BODY_SLAM), (102, 121));
}

/// Orbe Vie, Tonnerre (95, STAB) niv. 50, Atq Spé 150, sur Eau/Vol (×4), Déf Spé 100.
/// base = ⌊⌊22×95×150/50⌋/100⌋ = ⌊6270/100⌋ = 62 ; +2 = 64 ; Orbe Vie ⌊64×1,3⌋ = 83.
/// min : ⌊83×0,85⌋ = 70 → 105 → 210 → 420. max : 83 → 124 → 248 → 496.
#[test]
fn gen4_life_orb_quad_effective() {
    let mut a = mon(50, &[ELECTRIC], [150, 100, 100, 150, 100, 100]);
    a.item = it::LIFE_ORB;
    let d = mon(50, &[WATER, FLYING], [150, 100, 100, 100, 100, 100]);
    assert_eq!(rolls(Game::Pt, &a, &d, THUNDERBOLT), (420, 496));
}

/// Filtre : Séisme sans STAB, Att 150, sur Électrik (Déf 100) avec Filtre.
/// base = ⌊⌊22×100×150/50⌋/100⌋ = 66 ; +2 = 68.
/// min : ⌊68×0,85⌋ = 57 → ×2 = 114 → ⌊114×0,75⌋ = 85. max : 68 → 136 → 102.
#[test]
fn gen4_filter() {
    let a = mon(50, &[NORMAL], [150, 150, 100, 100, 100, 100]);
    let mut d = mon(50, &[ELECTRIC], [150, 100, 100, 100, 100, 100]);
    d.ability = ab::FILTER;
    assert_eq!(rolls(Game::Pt, &a, &d, EARTHQUAKE), (85, 102));
}

/// Intimidation adverse : Att 150 à −1 → ⌊150×2/3⌋ = 100.
/// base = ⌊⌊22×100×100/50⌋/100⌋ = 44 ; +2 = 46. min : ⌊46×0,85⌋ = 39 → 78. max : 92.
#[test]
fn gen4_intimidate() {
    let a = mon(50, &[NORMAL], [150, 150, 100, 100, 100, 100]);
    let mut d = mon(50, &[ELECTRIC], [150, 100, 100, 100, 100, 100]);
    d.ability = ab::INTIMIDATE;
    assert_eq!(rolls(Game::Pt, &a, &d, EARTHQUAKE), (78, 92));
    let off = Field { intimidate: false, ..Field::default() };
    assert_eq!(rolls_with(Game::Pt, &a, &d, EARTHQUAKE, &off, false), (114, 136));
}

/// Dégâts fixes et immunités : Frappe Atlas = niveau, sans effet sur Spectre ; Lévitation contre Séisme.
#[test]
fn gen4_fixed_and_immunities() {
    let a = mon(42, &[NORMAL], [150, 150, 100, 100, 100, 100]);
    let d = mon(50, &[WATER], [150, 100, 100, 100, 100, 100]);
    assert_eq!(rolls(Game::Pt, &a, &d, 69), (42, 42));
    assert_eq!(rolls(Game::Pt, &a, &d, 82), (40, 40)); // Draco-Rage
    let ghost = mon(50, &[GHOST], [150, 100, 100, 100, 100, 100]);
    let side = SideState::default();
    let c = calculate(Game::Pt, &a, &side, &ghost, &side, 69, &Field::default(), false).unwrap();
    assert!(matches!(c.damage, Damage::None { .. }));
    let mut lev = d.clone();
    lev.ability = ab::LEVITATE;
    let c = calculate(Game::Pt, &a, &side, &lev, &side, EARTHQUAKE, &Field::default(), false).unwrap();
    assert!(matches!(c.damage, Damage::None { .. }));
}

/// Brûlure (Gen 4 : ÷2 sur la base avant le +2) : 66 → 33, +2 = 35 ; min ⌊35×0,85⌋ = 29 → 58 ; max 70.
#[test]
fn gen4_burn() {
    let a = mon(50, &[NORMAL], [150, 150, 100, 100, 100, 100]);
    let d = mon(50, &[ELECTRIC], [150, 100, 100, 100, 100, 100]);
    let burned = SideState { status: Status::Burn, ..SideState::default() };
    let c = calculate(Game::Pt, &a, &burned, &d, &SideState::default(), EARTHQUAKE, &Field::default(), false).unwrap();
    let Damage::Rolls { hits } = c.damage else { panic!() };
    assert_eq!((hits[0][0], hits[0][15]), (58, 70));
}

// ---------- Génération 5 (Noir/Blanc) ----------

/// Même Séisme qu'en Gen 4 avec la formule Gen 5 :
/// base = ⌊⌊22×100×182/95⌋/50⌋+2 = ⌊4214/50⌋+2 = 86.
/// min : ⌊86×0,85⌋ = 73 → 73×6144/4096 = 109,5 → pokeRound 109 → ×2 = 218. max : 86 → 129 → 258.
/// Critique Gen 5 (×2) : 172 → min 146 → 219 → 438 ; max 516.
#[test]
fn gen5_earthquake_and_crit() {
    let garchomp = mon(50, &[DRAGON, GROUND], [183, 182, 115, 100, 105, 122]);
    let electabuzz = mon(50, &[ELECTRIC], [140, 100, 95, 115, 105, 125]);
    assert_eq!(rolls(Game::BW, &garchomp, &electabuzz, EARTHQUAKE), (218, 258));
    assert_eq!(rolls_with(Game::BW, &garchomp, &electabuzz, EARTHQUAKE, &Field::default(), true), (438, 516));
    // Gen 6 : critique ×1,5 → base ⌊86×1,5⌋ = 129 ; min ⌊129×0,85⌋ = 109 → 163,5 → 163 → 326 ; max 129 → 193 → 386.
    assert_eq!(rolls_with(Game::XY, &garchomp, &electabuzz, EARTHQUAKE, &Field::default(), true), (326, 386));
}

/// Orbe Vie Gen 5 (5324/4096) : base = ⌊⌊22×95×150/100⌋/50⌋+2 = ⌊3135/50⌋+2 = 64.
/// min : ⌊64×0,85⌋ = 54 → STAB 81 → ×4 = 324 → 324×5324/4096 = 421,1 → 421.
/// max : 64 → 96 → 384 → 499,1 → 499.
#[test]
fn gen5_life_orb() {
    let mut a = mon(50, &[ELECTRIC], [150, 100, 100, 150, 100, 100]);
    a.item = it::LIFE_ORB;
    let d = mon(50, &[WATER, FLYING], [150, 100, 100, 100, 100, 100]);
    assert_eq!(rolls(Game::BW, &a, &d, THUNDERBOLT), (421, 499));
}

/// Technicien : Pisto-Poing 40 × 6144/4096 = 60. base = ⌊⌊22×60×200/100⌋/50⌋+2 = 54.
/// min : ⌊54×0,85⌋ = 45 → STAB 67,5 → 67. max : 54 → 81.
#[test]
fn gen5_technician() {
    let mut scizor = mon(50, &[BUG, STEEL], [150, 200, 100, 100, 100, 100]);
    scizor.ability = ab::TECHNICIAN;
    let d = mon(50, &[NORMAL], [150, 100, 100, 100, 100, 100]);
    assert_eq!(rolls(Game::BW, &scizor, &d, BULLET_PUNCH), (67, 81));
}

/// Multiécaille (PV pleins) : Laser Glace sur Dragon/Vol (×4), Atq Spé 200, Déf Spé 100.
/// base = ⌊⌊22×95×200/100⌋/50⌋+2 = 85. min : 72 → ×4 = 288 → ×0,5 = 144. max : 85 → 340 → 170.
/// À 50 % de PV, Multiécaille ne joue plus : 288 à 340.
#[test]
fn gen5_multiscale() {
    let a = mon(50, &[NORMAL], [150, 100, 100, 200, 100, 100]);
    let mut dragonite = mon(50, &[DRAGON, FLYING], [166, 100, 100, 100, 100, 100]);
    dragonite.ability = ab::MULTISCALE;
    assert_eq!(rolls(Game::BW, &a, &dragonite, ICE_BEAM), (144, 170));
    let half = SideState { hp_percent: 50, ..SideState::default() };
    let c = calculate(Game::BW, &a, &SideState::default(), &dragonite, &half, ICE_BEAM, &Field::default(), false).unwrap();
    let Damage::Rolls { hits } = c.damage else { panic!() };
    assert_eq!((hits[0][0], hits[0][15]), (288, 340));
}

/// Pluie : Tonnerre sans STAB n'est pas touché, une attaque Eau ×1,5 sur la base (pokeRound).
/// Surf (95, Eau) Atq Spé 150 sur Normal Déf Spé 100 : base = 64 → ×1,5 = 96.
/// min : ⌊96×0,85⌋ = 81 ; max : 96.
#[test]
fn gen5_rain() {
    let a = mon(50, &[NORMAL], [150, 100, 100, 150, 100, 100]);
    let d = mon(50, &[NORMAL], [150, 100, 100, 100, 100, 100]);
    let rain = Field { weather: Weather::Rain, ..Field::default() };
    assert_eq!(rolls_with(Game::BW, &a, &d, 57, &rain, false), (81, 96));
}

// ---------- Générations 6 et 7 ----------

/// Peau Féérique Gen 6 : Mégaphone 90 → Fée, 90×5325/4096 = 117. Atq Spé 180, STAB Fée.
/// base = ⌊⌊22×117×180/100⌋/50⌋+2 = ⌊4633/50⌋+2 = 94. min : 79 → 118,5 → 118. max : 94 → 141.
/// Gen 7 (×1,2 = 4915) : 90×4915/4096 = 108. base = ⌊4276/50⌋+2 = 87 ; min 73 → 109 ; max 87 → 130.
#[test]
fn gen6_gen7_pixilate() {
    let mut sylveon = mon(50, &[FAIRY], [170, 100, 100, 180, 100, 100]);
    sylveon.ability = ab::PIXILATE;
    let d = mon(50, &[NORMAL], [150, 100, 100, 100, 100, 100]);
    assert_eq!(rolls(Game::XY, &sylveon, &d, HYPER_VOICE), (118, 141));
    assert_eq!(rolls(Game::SM, &sylveon, &d, HYPER_VOICE), (109, 130));
}

/// Table des types : Acier résiste à Spectre et Ténèbres jusqu'en Gen 5, plus en Gen 6 ; Fée en Gen 6.
#[test]
fn type_chart_gen5_vs_gen6() {
    assert_eq!(effectiveness(5, GHOST, STEEL), RESISTED);
    assert_eq!(effectiveness(5, DARK, STEEL), RESISTED);
    assert_eq!(effectiveness(6, GHOST, STEEL), NEUTRAL);
    assert_eq!(effectiveness(6, DARK, STEEL), NEUTRAL);
    assert_eq!(effectiveness(6, FAIRY, DRAGON), SUPER);
    assert_eq!(effectiveness(6, DRAGON, FAIRY), IMMUNE);
    assert_eq!(total(6, GROUND, &[FLYING, ROCK]), 0.0);
    assert_eq!(total(4, ICE, &[DRAGON, GROUND]), 4.0);

    let a = mon(50, &[GHOST], [150, 100, 100, 150, 100, 100]);
    let steel = mon(50, &[STEEL], [150, 100, 100, 100, 100, 100]);
    let side = SideState::default();
    let g5 = calculate(Game::BW, &a, &side, &steel, &side, SHADOW_BALL, &Field::default(), false).unwrap();
    let g6 = calculate(Game::XY, &a, &side, &steel, &side, SHADOW_BALL, &Field::default(), false).unwrap();
    assert_eq!((g5.effectiveness, g6.effectiveness), (0.5, 1.0));
    let dragon = mon(50, &[DRAGON], [150, 100, 100, 100, 100, 100]);
    let moon = calculate(Game::XY, &a, &side, &dragon, &side, MOONBLAST, &Field::default(), false).unwrap();
    assert_eq!(moon.effectiveness, 2.0);
}

/// Vitesse : Mouchoir Choix ×1,5, paralysie ×0,25 (×0,5 en Gen 7), niveaux.
#[test]
fn speed_rules() {
    let mut c = mon(50, &[NORMAL], [150, 100, 100, 100, 100, 100]);
    let side = SideState::default();
    assert_eq!(speed(4, &c, &side, Weather::None), 100);
    let par = SideState { status: Status::Paralysis, ..side.clone() };
    assert_eq!(speed(4, &c, &par, Weather::None), 25);
    assert_eq!(speed(7, &c, &par, Weather::None), 50);
    let plus2 = SideState { boosts: [0, 0, 0, 0, 0, 2], ..side.clone() };
    assert_eq!(speed(5, &c, &plus2, Weather::None), 200);
    c.item = it::CHOICE_SCARF;
    assert_eq!(speed(5, &c, &side, Weather::None), 150);
    c.item = 0;
    c.ability = ab::SWIFT_SWIM;
    assert_eq!(speed(5, &c, &side, Weather::Rain), 200);
}

/// Puissance Cachée : IV tous à 31 → Ténèbres, 70 de puissance en Gen 4-5, 60 ensuite.
#[test]
fn hidden_power_values() {
    assert_eq!(calc::hidden_power(4, [31; 6]), (DARK, 70));
    assert_eq!(calc::hidden_power(6, [31; 6]), (DARK, 60));
    // Feu : 31/30/31/30/31/30 (PV, Att, Déf, Atq Spé, Déf Spé, Vit) ; bits PV, Déf, Déf Spé → 37 × 15 / 63 = 8 → Feu.
    assert_eq!(calc::hidden_power(4, [31, 30, 31, 30, 31, 30]).0, FIRE);
}

/// Nombre de coups pour K.O. et probabilité.
#[test]
fn ko_counts() {
    let flat = |v: u16| Damage::Rolls { hits: vec![[v; 16]] };
    let ko = ko_info(&flat(50), 100, false);
    assert_eq!((ko.hits, ko.guaranteed), (2, Some(2)));
    assert_eq!(ko.label, "K.O. assuré en 2 coups");
    // Jets de 43 à 58 sur 100 PV : 2 coups possibles (jets ≥ 50 en moyenne), 3 assurés.
    let mut r = [0u16; 16];
    for (i, x) in r.iter_mut().enumerate() {
        *x = 43 + i as u16;
    }
    let ko = ko_info(&Damage::Rolls { hits: vec![r] }, 100, false);
    assert_eq!((ko.hits, ko.guaranteed), (2, Some(3)));
    assert!(ko.chance > 0.4 && ko.chance < 0.6, "{}", ko.chance);
    // Ceinture Force : un coup de 200 sur 100 PV ne met plus K.O. en un coup.
    let ko = ko_info(&flat(200), 100, true);
    assert_eq!(ko.hits, 2);
    assert_eq!(ko_info(&Damage::None { reason: String::new() }, 100, false).hits, 0);
}

/// PID des dresseurs de Platine : sans tirage (classe 0), le PID est (graine << 8) + 136.
#[test]
fn gen4_trainer_pid() {
    assert_eq!(trainers::gen4_pid(0, 5, 1, 1, 0, false), (7 << 8) + 136);
    // Un tirage : graine 7 → 7 × 1103515245 + 24691 ; on garde les 16 bits de poids fort.
    let state = 7u32.wrapping_mul(1_103_515_245).wrapping_add(24_691);
    assert_eq!(trainers::gen4_pid(0, 5, 1, 1, 1, true), ((state >> 16) << 8) + 120);
}

/// Les identifiants utilisés correspondent bien aux noms français attendus.
#[test]
fn ids_match_names() {
    use crate::dex::{ability_name, item_name, move_name};
    assert_eq!(ability_name(ab::INTIMIDATE), Some("Intimidation"));
    assert_eq!(ability_name(ab::ADAPTABILITY), Some("Adaptabilité"));
    assert_eq!(ability_name(ab::LEVITATE), Some("Lévitation"));
    assert_eq!(ability_name(ab::MULTISCALE), Some("Multiécaille"));
    assert_eq!(ability_name(ab::PIXILATE), Some("Peau Féérique"));
    assert_eq!(ability_name(ab::THICK_FAT), Some("Isograisse"));
    assert_eq!(ability_name(ab::FLASH_FIRE), Some("Torche"));
    assert_eq!(ability_name(ab::PURE_POWER), Some("Force Pure"));
    assert_eq!(ability_name(ab::GUTS), Some("Cran"));
    assert_eq!(item_name(it::CHOICE_BAND), Some("Bandeau Choix"));
    assert_eq!(item_name(it::CHOICE_SPECS), Some("Lunettes Choix"));
    assert_eq!(item_name(it::LIFE_ORB), Some("Orbe Vie"));
    assert_eq!(item_name(it::EXPERT_BELT), Some("Ceinture Pro"));
    assert_eq!(item_name(it::LEFTOVERS), Some("Restes"));
    assert_eq!(item_name(251), Some("Mouchoir Soie"));
    assert_eq!(it::boosted_type(251), Some(NORMAL));
    assert_eq!(move_name(super::ids::moves::SEISMIC_TOSS), Some("Frappe Atlas"));
    assert_eq!(move_name(super::ids::moves::DRAGON_RAGE), Some("Draco-Rage"));
    assert_eq!(move_name(super::ids::moves::SUPER_FANG), Some("Croc Fatal"));
    assert_eq!(move_name(EARTHQUAKE), Some("Séisme"));
}

/// Matrice : un Pokémon qui met K.O. en un coup en étant plus rapide gagne.
#[test]
fn matrix_verdicts() {
    let mut fast = mon(50, &[GROUND], [150, 250, 100, 100, 100, 150]);
    fast.moves = [EARTHQUAKE, 0, 0, 0];
    let mut slow = mon(50, &[ELECTRIC], [100, 50, 60, 50, 60, 50]);
    slow.moves = [THUNDERBOLT, 0, 0, 0];
    let side = SideState::default();
    let m = matrix(Game::Pt, &[fast.clone()], &side, &[slow.clone()], &side, &Field::default());
    assert_eq!(m[0][0].verdict, Verdict::Win);
    assert!(m[0][0].theirs.is_none(), "Tonnerre n'a aucun effet sur Sol");
    let m = matrix(Game::Pt, &[slow], &side, &[fast], &side, &Field::default());
    assert_eq!(m[0][0].verdict, Verdict::Lose);
}

// ---------- Comparaison directe avec Showdown ----------

/// Un cas de référence : statistiques réelles relevées dans @smogon/calc (niveau 50),
/// et dégâts minimum / maximum du premier coup renvoyés par Showdown.
struct Case {
    game: Game,
    att: (&'static [u8], [u16; 6], u16, u16),
    att_side: SideState,
    def: (&'static [u8], [u16; 6], u16, u16),
    mv: u16,
    weather: Weather,
    crit: bool,
    expected: (u16, u16),
}

fn case(game: Game, att: (&'static [u8], [u16; 6], u16, u16), def: (&'static [u8], [u16; 6], u16, u16), mv: u16, expected: (u16, u16)) -> Case {
    Case { game, att, att_side: SideState::default(), def, mv, weather: Weather::None, crit: false, expected }
}

/// Valeurs obtenues avec @smogon/calc 0.10 (`calculate(gen, attaquant, défenseur, attaque)`),
/// talents et objets précisés, les autres par défaut (sans effet sur ces calculs).
#[test]
fn matches_showdown() {
    const GARCHOMP: [u16; 6] = [183, 200, 115, 90, 105, 122];
    const ELECTIVIRE: [u16; 6] = [150, 143, 87, 115, 105, 115];
    let burned = SideState { status: Status::Burn, ..SideState::default() };
    let cases = [
        // Gen 4
        case(Game::Pt, (&[DRAGON, GROUND], GARCHOMP, 0, 0), (&[ELECTRIC], ELECTIVIRE, 0, 0), EARTHQUAKE, (260, 308)),
        case(
            Game::Pt,
            (&[FIGHTING, STEEL], [145, 117, 90, 183, 90, 110], 0, it::LIFE_ORB),
            (&[ROCK, DARK], [207, 154, 130, 115, 121, 81], 0, 0),
            396,
            (400, 472),
        ),
        case(
            Game::Pt,
            (&[BUG, STEEL], [145, 200, 120, 67, 100, 85], ab::TECHNICIAN, it::CHOICE_BAND),
            (&[GHOST, POISON], [135, 85, 80, 150, 95, 130], 0, 0),
            BULLET_PUNCH,
            (127, 151),
        ),
        Case {
            att_side: SideState { hp_percent: 20, ..SideState::default() },
            ..case(
                Game::Pt,
                (&[FIRE, FIGHTING], [151, 156, 91, 111, 91, 140], ab::BLAZE, 0),
                (&[STEEL, PSYCHIC], [142, 109, 136, 99, 136, 53], ab::HEATPROOF, 0),
                394,
                (116, 140),
            )
        },
        case(
            Game::Pt,
            (&[DRAGON, FLYING], [166, 186, 115, 120, 120, 100], 0, it::EXPERT_BELT),
            (&[GROUND, FLYING], [150, 115, 145, 65, 95, 115], 0, 0),
            200,
            (87, 103),
        ),
        Case {
            att_side: burned.clone(),
            ..case(
                Game::Pt,
                (&[NORMAL], [235, 162, 85, 85, 130, 50], 0, 0),
                (&[WATER, GROUND], [175, 130, 110, 105, 110, 80], 0, 0),
                BODY_SLAM,
                (36, 43),
            )
        },
        Case {
            weather: Weather::Rain,
            ..case(
                Game::Pt,
                (&[WATER, PSYCHIC], [135, 95, 105, 152, 105, 135], 0, 0),
                (&[FIRE, STEEL], [166, 110, 126, 150, 126, 97], 0, 185),
                57,
                (97, 115),
            )
        },
        // Gen 5
        case(Game::BW, (&[DRAGON, GROUND], GARCHOMP, 0, 0), (&[ELECTRIC], ELECTIVIRE, 0, 0), EARTHQUAKE, (260, 308)),
        Case { crit: true, ..case(Game::BW, (&[DRAGON, GROUND], GARCHOMP, 0, 0), (&[ELECTRIC], ELECTIVIRE, 0, 0), EARTHQUAKE, (524, 618)) },
        Case {
            weather: Weather::Sand,
            ..case(
                Game::BW,
                (&[FIGHTING, STEEL], [145, 117, 90, 183, 90, 110], 0, it::LIFE_ORB),
                (&[ROCK, DARK], [207, 154, 130, 115, 121, 81], 0, 0),
                396,
                (270, 328),
            )
        },
        case(
            Game::BW,
            (&[BUG, STEEL], [145, 200, 120, 67, 100, 85], ab::TECHNICIAN, it::CHOICE_BAND),
            (&[GHOST, POISON], [135, 85, 80, 150, 95, 130], 0, 0),
            BULLET_PUNCH,
            (127, 151),
        ),
        case(
            Game::BW,
            (&[DARK, ICE], [145, 172, 85, 65, 105, 145], 0, 0),
            (&[DRAGON, FLYING], [166, 154, 115, 120, 120, 100], ab::MULTISCALE, 0),
            420,
            (68, 84),
        ),
        case(
            Game::BW,
            (&[GHOST, FIRE], [135, 75, 110, 197, 110, 100], 0, it::CHOICE_SPECS),
            (&[GRASS, STEEL], [149, 114, 151, 74, 136, 40], 0, 184),
            315,
            (342, 404),
        ),
        Case {
            att_side: burned,
            ..case(
                Game::BW,
                (&[FIGHTING], [180, 192, 115, 75, 85, 65], ab::GUTS, 0),
                (&[FIRE, STEEL], [166, 110, 126, 150, 126, 97], 0, 0),
                409,
                (194, 230),
            )
        },
        case(
            Game::BW,
            (&[GHOST, POISON], [135, 85, 80, 182, 95, 130], 0, 0),
            (&[STEEL, PSYCHIC], [142, 109, 136, 99, 136, 53], 0, 0),
            SHADOW_BALL,
            (61, 73),
        ),
        // Gen 6
        case(Game::XY, (&[FAIRY], [170, 85, 85, 162, 150, 80], ab::PIXILATE, 0), (&[DRAGON, GROUND], GARCHOMP, 0, 0), HYPER_VOICE, (204, 242)),
        case(
            Game::XY,
            (&[GHOST, POISON], [135, 85, 80, 182, 95, 130], 0, 0),
            (&[STEEL, PSYCHIC], [142, 109, 136, 99, 136, 53], 0, 0),
            SHADOW_BALL,
            (122, 146),
        ),
        Case { crit: true, ..case(Game::XY, (&[DRAGON, GROUND], GARCHOMP, 0, 0), (&[ELECTRIC], ELECTIVIRE, 0, 0), EARTHQUAKE, (390, 462)) },
        case(
            Game::XY,
            (&[GRASS, FIGHTING], [135, 182, 100, 80, 80, 90], ab::TECHNICIAN, 0),
            (&[WATER, PSYCHIC], [135, 95, 105, 120, 105, 135], 0, 0),
            331,
            (74, 90),
        ),
        // Gen 7
        case(Game::SM, (&[FAIRY], [170, 85, 85, 162, 150, 80], ab::PIXILATE, 0), (&[DRAGON, GROUND], GARCHOMP, 0, 0), HYPER_VOICE, (188, 224)),
        case(Game::SM, (&[FIGHTING], [165, 182, 100, 85, 105, 75], 0, 0), (&[NORMAL], [235, 130, 85, 85, 130, 50], 0, 0), 69, (50, 50)),
    ];
    for (i, c) in cases.iter().enumerate() {
        let make = |(types, stats, ability, item): (&[u8], [u16; 6], u16, u16)| Combatant { ability, item, ..mon(50, types, stats) };
        let (a, d) = (make(c.att), make(c.def));
        let field = Field { weather: c.weather, ..Field::default() };
        let r = calculate(c.game, &a, &c.att_side, &d, &SideState::default(), c.mv, &field, c.crit).unwrap();
        let got = match r.damage {
            Damage::Rolls { hits } => (hits[0][0], hits[0][15]),
            Damage::Fixed { amount } => (amount, amount),
            Damage::None { reason } => panic!("cas {i} : {reason}"),
        };
        assert_eq!(got, c.expected, "cas {i} ({:?}, attaque {})", c.game, c.mv);
    }
}

/// Probabilités de K.O. de Showdown : 68,8 % d'OHKO (Pisto-Poing sur Ectoplasma, 135 PV)
/// et 9,8 % de 2HKO (Ball'Ombre Gen 5 sur Archéodong, 142 PV).
#[test]
fn ko_chance_matches_showdown() {
    let side = SideState::default();
    let scizor = Combatant { ability: ab::TECHNICIAN, item: it::CHOICE_BAND, ..mon(50, &[BUG, STEEL], [145, 200, 120, 67, 100, 85]) };
    let gengar = mon(50, &[GHOST, POISON], [135, 85, 80, 150, 95, 130]);
    let c = calculate(Game::BW, &scizor, &side, &gengar, &side, BULLET_PUNCH, &Field::default(), false).unwrap();
    let ko = ko_info(&c.damage, 135, false);
    assert_eq!((ko.hits, (ko.chance * 1000.0).round() as u32), (1, 688));
    let gengar = mon(50, &[GHOST, POISON], [135, 85, 80, 182, 95, 130]);
    let bronzong = mon(50, &[STEEL, PSYCHIC], [142, 109, 136, 99, 136, 53]);
    let c = calculate(Game::BW, &gengar, &side, &bronzong, &side, SHADOW_BALL, &Field::default(), false).unwrap();
    let ko = ko_info(&c.damage, 142, false);
    assert_eq!((ko.hits, (ko.chance * 1000.0).round() as u32, ko.guaranteed), (2, 98, Some(3)));
}

// ---------- ROM réelles (ignorées si absentes) ----------

fn rom(name: &str) -> Option<std::path::PathBuf> {
    let p = crate::test_rom_path(name);
    p.exists().then_some(p)
}

/// Platine : Pierrick (Roark), premier champion — Racaillou 12, Onix 12, Kranidos 14.
#[test]
fn platinum_roark() {
    let Some(path) = rom("Pokemon - Platinum Version (Europe).nds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let roark = r.trainer(246).unwrap();
    assert_eq!(roark.role, Some(trainers::Role::Gym));
    assert_eq!(roark.name, "Roark");
    let team: Vec<(u16, u16)> = roark.team.iter().map(|p| (p.species, p.level)).collect();
    assert_eq!(team, [(74, 12), (95, 12), (408, 14)]);
    let built = r.team(246).unwrap();
    assert_eq!(built[2].name, "Kranidos");
    assert!(built[2].types.contains(&ROCK));
    assert!(built.iter().all(|c| c.moves.iter().any(|&m| m != 0)));
    // Premier dresseur important dans l'ordre de l'histoire : le rival (niveau 5).
    let list = r.summaries();
    assert_eq!(list[0].role, Some(trainers::Role::Rival));
    assert!(list.iter().take_while(|s| s.role.is_some()).any(|s| s.name == "Cynthia"));
}

/// Équipe de la sauvegarde de démonstration (Platine) contre Pierrick : la matrice se calcule.
#[test]
fn demo_save_against_roark() {
    let session = crate::save::session::SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
    let game = session.game();
    let party = session.view().unwrap().party;
    let mine: Vec<Combatant> = party.iter().filter_map(|v| party::from_slot(game, v, None)).collect();
    assert!(!mine.is_empty());
    assert!(mine.iter().all(|c| c.stats[0] > 0 && !c.types.is_empty()));
    let Some(path) = rom("Pokemon - Platinum Version (Europe).nds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    assert_eq!(crate::dex::Game::from(r.game), game);
    let theirs = r.team(246).unwrap();
    let side = SideState::default();
    let m = matrix(game, &mine, &side, &theirs, &side, &Field::default());
    assert_eq!((m.len(), m[0].len()), (mine.len(), 3));
    let d = duel(game, &theirs[2], &side, &mine[0], &side, &Field::default());
    assert!(!d.moves.is_empty());
}

/// Blanche : premier champion (Rachid / Armando / Noa selon le starter) au niveau 14.
#[test]
fn white_first_gym() {
    let Some(path) = rom("Pokemon - Version Blanche (France) (NDSi Enhanced).nds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let armando = r.trainer(11).unwrap();
    assert_eq!((armando.name.as_str(), armando.role), ("Armando", Some(trainers::Role::Gym)));
    let team: Vec<(u16, u16)> = armando.team.iter().map(|p| (p.species, p.level)).collect();
    assert_eq!(team, [(506, 12), (513, 14)]); // Ponchiot, Flamajou
    let built = r.team(11).unwrap();
    assert_eq!(built[1].types, vec![FIRE]);
    let names: Vec<&str> = r.trainers.iter().filter(|t| t.role == Some(trainers::Role::EliteFour)).map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Anis") && names.contains(&"Percila"));
}

/// Rubis Oméga : Roxanne (premier champion) et Pierre (Maître).
#[test]
fn omega_ruby_trainers() {
    let Some(path) = rom("Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let roxanne = r.trainer(561).unwrap();
    assert_eq!((roxanne.name.as_str(), roxanne.role), ("Roxanne", Some(trainers::Role::Gym)));
    let champion = r.trainer(557).unwrap();
    assert_eq!(champion.role, Some(trainers::Role::Champion));
    assert_eq!(r.team(561).unwrap().len(), 2);
}

/// Pokémon Y : fiches de 0x14 octets — Violette (première championne) et Dianthéa.
#[test]
fn pokemon_y_trainers() {
    let Some(path) = rom("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let viola = r.trainer(6).unwrap();
    assert_eq!((viola.name.as_str(), viola.class_name.as_str()), ("Violette", "Championne"));
    let team: Vec<(u16, u16)> = viola.team.iter().map(|p| (p.species, p.level)).collect();
    assert_eq!(team, [(283, 10), (666, 12)]); // Arakdo, Prismillon
    assert_eq!(r.trainer(276).unwrap().team.len(), 6);
}

/// Pokémon Y : rôles (Kalem rival, Conseil 4, Lysandre) et combats en duo (Jumelles).
#[test]
fn pokemon_y_roles_and_doubles() {
    let Some(path) = rom("Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let role = |id: u16| r.trainer(id).unwrap().role;
    assert_eq!(role(6), Some(trainers::Role::Gym));
    assert_eq!(role(26), Some(trainers::Role::Gym)); // Urup
    assert_eq!(role(187), Some(trainers::Role::EliteFour)); // Thyméo
    assert_eq!(role(276), Some(trainers::Role::Champion));
    assert_eq!((r.trainer(130).unwrap().name.as_str(), role(130)), ("Kalem", Some(trainers::Role::Rival)));
    assert_eq!(role(526), Some(trainers::Role::Boss)); // Lysandre
    assert_eq!(role(175), Some(trainers::Role::Admin)); // Ancolie
    let twins = r.trainer(88).unwrap();
    assert_eq!((twins.name.as_str(), twins.double), ("Eva & Lyn", true));
    assert!(!r.trainer(6).unwrap().double);
    let first_gym = r.summaries().into_iter().find(|s| s.role == Some(trainers::Role::Gym)).unwrap();
    assert_eq!(first_gym.name, "Violette");
}

/// Lune : fiches Gen 7 (nature, IV et EV exacts), Tili, Pectorius, Conseil 4 et Euphorbe.
#[test]
fn moon_trainers() {
    let Some(path) = rom("Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    assert!(r.verified);
    let hala = r.trainer(23).unwrap();
    assert_eq!((hala.class_name.as_str(), hala.name.as_str(), hala.role), ("Doyen", "Pectorius", Some(trainers::Role::Gym)));
    let team: Vec<(u16, u16)> = hala.team.iter().map(|p| (p.species, p.level)).collect();
    assert_eq!(team, [(56, 14), (296, 14), (739, 15)]); // Férosinge, Makuhita, Crabagarre
    assert_eq!(hala.exact.len(), 3);
    let built = r.team(23).unwrap();
    assert_eq!(built[2].types, vec![FIGHTING]);
    assert!(built.iter().all(|c| c.notes.is_empty()), "Gen 7 : nature et attaques connues");
    assert_eq!(built[0].ivs, hala.exact[0].ivs);
    let hau = r.trainer(6).unwrap();
    assert_eq!((hau.name.as_str(), hau.role), ("Tili", Some(trainers::Role::Rival)));
    assert_eq!(r.trainer(52).unwrap().name, "Althéo");
    for id in [149, 152, 153, 156] {
        assert_eq!(r.trainer(id).unwrap().role, Some(trainers::Role::EliteFour), "n°{id}");
    }
    let kukui = r.trainer(129).unwrap();
    assert_eq!((kukui.name.as_str(), kukui.role, kukui.team.len()), ("Euphorbe", Some(trainers::Role::Champion), 6));
    assert_eq!(r.trainer(138).unwrap().role, Some(trainers::Role::Boss)); // Guzma
    let list = r.summaries();
    assert_eq!(list[0].role, Some(trainers::Role::Rival));
    assert_eq!(list.iter().find(|s| s.id == 23).unwrap().role_label, Some("Capitaine / Doyen"));
}

/// Ultra-Soleil : Tili est le dernier adversaire de la Ligue, Molène au Conseil 4.
#[test]
fn ultra_sun_trainers() {
    let Some(path) = rom("Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    let ilima = r.trainer(52).unwrap();
    assert_eq!((ilima.name.as_str(), ilima.role), ("Althéo", Some(trainers::Role::Gym)));
    assert_eq!(ilima.team.iter().map(|p| p.level).max(), Some(11));
    let kiawe = r.trainer(504).unwrap();
    assert_eq!((kiawe.name.as_str(), kiawe.role), ("Kiawe", Some(trainers::Role::Gym)));
    let hau = r.trainer(494).unwrap();
    assert_eq!((hau.name.as_str(), hau.role), ("Tili", Some(trainers::Role::Champion)));
    assert_eq!(r.trainer(489).unwrap().role, Some(trainers::Role::EliteFour)); // Molène
    assert_eq!(r.trainer(497).unwrap().name, "Paulie");
    assert_eq!(r.team(497).unwrap().len(), 4);
}

/// Sauvegarde Soleil / Lune (PKHeX) contre les dresseurs de Lune : de bout en bout.
#[test]
fn sun_moon_save_against_hala() {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/pkhex/sm_project_802.main")).unwrap();
    let session = crate::save::session::SaveSession::open(&bytes).unwrap();
    let game = session.game();
    let Some(path) = rom("Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko).3ds") else {
        return;
    };
    let r = trainers::RomTrainers::open(&path).unwrap();
    assert_eq!(crate::dex::Game::from(r.game), game);
    let party = session.view().unwrap().party;
    let mine: Vec<Combatant> = party.iter().filter_map(|v| party::from_slot(game, v, Some(&r))).collect();
    assert!(!mine.is_empty());
    let theirs = r.team(23).unwrap();
    let side = SideState::default();
    let m = matrix(game, &mine, &side, &theirs, &side, &Field::default());
    assert_eq!((m.len(), m[0].len()), (mine.len(), 3));
    let d = duel(game, &theirs[2], &side, &mine[0], &side, &Field::default());
    assert!(!d.moves.is_empty());
}
