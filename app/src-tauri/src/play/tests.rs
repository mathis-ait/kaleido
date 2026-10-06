use super::*;

/// Dossier temporaire supprimé à la fin du test.
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Tmp {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("kaleido-play-{name}-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn file(&self, rel: &str, data: &[u8]) -> PathBuf {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn resolved(id: EmulatorId, exe: Option<PathBuf>, profile: Profile, env: Env) -> Resolved {
    Resolved { id, exe, detected: false, profile, env }
}

#[test]
fn detects_install_and_portable_folders() {
    let t = Tmp::new("detect");
    let pf = t.0.join("Program Files");
    let melon = t.file("Program Files/melonDS/melonDS.exe", b"MZ");
    let azahar = t.file("Local/Programs/Azahar/azahar.exe", b"MZ");
    let desmume = t.file("Portable/emus/DeSmuME 0.9.13/DeSmuME_0.9.13_x64.exe", b"MZ");
    let env = Env { program_files: vec![pf], appdata: None, local_appdata: Some(t.0.join("Local")), extra: vec![t.0.join("Portable")], known_exes: vec![] };
    assert_eq!(detect_exe(EmulatorId::Melonds, &env), Some(melon));
    assert_eq!(detect_exe(EmulatorId::Azahar, &env), Some(azahar));
    assert_eq!(detect_exe(EmulatorId::Desmume, &env), Some(desmume));
    assert_eq!(detect_exe(EmulatorId::Citra, &env), None);
}

#[test]
fn chosen_exe_wins_over_detection() {
    let t = Tmp::new("chosen");
    let custom = t.file("ailleurs/melonDS.exe", b"MZ");
    t.file("pf/melonDS/melonDS.exe", b"MZ");
    let env = Env { program_files: vec![t.0.join("pf")], ..Default::default() };
    let mut config = PlayConfig::default();
    config.profiles.insert(EmulatorId::Melonds, Profile { exe: Some(custom.clone()), ..Default::default() });
    let r = resolve(EmulatorId::Melonds, &config, &env);
    assert_eq!(r.exe, Some(custom));
    assert!(!r.detected);
    // Exécutable choisi disparu : on retombe sur la détection.
    config.profiles.insert(EmulatorId::Melonds, Profile { exe: Some(t.0.join("absent.exe")), ..Default::default() });
    let r = resolve(EmulatorId::Melonds, &config, &env);
    assert!(r.detected);
}

#[test]
fn config_roundtrip() {
    let mut config = PlayConfig { preferred_nds: Some(EmulatorId::Desmume), ..Default::default() };
    config.profiles.insert(EmulatorId::Lime3ds, Profile { user_dir: Some("D:/3ds".into()), ..Default::default() });
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"lime3ds\"") && json.contains("preferredNds"));
    assert_eq!(serde_json::from_str::<PlayConfig>(&json).unwrap(), config);
    assert_eq!(serde_json::from_str::<PlayConfig>("{}").unwrap(), PlayConfig::default());
}

#[test]
fn melonds_save_next_to_rom_by_default() {
    let t = Tmp::new("melon");
    let exe = t.file("melon/melonDS.exe", b"MZ");
    let rom = t.0.join("roms/Platine - Kaleido 42.nds");
    let r = resolved(EmulatorId::Melonds, Some(exe), Profile::default(), Env::default());
    assert_eq!(r.nds_save_path(&rom), t.0.join("roms").join("Platine - Kaleido 42.sav"));
}

#[test]
fn melonds_save_dir_from_toml_and_override() {
    let t = Tmp::new("melon-toml");
    let exe = t.file("melon/melonDS.exe", b"MZ");
    t.file("melon/melonDS.toml", b"[Instance0]\nSaveFilePath = \"D:\\\\Saves\\\\DS\"\n\n[Instance1]\nSaveFilePath = \"X:\"\n");
    let rom = t.0.join("Noir.nds");
    let r = resolved(EmulatorId::Melonds, Some(exe.clone()), Profile::default(), Env::default());
    assert_eq!(r.nds_save_path(&rom), PathBuf::from("D:\\Saves\\DS").join("Noir.sav"));
    let r = resolved(EmulatorId::Melonds, Some(exe), Profile { save_dir: Some(t.0.join("mes")), ..Default::default() }, Env::default());
    assert_eq!(r.nds_save_path(&rom), t.0.join("mes").join("Noir.sav"));
}

#[test]
fn melonds_empty_setting_and_appdata_config() {
    let t = Tmp::new("melon-appdata");
    let exe = t.file("melon/melonDS.exe", b"MZ");
    t.file("Local/melonDS/melonDS.toml", b"[Instance0]\nSaveFilePath = \"\"\n");
    let env = Env { local_appdata: Some(t.0.join("Local")), ..Default::default() };
    let r = resolved(EmulatorId::Melonds, Some(exe), Profile::default(), env);
    assert_eq!(r.nds_save_dir(), None);
}

#[test]
fn desmume_battery_folder() {
    let t = Tmp::new("desmume");
    let exe = t.file("DeSmuME/DeSmuME_0.9.13_x64.exe", b"MZ");
    let rom = t.0.join("roms/Platine.nds");
    let r = resolved(EmulatorId::Desmume, Some(exe.clone()), Profile::default(), Env::default());
    assert_eq!(r.nds_save_path(&rom), t.0.join("DeSmuME").join("Battery").join("Platine.dsv"));
    t.file("DeSmuME/desmume.ini", b"[PathSettings]\nRoms=.\\Roms\\\nBattery=.\\Saves\\\n");
    assert_eq!(r.nds_save_path(&rom), t.0.join("DeSmuME").join("Saves").join("Platine.dsv"));
    t.file("DeSmuME/desmume.ini", format!("[PathSettings]\nBattery={}\n", t.0.join("abs").display()).as_bytes());
    assert_eq!(r.nds_save_path(&rom), t.0.join("abs").join("Platine.dsv"));
}

#[test]
fn ctr_user_dir_portable_then_appdata() {
    let t = Tmp::new("ctr-user");
    let exe = t.file("Azahar/azahar.exe", b"MZ");
    let env = Env { appdata: Some(t.0.join("Roaming")), ..Default::default() };
    let r = resolved(EmulatorId::Azahar, Some(exe), Profile::default(), env.clone());
    assert_eq!(r.ctr_user_dir(), Some(t.0.join("Roaming").join("Azahar")));
    fs::create_dir_all(t.0.join("Azahar/user")).unwrap();
    assert_eq!(r.ctr_user_dir(), Some(t.0.join("Azahar").join("user")));
    let r = resolved(EmulatorId::Lime3ds, None, Profile::default(), env);
    assert_eq!(r.ctr_user_dir(), Some(t.0.join("Roaming").join("Lime3DS")));
}

#[test]
fn title_id_and_mod_paths() {
    assert_eq!(parse_title_id("000400000011C400"), Some(0x0004_0000_0011_C400));
    assert_eq!(parse_title_id("11C400"), None);
    let romfs = Path::new("C:/out/luma/titles/000400000011C500/romfs");
    assert_eq!(title_id_of_romfs(romfs), Some(0x0004_0000_0011_C500));
    assert_eq!(mod_dir(Path::new("U"), 0x0004_0000_0011_C400), Path::new("U").join("load").join("mods").join("000400000011C400"));
}

#[test]
fn ctr_save_dir_uses_existing_ids() {
    let t = Tmp::new("ctr-save");
    let user = &t.0;
    let zeros = "0".repeat(32);
    let tail = ["title", "00040000", "0011c400", "data", "00000001"];
    let default: PathBuf = tail.iter().fold(user.join("sdmc").join("Nintendo 3DS").join(&zeros).join(&zeros), |p, s| p.join(s));
    assert_eq!(ctr_save_dir(user, 0x0004_0000_0011_C400), default);
    let id0 = "a".repeat(32);
    let id1 = "b".repeat(32);
    let real: PathBuf = tail.iter().fold(user.join("sdmc").join("Nintendo 3DS").join(&id0).join(&id1), |p, s| p.join(s));
    fs::create_dir_all(&real).unwrap();
    assert_eq!(ctr_save_dir(user, 0x0004_0000_0011_C400), real);
}

#[test]
fn backup_names_never_collide() {
    let t = Tmp::new("backup");
    let save = t.file("main", b"v1");
    let a = backup_path(&save, "20261005-101010");
    assert_eq!(a.file_name().unwrap(), "main.kaleido-20261005-101010.bak");
    fs::write(&a, b"x").unwrap();
    let b = backup_path(&save, "20261005-101010");
    assert_eq!(b.file_name().unwrap(), "main.kaleido-20261005-101010-2.bak");
    assert_eq!(backup_file(&t.0.join("absent"), "s").unwrap(), None);
}

#[test]
fn timestamp_format() {
    assert_eq!(timestamp(UNIX_EPOCH), "19700101-000000");
    // 2024-02-29 12:34:56 UTC
    assert_eq!(timestamp(UNIX_EPOCH + Duration::from_secs(1_709_210_096)), "20240229-123456");
}

#[test]
fn install_save_backs_up_existing() {
    let t = Tmp::new("install");
    let src = t.file("mine.sav", &[1u8; 16]);
    let dst = t.file("emu/Jeu.sav", &[2u8; 16]);
    let backup = install_save(&src, &dst, "S").unwrap().expect("copie de sécurité");
    assert_eq!(fs::read(&backup).unwrap(), vec![2u8; 16]);
    assert_eq!(fs::read(&dst).unwrap(), vec![1u8; 16]);
    // Deuxième envoi : nouvelle copie, l'ancienne n'est pas écrasée.
    let src2 = t.file("mine2.sav", &[3u8; 16]);
    let backup2 = install_save(&src2, &dst, "S").unwrap().unwrap();
    assert_ne!(backup, backup2);
    assert_eq!(fs::read(&backup).unwrap(), vec![2u8; 16]);
    assert_eq!(fs::read(&backup2).unwrap(), vec![1u8; 16]);
    // Même fichier : rien à faire.
    assert_eq!(install_save(&dst, &dst, "S").unwrap(), None);
    // Cible absente : pas de copie de sécurité, dossiers créés.
    let fresh = t.0.join("new/dir/Jeu.sav");
    assert_eq!(install_save(&src, &fresh, "S").unwrap(), None);
    assert!(fresh.is_file());
}

#[test]
fn dsv_conversion() {
    let footer = dsv_footer(0x80000);
    assert_eq!(footer.len(), DSV_FOOTER);
    assert!(footer.ends_with(b"|-DESMUME SAVE-|"));
    // Taille, taille alignée, type (FLASH 4 Mbit = 6), octets d'adresse (3), taille, version.
    let fields: Vec<u32> = footer[82..106].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(fields, vec![0x80000, 0x80000, 6, 3, 0x80000, 0]);

    let raw = vec![7u8; 0x80000];
    let dsv = convert_for(raw.clone(), Path::new("a.dsv"));
    assert_eq!(dsv.len(), 0x80000 + DSV_FOOTER);
    // Déjà au bon format : inchangé.
    assert_eq!(convert_for(dsv.clone(), Path::new("b.dsv")), dsv);
    assert_eq!(convert_for(dsv, Path::new("c.sav")), raw);
    assert_eq!(convert_for(raw.clone(), Path::new("main")), raw);
}

#[test]
fn mod_install_requires_confirmation_and_keeps_old() {
    let t = Tmp::new("mod");
    let romfs =
        t.file("out/000400000011C400/romfs/a/0/1/2", b"new").parent().unwrap().parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    assert_eq!(title_id_of_romfs(&romfs), Some(0x0004_0000_0011_C400));
    let user = t.0.join("user");
    let tid = 0x0004_0000_0011_C400;
    let (dir, backup) = install_mod(&romfs, &user, tid, false, "S").unwrap();
    assert_eq!(backup, None);
    assert_eq!(fs::read(dir.join("romfs/a/0/1/2")).unwrap(), b"new");
    // Mod déjà présent : refus sans confirmation.
    assert!(install_mod(&romfs, &user, tid, false, "S").is_err());
    fs::write(dir.join("romfs/a/0/1/2"), b"old").unwrap();
    let (_, backup) = install_mod(&romfs, &user, tid, true, "S").unwrap();
    let backup = backup.unwrap();
    assert!(backup.starts_with(mod_backup_root(&user)));
    assert_eq!(fs::read(backup.join("romfs/a/0/1/2")).unwrap(), b"old");
    assert_eq!(fs::read(dir.join("romfs/a/0/1/2")).unwrap(), b"new");
    // Mod généré directement dans le dossier « mods » : rien à copier.
    assert_eq!(install_mod(&dir.join("romfs"), &user, tid, false, "S").unwrap().1, None);
}

#[test]
fn plan_and_prepare_nds() {
    let t = Tmp::new("plan-nds");
    let exe = t.file("melon/melonDS.exe", b"MZ");
    let rom = t.file("roms/Platine.nds", b"rom");
    let existing = t.file("roms/Platine.sav", &[1u8; 32]);
    let mine = t.file("autre/ma partie.sav", &[9u8; 32]);
    let r = resolved(EmulatorId::Melonds, Some(exe), Profile::default(), Env::default());
    let req = PlayRequest { emulator: EmulatorId::Melonds, rom: Some(rom), mod_romfs: None, save: Some(mine), replace_mod: false, track_key: None };
    let p = plan(&req, &r, None);
    assert!(p.save_exists && p.save_conflict);
    let res = prepare_files(&req, &r, None, "S").unwrap();
    assert_eq!(res.save_path.map(PathBuf::from), Some(existing.clone()));
    assert_eq!(res.backups.len(), 1);
    assert_eq!(fs::read(&existing).unwrap(), vec![9u8; 32]);
    assert_eq!(fs::read(&res.backups[0]).unwrap(), vec![1u8; 32]);
}

#[test]
fn plan_and_prepare_ctr() {
    let t = Tmp::new("plan-ctr");
    let exe = t.file("Azahar/azahar.exe", b"MZ");
    fs::create_dir_all(t.0.join("Azahar/user")).unwrap();
    let game = t.file("jeux/Rubis Omega.3ds", b"game");
    let romfs = t.file("mod/000400000011C400/romfs/x", b"m").parent().unwrap().to_path_buf();
    let mine = t.file("ma.main", &[5u8; 8]);
    let r = resolved(EmulatorId::Azahar, Some(exe), Profile::default(), Env::default());
    let tid = Some(0x0004_0000_0011_C400);
    let req = PlayRequest { emulator: EmulatorId::Azahar, rom: Some(game), mod_romfs: Some(romfs), save: Some(mine.clone()), replace_mod: false, track_key: None };
    let p = plan(&req, &r, tid);
    assert!(!p.needs_game && !p.mod_exists && !p.save_exists);
    assert_eq!(p.title_id.as_deref(), Some("000400000011C400"));
    assert_eq!(p.warnings.len(), 1, "jeu jamais lancé : avertissement");
    // Jeu jamais lancé : le mod est installé mais la sauvegarde n'est pas copiée.
    let res = prepare_files(&req, &r, tid, "S").unwrap();
    assert!(res.mod_path.is_some() && res.warnings.len() == 1);
    let save = PathBuf::from(res.save_path.unwrap());
    assert!(!save.exists());
    // Archive créée par l'émulateur : la sauvegarde est copiée.
    fs::create_dir_all(save.parent().unwrap()).unwrap();
    let req = PlayRequest { replace_mod: true, ..req };
    let res = prepare_files(&req, &r, tid, "S").unwrap();
    assert!(res.warnings.is_empty());
    assert_eq!(fs::read(&save).unwrap(), vec![5u8; 8]);
    assert_eq!(res.backups.len(), 1, "ancien mod mis de côté");
}

#[test]
fn debounce_waits_for_stable_file() {
    let t0 = Instant::now();
    let ms = |n| t0 + Duration::from_millis(n);
    let mut d = Debouncer::new(Some((10, 1)), Duration::from_millis(1000));
    assert!(!d.observe(Some((10, 1)), ms(0)));
    // Écriture en plusieurs fois : on attend que le fichier ne bouge plus.
    assert!(!d.observe(Some((20, 2)), ms(100)));
    assert!(!d.observe(Some((30, 3)), ms(600)));
    assert!(!d.observe(Some((30, 3)), ms(1500)));
    assert!(d.observe(Some((30, 3)), ms(1700)));
    // Une seule notification par changement.
    assert!(!d.observe(Some((30, 3)), ms(3000)));
    // Fichier momentanément absent : pas de notification.
    assert!(!d.observe(None, ms(3100)));
    assert!(!d.observe(Some((30, 3)), ms(5000)));
    // Écriture faite par Kaleido : ignorée après resync.
    assert!(!d.observe(Some((40, 4)), ms(5100)));
    d.resync(Some((40, 4)));
    assert!(!d.observe(Some((40, 4)), ms(7000)));
}

#[test]
fn debounce_detects_new_file() {
    let t0 = Instant::now();
    let mut d = Debouncer::new(None, Duration::from_millis(500));
    assert!(!d.observe(None, t0));
    assert!(!d.observe(Some((1, 1)), t0 + Duration::from_millis(100)));
    assert!(d.observe(Some((1, 1)), t0 + Duration::from_millis(700)));
}

#[test]
fn version_parsing() {
    let mut exe = b"MZ....junk".to_vec();
    for (k, v) in [("FileVersion", "1.0.0.0"), ("ProductVersion", "1.0.0.0")] {
        exe.extend(k.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
        exe.extend([0, 0]);
        exe.extend(v.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    assert_eq!(pe_version(&exe).as_deref(), Some("1.0"));
    assert_eq!(pe_version(b"rien"), None);
    assert_eq!(version_from_name("DeSmuME_0.9.13_x64").as_deref(), Some("0.9.13"));
    assert_eq!(version_from_name("azahar-2121.2-windows-msvc").as_deref(), Some("2121.2"));
    assert_eq!(version_from_name("melonDS"), None);
    assert_eq!(tidy_version("0.9.5.0"), "0.9.5");
}

#[test]
fn ini_and_toml_values() {
    let toml = "[Instance0]\nSaveFilePath = 'C:\\Saves'\nOther = 1\n";
    assert_eq!(config_value(toml, Some("Instance0"), "SaveFilePath").as_deref(), Some("C:\\Saves"));
    assert_eq!(config_value(toml, Some("Instance1"), "SaveFilePath"), None);
    assert_eq!(config_value("SaveFilePath=D:\\x\n", None, "SaveFilePath").as_deref(), Some("D:\\x"));
}

#[test]
fn find_rom_beside_save() {
    let t = Tmp::new("findrom");
    let save = t.file("Noir.sav", b"s");
    assert_eq!(play_find_rom(save.clone()), None);
    let rom = t.file("Noir.nds", b"r");
    assert_eq!(play_find_rom(save), Some(rom.display().to_string()));
}

#[test]
fn azahar_play_time() {
    let user = std::env::temp_dir().join(format!("kaleido-playtime-{}", std::process::id()));
    fs::create_dir_all(user.join("sysdata")).unwrap();
    let mut data = Vec::new();
    for (id, secs) in [(0x0004_0000_0011_C400u64, 970u64), (0x0004_0000_0005_5E00, 39)] {
        data.extend_from_slice(&id.to_le_bytes());
        data.extend_from_slice(&secs.to_le_bytes());
    }
    fs::write(user.join("sysdata").join("play_time.bin"), data).unwrap();
    assert_eq!(emulator_play_time(&user, 0x0004_0000_0011_C400), Some(970));
    assert_eq!(emulator_play_time(&user, 0x0004_0000_0005_5E00), Some(39));
    assert_eq!(emulator_play_time(&user, 0x0004_0000_0016_4800), None);
    fs::remove_dir_all(&user).unwrap();
}
