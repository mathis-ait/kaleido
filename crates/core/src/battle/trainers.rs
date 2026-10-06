//! Dresseurs d'une ROM (d'origine ou randomisée) pour le calculateur : noms,
//! classes, équipes, et construction des Pokémon adverses comme le fait le jeu.
//!
//! Construction d'un Pokémon de dresseur :
//! - **IV** : `difficulté × 31 / 255` pour les six statistiques, 0 EV.
//! - **Attaques** : celles du dresseur s'il en a ; sinon les 4 dernières apprises
//!   par niveau jusqu'à son niveau, lues dans la ROM (comme le jeu).
//! - **Statistiques, types, talents** : fiches de la ROM (randomisées comprises).
//! - **Nature, talent (Gen 4)** : PID calculé comme dans Platine
//!   (pret/pokeplatinum, `TrainerData_BuildParty`) : graine = difficulté + niveau +
//!   espèce + numéro du dresseur, `classe` tirages du générateur congruentiel
//!   (×1103515245 + 24691), puis `PID = (tirage << 8) + 136` (120 si la classe
//!   est féminine). Nature = PID mod 25 ; le bit 0 du PID étant toujours nul, le
//!   talent est toujours le premier.
//! - **Nature (Gen 5/6)** : non reproduite (approximation : Hardi, neutre) ; talent
//!   d'après l'octet « genre/talent » (bits 4-5 : 1 = premier, 2 = second, 3 = caché).
//! - **Bonheur** : 255, 0 si le Pokémon connaît Frustration (règle de Platine).

use std::path::Path;

use serde::Serialize;

use super::{types, Combatant};
use crate::ctr_rom::CtrGameRom;
use crate::data::learnsets::{self, Learnset};
use crate::data::trainers::{read_team, read_team_with, TeamFormat, TrainerPokemon, FLAG_MOVES};
use crate::data::DataPaths;
use crate::dex::{self, PersonalInfo};
use crate::games::Game;
use crate::rom::{GameRom, RomError};
use crate::save::calc_stats;

/// Rôle d'un dresseur important.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Rival,
    Gym,
    Admin,
    Boss,
    EliteFour,
    Champion,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Rival => "Rival",
            Role::Gym => "Champion d'Arène",
            Role::Admin => "Admin / commandant",
            Role::Boss => "Chef d'équipe",
            Role::EliteFour => "Conseil 4",
            Role::Champion => "Maître de la Ligue",
        }
    }
}

/// Un dresseur de la ROM.
#[derive(Debug, Clone)]
pub struct Trainer {
    pub id: u16,
    pub class_id: u16,
    pub class_name: String,
    pub name: String,
    pub role: Option<Role>,
    pub double: bool,
    pub custom_moves: bool,
    pub team: Vec<TrainerPokemon>,
}

/// Un Pokémon de l'équipe, pour les cartes de l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSlot {
    pub species: u16,
    pub form: u16,
    pub level: u16,
    pub name: String,
}

/// Résumé d'un dresseur pour la liste.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerSummary {
    pub id: u16,
    pub class_name: String,
    pub name: String,
    pub role: Option<Role>,
    pub role_label: Option<&'static str>,
    pub double: bool,
    pub max_level: u16,
    pub team: Vec<TeamSlot>,
}

/// Données de combat lues dans une ROM.
pub struct RomTrainers {
    pub game: Game,
    /// Fiches « personal » brutes (une par espèce, puis les formes en Gen 5+).
    personal: Vec<Vec<u8>>,
    learnsets: Vec<Learnset>,
    pub trainers: Vec<Trainer>,
    /// Emplacements des données vérifiés sur une vraie ROM.
    pub verified: bool,
}

/// Fichiers de texte des noms et des classes de dresseurs (jeux DS).
fn nds_text(game: Game) -> Option<(usize, usize)> {
    Some(match game {
        Game::Diamond | Game::Pearl => (559, 560),
        Game::Platinum => (618, 619),
        // UPR-ZX `TrainerNamesTextOffset` / `TrainerClassesTextOffset` (non vérifié).
        Game::HeartGold | Game::SoulSilver => (729, 730),
        Game::Black | Game::White => (190, 191),
        Game::Black2 | Game::White2 => (382, 383),
        _ => return None,
    })
}

/// HeartGold / SoulSilver : rôle d'après le numéro du dresseur (`tagTrainersHGSS`
/// d'UPR-ZX, non vérifié sur une ROM ; les classes de HGSS ne sont pas reprises ici).
fn hgss_role(id: u16) -> Option<Role> {
    Some(match id {
        // Champions de Johto et de Kanto, puis leurs revanches.
        20 | 21 | 30..=35 | 253..=259 | 261 | 0x2C8..=0x2D7 => Role::Gym,
        245 | 246 | 247 | 418 | 0x2BE..=0x2C1 => Role::EliteFour,
        244 | 0x2BD => Role::Champion,
        // Silver : combats successifs (un dresseur par starter).
        1 | 0x107 | 0x108 | 0x10A..=0x110 | 0x11E..=0x121 | 0x1EA..=0x1EC | 0x1F0..=0x1F2 | 0x2E0..=0x2E2 => Role::Rival,
        _ => return None,
    })
}

/// Classes féminines de Platine (pret/pokeplatinum, `trainer_class_genders.h`) :
/// elles changent le PID, donc la nature.
const PT_FEMALE_CLASSES: [u16; 45] = [
    1, 3, 5, 7, 8, 10, 13, 17, 18, 21, 22, 25, 26, 30, 33, 35, 36, 40, 43, 45, 50, 54, 56, 61, 66, 69, 72, 74, 76, 77, 78, 80, 84, 85, 87, 88, 89,
    90, 92, 94, 96, 98, 99, 101, 104,
];

/// Rôle d'après la classe de dresseur (identifiants vérifiés sur les ROM :
/// Platine Europe, Blanche France, Rubis Oméga Europe). Les entrées sans nom
/// (dresseurs factices de la ROM) ne sont jamais importantes.
fn role_of(game: Game, class: u16, name: &str) -> Option<Role> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('{') {
        return None;
    }
    match game {
        // pret/pokeplatinum, `trainer_classes.h` ; mêmes classes en Diamant / Perle
        // (vérifié sur Diamant ADAF : champions, Conseil 4, Cynthia, Team Galaxie).
        Game::Platinum | Game::Diamond | Game::Pearl => Some(match class {
            62 | 64 | 74..=79 => Role::Gym,
            63 => Role::Rival,
            65..=68 => Role::EliteFour,
            69 => Role::Champion,
            72 | 87 | 88 => Role::Admin,
            86 => Role::Boss,
            _ => return None,
        }),
        // Classes de Noire/Blanche (texte 191).
        Game::Black | Game::White => Some(match class {
            10..=12 | 19..=23 | 54..=56 => Role::Gym,
            78..=81 => Role::EliteFour,
            // Goyah, puis Cynthia (après la Ligue).
            89 | 100 => Role::Champion,
            // Tcheren (37), Bianca (38), N (40, et 47 en tenue Team Plasma).
            37 | 38 | 40 | 47 => Role::Rival,
            // Ghetis (82) et N au château (101).
            82 | 101 => Role::Boss,
            _ => return None,
        }),
        // Classes de Noire 2 / Blanche 2 (texte 383, vérifié sur Noire 2 FR).
        Game::Black2 | Game::White2 => Some(match class {
            112..=119 => Role::Gym,
            78..=81 => Role::EliteFour,
            // Iris, puis Goyah et Cynthia (après la Ligue).
            193 | 89 | 100 => Role::Champion,
            // Matis (145), Bianca (38), N (40), Tcheren hors arène (197).
            145 | 38 | 40 | 197 => Role::Rival,
            // Ghetis (189), Nikolaï (186, et 235 après la Ligue).
            189 | 186 | 235 => Role::Boss,
            // Lilien (191) et les Ombres (192).
            191 | 192 => Role::Admin,
            _ => return None,
        }),
        // Classes de Rubis Oméga / Saphir Alpha (texte 21).
        Game::OmegaRuby | Game::AlphaSapphire => Some(match class {
            // Brice / Flora, Timmy.
            127 | 128 | 192 | 272 | 278 | 279 => Role::Rival,
            200..=207 => Role::Gym,
            194..=197 | 273..=276 => Role::EliteFour,
            198 | 277 => Role::Champion,
            // Arthur (Aqua), Max (Magma).
            174 | 178 => Role::Boss,
            175 | 180 | 182 | 186 | 187 => Role::Admin,
            _ => return None,
        }),
        _ => None,
    }
}

impl RomTrainers {
    /// Ouvre une ROM DS (.nds) ou 3DS (image ou dossier extrait).
    pub fn open(path: &Path) -> Result<Self, RomError> {
        let is_nds = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"));
        if is_nds {
            Self::from_nds(&GameRom::open(path)?)
        } else {
            Self::from_ctr(&CtrGameRom::open(path)?)
        }
    }

    pub fn from_nds(rom: &GameRom) -> Result<Self, RomError> {
        let game = rom.game;
        let unsupported = || RomError::Unsupported(format!("{} : dresseurs non pris en charge", game.name_fr()));
        let format = TeamFormat::for_game(game);
        let paths = DataPaths::for_game(game).ok_or_else(unsupported)?;
        let (names_file, classes_file) = nds_text(game).ok_or_else(unsupported)?;
        let generation = game.generation();
        let names = rom.text_file(names_file)?;
        let classes = rom.text_file(classes_file)?;
        let trdata = rom.narc(paths.trainer_data)?.files;
        let trpoke = rom.narc(paths.trainer_pokemon)?.files;
        let personal = rom.narc(rom.layout.personal)?.files;
        let learnsets = rom.narc(paths.learnsets)?.files.iter().map(|d| learnsets::read(generation, d)).collect();
        let mut trainers = Vec::new();
        for (i, (d, p)) in trdata.iter().zip(&trpoke).enumerate() {
            if i == 0 || d.len() < 4 {
                continue;
            }
            let Some(team) = read_team_with(format, d, p) else {
                continue;
            };
            if team.pokemon.is_empty() {
                continue;
            }
            let class_id = d[1] as u16;
            let hgss = matches!(game, Game::HeartGold | Game::SoulSilver);
            // Platine : type de combat (u32) en 0x10 ; Gen 5 : octet 2 (0 simple, 1 double, 2 triple, 3 rotatif).
            let double = if generation == 4 { d.get(0x10).is_some_and(|&b| b == 2) } else { d[2] != 0 };
            let name = names.get(i).cloned().unwrap_or_default();
            trainers.push(Trainer {
                id: i as u16,
                class_id,
                class_name: classes.get(class_id as usize).cloned().unwrap_or_default(),
                role: if hgss { hgss_role(i as u16).filter(|_| !name.trim().is_empty()) } else { role_of(game, class_id, &name) },
                name,
                double,
                custom_moves: team.flags & FLAG_MOVES != 0,
                team: team.pokemon,
            });
        }
        Ok(Self { game, personal, learnsets, trainers, verified: rom.layout.verified })
    }

    pub fn from_ctr(rom: &CtrGameRom) -> Result<Self, RomError> {
        let game = rom.game;
        if game.generation() != 6 {
            return Err(RomError::Unsupported(format!("{} : dresseurs non pris en charge (format Gen 7 non lu)", game.name_fr())));
        }
        let l = rom.layout;
        let entries = |path: &str| -> Result<Vec<Vec<u8>>, RomError> {
            let garc = rom.garc(path)?;
            Ok((0..garc.len()).map(|i| garc.file(i).map(<[u8]>::to_vec).unwrap_or_default()).collect())
        };
        let names = rom.text_file(l.trainer_names)?;
        let classes = rom.text_file(l.trainer_classes)?;
        let trdata = entries(l.trainer_data)?;
        let trpoke = entries(l.trainer_pokemon)?;
        let personal = entries(l.personal)?;
        // Gen 6 : couples (attaque u16, niveau u16) comme en Gen 5.
        let learnsets = entries(l.levelup)?.iter().map(|d| learnsets::read(5, d)).collect();
        let mut trainers = Vec::new();
        // X / Y : fiche de 0x14 octets, convertie au format de Rubis Oméga / Saphir Alpha.
        let xy = matches!(game, crate::games::Game::X | crate::games::Game::Y);
        for (i, (d, p)) in trdata.iter().zip(&trpoke).enumerate() {
            let converted;
            let d = if xy {
                converted = crate::data::trainers::xy_trdata_as_oras(d);
                &converted
            } else {
                d
            };
            if i == 0 || d.len() < 8 {
                continue;
            }
            let Some(team) = read_team(6, d, p) else {
                continue;
            };
            if team.pokemon.is_empty() {
                continue;
            }
            let class_id = u16::from_le_bytes([d[2], d[3]]);
            let name = names.get(i).cloned().unwrap_or_default();
            trainers.push(Trainer {
                id: i as u16,
                class_id,
                class_name: classes.get(class_id as usize).cloned().unwrap_or_default(),
                role: role_of(game, class_id, &name),
                name,
                // Type de combat non vérifié en Gen 6.
                double: false,
                custom_moves: team.flags & FLAG_MOVES != 0,
                team: team.pokemon,
            });
        }
        Ok(Self { game, personal, learnsets, trainers, verified: l.verified })
    }

    pub fn generation(&self) -> u8 {
        self.game.generation()
    }

    /// Jeu des données hors ligne (attaques, noms).
    pub fn dex_game(&self) -> dex::Game {
        self.game.into()
    }

    pub fn trainer(&self, id: u16) -> Option<&Trainer> {
        self.trainers.iter().find(|t| t.id == id)
    }

    /// Résumés de tous les dresseurs ; les importants d'abord, dans l'ordre de
    /// l'histoire (estimé par le niveau maximal de l'équipe), puis les autres par numéro.
    pub fn summaries(&self) -> Vec<TrainerSummary> {
        let mut list: Vec<TrainerSummary> = self
            .trainers
            .iter()
            .map(|t| TrainerSummary {
                id: t.id,
                class_name: t.class_name.clone(),
                name: display_name(t),
                role: t.role,
                role_label: t.role.map(Role::label),
                double: t.double,
                max_level: t.team.iter().map(|p| p.level).max().unwrap_or(0),
                team: t
                    .team
                    .iter()
                    .map(|p| TeamSlot {
                        species: p.species,
                        form: p.form,
                        level: p.level,
                        name: dex::species_name(p.species).unwrap_or("?").to_string(),
                    })
                    .collect(),
            })
            .collect();
        list.sort_by_key(|s| (s.role.is_none(), if s.role.is_some() { s.max_level } else { 0 }, s.id));
        list
    }

    /// Fiche « personal » d'une espèce (et de sa forme en Gen 5+).
    fn personal_of(&self, species: u16, form: u16) -> Option<(PersonalInfo, usize)> {
        let dg = self.dex_game();
        let base = PersonalInfo::from_rom(dg, self.personal.get(species as usize)?)?;
        let index = if self.generation() >= 5 && form > 0 { base.record_index(species, form as u8) } else { species as usize };
        if index != species as usize {
            if let Some(f) = self.personal.get(index).and_then(|d| PersonalInfo::from_rom(dg, d)) {
                return Some((f, index));
            }
        }
        Some((base, species as usize))
    }

    /// Types (canoniques) d'une espèce d'après la ROM (types randomisés compris).
    pub fn species_types(&self, species: u16, form: u16) -> Option<Vec<u8>> {
        let (info, _) = self.personal_of(species, form)?;
        let g = self.generation();
        Some(types::distinct(&[types::from_rom(g, info.types[0]), types::from_rom(g, info.types[1])]))
    }

    /// Pokémon adverses d'un dresseur, prêts pour le calcul.
    pub fn team(&self, id: u16) -> Option<Vec<Combatant>> {
        let t = self.trainer(id)?;
        Some(t.team.iter().map(|p| self.build(t, p)).collect())
    }

    fn build(&self, t: &Trainer, p: &TrainerPokemon) -> Combatant {
        let generation = self.generation();
        let dg = self.dex_game();
        let mut notes = Vec::new();
        let (info, record) = match self.personal_of(p.species, p.form) {
            Some(x) => x,
            None => {
                notes.push("Fiche de l'espèce introuvable dans la ROM : données du jeu d'origine.".into());
                (dex::personal(dg, p.species, p.form as u8).unwrap_or_else(|| dex::personal(dg, 1, 0).unwrap()), p.species as usize)
            }
        };
        let level = p.level.clamp(1, 100) as u8;
        let iv = (p.difficulty as u32 * 31 / 255) as u8;
        let ivs = [iv; 6];

        // Nature et talent.
        let (nature, ability) = if generation == 4 {
            // Diamant / Perle : classes identiques à celles de Platine (n° 0-97), même calcul.
            // HGSS : classes différentes, non reprises (nature approximative pour les dresseuses).
            let female = matches!(self.game, Game::Platinum | Game::Diamond | Game::Pearl) && PT_FEMALE_CLASSES.contains(&t.class_id);
            if matches!(self.game, Game::HeartGold | Game::SoulSilver) {
                notes.push("HeartGold / SoulSilver : nature calculée comme pour un dresseur masculin (classes non vérifiées).".into());
            }
            let pid = gen4_pid(p.difficulty, p.level, p.species, t.id, t.class_id, female);
            ((pid % 25) as u8, ability_slot(&info, if pid & 1 == 1 { 1 } else { 0 }))
        } else {
            notes.push("Nature inconnue (calcul du jeu non reproduit) : Hardi (neutre) supposée.".into());
            let slot = match (p.gender_ability >> 4) & 3 {
                2 => 1,
                3 => 2,
                _ => 0,
            };
            (0, ability_slot(&info, slot))
        };
        let b = info.base_stats;
        let stats = calc_stats(&b, level, ivs, [0; 6], nature);

        let custom = t.custom_moves && p.moves.iter().any(|&m| m != 0);
        let moves = if custom {
            p.moves
        } else {
            notes.push("Attaques : les 4 dernières apprises par niveau (le dresseur n'en a pas de fixées).".into());
            self.learnsets.get(record).or_else(|| self.learnsets.get(p.species as usize)).map_or([0; 4], |ls| learnsets::moves_at_level(ls, p.level))
        };
        let friendship = if moves.contains(&super::ids::moves::FRUSTRATION) { 0 } else { 255 };
        let weight = dex::personal(dg, p.species, p.form as u8)
            .map(|x| x.weight)
            .filter(|&w| w > 0)
            .or_else(|| dex::personal(dex::Game::B2W2, p.species, p.form as u8).map(|x| x.weight))
            .unwrap_or(0);
        let t0 = types::from_rom(generation, info.types[0]);
        let t1 = types::from_rom(generation, info.types[1]);
        Combatant {
            species: p.species,
            form: p.form as u8,
            name: dex::species_name(p.species).unwrap_or("?").to_string(),
            level,
            types: types::distinct(&[t0, t1]),
            stats,
            ability,
            item: p.item,
            moves,
            nature,
            ivs,
            evs: [0; 6],
            friendship,
            weight,
            notes,
        }
    }
}

/// Nom affiché : le rival de Platine prend le nom choisi par le joueur (absent de la ROM).
fn display_name(t: &Trainer) -> String {
    let n = t.name.trim();
    if n.is_empty() || n == "-" || n.starts_with('{') {
        match t.role {
            Some(Role::Rival) => "Rival".into(),
            _ => format!("Dresseur n°{}", t.id),
        }
    } else {
        n.to_string()
    }
}

/// Talent d'une case (0, 1 ou 2 = caché), avec repli sur le premier.
fn ability_slot(info: &PersonalInfo, slot: usize) -> u16 {
    let a = info.abilities.get(slot).copied().unwrap_or(0);
    if a == 0 {
        info.abilities[0]
    } else {
        a
    }
}

/// PID d'un Pokémon de dresseur en Gen 4 (pret/pokeplatinum, `TrainerData_BuildParty`).
pub fn gen4_pid(difficulty: u8, level: u16, species: u16, trainer_id: u16, class: u16, female: bool) -> u32 {
    let mut state = difficulty as u32 + level as u32 + species as u32 + trainer_id as u32;
    let mut rnd = state;
    for _ in 0..class {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(24_691);
        rnd = state >> 16;
    }
    (rnd << 8).wrapping_add(if female { 120 } else { 136 })
}
