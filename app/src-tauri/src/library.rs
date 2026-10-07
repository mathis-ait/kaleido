//! Bibliothèque de jeux : dossiers mémorisés, jaquettes et installation des
//! émulateurs en un clic.
//!
//! - Jaquettes : `cover://localhost/<jeu>.png`, boîte française en priorité
//!   (GameTDB, puis libretro-thumbnails), téléchargée une fois puis gardée dans
//!   `<données>/covers-fr`.
//! - Émulateurs : dernière release GitHub officielle (archive Windows .zip),
//!   décompressée dans `<données locales>/emulators/<nom>`, puis l'exécutable est
//!   enregistré dans le profil de l'émulateur (voir `play.rs`).

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::play::{self, EmulatorId, EmulatorsState};

pub(crate) const USER_AGENT: &str =concat!("Kaleido/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mathis-ait/kaleido)");

// ---------------------------------------------------------------------------
// Dossiers de la bibliothèque

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryConfig {
    /// Dossiers parcourus à chaque ouverture.
    pub folders: Vec<String>,
    /// Fichiers ajoutés un par un.
    pub files: Vec<String>,
    /// Fichiers retirés de la bibliothèque (même s'ils sont dans un dossier suivi).
    pub hidden: Vec<String>,
    /// Dossiers retirés par l'utilisateur : la recherche automatique ne les rajoute plus.
    pub ignored: Vec<String>,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("library.json"))
}

#[tauri::command]
pub fn library_config(app: AppHandle) -> LibraryConfig {
    config_path(&app).ok().and_then(|p| fs::read(p).ok()).and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn library_set_config(config: LibraryConfig, app: AppHandle) -> Result<(), String> {
    let path = config_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Jeux (ROMs et dossiers 3DS identifiés) contenus dans les dossiers et fichiers suivis.
#[tauri::command]
pub async fn library_scan(config: LibraryConfig) -> Result<Vec<kaleido_core::detect::Detection>, String> {
    crate::blocking(move || {
        let mut paths: Vec<PathBuf> = config.folders.iter().flat_map(|f| kaleido_core::detect::expand_path(Path::new(f))).collect();
        paths.extend(config.files.iter().map(PathBuf::from).filter(|p| p.exists()));
        paths.sort();
        paths.dedup();
        let games = paths
            .iter()
            .filter(|p| !config.hidden.iter().any(|h| Path::new(h) == p.as_path()))
            .filter_map(|p| kaleido_core::detect_path(p).ok())
            .filter(|d| d.game.is_some() && !matches!(d.kind, kaleido_core::detect::FileKind::Save | kaleido_core::detect::FileKind::Unknown))
            .collect();
        Ok(games)
    })
    .await
}

/// Jeux Switch des dossiers et fichiers suivis.
#[tauri::command]
pub async fn library_scan_switch(config: LibraryConfig, app: AppHandle) -> Result<Vec<crate::switch::SwitchGame>, String> {
    crate::blocking(move || {
        let roots: Vec<PathBuf> = config.folders.iter().chain(&config.files).map(PathBuf::from).collect();
        Ok(crate::switch::scan(&app, &roots, &config.hidden))
    })
    .await
}

// ---------------------------------------------------------------------------
// Jaquettes

const GAMETDB_URL: &str = "https://art.gametdb.com";
const LIBRETRO_URL: &str = "https://raw.githubusercontent.com/libretro-thumbnails";

/// Sources d'une boîte : GameTDB (plateforme, code produit européen) et noms No-Intro
/// libretro (dépôt, boîte française, boîte européenne si pas de française).
struct CoverSources {
    tdb: (&'static str, &'static str),
    repo: &'static str,
    fr: Option<&'static str>,
    eu: Option<&'static str>,
}

fn cover_sources(game: &str) -> Option<CoverSources> {
    const DS: &str = "Nintendo_-_Nintendo_DS";
    const CTR: &str = "Nintendo_-_Nintendo_3DS";
    let ds = |code, fr| CoverSources { tdb: ("ds", code), repo: DS, fr: Some(fr), eu: None };
    let ctr = |code, eu| CoverSources { tdb: ("3ds", code), repo: CTR, fr: None, eu: Some(eu) };
    Some(match game {
        "diamond" => ds("ADAF", "Pokemon - Version Diamant (France) (Rev 5)"),
        "pearl" => ds("APAF", "Pokemon - Version Perle (France) (Rev 5)"),
        "platinum" => ds("CPUF", "Pokemon - Version Platine (France)"),
        "heart_gold" => ds("IPKF", "Pokemon - Version Or HeartGold (France)"),
        "soul_silver" => ds("IPGF", "Pokemon - Version Argent SoulSilver (France)"),
        "black" => ds("IRBF", "Pokemon - Version Noire (France) (NDSi Enhanced)"),
        "white" => ds("IRAF", "Pokemon - Version Blanche (France) (NDSi Enhanced)"),
        "black2" => ds("IREF", "Pokemon - Version Noire 2 (France) (NDSi Enhanced)"),
        "white2" => ds("IRDF", "Pokemon - Version Blanche 2 (France) (NDSi Enhanced)"),
        // Pas de boîte française chez GameTDB pour X et Y : la boîte européenne est la même en France.
        "x" => ctr("EKJP", "Pokemon X (Europe) (En,Ja,Fr,De,Es,It,Ko)"),
        "y" => ctr("EK2P", "Pokemon Y (Europe) (En,Ja,Fr,De,Es,It,Ko)"),
        "omega_ruby" => ctr("ECRP", "Pokemon Omega Ruby (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2)"),
        "alpha_sapphire" => ctr("ECLP", "Pokemon Alpha Sapphire (Europe) (En,Ja,Fr,De,Es,It,Ko) (Rev 2)"),
        "sun" => ctr("BNDP", "Pokemon Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)"),
        "moon" => ctr("BNEP", "Pokemon Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)"),
        "ultra_sun" => ctr("A2AP", "Pokemon Ultra Sun (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)"),
        "ultra_moon" => ctr("A2BP", "Pokemon Ultra Moon (Europe) (En,Ja,Fr,De,Es,It,Zh,Ko)"),
        _ => return None,
    })
}

fn libretro_url(repo: &str, name: &str) -> String {
    // Les noms No-Intro ne contiennent que des lettres, chiffres, espaces, virgules et parenthèses.
    let encoded = name.replace(' ', "%20").replace(',', "%2C").replace('(', "%28").replace(')', "%29");
    format!("{LIBRETRO_URL}/{repo}/master/Named_Boxarts/{encoded}.png")
}

/// Adresses à essayer, de la meilleure à la moins bonne. La boîte française passe
/// toujours en premier, quelle que soit la langue de la ROM : GameTDB en haute
/// définition (768×680), libretro, GameTDB en taille moyenne (400×352), puis la
/// boîte européenne.
pub fn cover_urls(game: &str) -> Vec<String> {
    // Jeu Switch (`nx-<title ID>`) : icône officielle.
    if let Some(tid) = switch_cover_id(game) {
        return vec![format!("https://api.nlib.cc/nx/{tid}/icon/512/512")];
    }
    let Some(s) = cover_sources(game) else { return Vec::new() };
    let (platform, code) = s.tdb;
    let mut urls = vec![format!("{GAMETDB_URL}/{platform}/coverHQ/FR/{code}.jpg")];
    urls.extend(s.fr.map(|n| libretro_url(s.repo, n)));
    urls.push(format!("{GAMETDB_URL}/{platform}/coverM/FR/{code}.jpg"));
    urls.extend(s.eu.map(|n| libretro_url(s.repo, n)));
    urls
}

/// `nx-01001f5010dfa000` → « 01001F5010DFA000 ».
fn switch_cover_id(key: &str) -> Option<String> {
    let tid = key.strip_prefix("nx-")?;
    (tid.len() == 16 && tid.bytes().all(|b| b.is_ascii_hexdigit())).then(|| tid.to_ascii_uppercase())
}

fn covers_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    // `covers-fr` : l'ancien cache (`covers`) pouvait contenir des boîtes anglaises.
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("covers-fr");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else {
        None
    }
}

fn load_cover(dir: &Path, game: &str) -> Result<Vec<u8>, String> {
    let file = dir.join(format!("{game}.img"));
    if let Ok(data) = fs::read(&file) {
        return Ok(data);
    }
    let missing = dir.join(format!("{game}.missing"));
    if missing.exists() {
        return Err("pas de jaquette".into());
    }
    let urls = cover_urls(game);
    if urls.is_empty() {
        return Err("jeu inconnu".into());
    }
    let mut offline = None;
    for url in urls {
        match crate::sprites::fetch(&url) {
            Ok(Some(data)) if image_type(&data).is_some() => {
                let _ = fs::write(&file, &data);
                return Ok(data);
            }
            Ok(_) => {}
            Err(e) => offline = Some(e),
        }
    }
    // Hors ligne : on réessaiera la prochaine fois ; sinon, aucune source n'a d'image.
    if let Some(e) = offline {
        return Err(e);
    }
    let _ = fs::write(&missing, b"");
    Err("pas de jaquette".into())
}

/// Protocole `cover://localhost/<jeu>.png` (voir l'en-tête du module).
pub fn handle_cover<R: Runtime>(app: &AppHandle<R>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let key = request.uri().path().trim_start_matches('/').trim_end_matches(".png");
    // Ancien format `<jeu>-en` : la boîte française est désormais toujours servie.
    let game = key.strip_suffix("-en").unwrap_or(key).to_string();
    let valid = switch_cover_id(&game).is_some() || (!game.is_empty() && game.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    let result = if valid { covers_dir(app).and_then(|dir| load_cover(&dir, &game)) } else { Err("nom invalide".into()) };
    match result {
        Ok(data) => Response::builder()
            .header("Content-Type", image_type(&data).unwrap_or("application/octet-stream"))
            .header("Cache-Control", "max-age=31536000, immutable")
            // Lecture des pixels (couleur dominante) depuis un <canvas>.
            .header("Access-Control-Allow-Origin", "*")
            .body(data)
            .unwrap(),
        Err(e) => Response::builder().status(StatusCode::NOT_FOUND).header("Content-Type", "text/plain; charset=utf-8").body(e.into_bytes()).unwrap(),
    }
}

// ---------------------------------------------------------------------------
// État d'un jeu : sauvegarde et temps de jeu

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    /// Émulateur utilisé par défaut pour ce jeu.
    emulator: Option<&'static str>,
    /// Sauvegarde que l'émulateur utilise pour ce jeu.
    save_path: Option<String>,
    save_exists: bool,
    /// Nom du dresseur de la sauvegarde.
    trainer: Option<String>,
    /// Temps de jeu affiché par le jeu (carte de dresseur), en secondes.
    save_seconds: Option<u64>,
    /// Temps compté par l'émulateur (Azahar, Citra, Lime3DS).
    emulator_seconds: Option<u64>,
    /// Temps passé dans l'émulateur pour les parties lancées par Kaleido.
    kaleido_seconds: Option<u64>,
}

/// Sauvegarde, dresseur et temps de jeu d'un jeu de la bibliothèque.
#[tauri::command]
pub async fn game_status(rom: Option<PathBuf>, mod_romfs: Option<PathBuf>, ctr: bool, key: String, app: AppHandle) -> Result<GameStatus, String> {
    crate::blocking(move || {
        let mut status = GameStatus { kaleido_seconds: play::tracked_play_time(&app).get(&key).copied(), ..Default::default() };
        let config = play::load_config(&app);
        let Some(r) = play::default_emulator(&config, &play::Env::system(&config.search_dirs), ctr) else { return Ok(status) };
        status.emulator = Some(r.id.name());
        let request = play::PlayRequest { emulator: r.id, rom, mod_romfs, save: None, replace_mod: false, track_key: None };
        let tid = if ctr { play::request_title_id(&request) } else { None };
        let plan = play::plan(&request, &r, tid);
        status.save_exists = plan.save_exists;
        status.save_path = plan.save_path;
        if let (Some(tid), Some(user)) = (tid, r.ctr_user_dir()) {
            status.emulator_seconds = play::emulator_play_time(&user, tid).filter(|&s| s > 0);
        }
        if status.save_exists {
            let trainer = status
                .save_path
                .as_ref()
                .and_then(|p| fs::read(p).ok())
                .and_then(|bytes| kaleido_core::save::session::SaveSession::open(&bytes).ok())
                .and_then(|s| s.view().ok())
                .map(|v| v.trainer);
            if let Some(t) = trainer {
                let pt = &t.play_time;
                status.save_seconds = Some(pt.hours as u64 * 3600 + pt.minutes as u64 * 60 + pt.seconds as u64);
                status.trainer = Some(t.name);
            }
        }
        Ok(status)
    })
    .await
}

// ---------------------------------------------------------------------------
// Musique de l'écran titre

/// Durée de l'extrait joué dans le lanceur (il boucle).
pub(crate) const MUSIC_SECONDS: f32 = 50.0;

fn game_from_id(id: &str) -> Option<kaleido_core::games::Game> {
    kaleido_core::games::Game::ALL.into_iter().find(|g| serde_json::to_value(g).ok().and_then(|v| v.as_str().map(|s| s == id)).unwrap_or(false))
}

/// Clé de cache : chemin, taille et date de modification de la ROM.
pub(crate) fn music_key(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    // FNV-1a 64 bits.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.to_string_lossy().bytes().chain(meta.len().to_le_bytes()).chain(modified.to_le_bytes()) {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

/// Thème de l'écran titre d'une ROM, en WAV (rendu une fois puis gardé en cache).
#[tauri::command]
pub async fn music_title_theme(path: PathBuf, game: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::blocking(move || {
        let game = game_from_id(&game).ok_or("jeu inconnu")?;
        let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("music");
        let key = music_key(&path).ok_or("ROM introuvable")?;
        let cached = dir.join(format!("{key}.wav"));
        if let Ok(data) = fs::read(&cached) {
            return Ok(tauri::ipc::Response::new(data));
        }
        let wav = kaleido_core::music::title_theme(&path, game, MUSIC_SECONDS).map_err(|e| e.to_string())?.to_wav();
        if fs::create_dir_all(&dir).is_ok() {
            let _ = fs::write(&cached, &wav);
        }
        Ok(tauri::ipc::Response::new(wav))
    })
    .await
}

// ---------------------------------------------------------------------------
// Émulateurs en cours d'exécution

/// Noms des émulateurs connus présents dans la liste des processus Windows.
fn running_emulators() -> Vec<&'static str> {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW : pas de console qui clignote toutes les deux secondes.
        cmd.creation_flags(0x0800_0000);
    }
    let Ok(out) = cmd.output() else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let exes: Vec<&str> = text.lines().filter_map(|l| l.split(',').next()).map(|c| c.trim_matches('"')).collect();
    let mut found: Vec<&'static str> = EmulatorId::ALL.into_iter().filter(|id| exes.iter().any(|e| id.matches_exe(e))).map(|id| id.name()).collect();
    found.dedup();
    found
}

/// Dernier état connu (pour la commande `emulators_running`).
static RUNNING: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Surveille les émulateurs (toutes les 2 s) et émet `emulators-running` à chaque changement :
/// le lanceur coupe sa musique pendant qu'on joue.
pub fn watch_emulators(app: AppHandle) {
    std::thread::spawn(move || loop {
        let now = running_emulators();
        let changed = {
            let mut last = RUNNING.lock().unwrap();
            let changed = *last != now;
            *last = now.clone();
            changed
        };
        if changed {
            let _ = app.emit("emulators-running", now);
        }
        std::thread::sleep(Duration::from_secs(2));
    });
}

#[tauri::command]
pub fn emulators_running() -> Vec<&'static str> {
    RUNNING.lock().unwrap().clone()
}

// ---------------------------------------------------------------------------
// Installation des émulateurs

/// Dépôt officiel (GitHub, ou Forgejo d'Eden) et archive Windows 64 bits de chaque émulateur installable.
fn source(id: EmulatorId) -> Option<(&'static str, fn(&str) -> bool)> {
    Some(match id {
        EmulatorId::Melonds => ("melonDS-emu/melonDS", |n| n.contains("windows-x86_64") && n.ends_with(".zip")),
        EmulatorId::Azahar => ("azahar-emu/azahar", |n| n.starts_with("azahar-windows-msvc-") && n.ends_with(".zip") && !n.contains("installer")),
        EmulatorId::Desmume => ("TASEmulators/desmume", |n| n.ends_with("-win64.zip")),
        // Version MSVC : conseillée par Eden pour Pokémon Écarlate / Violet.
        EmulatorId::Eden => (EDEN_RELEASES, |n| n.starts_with("Eden-Windows-") && n.ends_with("-amd64-msvc-standard.zip")),
        _ => return None,
    })
}

/// Versions stables d'Eden (API Forgejo, mêmes champs que GitHub).
const EDEN_RELEASES: &str = "https://git.eden-emu.dev/api/v1/repos/eden-emu/eden/releases/latest";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorDownload {
    id: EmulatorId,
    name: &'static str,
    version: String,
    file: String,
    size: u64,
    page: String,
}

fn latest_release(repo: &str) -> Result<Release, String> {
    let url = if repo.starts_with("https://") { repo.to_string() } else { format!("https://api.github.com/repos/{repo}/releases/latest") };
    let response = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("impossible de joindre le site de l'émulateur : {e}"))?;
    serde_json::from_reader(response.into_reader()).map_err(|e| format!("réponse du site de l'émulateur illisible : {e}"))
}

fn find_download(id: EmulatorId) -> Result<(Release, usize), String> {
    let (repo, wanted) = source(id).ok_or_else(|| format!("{} ne peut pas être installé automatiquement", id.name()))?;
    let release = latest_release(repo)?;
    let index = release.assets.iter().position(|a| wanted(&a.name)).ok_or_else(|| format!("pas d'archive Windows dans la dernière version de {}", id.name()))?;
    Ok((release, index))
}

/// Version et taille de ce qui serait téléchargé.
#[tauri::command]
pub async fn emulator_download_info(id: EmulatorId) -> Result<EmulatorDownload, String> {
    crate::blocking(move || {
        let (release, i) = find_download(id)?;
        let asset = &release.assets[i];
        Ok(EmulatorDownload {
            id,
            name: id.name(),
            version: release.tag_name.trim_start_matches("release_").replace('_', ".").trim_start_matches(['v', 'V']).to_string(),
            file: asset.name.clone(),
            size: asset.size,
            page: release.html_url.clone(),
        })
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    id: EmulatorId,
    /// « download » puis « extract ».
    step: &'static str,
    done: u64,
    total: u64,
}

/// Chemin relatif sûr d'une entrée d'archive (`\` accepté, `..` et chemins absolus refusés).
pub fn safe_entry_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(p) if !p.to_string_lossy().contains(':') => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Télécharge `url` dans `dest` en signalant la progression (octets reçus, total attendu).
/// Le fichier n'apparaît sous son nom qu'une fois complet.
pub(crate) fn download_to(url: &str, dest: &Path, expected: u64, mut progress: impl FnMut(u64, u64)) -> Result<u64, String> {
    let response = ureq::get(url).set("User-Agent", USER_AGENT).call().map_err(|e| format!("téléchargement impossible : {e}"))?;
    let total = response.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(expected);
    let mut reader = response.into_reader();
    let part = dest.with_extension("part");
    let mut out = fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let (mut done, mut last) = (0u64, 0u64);
    progress(0, total);
    let result = loop {
        let n = match reader.read(&mut buf) {
            Ok(n) => n,
            Err(e) => break Err(format!("téléchargement interrompu : {e}")),
        };
        if n == 0 {
            break Ok(());
        }
        if let Err(e) = out.write_all(&buf[..n]) {
            break Err(e.to_string());
        }
        done += n as u64;
        if done - last >= 512 * 1024 {
            last = done;
            progress(done, total);
        }
    };
    drop(out);
    if let Err(e) = result {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    progress(done, total);
    fs::rename(&part, dest).map_err(|e| e.to_string())?;
    Ok(done)
}

pub(crate) fn extract_zip(archive: &Path, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("archive illisible : {e}"))?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("archive illisible : {e}"))?;
        let Some(rel) = safe_entry_path(entry.name()) else { continue };
        let target = dest.join(rel);
        if entry.is_dir() || entry.name().ends_with(['/', '\\']) {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&target).map_err(|e| format!("{} : {e}", target.display()))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
        progress(i as u64 + 1, total);
    }
    Ok(())
}

/// Télécharge la dernière version de l'émulateur, la décompresse et l'enregistre dans son profil.
#[tauri::command]
pub async fn emulator_install(id: EmulatorId, app: AppHandle) -> Result<EmulatorsState, String> {
    crate::blocking(move || {
        let (release, i) = find_download(id)?;
        let asset = &release.assets[i];
        let root = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("emulators");
        let dest = root.join(id.name());
        fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("emulator-install", InstallProgress { id, step, done, total });
        };

        // Téléchargement dans un fichier temporaire, avec progression.
        let archive = root.join(format!("{}.download", asset.name));
        download_to(&asset.browser_download_url, &archive, asset.size, |d, t| emit("download", d, t))?;

        let extracted = extract_zip(&archive, &dest, |d, t| emit("extract", d, t));
        let _ = fs::remove_file(&archive);
        extracted?;

        let exe = play::scan_for_exe(&dest, id, 3).ok_or_else(|| format!("{} introuvable dans l'archive téléchargée", id.name()))?;
        let mut config = play::load_config(&app);
        config.profiles.entry(id).or_default().exe = Some(exe);
        if !id.is_switch() {
            let preferred = if id.is_ctr() { &mut config.preferred_ctr } else { &mut config.preferred_nds };
            if preferred.is_none() {
                *preferred = Some(id);
            }
        }
        play::store_config(&app, &config)?;
        Ok(play::state_of(config))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_covers_first() {
        assert_eq!(
            cover_urls("platinum"),
            [
                "https://art.gametdb.com/ds/coverHQ/FR/CPUF.jpg",
                "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Nintendo_DS/master/Named_Boxarts/Pokemon%20-%20Version%20Platine%20%28France%29.png",
                "https://art.gametdb.com/ds/coverM/FR/CPUF.jpg",
            ]
        );
        let x = cover_urls("x");
        assert!(x[0].ends_with("/3ds/coverHQ/FR/EKJP.jpg"));
        assert!(x.last().unwrap().contains("Pokemon%20X%20%28Europe%29"));
        assert!(cover_urls("emerald").is_empty());
        assert_eq!(cover_urls("nx-01001f5010dfa000"), ["https://api.nlib.cc/nx/01001F5010DFA000/icon/512/512"]);
        assert!(switch_cover_id("nx-0100").is_none());
    }

    #[test]
    fn archive_paths() {
        assert_eq!(safe_entry_path("azahar-windows-msvc-1\\plugins\\a.dll"), Some(PathBuf::from("azahar-windows-msvc-1/plugins/a.dll")));
        assert_eq!(safe_entry_path("melonDS.exe"), Some(PathBuf::from("melonDS.exe")));
        assert_eq!(safe_entry_path("../evil.exe"), None);
        assert_eq!(safe_entry_path("C:\\Windows\\evil.exe"), None);
        assert_eq!(safe_entry_path("/abs"), None);
    }

    #[test]
    fn extract_then_find_exe() {
        let tmp = std::env::temp_dir().join(format!("kaleido-zip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let archive = tmp.join("emu.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            w.add_directory("azahar-windows-msvc-1\\plugins\\", opts).unwrap();
            w.start_file("azahar-windows-msvc-1\\azahar.exe", opts).unwrap();
            w.write_all(b"MZ").unwrap();
            w.start_file("../evil.txt", opts).unwrap();
            w.write_all(b"no").unwrap();
            w.finish().unwrap();
        }
        let dest = tmp.join("out");
        let mut steps = 0;
        extract_zip(&archive, &dest, |_, _| steps += 1).unwrap();
        assert_eq!(steps, 2);
        assert!(!tmp.join("evil.txt").exists());
        assert_eq!(play::scan_for_exe(&dest, EmulatorId::Azahar, 3), Some(dest.join("azahar-windows-msvc-1").join("azahar.exe")));
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn asset_choice() {
        let (_, melon) = source(EmulatorId::Melonds).unwrap();
        assert!(melon("melonDS-1.1-windows-x86_64.zip") && !melon("melonDS-1.1-windows-aarch64.zip"));
        let (_, az) = source(EmulatorId::Azahar).unwrap();
        assert!(az("azahar-windows-msvc-2126.1.2.zip"));
        assert!(!az("azahar-windows-msvc-2126.1.2-installer.exe") && !az("azahar-libretro-windows-x86_64-2126.1.2.zip"));
        let (_, des) = source(EmulatorId::Desmume).unwrap();
        assert!(des("desmume-0.9.13-win64.zip") && !des("desmume-0.9.9a-win64.7z"));
        assert!(source(EmulatorId::Citra).is_none());
    }
}
