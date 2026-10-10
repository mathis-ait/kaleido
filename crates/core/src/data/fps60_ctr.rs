//! « 60 fps natif » des Pokémon 3DS (runtime `ctr-smooth`, voir `ctr-smooth/README.md`).
//!
//! La logique du jeu reste à 30 Hz ; le jeu dessine à chaque VBlank avec des matrices
//! interpolées. Le patch est un `code.ips` précompilé par `ctr-smooth/runtime/build.py`, qui
//! n'est valable que pour un `code.bin` précis : on le refuse sur toute autre version.

use crate::RomError;
use kaleido_formats::ips;
use sha2::{Digest, Sha256};

/// Un jeu couvert : Title ID, SHA-256 du `code.bin` décompressé, patch.
pub struct Profile {
    pub title_id: u64,
    pub version: &'static str,
    pub code_sha256: &'static str,
    pub ips: &'static [u8],
    /// Zones réservées au runtime (positions dans le code.bin) : marges où vit son code, et la
    /// zone où la 0.9.0 l'avait placé (une initialisation statique du jeu, appelée au démarrage,
    /// qu'elle écrasait). On les remet d'origine avant de fusionner : réinstaller répare la 0.9.0.
    pub owned: &'static [(usize, usize)],
}

/// Le même patch convient aux deux jeux : Saphir Alpha a les mêmes adresses que Rubis Oméga
/// pour tout ce que le runtime touche (voir `ctr-smooth/runtime/build.py`).
const PATCH: &[u8] = include_bytes!("../../../../ctr-smooth/runtime/build/code.ips");

pub const PROFILES: &[Profile] = &[
    Profile {
        title_id: 0x0004_0000_0011_C400,
        version: "1.0",
        code_sha256: "d587c98ac5c4dedacf9be4baf2d6b7d10169e6a63f8bc35b132002cc27bc1a37",
        ips: PATCH,
        owned: &[(0x47_9610, 0x47_A000), (0x3F_BF20, 0x3F_E790), (0x4E_BA20, 0x4E_C000)],
    },
    Profile {
        title_id: 0x0004_0000_0011_C500,
        version: "1.0",
        code_sha256: "b7f9ce60361f3709ed0ce879658afe10a712f7db060640fa72bba826301b7c16",
        ips: PATCH,
        owned: &[(0x47_9610, 0x47_A000), (0x4E_BA20, 0x4E_C000)],
    },
];

/// Jeux pour lesquels un profil existe (quelle que soit la version).
pub fn supports(title_id: u64) -> bool {
    PROFILES.iter().any(|p| p.title_id == title_id)
}

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Profil correspondant à ce `code.bin` décompressé, s'il est couvert.
pub fn profile_for(title_id: u64, code: &[u8]) -> Option<&'static Profile> {
    let sha = sha256_hex(code);
    PROFILES.iter().find(|p| p.title_id == title_id && p.code_sha256 == sha)
}

/// `code.ips` à installer : les patchs déjà présents (`existing`, ex. taux de shiny) plus le
/// lissage, en un seul fichier (Azahar et Luma n'en lisent qu'un). Refuse si les deux patchs
/// écrivent aux mêmes octets.
pub fn merged_ips(profile: &Profile, original: &[u8], existing: Option<&[u8]>) -> Result<Vec<u8>, RomError> {
    let mut patched = original.to_vec();
    if let Some(prev) = existing {
        let mine = ips::ranges(profile.ips)?;
        for (a0, a1) in ips::ranges(prev)? {
            if let Some((b0, _)) = mine.iter().find(|(b0, b1)| a0 < *b1 && *b0 < a1) {
                return Err(RomError::Unsupported(format!(
                    "le code.ips déjà installé modifie le programme du jeu au même endroit que le 60 fps natif (0x{:X})",
                    0x10_0000 + a0.max(*b0)
                )));
            }
        }
        ips::apply(&mut patched, prev)?;
    }
    ips::apply(&mut patched, profile.ips)?;
    if patched.len() != original.len() {
        return Err(RomError::Unsupported("le patch dépasse la taille du programme du jeu".into()));
    }
    Ok(ips::create(original, &patched)?)
}

/// Le patch (`ips`, appliqué au programme d'origine) contient-il le 60 fps natif ?
pub fn contains(profile: &Profile, original: &[u8], ips_file: &[u8]) -> Result<bool, RomError> {
    let mut code = original.to_vec();
    ips::apply(&mut code, ips_file)?;
    Ok(ips::records(profile.ips)?.iter().all(|(at, b)| code.get(*at..*at + b.len()) == Some(&b[..])))
}

/// Retire le 60 fps natif d'un `code.ips`, y compris une version plus ancienne du runtime
/// (crochets aux mêmes adresses, code dans les zones réservées) ; les autres patchs restent.
/// `None` : il ne reste rien à patcher.
pub fn strip(profile: &Profile, original: &[u8], ips_file: &[u8]) -> Result<Option<Vec<u8>>, RomError> {
    let mut code = original.to_vec();
    ips::apply(&mut code, ips_file)?;
    code.truncate(original.len());
    for (a, b) in ips::ranges(profile.ips)?.into_iter().chain(profile.owned.iter().copied()) {
        code[a..b].copy_from_slice(&original[a..b]);
    }
    Ok((code != original).then(|| ips::create(original, &code)).transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_ips_is_valid() {
        for p in PROFILES {
            let r = ips::ranges(p.ips).unwrap();
            assert!(!r.is_empty());
            // tout tient dans .text/.rodata/.data du code.bin (moins de 6 Mio)
            assert!(r.iter().all(|(_, end)| *end < 0x60_0000));
        }
    }

    /// Le runtime ne doit écrire que dans la marge de fin de .text et aux crochets : la zone
    /// 0x4FBF18-0x4FE790 est une initialisation statique appelée au démarrage (bug de la 0.9.0).
    #[test]
    fn patch_stays_out_of_static_initializer() {
        for p in PROFILES {
            for (a, b) in ips::ranges(p.ips).unwrap() {
                assert!(b <= 0x3F_BF18 || a >= 0x3F_E790, "écriture en 0x{:X}", a + 0x10_0000);
                assert!(b - a <= 4 || (a >= 0x47_9604 && b <= 0x47_A000), "bloc inattendu en 0x{:X} ({} octets)", a + 0x10_0000, b - a);
            }
        }
    }

    #[test]
    fn merge_rejects_overlap_and_keeps_existing() {
        let p = &PROFILES[0];
        let (at, _) = ips::records(p.ips).unwrap()[0].clone();
        let original = vec![0u8; 0x50_0000];
        // patch voisin, sans chevauchement : conservé
        let other = ips::create(&original, &{
            let mut v = original.clone();
            v[0x10] = 0xAA;
            v
        })
        .unwrap();
        let merged = merged_ips(p, &original, Some(&other)).unwrap();
        let mut out = original.clone();
        ips::apply(&mut out, &merged).unwrap();
        assert_eq!(out[0x10], 0xAA);
        // patch au même endroit : refusé
        let clash = ips::create(&original, &{
            let mut v = original.clone();
            v[at] = 0x55;
            v
        })
        .unwrap();
        assert!(merged_ips(p, &original, Some(&clash)).is_err());
        // retrait : l'autre patch reste, le lissage part
        assert!(contains(p, &original, &merged).unwrap());
        let back = strip(p, &original, &merged).unwrap().unwrap();
        assert!(!contains(p, &original, &back).unwrap());
        let mut out = original.clone();
        ips::apply(&mut out, &back).unwrap();
        assert_eq!(out[0x10], 0xAA);
        assert!(strip(p, &original, &merged_ips(p, &original, None).unwrap()).unwrap().is_none());
    }
}
