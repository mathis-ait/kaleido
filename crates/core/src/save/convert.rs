//! Transfert d'un Pokémon vers une génération plus récente (PK3 → PK4 → PK5 → PK6 → PK7),
//! comme le Parc des Amis (DS), Poké Transfert (DS), Poké Transporteur et la Banque
//! Pokémon (3DS). PK3 → PK4 : `PK3.ConvertToPK4` de PKHeX.
//!
//! Portage de PKHeX (kwsch/PKHeX, GPLv3, <https://github.com/kwsch/PKHeX>) :
//! `PK4.ConvertToPK5`, `PK5.ConvertToPK6` (+ `GetTransferPID`, `GetTransferMetLocation4`,
//! `CountContestRibbons`, `CountBattleRibbons`, `CalculateTransferAbilityIndex`),
//! `PK6.ConvertToPK7`, `IMemoryHT.SetTradeMemoryHT6`, `PK7.TradeHT` / `G6PKM.TradeOT`,
//! `PersonalInfo4.GetPreferredTransferHMs` et les constantes de `Locations.cs`.
//!
//! Les jeux ne connaissent que le sens « vers plus récent » : un transfert vers une
//! génération plus ancienne est refusé. Les conversions s'enchaînent (PK4 → PK7 =
//! PK4 → PK5 → PK6 → PK7).

use serde::Serialize;

use super::pkm::{Gender, PkmError, PkmFormat, Pokemon};
use super::session::{max_pp, today};
use super::stats;
use super::strings;
use crate::dex::{self, Game};

// --- Constantes de PKHeX (Locations.cs).

/// « Kanto » / « Johto » (Gen 7) : lieux des Pokémon venus de la Console virtuelle (`Locations.Transfer1/2`).
pub const TRANSFER1: u16 = 30013;
pub const TRANSFER2: u16 = 30017;
/// « Parc des Amis » (Gen 4) : lieu de rencontre des Pokémon venus de la Gen 3 (`Locations.Transfer3`).
pub const TRANSFER3: u16 = 55;
/// « Poké Transfert » (Gen 5) : lieu de rencontre des Pokémon venus de la Gen 4.
pub const TRANSFER4: u16 = 30001;
/// Celebi événement transféré (Ilex non encore visité).
pub const TRANSFER4_CELEBI_UNUSED: u16 = 30010;
/// Bêtes légendaires événement transférées (Raikou, Entei, Suicune).
pub const TRANSFER4_CROWN_UNUSED: u16 = 30012;

// --- Attaques CS de la Gen 4 retirées au transfert (PersonalInfo4.MachineMovesHidden*).
const CUT: u16 = 15;
const FLY: u16 = 19;
const SURF: u16 = 57;
const STRENGTH: u16 = 70;
const WATERFALL: u16 = 127;
const ROCK_SMASH: u16 = 249;
const WHIRLPOOL: u16 = 250;
const ROCK_CLIMB: u16 = 431;
const DEFOG: u16 = 432;
const HMS_DPPT: [u16; 8] = [CUT, FLY, SURF, STRENGTH, DEFOG, ROCK_SMASH, WATERFALL, ROCK_CLIMB];
const HMS_HGSS: [u16; 8] = [CUT, FLY, SURF, STRENGTH, WHIRLPOOL, ROCK_SMASH, WATERFALL, ROCK_CLIMB];

const ARCEUS: u16 = 493;
const SHEDINJA: u16 = 292;
const RAIKOU: u16 = 243;
const ENTEI: u16 = 244;
const SUICUNE: u16 = 245;
const CELEBI: u16 = 251;

/// Dresseur qui reçoit le Pokémon (celui de la sauvegarde de destination).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferTrainer {
    pub name: String,
    pub gender: Gender,
    pub tid: u16,
    pub sid: u16,
}

impl From<&super::Trainer> for TransferTrainer {
    fn from(t: &super::Trainer) -> Self {
        Self { name: t.name.clone(), gender: t.gender, tid: t.tid, sid: t.sid }
    }
}

/// Élément absent du jeu de destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// "species", "form", "move", "item" ou "ability".
    pub kind: &'static str,
    pub id: u16,
    pub message: String,
    /// `true` si on peut le retirer pour permettre le transfert (attaque, objet).
    pub fixable: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("Transfert impossible : ce Pokémon est au format {from} et ce jeu attend du {to}. Les jeux ne transfèrent que vers une génération plus récente ; il reste dans la banque.")]
    Downgrade { from: PkmFormat, to: PkmFormat },
    #[error("Un œuf ne peut pas changer de génération : fais-le éclore dans son jeu d'origine d'abord.")]
    Egg,
    #[error("Ce Pokémon n'est pas compatible avec ce jeu : {}", .0.iter().map(|p| p.message.as_str()).collect::<Vec<_>>().join(" ; "))]
    Incompatible(Vec<Problem>),
    #[error("conversion impossible : {0}")]
    Pkm(#[from] PkmError),
}

/// Diagnostic avant un retrait vers une sauvegarde.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compatibility {
    /// Retrait possible tel quel.
    pub ok: bool,
    /// Retrait possible en retirant les attaques / objets absents du jeu.
    pub fixable: bool,
    /// Une conversion de format aura lieu (ex. PK4 → PK7).
    pub converts: bool,
    pub from: PkmFormat,
    pub to: PkmFormat,
    /// Raison bloquante (descente de génération, œuf…), en français.
    pub blocker: Option<String>,
    pub problems: Vec<Problem>,
    /// Ce qui changera au transfert, en français.
    pub changes: Vec<String>,
}

/// Jeu dont PKHeX utilise les fiches pour un format (`PKx.PersonalInfo`).
pub fn data_game(f: PkmFormat) -> Game {
    match f {
        PkmFormat::Gen1 => Game::Y,
        PkmFormat::Gen2 => Game::C,
        PkmFormat::Gen3 => Game::E,
        PkmFormat::Gen4 => Game::HGSS,
        PkmFormat::Gen5 => Game::B2W2,
        PkmFormat::Gen6 => Game::ORAS,
        PkmFormat::Gen7 => Game::USUM,
    }
}

/// Format des Pokémon d'un jeu.
pub fn format_of(game: Game) -> PkmFormat {
    match game.generation() {
        1 => PkmFormat::Gen1,
        2 => PkmFormat::Gen2,
        3 => PkmFormat::Gen3,
        4 => PkmFormat::Gen4,
        5 => PkmFormat::Gen5,
        6 => PkmFormat::Gen6,
        _ => PkmFormat::Gen7,
    }
}

fn next_format(f: PkmFormat) -> Option<PkmFormat> {
    match f {
        // Console virtuelle : la Banque Pokémon envoie directement en Gen 7.
        PkmFormat::Gen1 | PkmFormat::Gen2 => Some(PkmFormat::Gen7),
        PkmFormat::Gen3 => Some(PkmFormat::Gen4),
        PkmFormat::Gen4 => Some(PkmFormat::Gen5),
        PkmFormat::Gen5 => Some(PkmFormat::Gen6),
        PkmFormat::Gen6 => Some(PkmFormat::Gen7),
        PkmFormat::Gen7 => None,
    }
}

// --- Accès bruts aux octets déchiffrés (champs que `Pokemon` n'expose pas).

fn wr16(d: &mut [u8], at: usize, v: u16) {
    d[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn rd32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn bit(d: &[u8], at: usize, b: u8) -> bool {
    d[at] >> b & 1 != 0
}

fn set_bit(d: &mut [u8], at: usize, b: u8, v: bool) {
    d[at] = (d[at] & !(1 << b)) | (v as u8) << b;
}

/// Modifie les octets bruts d'un Pokémon (taille équipe conservée).
fn edit_raw(p: &Pokemon, f: impl FnOnce(&mut [u8])) -> Pokemon {
    let mut d = p.data().to_vec();
    f(&mut d);
    Pokemon::from_decrypted(p.format(), &d).expect("taille inchangée")
}

/// Niveau actuel calculé depuis l'expérience (courbe de l'espèce dans le jeu donné).
fn current_level(game: Game, p: &Pokemon) -> u8 {
    let growth = dex::personal(game, p.species(), p.form()).map(|i| i.growth_rate).or_else(|| crate::names::growth_rate(p.species()));
    p.level(growth).unwrap_or_else(|| stats::level_from_exp(stats::GrowthRate::MediumFast, p.exp()))
}

/// `PKM.HealPP` : PP au maximum (avec les PP Plus), d'après les attaques du jeu.
fn heal_pp(game: Game, p: &mut Pokemon) {
    let ups = p.pp_ups();
    let moves = p.moves();
    p.set_pp(std::array::from_fn(|i| if moves[i] == 0 { 0 } else { max_pp(dex::move_info_in(game, moves[i]).map_or(0, |m| m.pp), ups[i]) }));
}

/// `PKM.FixMoves` : resserre les attaques (pas de trou), PP Plus alignés.
fn fix_moves(p: &mut Pokemon) {
    let moves = p.moves();
    let ups = p.pp_ups();
    let mut m = [0u16; 4];
    let mut u = [0u8; 4];
    let mut n = 0;
    for i in 0..4 {
        if moves[i] != 0 {
            m[n] = moves[i];
            u[n] = ups[i];
            n += 1;
        }
    }
    // Écart Kaleido : un Pokémon sans aucune attaque ferait planter les jeux ; on lui laisse Écras'Face.
    if n == 0 {
        m[0] = 1;
    }
    p.set_moves(m);
    p.set_pp_ups(u);
}

/// Nom de l'espèce dans la langue du Pokémon, pour un Pokémon sans surnom. Kaleido n'a
/// que les noms français ; pour les autres langues latines, le nom en MAJUSCULES des
/// jeux DS passe en casse « Titre » comme sur 3DS (ex. « ROSERADE » → « Roserade »).
fn species_name_for(species: u16, language: u8, current: &str) -> String {
    const FRENCH: u8 = 3;
    const LATIN: [u8; 5] = [2, 3, 4, 5, 7];
    if language == FRENCH {
        if let Some(n) = dex::species_name(species) {
            return n.to_string();
        }
    }
    if LATIN.contains(&language) && current.chars().any(char::is_alphabetic) && current == current.to_uppercase() {
        let mut out = String::new();
        let mut start = true;
        for c in current.chars() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = !c.is_alphanumeric() && c != '\'';
        }
        return out;
    }
    current.to_string()
}

// =====================================================================================
// PK1 / PK2 → PK7 (Poké Transporteur, Console virtuelle)
// =====================================================================================

/// `PK1.ConvertToPK7` / `PK2.ConvertToPK7`. Les valeurs aléatoires (EC, PID, IV) viennent
/// d'un générateur initialisé avec les données du Pokémon : la même conversion donne le même
/// résultat. Pays et région de la console ne sont pas renseignés (pas de 3DS de référence).
pub fn pk12_to_pk7(p: &Pokemon) -> Result<Pokemon, PkmError> {
    let gen = p.format().generation();
    let species = p.species();
    let game7 = data_game(PkmFormat::Gen7);
    let pi = dex::personal(game7, species, 0).ok_or_else(|| PkmError::Invalid(format!("espèce n°{species} inconnue")))?;
    let level = stats::level_from_exp(pi.growth_rate, p.exp());
    let mut seed = (p.exp() << 8) ^ (p.tid() as u32) << 16 ^ (species as u32) ^ (crate::save::pk12::ivs_to_dvs(p.ivs()) as u32) << 4;
    let mut next = || {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        seed
    };
    let mut pk = Pokemon::blank(PkmFormat::Gen7);
    pk.set_encryption_constant(next());
    pk.set_pid(next() << 16 | (next() >> 16));
    pk.set_species(species);
    pk.set_tid(p.tid());
    pk.set_sid(0);
    pk.set_exp(stats::exp_for_level(pi.growth_rate, level));
    pk.set_met_level(level)?;
    pk.set_nature((p.exp() % 25) as u8)?;
    pk.set_ball(4);
    pk.set_met_date(Some(today()));
    // Version : Rouge (Console virtuelle Gen 1) ; Gen 2 : Cristal si données de capture, sinon Or.
    let version = if gen == 1 { 35 } else if p.met_level() != 0 || p.met_location() != 0 { 41 } else { 39 };
    pk.set_version(version);
    pk.set_moves(p.moves().map(|m| if m == 146 { 0 } else { m })); // Uppercut (Gen 2) non transférable
    pk.set_pp_ups(p.pp_ups());
    fix_moves(&mut pk);
    pk.set_met_location(if gen == 1 { TRANSFER1 } else { TRANSFER2 });
    pk.set_gender(p.gender());
    if gen == 2 {
        pk.set_form(p.form())?;
    }
    let lang = match p.language() {
        1..=8 => p.language(),
        _ => 3,
    };
    pk.set_language(lang);
    let name = if lang == 3 { dex::species_name(species).map(str::to_string) } else { dex::species_name_lang(species, dex::Lang::En).map(str::to_string) };
    pk.set_nickname(&name.unwrap_or_default())?;
    pk.set_is_nicknamed(false);
    pk.set_ot_name(&p.ot_name())?;
    pk.set_ot_gender(if gen == 2 { p.ot_gender() } else { Gender::Male });
    pk.set_friendship(pi.base_friendship);
    // Talent caché, sauf pour les espèces où le Poké Transporteur ne le donne pas.
    let no_hidden = matches!(species, 92..=94 | 109 | 110 | 151) || (gen == 2 && matches!(species, 200 | 201 | 251));
    let slot = if no_hidden || pi.abilities[2] == 0 { 0 } else { 2 };
    pk.set_ability(pi.abilities[slot])?;
    pk.set_ability_number(1 << slot)?;
    // IV : 3 à 31 (5 pour Mew et Celebi), les autres au hasard, dans un ordre mélangé.
    let special = matches!(species, 151 | 251);
    let mut ivs: [u8; 6] = std::array::from_fn(|_| (next() >> 27) as u8);
    for iv in ivs.iter_mut().take(if special { 5 } else { 3 }) {
        *iv = 31;
    }
    for i in (1..6).rev() {
        let j = (next() >> 16) as usize % (i + 1);
        ivs.swap(i, j);
    }
    pk.set_ivs(ivs)?;
    // Même statut chromatique qu'à l'origine (`SetTransferPID`).
    let shiny = p.is_shiny();
    if shiny != pk.is_shiny() {
        let pid = pk.pid();
        let new = if shiny { let low = pid & 0xFFFF; (low ^ p.tid() as u32) << 16 | low } else { pid ^ 0x1000_0000 };
        pk.set_pid(new);
    }
    if special {
        pk.set_fateful_encounter(true);
    }
    let mut pk = edit_raw(&pk, |d| set_trade_memory_ht6(d, seed));
    heal_pp(game7, &mut pk);
    pk.refresh_checksum();
    Ok(pk)
}

// =====================================================================================
// PK3 → PK4 (Parc des Amis)
// =====================================================================================

/// `PK3.ConvertToPK4`. Le PK3 est déjà tenu au format PK4 en mémoire (voir `pk3`) :
/// espèce, objet, textes, rubans de Hoenn, PID, IV, EV, attaques et origine sont repris
/// tels quels, puis les règles du Parc des Amis sont appliquées.
pub fn pk3_to_pk4(pk3: &Pokemon) -> Result<Pokemon, PkmError> {
    debug_assert_eq!(pk3.format(), PkmFormat::Gen3);
    let level = current_level(data_game(PkmFormat::Gen3), pk3);
    let ball = pk3.ball();
    let mut d = pk3.data()[..PkmFormat::Gen4.party_size()].to_vec();
    // Section équipe vidée : un Pokémon du Parc des Amis arrive en boîte.
    d[PkmFormat::Gen4.stored_size()..].fill(0);
    // Bonheur remis à 70, marquages limités aux 4 premiers symboles.
    d[0x14] = 70;
    d[0x16] &= 0b1111;
    // Plus d'œuf, surnom conservé (bit 31 de l'IV32).
    let iv32 = rd32(&d, 0x38) & !(1 << 30);
    d[0x38..0x3C].copy_from_slice(&iv32.to_le_bytes());
    // Lieux et Ball relus par les accesseurs Gen 4 (champs DP / Platine / HGSS).
    d[0x44..0x48].fill(0);
    d[0x7E..0x82].fill(0);
    d[0x83] = 0;
    d[0x86] = 0;
    let mut pk4 = Pokemon::from_decrypted(PkmFormat::Gen4, &d)?;
    // Objet sans équivalent en Gen 4 : perdu (`ItemConverter.IsItemTransferable34`).
    if pk4.held_item() & dex::GEN3_ITEM_FLAG != 0 {
        pk4.set_held_item(0);
    }
    pk4.set_ball(if (1..=12).contains(&ball) { ball } else { 4 });
    pk4.set_met_location(TRANSFER3);
    pk4.set_met_level(level)?;
    pk4.set_met_date(Some(today()));
    if pk4.species() == SHEDINJA {
        pk4.set_gender(Gender::Genderless);
    }
    pk4.refresh_checksum();
    Ok(pk4)
}

// =====================================================================================
// PK4 → PK5 (Poké Transfert)
// =====================================================================================

/// `PK5.GetTransferMetLocation4` : lieu « Poké Transfert », sauf bêtes légendaires et
/// Celebi événement (rencontre fatidique), qui ont leur lieu spécial.
pub fn transfer_met_location4(p: &Pokemon) -> u16 {
    if !p.fateful_encounter() {
        return TRANSFER4;
    }
    match p.species() {
        RAIKOU | ENTEI | SUICUNE => TRANSFER4_CROWN_UNUSED,
        CELEBI => TRANSFER4_CELEBI_UNUSED,
        _ => TRANSFER4,
    }
}

/// `PK4.ConvertToPK5`.
pub fn pk4_to_pk5(pk4: &Pokemon) -> Result<Pokemon, PkmError> {
    debug_assert_eq!(pk4.format(), PkmFormat::Gen4);
    let src_game = data_game(PkmFormat::Gen4);
    let dst_game = data_game(PkmFormat::Gen5);
    let nature = pk4.nature();
    let ball = pk4.ball();
    let level = current_level(src_game, pk4);
    let nickname = pk4.nickname();
    let nicknamed = pk4.is_nicknamed();
    let ot = pk4.ot_name();

    // Données stockées copiées telles quelles (même disposition PK4 / PK5).
    let mut d = vec![0u8; PkmFormat::Gen5.party_size()];
    d[..PkmFormat::Gen4.stored_size()].copy_from_slice(&pk4.stored_data());
    // Bonheur du dresseur d'origine remis à 70.
    d[0x14] = 70;
    // Nature détachée du PID (0x41 = feuille brillante en Gen 4).
    d[0x41] = nature;
    // Lieux Platine / HGSS effacés (la Gen 5 ne lit que les champs DP).
    d[0x44..0x48].fill(0);
    // Données HGSS effacées (Ball HGSS, humeur du Pokéwalker).
    d[0x86..0x88].fill(0);
    // Ball : la plus grande des deux Balls de la Gen 4.
    d[0x83] = ball;
    let mut pk5 = Pokemon::from_decrypted(PkmFormat::Gen5, &d)?;

    pk5.set_met_location(transfer_met_location4(pk4));
    // Écart : PKHeX garde la date de rencontre (son appel à GetDateNDS n'est pas affecté) ;
    // Poké Transfert met la date du transfert. On suit le jeu.
    pk5.set_met_date(Some(today()));
    // Arceus : la Plaque est retirée et la forme revient à Normal.
    if pk5.species() == ARCEUS {
        pk5.set_form(0)?;
        pk5.set_held_item(0);
    } else if dex::item_name_in(dst_game, pk5.held_item()).is_none() || pk5.held_item() > dex::max_item(dst_game) {
        pk5.set_held_item(0);
    }
    // Ré-encodage des textes (table de caractères Gen 4 → UTF-16 Gen 5).
    pk5.set_nickname(&nickname)?;
    pk5.set_is_nicknamed(nicknamed);
    pk5.set_ot_name(&ot)?;
    // Niveau de rencontre = niveau actuel.
    pk5.set_met_level(level)?;
    // CS retirées (Anti-Brume gardée si le Pokémon la connaît : on retire alors Siphon).
    let banned = if pk4.moves().contains(&DEFOG) { HMS_HGSS } else { HMS_DPPT };
    pk5.set_moves(pk5.moves().map(|m| if banned.contains(&m) { 0 } else { m }));
    fix_moves(&mut pk5);
    heal_pp(dst_game, &mut pk5);
    // Munja créé en DP / HGSS : sexe « asexué » oublié par le jeu.
    if pk5.species() == SHEDINJA {
        pk5.set_gender(Gender::Genderless);
    }
    pk5.refresh_checksum();
    Ok(pk5)
}

// =====================================================================================
// PK5 → PK6 (Poké Transporteur)
// =====================================================================================

/// `PK5.GetTransferPID` : la Gen 6 compare le XOR sur 4 bits au lieu de 3 ; un Pokémon
/// qui deviendrait chromatique par erreur voit le bit 31 de son PID inversé.
pub fn transfer_pid(ec: u32, tid: u16, sid: u16) -> u32 {
    let oid = (sid as u32) << 16 | tid as u32;
    let tmp = ec ^ oid;
    let xor = tmp ^ (tmp >> 16);
    if xor & 0xFFF8 == 8 {
        ec ^ 0x8000_0000
    } else {
        ec
    }
}

/// Rubans : (octet PK5, bit) → (octet PK6, bit). Tables `PK5.cs` / `PK6.cs` de PKHeX.
const RIBBONS_5_TO_6: [((usize, u8), (usize, u8)); 32] = [
    ((0x3E, 4), (0x30, 1)), // Maître Hoenn
    ((0x24, 0), (0x30, 2)), // Maître Sinnoh
    ((0x3F, 0), (0x30, 7)), // Effort
    ((0x24, 7), (0x31, 0)), // Alerte
    ((0x25, 0), (0x31, 1)), // Choc
    ((0x25, 1), (0x31, 2)), // Abattu
    ((0x25, 2), (0x31, 3)), // Insouciance
    ((0x25, 3), (0x31, 4)), // Détente
    ((0x25, 4), (0x31, 5)), // Sieste
    ((0x25, 5), (0x31, 6)), // Sourire
    ((0x25, 6), (0x31, 7)), // Splendide
    ((0x25, 7), (0x32, 0)), // Royal
    ((0x26, 0), (0x32, 1)), // Royal Splendide
    ((0x3E, 7), (0x32, 2)), // Artiste
    ((0x26, 1), (0x32, 3)), // Empreinte
    ((0x26, 2), (0x32, 4)), // Record
    ((0x26, 4), (0x32, 5)), // Légende
    ((0x3F, 4), (0x32, 6)), // Pays
    ((0x3F, 5), (0x32, 7)), // National
    ((0x3F, 6), (0x33, 0)), // Terre
    ((0x3F, 7), (0x33, 1)), // Monde
    ((0x27, 2), (0x33, 2)), // Classique
    ((0x27, 3), (0x33, 3)), // Premier
    ((0x26, 3), (0x33, 4)), // Événement
    ((0x26, 6), (0x33, 5)), // Anniversaire
    ((0x26, 7), (0x33, 6)), // Spécial
    ((0x27, 0), (0x33, 7)), // Souvenir
    ((0x27, 1), (0x34, 0)), // Souhait
    ((0x3F, 1), (0x34, 1)), // Champion Combat
    ((0x3F, 2), (0x34, 2)), // Champion Régional
    ((0x3F, 3), (0x34, 3)), // Champion National
    ((0x26, 5), (0x34, 4)), // Champion Mondial
];

/// `PK5.CountBattleRibbons` : rubans de combat de la Gen 4 (Tour de Combat, Victoire…).
fn count_battle_ribbons(d: &[u8]) -> u8 {
    let bits1 = (d[0x24] & 0b0111_1110) as u32;
    let bits2 = (d[0x3E] & 0b0110_0000) as u32;
    (bits1 | bits2 << 2).count_ones() as u8
}

/// `PK5.CountContestRibbons` : 40 rubans de concours (Hoenn et Sinnoh).
fn count_contest_ribbons(d: &[u8]) -> u8 {
    let bits1 = (rd32(d, 0x60) & 0xF_FFFF) as u64;
    let bits2 = (rd32(d, 0x3C) & 0xF_FFFF) as u64;
    (bits1 << 20 | bits2).count_ones() as u8
}

/// `PK5.CalculateTransferAbilityIndex` : 0, 1 ou 2 (caché).
fn transfer_ability_index(pk5: &Pokemon) -> usize {
    if pk5.ability_number() == 4 {
        return 2;
    }
    if let Some(info) = dex::personal(data_game(PkmFormat::Gen5), pk5.species(), pk5.form()) {
        if pk5.ability() == info.abilities[0] {
            return 0;
        }
        if pk5.ability() == info.abilities[1] {
            return 1;
        }
    }
    // Talent invalide : déduit du PID (bit 16 pour un Pokémon né en Gen 5).
    let mut pid = pk5.pid();
    if (20..=23).contains(&pk5.version()) {
        pid >>= 16;
    }
    (pid & 1) as usize
}

/// Ressenti du souvenir « échangé » (`MemoryContext6.GetRandomFeeling6(4, 10)`).
/// Écart : PKHeX tire au hasard parmi les ressentis permis pour le souvenir n°4 ; Kaleido
/// choisit, d'après le PID (reproductible), 6 ou 7 : les ressentis relevés sur de vrais
/// Pokémon passés par la Banque (fichiers de test de PKHeX).
fn trade_feeling(seed: u32) -> u8 {
    6 + (seed & 1) as u8
}

/// `SetTradeMemoryHT6(bank: true)` : souvenir « échangé » du dresseur actuel (lieu 0 = Banque).
fn set_trade_memory_ht6(d: &mut [u8], seed: u32) {
    d[0xA5] = 4; // souvenir : échange
    wr16(d, 0xA8, 0);
    d[0xA4] = 1; // intensité
    d[0xA6] = trade_feeling(seed);
}

/// `PK5.ConvertToPK6`. `ht` = dresseur qui transfère (sinon le dresseur d'origine).
pub fn pk5_to_pk6(pk5: &Pokemon, ht: Option<&TransferTrainer>) -> Result<Pokemon, PkmError> {
    debug_assert_eq!(pk5.format(), PkmFormat::Gen5);
    let s = pk5.data();
    let dst_game = data_game(PkmFormat::Gen6);
    let mut pk6 = Pokemon::blank(PkmFormat::Gen6);
    pk6.set_encryption_constant(pk5.pid());
    pk6.set_species(pk5.species());
    pk6.set_tid(pk5.tid());
    pk6.set_sid(pk5.sid());
    pk6.set_exp(pk5.exp());
    pk6.set_pid(transfer_pid(pk5.pid(), pk5.tid(), pk5.sid()));
    // Objet tenu : non transféré (PKHeX ne le recopie pas).
    pk6.set_markings(pk5.markings());
    pk6.set_language(pk5.language().max(1));
    // EV plafonnés à 252.
    pk6.set_evs(pk5.evs().map(|v| v.min(252)));
    pk6.set_moves(pk5.moves());
    pk6.set_pp_ups(pk5.pp_ups());
    pk6.set_ivs(pk5.ivs())?;
    pk6.set_is_egg(pk5.is_egg());
    pk6.set_is_nicknamed(pk5.is_nicknamed());
    pk6.set_fateful_encounter(pk5.fateful_encounter());
    pk6.set_gender(pk5.gender());
    pk6.set_form(pk5.form())?;
    pk6.set_nature(pk5.nature())?;
    pk6.set_version(pk5.version());
    // Dates et lieux gardés.
    pk6.set_met_date(pk5.met_date());
    pk6.set_egg_date(pk5.egg_date());
    pk6.set_met_location(pk5.met_location());
    pk6.set_egg_location(pk5.egg_location());
    let (strain, days) = pk5.pokerus();
    pk6.set_pokerus(strain, days);
    pk6.set_ball(pk5.ball());
    pk6.set_met_level(pk5.met_level())?;
    pk6.set_ot_gender(pk5.ot_gender());

    // Surnom : nom de l'espèce dans la langue du Pokémon, sauf vrai surnom.
    let nickname = pk5.nickname();
    let nickname = if pk5.is_nicknamed() { nickname } else { species_name_for(pk5.species(), pk5.language(), &nickname) };
    pk6.set_nickname(&nickname)?;
    pk6.set_is_nicknamed(pk5.is_nicknamed());
    pk6.set_ot_name(&pk5.ot_name())?;

    // Talent : même emplacement, talent de la Gen 6 (corrige les talents changés).
    let index = transfer_ability_index(pk5);
    pk6.set_ability_number(1 << index)?;
    let info = dex::personal(dst_game, pk6.species(), pk6.form());
    if let Some(info) = &info {
        let a = info.abilities[index];
        pk6.set_ability(if a == 0 { info.abilities[0] } else { a })?;
    } else {
        pk6.set_ability(pk5.ability())?;
    }

    fix_moves(&mut pk6);
    heal_pp(dst_game, &mut pk6);

    let base_friendship = info.as_ref().map_or(70, |i| i.base_friendship);
    let ht_name = ht.map_or_else(|| pk5.ot_name(), |t| t.name.clone());
    let ht_gender = ht.map_or(pk5.ot_gender(), |t| t.gender);
    let ht_bytes = strings::encode(PkmFormat::Gen6, &ht_name, 12)?;
    let seed = pk5.pid();
    let pk6 = edit_raw(&pk6, |d| {
        // Concours.
        d[0x24..0x2A].copy_from_slice(&s[0x1E..0x24]);
        // Type de terrain de la rencontre.
        d[0xDE] = s[0x85];
        // Rubans déplacés à leur nouvelle place.
        for ((from, fb), (to, tb)) in RIBBONS_5_TO_6 {
            set_bit(d, to, tb, bit(s, from, fb));
        }
        // Compteurs des rubans « souvenir » (concours et combats de la Gen 3 / 4).
        let contest = count_contest_ribbons(s);
        let battle = count_battle_ribbons(s);
        d[0x38] = contest;
        d[0x39] = battle;
        set_bit(d, 0x34, 5, contest != 0);
        set_bit(d, 0x34, 6, battle != 0);
        // Dresseur actuel : celui qui transfère, avec un souvenir d'échange.
        d[0x93] = 1;
        d[0x78..0x78 + ht_bytes.len()].copy_from_slice(&ht_bytes);
        d[0x92] = (ht_gender == Gender::Female) as u8;
        set_trade_memory_ht6(d, seed);
        // Bonheur remis à la valeur de base de l'espèce (dresseur d'origine et actuel).
        d[0xCA] = base_friendship;
        d[0xA2] = base_friendship;
    });
    let mut pk6 = pk6;
    pk6.refresh_checksum();
    Ok(pk6)
}

// =====================================================================================
// PK6 → PK7 (Banque Pokémon)
// =====================================================================================

/// `PK6.ConvertToPK7`.
pub fn pk6_to_pk7(pk6: &Pokemon) -> Result<Pokemon, PkmError> {
    debug_assert_eq!(pk6.format(), PkmFormat::Gen6);
    let markings = pk6.markings();
    let an = pk6.ability_number();
    let ability = pk6.ability();
    let seed = pk6.encryption_constant();
    let mut d = pk6.data().to_vec();
    // Champs propres à la Gen 6 réutilisés autrement en Gen 7.
    d[0x2A] = 0; // statut des événements du Poké Loisir (marquages en Gen 6)
    wr16(&mut d, 0x16, 0); // marquages Gen 7 (sac d'entraînement en Gen 6)
    d[0xDE] = 0; // Entraînement Ultime (type de terrain en Gen 6)
    d[0x3F] = 0; // argument de forme : seuls les 3 premiers octets sont gardés
    d[0xED] = 0; // saleté (Poké Loisir)
    d[0xEE] = 0;
    // Pays / régions des souvenirs, plénitude / plaisir (Poké Récré), divers.
    d[0x94..0x9E].fill(0);
    d[0xAA..0xB0].fill(0);
    d[0xE4..0xE8].fill(0);
    d[0x72] &= 0xFC;
    d[0xEF] = 0;
    set_trade_memory_ht6(&mut d, seed);
    let mut pk7 = Pokemon::from_decrypted(PkmFormat::Gen7, &d)?;
    // Marquages : présents en Gen 6 → bleus en Gen 7.
    pk7.set_markings(markings.map(|m| (m != 0) as u8));
    // Talent : même emplacement, s'il correspondait bien en Gen 6.
    if matches!(an, 1 | 2 | 4) {
        let index = (an >> 1) as usize;
        let old = dex::personal(data_game(PkmFormat::Gen6), pk6.species(), pk6.form());
        let new = dex::personal(data_game(PkmFormat::Gen7), pk7.species(), pk7.form());
        if let (Some(old), Some(new)) = (old, new) {
            if old.abilities[index] == ability && new.abilities[index] != 0 {
                pk7.set_ability(new.abilities[index])?;
            }
        }
    }
    heal_pp(data_game(PkmFormat::Gen7), &mut pk7);
    pk7.refresh_checksum();
    Ok(pk7)
}

// =====================================================================================
// Dresseur actuel (Gen 6 / 7) et vérifications
// =====================================================================================

/// `G6PKM.UpdateHandler` : en arrivant chez un dresseur, le Pokémon revient à son
/// dresseur d'origine (`TradeOT`) ou change de dresseur actuel (`PK7.TradeHT`).
/// Sans effet en Gen 4 / 5 (pas de notion de dresseur actuel).
pub fn update_handler(p: &Pokemon, tr: &TransferTrainer) -> Result<Pokemon, PkmError> {
    if p.format().generation() < 6 || p.is_egg() {
        return Ok(p.clone());
    }
    let is_ot = p.tid() == tr.tid && p.sid() == tr.sid && p.ot_gender() == tr.gender && p.ot_name() == tr.name;
    let base = dex::personal(data_game(p.format()), p.species(), p.form()).map_or(70, |i| i.base_friendship);
    let ht_bytes = strings::encode(p.format(), &tr.name, 12)?;
    let mut out = edit_raw(p, |d| {
        if is_ot {
            d[0x93] = 0;
            return;
        }
        let current = strings::decode(p.format(), &d[0x78..0x78 + 26]);
        if current != tr.name {
            d[0x78..0x78 + 26].copy_from_slice(&ht_bytes);
            d[0xA2] = base;
            d[0xA3] = 0;
        }
        d[0x93] = 1;
        d[0x92] = (tr.gender == Gender::Female) as u8;
    });
    out.refresh_checksum();
    Ok(out)
}

/// Éléments absents du jeu de destination (espèce, forme, attaques, objet, talent).
pub fn problems(p: &Pokemon, game: Game) -> Vec<Problem> {
    let mut out = Vec::new();
    let name = |n: Option<&str>, id: u16| n.map_or_else(|| format!("n°{id}"), str::to_string);
    let species = p.species();
    if species > dex::max_species(game) {
        out.push(Problem {
            kind: "species",
            id: species,
            message: format!(
                "{} n'existe pas dans ce jeu (Pokémon n°{species}, le dernier est le n°{})",
                name(dex::species_name(species), species),
                dex::max_species(game)
            ),
            fixable: false,
        });
        return out;
    }
    let forms = dex::form_names(game, species);
    if p.form() > 0 && !forms.is_empty() && p.form() as usize >= forms.len() {
        out.push(Problem {
            kind: "form", id: p.form() as u16, message: format!("la forme n°{} n'existe pas dans ce jeu", p.form()), fixable: false
        });
    }
    if p.ability() > dex::max_ability(game) {
        out.push(Problem {
            kind: "ability",
            id: p.ability(),
            message: format!("le talent {} n'existe pas dans ce jeu", name(dex::ability_name(p.ability()), p.ability())),
            fixable: false,
        });
    }
    for m in p.moves() {
        if m > dex::max_move(game) {
            out.push(Problem {
                kind: "move",
                id: m,
                message: format!("l'attaque {} n'existe pas dans ce jeu", name(dex::move_name(m), m)),
                fixable: true,
            });
        }
    }
    let item = p.held_item();
    if item != 0 && (item > dex::max_item(game) || dex::item_name_in(game, item).is_none()) {
        out.push(Problem {
            kind: "item",
            id: item,
            message: format!("l'objet {} n'existe pas dans ce jeu", name(dex::item_name(item), item)),
            fixable: true,
        });
    }
    out
}

/// Retire les attaques et l'objet absents du jeu.
pub fn strip_invalid(p: &Pokemon, game: Game) -> Pokemon {
    let mut p = p.clone();
    let max_move = dex::max_move(game);
    p.set_moves(p.moves().map(|m| if m > max_move { 0 } else { m }));
    fix_moves(&mut p);
    heal_pp(game, &mut p);
    let item = p.held_item();
    if item != 0 && (item > dex::max_item(game) || dex::item_name_in(game, item).is_none()) {
        p.set_held_item(0);
    }
    p.refresh_checksum();
    p
}

/// Ce qui changera au transfert (texte pour l'interface).
fn changes(from: PkmFormat, to: PkmFormat, p: &Pokemon) -> Vec<String> {
    let mut out = Vec::new();
    let mut f = from;
    while f != to {
        match f {
            PkmFormat::Gen1 | PkmFormat::Gen2 => {
                out.push("Console virtuelle → Gen 7 (Poké Transporteur) : nouvelles valeurs (IV, PID, nature d'après l'expérience), talent caché, lieu « Kanto » ou « Johto », surnom et objet perdus.".into());
            }
            PkmFormat::Gen3 => {
                out.push("Gen 3 → 4 (Parc des Amis) : lieu de rencontre « Parc des Amis », date de rencontre = aujourd'hui, niveau de rencontre = niveau actuel, bonheur remis à 70 ; un objet sans équivalent en Gen 4 est perdu.".into());
            }
            PkmFormat::Gen4 => {
                out.push("Gen 4 → 5 (Poké Transfert) : lieu de rencontre « Poké Transfert », date de rencontre = aujourd'hui, niveau de rencontre = niveau actuel, bonheur remis à 70.".into());
                if p.moves().iter().any(|m| HMS_DPPT.contains(m) || HMS_HGSS.contains(m)) {
                    out.push("Les CS de la Gen 4 (Coupe, Surf, Éclate-Roc…) sont oubliées.".into());
                }
            }
            PkmFormat::Gen5 => {
                out.push("Gen 5 → 6 (Poké Transporteur) : l'objet tenu n'est pas transféré, le bonheur revient à sa valeur de base, le dresseur qui transfère devient « dresseur actuel », les rubans sont réorganisés.".into());
            }
            PkmFormat::Gen6 => {
                out.push("Gen 6 → 7 (Banque Pokémon) : souvenirs de voyage (pays, régions) et données du Poké Loisir effacés, souvenir « échangé via la Banque » ajouté.".into());
            }
            PkmFormat::Gen7 => {}
        }
        f = next_format(f).unwrap_or(to);
    }
    if to.generation() >= 6 {
        out.push("En arrivant, le Pokémon reconnaît son dresseur d'origine ; sinon le dresseur de la sauvegarde devient son dresseur actuel.".into());
    }
    out
}

/// Diagnostic d'un retrait vers le jeu `game`.
pub fn compatibility(p: &Pokemon, game: Game) -> Compatibility {
    let from = p.format();
    let to = format_of(game);
    let mut c = Compatibility { ok: false, fixable: false, converts: from != to, from, to, blocker: None, problems: Vec::new(), changes: Vec::new() };
    if to.generation() < from.generation() {
        c.blocker = Some(ConvertError::Downgrade { from, to }.to_string());
        return c;
    }
    if c.converts && p.is_egg() {
        c.blocker = Some(ConvertError::Egg.to_string());
        return c;
    }
    // Problèmes évalués sur le Pokémon converti (objet retiré en Gen 6, etc.).
    match convert_chain(p, to, None) {
        Ok(converted) => c.problems = problems(&converted, game),
        Err(e) => {
            c.blocker = Some(e.to_string());
            return c;
        }
    }
    c.ok = c.problems.is_empty();
    c.fixable = !c.ok && c.problems.iter().all(|p| p.fixable);
    c.changes = changes(from, to, p);
    c
}

/// Enchaîne les conversions jusqu'au format voulu (sans vérifier le jeu).
pub fn convert_chain(p: &Pokemon, to: PkmFormat, ht: Option<&TransferTrainer>) -> Result<Pokemon, ConvertError> {
    let from = p.format();
    if to.generation() < from.generation() {
        return Err(ConvertError::Downgrade { from, to });
    }
    if from != to && p.is_egg() {
        return Err(ConvertError::Egg);
    }
    let mut cur = p.clone();
    while cur.format() != to {
        cur = match cur.format() {
            PkmFormat::Gen1 | PkmFormat::Gen2 => pk12_to_pk7(&cur)?,
            PkmFormat::Gen3 => pk3_to_pk4(&cur)?,
            PkmFormat::Gen4 => pk4_to_pk5(&cur)?,
            PkmFormat::Gen5 => pk5_to_pk6(&cur, ht)?,
            PkmFormat::Gen6 => pk6_to_pk7(&cur)?,
            PkmFormat::Gen7 => unreachable!("Gen 7 est le format le plus récent"),
        };
    }
    Ok(cur)
}

/// Prépare un Pokémon pour une sauvegarde du jeu `game` : conversion, vérification
/// (ou retrait des attaques / objet absents si `strip`), puis dresseur actuel.
pub fn convert_for(p: &Pokemon, game: Game, trainer: Option<&TransferTrainer>, strip: bool) -> Result<Pokemon, ConvertError> {
    let mut out = convert_chain(p, format_of(game), trainer)?;
    if strip {
        out = strip_invalid(&out, game);
    }
    let problems = problems(&out, game);
    if !problems.is_empty() {
        return Err(ConvertError::Incompatible(problems));
    }
    if let Some(tr) = trainer {
        out = update_handler(&out, tr)?;
    }
    // Section équipe vidée : elle sera recalculée par la sauvegarde si besoin.
    let stored = out.stored_data().to_vec();
    let mut out = Pokemon::from_decrypted(out.format(), &stored)?;
    out.refresh_checksum();
    Ok(out)
}

/// Détecte le format d'un fichier Pokémon d'après son extension ou sa taille.
pub fn format_from_file(ext: Option<&str>, len: usize) -> Option<PkmFormat> {
    let by_ext = match ext.map(str::to_ascii_lowercase).as_deref() {
        Some("pk1") => Some(PkmFormat::Gen1),
        Some("pk2") => Some(PkmFormat::Gen2),
        Some("pk3") => Some(PkmFormat::Gen3),
        Some("pk4") => Some(PkmFormat::Gen4),
        Some("pk5") => Some(PkmFormat::Gen5),
        Some("pk6") => Some(PkmFormat::Gen6),
        Some("pk7") => Some(PkmFormat::Gen7),
        _ => None,
    };
    by_ext.or(match len {
        69 => Some(PkmFormat::Gen1),
        73 => Some(PkmFormat::Gen2),
        80 | 100 => Some(PkmFormat::Gen3),
        136 | 236 => Some(PkmFormat::Gen4),
        220 => Some(PkmFormat::Gen5),
        // 232 / 260 : PK6 ou PK7 ; sans extension on suppose la Gen 7 si la version l'indique.
        _ => None,
    })
}

/// Valeur de chromatique d'un Pokémon avant / après (pour les tests et l'affichage).
pub fn shiny_xor(p: &Pokemon) -> u32 {
    let pid = p.pid();
    (p.tid() ^ p.sid()) as u32 ^ (pid >> 16) ^ (pid & 0xFFFF)
}

/// Dresseur actuel d'un Pokémon Gen 6 / 7 (`None` s'il est chez son dresseur d'origine).
pub fn handler(p: &Pokemon) -> Option<String> {
    (p.format().generation() >= 6 && p.data()[0x93] == 1).then(|| strings::decode(p.format(), &p.data()[0x78..0x78 + 26]))
}

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;
