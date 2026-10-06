//! À écrire.

use std::path::Path;

use super::{MusicError, Pcm};
use crate::games::Game;

pub fn title_theme(_path: &Path, _game: Game, _max_seconds: f32) -> Result<Pcm, MusicError> {
    Err(MusicError::Unsupported("pas encore pris en charge".into()))
}
