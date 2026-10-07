//! Modifications de la sauvegarde hors Pokémon : carte de dresseur et noms des boîtes.

use serde::Deserialize;

use super::{strings, Gender, PkmFormat, SaveError, SaveFile};

/// Champs modifiables de la carte de dresseur (champs absents = inchangés).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrainerPatch {
    pub name: Option<String>,
    pub tid: Option<u16>,
    pub sid: Option<u16>,
    pub gender: Option<Gender>,
    pub money: Option<u32>,
    pub hours: Option<u16>,
    pub minutes: Option<u8>,
    pub seconds: Option<u8>,
}

/// Argent maximal accepté par les jeux Gen 4 à 7.
pub const MAX_MONEY: u32 = 9_999_999;

fn write(data: &mut [u8], at: usize, bytes: &[u8], what: &'static str) -> Result<(), SaveError> {
    data.get_mut(at..at + bytes.len()).ok_or(SaveError::Truncated(what))?.copy_from_slice(bytes);
    Ok(())
}

impl SaveFile {
    pub fn set_trainer(&mut self, p: &TrainerPatch) -> Result<(), SaveError> {
        let t = self.layout.trainer.clone();
        let format = self.format();
        if format.is_gb() {
            return self.set_trainer_gb(p, &t);
        }
        if let Some(name) = &p.name {
            if name.trim().is_empty() {
                return Err(SaveError::Invalid("le nom du dresseur ne peut pas être vide".into()));
            }
            let bytes = if format == PkmFormat::Gen3 {
                crate::text::gen3::encode(name, t.name_max + 1, 0xFF)
            } else {
                let mut bytes = strings::encode(format, name, t.name_max).map_err(|e| SaveError::Invalid(e.to_string()))?;
                bytes.resize(2 * (t.name_max + 1), 0);
                bytes
            };
            write(&mut self.data, t.name, &bytes, "nom du dresseur")?;
        }
        if let Some(v) = p.tid {
            write(&mut self.data, t.tid, &v.to_le_bytes(), "ID dresseur")?;
        }
        if let Some(v) = p.sid {
            write(&mut self.data, t.sid, &v.to_le_bytes(), "ID secret")?;
        }
        if let Some(g) = p.gender {
            write(&mut self.data, t.gender, &[(g == Gender::Female) as u8], "sexe du dresseur")?;
        }
        if let Some(m) = p.money {
            // Gen 3 : 999 999 au plus, stocké XOR la clé de sécurité (Émeraude, RFVF).
            let (max, key) = if format == PkmFormat::Gen3 { (999_999, super::gen3::security_key(self.version, &self.data)) } else { (MAX_MONEY, 0) };
            write(&mut self.data, t.money, &(m.min(max) ^ key).to_le_bytes(), "argent")?;
        }
        if let Some(h) = p.hours {
            write(&mut self.data, t.hours, &h.min(999).to_le_bytes(), "temps de jeu")?;
        }
        if let Some(m) = p.minutes {
            write(&mut self.data, t.minutes, &[m.min(59)], "temps de jeu")?;
        }
        if let Some(s) = p.seconds {
            write(&mut self.data, t.seconds, &[s.min(59)], "temps de jeu")?;
        }
        Ok(())
    }

    fn set_trainer_gb(&mut self, p: &TrainerPatch, t: &super::TrainerLayout) -> Result<(), SaveError> {
        let gen = self.generation();
        if let Some(name) = &p.name {
            if name.trim().is_empty() {
                return Err(SaveError::Invalid("le nom du dresseur ne peut pas être vide".into()));
            }
            let bytes = crate::text::gen12::encode(&name.to_uppercase(), t.name_max + 1, crate::text::gen12::TERMINATOR);
            write(&mut self.data, t.name, &bytes, "nom du dresseur")?;
        }
        if let Some(v) = p.tid {
            write(&mut self.data, t.tid, &v.to_be_bytes(), "ID dresseur")?;
        }
        if let Some(g) = p.gender {
            if self.version == super::SaveVersion::Crystal {
                write(&mut self.data, t.gender, &[(g == Gender::Female) as u8], "sexe du dresseur")?;
            }
        }
        if let Some(m) = p.money {
            super::gen12::set_money(gen, &mut self.data, t.money, m);
        }
        if let Some(h) = p.hours {
            if gen == 1 {
                write(&mut self.data, t.hours, &[h.min(255) as u8], "temps de jeu")?;
            } else {
                write(&mut self.data, t.hours, &h.min(999).to_be_bytes(), "temps de jeu")?;
            }
        }
        if let Some(m) = p.minutes {
            write(&mut self.data, t.minutes, &[m.min(59)], "temps de jeu")?;
        }
        if let Some(s) = p.seconds {
            write(&mut self.data, t.seconds, &[s.min(59)], "temps de jeu")?;
        }
        Ok(())
    }

    pub fn set_box_name(&mut self, index: usize, name: &str) -> Result<(), SaveError> {
        if index >= self.layout.box_count {
            return Err(SaveError::BadBox(index));
        }
        if self.format().is_gb() {
            if self.generation() == 1 {
                return Err(SaveError::Invalid("les boîtes n'ont pas de nom en Gen 1".into()));
            }
            let at = self.layout.box_names + index * 9;
            let bytes = crate::text::gen12::encode(&name.to_uppercase(), 9, crate::text::gen12::TERMINATOR);
            return write(&mut self.data, at, &bytes, "noms des boîtes");
        }
        let max = self.layout.box_name_max;
        let bytes = if self.format() == PkmFormat::Gen3 {
            crate::text::gen3::encode(name, max + 1, 0)
        } else {
            let mut bytes = strings::encode(self.format(), name, max).map_err(|e| SaveError::Invalid(e.to_string()))?;
            bytes.resize(2 * (max + 1), 0);
            bytes
        };
        let at = self.layout.box_names + index * self.layout.box_name_stride;
        write(&mut self.data, at, &bytes, "noms des boîtes")
    }

    /// Longueur maximale des noms (dresseur, boîtes), pour l'interface.
    pub fn name_limits(&self) -> (usize, usize) {
        (self.layout.trainer.name_max, self.layout.box_name_max)
    }
}
