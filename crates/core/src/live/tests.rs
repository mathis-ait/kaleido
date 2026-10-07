use super::reader::{read_party_at, Console, LiveReader};
use super::scan::{self, DS_HEADER_OFFSET, DS_RAM_SIZE};
use super::*;
use crate::save::{PkmFormat, Pokemon, RamHints};

/// RAM de melonDS 1.1 figée sur Argent SoulSilver FR (IPGF), en jeu, à Bourg Geon (sauvegarde faite
/// dans le labo du Pr Orme) :
/// en-tête de cartouche et bloc général de la sauvegarde (dump creux, voir `DumpSource::from_sparse`).
/// Produit avec `cargo run -p kaleido-core --example live_probe -- fixture <pid> <sav> <sortie>`.
const SS: &[u8] = include_bytes!("../../tests/data/live/melonds_ss_fr.kldram");
/// Même chose sur Blanche FR randomisée par Kaleido (IRAF), dans la chambre du joueur.
const W: &[u8] = include_bytes!("../../tests/data/live/melonds_w_fr_kaleido.kldram");

/// Repères tels que `SaveFile::ram_hints` les donne pour la sauvegarde SoulSilver de test.
fn ss_hints() -> RamHints {
    RamHints {
        format: PkmFormat::Gen4,
        party: 0x40098,
        party_count: 0x40094,
        trainer_name: 0x40064,
        trainer_name_bytes: vec![0x31, 0x01, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        tid: 0x6E14,
        sid: 0xF0CC,
        tid_offset: 0x40074,
        map: 0x41234,
        badges: Some(0x4007E),
        hours: 0x40086,
        party_keys: vec![0x3205_508B],
    }
}

fn w_hints() -> RamHints {
    RamHints {
        format: PkmFormat::Gen5,
        party: 0x18E08,
        party_count: 0x18E04,
        trainer_name: 0x19404,
        trainer_name_bytes: vec![0x54, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        tid: 0x3C30,
        sid: 0x6B73,
        tid_offset: 0x19414,
        map: 0x19580,
        badges: Some(0x21204),
        hours: 0x19424,
        party_keys: vec![0x02AA_1BED, 0xA0FF_F7BD],
    }
}

fn ss() -> DumpSource {
    DumpSource::from_sparse(SS).expect("dump creux lisible")
}

fn attach(src: &DumpSource, hints: RamHints) -> LiveReader {
    let ram = scan::find_ds_ram(src).expect("RAM DS trouvée");
    LiveReader::new(Console::Ds(ram), hints)
}

#[test]
fn ram_ds_trouvee_par_l_en_tete_de_cartouche() {
    let src = ss();
    let ram = scan::find_ds_ram(&src).unwrap();
    assert_eq!(ram.game_code, "IPGF");
    assert_eq!(ram.title, "POKEMON SS");
    assert_eq!(ram.base, src.regions()[0].base);
    assert_eq!(ram.host(0x027F_FE00), ram.base + DS_HEADER_OFFSET);
    assert!(maps::for_ds_code(&ram.game_code).is_some_and(|m| m.party_verified("IPGF")));
}

#[test]
fn copie_de_la_rom_ecartee() {
    // Une ROM chargée en mémoire commence par le même en-tête : la RAM supposée commencerait
    // 0x3FFE00 octets avant son allocation, elle ne doit pas être retenue.
    let mut rom = vec![0u8; DS_RAM_SIZE as usize];
    rom[..0x12].copy_from_slice(b"POKEMON SS\0\0IPGF01");
    let src = DumpSource::new().with_zone(0x1000_0000, rom);
    assert!(scan::find_ds_ram(&src).is_none());
}

#[test]
fn equipe_carte_et_badges_lus_en_ram() {
    let src = ss();
    let mut reader = attach(&src, ss_hints());
    let read = reader.tick(&src).unwrap().expect("équipe lue");
    assert_eq!(read.party.len(), 1);
    let p = &read.party[0];
    assert_eq!(p.species(), 158, "Kaiminus");
    assert_eq!(p.party_level(), Some(5));
    assert_eq!((p.current_hp(), p.party_stats().unwrap()[0]), (20, 20));
    // La sauvegarde dit 61 (labo) : la RAM a été figée dehors, après quelques pas (60).
    assert_eq!(read.map, Some(60));
    assert_eq!(read.badges, Some(0));
}

#[test]
fn noir_blanc_bloc_dresseur_apres_l_equipe() {
    let src = DumpSource::from_sparse(W).unwrap();
    let ram = scan::find_ds_ram(&src).unwrap();
    assert_eq!(ram.game_code, "IRAF");
    let mut reader = LiveReader::new(Console::Ds(ram), w_hints());
    let read = reader.tick(&src).unwrap().expect("équipe lue");
    let party: Vec<(u16, u16)> = read.party.iter().map(|p| (p.species(), p.current_hp())).collect();
    assert_eq!(party, vec![(230, 16), (6, 24)]);
    assert_eq!(read.map, Some(391));
    assert_eq!(reader.party_addresses().len(), 2, "deux copies de l'équipe en RAM");
}

/// Adresse de l'équipe du bloc de sauvegarde dans le dump SoulSilver.
fn ss_party(src: &DumpSource) -> (LiveReader, u64) {
    let mut reader = attach(src, ss_hints());
    reader.tick(src).unwrap().unwrap();
    let addr = reader.party_addresses()[0];
    (reader, addr)
}

#[test]
fn pv_qui_changent_en_direct() {
    let mut src = ss();
    let (mut reader, addr) = ss_party(&src);
    let raw = src.read_vec(addr, PkmFormat::Gen4.party_size()).unwrap();
    let mut p = Pokemon::from_encrypted(PkmFormat::Gen4, &raw).unwrap();
    p.set_current_hp(7);
    src.poke(addr, &p.encrypt_party());
    let read = reader.tick(&src).unwrap().unwrap();
    assert_eq!(read.party[0].current_hp(), 7);
}

#[test]
fn pokemon_lu_pendant_une_ecriture_rejete() {
    let mut src = ss();
    let (mut reader, addr) = ss_party(&src);
    let good = src.read_vec(addr + 0x20, 4).unwrap();
    src.poke(addr + 0x20, &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert!(reader.tick(&src).unwrap().is_none(), "somme de contrôle fausse : instantané rejeté");
    src.poke(addr + 0x20, &good);
    assert!(reader.tick(&src).unwrap().is_some(), "relu au tick suivant");
}

/// Équipe adverse synthétique `[6][n][Pokémon chiffrés]` posée dans une zone libre du dump.
fn put_enemy(src: &mut DumpSource, at: u64, species: u16, pid: u32, ids: (u16, u16)) {
    let mut p = Pokemon::blank(PkmFormat::Gen4);
    p.set_pid(pid);
    p.set_species(species);
    p.set_tid(ids.0);
    p.set_sid(ids.1);
    p.set_party_stats(4, [17, 9, 8, 7, 8, 10]);
    p.set_current_hp(17);
    let mut bytes = 6u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&p.encrypt_party());
    src.poke(at, &bytes);
}

/// Copie de combat de notre équipe (`[6][n]` + nos Pokémon), comme le jeu la pose en combat.
fn put_our_copy(src: &mut DumpSource, at: u64, party_addr: u64, n: usize) {
    let size = PkmFormat::Gen4.party_size();
    let mut bytes = 6u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(n as u32).to_le_bytes());
    bytes.extend_from_slice(&src.read_vec(party_addr, n * size).unwrap());
    src.poke(at, &bytes);
}

/// Combat posé en zone libre : notre copie, puis l'adversaire 0x560 octets plus loin (Blanche).
const FIGHT: u64 = 0x10_0000;
const FOE: u64 = FIGHT + 0x560;

fn battle_of(reader: &mut LiveReader, src: &DumpSource) -> Option<crate::live::reader::Battle> {
    (0..4).find_map(|_| reader.tick(src).unwrap().and_then(|r| r.battle))
}

#[test]
fn rencontre_sauvage_detectee_une_seule_fois() {
    let mut src = ss();
    let (mut reader, addr) = ss_party(&src);
    reader.prime_battle(&src);
    let base = scan::find_ds_ram(&src).unwrap().base;
    put_our_copy(&mut src, base + FIGHT, addr, 1);
    put_enemy(&mut src, base + FOE, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    let b = battle_of(&mut reader, &src).expect("combat vu");
    assert!(b.new && b.wild);
    assert_eq!(b.enemies[0].species(), 263);
    assert_eq!(b.ours.len(), 1, "notre équipe lue dans la copie de combat");
    // Le combat continue : plus « nouveau ».
    let again = battle_of(&mut reader, &src).unwrap();
    assert!(!again.new);
    // Équipe d'un dresseur : identifiants différents des nôtres.
    put_enemy(&mut src, base + FOE, 16, 0x0BAD_CAFE, (1, 2));
    let trainer = battle_of(&mut reader, &src).unwrap();
    assert!(trainer.new && !trainer.wild);
}

#[test]
fn equipe_adverse_isolee_ignoree() {
    // Ancien combat resté en RAM loin de toute copie de combat : ce n'est pas un combat en cours.
    let mut src = ss();
    let (mut reader, _) = ss_party(&src);
    reader.prime_battle(&src);
    let base = scan::find_ds_ram(&src).unwrap().base;
    put_enemy(&mut src, base + FOE, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    assert!(battle_of(&mut reader, &src).is_none());
}

#[test]
fn ancien_combat_en_ram_pas_annonce() {
    let mut src = ss();
    let base = scan::find_ds_ram(&src).unwrap().base;
    let (mut reader, addr) = ss_party(&src);
    put_our_copy(&mut src, base + FIGHT, addr, 1);
    put_enemy(&mut src, base + FOE, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    reader.prime_battle(&src);
    assert!(battle_of(&mut reader, &src).is_none(), "déjà en mémoire à l'attache : combat terminé");
}

#[test]
fn combat_fini_quand_le_jeu_reecrit_l_equipe() {
    let mut src = ss();
    let (mut reader, addr) = ss_party(&src);
    reader.prime_battle(&src);
    let base = scan::find_ds_ram(&src).unwrap().base;
    put_our_copy(&mut src, base + FIGHT, addr, 1);
    put_enemy(&mut src, base + FOE, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    assert!(battle_of(&mut reader, &src).is_some());
    // Fin du combat : le jeu recopie l'équipe (PV, expérience) dans le bloc de sauvegarde.
    let raw = src.read_vec(addr, PkmFormat::Gen4.party_size()).unwrap();
    let mut p = Pokemon::from_encrypted(PkmFormat::Gen4, &raw).unwrap();
    p.set_current_hp(p.current_hp().saturating_sub(3));
    src.poke(addr, &p.encrypt_party());
    assert!(battle_of(&mut reader, &src).is_none(), "les restes du combat ne comptent plus");
}

#[test]
fn dump_creux_aller_retour() {
    let src = ss();
    let base = src.regions()[0].base;
    let bytes = DumpSource::to_sparse(&src, base, DS_RAM_SIZE as u32, &[(DS_HEADER_OFFSET as u32, 0x20)]).unwrap();
    let back = DumpSource::from_sparse(&bytes).unwrap();
    assert_eq!(scan::ds_header_at(&back, base + DS_HEADER_OFFSET).unwrap().1, "IPGF");
}

#[test]
fn equipe_vide_ou_illisible() {
    let src = DumpSource::new().with_zone(0, vec![0; 0x1000]);
    assert!(read_party_at(&src, PkmFormat::Gen4, 0x100).unwrap().is_none());
}

#[test]
fn cartes_memoire_chargees() {
    let games: Vec<&str> = maps::games().iter().map(|g| g.game.as_str()).collect();
    assert_eq!(games, ["DP", "Pt", "HGSS", "BW", "B2W2", "XY", "ORAS", "SM", "USUM"]);
    assert!(maps::for_ds_code("CPUE").is_some());
    assert!(maps::for_ds_code("CPUF").is_some_and(|m| !m.party_verified("CPUF")));
    assert!(maps::for_title_id(0x0004_0000_0017_5E00).is_some_and(|m| m.game == "SM"));
    assert!(maps::for_ds_code("ZZZZ").is_none());
}

#[test]
fn ram_dsi_de_16_mio() {
    // Noir 2 lancé en console DSi : en-tête en 0x02FFFE00, RAM principale de 16 Mio.
    let mut ram = vec![0u8; scan::DSI_RAM_SIZE as usize];
    let at = scan::DSI_HEADER_OFFSET as usize;
    ram[at..at + 0x12].copy_from_slice(b"POKEMON B2\0\0IREF01");
    let src = DumpSource::new().with_zone(0x4000_0000, ram);
    let found = scan::find_ds_ram(&src).unwrap();
    assert_eq!((found.game_code.as_str(), found.size), ("IREF", scan::DSI_RAM_SIZE));
    assert_eq!(found.host(0x02FF_FE00), 0x4000_0000 + scan::DSI_HEADER_OFFSET);
}

/// Scénario Nuzlocke de `diff_tests` (Platine) rejoué en mémoire : à chaque étape, le bloc général
/// de la partie est recopié dans une RAM DS figée, sans aucune sauvegarde intermédiaire. La lecture
/// en direct doit annoncer chaque K.O. dès le tick suivant et ne jamais annoncer de mort.
#[test]
fn scenario_nuzlocke_rejoue_en_memoire() {
    use crate::save::diff::{diff_memory, Brief, GameEvent};
    use crate::save::session::{PokemonPatch, SaveSession, Slot};

    const BASE: u64 = 0x2_0000_0000;
    const AT: u64 = 0x10_0000;
    let mut s = SaveSession::open(&crate::save::demo_save().unwrap()).unwrap();
    // La sauvegarde de départ : seule vérité « fichier » pendant tout le scénario.
    let file = s.live().unwrap();
    let hints = s.save.ram_hints();
    let start = hints.trainer_name.min(hints.party).saturating_sub(0x100);
    let end = hints.map.max(hints.party + 6 * 236) + 0x100;

    let mut ram = vec![0u8; DS_RAM_SIZE as usize];
    ram[DS_HEADER_OFFSET as usize..][..0x12].copy_from_slice(b"POKEMON PL\0\0CPUF01");
    let mut src = DumpSource::new().with_zone(BASE, ram);
    let sync = |src: &mut DumpSource, s: &SaveSession| {
        // Le jeu garde l'équipe chiffrée en RAM, au même endroit relatif que dans le fichier.
        let bytes = s.save.to_bytes();
        src.poke(BASE + AT, &bytes[start..end]);
    };
    sync(&mut src, &s);
    let mut reader = attach(&src, hints);
    let mut prev: Option<Brief> = None;
    let mut step = |src: &mut DumpSource, s: &SaveSession| -> Vec<String> {
        sync(src, s);
        let read = reader.tick(src).unwrap().expect("équipe lue");
        let brief = Brief::from(&file.with_memory(&read.party, read.map, read.badges));
        let events = prev.as_ref().map(|p| diff_memory(p, &brief)).unwrap_or_default();
        prev = Some(brief);
        events
            .iter()
            .map(|e| match e {
                GameEvent::Fainted { mon } => format!("ko {}", mon.species),
                GameEvent::Revived { mon } => format!("soin {}", mon.species),
                GameEvent::Caught { mon } => format!("capture {}", mon.species),
                GameEvent::LevelUp { mon, .. } => format!("niveau {} {}", mon.species, mon.level),
                GameEvent::Evolved { mon, .. } => format!("évolution {}", mon.species),
                other => format!("autre {other:?}"),
            })
            .collect()
    };
    let p3 = Slot::Party { index: 3 };
    let p1 = Slot::Party { index: 1 };
    let set_hp = |s: &mut SaveSession, slot: Slot, hp: u16| {
        let Slot::Party { index } = slot else { unreachable!() };
        let mut p = s.get(slot).unwrap().unwrap();
        p.set_current_hp(hp);
        s.save.set_party_slot(index, Some(p)).unwrap();
    };
    let level = |s: &mut SaveSession, slot: Slot, l: u8, species: Option<u16>| {
        let patch = PokemonPatch { level: Some(l), species, ..PokemonPatch::default() };
        s.patch(slot, &patch).unwrap();
    };

    assert!(step(&mut src, &s).is_empty());
    s.create(p3, 396, 3).unwrap();
    assert_eq!(step(&mut src, &s), ["capture 396"]);
    level(&mut s, p3, 5, None);
    assert_eq!(step(&mut src, &s), ["niveau 396 5"]);
    // Capture rangée au PC : invisible en mémoire (boîtes non relues), annoncée à la sauvegarde.
    s.create(Slot::Box { r#box: 1, index: 0 }, 399, 4).unwrap();
    assert!(step(&mut src, &s).is_empty());
    set_hp(&mut s, p3, 0);
    assert_eq!(step(&mut src, &s), ["ko 396"]);
    assert!(step(&mut src, &s).is_empty(), "pas de K.O. répété");
    set_hp(&mut s, p3, 10);
    assert_eq!(step(&mut src, &s), ["soin 396"]);
    level(&mut s, p3, 14, Some(397));
    assert_eq!(step(&mut src, &s), ["évolution 397", "niveau 397 14"]);
    set_hp(&mut s, p3, 0);
    assert_eq!(step(&mut src, &s), ["ko 397"]);
    // Déposé K.O. au PC : la mort reste comptée à la sauvegarde, jamais en mémoire.
    s.move_pokemon(p3, Slot::Box { r#box: 2, index: 0 }).unwrap();
    assert!(step(&mut src, &s).is_empty());
    set_hp(&mut s, p1, 0);
    assert_eq!(step(&mut src, &s), ["ko 448"]);
    s.delete(p1).unwrap();
    assert!(step(&mut src, &s).is_empty(), "relâché K.O. : pas de mort annoncée par la mémoire");
}

#[test]
fn seconde_copie_reutilisee_par_l_adversaire_ignoree() {
    // SoulSilver : pendant un combat, l'emplacement de la seconde copie de l'équipe reçoit
    // l'équipe adverse. Le compagnon ne doit pas y voir une évolution de notre Pokémon.
    let mut src = ss();
    let base = scan::find_ds_ram(&src).unwrap().base;
    let saved = ss_party(&src).1;
    // Seconde copie de notre équipe, présente avant le combat.
    put_our_copy(&mut src, base + FIGHT, saved, 1);
    let mut reader = attach(&src, ss_hints());
    let before = reader.tick(&src).unwrap().unwrap().party[0].species();
    assert!(reader.party_addresses().contains(&(base + FIGHT + 8)), "seconde copie suivie");
    put_enemy(&mut src, base + FIGHT, 10, 0x0BAD_CAFE, (0x6E14, 0xF0CC));
    for _ in 0..3 {
        if let Some(r) = reader.tick(&src).unwrap() {
            assert_eq!(r.party[0].species(), before, "notre équipe reste la nôtre");
        }
    }
}
