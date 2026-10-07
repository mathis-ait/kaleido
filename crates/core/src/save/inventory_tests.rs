//! Tests du sac sur des sauvegardes synthétiques, pour chaque jeu.

use super::super::{gen4, gen5, gen6, gen7};
use super::*;

const ALL: [SaveVersion; 9] = [
    SaveVersion::DiamondPearl,
    SaveVersion::Platinum,
    SaveVersion::HeartGoldSoulSilver,
    SaveVersion::BlackWhite,
    SaveVersion::Black2White2,
    SaveVersion::XY,
    SaveVersion::OmegaRubyAlphaSapphire,
    SaveVersion::SunMoon,
    SaveVersion::UltraSunUltraMoon,
];

fn blank(version: SaveVersion) -> Vec<u8> {
    match version {
        SaveVersion::RubySapphire | SaveVersion::Emerald | SaveVersion::FireRedLeafGreen => super::super::gen3::blank(version),
        SaveVersion::DiamondPearl | SaveVersion::Platinum | SaveVersion::HeartGoldSoulSilver => gen4::blank(version, 0, 0),
        SaveVersion::BlackWhite | SaveVersion::Black2White2 => gen5::blank(version),
        SaveVersion::XY => gen6::blank(0x65600, &gen6::synthetic_lengths(version)).0,
        SaveVersion::OmegaRubyAlphaSapphire => gen6::blank(0x76000, &gen6::synthetic_lengths(version)).0,
        SaveVersion::SunMoon => gen6::blank(0x6BE00, &gen7::synthetic_lengths(version)).0,
        SaveVersion::UltraSunUltraMoon => gen6::blank(0x6CC00, &gen7::synthetic_lengths(version)).0,
    }
}

fn item(id: u16, count: u16) -> InventoryItem {
    InventoryItem { id, count, is_new: false, is_favorite: false }
}

/// Taille de la zone du sac d'après PKHeX : bloc « MyItem / Inventory » (Gen 5 à 7) ;
/// en Gen 4, bloc général sans son pied.
fn bag_block_len(version: SaveVersion) -> usize {
    match version {
        SaveVersion::RubySapphire | SaveVersion::Emerald | SaveVersion::FireRedLeafGreen => 0x360,
        SaveVersion::DiamondPearl => 0xC100 - 0x14 - 0x624,
        SaveVersion::Platinum => 0xCF2C - 0x14 - 0x630,
        SaveVersion::HeartGoldSoulSilver => 0xF628 - 0x10 - 0x644,
        SaveVersion::BlackWhite => 0x9C0,
        SaveVersion::Black2White2 => 0x9EC,
        SaveVersion::XY => 0xB88,
        SaveVersion::OmegaRubyAlphaSapphire => 0xB90,
        SaveVersion::SunMoon => 0xDE0,
        SaveVersion::UltraSunUltraMoon => 0xE28,
    }
}

#[test]
fn pouches_fit_without_overlap() {
    for v in ALL {
        let (encoding, specs) = bag(v);
        let mut ranges: Vec<(usize, usize)> = specs.iter().map(|s| (s.offset, s.offset + 4 * s.capacity)).collect();
        ranges.sort();
        for w in ranges.windows(2) {
            assert!(w[0].1 <= w[1].0, "{v:?} : {w:?}");
        }
        assert!(ranges.last().unwrap().1 <= bag_block_len(v), "{v:?}");
        for s in specs {
            assert!(!s.allowed.is_empty(), "{v:?} {:?}", s.kind);
            if encoding == Encoding::Pair {
                assert_eq!(s.capacity, s.allowed.len(), "{v:?} {:?}", s.kind);
            } else {
                assert!(s.allowed.len() <= s.capacity, "{v:?} {:?}", s.kind);
                // 10 bits pour l'id et la quantité.
                assert!(s.allowed.iter().all(|&id| id <= 0x3FF) && s.max_count <= 0x3FF);
            }
            let mut sorted = s.allowed.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), s.allowed.len(), "{v:?} {:?} : doublon", s.kind);
        }
        let mut kinds: Vec<_> = specs.iter().map(|s| s.kind).collect();
        kinds.dedup();
        assert_eq!(kinds.len(), specs.len());
    }
}

#[test]
fn list_sizes_match_pkhex() {
    assert_eq!(G4_GENERAL_DP.len(), 161);
    assert_eq!(G4_GENERAL_PT.len(), 162);
    assert_eq!(G4_MACHINE.len(), 100);
    assert_eq!(G5_GENERAL.len(), 261);
    assert_eq!(XY_GENERAL.len(), 289);
    assert_eq!(AO_GENERAL.len(), 305);
    assert_eq!(SM_GENERAL.len(), 335);
    assert_eq!(USUM_ROTO, &[949, 950, 951, 952, 953, 954, 955, 956, 957, 958, 959]);
    assert_eq!(USUM_ZCRYSTAL_KEY.len(), 35);
}

/// Contenu de test d'une poche : premier objet admis (5 ou max), dernier au maximum
/// (propre à l'objet), et un du milieu à 1.
fn sample_items(v: SaveVersion, p: &Pouch) -> Vec<InventoryItem> {
    let (encoding, specs) = bag(v);
    let spec = specs.iter().find(|s| s.kind == p.kind).unwrap();
    let a = &p.allowed;
    let last = a[a.len() - 1];
    let mut out = vec![item(a[0], p.max_count.min(5)), item(last, max_count_for(encoding, spec, last))];
    if a.len() > 2 {
        out.push(item(a[a.len() / 2], 1));
    }
    out
}

#[test]
fn roundtrip_every_game() {
    for v in ALL {
        let bytes = blank(v);
        let mut save = SaveFile::from_bytes(&bytes).unwrap();
        assert_eq!(save.version(), v);
        let empty = save.inventory().unwrap();
        assert_eq!(empty.len(), bag(v).1.len(), "{v:?}");
        assert!(empty.iter().all(|p| p.items.is_empty()), "{v:?}");

        let mut pouches = empty.clone();
        for p in &mut pouches {
            p.items = sample_items(v, p);
        }
        save.set_inventory(&pouches).unwrap();
        assert_eq!(save.inventory().unwrap(), pouches, "{v:?}");

        let out = save.to_bytes();
        assert_eq!(out.len(), bytes.len());
        let mut re = SaveFile::from_bytes(&out).unwrap();
        let bad: Vec<_> = re.checksums().into_iter().filter(|c| !c.valid).collect();
        assert!(bad.is_empty(), "{v:?} : {bad:?}");
        assert_eq!(re.inventory().unwrap(), pouches, "{v:?}");

        // Retrait du premier objet de la première poche : le reste remonte et le
        // dernier emplacement est remis à zéro ; les autres poches ne bougent pas.
        let mut first = pouches[0].clone();
        first.items.remove(0);
        re.set_inventory(std::slice::from_ref(&first)).unwrap();
        let spec = bag(v).1[0];
        let base = re.layout.items + spec.offset;
        let n = first.items.len();
        assert_eq!(&re.data[base + 4 * n..base + 4 * (n + 1)], &[0; 4], "{v:?}");
        let again = SaveFile::from_bytes(&re.to_bytes()).unwrap();
        assert!(again.checksums_valid(), "{v:?}");
        let inv = again.inventory().unwrap();
        assert_eq!(inv[0], first, "{v:?}");
        assert_eq!(&inv[1..], &pouches[1..], "{v:?}");
    }
}

#[test]
fn gen4_writes_in_active_general_block() {
    for v in [SaveVersion::DiamondPearl, SaveVersion::Platinum, SaveVersion::HeartGoldSoulSilver] {
        // Bloc général le plus récent dans la 2e partition.
        let mut save = SaveFile::from_bytes(&gen4::blank(v, 1, 0)).unwrap();
        let mut inv = save.inventory().unwrap();
        inv[0].items = vec![item(68, 3)];
        save.set_inventory(&inv[..1]).unwrap();
        let out = save.to_bytes();
        let bag = gen4::PARTITION
            + match v {
                SaveVersion::DiamondPearl => 0x624,
                SaveVersion::Platinum => 0x630,
                _ => 0x644,
            };
        assert_eq!(&out[bag..bag + 4], &[68, 0, 3, 0], "{v:?}");
        assert_eq!(&out[bag - gen4::PARTITION..bag - gen4::PARTITION + 4], &[0; 4], "{v:?}");
        assert!(SaveFile::from_bytes(&out).unwrap().checksums_valid());
    }
}

#[test]
fn gen5_inventory_block_is_checksummed() {
    for (v, len) in [(SaveVersion::BlackWhite, 0x9C0), (SaveVersion::Black2White2, 0x9EC)] {
        let save = SaveFile::from_bytes(&gen5::blank(v)).unwrap();
        let sac = save.checksums().into_iter().find(|c| c.name == "sac").unwrap();
        assert_eq!((sac.offset, sac.length), (0x18400, len));
    }
}

#[test]
fn validation() {
    let mut save = SaveFile::from_bytes(&blank(SaveVersion::Platinum)).unwrap();
    let inv = save.inventory().unwrap();
    let before = save.data.clone();
    let with = |kind: PouchKind, items: Vec<InventoryItem>| {
        let mut p = inv.iter().find(|p| p.kind == kind).unwrap().clone();
        p.items = items;
        p
    };
    let invalid = |r: Result<(), SaveError>| matches!(r, Err(SaveError::Invalid(_)));

    // Objet d'une autre poche (Super Bonbon 50 dans les Poké Balls).
    assert!(invalid(save.set_inventory(&[with(PouchKind::Balls, vec![item(50, 1)])])));
    // Quantité au-delà du maximum : objets rares à 1, CT à 99, CS à 1.
    assert!(invalid(save.set_inventory(&[with(PouchKind::KeyItems, vec![item(428, 2)])])));
    assert!(invalid(save.set_inventory(&[with(PouchKind::TmHm, vec![item(328, 100)])])));
    assert!(invalid(save.set_inventory(&[with(PouchKind::TmHm, vec![item(420, 2)])])));
    save.set_inventory(&[with(PouchKind::TmHm, vec![item(328, 99), item(420, 1)])]).unwrap();
    // Doublon.
    assert!(invalid(save.set_inventory(&[with(PouchKind::Medicine, vec![item(17, 1), item(17, 2)])])));
    // Trop d'objets.
    let too_many: Vec<_> = (0..16).map(|i| item(1 + (i % 15) as u16, 1)).collect();
    assert!(invalid(save.set_inventory(&[with(PouchKind::Balls, too_many)])));
    // Poche absente du jeu, ou donnée deux fois.
    let mut z = inv[0].clone();
    z.kind = PouchKind::ZCrystals;
    assert!(invalid(save.set_inventory(&[z])));
    assert!(invalid(save.set_inventory(&[inv[0].clone(), inv[0].clone()])));
    // Une erreur dans la 2e poche n'écrit pas la 1re.
    let ok = with(PouchKind::Items, vec![item(68, 1)]);
    let bad = with(PouchKind::Balls, vec![item(50, 1)]);
    let snapshot = save.data.clone();
    assert!(invalid(save.set_inventory(&[ok, bad])));
    assert_eq!(save.data, snapshot);
    assert_ne!(save.data, before);

    // Id 0 et quantité nulle ignorés (Gen 4).
    save.set_inventory(&[with(PouchKind::Medicine, vec![item(0, 5), item(17, 0), item(18, 2)])]).unwrap();
    let med = save.inventory().unwrap().into_iter().find(|p| p.kind == PouchKind::Medicine).unwrap();
    assert_eq!(med.items, vec![item(18, 2)]);
}

#[test]
fn foreign_item_already_present_is_kept() {
    // Un objet hors liste déjà dans la sauvegarde (édition externe) n'empêche pas
    // de réécrire la poche telle quelle.
    let mut save = SaveFile::from_bytes(&blank(SaveVersion::BlackWhite)).unwrap();
    let at = save.layout.items;
    save.data[at..at + 4].copy_from_slice(&[0xE8, 0x03, 1, 0]); // objet 1000
    let inv = save.inventory().unwrap();
    assert_eq!(inv[0].items, vec![item(1000, 1)]);
    save.set_inventory(&inv).unwrap();
    assert_eq!(save.inventory().unwrap(), inv);
}

#[test]
fn gen7_flags_and_z_ring() {
    for v in [SaveVersion::SunMoon, SaveVersion::UltraSunUltraMoon] {
        let mut save = SaveFile::from_bytes(&blank(v)).unwrap();
        let (_, specs) = bag(v);
        let key = specs.iter().position(|s| s.kind == PouchKind::KeyItems).unwrap();
        let at = save.layout.items + specs[key].offset;
        // Anneau Z (797), quantité 1, indice d'espace libre 0x155, « nouveau », bit 31.
        let raw: u32 = 797 | (1 << 10) | (0x155 << 20) | NEW_FLAG | 0x8000_0000;
        save.data[at..at + 4].copy_from_slice(&raw.to_le_bytes());
        let mut inv = save.inventory().unwrap();
        assert_eq!(inv[key].items, vec![InventoryItem { id: 797, count: 1, is_new: true, is_favorite: false }]);

        // Deux Anneaux Z admis, pas trois ; « nouveau » retiré, espace libre conservé.
        inv[key].items[0].count = 3;
        assert!(matches!(save.set_inventory(&inv[key..=key]), Err(SaveError::Invalid(_))));
        inv[key].items[0].count = 2;
        inv[key].items[0].is_new = false;
        inv[key].items.push(InventoryItem { id: 216, count: 1, is_new: true, is_favorite: false });
        save.set_inventory(&inv[key..=key]).unwrap();
        let out = SaveFile::from_bytes(&save.to_bytes()).unwrap();
        assert!(out.checksums_valid());
        assert_eq!(rd_u32(&out.data, at), 797 | (2 << 10) | (0x155 << 20) | 0x8000_0000);
        assert_eq!(rd_u32(&out.data, at + 4), 216 | (1 << 10) | NEW_FLAG);
        assert_eq!(out.inventory().unwrap()[key], inv[key]);

        // Quantité nulle conservée en Gen 7.
        let mut med = inv.iter().find(|p| p.kind == PouchKind::Medicine).unwrap().clone();
        med.items = vec![item(17, 0)];
        save.set_inventory(std::slice::from_ref(&med)).unwrap();
        assert!(save.inventory().unwrap().contains(&med));
    }
    // Rotom-Pouvoirs : Ultra-Soleil/Ultra-Lune seulement.
    let sm = SaveFile::from_bytes(&blank(SaveVersion::SunMoon)).unwrap();
    assert!(sm.inventory().unwrap().iter().all(|p| p.kind != PouchKind::RotoPowers));
    let usum = SaveFile::from_bytes(&blank(SaveVersion::UltraSunUltraMoon)).unwrap();
    assert!(usum.inventory().unwrap().iter().any(|p| p.kind == PouchKind::RotoPowers));
}

#[test]
fn serializes_camel_case() {
    let save = SaveFile::from_bytes(&blank(SaveVersion::XY)).unwrap();
    let mut p = save.inventory().unwrap().remove(0);
    p.items = vec![item(1, 2)];
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["kind"], "items");
    assert_eq!(json["maxCount"], 999);
    assert_eq!(json["items"][0]["isNew"], false);
    assert_eq!(serde_json::to_value(PouchKind::TmHm).unwrap(), "tm_hm");
    let back: Pouch = serde_json::from_value(json).unwrap();
    assert_eq!(back, p);
    assert_eq!(PouchKind::RotoPowers.label_fr(), "Rotom-Pouvoirs");
}
