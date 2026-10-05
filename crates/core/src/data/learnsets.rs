//! Attaques apprises par niveau.
//!
//! - Gen 4 : u16 = attaque (9 bits) | niveau << 9, terminé par 0xFFFF.
//! - Gen 5 : paires (attaque u16, niveau u16), terminées par 0xFFFF 0xFFFF.

use super::u16_at;

pub type Learnset = Vec<(u16, u8)>;

pub fn read(generation: u8, d: &[u8]) -> Learnset {
    let mut out = Vec::new();
    if generation <= 4 {
        for at in (0..d.len().saturating_sub(1)).step_by(2) {
            let v = u16_at(d, at);
            if v == 0xFFFF {
                break;
            }
            out.push((v & 0x1FF, (v >> 9) as u8));
        }
    } else {
        for at in (0..d.len().saturating_sub(3)).step_by(4) {
            let (mv, lvl) = (u16_at(d, at), u16_at(d, at + 2));
            if mv == 0xFFFF {
                break;
            }
            out.push((mv, lvl as u8));
        }
    }
    out
}

/// Les 4 dernières attaques apprises jusqu'à `level` (comme un Pokémon sauvage).
pub fn moves_at_level(learnset: &Learnset, level: u16) -> [u16; 4] {
    let mut known: Vec<u16> = Vec::new();
    for &(mv, lvl) in learnset {
        if lvl as u16 <= level && !known.contains(&mv) {
            known.push(mv);
        }
    }
    let mut out = [0; 4];
    for (slot, mv) in out.iter_mut().zip(known.iter().rev().take(4).rev()) {
        *slot = *mv;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_bulbasaur_learnsets() {
        // Bulbizarre, Platine : Charge (33) niv. 1, Rugissement (45) niv. 3.
        let pt = [0x21, 0x02, 0x2D, 0x06, 0xFF, 0xFF];
        assert_eq!(read(4, &pt), vec![(33, 1), (45, 3)]);
        let bw = [0x21, 0, 1, 0, 0x2D, 0, 3, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(read(5, &bw), vec![(33, 1), (45, 3)]);
    }

    #[test]
    fn last_four_moves() {
        let ls = vec![(1, 1), (2, 3), (3, 5), (4, 7), (5, 9)];
        assert_eq!(moves_at_level(&ls, 8), [1, 2, 3, 4]);
        assert_eq!(moves_at_level(&ls, 50), [2, 3, 4, 5]);
        assert_eq!(moves_at_level(&ls, 2), [1, 0, 0, 0]);
    }
}
