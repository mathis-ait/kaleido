//! Scénario du PRD : une partie de Platine rejouée en 20 sauvegardes successives (sauvegarde
//! de démo modifiée comme le ferait le jeu). Aucun faux décès, aucune capture manquée.

use super::*;
use crate::save::session::{PokemonPatch, SaveSession, Slot};

struct Game {
    s: SaveSession,
    last: Brief,
}

impl Game {
    fn new() -> Self {
        let s = SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
        let last = Brief::from(&s.live().unwrap());
        Game { s, last }
    }

    /// « Sauvegarde en jeu » : relit le fichier écrit et compare à la précédente.
    fn save(&mut self) -> Vec<GameEvent> {
        let bytes = self.s.to_bytes();
        let next = Brief::from(&SaveSession::open(&bytes).unwrap().live().unwrap());
        let events = diff(&self.last, &next);
        self.last = next;
        events
    }

    /// PV écrits tels quels, comme le jeu (l'éditeur, lui, les recalcule en plaçant un Pokémon dans l'équipe).
    fn set_hp(&mut self, slot: Slot, hp: u16) {
        let Slot::Party { index } = slot else { panic!("équipe seulement") };
        let mut p = self.s.get(slot).unwrap().unwrap();
        p.set_current_hp(hp);
        self.s.save.set_party_slot(index, Some(p)).unwrap();
    }

    fn patch(&mut self, slot: Slot, f: impl FnOnce(&mut PokemonPatch)) {
        let mut patch = PokemonPatch::default();
        f(&mut patch);
        self.s.patch(slot, &patch).unwrap();
    }
}

const P0: Slot = Slot::Party { index: 0 };
const P1: Slot = Slot::Party { index: 1 };
const P3: Slot = Slot::Party { index: 3 };
fn b(r#box: usize, index: usize) -> Slot {
    Slot::Box { r#box, index }
}

fn kinds(events: &[GameEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| match e {
            GameEvent::Caught { mon } => format!("capture {}", mon.species),
            GameEvent::Hatched { mon } => format!("éclosion {}", mon.species),
            GameEvent::Evolved { mon, .. } => format!("évolution {}", mon.species),
            GameEvent::LevelUp { mon, .. } => format!("niveau {} {}", mon.species, mon.level),
            GameEvent::Fainted { mon } => format!("ko {}", mon.species),
            GameEvent::Revived { mon } => format!("soin {}", mon.species),
            GameEvent::Died { mon, how } => format!("mort {} {how:?}", mon.species),
            GameEvent::Gone { mon } => format!("parti {}", mon.species),
            GameEvent::Badge { count } => format!("badge {count}"),
        })
        .collect()
}

#[test]
fn vingt_sauvegardes_de_platine() {
    let mut g = Game::new();
    let mut deaths = 0;
    let mut captures = 0;
    let mut check = |g: &mut Game, expected: &[&str]| {
        let got = kinds(&g.save());
        deaths += got.iter().filter(|k| k.starts_with("mort")).count();
        captures += got.iter().filter(|k| k.starts_with("capture")).count();
        assert_eq!(got, expected);
    };

    // 1. Rien n'a changé.
    check(&mut g, &[]);
    // 2. Capture d'un Étourmi (396) qui rejoint l'équipe.
    g.s.create(P3, 396, 3).unwrap();
    check(&mut g, &["capture 396"]);
    // 3. Il monte de deux niveaux.
    g.patch(P3, |p| p.level = Some(5));
    check(&mut g, &["niveau 396 5"]);
    // 4. Capture envoyée au PC (équipe pas pleine mais on range directement).
    g.s.create(b(1, 0), 399, 4).unwrap();
    check(&mut g, &["capture 399"]);
    // 5. Combat difficile : Étourmi tombe K.O. (avertissement, pas une mort).
    g.set_hp(P3, 0);
    check(&mut g, &["ko 396"]);
    // 6. Sauvegarde sans changement : rien de neuf (pas de K.O. répété).
    check(&mut g, &[]);
    // 7. Centre Pokémon : soigné.
    g.set_hp(P3, 10);
    check(&mut g, &["soin 396"]);
    // 8. Évolution en Étourvol (397).
    g.patch(P3, |p| {
        p.species = Some(397);
        p.level = Some(14);
    });
    check(&mut g, &["évolution 397", "niveau 397 14"]);
    // 9. Premier badge.
    g.s.save.set_badges(0b1);
    check(&mut g, &["badge 1"]);
    // 10. Nouveau K.O. d'Étourvol…
    g.set_hp(P3, 0);
    check(&mut g, &["ko 397"]);
    // 11. … puis déposé au PC : mort.
    g.s.move_pokemon(P3, b(2, 0)).unwrap();
    check(&mut g, &["mort 397 Deposited"]);
    // 12. Un Pokémon de boîte sorti dans l'équipe : rien à signaler.
    g.s.move_pokemon(b(1, 0), P3).unwrap();
    check(&mut g, &[]);
    // 13. Deux captures dans la même session.
    g.s.create(b(1, 1), 403, 6).unwrap();
    g.s.create(b(1, 2), 63, 7).unwrap();
    check(&mut g, &["capture 403", "capture 63"]);
    // 14. K.O. de Lucario en équipe.
    g.set_hp(P1, 0);
    check(&mut g, &["ko 448"]);
    // 15. Relâché alors qu'il est K.O. : mort.
    g.s.delete(P1).unwrap();
    check(&mut g, &["mort 448 Released"]);
    // 16. Un Pokémon en bonne santé relâché : pas une mort.
    g.s.delete(b(1, 2)).unwrap();
    check(&mut g, &["parti 63"]);
    // 17. Deux badges d'un coup.
    g.s.save.set_badges(0b111);
    check(&mut g, &["badge 2", "badge 3"]);
    // 18. Équipe réorganisée (échange de places) : rien.
    g.s.move_pokemon(P0, b(3, 0)).unwrap();
    g.s.move_pokemon(b(3, 0), P0).unwrap();
    check(&mut g, &[]);
    // 19. Montée de niveau d'un Pokémon de boîte (garderie) : signalée aussi.
    g.patch(b(1, 1), |p| p.level = Some(9));
    check(&mut g, &["niveau 403 9"]);
    // 20. Dernière capture.
    g.s.create(b(1, 3), 74, 12).unwrap();
    check(&mut g, &["capture 74"]);

    assert_eq!(deaths, 2, "exactement les deux vraies morts");
    assert_eq!(captures, 5, "aucune capture manquée");
}

#[test]
fn le_texte_des_evenements_est_lisible() {
    let mut g = Game::new();
    g.s.create(P3, 396, 3).unwrap();
    let e = g.save();
    assert_eq!(e[0].text(), "Nouveau Pokémon : Étourmi (N. 3)");
}
