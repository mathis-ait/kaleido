use super::*;

#[test]
fn names() {
    assert_eq!(species_name(0), Some("Œuf"));
    assert_eq!(species_name(1), Some("Bulbizarre"));
    assert_eq!(species_name(445), Some("Carchacrok"));
    assert_eq!(species_name(1025), Some("Pêchaminus"));
    assert_eq!(species_name(1026), None);
    assert_eq!(move_name(89), Some("Séisme"));
    assert_eq!(move_name(0), None);
    assert_eq!(ability_name(8), Some("Voile Sable"));
    assert_eq!(item_name(4), Some("Poké Ball"));
    assert_eq!(nature_name(0), Some("Hardi"));
    assert_eq!(nature_names().len(), 25);
    assert_eq!(type_name(15), Some("Dragon"));
    assert_eq!(type_name(4), Some("Sol"));
    assert_eq!(game_name(12), Some("Platine"));
    assert_eq!(game_name(27), Some("Rubis Oméga"));
    assert_eq!(game_name(33), Some("Ultra-Lune"));
}

#[test]
fn balls_and_items() {
    let balls = ball_names();
    assert_eq!(balls.len(), 27);
    assert_eq!(ball_name(1), Some("Master Ball"));
    assert_eq!(ball_name(4), Some("Poké Ball"));
    assert_eq!(ball_name(24), Some("Compét’Ball"));
    assert_eq!(ball_name(25), Some("Rêve Ball"));
    assert_eq!(ball_name(26), Some("Ultra Ball"));
    assert_eq!(ball_name(0), None);
    // Lettres renommées après la Gen 4.
    assert_eq!(item_name_in(Game::DP, 137), Some("Lettre Herbe"));
    assert_eq!(item_name_in(Game::BW, 137), item_name(137));
    assert_eq!(ribbon_name("RibbonChampionSinnoh"), Some("Maître de Sinnoh"));
}

#[test]
fn personal_garchomp() {
    let p = personal(Game::Pt, 445, 0).unwrap();
    let b = p.base_stats;
    assert_eq!((b.hp, b.attack, b.defense, b.sp_attack, b.sp_defense, b.speed), (108, 130, 95, 80, 85, 102));
    assert_eq!(p.types, [15, 4]); // Dragon / Sol
    assert!(p.abilities.contains(&8)); // Voile Sable
    assert_eq!(p.abilities[2], 0); // pas de talent caché en Gen 4
    assert_eq!(p.ev_yield.attack, 3);
    assert_eq!(p.catch_rate, 45);
    assert_eq!(p.gender_ratio, 127);
    // Talent caché Peau Dure (24) à partir de la Gen 5.
    let p5 = personal(Game::B2W2, 445, 0).unwrap();
    assert_eq!(p5.abilities, [8, 8, 24]);
    // Méga-Carchacrok en ROSA.
    let mega = personal(Game::ORAS, 445, 1).unwrap();
    assert_eq!(mega.base_stats.attack, 170);
    assert_eq!(personal(Game::Pt, 494, 0), None);
    assert_eq!(personal(Game::BW, 650, 0), None);
    assert!(personal(Game::USUM, 807, 0).is_some());
    assert!(personal(Game::SM, 807, 0).is_none());
}

#[test]
fn personal_forms() {
    // Motisma Chaleur : Électrik / Feu à partir de la Gen 5, Électrik / Spectre de base.
    let rotom = personal(Game::B2W2, 479, 0).unwrap();
    assert_eq!(rotom.form_count, 6);
    assert_eq!(rotom.types, [12, 7]);
    assert_eq!(personal(Game::B2W2, 479, 1).unwrap().types, [12, 9]);
    // Forme inexistante : repli sur la forme 0.
    assert_eq!(personal(Game::XY, 1, 3), personal(Game::XY, 1, 0));
    // Raichu d'Alola.
    assert_eq!(personal(Game::SM, 26, 1).unwrap().types, [12, 13]);
}

#[test]
fn learnsets() {
    let garchomp = levelup(Game::Pt, 445, 0);
    assert!(!garchomp.is_empty());
    assert_eq!(garchomp.first(), Some(&(424, 1))); // Crocs Feu
    assert_eq!(garchomp.last().map(|&(m, l)| (move_name(m), l)), Some((Some("Draco-Charge"), 55)));
    for game in Game::ALL {
        let bulbasaur = levelup(game, 1, 0);
        // Gen 1 : les attaques de départ sont dans la fiche, pas dans la liste par niveau.
        if game.generation() > 1 {
            assert_eq!(bulbasaur[0], (33, 1), "{game:?}"); // Charge au niveau 1
            assert!(!egg_moves(game, 1, 0).is_empty(), "{game:?}");
        }
        assert!(bulbasaur.iter().all(|&(m, l)| m != 0 && m <= max_move(game) && l <= 100), "{game:?}");
    }
    assert!(levelup(Game::DP, 0, 0).is_empty());
    // Miaouss d'Alola n'a pas les mêmes capacités Œuf.
    assert_ne!(egg_moves(Game::SM, 52, 0), egg_moves(Game::SM, 52, 1));
}

#[test]
fn moves() {
    let eq = move_info(89).unwrap();
    assert_eq!((eq.type_id, eq.pp, eq.power, eq.accuracy, eq.category), (4, 10, Some(100), Some(100), MoveCategory::Physical));
    // Charge : 35 puissance en Gen 4, 50 en Gen 5-6, 40 en Gen 7.
    assert_eq!(move_info_in(Game::DP, 33).unwrap().power, Some(35));
    assert_eq!(move_info_in(Game::BW, 33).unwrap().power, Some(50));
    assert_eq!(move_info_in(Game::SM, 33).unwrap().power, Some(40));
    // Charme : Normal avant la Gen 6, Fée ensuite.
    assert_eq!(move_info_in(Game::HGSS, 204).unwrap().type_id, 0);
    assert_eq!(move_info_in(Game::XY, 204).unwrap().type_id, 17);
    assert_eq!(move_info_in(Game::HGSS, 204).unwrap().category, MoveCategory::Status);
    assert_eq!(move_info(98).unwrap().priority, 1); // Vive-Attaque
    assert_eq!(move_info(129).unwrap().accuracy, None); // Météores
    assert_eq!(move_info_in(Game::DP, 468), None);
    assert!(move_info(728).is_some());
    assert_eq!(move_info(729), None);
}

#[test]
fn locations_by_generation() {
    assert_eq!(location_name(4, 16), Some("Route 201"));
    assert!(locations(4).iter().any(|&(_, n)| n == "Route 201"));
    assert_eq!(location_name(4, 2000), Some("Couple Pension"));
    assert!(location_name(5, 60002).is_some());
    assert!(location_name(5, 30001).unwrap().contains("Poké Fret"));
    // Gen 6 : sous-lieux fusionnés, ROSA à partir de 170.
    assert!(locations(6).iter().any(|&(id, _)| id == 170));
    assert!(location_name(6, 9).is_none());
    // Gen 7 : Ekaeka.
    let gen7 = locations(7);
    assert!(gen7.iter().any(|&(_, n)| n.contains("Ekaeka")));
    assert_eq!(location_name(7, 0), None);
    assert_eq!(locations(7)[0], (0, "(Aucun)"));
    assert!(locations(2).is_empty());
    assert_eq!(location_name(3, 16), Some("Route 101"));
    assert_eq!(location_name(3, 101), Some("Route 1"));
    assert_eq!(location_name(3, 254), Some("(Échange in-game)"));
    for generation in 3..=7 {
        let list = locations(generation);
        let mut ids: Vec<u16> = list.iter().map(|&(id, _)| id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), list.len());
        assert!(list.len() > 100, "{generation}");
    }
}

#[test]
fn forms() {
    assert!(form_names(Game::Pt, 1).is_empty());
    assert_eq!(form_names(Game::XY, 3).len(), 2);
    assert_eq!(form_name(Game::XY, 3, 1).as_deref(), Some("Méga"));
    assert_eq!(form_names(Game::Pt, 201).len(), 28);
    assert_eq!(form_names(Game::Pt, 493).len(), 18);
    assert_eq!(form_names(Game::Pt, 493)[9], "???");
    assert_eq!(form_names(Game::B2W2, 493).len(), 17);
    assert_eq!(form_names(Game::USUM, 493).len(), 18);
    assert_eq!(form_names(Game::HGSS, 172).len(), 2);
    assert!(form_names(Game::BW, 172).is_empty());
    assert_eq!(form_names(Game::SM, 25).len(), 8);
    assert_eq!(form_names(Game::ORAS, 25).len(), 7);
    assert_eq!(form_names(Game::SM, 778).len(), 4);
    assert_eq!(form_names(Game::SM, 52).len(), 2);
    assert_eq!(form_names(Game::ORAS, 666).len(), 20);
    // Le nombre de formes PKHeX correspond aux fiches des jeux pour les espèces à fiches de forme.
    for (game, species) in [(Game::Pt, 479), (Game::B2W2, 641), (Game::ORAS, 445), (Game::USUM, 800), (Game::USUM, 745)] {
        assert_eq!(form_names(game, species).len(), personal(game, species, 0).unwrap().form_count as usize, "{game:?} {species}");
    }
}

#[test]
fn maxima_and_conversion() {
    assert_eq!(max_species(Game::HGSS), 493);
    assert_eq!(max_species(Game::USUM), 807);
    assert_eq!(max_item(Game::B2W2), 638);
    assert_eq!(max_ability(Game::ORAS), 191);
    assert_eq!(Game::from(crate::games::Game::AlphaSapphire), Game::ORAS);
    assert_eq!(serde_json::to_string(&Game::USUM).unwrap(), "\"usum\"");
    let json = serde_json::to_value(personal(Game::XY, 1, 0).unwrap()).unwrap();
    assert!(json.get("baseStats").is_some() && json.get("formCount").is_some());
}
