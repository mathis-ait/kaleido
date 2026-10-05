//! Signature « MemeCrypto » des sauvegardes Soleil/Lune et Ultra-Soleil/Ultra-Lune.
//!
//! Portage de `Saves/Encryption/MemeCrypto/MemeCrypto.cs` et `MemeKey.cs` de PKHeX
//! (kwsch/PKHeX, GPLv3). Le jeu range dans le bloc 36 (à +0x100, 0x80 octets) le
//! SHA-256 de la table des blocs (0x140 octets en SL, 0x150 en USUL), chiffré et signé :
//!
//! 1. la zone est remise à zéro, puis reçoit le SHA-256 de la table (0x20 octets) ;
//! 2. les 8 derniers octets reçoivent le début du SHA-1 des 0x78 premiers ;
//! 3. les 0x60 derniers octets sont chiffrés en AES-128 (mode maison, clé = SHA-1 de
//!    la clé publique DER suivie des 0x20 premiers octets) ;
//! 4. puis signés en RSA avec la clé privée « PokedexAndSaveFile » (n° 3), connue.
//!
//! Écart volontaire avec PKHeX : le résultat RSA est complété par des zéros à gauche
//! (PKHeX écrit `BigInteger.TryWriteBytes` sans remplissage, ce qui décalerait la
//! signature dans le cas rare où elle commence par un octet nul).

use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;
use num_bigint::BigUint;
use sha1::{Digest, Sha1};
use sha2::Sha256;

use super::BlockCheck;

/// Bloc des sauvegardes SL/USUL qui contient la signature.
pub(super) const SAVE_BLOCK_INDEX: usize = 36;
/// Position de la signature dans ce bloc.
pub(super) const SIGNATURE_OFFSET: usize = 0x100;
pub(super) const SIGNATURE_LENGTH: usize = 0x80;

const SIZE_SM: usize = 0x6BE00;
const SIZE_USUM: usize = 0x6CC00;
const RSA_LENGTH: usize = 0x60;
const CHUNK: usize = 0x10;

// Vérifié : PKHeX MemeKey.cs (DER_3, clé publique « PokedexAndSaveFile »).
const DER_3: [u8; 0x7E] = [
    0x30, 0x7C, 0x30, 0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01, 0x05, 0x00, 0x03, 0x6B,
    0x00, 0x30, 0x68, 0x02, 0x61, 0x00, 0xB6, 0x1E, 0x19, 0x20, 0x91, 0xF9, 0x0A, 0x8F, 0x76, 0xA6, 0xEA, 0xAA, 0x9A,
    0x3C, 0xE5, 0x8C, 0x86, 0x3F, 0x39, 0xAE, 0x25, 0x3F, 0x03, 0x78, 0x16, 0xF5, 0x97, 0x58, 0x54, 0xE0, 0x7A, 0x9A,
    0x45, 0x66, 0x01, 0xE7, 0xC9, 0x4C, 0x29, 0x75, 0x9F, 0xE1, 0x55, 0xC0, 0x64, 0xED, 0xDF, 0xA1, 0x11, 0x44, 0x3F,
    0x81, 0xEF, 0x1A, 0x42, 0x8C, 0xF6, 0xCD, 0x32, 0xF9, 0xDA, 0xC9, 0xD4, 0x8E, 0x94, 0xCF, 0xB3, 0xF6, 0x90, 0x12,
    0x0E, 0x8E, 0x6B, 0x91, 0x11, 0xAD, 0xDA, 0xF1, 0x1E, 0x7C, 0x96, 0x20, 0x8C, 0x37, 0xC0, 0x14, 0x3F, 0xF2, 0xBF,
    0x3D, 0x7E, 0x83, 0x11, 0x41, 0xA9, 0x73, 0x02, 0x03, 0x01, 0x00, 0x01,
];

// Vérifié : PKHeX MemeKey.cs (D_3, exposant privé de la même clé).
const D_3: [u8; 0x61] = [
    0x00, 0x77, 0x54, 0x55, 0x66, 0x8F, 0xFF, 0x3C, 0xBA, 0x30, 0x26, 0xC2, 0xD0, 0xB2, 0x6B, 0x80, 0x85, 0x89, 0x59,
    0x58, 0x34, 0x11, 0x57, 0xAE, 0xB0, 0x3B, 0x6B, 0x04, 0x95, 0xEE, 0x57, 0x80, 0x3E, 0x21, 0x86, 0xEB, 0x6C, 0xB2,
    0xEB, 0x62, 0xA7, 0x1D, 0xF1, 0x8A, 0x3C, 0x9C, 0x65, 0x79, 0x07, 0x76, 0x70, 0x96, 0x1B, 0x3A, 0x61, 0x02, 0xDA,
    0xBE, 0x5A, 0x19, 0x4A, 0xB5, 0x8C, 0x32, 0x50, 0xAE, 0xD5, 0x97, 0xFC, 0x78, 0x97, 0x8A, 0x32, 0x6D, 0xB1, 0xD7,
    0xB2, 0x8D, 0xCC, 0xCB, 0x2A, 0x3E, 0x01, 0x4E, 0xDB, 0xD3, 0x97, 0xAD, 0x33, 0xB8, 0xF2, 0x8C, 0xD5, 0x25, 0x05,
    0x42, 0x51,
];

/// `x^e mod n` sur 0x60 octets (gros-boutiste, complété à gauche).
fn rsa(input: &[u8], exponent: &BigUint) -> [u8; RSA_LENGTH] {
    // Module : DER[0x18..0x79], exposant public : DER[0x7B..0x7E].
    let n = BigUint::from_bytes_be(&DER_3[0x18..0x18 + 0x61]);
    let raw = BigUint::from_bytes_be(input).modpow(exponent, &n).to_bytes_be();
    let mut out = [0u8; RSA_LENGTH];
    out[RSA_LENGTH - raw.len()..].copy_from_slice(&raw);
    out
}

fn public_exponent() -> BigUint {
    BigUint::from_bytes_be(&DER_3[0x7B..0x7E])
}

fn private_exponent() -> BigUint {
    BigUint::from_bytes_be(&D_3)
}

/// Clé AES : 16 premiers octets du SHA-1 de la clé DER suivie de la partie en clair.
fn aes_key(payload: &[u8]) -> Aes128 {
    let mut h = Sha1::new();
    h.update(DER_3);
    h.update(payload);
    let hash = h.finalize();
    Aes128::new(GenericArray::from_slice(&hash[..CHUNK]))
}

fn xor(a: &mut [u8], b: &[u8]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x ^= y;
    }
}

/// Décalage d'un bit à gauche avec réduction (`GetSubKey`, « ROL imparfait » de PKHeX).
fn sub_key(temp: &[u8; CHUNK]) -> [u8; CHUNK] {
    let mut sub = [0u8; CHUNK];
    for i in (0..CHUNK).step_by(2) {
        let (b1, b2) = (temp[i], temp[i + 1]);
        sub[i] = b1.wrapping_mul(2).wrapping_add(b2 >> 7);
        sub[i + 1] = b2.wrapping_mul(2);
        if i + 2 < CHUNK {
            sub[i + 1] = sub[i + 1].wrapping_add(temp[i + 2] >> 7);
        }
    }
    if temp[0] & 0x80 != 0 {
        sub[CHUNK - 1] ^= 0x87;
    }
    sub
}

fn aes_encrypt(data: &mut [u8]) {
    let (payload, sig) = data.split_at_mut(data.len() - RSA_LENGTH);
    let aes = aes_key(payload);
    let mut temp = [0u8; CHUNK];
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut() {
        xor(slice, &temp);
        aes.encrypt_block(GenericArray::from_mut_slice(slice));
        temp.copy_from_slice(slice);
    }
    xor(&mut temp, &sig[..CHUNK]);
    let next = sub_key(&temp);
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut() {
        xor(slice, &next);
    }
    let mut temp = [0u8; CHUNK];
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut().rev() {
        let mut plain = [0u8; CHUNK];
        plain.copy_from_slice(slice);
        aes.encrypt_block(GenericArray::from_mut_slice(slice));
        xor(slice, &temp);
        temp = plain;
    }
}

fn aes_decrypt(data: &mut [u8]) {
    let (payload, sig) = data.split_at_mut(data.len() - RSA_LENGTH);
    let aes = aes_key(payload);
    let mut temp = [0u8; CHUNK];
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut().rev() {
        xor(&mut temp, slice);
        aes.decrypt_block(GenericArray::from_mut_slice(&mut temp));
        slice.copy_from_slice(&temp);
    }
    xor(&mut temp, &sig[sig.len() - CHUNK..]);
    let next = sub_key(&temp);
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut() {
        xor(slice, &next);
    }
    let mut next = [0u8; CHUNK];
    for slice in sig.as_chunks_mut::<CHUNK>().0.iter_mut() {
        let mut cipher = [0u8; CHUNK];
        cipher.copy_from_slice(slice);
        aes.decrypt_block(GenericArray::from_mut_slice(slice));
        xor(slice, &next);
        next = cipher;
    }
}

/// `SignMemeDataInPlace` : signe `data` (au moins 0x60 octets) sur place.
fn sign_meme_data(data: &mut [u8]) {
    let n = data.len();
    let hash = Sha1::digest(&data[..n - 8]);
    data[n - 8..].copy_from_slice(&hash[..8]);
    aes_encrypt(data);
    let sig = &mut data[n - RSA_LENGTH..];
    sig[0] &= 0x7F;
    let signed = rsa(sig, &private_exponent());
    sig.copy_from_slice(&signed);
}

/// `VerifyMemeData` (clé n° 3) : données déchiffrées si la signature est valide.
fn verify_meme_data(input: &[u8]) -> Option<Vec<u8>> {
    let n = input.len();
    if n < RSA_LENGTH {
        return None;
    }
    let mut sig = rsa(&input[n - RSA_LENGTH..], &public_exponent());
    for attempt in 0..2 {
        if attempt == 1 {
            sig[0] |= 0x80;
        }
        let mut out = input.to_vec();
        out[n - RSA_LENGTH..].copy_from_slice(&sig);
        aes_decrypt(&mut out);
        let hash = Sha1::digest(&out[..n - 8]);
        if hash[..8] == out[n - 8..] {
            return Some(out);
        }
    }
    None
}

/// Zone signée et table des blocs couverte, selon la taille du fichier.
// Vérifié : PKHeX MemeCrypto.cs (SignInPlace : table de 0x140 / 0x150 octets à
// taille − 0x200, signature à 0x6BA00 / 0x6C000 + 0x100).
fn regions(len: usize) -> Option<(usize, usize, usize)> {
    match len {
        SIZE_SM => Some((0x6BA00 + SIGNATURE_OFFSET, len - 0x200, 0x140)),
        SIZE_USUM => Some((0x6C000 + SIGNATURE_OFFSET, len - 0x200, 0x150)),
        _ => None,
    }
}

/// Signe une sauvegarde SL/USUL (après le calcul des CRC). Sans effet sur les autres tailles.
pub(super) fn sign_save(data: &mut [u8]) {
    let Some((sig_at, table_at, table_len)) = regions(data.len()) else { return };
    let hash = Sha256::digest(&data[table_at..table_at + table_len]);
    let sig = &mut data[sig_at..sig_at + SIGNATURE_LENGTH];
    sig.fill(0);
    sig[..hash.len()].copy_from_slice(&hash);
    sign_meme_data(sig);
}

/// État de la signature : valide si elle se vérifie avec la clé publique et contient
/// bien le SHA-256 de la table des blocs actuelle.
pub(super) fn verify_save(data: &[u8]) -> Option<BlockCheck> {
    let (sig_at, table_at, table_len) = regions(data.len())?;
    let hash = Sha256::digest(&data[table_at..table_at + table_len]);
    let valid = verify_meme_data(&data[sig_at..sig_at + SIGNATURE_LENGTH]).is_some_and(|out| out[..hash.len()] == hash[..]);
    Some(BlockCheck {
        name: "signature MemeCrypto".into(),
        offset: sig_at,
        length: SIGNATURE_LENGTH,
        stored: 0,
        computed: 0,
        valid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify() {
        let mut data = [0u8; SIGNATURE_LENGTH];
        for (i, b) in data.iter_mut().take(0x20).enumerate() {
            *b = i as u8 * 7;
        }
        let plain = data;
        sign_meme_data(&mut data);
        assert_ne!(data, plain);
        let out = verify_meme_data(&data).expect("signature invalide");
        assert_eq!(out[..0x20], plain[..0x20]);
        // Un octet modifié : la vérification échoue.
        data[0x40] ^= 1;
        assert!(verify_meme_data(&data).is_none());
    }

    #[test]
    fn aes_roundtrip() {
        let mut data: Vec<u8> = (0..0x80u8).collect();
        let plain = data.clone();
        aes_encrypt(&mut data);
        assert_eq!(data[..0x20], plain[..0x20]);
        assert_ne!(data, plain);
        aes_decrypt(&mut data);
        assert_eq!(data, plain);
    }
}
