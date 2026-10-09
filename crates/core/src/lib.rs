//! Cœur de Kaleido : connaissance des jeux, détection des fichiers, textes et données.

pub mod bank;
pub mod battle;
pub mod ctr_rom;
pub mod data;
pub mod detect;
pub mod dex;
pub mod games;
pub mod gb_rom;
pub mod gba_rom;
pub mod gifts;
pub mod legality;
pub mod live;
pub mod music;
pub mod names;
pub mod nx;
pub mod trinity;
pub mod nuzlocke;
pub mod pokemon;
pub mod randomizer;
pub mod rom;
pub mod romedit;
pub mod romedit_ctr;
pub mod romedit_gb;
pub mod romedit_gba;
pub mod romhack;
pub mod save;
pub mod saves;
pub mod showdown;
pub mod text;

pub use ctr_rom::{CtrGameRom, CtrLayout};
pub use detect::{detect_path, Detection, FileKind};
pub use games::{Game, Platform};
pub use gb_rom::GbGameRom;
pub use gba_rom::GbaGameRom;
pub use rom::{GameRom, RomError};
/// Formats bruts (RomFS, NARC…), pour l'application.
pub use kaleido_formats as formats;

/// Vraie ROM pour les tests (ignorés si elle est absente) : dossier `KALEIDO_ROMS`,
/// sinon `~/Documents/NDS & 3DS`.
#[cfg(test)]
pub(crate) fn test_rom_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::var_os("KALEIDO_ROMS").map(std::path::PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
        std::path::PathBuf::from(home).join("Documents").join("NDS & 3DS")
    });
    dir.join(name)
}
