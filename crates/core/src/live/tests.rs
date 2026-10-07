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

#[test]
fn rencontre_sauvage_detectee_une_seule_fois() {
    let mut src = ss();
    let (mut reader, _) = ss_party(&src);
    reader.prime_battle(&src);
    let base = scan::find_ds_ram(&src).unwrap().base;
    // Zone à zéro du dump (loin de l'équipe) : 0x100000.
    put_enemy(&mut src, base + 0x10_0000, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    let mut battle = None;
    for _ in 0..4 {
        if let Some(b) = reader.tick(&src).unwrap().and_then(|r| r.battle) {
            battle = Some(b);
            break;
        }
    }
    let b = battle.expect("combat vu");
    assert!(b.new && b.wild);
    assert_eq!(b.enemies[0].species(), 263);
    // Le combat continue : plus « nouveau ».
    let again = (0..4).find_map(|_| reader.tick(&src).unwrap().and_then(|r| r.battle)).unwrap();
    assert!(!again.new);
    // Équipe d'un dresseur : identifiants différents des nôtres.
    put_enemy(&mut src, base + 0x10_0000, 16, 0x0BAD_CAFE, (1, 2));
    let trainer = (0..4).find_map(|_| reader.tick(&src).unwrap().and_then(|r| r.battle)).unwrap();
    assert!(trainer.new && !trainer.wild);
}

#[test]
fn ancien_combat_en_ram_pas_annonce() {
    let mut src = ss();
    let base = scan::find_ds_ram(&src).unwrap().base;
    put_enemy(&mut src, base + 0x10_0000, 263, 0x1234_5678, (0x6E14, 0xF0CC));
    let (mut reader, _) = ss_party(&src);
    reader.prime_battle(&src);
    let b = (0..4).find_map(|_| reader.tick(&src).unwrap().and_then(|r| r.battle)).unwrap();
    assert!(!b.new, "déjà en mémoire à l'attache");
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
