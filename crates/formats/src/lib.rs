//! Lecture des formats de fichiers bas niveau : ROMs Nintendo DS et 3DS.
//!
//! Ce crate ne connaît rien de Pokémon : il sait seulement ouvrir des conteneurs
//! et en extraire les métadonnées. Les fichiers peuvent peser plusieurs Go, donc
//! tout passe par `Read + Seek` et on ne lit que les octets nécessaires.

pub mod ctr;
pub mod garc;
pub mod lz;
pub mod narc;
pub mod nds;
pub mod romfs;
mod util;

pub use util::stream_len;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("erreur d'entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    #[error("format invalide : {0}")]
    Invalid(&'static str),
    #[error("introuvable : {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;
