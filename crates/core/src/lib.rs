//! Cœur de Kaleido : connaissance des jeux, détection des fichiers, textes et données.

pub mod detect;
pub mod games;
pub mod pokemon;
pub mod rom;
pub mod save;
pub mod saves;
pub mod text;

pub use detect::{detect_path, Detection, FileKind};
pub use games::{Game, Platform};
pub use rom::{GameRom, RomError};
