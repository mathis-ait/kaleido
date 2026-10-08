// Pokémon 3DS (PK6/PK7, chiffrés ou non) dans une copie brute de la FCRAM : pk_file <dump.bin> <gen6|gen7> [autre.bin]
// Filtre : somme de contrôle des 232 octets (le mélange des blocs ne change pas la somme).
// Avec un second dump : n'affiche que les Pokémon absents de l'autre (par constante de chiffrement).
fn scan(bytes: &[u8]) -> Vec<(usize, bool, u32)> {
    let mut out = Vec::new();
    let rd16 = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let mut i = 0;
    while i + 232 <= bytes.len() {
        let ec = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let chk = rd16(i + 6);
        if ec != 0 && bytes[i + 4] == 0 && bytes[i + 5] == 0 && chk != 0 {
            // Chiffré.
            let mut seed = ec;
            let mut sum: u16 = 0;
            let mut plain: u16 = 0;
            for w in 0..112 {
                seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
                let v = rd16(i + 8 + 2 * w);
                sum = sum.wrapping_add(v ^ (seed >> 16) as u16);
                plain = plain.wrapping_add(v);
            }
            if sum == chk {
                out.push((i, true, ec));
            } else if plain == chk {
                out.push((i, false, ec));
            }
        }
        i += 4;
    }
    out
}

fn main() {
    use kaleido_core::save::{PkmFormat, Pokemon};
    let a: Vec<String> = std::env::args().skip(1).collect();
    let format = if a[1] == "gen7" { PkmFormat::Gen7 } else { PkmFormat::Gen6 };
    let bytes = std::fs::read(&a[0]).unwrap();
    if let Ok(at) = std::env::var("AT") {
        debug_at(&bytes, usize::from_str_radix(&at, 16).unwrap());
        return;
    }
    let hits = scan(&bytes);
    let other: std::collections::HashSet<u32> =
        a.get(2).map(|f| scan(&std::fs::read(f).unwrap()).into_iter().map(|h| h.2).collect()).unwrap_or_default();
    for (i, enc, ec) in hits {
        if other.contains(&ec) {
            continue;
        }
        let n = if i + 260 <= bytes.len() { 260 } else { 232 };
        let raw = &bytes[i..i + n];
        let p = if enc { Pokemon::from_encrypted(format, raw) } else { Pokemon::from_decrypted(format, raw) };
        let Ok(p) = p else { continue };
        println!(
            "{i:#010x} {} ec {ec:08x} esp {} N.{:?} PV {} stats {:?} TID {} SID {} pid {:08x}",
            if enc { "chiffré" } else { "clair  " },
            p.species(),
            p.party_level(),
            p.current_hp(),
            p.party_stats(),
            p.tid(),
            p.sid(),
            p.pid()
        );
    }
}
#[allow(dead_code)]
fn debug_at(bytes: &[u8], i: usize) {
    use kaleido_core::save::{PkmFormat, Pokemon};
    let p = Pokemon::from_encrypted(PkmFormat::Gen6, &bytes[i..i + 260]).unwrap();
    println!("esp {} chk {:x} calc {:x}", p.species(), p.checksum(), p.calc_checksum());
}
