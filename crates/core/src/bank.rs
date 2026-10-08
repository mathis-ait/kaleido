//! Banque Kaleido : un PC partagé par toutes les sauvegardes, comme la Banque Pokémon,
//! HOME ou le dossier de PKHeX.
//!
//! Sur le disque, un dossier contient :
//! - `pokemon/` : un fichier par Pokémon, dans son format d'origine (`.pk4` à `.pk7`,
//!   données déchiffrées, comme les fichiers exportés par PKHeX) ;
//! - `corbeille/` : les fichiers retirés ou supprimés (jamais effacés pour de bon) ;
//! - `index.json` : les boîtes (30 cases nommées, nombre illimité) et, pour chaque
//!   Pokémon, quelques informations gardées en cache pour un affichage rapide.
//!
//! Les transferts vers une sauvegarde passent par [`crate::save::convert`] et par la
//! [`SaveSession`] (annulables, sauvegarde marquée comme modifiée).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dex::{self, Game};
use crate::save::convert::{self, Compatibility, ConvertError, TransferTrainer};
use crate::save::session::{self, SaveSession, Slot, SlotView};
use crate::save::{PkmFormat, Pokemon, SaveError, BOX_SLOTS, PARTY_SLOTS};

const INDEX: &str = "index.json";
const POKEMON_DIR: &str = "pokemon";
const TRASH_DIR: &str = "corbeille";

#[derive(Debug, thiserror::Error)]
pub enum BankError {
    #[error("banque : accès au disque impossible ({0})")]
    Io(#[from] std::io::Error),
    #[error("banque : index illisible ({0})")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Convert(#[from] ConvertError),
    #[error(transparent)]
    Save(#[from] SaveError),
    #[error(transparent)]
    Pkm(#[from] crate::save::PkmError),
}

fn invalid(msg: impl Into<String>) -> BankError {
    BankError::Invalid(msg.into())
}

/// Informations gardées en cache dans l'index pour chaque Pokémon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankEntry {
    pub id: String,
    /// Nom du fichier dans `pokemon/`.
    pub file: String,
    /// Génération du format (4 = PK4 … 7 = PK7).
    pub generation: u8,
    pub species: u16,
    #[serde(default)]
    pub form: u8,
    pub nickname: String,
    pub level: u8,
    pub shiny: bool,
    #[serde(default)]
    pub is_egg: bool,
    #[serde(default)]
    pub gender: String,
    #[serde(default)]
    pub ot_name: String,
    /// Jeu de la sauvegarde d'où vient le Pokémon (ex. « Pokémon Platine »).
    #[serde(default)]
    pub origin_game: Option<String>,
    /// Nom du fichier de sauvegarde d'origine.
    #[serde(default)]
    pub origin_save: Option<String>,
    /// Date d'arrivée dans la banque (AAAA-MM-JJ).
    pub added: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankBox {
    pub name: String,
    pub slots: Vec<Option<BankEntry>>,
}

impl BankBox {
    fn new(name: String) -> Self {
        Self { name, slots: vec![None; BOX_SLOTS] }
    }

    fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BankIndex {
    version: u32,
    boxes: Vec<BankBox>,
}

/// Case de la banque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankSlot {
    pub r#box: usize,
    pub index: usize,
}

/// Résumé d'une boîte pour le sélecteur.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankBoxInfo {
    pub name: String,
    pub count: usize,
}

/// Pokémon de la banque pour l'affichage.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankSlotView {
    pub slot: BankSlot,
    #[serde(flatten)]
    pub entry: BankEntry,
    pub species_name: String,
    /// « PK4 » … « PK7 ».
    pub format: &'static str,
}

/// Fiche complète d'un Pokémon de la banque.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankDetail {
    #[serde(flatten)]
    pub item: BankSlotView,
    /// Fiche lue avec les données du jeu le plus récent de sa génération.
    pub pokemon: SlotView,
    /// Dresseur actuel (Gen 6 / 7), s'il diffère du dresseur d'origine.
    pub handler: Option<String>,
    /// Retrait vers la sauvegarde ouverte, si une sauvegarde est ouverte.
    pub compatibility: Option<Compatibility>,
}

/// Filtres de recherche.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BankQuery {
    /// Texte cherché dans le nom de l'espèce (français), le surnom ou le dresseur.
    pub text: String,
    pub shiny: Option<bool>,
    pub generation: Option<u8>,
}

/// Origine d'un dépôt (pour l'affichage).
#[derive(Debug, Clone, Default)]
pub struct Origin {
    pub game: Option<String>,
    pub save: Option<String>,
}

pub struct Bank {
    root: PathBuf,
    index: BankIndex,
}

fn format_of_gen(generation: u8) -> Option<PkmFormat> {
    match generation {
        1 => Some(PkmFormat::Gen1),
        2 => Some(PkmFormat::Gen2),
        3 => Some(PkmFormat::Gen3),
        4 => Some(PkmFormat::Gen4),
        5 => Some(PkmFormat::Gen5),
        6 => Some(PkmFormat::Gen6),
        7 => Some(PkmFormat::Gen7),
        _ => None,
    }
}

fn date_string() -> String {
    let d = session::today();
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// Sans accents et en minuscules, pour la recherche.
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Résumé à mettre en cache pour un Pokémon.
fn entry_for(p: &Pokemon, id: String, file: String, origin: &Origin) -> BankEntry {
    let game = convert::data_game(p.format());
    let growth = dex::personal(game, p.species(), p.form()).map(|i| i.growth_rate).or_else(|| crate::names::growth_rate(p.species()));
    let s = p.summary(growth);
    BankEntry {
        id,
        file,
        generation: p.format().generation(),
        species: s.species,
        form: s.form,
        nickname: s.nickname,
        level: s.level,
        shiny: s.shiny,
        is_egg: s.is_egg,
        gender: match s.gender {
            crate::save::Gender::Male => "male",
            crate::save::Gender::Female => "female",
            crate::save::Gender::Genderless => "genderless",
        }
        .into(),
        ot_name: s.ot_name,
        origin_game: origin.game.clone(),
        origin_save: origin.save.clone(),
        added: date_string(),
    }
}

impl Bank {
    /// Ouvre la banque du dossier `root` (créée vide si besoin, avec une boîte).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, BankError> {
        let root = root.into();
        std::fs::create_dir_all(root.join(POKEMON_DIR))?;
        let path = root.join(INDEX);
        let index = if path.exists() {
            let mut index: BankIndex = serde_json::from_slice(&std::fs::read(&path)?)?;
            // Index abîmé à la main : on remet 30 cases par boîte et au moins une boîte.
            for b in &mut index.boxes {
                b.slots.resize(BOX_SLOTS, None);
            }
            if index.boxes.is_empty() {
                index.boxes.push(BankBox::new("Boîte 1".into()));
            }
            index
        } else {
            BankIndex { version: 1, boxes: vec![BankBox::new("Boîte 1".into())] }
        };
        let bank = Self { root, index };
        if !path.exists() {
            bank.save_index()?;
        }
        Ok(bank)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Écrit l'index (fichier temporaire puis renommage : jamais d'index à moitié écrit).
    fn save_index(&self) -> Result<(), BankError> {
        let tmp = self.root.join("index.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.index)?)?;
        std::fs::rename(&tmp, self.root.join(INDEX))?;
        Ok(())
    }

    fn check_box(&self, b: usize) -> Result<(), BankError> {
        if b < self.index.boxes.len() {
            Ok(())
        } else {
            Err(invalid(format!("la boîte {} de la banque n'existe pas", b + 1)))
        }
    }

    fn check_slot(&self, s: BankSlot) -> Result<(), BankError> {
        self.check_box(s.r#box)?;
        if s.index < BOX_SLOTS {
            Ok(())
        } else {
            Err(invalid(format!("case {} inexistante", s.index + 1)))
        }
    }

    fn entry(&self, s: BankSlot) -> Option<&BankEntry> {
        self.index.boxes.get(s.r#box)?.slots.get(s.index)?.as_ref()
    }

    fn entry_mut(&mut self, s: BankSlot) -> &mut Option<BankEntry> {
        &mut self.index.boxes[s.r#box].slots[s.index]
    }

    /// Nombre total de Pokémon.
    pub fn count(&self) -> usize {
        self.index.boxes.iter().map(BankBox::count).sum()
    }

    pub fn boxes(&self) -> Vec<BankBoxInfo> {
        self.index.boxes.iter().map(|b| BankBoxInfo { name: b.name.clone(), count: b.count() }).collect()
    }

    fn view(&self, slot: BankSlot, e: &BankEntry) -> BankSlotView {
        BankSlotView {
            slot,
            species_name: dex::species_name(e.species).map_or_else(|| format!("n°{}", e.species), str::to_string),
            format: format_of_gen(e.generation).map_or("?", PkmFormat::label),
            entry: e.clone(),
        }
    }

    /// Les 30 cases d'une boîte.
    pub fn box_view(&self, b: usize) -> Result<Vec<Option<BankSlotView>>, BankError> {
        self.check_box(b)?;
        Ok(self.index.boxes[b].slots.iter().enumerate().map(|(i, e)| e.as_ref().map(|e| self.view(BankSlot { r#box: b, index: i }, e))).collect())
    }

    /// Lit le fichier d'un Pokémon.
    pub fn read(&self, s: BankSlot) -> Result<Pokemon, BankError> {
        self.check_slot(s)?;
        let e = self.entry(s).ok_or_else(|| invalid("case vide"))?;
        let format = format_of_gen(e.generation).ok_or_else(|| invalid("format inconnu"))?;
        let bytes = std::fs::read(self.root.join(POKEMON_DIR).join(&e.file))?;
        Ok(Pokemon::from_bytes(format, &bytes)?)
    }

    /// Fiche complète ; `game` = jeu de la sauvegarde ouverte, pour le diagnostic de retrait.
    pub fn detail(&self, s: BankSlot, game: Option<Game>) -> Result<BankDetail, BankError> {
        let p = self.read(s)?;
        let e = self.entry(s).ok_or_else(|| invalid("case vide"))?;
        let pseudo = Slot::Box { r#box: s.r#box, index: s.index };
        Ok(BankDetail {
            item: self.view(s, e),
            pokemon: session::view_of(convert::data_game(p.format()), pseudo, &p),
            handler: convert::handler(&p),
            compatibility: game.map(|g| convert::compatibility(&p, g)),
        })
    }

    /// Première case libre (une boîte est ajoutée si tout est plein).
    pub fn first_free(&mut self) -> Result<BankSlot, BankError> {
        for (b, bx) in self.index.boxes.iter().enumerate() {
            if let Some(i) = bx.slots.iter().position(Option::is_none) {
                return Ok(BankSlot { r#box: b, index: i });
            }
        }
        let b = self.add_box(None)?;
        Ok(BankSlot { r#box: b, index: 0 })
    }

    fn new_id(&self) -> String {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let mut n = nanos;
        loop {
            let id = format!("{n:x}");
            let taken = ["pk1", "pk2", "pk3", "pk4", "pk5", "pk6", "pk7"].iter().any(|ext| self.root.join(POKEMON_DIR).join(format!("{id}.{ext}")).exists());
            if !taken {
                return id;
            }
            n += 1;
        }
    }

    /// Range un Pokémon dans la banque (case donnée, ou première case libre).
    pub fn deposit(&mut self, p: &Pokemon, to: Option<BankSlot>, origin: &Origin) -> Result<BankSlot, BankError> {
        if p.is_empty() {
            return Err(invalid("emplacement vide"));
        }
        let slot = match to {
            Some(s) => {
                self.check_slot(s)?;
                if self.entry(s).is_some() {
                    return Err(invalid("cette case de la banque est déjà occupée"));
                }
                s
            }
            None => self.first_free()?,
        };
        let id = self.new_id();
        let file = format!("{id}.pk{}", p.format().generation());
        let mut stored = Pokemon::from_decrypted(p.format(), &p.stored_data())?;
        stored.refresh_checksum();
        std::fs::write(self.root.join(POKEMON_DIR).join(&file), stored.stored_data())?;
        *self.entry_mut(slot) = Some(entry_for(&stored, id, file, origin));
        if let Err(e) = self.save_index() {
            *self.entry_mut(slot) = None;
            return Err(e);
        }
        Ok(slot)
    }

    /// Retire un Pokémon de l'index ; son fichier part dans la corbeille.
    pub fn remove(&mut self, s: BankSlot) -> Result<(), BankError> {
        self.check_slot(s)?;
        let e = self.entry_mut(s).take().ok_or_else(|| invalid("case vide"))?;
        self.save_index()?;
        let trash = self.root.join(TRASH_DIR);
        std::fs::create_dir_all(&trash)?;
        let src = self.root.join(POKEMON_DIR).join(&e.file);
        if src.exists() {
            std::fs::rename(&src, trash.join(&e.file))?;
        }
        Ok(())
    }

    /// Déplace (ou échange) deux cases de la banque.
    pub fn move_slot(&mut self, from: BankSlot, to: BankSlot) -> Result<(), BankError> {
        self.check_slot(from)?;
        self.check_slot(to)?;
        if from == to {
            return Ok(());
        }
        let a = self.entry_mut(from).take();
        let b = self.entry_mut(to).take();
        *self.entry_mut(to) = a;
        *self.entry_mut(from) = b;
        self.save_index()
    }

    /// Ajoute une boîte à la fin ; renvoie son numéro.
    pub fn add_box(&mut self, name: Option<String>) -> Result<usize, BankError> {
        let n = self.index.boxes.len();
        let name = name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Boîte {}", n + 1));
        self.index.boxes.push(BankBox::new(name));
        self.save_index()?;
        Ok(n)
    }

    pub fn rename_box(&mut self, b: usize, name: &str) -> Result<(), BankError> {
        self.check_box(b)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("le nom de la boîte ne peut pas être vide"));
        }
        if name.chars().count() > 40 {
            return Err(invalid("nom trop long (40 caractères au maximum)"));
        }
        self.index.boxes[b].name = name.to_string();
        self.save_index()
    }

    /// Supprime une boîte vide (il en reste toujours au moins une).
    pub fn delete_box(&mut self, b: usize) -> Result<(), BankError> {
        self.check_box(b)?;
        if self.index.boxes[b].count() > 0 {
            return Err(invalid("vide d'abord cette boîte : elle contient encore des Pokémon"));
        }
        if self.index.boxes.len() == 1 {
            return Err(invalid("la banque doit garder au moins une boîte"));
        }
        self.index.boxes.remove(b);
        self.save_index()
    }

    /// Recherche dans toute la banque.
    pub fn search(&self, q: &BankQuery) -> Vec<BankSlotView> {
        let text = fold(q.text.trim());
        let mut out = Vec::new();
        for (b, bx) in self.index.boxes.iter().enumerate() {
            for (i, e) in bx.slots.iter().enumerate() {
                let Some(e) = e else { continue };
                if q.shiny.is_some_and(|s| s != e.shiny) || q.generation.is_some_and(|g| g != e.generation) {
                    continue;
                }
                let v = self.view(BankSlot { r#box: b, index: i }, e);
                if !text.is_empty() && ![&v.species_name, &e.nickname, &e.ot_name].iter().any(|s| fold(s).contains(&text)) {
                    continue;
                }
                out.push(v);
            }
        }
        out
    }

    /// Importe un fichier `.pk3` à `.pk7` (format déduit de l'extension, sinon de la taille).
    pub fn import_bytes(&mut self, bytes: &[u8], ext: Option<&str>, to: Option<BankSlot>, origin: &Origin) -> Result<BankSlot, BankError> {
        let format = match convert::format_from_file(ext, bytes.len()) {
            Some(f) => f,
            // 232 / 260 octets sans extension : Gen 7 si la version d'origine est 3DS récente.
            None if bytes.len() == 232 || bytes.len() == 260 => {
                let p = Pokemon::from_bytes(PkmFormat::Gen6, bytes)?;
                if p.version() >= 30 {
                    PkmFormat::Gen7
                } else {
                    PkmFormat::Gen6
                }
            }
            None => return Err(invalid(format!("fichier Pokémon non reconnu ({} octets)", bytes.len()))),
        };
        let p = Pokemon::from_bytes(format, bytes)?;
        if p.is_empty() {
            return Err(invalid("fichier Pokémon vide"));
        }
        self.deposit(&p, to, origin)
    }

    /// Données déchiffrées et extension (`pk4` … `pk7`) pour un export.
    pub fn export(&self, s: BankSlot) -> Result<(Vec<u8>, String), BankError> {
        let p = self.read(s)?;
        Ok((p.stored_data().to_vec(), format!("pk{}", p.format().generation())))
    }
}

// =====================================================================================
// Échanges avec une sauvegarde ouverte
// =====================================================================================

/// Dépose un Pokémon de la sauvegarde dans la banque. `copy = false` : il quitte la
/// sauvegarde (une seule étape d'annulation côté sauvegarde).
pub fn deposit_from_save(
    bank: &mut Bank,
    s: &mut SaveSession,
    from: Slot,
    to: Option<BankSlot>,
    copy: bool,
    save_name: Option<String>,
) -> Result<BankSlot, BankError> {
    let p = s.get(from)?.ok_or_else(|| invalid("emplacement vide"))?;
    if !copy && matches!(from, Slot::Party { .. }) && s.save.party_count() <= 1 {
        return Err(invalid("l'équipe doit garder au moins un Pokémon : dépose-le en copie (Maj) ou ajoute d'abord un autre Pokémon à l'équipe"));
    }
    let origin = Origin { game: Some(s.save.version().label().to_string()), save: save_name };
    let slot = bank.deposit(&p, to, &origin)?;
    if !copy {
        if let Err(e) = s.delete(from) {
            // Échec côté sauvegarde : on annule le dépôt.
            let _ = bank.remove(slot);
            return Err(e.into());
        }
    }
    Ok(slot)
}

/// Retire un Pokémon de la banque vers la sauvegarde, converti au format du jeu.
/// `copy = false` : il quitte la banque (fichier gardé dans la corbeille). `strip` :
/// retire les attaques / l'objet absents du jeu au lieu de refuser.
pub fn withdraw_to_save(bank: &mut Bank, s: &mut SaveSession, from: BankSlot, to: Slot, copy: bool, strip: bool) -> Result<SlotView, BankError> {
    let p = bank.read(from)?;
    let target = match to {
        Slot::Party { index } if index >= s.save.party_count() => {
            if s.save.party_count() >= PARTY_SLOTS {
                return Err(invalid("l'équipe est complète"));
            }
            Slot::Party { index: s.save.party_count() }
        }
        t => t,
    };
    if s.get(target)?.is_some() {
        return Err(invalid("cette case de la sauvegarde est occupée : choisis une case vide"));
    }
    let trainer = TransferTrainer::from(&s.save.trainer());
    let converted = convert::convert_for(&p, s.game(), Some(&trainer), strip)?;
    let view = s.import(target, &converted.stored_data())?;
    if !copy {
        bank.remove(from)?;
    }
    Ok(view)
}

#[cfg(test)]
#[path = "bank_tests.rs"]
mod tests;
