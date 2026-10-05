//! Identifiants des talents, objets et attaques utiles au calcul (numérotation
//! nationale, identique de la Gen 4 à la Gen 7 pour ce qui existe déjà).
//! Les noms français sont vérifiés dans les tests (`battle::tests::ids_match_names`).

/// Talents.
pub mod ability {
    pub const DRIZZLE: u16 = 2;
    pub const BATTLE_ARMOR: u16 = 4;
    pub const STURDY: u16 = 5;
    pub const VOLT_ABSORB: u16 = 10;
    pub const WATER_ABSORB: u16 = 11;
    pub const CLOUD_NINE: u16 = 13;
    pub const FLASH_FIRE: u16 = 18;
    pub const INTIMIDATE: u16 = 22;
    pub const WONDER_GUARD: u16 = 25;
    pub const LEVITATE: u16 = 26;
    pub const CLEAR_BODY: u16 = 29;
    pub const LIGHTNING_ROD: u16 = 31;
    pub const SWIFT_SWIM: u16 = 33;
    pub const CHLOROPHYLL: u16 = 34;
    pub const HUGE_POWER: u16 = 37;
    pub const SOUNDPROOF: u16 = 43;
    pub const SAND_STREAM: u16 = 45;
    pub const THICK_FAT: u16 = 47;
    pub const HYPER_CUTTER: u16 = 52;
    pub const HUSTLE: u16 = 55;
    pub const GUTS: u16 = 62;
    pub const MARVEL_SCALE: u16 = 63;
    pub const OVERGROW: u16 = 65;
    pub const BLAZE: u16 = 66;
    pub const TORRENT: u16 = 67;
    pub const SWARM: u16 = 68;
    pub const DROUGHT: u16 = 70;
    pub const WHITE_SMOKE: u16 = 73;
    pub const PURE_POWER: u16 = 74;
    pub const SHELL_ARMOR: u16 = 75;
    pub const AIR_LOCK: u16 = 76;
    pub const MOTOR_DRIVE: u16 = 78;
    pub const HEATPROOF: u16 = 85;
    pub const SIMPLE: u16 = 86;
    pub const DRY_SKIN: u16 = 87;
    pub const IRON_FIST: u16 = 89;
    pub const ADAPTABILITY: u16 = 91;
    pub const SKILL_LINK: u16 = 92;
    pub const SOLAR_POWER: u16 = 94;
    pub const QUICK_FEET: u16 = 95;
    pub const NORMALIZE: u16 = 96;
    pub const SNIPER: u16 = 97;
    pub const TECHNICIAN: u16 = 101;
    pub const MOLD_BREAKER: u16 = 104;
    pub const UNAWARE: u16 = 109;
    pub const TINTED_LENS: u16 = 110;
    pub const FILTER: u16 = 111;
    pub const SCRAPPY: u16 = 113;
    pub const STORM_DRAIN: u16 = 114;
    pub const SOLID_ROCK: u16 = 116;
    pub const SNOW_WARNING: u16 = 117;
    pub const RECKLESS: u16 = 120;
    pub const CONTRARY: u16 = 126;
    pub const DEFIANT: u16 = 128;
    pub const DEFEATIST: u16 = 129;
    pub const MULTISCALE: u16 = 136;
    pub const SAND_RUSH: u16 = 146;
    pub const ANALYTIC: u16 = 148;
    pub const SAP_SIPPER: u16 = 157;
    pub const SAND_FORCE: u16 = 159;
    pub const TURBOBLAZE: u16 = 163;
    pub const TERAVOLT: u16 = 164;
    pub const FUR_COAT: u16 = 169;
    pub const BULLETPROOF: u16 = 171;
    pub const COMPETITIVE: u16 = 172;
    pub const STRONG_JAW: u16 = 173;
    pub const REFRIGERATE: u16 = 174;
    pub const MEGA_LAUNCHER: u16 = 178;
    pub const PIXILATE: u16 = 182;
    pub const AERILATE: u16 = 184;
    pub const SLUSH_RUSH: u16 = 202;
    pub const GALVANIZE: u16 = 206;
    pub const FLUFFY: u16 = 218;
    pub const FULL_METAL_BODY: u16 = 230;
    pub const SHADOW_SHIELD: u16 = 231;
    pub const PRISM_ARMOR: u16 = 232;
    pub const NEUROFORCE: u16 = 233;
}

/// Objets.
pub mod item {
    pub const GRISEOUS_ORB: u16 = 112;
    pub const ADAMANT_ORB: u16 = 135;
    pub const LUSTROUS_ORB: u16 = 136;
    pub const SITRUS_BERRY: u16 = 158;
    pub const MACHO_BRACE: u16 = 215;
    pub const CHOICE_BAND: u16 = 220;
    pub const SOUL_DEW: u16 = 225;
    pub const DEEP_SEA_TOOTH: u16 = 226;
    pub const DEEP_SEA_SCALE: u16 = 227;
    pub const LEFTOVERS: u16 = 234;
    pub const LIGHT_BALL: u16 = 236;
    pub const METAL_POWDER: u16 = 257;
    pub const THICK_CLUB: u16 = 258;
    pub const MUSCLE_BAND: u16 = 266;
    pub const WISE_GLASSES: u16 = 267;
    pub const EXPERT_BELT: u16 = 268;
    pub const LIFE_ORB: u16 = 270;
    pub const FOCUS_SASH: u16 = 275;
    pub const IRON_BALL: u16 = 278;
    pub const BLACK_SLUDGE: u16 = 281;
    pub const CHOICE_SCARF: u16 = 287;
    /// Poids Puissance … Chaîne Puissance (289-294) : divisent la Vitesse par deux.
    pub const POWER_ITEMS: std::ops::RangeInclusive<u16> = 289..=294;
    pub const CHOICE_SPECS: u16 = 297;
    pub const AIR_BALLOON: u16 = 541;
    pub const ASSAULT_VEST: u16 = 640;

    use crate::battle::types::*;

    /// Objets qui augmentent de 20 % la puissance d'un type (Mouchoir Soie, Plaques, Encens…).
    pub fn boosted_type(item: u16) -> Option<u8> {
        Some(match item {
            251 => NORMAL,                  // Mouchoir Soie
            241 | 303 => FIGHTING,          // Ceinture Noire, Plaque Poing
            244 | 306 => FLYING,            // Bec Pointu, Plaque Ciel
            245 | 304 => POISON,            // Pic Venin, Plaque Toxicité
            237 | 305 => GROUND,            // Sable Doux, Plaque Terre
            238 | 309 | 315 => ROCK,        // Pierre Dure, Plaque Roc, Encens Roc
            222 | 308 => BUG,               // Poudre Argentée, Plaque Insecte
            247 | 310 => GHOST,             // Rune Sort, Plaque Fantôme
            233 | 313 => STEEL,             // Peau Métal, Plaque Fer
            249 | 298 => FIRE,              // Charbon, Plaque Flamme
            243 | 299 | 254 | 317 => WATER, // Eau Mystique, Plaque Hydro, Encens Mer, Encens Vague
            239 | 301 | 318 => GRASS,       // Grain Miracle, Plaque Herbe, Encens Fleur
            242 | 300 => ELECTRIC,          // Aimant, Plaque Volt
            248 | 307 | 314 => PSYCHIC,     // Cuiller Tordue, Plaque Esprit, Encens Bizarre
            246 | 302 => ICE,               // Glace Éternelle, Plaque Glace
            250 | 311 => DRAGON,            // Croc Dragon, Plaque Draco
            240 | 312 => DARK,              // Lunettes Noires, Plaque Ombre
            644 => FAIRY,                   // Plaque Pixie
            _ => return None,
        })
    }

    /// Type d'une Plaque (pour Jugement).
    pub fn plate_type(item: u16) -> Option<u8> {
        if (298..=313).contains(&item) || item == 644 {
            boosted_type(item)
        } else {
            None
        }
    }

    /// Joyaux (Gen 5+) : type renforcé.
    pub fn gem_type(item: u16) -> Option<u8> {
        Some(match item {
            548 => FIRE,
            549 => WATER,
            550 => ELECTRIC,
            551 => GRASS,
            552 => ICE,
            553 => FIGHTING,
            554 => POISON,
            555 => GROUND,
            556 => FLYING,
            557 => PSYCHIC,
            558 => BUG,
            559 => ROCK,
            560 => GHOST,
            561 => DRAGON,
            562 => DARK,
            563 => STEEL,
            564 => NORMAL,
            _ => return None,
        })
    }

    /// Baies qui divisent par deux les dégâts d'une attaque super efficace (ou Normal pour la Baie Zalis).
    pub fn resist_berry_type(item: u16) -> Option<u8> {
        Some(match item {
            184 => FIRE,     // Baie Chocco
            185 => WATER,    // Baie Pocpoc
            186 => ELECTRIC, // Baie Parma
            187 => GRASS,    // Baie Ratam
            188 => ICE,      // Baie Nanone
            189 => FIGHTING, // Baie Pomroz
            190 => POISON,   // Baie Kébia
            191 => GROUND,   // Baie Jouca
            192 => FLYING,   // Baie Cobaba
            193 => PSYCHIC,  // Baie Yapap
            194 => BUG,      // Baie Panga
            195 => ROCK,     // Baie Charti
            196 => GHOST,    // Baie Sédra
            197 => DRAGON,   // Baie Fraigo
            198 => DARK,     // Baie Lampou
            199 => STEEL,    // Baie Babiri
            200 => NORMAL,   // Baie Zalis
            686 => FAIRY,    // Baie Selro
            _ => return None,
        })
    }
}

/// Attaques.
pub mod moves {
    pub const SOLAR_BEAM: u16 = 76;
    pub const SONIC_BOOM: u16 = 49;
    pub const LOW_KICK: u16 = 67;
    pub const SEISMIC_TOSS: u16 = 69;
    pub const DRAGON_RAGE: u16 = 82;
    pub const NIGHT_SHADE: u16 = 101;
    pub const SELF_DESTRUCT: u16 = 120;
    pub const PSYWAVE: u16 = 149;
    pub const EXPLOSION: u16 = 153;
    pub const SUPER_FANG: u16 = 162;
    pub const STRUGGLE: u16 = 165;
    pub const TRIPLE_KICK: u16 = 167;
    pub const FLAIL: u16 = 175;
    pub const REVERSAL: u16 = 179;
    pub const RETURN: u16 = 216;
    pub const FRUSTRATION: u16 = 218;
    pub const HIDDEN_POWER: u16 = 237;
    pub const FACADE: u16 = 263;
    pub const KNOCK_OFF: u16 = 282;
    pub const ENDEAVOR: u16 = 283;
    pub const ERUPTION: u16 = 284;
    pub const WEATHER_BALL: u16 = 311;
    pub const WATER_SPOUT: u16 = 323;
    pub const GYRO_BALL: u16 = 360;
    pub const BRINE: u16 = 362;
    pub const PAYBACK: u16 = 371;
    pub const WRING_OUT: u16 = 378;
    pub const PUNISHMENT: u16 = 386;
    pub const GRASS_KNOT: u16 = 447;
    pub const JUDGMENT: u16 = 449;
    pub const CRUSH_GRIP: u16 = 462;
    pub const PSYSHOCK: u16 = 473;
    pub const HEAVY_SLAM: u16 = 484;
    pub const ELECTRO_BALL: u16 = 486;
    pub const FOUL_PLAY: u16 = 492;
    pub const STORED_POWER: u16 = 500;
    pub const HEX: u16 = 506;
    pub const ACROBATICS: u16 = 512;
    pub const FINAL_GAMBIT: u16 = 515;
    pub const HEAT_CRASH: u16 = 535;
    pub const PSYSTRIKE: u16 = 540;
    pub const SECRET_SWORD: u16 = 548;
    pub const FLYING_PRESS: u16 = 560;
    pub const FREEZE_DRY: u16 = 573;
    pub const POWER_TRIP: u16 = 681;
    pub const NATURES_MADNESS: u16 = 717;

    /// Attaques qui mettent K.O. en un coup (Guillotine, Empal'Korne, Abîme, Glaciation).
    pub const OHKO: [u16; 4] = [12, 32, 90, 329];

    /// Attaques « poing » (Poing de Fer).
    pub const PUNCH: [u16; 17] = [4, 5, 7, 8, 9, 146, 183, 223, 264, 309, 325, 327, 359, 409, 418, 612, 665];
    /// Attaques à contrecoup ou à « chute » (Téméraire).
    pub const RECKLESS: [u16; 12] = [26, 36, 38, 66, 136, 344, 394, 413, 452, 457, 528, 543];
    /// Morsures (Prognathe).
    pub const BITE: [u16; 8] = [44, 158, 242, 305, 422, 423, 424, 706];
    /// Attaques « onde » (Méga Blaster).
    pub const PULSE: [u16; 5] = [352, 396, 399, 406, 618];
    /// Attaques sonores (Anti-Bruit).
    pub const SOUND: [u16; 12] = [253, 304, 405, 448, 496, 497, 547, 555, 574, 586, 664, 691];
    /// Attaques « balle » (Pare-Balles).
    pub const BULLET: [u16; 12] = [121, 140, 188, 247, 296, 311, 331, 350, 360, 396, 412, 486];

    /// Attaques qui frappent toujours deux fois.
    pub const TWO_HITS: [u16; 6] = [24, 41, 155, 458, 530, 544];
    /// Attaques qui frappent de 2 à 5 fois.
    pub const MULTI_HITS: [u16; 14] = [3, 4, 31, 42, 131, 140, 154, 198, 292, 331, 333, 350, 541, 594];
}
