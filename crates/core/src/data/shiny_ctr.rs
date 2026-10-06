//! Taux de Pokémon chromatiques (3DS, Gen 6 / 7).
//!
//! Sur 3DS, un Pokémon est chromatique si `(ID ^ ID secret ^ PID haut ^ PID bas) < 16`,
//! soit 1 chance sur 4 096 par PID tiré. La fonction qui crée le PID en tire
//! plusieurs (1 normalement, 3 avec le Charme Chroma) et s'arrête au premier
//! chromatique. Le nombre de tirages est relu à chaque tour de boucle :
//! `ldrb r0, [r4, #0x23] ; add r5, r5, #1 ; cmp r0, r5 ; bgt …`, identique dans
//! X / Y, Rubis Oméga / Saphir Alpha, Soleil / Lune et Ultra-Soleil / Ultra-Lune
//! (même méthode que l'outil « Shiny Rate » de pk3DS).
//!
//! - Taux choisi : le `ldrb` devient `mov r0, #N` (N tirages, quel que soit le Charme
//!   Chroma). Probabilité : 1 − (4095 / 4096)^N. On ne peut pas descendre sous 1 / 4 096.
//! - « Tous » : juste avant, le cas « PID chromatique forcé » (`beq`) devient un saut
//!   inconditionnel. Les Pokémon qui ne doivent jamais être chromatiques le restent.
//!
//! Les octets sont cherchés dans le programme **décompressé** (code.bin).

use crate::rom::RomError;

/// Taux normal des jeux 3DS.
pub const BASE_ODDS: u32 = 4096;
/// Plus grand nombre de tirages utilisé (≈ 98 % de chromatiques).
const MAX_REROLLS: u32 = 0x4000;

/// `add r5, r5, #1 ; cmp r0, r5 ; bgt` : suit le `ldrb r0, [r4, #0x23]` à remplacer.
const REROLL_LOOP: [u8; 12] = [0x01, 0x50, 0x85, 0xE2, 0x05, 0x00, 0x50, 0xE1, 0xDE, 0xFF, 0xFF, 0xCA];
/// `ldrb r0, [r4, #0x23]` d'origine.
const LDRB_REROLLS: [u8; 4] = [0x23, 0x00, 0xD4, 0xE5];
/// `eor r2, r2, r0 ; eor r3, r1, #2 ; orrs r2, r2, r3 ; b?? +0x1C` (sans l'octet de condition).
const FORCED_CASE: [u8; 15] = [0x00, 0x20, 0x22, 0xE0, 0x02, 0x30, 0x21, 0xE2, 0x03, 0x20, 0x92, 0xE1, 0x1C, 0x00, 0x00];
const COND_EQ: u8 = 0x0A;
const COND_ALWAYS: u8 = 0xEA;

/// Ce que l'on fait au programme du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Unchanged,
    /// N tirages de PID.
    Rerolls(u32),
    /// Tous les Pokémon (sauf ceux qui ne doivent jamais l'être).
    Always,
}

/// Emplacements des deux modifications dans le programme décompressé.
#[derive(Debug, Clone, Copy)]
pub struct Sites {
    /// Instruction `ldrb` du nombre de tirages.
    pub reroll: usize,
    /// Octet de condition du cas « chromatique forcé ».
    pub always: Option<usize>,
}

fn find_unique(code: &[u8], pattern: &[u8]) -> Option<usize> {
    let mut hits = code.windows(pattern.len()).enumerate().filter(|(_, w)| *w == pattern).map(|(i, _)| i);
    match (hits.next(), hits.next()) {
        (Some(p), None) => Some(p),
        _ => None,
    }
}

/// Retrouve la fonction de création du PID (aussi dans un programme déjà modifié).
pub fn locate(code: &[u8]) -> Result<Sites, RomError> {
    // Rubis Oméga / Saphir Alpha contiennent d'autres boucles identiques : on garde
    // celle qui suit le cas « chromatique forcé », ou à défaut celle qui relit le `ldrb`.
    let loops: Vec<usize> = code.windows(REROLL_LOOP.len()).enumerate().filter(|(_, w)| *w == REROLL_LOOP).map(|(i, _)| i).collect();
    let always = find_unique(code, &FORCED_CASE).map(|p| p + FORCED_CASE.len()).filter(|&p| matches!(code.get(p), Some(&COND_EQ | &COND_ALWAYS)));
    let reroll = match always {
        Some(a) => loops.iter().copied().filter(|&l| l > a && l - a < 0x200).min(),
        None => {
            let mut original = loops.iter().copied().filter(|&l| l >= 4 && code[l - 4..l] == LDRB_REROLLS);
            match (original.next(), original.next()) {
                (Some(p), None) => Some(p),
                _ => None,
            }
        }
    };
    let reroll = reroll.filter(|&l| l >= 4).ok_or_else(|| RomError::Layout("fonction de création du PID introuvable dans code.bin".into()))?;
    Ok(Sites { reroll: reroll - 4, always })
}

/// Valeurs exprimables par `mov r0, #imm` en ARM : un octet tourné d'un nombre pair de bits.
fn arm_immediate(value: u32) -> Option<[u8; 2]> {
    (0..16u32).find_map(|rot| {
        let imm = value.rotate_left(2 * rot);
        (imm <= 0xFF).then_some([imm as u8, rot as u8])
    })
}

/// Valeur exprimable la plus proche de `n` (entre 1 et [`MAX_REROLLS`]).
fn nearest_encodable(n: u32) -> u32 {
    let n = n.clamp(1, MAX_REROLLS);
    (0..=n)
        .flat_map(|d| [n - d, n + d])
        .find(|&v| (1..=MAX_REROLLS).contains(&v) && arm_immediate(v).is_some())
        .unwrap_or(1)
}

/// Probabilité « 1 / x » obtenue avec N tirages.
pub fn effective_odds(rerolls: u32) -> u32 {
    let miss = ((BASE_ODDS - 1) as f64 / BASE_ODDS as f64).powi(rerolls as i32);
    (1.0 / (1.0 - miss)).round().max(1.0) as u32
}

/// Modification correspondant à « 1 chance sur `odds` ».
pub fn plan(odds: u32) -> Plan {
    if odds <= 1 {
        return Plan::Always;
    }
    if odds >= BASE_ODDS {
        return Plan::Unchanged;
    }
    let p = 1.0 / odds as f64;
    let n = ((1.0 - p).ln() / ((BASE_ODDS - 1) as f64 / BASE_ODDS as f64).ln()).round() as u32;
    match nearest_encodable(n) {
        1 => Plan::Unchanged,
        n => Plan::Rerolls(n),
    }
}

/// Applique `plan` au programme décompressé. Renvoie la probabilité obtenue « 1 / n ».
pub fn apply(code: &mut [u8], plan: Plan) -> Result<u32, RomError> {
    let sites = locate(code)?;
    let r = sites.reroll;
    match plan {
        Plan::Unchanged => {
            code[r..r + 4].copy_from_slice(&LDRB_REROLLS);
            if let Some(a) = sites.always {
                code[a] = COND_EQ;
            }
            Ok(BASE_ODDS)
        }
        Plan::Rerolls(n) => {
            let [imm, rot] = arm_immediate(n).ok_or_else(|| RomError::Layout(format!("{n} tirages : valeur non exprimable")))?;
            code[r..r + 4].copy_from_slice(&[imm, rot, 0xA0, 0xE3]); // mov r0, #n
            if let Some(a) = sites.always {
                code[a] = COND_EQ;
            }
            Ok(effective_odds(n))
        }
        Plan::Always => match sites.always {
            Some(a) => {
                code[r..r + 4].copy_from_slice(&LDRB_REROLLS);
                code[a] = COND_ALWAYS;
                Ok(1)
            }
            // Repli : le plus de tirages possible.
            None => apply(code, Plan::Rerolls(MAX_REROLLS)),
        },
    }
}

/// Modification présente dans un programme (taux normal si le `ldrb` est intact).
pub fn current(code: &[u8]) -> Result<Plan, RomError> {
    let sites = locate(code)?;
    if sites.always.is_some_and(|a| code[a] == COND_ALWAYS) {
        return Ok(Plan::Always);
    }
    let ins = &code[sites.reroll..sites.reroll + 4];
    if ins == LDRB_REROLLS {
        return Ok(Plan::Unchanged);
    }
    if ins[2..] == [0xA0, 0xE3] {
        return Ok(Plan::Rerolls((ins[0] as u32).rotate_right(2 * (ins[1] & 0xF) as u32)));
    }
    Err(RomError::Layout("fonction de création du PID modifiée par un autre outil".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odds_to_plan() {
        assert_eq!(plan(8192), Plan::Unchanged);
        assert_eq!(plan(4096), Plan::Unchanged);
        assert_eq!(plan(1), Plan::Always);
        assert_eq!(plan(2048), Plan::Rerolls(2));
        assert_eq!(plan(1024), Plan::Rerolls(4));
        let Plan::Rerolls(n) = plan(100) else { panic!() };
        assert!(arm_immediate(n).is_some());
        assert!((95..=105).contains(&effective_odds(n)), "{n} → {}", effective_odds(n));
        assert_eq!(effective_odds(1), 4096);
        assert_eq!(effective_odds(3), 1366);
    }

    #[test]
    fn immediates() {
        assert_eq!(arm_immediate(4), Some([4, 0]));
        assert_eq!(arm_immediate(0x400), Some([1, 11]));
        assert_eq!(arm_immediate(0x101), None);
        assert_eq!(nearest_encodable(0x101), 0x100);
    }
}
