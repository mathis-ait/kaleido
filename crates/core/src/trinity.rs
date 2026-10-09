//! Moteur Trinity (Pokémon Écarlate / Violet, Légendes Z-A) : index `arc/data.trpfd`.
//!
//! Le jeu cherche dans ses archives `.trpfs` tout fichier dont l'empreinte figure dans
//! l'index. Pour qu'il lise un fichier « en vrac » du romfs (un mod), il faut retirer cette
//! empreinte. On réécrit donc l'index d'origine de la version installée, moins les fichiers
//! de tous les mods actifs : les mods se cumulent au lieu d'apporter chacun leur index.
//!
//! Format, d'après la description publique du format (Trinity Mod Loader, pkZukan/gftool),
//! réimplémenté ici : FlatBuffer `FileDescriptor` { 0: `[u64]` empreintes triées,
//! 1: `[string]` noms des archives, 2: `[FileInfo { 0: u64 archive, 1: u32 }]`,
//! 3: `[PackInfo { 0: u64 taille, 1: u64 nombre de fichiers }]` }. Empreinte = FNV-1a 64 bits
//! du chemin dans le romfs (`world/data/…`, casse d'origine), base 0xCBF29CE484222645.

use std::collections::HashSet;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrinityError {
    #[error("index Trinity illisible : {0}")]
    Invalid(&'static str),
}

pub type Result<T> = std::result::Result<T, TrinityError>;

const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
const FNV_BASIS: u64 = 0xCBF2_9CE4_8422_2645;

/// Empreinte d'un chemin du romfs (`world/data/encount/...`).
pub fn path_hash(path: &str) -> u64 {
    path.trim_start_matches('/').replace('\\', "/").bytes().fold(FNV_BASIS, |h, b| (h ^ b as u64).wrapping_mul(FNV_PRIME))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileInfo {
    pub pack: u64,
    pub unused: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackInfo {
    pub size: u64,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDescriptor {
    pub hashes: Vec<u64>,
    pub pack_names: Vec<String>,
    pub files: Vec<FileInfo>,
    pub packs: Vec<PackInfo>,
}

// ---------------------------------------------------------------------------
// Lecture FlatBuffers (sous-ensemble utile)

struct Buf<'a>(&'a [u8]);

impl Buf<'_> {
    fn u16(&self, at: usize) -> Result<u16> {
        self.0.get(at..at + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap())).ok_or(TrinityError::Invalid("lecture hors limites"))
    }
    fn u32(&self, at: usize) -> Result<u32> {
        self.0.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or(TrinityError::Invalid("lecture hors limites"))
    }
    fn u64(&self, at: usize) -> Result<u64> {
        self.0.get(at..at + 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).ok_or(TrinityError::Invalid("lecture hors limites"))
    }

    /// Position d'un champ d'une table, `None` s'il est absent (valeur par défaut).
    fn field(&self, table: usize, index: usize) -> Result<Option<usize>> {
        let vt = (table as i64 - self.u32(table)? as i32 as i64) as usize;
        let vt_size = self.u16(vt)? as usize;
        let slot = 4 + index * 2;
        if slot + 2 > vt_size {
            return Ok(None);
        }
        let off = self.u16(vt + slot)? as usize;
        Ok((off != 0).then_some(table + off))
    }

    /// Cible d'un décalage (`uoffset`) écrit en `at`.
    fn deref(&self, at: usize) -> Result<usize> {
        Ok(at + self.u32(at)? as usize)
    }

    /// (début des éléments, nombre) du vecteur pointé par le champ.
    fn vector(&self, table: usize, index: usize) -> Result<(usize, usize)> {
        match self.field(table, index)? {
            Some(at) => {
                let v = self.deref(at)?;
                Ok((v + 4, self.u32(v)? as usize))
            }
            None => Ok((0, 0)),
        }
    }

    fn scalar64(&self, table: usize, index: usize) -> Result<u64> {
        self.field(table, index)?.map_or(Ok(0), |at| self.u64(at))
    }

    fn scalar32(&self, table: usize, index: usize) -> Result<u32> {
        self.field(table, index)?.map_or(Ok(0), |at| self.u32(at))
    }
}

impl FileDescriptor {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let b = Buf(data);
        let root = b.deref(0)?;
        let mut d = FileDescriptor::default();
        let (h, n) = b.vector(root, 0)?;
        d.hashes = (0..n).map(|i| b.u64(h + i * 8)).collect::<Result<_>>()?;
        let (p, n) = b.vector(root, 1)?;
        for i in 0..n {
            let s = b.deref(p + i * 4)?;
            let len = b.u32(s)? as usize;
            let bytes = data.get(s + 4..s + 4 + len).ok_or(TrinityError::Invalid("nom d'archive tronqué"))?;
            d.pack_names.push(String::from_utf8_lossy(bytes).into_owned());
        }
        let (f, n) = b.vector(root, 2)?;
        for i in 0..n {
            let t = b.deref(f + i * 4)?;
            d.files.push(FileInfo { pack: b.scalar64(t, 0)?, unused: b.scalar32(t, 1)? });
        }
        let (k, n) = b.vector(root, 3)?;
        for i in 0..n {
            let t = b.deref(k + i * 4)?;
            d.packs.push(PackInfo { size: b.scalar64(t, 0)?, count: b.scalar64(t, 1)? });
        }
        if d.files.len() != d.hashes.len() {
            return Err(TrinityError::Invalid("nombre d'empreintes et d'entrées différent"));
        }
        if d.pack_names.len() != d.packs.len() {
            return Err(TrinityError::Invalid("nombre d'archives incohérent"));
        }
        Ok(d)
    }

    /// Retire les fichiers donnés (empreintes) : le jeu les lira dans le romfs. Rend le
    /// nombre d'empreintes retirées.
    pub fn remove(&mut self, hashes: &HashSet<u64>) -> usize {
        let before = self.hashes.len();
        let keep: Vec<bool> = self.hashes.iter().map(|h| !hashes.contains(h)).collect();
        let mut i = 0;
        self.hashes.retain(|_| {
            i += 1;
            keep[i - 1]
        });
        let mut j = 0;
        self.files.retain(|_| {
            j += 1;
            keep[j - 1]
        });
        before - self.hashes.len()
    }

    /// Sérialise en FlatBuffer : racine, puis table et vtable, puis vecteurs et sous-tables
    /// (les décalages non signés pointent toujours plus loin, comme l'exige le format).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(0); // racine, remplie plus bas
        // vtable de la racine : 4 champs (décalages d'un uoffset de 4 octets chacun).
        let vt = w.pos();
        w.u16(4 + 4 * 2);
        w.u16(4 + 4 * 4);
        for i in 0..4u16 {
            w.u16(4 + i * 4);
        }
        w.align(4);
        let table = w.pos();
        w.i32((table - vt) as i32);
        let slots: Vec<usize> = (0..4).map(|_| {
            let p = w.pos();
            w.u32(0);
            p
        }).collect();
        w.patch(0, table as u32);

        // 0 : empreintes (u64, alignées sur 8 : la longueur est en position ≡ 4 mod 8).
        w.align_for(8, 4);
        let v = w.pos();
        w.patch_offset(slots[0], v);
        w.u32(self.hashes.len() as u32);
        for h in &self.hashes {
            w.u64(*h);
        }

        // 1 : noms des archives.
        w.align(4);
        let names = w.pos();
        w.patch_offset(slots[1], names);
        w.u32(self.pack_names.len() as u32);
        let name_slots: Vec<usize> = self.pack_names.iter().map(|_| {
            let p = w.pos();
            w.u32(0);
            p
        }).collect();
        for (slot, name) in name_slots.iter().zip(&self.pack_names) {
            w.align(4);
            let s = w.pos();
            w.patch_offset(*slot, s);
            w.u32(name.len() as u32);
            w.bytes(name.as_bytes());
            w.u8(0);
        }

        // 2 et 3 : vecteurs de tables, chaque table avec sa vtable juste avant elle.
        let table_vector = |w: &mut Writer, slot: usize, items: &[(u64, Option<u32>, Option<u64>)]| {
            w.align(4);
            let v = w.pos();
            w.patch_offset(slot, v);
            w.u32(items.len() as u32);
            let item_slots: Vec<usize> = items.iter().map(|_| {
                let p = w.pos();
                w.u32(0);
                p
            }).collect();
            for (slot, (a, b32, b64)) in item_slots.iter().zip(items) {
                // vtable : 2 champs. Table : soffset (4), remplissage (4), u64, puis u32 ou u64.
                w.align(2);
                let vt = w.pos();
                let size = if b64.is_some() { 4 + 4 + 8 + 8 } else { 4 + 4 + 8 + 4 + 4 };
                w.u16(4 + 2 * 2);
                w.u16(size);
                w.u16(8);
                w.u16(16);
                // La table : son champ u64 en +8 doit être aligné sur 8.
                w.align_for(8, 0);
                let t = w.pos();
                w.i32((t - vt) as i32);
                w.u32(0);
                w.u64(*a);
                match (b32, b64) {
                    (_, Some(x)) => w.u64(*x),
                    (Some(x), None) => {
                        w.u32(*x);
                        w.u32(0);
                    }
                    (None, None) => w.u64(0),
                }
                w.patch_offset(*slot, t);
            }
        };
        let files: Vec<(u64, Option<u32>, Option<u64>)> = self.files.iter().map(|f| (f.pack, Some(f.unused), None)).collect();
        table_vector(&mut w, slots[2], &files);
        let packs: Vec<(u64, Option<u32>, Option<u64>)> = self.packs.iter().map(|p| (p.size, None, Some(p.count))).collect();
        table_vector(&mut w, slots[3], &packs);
        w.align(8);
        w.0
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn pos(&self) -> usize {
        self.0.len()
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
    fn align(&mut self, n: usize) {
        while self.0.len() % n != 0 {
            self.0.push(0);
        }
    }
    /// Complète jusqu'à une position ≡ `rem` modulo `n`.
    fn align_for(&mut self, n: usize, rem: usize) {
        while self.0.len() % n != rem {
            self.0.push(0);
        }
    }
    fn patch(&mut self, at: usize, v: u32) {
        self.0[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    /// Écrit en `at` le décalage (non signé, vers l'avant) jusqu'à `target`.
    fn patch_offset(&mut self, at: usize, target: usize) {
        self.patch(at, (target - at) as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> FileDescriptor {
        let mut hashes = vec![path_hash("world/data/encount/pokedata/pokedata_array.bin"), path_hash("ai/data/a.bin"), 7, 3];
        hashes.sort();
        FileDescriptor {
            files: hashes.iter().enumerate().map(|(i, _)| FileInfo { pack: (i % 2) as u64, unused: 0 }).collect(),
            hashes,
            pack_names: vec!["arc/pokemon_data.trpak".into(), "arc/world.trpak".into()],
            packs: vec![PackInfo { size: 1234, count: 2 }, PackInfo { size: 99, count: 2 }],
        }
    }

    #[test]
    fn roundtrip_and_remove() {
        let d = sample();
        let bytes = d.to_bytes();
        assert_eq!(bytes.len() % 8, 0);
        assert_eq!(FileDescriptor::parse(&bytes).unwrap(), d);
        let mut m = d.clone();
        let gone: HashSet<u64> = [path_hash("ai/data/a.bin"), 12345].into_iter().collect();
        assert_eq!(m.remove(&gone), 1);
        assert_eq!(m.hashes.len(), 3);
        assert!(!m.hashes.contains(&path_hash("ai/data/a.bin")));
        let again = FileDescriptor::parse(&m.to_bytes()).unwrap();
        assert_eq!(again, m);
        // Chaque fichier garde son archive.
        for (h, f) in m.hashes.iter().zip(&m.files) {
            let i = d.hashes.iter().position(|x| x == h).unwrap();
            assert_eq!(*f, d.files[i]);
        }
    }

    /// `KALEIDO_TRPFD_MOD=<dossier romfs d'un mod « standalone »> cargo test -p kaleido-core real_trpfd -- --ignored --nocapture`
    /// : relit son `arc/data.trpfd`, vérifie que les fichiers du mod en sont absents et que la
    /// réécriture redonne le même index.
    #[test]
    #[ignore]
    fn real_trpfd() {
        let Ok(dir) = std::env::var("KALEIDO_TRPFD_MOD") else { return };
        let root = std::path::Path::new(&dir);
        let data = std::fs::read(root.join("arc").join("data.trpfd")).unwrap();
        let d = FileDescriptor::parse(&data).unwrap();
        println!("{} fichiers indexés, {} archives ({})", d.hashes.len(), d.packs.len(), d.pack_names.first().cloned().unwrap_or_default());
        assert!(d.hashes.windows(2).all(|w| w[0] < w[1]), "empreintes triées");
        fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, out);
                } else {
                    out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut files = Vec::new();
        walk(root, root, &mut files);
        for f in files.iter().filter(|f| !f.starts_with("arc/")) {
            let present = d.hashes.binary_search(&path_hash(f)).is_ok();
            println!("{f} : {}", if present { "ENCORE DANS L'INDEX" } else { "retiré, lu en vrac" });
            assert!(!present);
        }
        assert_eq!(FileDescriptor::parse(&d.to_bytes()).unwrap(), d);
    }

    /// `KALEIDO_TRPFD_A=<data.trpfd> KALEIDO_TRPFD_B=<data.trpfd> KALEIDO_TRPFD_PATHS=<chemins séparés par ;>`
    /// : les empreintes présentes dans un index et pas dans l'autre doivent être celles des chemins donnés.
    #[test]
    #[ignore]
    fn real_trpfd_diff() {
        let (Ok(a), Ok(b), Ok(paths)) = (std::env::var("KALEIDO_TRPFD_A"), std::env::var("KALEIDO_TRPFD_B"), std::env::var("KALEIDO_TRPFD_PATHS")) else { return };
        let a = FileDescriptor::parse(&std::fs::read(a).unwrap()).unwrap();
        let b = FileDescriptor::parse(&std::fs::read(b).unwrap()).unwrap();
        let sa: HashSet<u64> = a.hashes.iter().copied().collect();
        let sb: HashSet<u64> = b.hashes.iter().copied().collect();
        let mut diff: Vec<u64> = sa.symmetric_difference(&sb).copied().collect();
        diff.sort();
        let mut expected: Vec<u64> = paths.split(';').map(path_hash).collect();
        expected.sort();
        println!("différence : {:016X?}", diff);
        println!("attendu    : {:016X?}", expected);
        assert_eq!(diff, expected);
    }

    #[test]
    fn hash_is_fnv1a_with_trinity_basis() {
        assert_eq!(path_hash(""), 0xCBF2_9CE4_8422_2645);
        assert_eq!(path_hash("a"), (0xCBF2_9CE4_8422_2645u64 ^ 0x61).wrapping_mul(FNV_PRIME));
        assert_eq!(path_hash("/world\\x"), path_hash("world/x"));
    }

    #[test]
    fn vectors_are_aligned() {
        let bytes = sample().to_bytes();
        let b = Buf(&bytes);
        let root = b.deref(0).unwrap();
        let (h, _) = b.vector(root, 0).unwrap();
        assert_eq!(h % 8, 0, "les u64 doivent être alignés sur 8");
        let (f, n) = b.vector(root, 2).unwrap();
        for i in 0..n {
            let t = b.deref(f + i * 4).unwrap();
            assert_eq!(b.field(t, 0).unwrap().unwrap() % 8, 0);
        }
    }
}

