//! Tests du format Showdown : lecture, écriture, noms anglais / français, import dans
//! une sauvegarde Platine synthétique et dans une vraie sauvegarde Soleil/Lune,
//! sets Smogon (petit extrait embarqué, sans réseau).

use super::smogon::SmogonData;
use super::*;
use crate::save::session::{SaveSession, Slot};
use crate::save::showdown_apply::ImportTarget;
use crate::save::SaveFile;

fn sm_save() -> SaveSession {
    let bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pkhex/sm_project_802.main"));
    SaveSession::open(bytes).unwrap()
}

fn pt_save() -> SaveSession {
    SaveSession::open(&crate::save::demo_save().unwrap()).unwrap()
}

/// Équipe d'exemple au format exporté par Pokémon Showdown.
const SAMPLE_EN: &str = "=== [gen7ou] Exemple ===

Chompy (Garchomp) (F) @ Choice Scarf
Ability: Rough Skin
Level: 50
Shiny: Yes
EVs: 252 Atk / 4 SpD / 252 Spe
Jolly Nature
- Earthquake
- Outrage
- Stone Edge
- Fire Fang

Rotom-Wash @ Leftovers
Ability: Levitate
EVs: 248 HP / 252 Def / 8 SpD
Bold Nature
IVs: 0 Atk
- Hydro Pump
- Volt Switch
- Will-O-Wisp
- Pain Split

Magnezone @ Choice Specs
Ability: Magnet Pull
EVs: 4 HP / 252 SpA / 252 Spe
Timid Nature
IVs: 0 Atk
- Thunderbolt
- Flash Cannon
- Hidden Power [Fire]
- Volt Switch
";

#[test]
fn parse_showdown_sample() {
    let sets = parse_team(SAMPLE_EN);
    assert_eq!(sets.len(), 3);
    let g = &sets[0];
    assert_eq!(g.nickname.as_deref(), Some("Chompy"));
    assert_eq!(g.species, "Garchomp");
    assert_eq!(g.gender, Some('F'));
    assert_eq!(g.item.as_deref(), Some("Choice Scarf"));
    assert_eq!(g.ability.as_deref(), Some("Rough Skin"));
    assert_eq!(g.level, Some(50));
    assert!(g.shiny);
    assert_eq!(g.evs, [0, 252, 0, 0, 4, 252]);
    assert_eq!(g.ivs, [31; 6]);
    assert_eq!(g.nature.as_deref(), Some("Jolly"));
    assert_eq!(g.moves, ["Earthquake", "Outrage", "Stone Edge", "Fire Fang"]);
    assert_eq!(sets[1].species, "Rotom-Wash");
    assert_eq!(sets[1].ivs, [31, 0, 31, 31, 31, 31]);
    assert_eq!(sets[2].moves[2], "Hidden Power");
    assert_eq!(sets[2].hidden_power.as_deref(), Some("Fire"));
    assert!(sets.iter().all(|s| s.ignored.is_empty()));
    assert_eq!(guess_lang(&sets), Lang::En);
}

#[test]
fn resolve_english_sample() {
    let sets = parse_team(SAMPLE_EN);
    let r = resolve(&sets[0], Game::SM, Lang::En);
    assert_eq!((r.species, r.form), (445, 0));
    assert_eq!(r.species_name, "Carchacrok");
    assert_eq!(r.held_item, 287); // Mouchoir Choix
    assert_eq!(r.ability, Some(24)); // Peau Dure (talent caché)
    assert_eq!(r.ability_number, Some(4));
    assert_eq!(r.nature, Some(13)); // Jovial
    assert_eq!(r.moves, [89, 200, 444, 424]);
    assert_eq!(r.gender, Some(Gender::Female));
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);

    let rotom = resolve(&sets[1], Game::SM, Lang::En);
    assert_eq!((rotom.species, rotom.form), (479, 2));
    assert_eq!(rotom.form_name.as_deref(), Some("Lavage"));

    // Puissance Cachée Feu : les IV sont ajustés (Att 0 → reste pair, etc.).
    let mag = resolve(&sets[2], Game::SM, Lang::En);
    assert_eq!(mag.moves[2], HIDDEN_POWER);
    assert_eq!(hidden_power_type(mag.ivs), 9);
    assert_eq!(mag.ivs[1], 0);
    assert!(mag.warnings.iter().any(|w| w.contains("Puissance Cachée")));
}

#[test]
fn tolerant_names() {
    let sp = |n: &str| resolve_species(Game::USUM, n, Lang::En).map(|(s, f, _)| (s, f));
    assert_eq!(sp("Mr. Mime"), Some((122, 0)));
    assert_eq!(sp("mr mime"), Some((122, 0)));
    assert_eq!(sp("M. Mime"), Some((122, 0)));
    assert_eq!(sp("Nidoran-F"), Some((29, 0)));
    assert_eq!(sp("Nidoran♂"), Some((32, 0)));
    assert_eq!(sp("Farfetch’d"), Some((83, 0)));
    assert_eq!(sp("Farfetch'd"), Some((83, 0)));
    assert_eq!(sp("Canarticho"), Some((83, 0)));
    assert_eq!(sp("Ho-Oh"), Some((250, 0)));
    assert_eq!(sp("Kommo-o"), Some((784, 0)));
    assert_eq!(sp("Type: Null"), Some((772, 0)));
    assert_eq!(sp("Flabébé"), Some((669, 0)));
    assert_eq!(sp("Électhor"), Some((145, 0)));
    assert_eq!(sp("electhor"), Some((145, 0)));
    assert_eq!(sp("Rotom-Wash"), Some((479, 2)));
    assert_eq!(sp("Motisma-Lavage"), Some((479, 2)));
    assert_eq!(sp("Landorus-Therian"), Some((645, 1)));
    assert_eq!(sp("Vivillon-Polar"), Some((666, 1)));
    assert_eq!(sp("Prismillon-Banquise"), Some((666, 1)));
    assert_eq!(sp("Necrozma-Dusk-Mane"), Some((800, 1)));
    assert_eq!(sp("Necrozma-Dawn-Wings"), Some((800, 2)));
    assert_eq!(sp("Zygarde-10%"), Some((718, 1)));
    assert_eq!(sp("Oricorio-Pa'u"), Some((741, 2)));
    assert_eq!(sp("Oricorio-Pom-Pom"), Some((741, 1)));
    assert_eq!(sp("Meowstic-F"), Some((678, 1)));
    assert_eq!(sp("Marowak-Alola"), Some((105, 1)));
    assert_eq!(sp("Arceus-Fire"), Some((493, 9)));
    assert_eq!(sp("Arceus-Feu"), Some((493, 9)));
    assert_eq!(sp("Genesect-Douse"), Some((649, 1)));
    assert_eq!(sp("Basculin-Blue-Striped"), Some((550, 1)));
    assert_eq!(sp("Lycanroc-Dusk"), Some((745, 2)));
    // Formes de combat : rangées sous la forme de base, avec un avertissement.
    let (s, f, note) = resolve_species(Game::USUM, "Charizard-Mega-X", Lang::En).unwrap();
    assert_eq!((s, f), (6, 0));
    assert!(note.is_some());
    assert_eq!(sp("Aegislash-Blade"), Some((681, 0)));
    assert_eq!(sp("Pikachu-Truc"), Some((25, 0)));
    assert_eq!(sp("Bidulemon"), None);

    // Attaques, objets, talents, natures dans les deux langues.
    assert_eq!(dex::find_move("Thunderbolt", Lang::En, 999), Some(85));
    assert_eq!(dex::find_move("Tonnerre", Lang::Fr, 999), Some(85));
    assert_eq!(dex::find_move("U-turn", Lang::En, 999), Some(369));
    assert_eq!(dex::find_move("Demi-Tour", Lang::Fr, 999), Some(369));
    assert_eq!(dex::find_move("ThunderPunch", Lang::En, 999), Some(9)); // ancienne graphie Gen 4
    assert_eq!(dex::find_item("Never-Melt Ice", Lang::En, 999), Some(246));
    assert_eq!(dex::find_item("BrightPowder", Lang::En, 999), Some(213));
    assert_eq!(dex::find_item("Restes", Lang::Fr, 999), Some(234));
    assert_eq!(dex::find_ability("Intimidate", Lang::En, 999), Some(22));
    assert_eq!(dex::find_ability("Intimidation", Lang::Fr, 999), Some(22));
    assert_eq!(dex::find_nature("Adamant", Lang::En), Some(3));
    assert_eq!(dex::find_nature("Rigide", Lang::Fr), Some(3));
    assert_eq!(dex::find_ball("Ultra Ball"), Some(2));
    assert_eq!(dex::find_ball("Poke Ball"), Some(4));
    assert_eq!(dex::find_ball("Scuba Ball"), Some(7));
    // « Charge » : Charge (en) = Chargeur (fr, 268) ; Charge (fr) = Tackle (33).
    assert_eq!(dex::find_move("Charge", Lang::En, 999), Some(268));
    assert_eq!(dex::find_move("Charge", Lang::Fr, 999), Some(33));
}

#[test]
fn ability_of_another_form() {
    // Synergie n'existe que chez Amphinobi forme 1 : la forme suit le talent.
    let set = ShowdownSet { species: "Greninja".into(), ability: Some("Battle Bond".into()), ..ShowdownSet::default() };
    let r = resolve(&set, Game::SM, Lang::En);
    assert_eq!((r.form, r.ability_number), (1, Some(1)));
    assert!(!r.warnings.iter().any(|w| w.contains("talent")), "{:?}", r.warnings);
    // Talent de Méga (Griffe Dure) : forme de base, talent normal, simple remarque.
    let set = ShowdownSet { species: "Charizard".into(), item: Some("Charizardite X".into()), ability: Some("Tough Claws".into()), ..ShowdownSet::default() };
    let r = resolve(&set, Game::SM, Lang::En);
    assert_eq!((r.form, r.ability), (0, Some(66)));
    assert!(r.warnings.iter().any(|w| w.contains("forme de combat")), "{:?}", r.warnings);
    // Zygarde Parfait se range en Zygarde 50 % avec Rassemblement.
    let set = ShowdownSet { species: "Zygarde-Complete".into(), ability: Some("Power Construct".into()), ..ShowdownSet::default() };
    let r = resolve(&set, Game::USUM, Lang::En);
    assert_eq!((r.form, r.ability), (3, Some(211)));
}

#[test]
fn hidden_power_ivs() {
    assert_eq!(hidden_power_type([31; 6]), 16); // Ténèbres
    for t in 1..=16 {
        let ivs = ivs_for_hidden_power([31; 6], t).unwrap();
        assert_eq!(hidden_power_type(ivs), t);
        assert!(ivs.iter().all(|&v| v >= 30));
    }
}

#[test]
fn french_text_and_keywords() {
    let text = "Carchacrok (F) @ Mouchoir Choix
Talent : Peau Dure
Niveau : 50
Chromatique : Oui
EVs : 252 Att / 4 Déf Spé / 252 Vit
IVs : 0 Atq Spé
Nature : Jovial
Ball : Bis Ball
- Séisme
- Colère
- Lame de Roc
- Crocs Feu
";
    let sets = parse_team(text);
    assert_eq!(sets.len(), 1);
    assert_eq!(guess_lang(&sets), Lang::Fr);
    let r = resolve(&sets[0], Game::SM, Lang::Fr);
    assert_eq!(r.species, 445);
    assert_eq!(r.held_item, 287);
    assert_eq!(r.ability, Some(24));
    assert_eq!(r.nature, Some(13));
    assert_eq!(r.evs, [0, 252, 0, 0, 4, 252]);
    assert_eq!(r.ivs, [31, 31, 31, 0, 31, 31]);
    assert_eq!(r.moves, [89, 200, 444, 424]);
    assert_eq!(r.ball, Some(9)); // Bis Ball
    assert!(r.shiny);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn warnings_for_bad_sets() {
    let text = "Garchomp @ Z-Crystal-Inconnu
Ability: Levitate
EVs: 252 HP / 252 Atk / 252 Spe
- Earthquake
- Moonblast
- Pas Une Attaque

Greninja

Bidulemon @ Leftovers
";
    let sets = parse_team(text);
    assert_eq!(sets.len(), 3);
    let r = resolve(&sets[0], Game::Pt, Lang::En);
    let all = r.warnings.join(" | ");
    assert!(all.contains("Objet inconnu"), "{all}");
    assert!(all.contains("ne peut pas avoir le talent"), "{all}");
    assert!(all.contains("510"), "{all}");
    assert!(all.contains("n'existe pas en Gen 4"), "{all}"); // Moonblast
    assert!(all.contains("Attaque inconnue"), "{all}");
    assert_eq!(r.moves, [89, 0, 0, 0]);
    assert!(resolve(&sets[1], Game::Pt, Lang::En).error.is_some()); // Amphinobi : Gen 6
    assert!(resolve(&sets[2], Game::Pt, Lang::En).error.is_some());
}

#[test]
fn round_trip_sm_party_en_and_fr() {
    let s = sm_save();
    let game = s.game();
    let party = s.save.party().unwrap();
    assert!(!party.is_empty());
    for lang in [Lang::En, Lang::Fr] {
        let slots: Vec<Slot> = (0..party.len()).map(|i| Slot::Party { index: i }).collect();
        let text = s.export_showdown(&slots, lang).unwrap();
        let sets = parse_team(&text);
        assert_eq!(sets.len(), party.len(), "{text}");
        assert_eq!(guess_lang(&sets), lang, "{text}");
        // Texte → set → texte : identique.
        assert_eq!(format_team(&sets, lang), text);
        for (set, p) in sets.iter().zip(&party) {
            assert!(set.ignored.is_empty(), "{:?}", set.ignored);
            let r = resolve(set, game, lang);
            assert!(r.error.is_none(), "{text}");
            assert_eq!((r.species, r.form), (p.species(), p.form()), "{text}");
            assert_eq!(r.held_item, p.held_item(), "{text}");
            assert_eq!(r.nature, Some(p.nature()));
            assert_eq!(r.evs, p.evs());
            assert_eq!(r.ivs, p.ivs());
            assert_eq!(r.moves, p.moves(), "{text}");
            assert_eq!(r.shiny, p.is_shiny());
        }
    }
}

#[test]
fn import_into_platinum_demo() {
    let mut s = pt_save();
    let before = s.save.box_slot(0, 12).unwrap();
    assert!(before.is_none());
    let text = "Garchomp (M) @ Life Orb
Ability: Sand Veil
Level: 78
Shiny: Yes
EVs: 4 HP / 252 Atk / 252 Spe
Adamant Nature
- Dragon Claw
- Earthquake
- Fire Fang
- Swords Dance

Sparky (Pikachu) (F) @ Light Ball
Happiness: 0
- Volt Tackle
- Frustration
- Hidden Power [Ice]
- Fake Out
";
    let report = s.import_showdown(text, ImportTarget::Box { r#box: 0 }).unwrap();
    assert_eq!(report.imported, 2, "{report:?}");
    assert_eq!(report.lang, Lang::En);
    // La boîte 0 contient déjà 12 Pokémon, moins 3 passés dans l'équipe.
    let slot = report.sets[0].slot.unwrap();
    let p = s.get(slot).unwrap().unwrap();
    assert_eq!(p.species(), 445);
    assert_eq!(p.held_item(), 270); // Orbe Vie
    assert_eq!(p.ability(), 8); // Voile Sable
    assert_eq!(p.nature(), 3); // Rigide (Gen 4 : PID % 25)
    assert!(p.is_shiny());
    assert_eq!(p.gender(), Gender::Male);
    assert_eq!(p.evs(), [4, 252, 0, 0, 0, 252]);
    assert_eq!(p.moves(), [337, 89, 424, 14]);
    assert_eq!(p.pp_ups(), [3; 4]);
    assert!(p.checksum_valid());
    let view = s.view_slot(slot).unwrap();
    assert_eq!(view.summary.level, 78);

    let pika = s.get(report.sets[1].slot.unwrap()).unwrap().unwrap();
    assert_eq!(pika.species(), 25);
    assert_eq!(pika.nickname(), "Sparky");
    assert_eq!(pika.friendship(), 0);
    assert_eq!(pika.gender(), Gender::Female);
    assert_eq!(hidden_power_type(pika.ivs()), 14); // Glace
    assert_eq!(pika.moves()[0], 344);

    // Une seule étape d'historique pour tout l'import.
    assert!(s.undo());
    assert!(s.get(slot).unwrap().is_none());
    assert!(s.get(report.sets[1].slot.unwrap()).unwrap().is_none());
}

#[test]
fn import_into_sun_moon_and_export() {
    let mut s = sm_save();
    let text = "Toxapex @ Black Sludge
Ability: Regenerator
EVs: 252 HP / 252 Def / 4 SpD
Bold Nature
IVs: 0 Atk
- Scald
- Recover
- Haze
- Toxic Spikes

Oricorio-Pa'u (F) @ Life Orb
Ability: Dancer
Pokeball: Dive Ball
- Quiver Dance
- Revelation Dance
";
    let report = s.import_showdown(text, ImportTarget::Box { r#box: 1 }).unwrap();
    assert_eq!(report.imported, 2, "{report:?}");
    let pex = s.get(report.sets[0].slot.unwrap()).unwrap().unwrap();
    assert_eq!(pex.species(), 748);
    assert_eq!(pex.ability(), 144); // Régé-Force (talent caché)
    assert_eq!(pex.ability_number(), 4);
    assert_eq!(pex.ivs()[1], 0);
    let ori = s.get(report.sets[1].slot.unwrap()).unwrap().unwrap();
    assert_eq!((ori.species(), ori.form()), (741, 2));
    assert_eq!(ori.ball(), 7);
    assert_eq!(ori.gender(), Gender::Female);

    // Export anglais : nom Showdown de la forme.
    let out = s.export_showdown(&[report.sets[1].slot.unwrap()], Lang::En).unwrap();
    assert!(out.starts_with("Oricorio-Pa'u (F) @ Life Orb\n"), "{out}");
    assert!(out.contains("Ability: Dancer"), "{out}");
    assert!(out.contains("Pokeball: Dive Ball"), "{out}");
    let fr = s.export_showdown(&[report.sets[1].slot.unwrap()], Lang::Fr).unwrap();
    assert!(fr.starts_with("Plumeline-Hula (F) @ Orbe Vie\n"), "{fr}");

    // La sauvegarde modifiée se relit.
    let bytes = s.to_bytes();
    let again = SaveFile::from_bytes(&bytes).unwrap();
    let slot = report.sets[0].slot.unwrap();
    let Slot::Box { r#box, index } = slot else { panic!() };
    assert_eq!(again.box_slot(r#box, index).unwrap().unwrap().species(), 748);
}

#[test]
fn apply_set_to_existing_pokemon() {
    let mut s = sm_save();
    let slot = Slot::Party { index: 0 };
    let p = s.get(slot).unwrap().unwrap();
    let ot = p.ot_name();
    let species = p.species();
    let set = ShowdownSet {
        species: dex::species_name_en(species).unwrap().to_string(),
        level: Some(100),
        evs: [252, 0, 0, 252, 4, 0],
        nature: Some("Modest".into()),
        moves: vec!["Protect".into(), "Substitute".into()],
        ..ShowdownSet::default()
    };
    let (view, _) = s.apply_showdown_set(slot, &set, Lang::En).unwrap();
    assert_eq!(view.summary.level, 100);
    assert_eq!(view.summary.nature, 15);
    let p = s.get(slot).unwrap().unwrap();
    assert_eq!(p.ot_name(), ot);
    assert_eq!(p.moves(), [182, 164, 0, 0]);
    assert_eq!(p.evs(), [252, 0, 0, 252, 4, 0]);
}

const SMOGON_FIXTURE: &str = r#"{
  "Garchomp": {
    "ou": {"Swords Dance": {"moves": ["Swords Dance", "Earthquake", ["Stone Edge", "Poison Jab"], "Fire Fang"], "ability": "Rough Skin", "item": ["Life Orb", "Yache Berry"], "nature": "Jolly", "evs": {"atk": 252, "spd": 4, "spe": 252}}},
    "lc": {"Weird": {"moves": ["Tackle"], "nature": "Hardy", "evs": {"hp": 4}}}
  },
  "Garchomp-Mega": {"ubers": {"Mega": {"moves": ["Earthquake", "Outrage"], "item": "Garchompite", "nature": "Adamant", "evs": {"hp": 4, "atk": 252, "spe": 252}, "ivs": {"spa": 0}}}},
  "Rotom-Wash": {"uu": {"Defensive": {"moves": ["Volt Switch", "Hydro Pump", "Will-O-Wisp", "Pain Split"], "ability": "Levitate", "item": "Leftovers", "nature": "Bold", "evs": {"hp": 248, "def": 252, "spd": 8}}}},
  "Rotom": {"ou": {"Scarf": {"moves": ["Volt Switch", "Shadow Ball", "Trick", ["Hidden Power Ice", "Hidden Power Fire"]], "ability": "Levitate", "item": "Choice Scarf", "nature": "Timid", "evs": {"spa": 252, "spd": 4, "spe": 252}, "ivs": {"atk": 0}}}},
  "Syclant": {"cap": {"Fake": {"moves": ["Ice Beam"]}}}
}"#;

#[test]
fn smogon_sets_from_fixture() {
    let data = SmogonData::parse(SMOGON_FIXTURE.as_bytes()).unwrap();
    assert_eq!(data.len(), 5);

    let sets = data.sets_for(Game::SM, 445, 0);
    let names: Vec<_> = sets.iter().map(|s| (s.format.as_str(), s.name.as_str())).collect();
    // Ubers avant OU avant LC ; la Méga est proposée pour Carchacrok.
    assert_eq!(names, [("ubers", "Mega"), ("ou", "Swords Dance"), ("lc", "Weird")]);
    let sd = &sets[1];
    assert_eq!(sd.format_label, "OU");
    assert_eq!(sd.resolved.moves, [14, 89, 444, 424]);
    assert_eq!(sd.resolved.held_item, 270);
    assert_eq!(sd.resolved.level, 100);
    assert_eq!(sd.options.moves[2], ["Direct Toxik"]);
    assert_eq!(sd.options.items, ["Baie Nanone"]);
    assert_eq!(sets[2].resolved.level, 5);
    assert_eq!(sets[0].resolved.ivs[3], 0);
    assert!(sets[0].resolved.warnings.iter().any(|w| w.contains("combat")));

    // Motisma de base : son set d'abord, puis celui de Motisma Lavage.
    let rotom = data.sets_for(Game::SM, 479, 0);
    assert_eq!(rotom.len(), 2);
    assert!(rotom[0].same_form && !rotom[1].same_form);
    assert_eq!(rotom[0].resolved.moves[3], HIDDEN_POWER);
    assert_eq!(hidden_power_type(rotom[0].resolved.ivs), 14);
    assert_eq!((rotom[1].resolved.species, rotom[1].resolved.form), (479, 2));

    // Appliquer un set Smogon dans une nouvelle case.
    let mut s = sm_save();
    let report = s.import_sets(std::slice::from_ref(&sd.set), Lang::En, ImportTarget::Box { r#box: 2 }).unwrap();
    assert_eq!(report.imported, 1);
    let p = s.get(report.sets[0].slot.unwrap()).unwrap().unwrap();
    assert_eq!(p.moves(), [14, 89, 444, 424]);
    assert_eq!(p.ability(), 24);
}
