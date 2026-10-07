//! Import / export Showdown dans la sauvegarde ouverte, et application d'un set
//! (Showdown ou Smogon) à un Pokémon existant.

use serde::{Deserialize, Serialize};

use super::session::{apply_patch, max_pp, view_of, PokemonPatch, SaveSession, Slot, SlotView};
use super::{Gender, Pokemon, SaveError, ShinyMode, BOX_SLOTS, PARTY_SLOTS};
use crate::dex::{self, Lang};
use crate::legality::{self, Change, EncounterOption, GenerateRequest, LegalizeOutcome};
use crate::showdown::{self, ResolvedSet, ShowdownSet};

/// Où ranger les Pokémon importés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ImportTarget {
    /// Cases vides à partir de cette boîte (puis les suivantes).
    Box { r#box: usize },
    /// Le premier Pokémon va dans cet emplacement (remplacé s'il est occupé),
    /// les suivants dans les cases vides des boîtes.
    Slot { slot: Slot },
    /// À la fin de l'équipe, puis dans les boîtes quand elle est complète.
    Party,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSet {
    pub species_name: String,
    /// Emplacement utilisé (`None` si le Pokémon n'a pas été ajouté).
    pub slot: Option<Slot>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
    /// Légalisation (`None` : importé tel quel).
    pub legality: Option<ImportLegality>,
}

/// Ce que la légalisation a fait d'un set importé.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLegality {
    /// Le Pokémon obtenu est légal (ou seulement douteux).
    pub legal: bool,
    /// Rencontre retenue (`None` : rencontre du Pokémon gardée).
    pub encounter: Option<EncounterOption>,
    /// Écarts avec le set (nature, Ball, attaques…) ; vide : set gardé tel quel.
    pub adjustments: Vec<Change>,
    /// Détail des réécritures (lieu, PID, dresseur…).
    pub changes: Vec<Change>,
}

impl ImportLegality {
    fn of(out: &LegalizeOutcome) -> Self {
        ImportLegality { legal: out.success, encounter: out.encounter.clone(), adjustments: out.adjustments.clone(), changes: out.changes.clone() }
    }
}

/// Demande de création légale à partir d'un set lu.
pub fn request_of_set(r: &ResolvedSet) -> GenerateRequest {
    GenerateRequest {
        species: r.species,
        form: r.form,
        level: r.level,
        shiny: Some(r.shiny),
        nature: r.nature,
        gender: r.gender,
        ability_number: r.ability_number,
        ball: r.ball,
        moves: r.moves.iter().any(|&m| m != 0).then_some(r.moves),
        ivs: Some(r.ivs),
        evs: Some(r.evs),
        held_item: Some(r.held_item),
        nickname: r.nickname.clone(),
        encounter_index: None,
        encounter_game: None,
    }
}

/// PP au maximum (3 PP Plus par attaque), comme le suppose Showdown.
fn max_out_pp(game: dex::Game, p: &mut Pokemon) {
    if p.is_egg() {
        return;
    }
    let moves = p.moves();
    let ups = moves.map(|m| if m != 0 { 3 } else { 0 });
    p.set_pp_ups(ups);
    p.set_pp(std::array::from_fn(|i| max_pp(dex::move_info_in(game, moves[i]).map_or(0, |m| m.pp), ups[i])));
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub sets: Vec<ImportedSet>,
    pub lang: Lang,
}

/// Aperçu d'un texte Showdown : Pokémon lus et convertis pour le jeu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowdownPreview {
    pub lang: Lang,
    pub sets: Vec<ResolvedSet>,
}

/// PID dont l'octet bas donne le sexe voulu (Gen 3 à 5 : sexe = octet bas comparé au taux).
/// On vise le milieu de la plage et un octet pair : le bit 0 (talent en Gen 4) reste libre.
fn pid_for_gender(pid: u32, ratio: u8, gender: Gender) -> Option<u32> {
    let low = match (ratio, gender) {
        (0 | 254 | 255, _) | (_, Gender::Genderless) => return None,
        (r, Gender::Female) => (r / 2) & !1,
        (r, Gender::Male) => ((r as u16 + 256) / 2) as u8 & !1,
    };
    Some((pid & !0xFF) | low as u32)
}

/// Modifications qui donnent au Pokémon `p` le set `r`.
/// `fresh` : Pokémon tout juste créé (tous les champs du set sont imposés).
fn patch_for(r: &ResolvedSet, p: &Pokemon, game: dex::Game, fresh: bool) -> PokemonPatch {
    let moves = if r.moves.iter().any(|&m| m != 0) { Some(r.moves) } else { None };
    let mut patch = PokemonPatch {
        species: (p.species() != r.species).then_some(r.species),
        form: Some(r.form),
        nickname: r.nickname.clone(),
        is_nicknamed: (fresh && r.nickname.is_none()).then_some(false),
        level: Some(r.level),
        nature: r.nature,
        ability_number: r.ability_number,
        ability: r.ability,
        held_item: (fresh || r.held_item != 0).then_some(r.held_item),
        ivs: Some(r.ivs),
        evs: Some(r.evs),
        friendship: r.friendship,
        ball: r.ball,
        shiny: r.shiny.then_some(ShinyMode::Star),
        ..PokemonPatch::default()
    };
    // Attaques au maximum de PP (3 PP Plus chacune, comme le suppose Showdown).
    if let Some(moves) = moves {
        let ups = moves.map(|m| if m != 0 { 3 } else { 0 });
        patch.moves = Some(moves);
        patch.pp_ups = Some(ups);
        patch.pp = Some(std::array::from_fn(|i| max_pp(dex::move_info_in(game, moves[i]).map_or(0, |m| m.pp), ups[i])));
    }
    // Sexe : stocké en Gen 6+, déduit du PID avant (le PID est retouché ; nature,
    // talent et chromatique sont réappliqués ensuite par `apply_patch`).
    if let Some(g) = r.gender {
        let ratio = dex::personal(game, r.species, r.form).map_or(127, |i| i.gender_ratio);
        if p.format().generation() <= 5 {
            if let Some(pid) = pid_for_gender(p.pid(), ratio, g).filter(|_| p.gender() != g) {
                patch.pid = Some(pid);
            }
        }
        patch.gender = Some(g);
    }
    patch
}

impl SaveSession {
    /// Lit un texte Showdown et le convertit pour le jeu de la sauvegarde, sans rien modifier.
    pub fn preview_showdown(&self, text: &str) -> ShowdownPreview {
        let sets = showdown::parse_team(text);
        let lang = showdown::guess_lang(&sets);
        let game = self.game();
        ShowdownPreview { lang, sets: sets.iter().map(|s| showdown::resolve(s, game, lang)).collect() }
    }

    /// Prochain emplacement libre pour un import.
    fn next_free(&self, target: ImportTarget, first: bool) -> Result<Option<Slot>, SaveError> {
        let start_box = match target {
            ImportTarget::Slot { slot } if first => return Ok(Some(slot)),
            ImportTarget::Party if self.save.party_count() < PARTY_SLOTS => return Ok(Some(Slot::Party { index: self.save.party_count() })),
            ImportTarget::Slot { slot: Slot::Box { r#box, .. } } | ImportTarget::Box { r#box } => r#box,
            _ => 0,
        };
        let count = self.save.box_count();
        for b in (0..count).map(|k| (start_box + k) % count.max(1)) {
            for i in 0..BOX_SLOTS {
                let slot = Slot::Box { r#box: b, index: i };
                if self.get(slot)?.is_none() {
                    return Ok(Some(slot));
                }
            }
        }
        Ok(None)
    }

    /// Crée un Pokémon conforme au set (attrapé par le dresseur de la sauvegarde).
    fn build_from_set(&self, r: &ResolvedSet) -> Result<Pokemon, SaveError> {
        let game = self.game();
        let p = self.new_pokemon(r.species, r.level)?;
        let patch = patch_for(r, &p, game, true);
        apply_patch(game, p, &patch)
    }

    /// Crée un Pokémon légal au plus près du set (rencontre, PID, Ball… choisis par le
    /// légaliseur). Si aucune rencontre ne convient, le set est importé tel quel.
    fn build_legal(&self, r: &ResolvedSet) -> Result<(Pokemon, ImportLegality), SaveError> {
        let game = self.game();
        let req = request_of_set(r);
        match legality::generate_legal(game, self.save.format(), &self.save.trainer(), &req) {
            Ok(out) if out.success => {
                let mut p = out.pokemon.clone();
                if let Some(f) = r.friendship {
                    p.set_friendship(f);
                }
                max_out_pp(game, &mut p);
                p.refresh_checksum();
                Ok((p, ImportLegality::of(&out)))
            }
            other => {
                let why = match other {
                    Err(e) => e,
                    Ok(_) => "aucune rencontre de ce jeu ne permet ce set".into(),
                };
                let p = self.build_from_set(r)?;
                let note = Change { text: format!("Importé tel quel : {why}"), term: "legalize" };
                Ok((p, ImportLegality { legal: false, encounter: None, adjustments: vec![note], changes: Vec::new() }))
            }
        }
    }

    /// Ajoute les Pokémon d'un texte Showdown, en une seule étape d'historique.
    /// `legal` : chaque Pokémon passe par le légaliseur (sinon : importé tel quel).
    pub fn import_showdown(&mut self, text: &str, target: ImportTarget, legal: bool) -> Result<ImportReport, SaveError> {
        let sets = showdown::parse_team(text);
        let lang = showdown::guess_lang(&sets);
        self.import_sets(&sets, lang, target, legal)
    }

    /// Ajoute des sets déjà lus (Showdown ou Smogon), en une seule étape d'historique.
    pub fn import_sets(&mut self, sets: &[ShowdownSet], lang: Lang, target: ImportTarget, legal: bool) -> Result<ImportReport, SaveError> {
        let game = self.game();
        let resolved: Vec<ResolvedSet> = sets.iter().map(|s| showdown::resolve(s, game, lang)).collect();
        let mut report = ImportReport { imported: 0, sets: Vec::new(), lang };
        let entry = |r: &ResolvedSet| ImportedSet {
            species_name: r.species_name.clone(),
            slot: None,
            warnings: r.warnings.clone(),
            error: r.error.clone(),
            legality: None,
        };
        if resolved.iter().all(|r| r.error.is_some()) {
            report.sets = resolved.iter().map(entry).collect();
            return Ok(report);
        }
        self.mutate(|s| {
            let mut first = true;
            for r in &resolved {
                let mut out = entry(r);
                if r.error.is_none() {
                    match s.next_free(target, first)? {
                        None => out.error = Some("Plus aucune case libre dans les boîtes".into()),
                        Some(slot) => {
                            let built = if legal {
                                s.build_legal(r).map(|(p, l)| (p, Some(l)))
                            } else {
                                s.build_from_set(r).map(|p| (p, None))
                            };
                            match built.and_then(|(p, l)| s.put(slot, p).map(|used| (used, l))) {
                                Ok((used, l)) => {
                                    out.slot = Some(used);
                                    out.legality = l;
                                    report.imported += 1;
                                    first = false;
                                }
                                Err(e) => out.error = Some(e.to_string()),
                            }
                        }
                    }
                }
                report.sets.push(out);
            }
            Ok(())
        })?;
        Ok(report)
    }

    /// Applique un set à un Pokémon existant (dresseur, rencontre et PID gardés autant que
    /// possible). `legal` : le résultat passe ensuite par « Rendre légal ».
    /// Renvoie la fiche mise à jour, les avertissements et le bilan de légalisation.
    pub fn apply_showdown_set(
        &mut self,
        slot: Slot,
        set: &ShowdownSet,
        lang: Lang,
        legal: bool,
    ) -> Result<(SlotView, Vec<String>, Option<ImportLegality>), SaveError> {
        let game = self.game();
        let r = showdown::resolve(set, game, lang);
        if let Some(e) = &r.error {
            return Err(SaveError::Invalid(e.clone()));
        }
        let trainer = self.save.trainer();
        let mut legality_out = None;
        self.mutate(|s| {
            let p = s.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
            if p.is_egg() {
                return Err(SaveError::Invalid("impossible d'appliquer un set à un œuf".into()));
            }
            let patch = patch_for(&r, &p, game, false);
            let mut p = apply_patch(game, p, &patch)?;
            if legal {
                let out = legality::legalize(&p, game, &trainer);
                let mut info = ImportLegality::of(&out);
                if out.success {
                    let req = request_of_set(&r);
                    info.adjustments = legality::legalize::deviations(game, &req, &out.pokemon).into_iter().map(|(_, t)| Change { term: legality::legalize::term_of(&t), text: t }).collect();
                    p = out.pokemon;
                    max_out_pp(game, &mut p);
                    p.refresh_checksum();
                }
                legality_out = Some(info);
            }
            s.put(slot, p).map(|_| ())
        })?;
        let p = self.get(slot)?.ok_or_else(|| SaveError::Invalid("emplacement vide".into()))?;
        Ok((view_of(game, slot, &p), r.warnings, legality_out))
    }

    /// Texte Showdown des Pokémon demandés (les œufs et cases vides sont sautés).
    pub fn export_showdown(&self, slots: &[Slot], lang: Lang) -> Result<String, SaveError> {
        let game = self.game();
        let mut sets = Vec::new();
        for &slot in slots {
            if let Some(p) = self.get(slot)?.filter(|p| !p.is_egg()) {
                sets.push(showdown::set_from_pokemon(game, &p, lang));
            }
        }
        Ok(showdown::format_team(&sets, lang))
    }
}
