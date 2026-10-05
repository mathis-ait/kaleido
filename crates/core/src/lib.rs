//! Cœur de Kaleido : connaissance des jeux, détection des fichiers, textes et données.

pub mod ctr_rom;
pub mod detect;
pub mod games;
pub mod pokemon;
pub mod rom;
pub mod saves;
pub mod text;

pub use ctr_rom::{CtrGameRom, CtrLayout};
pub use detect::{detect_path, Detection, FileKind};
pub use games::{Game, Platform};
pub use rom::{GameRom, RomError};
