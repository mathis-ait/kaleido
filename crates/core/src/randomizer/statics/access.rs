//! Lecture et écriture des emplacements des rencontres fixes (scripts, cartes,
//! ARM9, overlays), avec les correctifs de code de l'Universal Pokémon Randomizer.

use std::collections::{BTreeMap, BTreeSet};

use kaleido_formats::narc::Narc;

use super::tables::{self, Def, Kind, Loc};
use crate::games::Game;
use crate::rom::{GameRom, RomError};

const PT_SCRIPTS: &str = "fielddata/script/scr_seq.narc";
const BW_SCRIPTS: &str = "a/0/5/7";
const BW_MAPS: &str = "a/1/2/5";
const B2W2_SCRIPTS: &str = "a/0/5/6";
const B2W2_MAPS: &str = "a/1/2/6";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Scripts,
    Maps,
    Arm9,
    Overlay(u32),
}

/// Fichiers chargés en mémoire ; seuls ceux qui ont été modifiés sont réécrits.
pub(super) struct Files {
    scripts_path: &'static str,
    scripts: Narc,
    maps_path: &'static str,
    maps: Option<Narc>,
    arm9: Option<Vec<u8>>,
    overlays: BTreeMap<u32, Vec<u8>>,
    dirty: BTreeSet<Part>,
}

fn rd16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn find_unique(data: &[u8], pattern: &[u8]) -> Option<usize> {
    let mut hits = data.windows(pattern.len()).enumerate().filter(|(_, w)| *w == pattern).map(|(i, _)| i);
    let first = hits.next()?;
    hits.next().is_none().then_some(first)
}

impl Files {
    pub(super) fn load(game: &GameRom) -> Result<Self, RomError> {
        let bw = matches!(game.game, Game::Black | Game::White);
        let b2w2 = matches!(game.game, Game::Black2 | Game::White2);
        let (scripts_path, maps_path) = if b2w2 { (B2W2_SCRIPTS, B2W2_MAPS) } else { (if bw { BW_SCRIPTS } else { PT_SCRIPTS }, BW_MAPS) };
        let mut files = Files {
            scripts_path,
            scripts: game.narc(scripts_path)?,
            maps_path,
            maps: if bw || b2w2 { Some(game.narc(maps_path)?) } else { None },
            arm9: if bw || b2w2 { None } else { Some(game.rom().arm9_decompressed()?) },
            overlays: BTreeMap::new(),
            dirty: BTreeSet::new(),
        };
        if bw {
            files.overlays.insert(tables::BW_ROAMER_OVERLAY, game.rom().overlay(tables::BW_ROAMER_OVERLAY)?);
        }
        Ok(files)
    }

    fn bytes(&self, loc: Loc) -> Option<(&[u8], usize)> {
        Some(match loc {
            Loc::Script(f, o) => (self.scripts.files.get(f as usize)?.as_slice(), o as usize),
            Loc::Map(f, o) => (self.maps.as_ref()?.files.get(f as usize)?.as_slice(), o as usize),
            Loc::Arm9(o) => (self.arm9.as_deref()?, o as usize),
            Loc::Overlay(id, o) => (self.overlays.get(&id)?.as_slice(), o as usize),
        })
    }

    fn bytes_mut(&mut self, loc: Loc) -> Option<(&mut Vec<u8>, usize)> {
        let (part, data, at) = match loc {
            Loc::Script(f, o) => (Part::Scripts, self.scripts.files.get_mut(f as usize)?, o),
            Loc::Map(f, o) => (Part::Maps, self.maps.as_mut()?.files.get_mut(f as usize)?, o),
            Loc::Arm9(o) => (Part::Arm9, self.arm9.as_mut()?, o),
            Loc::Overlay(id, o) => (Part::Overlay(id), self.overlays.get_mut(&id)?, o),
        };
        self.dirty.insert(part);
        Some((data, at as usize))
    }

    pub(super) fn u16_at(&self, loc: Loc) -> Option<u16> {
        let (d, at) = self.bytes(loc)?;
        rd16(d, at)
    }

    pub(super) fn u8_at(&self, loc: Loc) -> Option<u8> {
        let (d, at) = self.bytes(loc)?;
        d.get(at).copied()
    }

    /// Écrit un u16 ; `false` si l'emplacement est hors du fichier.
    pub(super) fn set_u16(&mut self, loc: Loc, v: u16) -> bool {
        if self.u16_at(loc).is_none() {
            return false;
        }
        let Some((d, at)) = self.bytes_mut(loc) else { return false };
        d[at..at + 2].copy_from_slice(&v.to_le_bytes());
        true
    }

    pub(super) fn set_u8(&mut self, loc: Loc, v: u8) -> bool {
        if self.u8_at(loc).is_none() {
            return false;
        }
        let Some((d, at)) = self.bytes_mut(loc) else { return false };
        d[at] = v;
        true
    }

    fn overlay_mut(&mut self, game: &GameRom, id: u32) -> Result<&mut Vec<u8>, RomError> {
        if let std::collections::btree_map::Entry::Vacant(e) = self.overlays.entry(id) {
            e.insert(game.rom().overlay(id)?);
        }
        self.dirty.insert(Part::Overlay(id));
        Ok(self.overlays.get_mut(&id).expect("overlay chargé"))
    }

    /// Réécrit dans la ROM les fichiers modifiés.
    pub(super) fn save(self, game: &mut GameRom) -> Result<(), RomError> {
        for part in &self.dirty {
            match *part {
                Part::Scripts => game.replace_narc(self.scripts_path, &self.scripts)?,
                Part::Maps => {
                    if let Some(maps) = &self.maps {
                        game.replace_narc(self.maps_path, maps)?;
                    }
                }
                Part::Arm9 => {
                    if let Some(arm9) = &self.arm9 {
                        game.rom_mut().replace_arm9(arm9)?;
                    }
                }
                Part::Overlay(id) => {
                    if let Some(data) = self.overlays.get(&id) {
                        save_overlay(game, id, data)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Vagabonds de Noire/Blanche : retrouve la fonction qui distingue Fulguris et Boréas
    /// et applique en mémoire le correctif de l'UPR (`applyBlackWhiteRoamerPatch`) :
    /// l'espèce de Fulguris devient une constante au lieu de « Boréas + 1 ».
    /// Renvoie les deux rencontres, ou la raison de l'échec.
    pub(super) fn bw_roamers(&mut self) -> Result<Vec<Def>, String> {
        let id = tables::BW_ROAMER_OVERLAY;
        let ovl = self.overlays.get_mut(&id).ok_or("overlay 10 absent")?;
        let mut patched = tables::BW_ROAMER_FUNCTION;
        patched[6] = 0x03;
        patched[7] = 0x49;
        patched[10] = 0x00;
        let base = match find_unique(ovl, &tables::BW_ROAMER_FUNCTION) {
            Some(b) => {
                // ldr r1, [pc, #0xC] → constante écrite à base + 20 (à la place de « mov r0, #2 »).
                ovl[b + 6] = 0x03;
                ovl[b + 7] = 0x49;
                ovl[b + 10] = 0x00;
                ovl[b + 20..b + 24].copy_from_slice(&642u32.to_le_bytes());
                b
            }
            None => find_unique(ovl, &patched[..20]).ok_or("fonction des vagabonds introuvable dans l'overlay 10")?,
        };
        let delta = base as i64 - tables::BW_ROAMER_FUNCTION_US as i64;
        let shift = |o: u32| (o as i64 + delta) as u32;
        let level = Loc::Overlay(id, shift(tables::BW_ROAMER_LEVEL_US));
        // Cris du script 674 : l'UPR attribue deux emplacements à chaque vagabond, mais
        // chaque version n'y met que le sien (Blanche : Fulguris partout). On rattache
        // donc chaque emplacement à l'espèce qu'il contient.
        let script_locs: Vec<Loc> =
            tables::BW_ROAMERS.iter().flat_map(|(_, _, s)| s.iter().map(|&o| Loc::Script(tables::BW_ROAMER_SCRIPT, o))).collect();
        let mut defs = Vec::new();
        for (original, ovl_offsets, _) in tables::BW_ROAMERS {
            // ROM déjà randomisée : l'espèce actuelle est dans la fonction corrigée.
            let species = self.u16_at(Loc::Overlay(id, shift(ovl_offsets[0]))).unwrap_or(original);
            let mut locs: Vec<Loc> = ovl_offsets.iter().map(|&o| Loc::Overlay(id, shift(o))).collect();
            if locs.iter().any(|&l| self.u16_at(l) != Some(species)) {
                return Err("vagabonds : valeurs inattendues dans l'overlay 10 (ROM modifiée ?)".into());
            }
            locs.extend(script_locs.iter().copied().filter(|&l| self.u16_at(l) == Some(species)));
            defs.push(Def { species: locs, levels: vec![level], kind: Kind::Roamer });
        }
        let attached: usize = defs.iter().map(|d| d.species.len()).sum::<usize>() - 2 * tables::BW_ROAMERS.len();
        if attached != script_locs.len() {
            return Err("vagabonds : script 674 inattendu".into());
        }
        // Niveau : « mov r0, #40 » (octet bas de l'instruction 0x20xx).
        if self.u8_at(level).is_none_or(|l| l == 0 || l > 100) || self.bytes(level).and_then(|(d, at)| d.get(at + 1).copied()) != Some(0x20) {
            return Err("vagabonds : niveau introuvable".into());
        }
        Ok(defs)
    }

    /// Platine : correctif du sol du Monde Distorsion (`patchDistortionWorldGroundCheck`).
    /// Sans lui, le jeu plante si Giratina est remplacé par un Pokémon sans animation d'entrée.
    pub(super) fn patch_distortion_world(&mut self, game: &GameRom) -> Result<bool, RomError> {
        let ovl = self.overlay_mut(game, tables::PT_FIELD_OVERLAY)?;
        let Some(at) = find_unique(ovl, &tables::PT_DISTORTION_PREFIX) else { return Ok(false) };
        let target = at + tables::PT_DISTORTION_PREFIX.len() + 2 * 23;
        let Some(b) = ovl.get_mut(target) else { return Ok(false) };
        *b = 0x30;
        Ok(true)
    }

    /// Noire/Blanche : le jeu vérifie en dur l'espèce de Reshiram (Noire) ou de Zekrom
    /// (Blanche) pour le placer en tête d'équipe. Portage de `fixBoxLegendaryBW1`.
    /// Renvoie le nombre de correctifs appliqués (2 attendus).
    pub(super) fn fix_box_legendary(&mut self, game: &GameRom, species: u16) -> Result<usize, RomError> {
        let black = game.game == Game::Black;
        let ovl = self.overlay_mut(game, tables::BW_FIELD_OVERLAY)?;
        let value = (species as u32).to_le_bytes();
        let mut done = 0;
        fn write(ovl: &mut [u8], at: usize, bytes: &[u8]) -> bool {
            match ovl.get_mut(at..at + bytes.len()) {
                Some(dst) => {
                    dst.copy_from_slice(bytes);
                    true
                }
                None => false,
            }
        }
        // Les préfixes de l'UPR contiennent des appels (BL) relatifs, qui changent d'une
        // langue à l'autre : à défaut, on cherche la même structure de code.
        let unique = |hits: Vec<usize>| (hits.len() == 1).then(|| hits[0]);
        let at = |d: &[u8], p: usize, bytes: &[u8]| d.get(p..p + bytes.len()) == Some(bytes);
        if black {
            // Reshiram (643) chargé depuis une constante, juste après la fin de fonction.
            for (prefix, tail) in [
                (&tables::BLACK_BOX_PREFIX1[..], &[0x07, 0xB0, 0xF0, 0xBD, 0xC0, 0x46]),
                (&tables::BLACK_BOX_PREFIX2[..], &[0x02, 0xB0, 0xF8, 0xBD, 0xC0, 0x46]),
            ] {
                let found = find_unique(ovl, prefix).map(|p| p + prefix.len()).or_else(|| {
                    unique(
                        (0..ovl.len())
                            .filter(|&p| at(ovl, p, tail) && rd16(ovl, p + 6) == Some(643) && rd16(ovl, p + 8) == Some(0))
                            .map(|p| p + 6)
                            .collect(),
                    )
                });
                if let Some(c) = found {
                    if write(ovl, c, &value) {
                        done += 1;
                    }
                }
            }
        } else {
            // Zekrom (644) obtenu par « mov r1, #161 » puis « lsl r1, r1, #2 ».
            let f1 = find_unique(ovl, &tables::WHITE_BOX_PREFIX1).map(|p| p + tables::WHITE_BOX_PREFIX1.len()).or_else(|| {
                unique(
                    (4..ovl.len())
                        .filter(|&f| at(ovl, f - 4, &[0x00, 0x20, 0x70, 0xBD]) && at(ovl, f + 18, &[0xA1, 0x21]) && at(ovl, f + 26, &[0x89, 0x00]))
                        .collect(),
                )
            });
            let f2 = find_unique(ovl, &tables::WHITE_BOX_PREFIX2).map(|p| p + tables::WHITE_BOX_PREFIX2.len()).or_else(|| {
                unique(
                    (4..ovl.len())
                        .filter(|&f| {
                            at(ovl, f - 4, &[0x70, 0xBD, 0x00, 0x00])
                                && at(ovl, f + 78, &[0xA1, 0x21, 0x89, 0x00])
                                && at(ovl, f + 502, &[0x06, 0x1C])
                                && ovl.get(f + 505) == Some(&0x4C)
                                && at(ovl, f + 556, &[0, 0, 0, 0])
                        })
                        .collect(),
                )
            });
            if let Some(f) = f1 {
                if f + 324 <= ovl.len() {
                    write(ovl, f + 66, &[0, 0]);
                    write(ovl, f + 320, &value);
                    write(ovl, f + 18, &[0, 0]);
                    write(ovl, f + 26, &[0x49, 0x49]);
                    done += 1;
                }
            }
            if let Some(f) = f2 {
                if f + 560 <= ovl.len() {
                    write(ovl, f + 502, &[0x00, 0x24, 0x06, 0x1C]);
                    write(ovl, f + 556, &value);
                    write(ovl, f + 78, &[0x77, 0x49, 0x00, 0x00]);
                    done += 1;
                }
            }
        }
        Ok(done)
    }
}

/// Réécrit un overlay. S'il est compressé (BLZ), les octets modifiés sont changés
/// directement dans le flux compressé quand c'est possible : l'overlay garde sa
/// taille, ce qui compte sur les ROMs « DSi » dont la zone DS est pleine.
/// Sinon, il est recompressé.
fn save_overlay(game: &mut GameRom, id: u32, data: &[u8]) -> Result<(), RomError> {
    let original = game.rom().overlay(id)?;
    if original == data {
        return Ok(());
    }
    let ovl = game.rom().overlays().iter().find(|o| o.id == id).cloned();
    if let Some(ovl) = ovl.filter(|o| o.is_compressed() && original.len() == data.len()) {
        if let Some(raw) = game.rom().file(ovl.file_id) {
            let mut raw = raw.to_vec();
            let in_place =
                (0..data.len()).filter(|&i| original[i] != data[i]).all(|i| kaleido_formats::lz::blz_patch_byte(&mut raw, i, data[i]).is_ok());
            if in_place && kaleido_formats::lz::decompress_blz(&raw).ok().as_deref() == Some(data) {
                game.rom_mut().replace_file(ovl.file_id, raw)?;
                return Ok(());
            }
        }
    }
    game.rom_mut().replace_overlay_recompressed(id, data.to_vec())?;
    Ok(())
}

/// Une rencontre relue dans la ROM.
#[derive(Debug, Clone)]
pub(super) struct Entry {
    /// Position dans la liste de l'UPR (`StaticPokemon{}`, puis faux objets / fossiles, puis vagabonds).
    pub index: usize,
    pub def: Def,
    pub species: u16,
    pub levels: Vec<u8>,
}

/// Relit une rencontre ; toutes les copies de l'espèce doivent concorder.
pub(super) fn read_entry(files: &Files, index: usize, def: &Def, count: u16) -> Result<Entry, String> {
    let values: Vec<Option<u16>> = def.species.iter().map(|&l| files.u16_at(l)).collect();
    let species = values.first().copied().flatten().ok_or("emplacement hors du fichier")?;
    if values.iter().any(|v| *v != Some(species)) || species == 0 || species > count {
        return Err(format!("espèces incohérentes {values:?}"));
    }
    let levels: Vec<u8> = def.levels.iter().map(|&l| files.u8_at(l).unwrap_or(0)).collect();
    if levels.iter().any(|&l| l == 0 || l > 100) {
        return Err(format!("niveaux incohérents {levels:?}"));
    }
    Ok(Entry { index, def: def.clone(), species, levels })
}

/// Toutes les rencontres du jeu, et les remarques (emplacements ignorés).
pub(super) fn entries(game: &GameRom, files: &mut Files, count: u16) -> (Vec<Entry>, Vec<String>) {
    let mut notes = Vec::new();
    let mut defs = match game.game {
        Game::Platinum => tables::platinum(),
        Game::Black | Game::White => tables::black_white(),
        Game::Black2 | Game::White2 => tables::black2_white2(),
        _ => Vec::new(),
    };
    match game.game {
        Game::Black | Game::White => match files.bw_roamers() {
            Ok(r) => defs.extend(r),
            Err(e) => notes.push(e),
        },
        Game::Platinum => {
            notes.push("vagabonds (Créfadet, Cresselia, oiseaux légendaires) non pris en charge : l'UPR ajoute pour cela du code à l'ARM9".into())
        }
        Game::Black2 | Game::White2 => notes.push("Passages Cachés et Pokémon du PWT non modifiés".into()),
        _ => {}
    }
    let mut out = Vec::new();
    for (i, def) in defs.iter().enumerate() {
        match read_entry(files, i, def, count) {
            Ok(e) => out.push(e),
            Err(e) => notes.push(format!("rencontre n°{i} ignorée : {e}")),
        }
    }
    (out, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(scripts: Vec<Vec<u8>>, overlay10: Vec<u8>) -> Files {
        Files {
            scripts_path: BW_SCRIPTS,
            scripts: Narc::from_files(scripts),
            maps_path: BW_MAPS,
            maps: None,
            arm9: None,
            overlays: BTreeMap::from([(tables::BW_ROAMER_OVERLAY, overlay10)]),
            dirty: BTreeSet::new(),
        }
    }

    #[test]
    fn entries_must_agree() {
        let mut script = vec![0u8; 0x20];
        script[4..6].copy_from_slice(&643u16.to_le_bytes());
        script[8..10].copy_from_slice(&643u16.to_le_bytes());
        script[12] = 50;
        let mut files = synthetic(vec![script], vec![]);
        let def = Def { species: vec![Loc::Script(0, 4), Loc::Script(0, 8)], levels: vec![Loc::Script(0, 12)], kind: Kind::Static };
        let e = read_entry(&files, 3, &def, 649).unwrap();
        assert_eq!((e.index, e.species, e.levels.clone()), (3, 643, vec![50]));
        assert!(files.set_u16(Loc::Script(0, 8), 1));
        assert!(read_entry(&files, 3, &def, 649).is_err());
        assert!(!files.set_u16(Loc::Script(0, 0x1F), 1), "écriture hors du fichier");
        assert!(!files.set_u8(Loc::Arm9(0), 1));
        let outside = Def { species: vec![Loc::Script(5, 0)], levels: vec![], kind: Kind::Egg };
        assert!(read_entry(&files, 0, &outside, 649).is_err());
    }

    #[test]
    fn roamers_found_with_shifted_code() {
        // Code décalé de −0x10 par rapport à la version américaine (comme Blanche FR).
        let shift = |o: u32| (o - 0x10) as usize;
        let mut ovl = vec![0u8; 0xA000];
        let base = shift(tables::BW_ROAMER_FUNCTION_US);
        ovl[base..base + 24].copy_from_slice(&tables::BW_ROAMER_FUNCTION);
        ovl[base + 24..base + 26].copy_from_slice(&641u16.to_le_bytes());
        ovl[shift(0x940C)..shift(0x940C) + 2].copy_from_slice(&642u16.to_le_bytes());
        ovl[shift(0x9410)..shift(0x9410) + 2].copy_from_slice(&641u16.to_le_bytes());
        ovl[shift(tables::BW_ROAMER_LEVEL_US)] = 40;
        ovl[shift(tables::BW_ROAMER_LEVEL_US) + 1] = 0x20;
        let mut scripts = vec![Vec::new(); 675];
        let mut s674 = vec![0u8; 0x600];
        for at in [0x572, 0x57E, 0x5DC, 0x5F1] {
            s674[at..at + 2].copy_from_slice(&642u16.to_le_bytes());
        }
        scripts[674] = s674;
        let mut files = synthetic(scripts, ovl);
        let defs = files.bw_roamers().unwrap();
        assert_eq!(defs.len(), 2);
        // Blanche : les quatre cris sont ceux de Fulguris.
        assert_eq!(defs[0].species.len(), 2 + 4);
        assert_eq!(defs[1].species.len(), 2);
        let ovl = &files.overlays[&tables::BW_ROAMER_OVERLAY];
        assert_eq!(&ovl[base + 6..base + 8], &[0x03, 0x49]);
        assert_eq!(ovl[base + 10], 0);
        assert_eq!(rd16(ovl, base + 20), Some(642));
        for d in &defs {
            assert!(read_entry(&files, 0, d, 649).is_ok());
        }
        // Une seconde lecture (ROM déjà corrigée) retrouve la fonction.
        assert_eq!(files.bw_roamers().unwrap().len(), 2);
    }
}
