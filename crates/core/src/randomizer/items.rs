//! Objets ramassables et boutiques (Platine, Noire, Blanche).
//!
//! Portage de `ItemRandomizer` de l'Universal Pokémon Randomizer :
//!
//! - objets ramassables : seuls les objets « autorisés » changent (jamais les objets
//!   rares, les CS ni les objets inutilisés) ; les CT restent des CT, et en mode
//!   aléatoire toutes les CT nécessaires (`RequiredFieldTMs` de l'UPR) restent au sol ;
//! - boutiques : seuls les comptoirs secondaires changent ; les comptoirs principaux
//!   (Poké Balls, Potions…) et les boutiques de CT sont conservés, comme dans l'UPR.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::ops::RangeInclusive;

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::rng_for;
use crate::data::field_items::{self, FieldItem, ItemLayout};
use crate::data::shops::{self, ShopKind};
use crate::rom::{GameRom, RomError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FieldItemsMode {
    #[default]
    Unchanged,
    /// Les objets existants sont mélangés (les CT entre elles).
    Shuffle,
    /// Objets tirés au hasard (une CT est remplacée par une CT).
    Random,
    /// Comme `Random`, mais chaque objet possible sort autant de fois avant qu'un objet revienne.
    RandomEven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShopsMode {
    #[default]
    Unchanged,
    /// Les objets des comptoirs secondaires sont mélangés entre eux.
    Shuffle,
    /// Objets des comptoirs secondaires tirés au hasard.
    Random,
}

/// Réglages des objets ramassables et des boutiques.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemSettings {
    pub field_items: FieldItemsMode,
    /// Objets ramassables aléatoires : exclut les objets peu utiles (lettres, baies sans effet en combat…).
    pub ban_bad_field_items: bool,
    pub shops: ShopsMode,
    /// Boutiques aléatoires : exclut les objets peu utiles.
    pub ban_bad_shop_items: bool,
    /// Boutiques aléatoires : exclut les objets des comptoirs principaux (Poké Balls, Potions…).
    pub ban_regular_shop_items: bool,
    /// Boutiques aléatoires : exclut les objets trop forts ou revendables cher (Super Bonbon, Pépite…).
    pub ban_op_shop_items: bool,
    /// Boutiques aléatoires : toutes les pierres et objets d'évolution sont en vente.
    pub guarantee_evolution_items: bool,
    /// Boutiques aléatoires : tous les objets X (Attaque +, Défense +…) sont en vente.
    pub guarantee_x_items: bool,
    /// Aucun Super Bonbon parmi les objets mélangés ou tirés au hasard.
    pub no_rare_candy: bool,
    /// Aucune Master Ball parmi les objets mélangés ou tirés au hasard.
    pub no_master_ball: bool,
}

impl ItemSettings {
    pub fn changes_anything(&self) -> bool {
        self.field_items != FieldItemsMode::Unchanged || self.shops != ShopsMode::Unchanged
    }
}

const MASTER_BALL: u16 = 1;
const RARE_CANDY: u16 = 50;

/// Catégories d'objets (identifiants communs aux Gen 4 et 5, `ItemIDs` de l'UPR).
pub(crate) struct ItemCatalog {
    gen: u8,
    max: u16,
    unnamed: BTreeSet<u16>,
    /// Noire 2 / Blanche 2 : autres CT requises au sol, tessons utiles (UPR `Type_BW2`).
    b2w2: bool,
}

fn any_in(id: u16, ranges: &[RangeInclusive<u16>]) -> bool {
    ranges.iter().any(|r| r.contains(&id))
}

impl ItemCatalog {
    pub(crate) fn new(gen: u8, names: &[String]) -> Self {
        let unnamed = names.iter().enumerate().filter(|(_, n)| n.trim().is_empty() || n.trim() == "???").map(|(i, _)| i as u16).collect();
        Self { gen, max: names.len().saturating_sub(1) as u16, unnamed, b2w2: false }
    }

    /// Règles de Noire 2 / Blanche 2.
    pub(crate) fn with_b2w2(mut self, b2w2: bool) -> Self {
        self.b2w2 = b2w2;
        self
    }

    pub(crate) fn is_tm(&self, id: u16) -> bool {
        (328..=419).contains(&id) || (self.gen >= 5 && (618..=620).contains(&id))
    }

    /// Objets jamais déplacés ni distribués : objets rares, CS, objets inutilisés (`bannedItems`).
    fn is_banned(&self, id: u16) -> bool {
        let common: &[RangeInclusive<u16>] = &[0..=0, 420..=427];
        if any_in(id, common) || id > self.max || self.unnamed.contains(&id) {
            return true;
        }
        if self.gen == 4 {
            any_in(id, &[428..=536, 112..=134])
        } else {
            any_in(id, &[428..=503, 505..=536, 621..=638, 113..=115, 120..=133, 592..=615, 616..=617, 574..=574, 578..=579])
        }
    }

    pub(crate) fn is_allowed(&self, id: u16) -> bool {
        !self.is_banned(id)
    }

    /// Objets peu utiles (`badItems`) : lettres, paillis, baies sans effet en combat…
    fn is_bad(&self, id: u16) -> bool {
        let common: &[RangeInclusive<u16>] =
            &[111..=112, 135..=136, 225..=225, 236..=236, 155..=155, 274..=274, 95..=98, 137..=148, 159..=183, 256..=259, 260..=264];
        if any_in(id, common) {
            return true;
        }
        if self.gen == 4 {
            (70..=71).contains(&id)
        } else if self.b2w2 {
            // Les tessons s'échangent contre des donneurs de capacités en Noire 2 / Blanche 2.
            any_in(id, &[571..=571, 575..=575])
        } else {
            any_in(id, &[571..=571, 575..=575, 72..=75])
        }
    }

    /// Objets trop forts ou revendables cher (`opShopItems`).
    fn is_op(&self, id: u16) -> bool {
        if any_in(id, &[50..=50, 86..=92, 106..=106, 231..=231]) {
            return true;
        }
        self.gen >= 5 && any_in(id, &[42..=43, 54..=54, 65..=71, 206..=212, 571..=571, 580..=591])
    }

    /// Objets des comptoirs principaux (`regularShopItems`).
    fn is_regular(&self, id: u16) -> bool {
        any_in(id, &[2..=4, 17..=28, 76..=79])
    }

    fn evolution_items(&self) -> Vec<u16> {
        let mut v: Vec<u16> = [80..=85, 107..=110, 321..=327].into_iter().flatten().collect();
        v.extend([221, 226, 227, 233, 235, 252]);
        if self.gen >= 5 {
            v.push(537); // Bel'Écaille
        }
        v.into_iter().filter(|&i| self.is_allowed(i)).collect()
    }

    fn x_items(&self) -> Vec<u16> {
        (55..=62).filter(|&i| self.is_allowed(i)).collect()
    }

    /// CT à garder au sol en mode aléatoire (`RequiredFieldTMs` de l'UPR).
    fn required_tms(&self) -> Vec<u16> {
        let numbers: &[u16] = if self.gen == 4 {
            &[2, 3, 5, 7, 9, 11, 12, 18, 19, 23, 28, 34, 37, 39, 41, 43, 46, 47, 49, 50, 62, 69, 79, 80, 82, 84, 85, 87]
        } else if self.b2w2 {
            &[
                1, 2, 3, 5, 6, 12, 13, 19, 22, 26, 28, 29, 30, 36, 39, 41, 46, 47, 50, 52, 53, 56, 58, 61, 63, 65, 66, 67, 69, 71, 80, 81, 84, 85,
                86, 90, 91, 92, 93,
            ]
        } else {
            &[
                2, 3, 5, 6, 9, 12, 13, 19, 22, 24, 26, 29, 30, 35, 36, 39, 41, 46, 47, 50, 52, 53, 55, 58, 61, 63, 65, 66, 71, 80, 81, 84, 85, 86,
                90, 91, 92, 93,
            ]
        };
        numbers.iter().map(|&n| tm_item(n)).collect()
    }

    fn all_tms(&self) -> Vec<u16> {
        (1..=if self.gen >= 5 { 95 } else { 92 }).map(tm_item).collect()
    }

    fn excluded_by_user(settings: &ItemSettings, id: u16) -> bool {
        (settings.no_rare_candy && id == RARE_CANDY) || (settings.no_master_ball && id == MASTER_BALL)
    }

    /// Objets possibles (hors CT) : autorisés, éventuellement sans les objets peu utiles.
    fn pool(&self, ban_bad: bool, settings: &ItemSettings) -> Vec<u16> {
        (1..=self.max)
            .filter(|&i| self.is_allowed(i) && !self.is_tm(i) && !(ban_bad && self.is_bad(i)) && !Self::excluded_by_user(settings, i))
            .collect()
    }
}

/// Identifiant de la CT n° `n` (CT93 à CT95 : Noire/Blanche).
fn tm_item(n: u16) -> u16 {
    if n <= 92 {
        327 + n
    } else {
        617 + (n - 92)
    }
}

/// `count` objets tirés de `pool` : au hasard, ou en épuisant le stock avant de recommencer.
fn draw(pool: &[u16], count: usize, even: bool, rng: &mut impl Rng) -> Vec<u16> {
    if pool.is_empty() {
        return Vec::new();
    }
    if !even {
        return (0..count).map(|_| pool[rng.gen_range(0..pool.len())]).collect();
    }
    let mut out = Vec::with_capacity(count);
    let mut stock: Vec<u16> = Vec::new();
    while out.len() < count {
        if stock.is_empty() {
            stock = pool.to_vec();
            stock.shuffle(rng);
        }
        out.extend(stock.pop());
    }
    out
}

/// Nouvelle liste d'objets ramassables (même ordre). Les objets non autorisés restent en place.
pub(crate) fn new_field_items(catalog: &ItemCatalog, settings: &ItemSettings, current: &[u16], seed: u64) -> Vec<u16> {
    let mut rng = rng_for(seed, "field_items");
    let movable: Vec<usize> = (0..current.len()).filter(|&i| catalog.is_allowed(current[i])).collect();
    let (tm_slots, other_slots): (Vec<usize>, Vec<usize>) = movable.iter().partition(|&&i| catalog.is_tm(current[i]));
    let mut tms: Vec<u16> = tm_slots.iter().map(|&i| current[i]).collect();
    let mut others: Vec<u16> = other_slots.iter().map(|&i| current[i]).collect();
    let pool = catalog.pool(settings.ban_bad_field_items, settings);

    match settings.field_items {
        FieldItemsMode::Unchanged => return current.to_vec(),
        FieldItemsMode::Shuffle => {
            tms.shuffle(&mut rng);
            others.shuffle(&mut rng);
            // Objets interdits par le joueur : remplacés par un objet au hasard.
            for item in others.iter_mut().filter(|i| ItemCatalog::excluded_by_user(settings, **i)) {
                if let Some(&new) = pool.choose(&mut rng) {
                    *item = new;
                }
            }
        }
        FieldItemsMode::Random | FieldItemsMode::RandomEven => {
            let needed = tms.len();
            let mut chosen: Vec<u16> = catalog.required_tms();
            let all = catalog.all_tms();
            if chosen.len() <= needed {
                let mut rest: Vec<u16> = all.into_iter().filter(|t| !chosen.contains(t)).collect();
                rest.shuffle(&mut rng);
                chosen.extend(rest.into_iter().take(needed - chosen.len()));
                if chosen.len() == needed {
                    tms = chosen;
                }
            }
            // Sinon (trop peu d'emplacements) : les CT d'origine sont simplement mélangées.
            tms.shuffle(&mut rng);
            others = draw(&pool, others.len(), settings.field_items == FieldItemsMode::RandomEven, &mut rng);
            if others.len() != other_slots.len() {
                others = other_slots.iter().map(|&i| current[i]).collect();
            }
        }
    }

    let mut out = current.to_vec();
    for (&slot, item) in tm_slots.iter().zip(tms) {
        out[slot] = item;
    }
    for (&slot, item) in other_slots.iter().zip(others) {
        out[slot] = item;
    }
    out
}

/// Nouveaux objets des boutiques ; `shops` : (genre, aventure principale, objets).
pub(crate) fn new_shop_items(catalog: &ItemCatalog, settings: &ItemSettings, shops: &[(ShopKind, bool, Vec<u16>)], seed: u64) -> Vec<Vec<u16>> {
    let mut rng = rng_for(seed, "shops");
    let mut out: Vec<Vec<u16>> = shops.iter().map(|s| s.2.clone()).collect();
    let special: Vec<usize> = (0..shops.len()).filter(|&i| shops[i].0 == ShopKind::Special).collect();
    let total: usize = special.iter().map(|&i| shops[i].2.len()).sum();

    match settings.shops {
        ShopsMode::Unchanged => {}
        ShopsMode::Shuffle => {
            let mut all: Vec<u16> = special.iter().flat_map(|&i| shops[i].2.iter().copied()).collect();
            all.shuffle(&mut rng);
            let mut it = all.into_iter();
            for &i in &special {
                for slot in out[i].iter_mut() {
                    *slot = it.next().unwrap_or(*slot);
                }
            }
        }
        ShopsMode::Random => {
            let mut guaranteed: Vec<u16> = Vec::new();
            if settings.guarantee_evolution_items {
                guaranteed.extend(catalog.evolution_items());
            }
            if settings.guarantee_x_items {
                guaranteed.extend(catalog.x_items());
            }
            guaranteed.retain(|&i| !ItemCatalog::excluded_by_user(settings, i));
            guaranteed.dedup();
            let main_slots: usize = special.iter().filter(|&&i| shops[i].1).map(|&i| shops[i].2.len()).sum();
            guaranteed.truncate(main_slots);

            let possible: Vec<u16> = catalog
                .pool(settings.ban_bad_shop_items, settings)
                .into_iter()
                .filter(|&i| !(settings.ban_regular_shop_items && catalog.is_regular(i)))
                .filter(|&i| !(settings.ban_op_shop_items && catalog.is_op(i)))
                .filter(|i| !guaranteed.contains(i))
                .collect();
            let fill_count = total - guaranteed.len();
            let mut fill = draw(&possible, fill_count, true, &mut rng);
            if fill.len() != fill_count {
                return out; // aucun objet possible : boutiques inchangées
            }
            fill.shuffle(&mut rng);

            // Boutiques hors aventure principale : objets ordinaires seulement.
            for &i in special.iter().filter(|&&i| !shops[i].1) {
                for slot in out[i].iter_mut() {
                    *slot = fill.pop().unwrap_or(*slot);
                }
            }
            // Le reste, objets garantis compris, dans les boutiques de l'aventure principale.
            let mut rest = guaranteed;
            rest.append(&mut fill);
            rest.shuffle(&mut rng);
            for &i in special.iter().filter(|&&i| shops[i].1) {
                for slot in out[i].iter_mut() {
                    *slot = rest.pop().unwrap_or(*slot);
                }
            }
        }
    }
    out
}

/// Étape du randomizer : objets ramassables puis boutiques.
pub(crate) fn randomize_items(game: &mut GameRom, settings: &ItemSettings, seed: u64, log: &mut String) -> Result<(), RomError> {
    if !settings.changes_anything() {
        return Ok(());
    }
    let layout = ItemLayout::for_rom(game).ok_or_else(field_items::unsupported)?;
    let names = game.text_file(layout.item_names)?;
    let catalog = ItemCatalog::new(layout.gen, &names).with_b2w2(matches!(game.game, crate::games::Game::Black2 | crate::games::Game::White2));
    let name = |id: u16| names.get(id as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("objet n°{id}"));

    if settings.field_items != FieldItemsMode::Unchanged {
        let mut items: Vec<FieldItem> = field_items::read(game)?;
        let current: Vec<u16> = items.iter().map(|f| f.item).collect();
        let new = new_field_items(&catalog, settings, &current, seed);
        let _ = writeln!(log, "== Objets ramassables ==");
        for (i, (f, &n)) in items.iter_mut().zip(&new).enumerate() {
            if f.item != n {
                let _ = writeln!(log, "n°{i:3} ({}) : {} → {}", f.kind.name_fr(), name(f.item), name(n));
                f.item = n;
            }
        }
        let _ = writeln!(log);
        field_items::write(game, &items)?;
    }

    if settings.shops != ShopsMode::Unchanged {
        let mut list = shops::read(game)?;
        let input: Vec<(ShopKind, bool, Vec<u16>)> = list.iter().map(|s| (s.kind, s.main_game, s.items.clone())).collect();
        let new = new_shop_items(&catalog, settings, &input, seed);
        let _ = writeln!(log, "== Boutiques ==");
        for (shop, items) in list.iter_mut().zip(new) {
            if shop.items != items {
                let labels: Vec<String> = items.iter().map(|&i| name(i)).collect();
                let _ = writeln!(log, "{} : {}", shop.name, labels.join(", "));
                shop.items = items;
            }
        }
        let _ = writeln!(log);
        shops::write(game, &list)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(gen: u8) -> ItemCatalog {
        let count = if gen == 4 { 468 } else { 639 };
        let names: Vec<String> = (0..count).map(|i| if i == 0 { String::new() } else { format!("Objet {i}") }).collect();
        ItemCatalog::new(gen, &names)
    }

    #[test]
    fn categories() {
        let c = catalog(5);
        assert!(c.is_tm(328) && c.is_tm(419) && c.is_tm(618) && !c.is_tm(420));
        assert!(!c.is_allowed(420) && !c.is_allowed(0) && !c.is_allowed(450) && !c.is_allowed(700));
        assert!(c.is_allowed(MASTER_BALL) && c.is_allowed(RARE_CANDY) && c.is_allowed(328));
        assert_eq!(tm_item(1), 328);
        assert_eq!(tm_item(93), 618);
        assert_eq!(catalog(4).all_tms().len(), 92);
        assert!(!catalog(4).is_tm(618));
    }

    #[test]
    fn field_items_keep_tms_and_key_items() {
        let c = catalog(4);
        // Potion, CT01, CS01, objet rare (Vélo 450), Super Bonbon, CT02, Master Ball.
        let current = vec![17, 328, 420, 450, 50, 329, 1, 18, 19, 20];
        for mode in [FieldItemsMode::Shuffle, FieldItemsMode::Random, FieldItemsMode::RandomEven] {
            let settings = ItemSettings { field_items: mode, no_rare_candy: true, no_master_ball: true, ..Default::default() };
            let new = new_field_items(&c, &settings, &current, 7);
            assert_eq!(new.len(), current.len());
            assert_eq!(new[2], 420, "CS inchangée");
            assert_eq!(new[3], 450, "objet rare inchangé");
            assert!(c.is_tm(new[1]) && c.is_tm(new[5]), "CT remplacées par des CT");
            for (i, &n) in new.iter().enumerate().filter(|(i, _)| ![1, 2, 3, 5].contains(i)) {
                assert!(!c.is_tm(n) && n != RARE_CANDY && n != MASTER_BALL, "emplacement {i} : {n}");
            }
            assert_eq!(new, new_field_items(&c, &settings, &current, 7), "déterministe");
        }
    }

    #[test]
    fn random_tms_include_required() {
        let c = catalog(4);
        let current: Vec<u16> = (328..=419).collect();
        let settings = ItemSettings { field_items: FieldItemsMode::Random, ..Default::default() };
        let new = new_field_items(&c, &settings, &current, 3);
        assert!(c.required_tms().iter().all(|t| new.contains(t)));
        let unique: BTreeSet<u16> = new.iter().copied().collect();
        assert_eq!(unique.len(), new.len());
    }

    #[test]
    fn shops_keep_regular_and_tm() {
        let c = catalog(5);
        let shops = vec![
            (ShopKind::Regular, true, vec![4, 17]),
            (ShopKind::Tm, true, vec![348, 354]),
            (ShopKind::Special, true, vec![13, 14, 15, 16, 137, 138, 139, 140, 141, 142]),
            (ShopKind::Special, true, (55..=62).chain(80..=85).chain(107..=110).chain(321..=327).chain(200..=210).collect()),
            (ShopKind::Special, false, vec![300, 301, 302]),
        ];
        let settings = ItemSettings {
            shops: ShopsMode::Random,
            guarantee_evolution_items: true,
            guarantee_x_items: true,
            ban_op_shop_items: true,
            ban_regular_shop_items: true,
            no_rare_candy: true,
            ..Default::default()
        };
        let new = new_shop_items(&c, &settings, &shops, 11);
        assert_eq!(new[0], shops[0].2);
        assert_eq!(new[1], shops[1].2);
        for (n, s) in new.iter().zip(&shops) {
            assert_eq!(n.len(), s.2.len());
        }
        let main: Vec<u16> = new[2].iter().chain(&new[3]).copied().collect();
        assert!(c.evolution_items().iter().all(|e| main.contains(e)), "objets d'évolution garantis");
        assert!(c.x_items().iter().all(|e| main.contains(e)), "objets X garantis");
        for &i in new[2..].iter().flatten() {
            assert!(c.is_allowed(i) && !c.is_tm(i) && !c.is_op(i) && !c.is_regular(i) && i != RARE_CANDY);
        }
        assert!(new[4].iter().all(|i| !c.evolution_items().contains(i) && !c.x_items().contains(i)));

        let shuffle = ItemSettings { shops: ShopsMode::Shuffle, ..Default::default() };
        let mixed = new_shop_items(&c, &shuffle, &shops, 11);
        let mut before: Vec<u16> = shops[2..].iter().flat_map(|s| s.2.clone()).collect();
        let mut after: Vec<u16> = mixed[2..].iter().flatten().copied().collect();
        before.sort_unstable();
        after.sort_unstable();
        assert_eq!(before, after);
    }

    #[test]
    fn settings_default_and_json() {
        let s = ItemSettings::default();
        assert!(!s.changes_anything());
        let parsed: ItemSettings = serde_json::from_str(r#"{"fieldItems":"random_even","shops":"random","noRareCandy":true}"#).unwrap();
        assert_eq!(parsed.field_items, FieldItemsMode::RandomEven);
        assert_eq!(parsed.shops, ShopsMode::Random);
        assert!(parsed.no_rare_candy && !parsed.no_master_ball);
    }
}
