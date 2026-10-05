//! Cœur de Kaleido : connaissance des jeux et détection des fichiers.

pub mod detect;
pub mod games;
pub mod saves;

pub use detect::{detect_path, Detection, FileKind};
pub use games::{Game, Platform};
