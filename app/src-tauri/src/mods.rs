//! Mods en un clic pour les jeux de la bibliothèque.
//!
//! - **Switch (Eden)** : mods de Fl4sh9174/Switch-Emulator-Ultrawide-FPS-Mods (une
//!   archive par jeu, `Nom [TITLEID][mods].zip`, un dossier `[Mod vX.Y.Z]/exefs|romfs`
//!   par mod). Les « 60 FPS » de Fl4sh sont de vrais correctifs de cadence : le jeu
//!   tourne à vitesse normale (animations, physique, musique), contrairement à une
//!   accélération de l'émulateur. Pour Légendes Arceus, quelques mods GameBanana
//!   (textures HD, ciel, distance d'affichage), vérifiés par MD5.
//!   Installation : `<load>/<TITLEID>/<nom du mod>/` (dossier `load_directory` d'Eden),
//!   avec un `kaleido-mod.json`. Les autres mods déjà présents peuvent être mis de
//!   côté (déplacés dans `<données>/mods-disabled`) et remis en place.
//! - **3DS (Azahar)** : codes de triche (iSharingan/CTRPF-AR-CHEAT-CODES, convertis au
//!   format d'Azahar : `cheats/<TITLEID>.txt`) et packs de textures HD
//!   (Gray-Rice/PokeTex-3DS, `load/textures/<TITLEID>/`).
//! - **DS (melonDS)** : codes de triche au format `.mch` (Lyrx997/MelonDS-Desktop-Cheats),
//!   placés à côté de la ROM, là où melonDS les cherche.
//!
//! Les fichiers sont pris sur raw.githubusercontent.com et dans les « releases », sans
//! limite d'appels ; l'API GitHub (60 appels par heure) ne sert qu'à découvrir de
//! nouvelles archives Fl4sh, avec un cache d'un jour et une liste intégrée en secours.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{download_to, extract_zip, safe_entry_path, USER_AGENT};
use crate::play::{self, EmulatorId, Env, Resolved};
use crate::switch::base_title_id;
use crate::tuning::{self, Tier};

// ---------------------------------------------------------------------------
// Types échangés avec l'interface

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModTarget {
    /// « nds », « 3ds » ou « switch ».
    pub platform: String,
    /// Title ID (Switch, 3DS).
    pub title_id: Option<String>,
    /// ROM (DS) : les codes de triche se placent à côté.
    pub rom: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// « fps », « graphics », « resolution », « display », « textures », « cheats », « other ».
    pub category: &'static str,
    /// Mods incompatibles entre eux (un seul à la fois).
    pub group: Option<&'static str>,
    pub recommended: bool,
    pub author: String,
    pub page: String,
    pub size: Option<u64>,
    /// Version du jeu visée par le mod.
    pub game_version: Option<String>,
    pub installed: bool,
    /// Version installée différente de celle proposée.
    pub update_available: bool,
    /// Mods déjà installés qui entrent en conflit avec celui-ci.
    pub conflicts: Vec<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtherMod {
    pub name: String,
    pub enabled: bool,
    pub category: &'static str,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsView {
    pub emulator: Option<&'static str>,
    pub emulator_found: bool,
    /// Dossier où les mods sont installés.
    pub location: Option<String>,
    pub mods: Vec<ModEntry>,
    /// Mods présents qui ne viennent pas de Kaleido (Switch).
    pub others: Vec<OtherMod>,
    pub notes: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    id: String,
    version: Option<String>,
    source: String,
}

const MARKER: &str = "kaleido-mod.json";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: String,
    /// « download », « extract », « install ».
    step: &'static str,
    done: u64,
    total: u64,
}

// ---------------------------------------------------------------------------
// Utilitaires

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("mods");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn work_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("mods-tmp");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn get(url: &str, browser: bool) -> Result<Vec<u8>, String> {
    // GameBanana refuse les clients qui ne ressemblent pas à un navigateur.
    let ua = if browser { "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Kaleido" } else { USER_AGENT };
    let response = ureq::get(url).set("User-Agent", ua).timeout(Duration::from_secs(20)).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("le site a répondu {code}"),
        e => format!("connexion impossible ({e})"),
    })?;
    let mut body = Vec::new();
    response.into_reader().take(64 << 20).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(body)
}

/// Fichier téléchargé gardé en cache `max_age` (version périmée utilisée hors ligne).
fn cached(app: &AppHandle, name: &str, url: &str, max_age: Duration, browser: bool) -> Result<Vec<u8>, String> {
    let file = cache_dir(app)?.join(name);
    let fresh = fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < max_age);
    if fresh {
        if let Ok(data) = fs::read(&file) {
            return Ok(data);
        }
    }
    match get(url, browser) {
        Ok(data) => {
            let _ = fs::write(&file, &data);
            Ok(data)
        }
        Err(e) => fs::read(&file).map_err(|_| e),
    }
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Nom de dossier valide sous Windows.
fn folder_name(name: &str) -> String {
    name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) { '-' } else { c }).collect::<String>().trim_end_matches(['.', ' ']).to_string()
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// Déplace un dossier (copie puis suppression si le volume diffère).
fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy_dir(src, dst).map_err(|e| format!("copie impossible vers {} : {e}", dst.display()))?;
    fs::remove_dir_all(src).map_err(|e| e.to_string())
}

/// Fusionne le contenu de `src` dans `dst` (les fichiers existants sont remplacés).
fn merge_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            merge_dir(&entry.path(), &to)?;
        } else {
            if to.exists() {
                let _ = fs::remove_file(&to);
            }
            if fs::rename(entry.path(), &to).is_err() {
                fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Décompresse une archive .zip ou .7z (chemins dangereux ignorés).
pub(crate) fn extract_any(archive: &Path, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    let mut magic = [0u8; 6];
    fs::File::open(archive).and_then(|mut f| f.read_exact(&mut magic)).map_err(|e| e.to_string())?;
    if magic.starts_with(b"PK") {
        return extract_zip(archive, dest, progress);
    }
    if magic == [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C] {
        let total = fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
        let mut done = 0u64;
        return sevenz_rust2::decompress_file_with_extract_fn(archive, dest, |entry, reader, _| {
            let Some(rel) = safe_entry_path(entry.name()) else { return Ok(true) };
            let target = dest.join(rel);
            if entry.is_directory() {
                fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = std::io::BufWriter::new(fs::File::create(&target)?);
                std::io::copy(reader, &mut out)?;
                done += entry.compressed_size;
                progress(done.min(total), total);
            }
            Ok(true)
        })
        .map_err(|e| format!("archive 7z illisible : {e}"));
    }
    Err("format d'archive non pris en charge (seuls .zip et .7z le sont)".into())
}

fn md5_hex(path: &Path) -> Result<String, String> {
    use md5::{Digest, Md5};
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn parse_tid(s: Option<&str>) -> Result<u64, String> {
    s.and_then(|t| u64::from_str_radix(t, 16).ok()).ok_or_else(|| "title ID du jeu inconnu".to_string())
}

fn resolve(id: EmulatorId, app: &AppHandle) -> Resolved {
    let config = play::load_config(app);
    play::resolve(id, &config, &Env::system(&config.search_dirs))
}

/// Émulateur 3DS à utiliser : celui choisi, sinon le premier trouvé, sinon Azahar.
fn ctr_emulator(app: &AppHandle) -> Resolved {
    let config = play::load_config(app);
    let env = Env::system(&config.search_dirs);
    let order = config.preferred_ctr.into_iter().chain([EmulatorId::Azahar, EmulatorId::Lime3ds, EmulatorId::Citra]);
    let all: Vec<Resolved> = order.map(|id| play::resolve(id, &config, &env)).collect();
    all.iter().find(|r| r.exe.is_some()).cloned().unwrap_or_else(|| all[0].clone())
}

// ---------------------------------------------------------------------------
// Switch : classement des mods Fl4sh

struct Kind {
    category: &'static str,
    group: Option<&'static str>,
    name: String,
    description: String,
    warning: Option<String>,
    hidden: bool,
    /// Rang de recommandation (0 : non recommandé).
    rank: u8,
}

/// « [60 FPS v1.3.0] » → (« 60 FPS », Some(« 1.3.0 »)).
pub fn split_version(raw: &str) -> (String, Option<String>) {
    let cleaned = raw.replace(['[', ']'], " ");
    let mut version = None;
    let mut words = Vec::new();
    for w in cleaned.split_whitespace() {
        let v = w.strip_prefix(['v', 'V']).filter(|r| r.chars().next().is_some_and(|c| c.is_ascii_digit()) && r.contains('.'));
        match v {
            Some(v) if version.is_none() => version = Some(v.to_string()),
            _ => words.push(w),
        }
    }
    (words.join(" "), version)
}

/// Clé stable d'un mod (sans la version), pour le reconnaître après une mise à jour.
fn mod_key(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().split_whitespace().collect::<Vec<_>>().join("-")
}

fn resolution_in(lower: &str) -> Option<(u32, u32)> {
    let bytes: Vec<char> = lower.chars().collect();
    let s: String = bytes.iter().collect();
    for (i, _) in s.match_indices('x') {
        let w: String = s[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        let h: String = s[i + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
        if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
            if w >= 640 && h >= 360 {
                return Some((w, h));
            }
        }
    }
    None
}

fn classify(raw: &str, tier: Tier) -> Kind {
    let (name, _) = split_version(raw);
    let lower = name.to_lowercase();
    let mut k = Kind { category: "other", group: None, name: name.clone(), description: "Mod de l'auteur (Fl4sh).".into(), warning: None, hidden: false, rank: 0 };
    if lower.contains("ryujinx") {
        k.hidden = true;
        return k;
    }
    let strong = tier >= Tier::High;
    if lower.contains("fps") {
        k.category = "fps";
        k.group = Some("fps");
        if lower.contains("120") {
            k.name = "120 FPS (expérimental)".into();
            k.description = "Pour les écrans 120 Hz et les PC très puissants. Le jeu reste à vitesse normale.".into();
            k.warning = Some("Encore en test selon l'auteur : certains menus peuvent aller deux fois trop vite.".into());
        } else if lower.contains("dynamic") {
            k.name = "60 FPS adaptatif".into();
            if lower.contains("exlaunch") {
                k.name.push_str(" (Exlaunch)");
            } else if lower.contains("asm") {
                k.name.push_str(" (ASM)");
            }
            k.description = "Vise 60 images par seconde et s'adapte quand le PC ralentit : le jeu garde toujours sa vitesse normale (déplacements, animations, musique). Le choix sûr pour les PC moyens.".into();
            k.rank = if strong { 1 } else { 3 };
        } else {
            k.name = "60 FPS".into();
            k.description = "Le jeu s'affiche en 60 images par seconde au lieu de 30, à vitesse normale : animations, déplacements et musique gardent leur rythme. Le PC doit tenir 60 FPS en permanence, sinon le jeu ralentit.".into();
            k.rank = if strong { 3 } else { 1 };
        }
        return k;
    }
    if lower.contains("ultrawide") || lower.contains("21.9") || lower.contains("21:9") || lower.contains("32.9") {
        k.category = "display";
        k.group = Some("aspect");
        k.name = match resolution_in(&lower) {
            Some((w, h)) => format!("Écran ultra-large ({w}×{h})"),
            None => "Écran ultra-large 21/9".into(),
        };
        k.description = "Uniquement pour les écrans ultra-larges : l'image remplit l'écran au lieu d'avoir des bandes noires.".into();
        return k;
    }
    if lower.contains("shadow") {
        k.category = "graphics";
        k.group = Some("shadows");
        let four_k = lower.contains("4k");
        k.name = if four_k { "Ombres 4K".into() } else { "Ombres 2K".into() };
        k.description = "Ombres plus nettes et plus détaillées.".into();
        k.rank = if four_k == strong { 2 } else { 0 };
        return k;
    }
    if let Some((w, h)) = resolution_in(&lower).or_else(|| lower.contains("4k").then_some((3840, 2160))) {
        k.category = "resolution";
        k.group = Some("resolution");
        k.name = format!("Résolution du jeu {w}×{h}");
        k.description = "Change la résolution calculée par le jeu lui-même. Inutile avec les réglages optimaux (Eden agrandit déjà l'image) et parfois source de défauts d'éclairage.".into();
        return k;
    }
    k.category = "graphics";
    let (n, d, rank): (&str, &str, u8) = if lower.contains("level of detail") || lower.split_whitespace().any(|w| w == "lod") {
        if lower.contains("qol") {
            k.warning = Some("Contient aussi des changements de gameplay voulus par l'auteur (endurance, combats).".into());
            ("Niveau de détail (LOD) + confort", "Les objets lointains gardent leur version détaillée plus longtemps, avec des retouches de confort de l'auteur.", 0)
        } else {
            ("Niveau de détail (LOD)", "Les objets lointains gardent leur version détaillée plus longtemps : moins d'éléments qui apparaissent d'un coup.", 2)
        }
    } else if lower.contains("depth of field") || lower.contains("dof") {
        ("Sans flou de profondeur", "Retire le flou appliqué aux arrière-plans.", 0)
    } else if lower.contains("fxaa") {
        ("Sans FXAA", "Retire l'anticrénelage flou du jeu (les réglages optimaux utilisent déjà le SMAA d'Eden).", 0)
    } else if lower.contains("bloom") {
        ("Halo lumineux réduit", "Atténue l'effet de halo autour des sources de lumière.", 0)
    } else if lower.contains("fov") {
        ("Champ de vision élargi", "La caméra montre une plus grande partie du décor.", 0)
    } else if lower.contains("graphic") || lower.contains("quality") {
        ("Qualité graphique améliorée", "Augmente plusieurs réglages internes de qualité du jeu.", 2)
    } else if lower.contains("max res") || lower.contains("dynamic res") {
        ("Résolution maximale permanente", "Désactive la baisse automatique de résolution du jeu : image toujours nette.", 2)
    } else if lower.contains("outline") {
        ("Sans contours noirs", "Retire le contour noir autour des personnages et des Pokémon.", 0)
    } else if lower.contains("wifi") || lower.contains("wi-fi") {
        k.category = "other";
        ("Correctif Wi-Fi", "Corrige un blocage lié aux fonctions en ligne sur émulateur.", 0)
    } else {
        k.category = "other";
        return k;
    };
    k.name = n.into();
    k.description = d.into();
    k.rank = rank;
    k
}

// ---------------------------------------------------------------------------
// Switch : sources

const FL4SH_REPO: &str = "Fl4sh9174/Switch-Emulator-Ultrawide-FPS-Mods";

/// Archives Fl4sh connues (liste du dépôt au 6 octobre 2026), si l'API GitHub ne répond pas.
const FL4SH_KNOWN: &[&str] = &[
    "13 Sentinels Aegis Rim [01003FC01670C000][USA][mods].zip",
    "Animal Crossing New Horizons [01006F8002326000][mods].zip",
    "Another Crab's Treasure [0100A21017C42000][mods].zip",
    "Atelier Yumia The Alchemist of Memories & the Envisioned Land [0100544020572000][mods].zip",
    "Bayonetta Origins Cereza and the Lost Demon [0100CF5010FEC000][mods].zip",
    "Bomb Rush Cyberfunk [0100317014B7C000][mods].zip",
    "Burnout Paradise Remastered [0100DBF01000A000][mods].zip",
    "Captain Toad Treasure Tracker [01009BF0072D4000][mods].zip",
    "Crash Team Racing Nitro-Fueled [0100F9F00C696000][mods].zip",
    "Cruis'n Blast [0100B41013C82000][mods].zip",
    "DRAGON QUEST MONSTERS The Dark Prince [0100A77018EA0000][mods].zip",
    "DYNASTY WARRIORS 8 Xtreme Legends Definitive Edition [0100E9A00CB30000][mods].zip",
    "Demon Slayer -Kimetsu no Yaiba- The Hinokami Chronicles [0100309016E7A000][mods].zip",
    "Diablo III Eternal Collection [01001B300B9BE000][mods].zip",
    "Disney Epic Mickey Rebrushed [0100DA201EBF8000][mods].zip",
    "Donkey Kong Country Returns HD [01009D901BC56000][mods].zip",
    "Donkey Kong Country Tropical Freeze [0100C1F0051B6000][mods].zip",
    "Dragon Ball Z Kakarot and A New Power Awakens Set [010051C0134F8000].zip",
    "Eiyuden Chronicle Hundred Heroes [0100ED9018F3E000][mods].zip",
    "Endless Ocean Luminous [010067B017588000][mods].zip",
    "FANTASIAN Neo Dimension [01001BB01E8E2000][mods].zip",
    "FANTASY LIFE i The Girl Who Steals Time [0100755017EE0000][mods].zip",
    "FIFA 23 Legacy Edition [01001C8016B4E000][mods].zip",
    "Fire Emblem Engage [0100A6301214E000][mods].zip",
    "Fire Emblem Three Houses [010055D009F78800][mods].zip",
    "Fire Emblem Warriors [0100F15003E64000][mods].zip",
    "GRIME Definitive Edition [0100F300169B6000][mods].zip",
    "Ghostbusters The Video Game Remastered [0100EAE00D9EC000] [EU][mods].zip",
    "Hollow Knight Silksong [010013C00E930000][mods].zip",
    "Hyrule Warriors Age of Calamity [01002B00111A2000][mods].zip",
    "Hyrule Warriors Definitive Edition [0100AE00096EA000][mods].zip",
    "INAZUMA ELEVEN Victory Road [0100B36008F90000][mods].zip",
    "Jujutsu Kaisen Cursed Clash [010085401A454000][EUR][mods].zip",
    "Kirby and the Forgotten Land [01004D300C5AE000][mods].zip",
    "Kirby’s Return to Dream Land Deluxe [01006B601380E000][mods].zip",
    "LOLLIPOP CHAINSAW RePOP [0100DD301A686000][mods].zip",
    "Luigis Mansion 2 HD [010048701995E000][mods].zip",
    "Luigis Mansion 3 [0100DCA0064A6800][mods].zip",
    "MEGATON MUSASHI W WIRED [01003EB01C2F0000][mods].zip",
    "MONSTER HUNTER GENERATIONS ULTIMATE [0100770008DD8000][US][mods].zip",
    "Mario + Rabbids Kingdom Battle [010067300059A000][mods].zip",
    "Mario + Rabbids Sparks of Hope [0100317013770000][mods].zip",
    "Mario Kart 8 Deluxe [0100152000022000][mods].zip",
    "Mario Party Superstars [01006FE013472000][mods].zip",
    "Mario Strikers Battle League [010019401051C000][mods].zip",
    "Mario Tennis Aces [0100BDE00862A000][mods].zip",
    "Mario and Luigi Brothership [01006D0017F7A000][mods].zip",
    "Mario vs. Donkey Kong [0100B99019412000][mods].zip",
    "Marvel Ultimate Alliance 3 [010060700AC50000][mods].zip",
    "Metroid Dread [010093801237C000][mods].zip",
    "Metroid Prime 4 Beyond [010019A01E2F2000][mods].zip",
    "Metroid Prime Remastered [010012101468C000][mods].zip",
    "NARUTO X BORUTO Ultimate Ninja STORM CONNECTIONS [0100D2D0190A4000][mods].zip",
    "NINJA GAIDEN 2 [0100696014F4A000][mods].zip",
    "NINJA GAIDEN 3 Razor's Edge [01002AF014F4C000][mods].zip",
    "Need For Speed Hot Pursuit Remastered [010029B0118E8000][mods].zip",
    "Nikoderiko the Magical World [01009FA01FF6C000][mods].zip",
    "Ori and the Blind Forest Definitive Edition [010061D00DB74000][mods].zip",
    "Paper Mario The Origami King [0100A3900C3E2000][mods].zip",
    "Paper Mario The Thousand-Year Door [0100ECD018EBE000][mods].zip",
    "Pikmin 4 [0100B7C00933A000][mods].zip",
    "Pokemon Brilliant Diamond [0100000011D90000][mods].zip",
    "Pokemon Legends Arceus [01001F5010DFA000][mods].zip",
    "Pokemon Let’s Go Eevee [0100187003A36000][mods].zip",
    "Pokemon Mystery Dungeon Rescue Team DX [01003D200BAA2000][mods].zip",
    "Pokemon Scarlet [0100A3D008C5C000][mods].zip",
    "Pokemon Shield [01008DB008C2C000][mods].zip",
    "Pokemon Shining Pearl [010018E011D92000][mods].zip",
    "Pokemon Sword [0100ABF008968000][mods].zip",
    "Pokemon Violet [01008F6008C5E000][mods].zip",
    "Pokémon Legends Z-A [0100F43008C44000][mods].zip",
    "Pokémon Let's Go, Pikachu! [010003F003A34000][mods].zip",
    "Prince of Persia The Lost Crown [0100210019428000][mods].zip",
    "Princess Peach Showtime! [01007A3009184000][mods].zip",
    "SONIC X SHADOW GENERATIONS [01005EA01C0FC000][mods].zip",
    "SPY×FAMILY OPERATION DIARY [010041601AB40000][mods].zip",
    "Sea of Stars [01008C0016544000][mods].zip",
    "Sonic Colors Ultimate [010040E0116B8000][mods].zip",
    "Sonic Frontiers [01004AD014BF0000][mods].zip",
    "Sonic Racing CrossWorlds [01006E001823C000][mods].zip",
    "Sonic Superstars [01008F701C074000][mods].zip",
    "South Park Snow Day [0100D1501ABAE000][mods].zip",
    "Splatoon 3 [0100C2500FC20000][mods].zip",
    "Super Mario 3D All-Stars (Galaxy)[010049900F546003][mods].zip",
    "Super Mario 3D World + Bowsers Fury [010028600EBDA000][mods].zip",
    "Super Mario Bros. Wonder [010015100B514000][mods].zip",
    "Super Mario Odyssey [0100000000010000][mods].zip",
    "Super Mario Party Jamboree [0100965017338000][mods].zip",
    "Super Mario Party [010036B0034E4000][mods].zip",
    "Super Mario RPG [0100BC0018138000][mods].zip",
    "Super Smash Bros Ultimate [01006A800016E000][mods].zip",
    "The Legend of Legacy HD Remastered [010099F01C258000][mods].zip",
    "The Legend of Zelda Breath of the Wild [01007EF00011E000][mods].zip",
    "The Legend of Zelda Echoes of Wisdom [01008CF01BAAC800][mods].zip",
    "The Legend of Zelda Link’s Awakening [01006BB00C6F0000][mods].zip",
    "The Legend of Zelda Skyward Sword HD [01002DA013484000][mods].zip",
    "The Legend of Zelda Tears of the Kingdom [0100F2C0115B6000][mods].zip",
    "The Witcher 3 Wild Hunt [0100E67012924000][mods].zip",
    "Tomodachi Life Living the Dream [010051F0207B2000][mods].zip",
    "Unicorn Overlord [010069401ADB8000][US][JP][mods].zip",
    "WARRIORS OROCHI 4 [010016A00AEC0000][mods].zip",
    "Xenoblade Chronicles 2 [0100E95004038000][mods].zip",
    "Xenoblade Chronicles 3 [010074F013262000][mods].zip",
    "Xenoblade Chronicles Definitive Edition [0100FF500E34A000][USA][mods].zip",
    "Xenoblade Chronicles X Definitive Edition [0100453019AA8000][mods].zip",
    "Yo-kai Watch 4 PuraPura [010086C00AF7C000][mods].zip",
    "Yoshi's Crafted World [01006000040C2000][mods].zip",
    "Ys VIII Lacrimosa of DANA [01007F200B0C0000][mods].zip",
];

#[derive(Deserialize)]
struct GhContent {
    name: String,
    #[serde(default)]
    sha: String,
}

/// Archive Fl4sh d'un jeu : (nom, empreinte git si connue).
fn fl4sh_archive(app: &AppHandle, tid: u64) -> Option<(String, String)> {
    let matches = |name: &str| crate::switch::title_id_in_name(name).is_some_and(|t| base_title_id(t) == tid);
    let url = format!("https://api.github.com/repos/{FL4SH_REPO}/contents/?ref=main");
    if let Some(list) = cached(app, "fl4sh-index.json", &url, Duration::from_secs(24 * 3600), false).ok().and_then(|d| serde_json::from_slice::<Vec<GhContent>>(&d).ok()) {
        if let Some(e) = list.into_iter().find(|e| e.name.ends_with(".zip") && matches(&e.name)) {
            return Some((e.name, e.sha));
        }
    }
    FL4SH_KNOWN.iter().find(|n| matches(n)).map(|n| (n.to_string(), String::new()))
}

/// Archive Fl4sh en cache (re-téléchargée au plus une fois par jour si l'empreinte est inconnue).
fn fl4sh_zip(app: &AppHandle, name: &str, sha: &str) -> Result<PathBuf, String> {
    let dir = cache_dir(app)?.join("fl4sh");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = if sha.is_empty() { format!("{:016x}", fxhash(name)) } else { sha.to_string() };
    let file = dir.join(format!("{key}.zip"));
    let max_age = if sha.is_empty() { Duration::from_secs(24 * 3600) } else { Duration::MAX };
    let fresh = fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|a| a < max_age);
    if fresh {
        return Ok(file);
    }
    let url = format!("https://raw.githubusercontent.com/{FL4SH_REPO}/main/{}", encode(name));
    match download_to(&url, &file, 0, |_, _| {}) {
        Ok(_) => Ok(file),
        Err(e) if file.is_file() => {
            let _ = e;
            Ok(file)
        }
        Err(e) => Err(e),
    }
}

fn fxhash(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Mod contenu dans une archive Fl4sh.
#[derive(Debug, Clone, PartialEq)]
pub struct ZipMod {
    /// Dossier de premier niveau.
    pub folder: String,
    /// Préfixe des fichiers à installer (dossier qui contient exefs/romfs).
    pub root: String,
    pub size: u64,
}

/// Mods d'une archive : dossiers de premier niveau qui contiennent `exefs`, `romfs` ou
/// `cheats` (directement, ou dans un sous-dossier au title ID du jeu).
pub fn zip_mods(entries: &[(String, u64)], tid: u64) -> Vec<ZipMod> {
    let tid_hex = format!("{tid:016x}");
    let mut out: Vec<ZipMod> = Vec::new();
    let norm: Vec<(String, u64)> = entries.iter().map(|(n, s)| (n.replace('\\', "/"), *s)).collect();
    let mut folders: Vec<&str> = norm.iter().filter_map(|(n, _)| n.split_once('/').map(|(f, _)| f)).collect();
    folders.sort();
    folders.dedup();
    let is_content = |rest: &str| {
        let first = rest.split('/').next().unwrap_or("").to_ascii_lowercase();
        matches!(first.as_str(), "exefs" | "romfs" | "cheats")
    };
    for folder in folders {
        let inside: Vec<(&str, u64)> = norm.iter().filter_map(|(n, s)| n.strip_prefix(folder).and_then(|r| r.strip_prefix('/')).map(|r| (r, *s))).collect();
        let by_tid: Vec<(&str, u64)> = inside
            .iter()
            .filter_map(|(r, s)| r.split_once('/').filter(|(d, _)| d.eq_ignore_ascii_case(&tid_hex)).map(|(_, rest)| (rest, *s)))
            .collect();
        let (root, files) = if by_tid.iter().any(|(r, _)| is_content(r)) {
            let sub = inside.iter().find_map(|(r, _)| r.split_once('/').filter(|(d, _)| d.eq_ignore_ascii_case(&tid_hex)).map(|(d, _)| d)).unwrap_or(&tid_hex);
            (format!("{folder}/{sub}/"), by_tid)
        } else if inside.iter().any(|(r, _)| is_content(r)) {
            (format!("{folder}/"), inside.clone())
        } else {
            continue;
        };
        let size = files.iter().filter(|(r, _)| is_content(r)).map(|(_, s)| s).sum();
        out.push(ZipMod { folder: folder.to_string(), root, size });
    }
    out
}

fn zip_listing(path: &Path) -> Result<Vec<(String, u64)>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("archive illisible : {e}"))?;
    (0..zip.len()).map(|i| zip.by_index(i).map(|e| (e.name().to_string(), e.size())).map_err(|e| e.to_string())).collect()
}

/// Mods GameBanana choisis à la main (Légendes Arceus).
struct GbMod {
    tid: u64,
    id: u32,
    /// Début du nom du fichier à prendre (le plus récent l'emporte).
    prefix: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    /// Mots qui reconnaissent ce mod s'il a été installé à la main.
    aliases: &'static [&'static str],
    author: &'static str,
}

const GB_MODS: &[GbMod] = &[
    GbMod {
        tid: 0x01001F5010DFA000,
        id: 355407,
        prefix: "hd_texture_overhaul",
        name: "Textures HD",
        description: "Remplace de nombreuses textures du jeu (sols, falaises, végétation) par des versions haute définition. Pack de la communauté encore incomplet : certaines textures d'origine restent visibles.",
        category: "textures",
        aliases: &["hd texture"],
        author: "GameBanana (HD Texture Overhaul)",
    },
    GbMod {
        tid: 0x01001F5010DFA000,
        id: 353940,
        prefix: "sky_moon_improvement_mod_0",
        name: "Ciel et lune améliorés",
        description: "Ciel, nuages, étoiles et lune plus détaillés.",
        category: "graphics",
        aliases: &["sky"],
        author: "GameBanana (Sky & Moon Improvement)",
    },
    GbMod {
        tid: 0x01001F5010DFA000,
        id: 353719,
        prefix: "lod_wip",
        name: "Arbres et personnages visibles de plus loin",
        description: "Augmente la distance d'affichage des arbres, des personnages et des objets : le paysage lointain est beaucoup moins vide.",
        category: "graphics",
        aliases: &["draw distance"],
        author: "GameBanana (Trees/NPC draw distance)",
    },
];

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct GbFile {
    _sFile: String,
    _nFilesize: u64,
    _tsDateAdded: i64,
    _sDownloadUrl: String,
    #[serde(default)]
    _sMd5Checksum: String,
}

fn gb_file(app: &AppHandle, m: &GbMod) -> Result<GbFile, String> {
    let url = format!("https://gamebanana.com/apiv11/Mod/{}/Files", m.id);
    let data = cached(app, &format!("gb-{}.json", m.id), &url, Duration::from_secs(12 * 3600), true)?;
    let files: Vec<GbFile> = serde_json::from_slice(&data).map_err(|e| format!("réponse de GameBanana illisible : {e}"))?;
    files.into_iter().filter(|f| f._sFile.to_lowercase().starts_with(m.prefix)).max_by_key(|f| f._tsDateAdded).ok_or_else(|| "fichier introuvable sur GameBanana".into())
}

/// Dossier `load` d'Eden (réglage `load_directory`, sinon `<utilisateur>/load`).
fn eden_load_dir(r: &Resolved) -> Option<PathBuf> {
    let user = r.ctr_user_dir()?;
    let configured = fs::read_to_string(user.join("config").join("qt-config.ini"))
        .ok()
        .and_then(|t| tuning::get_qt(&t, "Data%20Storage", "load_directory"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    Some(configured.unwrap_or_else(|| user.join("load")))
}

fn disabled_dir(app: &AppHandle, tid: u64) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("mods-disabled").join(format!("{tid:016X}")))
}

/// Dossiers de mods d'un jeu : (nom, marqueur Kaleido éventuel).
fn installed_dirs(dir: &Path) -> Vec<(String, Option<Marker>)> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(String, Option<Marker>)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let marker = fs::read(e.path().join(MARKER)).ok().and_then(|d| serde_json::from_slice(&d).ok());
            (e.file_name().to_string_lossy().into_owned(), marker)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Catégorie d'un mod installé à la main (les mods GameBanana connus sont reconnus à leur nom).
fn other_category(name: &str, tier: Tier) -> &'static str {
    let lower = name.to_lowercase();
    GB_MODS.iter().find(|m| m.aliases.iter().any(|a| lower.contains(a))).map_or_else(|| classify(name, tier).category, |m| m.category)
}

fn switch_view(app: &AppHandle, tid: u64) -> ModsView {
    let r = resolve(EmulatorId::Eden, app);
    let mut view = ModsView { emulator: Some("Eden"), emulator_found: r.exe.is_some(), ..Default::default() };
    let Some(load) = eden_load_dir(&r) else {
        view.errors.push("Dossier d'Eden introuvable.".into());
        return view;
    };
    let game_dir = load.join(format!("{tid:016X}"));
    view.location = Some(game_dir.display().to_string());
    let tier = tuning::current_tier();
    let installed = installed_dirs(&game_dir);
    let disabled = disabled_dir(app, tid).map(|d| installed_dirs(&d)).unwrap_or_default();

    // Mods présents qui ne viennent pas de Kaleido (ou mis de côté par Kaleido).
    for (name, marker) in &installed {
        if marker.is_none() {
            view.others.push(OtherMod { name: name.clone(), enabled: true, category: other_category(name, tier) });
        }
    }
    for (name, _) in &disabled {
        view.others.push(OtherMod { name: name.clone(), enabled: false, category: other_category(name, tier) });
    }
    let active_others: Vec<(String, Kind)> = installed.iter().filter(|(_, m)| m.is_none()).map(|(n, _)| (n.clone(), classify(n, tier))).collect();
    let mine = |id: &str| installed.iter().find_map(|(n, m)| m.as_ref().filter(|m| m.id == id).map(|m| (n.clone(), m.version.clone())));

    // Fl4sh.
    let mut entries: Vec<ModEntry> = Vec::new();
    match fl4sh_archive(app, tid) {
        Some((archive, sha)) => match fl4sh_zip(app, &archive, &sha).and_then(|z| zip_listing(&z)) {
            Ok(listing) => {
                let mods = zip_mods(&listing, tid);
                let kinds: Vec<Kind> = mods.iter().map(|m| classify(&m.folder, tier)).collect();
                // Dans chaque groupe, seul le mieux classé est recommandé.
                let mut best: BTreeMap<&str, u8> = BTreeMap::new();
                for k in kinds.iter().filter(|k| !k.hidden) {
                    let g = k.group.unwrap_or("");
                    let b = best.entry(g).or_default();
                    *b = (*b).max(k.rank);
                }
                for (m, k) in mods.iter().zip(kinds) {
                    if k.hidden {
                        continue;
                    }
                    let (_, version) = split_version(&m.folder);
                    let id = format!("fl4sh:{}", mod_key(&split_version(&m.folder).0));
                    let current = mine(&id);
                    let recommended = k.rank > 0 && (k.group.is_none() || best.get(k.group.unwrap_or("")) == Some(&k.rank));
                    entries.push(ModEntry {
                        installed: current.is_some(),
                        update_available: current.as_ref().is_some_and(|(_, v)| *v != version),
                        conflicts: vec![],
                        id,
                        name: k.name,
                        description: k.description,
                        category: k.category,
                        group: k.group,
                        recommended,
                        author: "Fl4sh9174".into(),
                        page: format!("https://github.com/{FL4SH_REPO}"),
                        size: Some(m.size),
                        game_version: version,
                        warning: k.warning,
                    });
                }
            }
            Err(e) => view.errors.push(format!("Mods de Fl4sh indisponibles : {e}")),
        },
        None => view.notes.push("Fl4sh ne propose pas encore de mods pour ce jeu.".into()),
    }

    // GameBanana.
    for m in GB_MODS.iter().filter(|m| m.tid == tid) {
        let id = format!("gb:{}", m.id);
        let current = mine(&id);
        let manual = active_others.iter().any(|(n, _)| m.aliases.iter().any(|a| n.to_lowercase().contains(a)));
        let file = gb_file(app, m);
        if let Err(e) = &file {
            view.errors.push(format!("{} : {e}", m.name));
        }
        let file = file.ok();
        let version = file.as_ref().map(|f| f._sFile.clone());
        entries.push(ModEntry {
            installed: current.is_some() || manual,
            update_available: current.as_ref().is_some_and(|(_, v)| v.is_some() && *v != version),
            conflicts: vec![],
            id,
            name: m.name.into(),
            description: m.description.into(),
            category: m.category,
            group: None,
            recommended: m.category != "textures" || tier >= Tier::High,
            author: m.author.into(),
            page: format!("https://gamebanana.com/mods/{}", m.id),
            size: file.as_ref().map(|f| f._nFilesize),
            game_version: None,
            warning: manual.then(|| "Déjà installé à la main (dans la liste « Autres mods » plus bas).".to_string()),
        });
    }

    // Conflits : même groupe, déjà en place (Kaleido ou installé à la main).
    let installed_groups: Vec<(String, &'static str)> = entries
        .iter()
        .filter(|e| e.installed && e.group.is_some())
        .map(|e| (e.name.clone(), e.group.unwrap()))
        .chain(active_others.iter().filter_map(|(n, k)| k.group.map(|g| (n.clone(), g))))
        .collect();
    for e in entries.iter_mut().filter(|e| !e.installed) {
        if let Some(g) = e.group {
            e.conflicts = installed_groups.iter().filter(|(_, ig)| *ig == g).map(|(n, _)| n.clone()).collect();
        }
    }
    if entries.iter().any(|e| e.category == "fps") {
        view.notes.push("Les mods 60 FPS ne s'appliquent qu'à la version du jeu indiquée : installe la dernière mise à jour du jeu dans Eden.".into());
    }
    view.mods = entries;
    view
}

fn write_marker(dir: &Path, id: &str, version: Option<String>, source: &str) -> Result<(), String> {
    let marker = Marker { id: id.to_string(), version, source: source.to_string() };
    fs::write(dir.join(MARKER), serde_json::to_vec_pretty(&marker).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Plus haut dossier qui contient `exefs` / `romfs` dans une archive décompressée.
fn find_mod_root(dir: &Path, depth: u8) -> Option<PathBuf> {
    let entries: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    if entries.iter().any(|p| p.file_name().is_some_and(|n| matches!(n.to_string_lossy().to_ascii_lowercase().as_str(), "romfs" | "exefs"))) {
        return Some(dir.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    entries.iter().find_map(|p| find_mod_root(p, depth - 1))
}

fn install_switch(app: &AppHandle, tid: u64, id: &str, disable_conflicts: bool, emit: &dyn Fn(&'static str, u64, u64)) -> Result<(), String> {
    let r = resolve(EmulatorId::Eden, app);
    let load = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?;
    let game_dir = load.join(format!("{tid:016X}"));
    let view = switch_view(app, tid);
    let entry = view.mods.iter().find(|m| m.id == id).ok_or("mod inconnu")?;

    // Conflits : les mods Kaleido sont retirés, les autres mis de côté.
    if !entry.conflicts.is_empty() {
        if !disable_conflicts {
            return Err(format!("Conflit avec : {}", entry.conflicts.join(", ")));
        }
        for (name, marker) in installed_dirs(&game_dir) {
            let tier = tuning::current_tier();
            let same_group = match &marker {
                Some(m) => view.mods.iter().any(|e| e.id == m.id && e.group.is_some() && e.group == entry.group),
                None => classify(&name, tier).group.is_some() && classify(&name, tier).group == entry.group,
            };
            if !same_group {
                continue;
            }
            if marker.is_some() {
                fs::remove_dir_all(game_dir.join(&name)).map_err(|e| e.to_string())?;
            } else {
                move_dir(&game_dir.join(&name), &disabled_dir(app, tid)?.join(&name))?;
            }
        }
    }

    let target = game_dir.join(folder_name(&entry.name));
    let work = work_dir(app)?.join(format!("{tid:016X}-{}", fxhash(id)));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(PathBuf, Option<String>, &'static str), String> {
        if let Some(key) = id.strip_prefix("fl4sh:") {
            let (archive, sha) = fl4sh_archive(app, tid).ok_or("archive de Fl4sh introuvable")?;
            let zip_path = fl4sh_zip(app, &archive, &sha)?;
            let listing = zip_listing(&zip_path)?;
            let m = zip_mods(&listing, tid).into_iter().find(|m| mod_key(&split_version(&m.folder).0) == key).ok_or("mod introuvable dans l'archive")?;
            emit("extract", 0, 1);
            extract_zip(&zip_path, &work, |d, t| emit("extract", d, t))?;
            let root = work.join(m.root.trim_end_matches('/'));
            Ok((root, split_version(&m.folder).1, "Fl4sh9174/Switch-Emulator-Ultrawide-FPS-Mods"))
        } else if let Some(gb) = id.strip_prefix("gb:").and_then(|n| n.parse::<u32>().ok()) {
            let m = GB_MODS.iter().find(|m| m.id == gb).ok_or("mod inconnu")?;
            let file = gb_file(app, m)?;
            let archive = work.join(&file._sFile);
            download_to(&file._sDownloadUrl, &archive, file._nFilesize, |d, t| emit("download", d, t))?;
            if !file._sMd5Checksum.is_empty() {
                emit("verify", 0, 1);
                let sum = md5_hex(&archive)?;
                if !sum.eq_ignore_ascii_case(&file._sMd5Checksum) {
                    return Err("le fichier téléchargé est abîmé (somme MD5 différente de celle de GameBanana) : réessaie".into());
                }
            }
            let out = work.join("x");
            extract_any(&archive, &out, |d, t| emit("extract", d, t))?;
            let root = find_mod_root(&out, 3).ok_or("l'archive ne contient pas de dossier romfs ou exefs")?;
            Ok((root, Some(file._sFile.clone()), "GameBanana"))
        } else {
            Err("mod inconnu".into())
        }
    })();
    let outcome = result.and_then(|(root, version, source)| {
        emit("install", 0, 1);
        if target.exists() {
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        move_dir(&root, &target)?;
        write_marker(&target, id, version, source)
    });
    let _ = fs::remove_dir_all(&work);
    outcome
}

fn uninstall_switch(app: &AppHandle, tid: u64, id: &str) -> Result<(), String> {
    let r = resolve(EmulatorId::Eden, app);
    let game_dir = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?.join(format!("{tid:016X}"));
    let name = installed_dirs(&game_dir).into_iter().find(|(_, m)| m.as_ref().is_some_and(|m| m.id == id)).map(|(n, _)| n).ok_or("ce mod n'est pas installé par Kaleido")?;
    fs::remove_dir_all(game_dir.join(name)).map_err(|e| e.to_string())
}

/// Met de côté (ou remet en place) un mod installé hors de Kaleido.
fn toggle_other(app: &AppHandle, tid: u64, name: &str, enabled: bool) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." {
        return Err("nom invalide".into());
    }
    let r = resolve(EmulatorId::Eden, app);
    let game_dir = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?.join(format!("{tid:016X}"));
    let parked = disabled_dir(app, tid)?.join(name);
    let active = game_dir.join(name);
    let (from, to) = if enabled { (parked, active) } else { (active, parked) };
    if !from.is_dir() {
        return Err("dossier introuvable".into());
    }
    if to.exists() {
        return Err(format!("un dossier « {name} » existe déjà"));
    }
    move_dir(&from, &to)
}

// ---------------------------------------------------------------------------
// 3DS

const CTR_GAMES: &[(u64, &str, &str)] = &[
    (0x0004000000055D00, "X", "X"),
    (0x0004000000055E00, "Y", "Y"),
    (0x000400000011C400, "Omega Ruby", "Rubis Oméga"),
    (0x000400000011C500, "Alpha Sapphire", "Saphir Alpha"),
    (0x0004000000164800, "Sun", "Soleil"),
    (0x0004000000175E00, "Moon", "Lune"),
    (0x00040000001B5000, "Ultra Sun", "Ultra-Soleil"),
    (0x00040000001B5100, "Ultra Moon", "Ultra-Lune"),
];

/// Packs de textures HD : (titles, archives, taille totale, auteur, page).
struct TexturePack {
    titles: &'static [u64],
    urls: &'static [&'static str],
    size: u64,
    author: &'static str,
    note: &'static str,
}

const TEXTURE_PACKS: &[TexturePack] = &[
    TexturePack {
        titles: &[0x0004000000055D00, 0x0004000000055E00],
        urls: &["https://github.com/Gray-Rice/PokeTex-3DS/releases/download/xy/XY.7z"],
        size: 1_834_769_856,
        author: "Ullr8 (Pokémon Y Resurrection), archivé par Gray-Rice",
        note: "Pack réalisé sur Pokémon Y (la plupart des textures sont communes avec X). Encore en cours selon l'auteur.",
    },
    TexturePack {
        titles: &[0x000400000011C400, 0x000400000011C500],
        urls: &["https://github.com/Gray-Rice/PokeTex-3DS/releases/download/ORAS/Part-1.7z", "https://github.com/Gray-Rice/PokeTex-3DS/releases/download/ORAS/Part-2.7z"],
        size: 1_399_327_724 + 1_544_698_598,
        author: "Donel, archivé par Gray-Rice",
        note: "",
    },
    TexturePack {
        titles: &[0x00040000001B5000, 0x00040000001B5100],
        urls: &["https://github.com/Gray-Rice/PokeTex-3DS/releases/download/usum/USUM.7z"],
        size: 124_548_993,
        author: "Volya, archivé par Gray-Rice",
        note: "",
    },
];

fn texture_pack(tid: u64) -> Option<&'static TexturePack> {
    TEXTURE_PACKS.iter().find(|p| p.titles.contains(&tid))
}

/// Convertit un fichier de codes CTRPF (`{commentaires}`, dossiers `[++…++]` / `[--]`) au format d'Azahar.
pub fn ctrpf_to_azahar(text: &str) -> String {
    let mut out = String::new();
    let mut current: Option<(String, Vec<String>, Vec<String>)> = None;
    let mut names: Vec<String> = Vec::new();
    let mut flush = |c: Option<(String, Vec<String>, Vec<String>)>, out: &mut String| {
        if let Some((name, comments, codes)) = c {
            if codes.is_empty() {
                return;
            }
            let mut unique = name.clone();
            let mut n = 2;
            while names.contains(&unique) {
                unique = format!("{name} ({n})");
                n += 1;
            }
            names.push(unique.clone());
            out.push_str(&format!("[{unique}]\n"));
            for c in comments {
                out.push_str(&format!("*{c}\n"));
            }
            for c in codes {
                out.push_str(&format!("{c}\n"));
            }
            out.push('\n');
        }
    };
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            flush(current.take(), &mut out);
            let name = name.trim();
            if name.starts_with("++") || name.chars().all(|c| c == '-') {
                continue;
            }
            current = Some((name.replace(['[', ']'], ""), vec![], vec![]));
        } else if let Some(comment) = line.strip_prefix('{') {
            if let Some(c) = current.as_mut() {
                let text = comment.trim_end_matches('}').trim();
                if !text.is_empty() && !text.starts_with("citra_enabled") {
                    c.1.push(text.to_string());
                }
            }
        } else if is_code_line(line) {
            if let Some(c) = current.as_mut() {
                c.2.push(line.to_ascii_uppercase());
            }
        }
    }
    flush(current.take(), &mut out);
    out
}

fn is_code_line(line: &str) -> bool {
    let parts: Vec<&str> = line.split_whitespace().collect();
    parts.len() == 2 && parts.iter().all(|p| p.len() == 8 && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn ctr_view(app: &AppHandle, tid: u64) -> ModsView {
    let r = ctr_emulator(app);
    let mut view = ModsView { emulator: Some(r.id.name()), emulator_found: r.exe.is_some(), ..Default::default() };
    let Some(user) = r.ctr_user_dir() else {
        view.errors.push("Dossier de l'émulateur 3DS introuvable.".into());
        return view;
    };
    view.location = Some(user.display().to_string());
    let Some((_, _, fr)) = CTR_GAMES.iter().find(|g| g.0 == tid) else {
        view.notes.push("Aucun mod connu pour ce jeu.".into());
        return view;
    };
    let cheats = user.join("cheats").join(format!("{tid:016X}.txt"));
    view.mods.push(ModEntry {
        id: "cheats".into(),
        name: "Codes de triche".into(),
        description: "Ajoute la base de codes de la communauté (argent, objets, éclosion rapide, chromatiques…). Ils sont installés désactivés : tu choisis ensuite lesquels activer, ici ou dans le menu Triches de l'émulateur.".into(),
        category: "cheats",
        group: None,
        recommended: false,
        author: "iSharingan / JourneyOver (CTRPF-AR-CHEAT-CODES)".into(),
        page: "https://github.com/iSharingan/CTRPF-AR-CHEAT-CODES".into(),
        size: None,
        game_version: None,
        installed: cheats.is_file(),
        update_available: false,
        conflicts: vec![],
        warning: Some("Certains codes ne marchent qu'avec une version précise du jeu (indiquée dans leur nom). Sauvegarde avant d'en essayer.".into()),
    });
    if let Some(pack) = texture_pack(tid) {
        let dir = user.join("load").join("textures").join(format!("{tid:016X}"));
        view.mods.push(ModEntry {
            id: "textures".into(),
            name: "Textures HD".into(),
            description: format!(
                "Remplace les textures du jeu par des versions haute définition, affichées par {}. {}",
                r.id.name(),
                if pack.note.is_empty() { "" } else { pack.note }
            ),
            category: "textures",
            group: None,
            recommended: tuning::current_tier() >= Tier::Medium,
            author: pack.author.into(),
            page: "https://github.com/Gray-Rice/PokeTex-3DS".into(),
            size: Some(pack.size),
            game_version: None,
            installed: dir.join(MARKER).is_file(),
            update_available: false,
            conflicts: vec![],
            warning: Some("Ferme l'émulateur avant d'installer : Kaleido active l'option « textures personnalisées » dans sa configuration.".into()),
        });
    }
    view.notes.push(format!(
        "Pas de vrai mode 60 FPS pour Pokémon {fr} : les codes « 60 FPS » qui circulent font tourner tout le jeu deux fois plus vite (sa logique est calée sur 30 images par seconde). Kaleido ne les propose donc pas."
    ));
    view
}

fn install_ctr(app: &AppHandle, tid: u64, id: &str, emit: &dyn Fn(&'static str, u64, u64)) -> Result<(), String> {
    let r = ctr_emulator(app);
    let user = r.ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
    let (_, en, _) = CTR_GAMES.iter().find(|g| g.0 == tid).ok_or("jeu inconnu")?;
    match id {
        "cheats" => {
            emit("download", 0, 1);
            let url = format!("https://raw.githubusercontent.com/iSharingan/CTRPF-AR-CHEAT-CODES/master/Cheats/{}/{tid:016X}.txt", encode(&format!("Pokémon {en} (GLO)")));
            let text = String::from_utf8_lossy(&get(&url, false)?).into_owned();
            let converted = ctrpf_to_azahar(&text);
            let file = user.join("cheats").join(format!("{tid:016X}.txt"));
            fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
            // Codes déjà présents (ajoutés à la main) : on les garde.
            let mut out = fs::read_to_string(&file).unwrap_or_default();
            let existing: Vec<String> = parse_azahar(&out).into_iter().map(|c| c.name).collect();
            for cheat in converted.split("\n\n").filter(|c| !c.trim().is_empty()) {
                let name = cheat.lines().next().unwrap_or("").trim_matches(['[', ']']).to_string();
                if !existing.contains(&name) {
                    if !out.is_empty() && !out.ends_with("\n\n") {
                        out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
                    }
                    out.push_str(cheat.trim_end());
                    out.push_str("\n\n");
                }
            }
            fs::write(&file, out).map_err(|e| e.to_string())
        }
        "textures" => {
            let pack = texture_pack(tid).ok_or("pas de pack de textures pour ce jeu")?;
            let target = user.join("load").join("textures").join(format!("{tid:016X}"));
            let work = work_dir(app)?.join(format!("tex-{tid:016X}"));
            let _ = fs::remove_dir_all(&work);
            fs::create_dir_all(&work).map_err(|e| e.to_string())?;
            let result = (|| -> Result<(), String> {
                let out = work.join("x");
                let total = pack.size;
                let mut before = 0u64;
                for (i, url) in pack.urls.iter().enumerate() {
                    let archive = work.join(format!("part{i}.7z"));
                    let got = download_to(url, &archive, 0, |d, _| emit("download", before + d, total))?;
                    before += got;
                    extract_any(&archive, &out, |d, t| emit("extract", d, t))?;
                    let _ = fs::remove_file(&archive);
                }
                emit("install", 0, 1);
                // Le pack est rangé dans un dossier (souvent au title ID) : on prend le plus haut
                // dossier qui contient des images.
                let root = texture_root(&out, 4).ok_or("aucune texture dans l'archive")?;
                if target.exists() && !target.join(MARKER).is_file() {
                    let backup = target.with_file_name(format!("{tid:016X}.avant-kaleido"));
                    let _ = fs::remove_dir_all(&backup);
                    fs::rename(&target, &backup).map_err(|e| e.to_string())?;
                }
                merge_dir(&root, &target)?;
                write_marker(&target, "textures", None, "Gray-Rice/PokeTex-3DS")?;
                tuning::set_azahar_custom_textures(&user, true, tuning::current_tier() >= Tier::High)
            })();
            let _ = fs::remove_dir_all(&work);
            result
        }
        _ => Err("mod inconnu".into()),
    }
}

/// Plus haut dossier contenant des images `.png` (ou un `pack.json`).
fn texture_root(dir: &Path, depth: u8) -> Option<PathBuf> {
    let entries: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    if entries.iter().any(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("dds")) || p.file_name().is_some_and(|n| n == "pack.json")) {
        return Some(dir.to_path_buf());
    }
    let dirs: Vec<&PathBuf> = entries.iter().filter(|p| p.is_dir()).collect();
    if depth == 0 || dirs.is_empty() {
        return None;
    }
    // Un seul sous-dossier : on descend. Plusieurs : ce dossier est la racine s'ils contiennent des images.
    if dirs.len() == 1 {
        return texture_root(dirs[0], depth - 1);
    }
    dirs.iter().any(|d| texture_root(d, depth - 1).is_some()).then(|| dir.to_path_buf())
}

fn uninstall_ctr(app: &AppHandle, tid: u64, id: &str) -> Result<(), String> {
    let r = ctr_emulator(app);
    let user = r.ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
    match id {
        "cheats" => fs::remove_file(user.join("cheats").join(format!("{tid:016X}.txt"))).map_err(|e| e.to_string()),
        "textures" => {
            let target = user.join("load").join("textures").join(format!("{tid:016X}"));
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            let backup = target.with_file_name(format!("{tid:016X}.avant-kaleido"));
            if backup.is_dir() {
                fs::rename(&backup, &target).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        _ => Err("mod inconnu".into()),
    }
}

// ---------------------------------------------------------------------------
// DS

/// Titres No-Intro (français, anglais) d'après les 3 premières lettres du code du jeu.
fn nds_titles(code3: &str) -> Option<(&'static str, &'static str)> {
    Some(match code3 {
        "ADA" => ("Pokemon - Version Diamant", "Pokemon - Diamond Version"),
        "APA" => ("Pokemon - Version Perle", "Pokemon - Pearl Version"),
        "CPU" => ("Pokemon - Version Platine", "Pokemon - Platinum Version"),
        "IPK" => ("Pokemon - Version Or HeartGold", "Pokemon - HeartGold Version"),
        "IPG" => ("Pokemon - Version Argent SoulSilver", "Pokemon - SoulSilver Version"),
        "IRB" => ("Pokemon - Version Noire", "Pokemon - Black Version"),
        "IRA" => ("Pokemon - Version Blanche", "Pokemon - White Version"),
        "IRE" => ("Pokemon - Version Noire 2", "Pokemon - Black Version 2"),
        "IRD" => ("Pokemon - Version Blanche 2", "Pokemon - White Version 2"),
        _ => return None,
    })
}

/// Noms de fichiers `.mch` possibles pour un code de jeu (`CPUF` → « … Platine (France) [CPUF] »).
pub fn mch_candidates(code: &str) -> Vec<String> {
    let Some((fr, en)) = code.get(..3).and_then(nds_titles) else { return vec![] };
    let (title, region) = match code.as_bytes().get(3) {
        Some(b'F') => (fr, "France"),
        Some(b'E') => (en, "USA"),
        Some(b'O') => (en, "USA, Europe"),
        Some(b'P') => (en, "Europe"),
        _ => return vec![],
    };
    let mut out = vec![format!("{title} ({region}) [{code}]")];
    for rev in 1..=5 {
        out.push(format!("{title} ({region}) (Rev {rev}) [{code}]"));
    }
    out
}

fn nds_code(rom: &Path) -> Option<String> {
    let mut f = fs::File::open(rom).ok()?;
    let mut header = [0u8; 0x10];
    f.read_exact(&mut header).ok()?;
    let code = std::str::from_utf8(&header[0x0C..0x10]).ok()?;
    code.bytes().all(|b| b.is_ascii_alphanumeric()).then(|| code.to_string())
}

/// En dessous, le fichier de la base est jugé inutilisable (Diamant FR : 9 codes d'un autre jeu).
const MIN_NDS_CHEATS: usize = 20;

fn mch_path(rom: &Path) -> PathBuf {
    rom.with_extension("mch")
}

fn nds_view(app: &AppHandle, rom: &Path) -> ModsView {
    let r = resolve(EmulatorId::Melonds, app);
    let mut view = ModsView { emulator: Some("melonDS"), emulator_found: r.exe.is_some(), location: rom.parent().map(|p| p.display().to_string()), ..Default::default() };
    let code = nds_code(rom);
    if code.as_deref().and_then(|c| c.get(..3)).and_then(nds_titles).is_none() {
        view.notes.push("Aucun mod connu pour ce jeu.".into());
        return view;
    }
    view.mods.push(ModEntry {
        id: "cheats".into(),
        name: "Codes de triche".into(),
        description: "Ajoute la base de codes de la communauté (chaussures de course, traverser les murs, texte rapide, objets…), lue par melonDS. Les codes sont installés désactivés : tu choisis ensuite lesquels activer.".into(),
        category: "cheats",
        group: None,
        recommended: false,
        author: "DeadSkullzJr, converti par Lyrx997".into(),
        page: "https://github.com/Lyrx997/MelonDS-Desktop-Cheats".into(),
        size: None,
        game_version: None,
        installed: mch_path(rom).is_file(),
        update_available: false,
        conflicts: vec![],
        warning: Some("Sauvegarde avant d'essayer un code : certains peuvent bloquer le jeu.".into()),
    });
    let config = play::load_config(app);
    if config.preferred_nds == Some(EmulatorId::Desmume) {
        view.notes.push("Les codes de triche sont lus par melonDS, pas par DeSmuME.".into());
    }
    view.notes.push("Pas de vrai mode 60 FPS pour les Pokémon DS : le jeu est calé sur 30 images par seconde et les codes qui débloquent la cadence l'accélèrent entièrement. Kaleido ne les propose donc pas.".into());
    view
}

fn install_nds(app: &AppHandle, rom: &Path, emit: &dyn Fn(&'static str, u64, u64)) -> Result<(), String> {
    let code = nds_code(rom).ok_or("code du jeu illisible")?;
    emit("download", 0, 1);
    let mut last = String::from("aucun fichier pour ce jeu");
    for name in mch_candidates(&code) {
        let url = format!("https://raw.githubusercontent.com/Lyrx997/MelonDS-Desktop-Cheats/master/cheats/{}.mch", encode(&name));
        match get(&url, false) {
            // Certains fichiers de la base sont presque vides ou visent un autre jeu.
            Ok(data) if parse_mch(&String::from_utf8_lossy(&data)).len() < MIN_NDS_CHEATS => {
                last = "la base ne contient pas de codes fiables pour cette version du jeu".into();
            }
            Ok(data) => {
                let file = mch_path(rom);
                if file.is_file() {
                    let _ = fs::copy(&file, rom.with_extension("mch.avant-kaleido"));
                }
                fs::write(&file, data).map_err(|e| e.to_string())?;
                if let Some(exe) = resolve(EmulatorId::Melonds, app).exe {
                    tuning::enable_melonds_cheats(&exe)?;
                }
                return Ok(());
            }
            Err(e) => last = e,
        }
    }
    Err(format!("codes introuvables pour {code} ({last})"))
}

// ---------------------------------------------------------------------------
// Liste des codes de triche (activation un par un)

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cheat {
    pub name: String,
    pub group: Option<String>,
    pub enabled: bool,
}

/// Codes d'un fichier Azahar (`[Nom]`, `*citra_enabled`).
pub fn parse_azahar(text: &str) -> Vec<Cheat> {
    let mut out: Vec<Cheat> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Cheat { name: name.to_string(), group: None, enabled: false });
        } else if line == "*citra_enabled" {
            if let Some(c) = out.last_mut() {
                c.enabled = true;
            }
        }
    }
    out
}

pub fn set_azahar(text: &str, enabled: &[String]) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "*citra_enabled" {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if let Some(name) = trimmed.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if enabled.iter().any(|e| e == name) {
                out.push_str("*citra_enabled\n");
            }
        }
    }
    out
}

/// Codes d'un fichier melonDS (`CAT nom`, `CODE <0|1> nom`).
pub fn parse_mch(text: &str) -> Vec<Cheat> {
    let mut group = None;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(cat) = line.strip_prefix("CAT ") {
            group = Some(cat.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("CODE ") {
            let (flag, name) = rest.split_once(' ').unwrap_or((rest, ""));
            out.push(Cheat { name: name.trim().to_string(), group: group.clone(), enabled: flag == "1" });
        }
    }
    out
}

pub fn set_mch(text: &str, enabled: &[String]) -> String {
    let mut out = String::new();
    for line in text.lines() {
        match line.strip_prefix("CODE ").and_then(|r| r.split_once(' ')) {
            Some((_, name)) => {
                let on = enabled.iter().any(|e| e == name.trim());
                out.push_str(&format!("CODE {} {}", if on { 1 } else { 0 }, name));
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

fn cheats_file(app: &AppHandle, target: &ModTarget) -> Result<(PathBuf, bool), String> {
    match target.platform.as_str() {
        "3ds" => {
            let tid = parse_tid(target.title_id.as_deref())?;
            let user = ctr_emulator(app).ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
            Ok((user.join("cheats").join(format!("{tid:016X}.txt")), false))
        }
        "nds" => Ok((mch_path(Path::new(target.rom.as_deref().ok_or("ROM inconnue")?)), true)),
        _ => Err("pas de codes de triche pour cette console".into()),
    }
}

// ---------------------------------------------------------------------------
// Commandes

fn view_for(app: &AppHandle, target: &ModTarget) -> Result<ModsView, String> {
    match target.platform.as_str() {
        "switch" => Ok(switch_view(app, base_title_id(parse_tid(target.title_id.as_deref())?))),
        "3ds" => Ok(ctr_view(app, parse_tid(target.title_id.as_deref())?)),
        "nds" => Ok(nds_view(app, Path::new(target.rom.as_deref().ok_or("ROM inconnue")?))),
        "gba" => Ok(gba_view(app)),
        _ => Err("console inconnue".into()),
    }
}

/// Game Boy et Game Boy Advance (mGBA) : pas encore de mods ni de codes proposés.
fn gba_view(app: &AppHandle) -> ModsView {
    let r = resolve(EmulatorId::Mgba, app);
    let mut view = ModsView { emulator: Some("mGBA"), emulator_found: r.exe.is_some(), ..Default::default() };
    view.notes.push("Pas encore de mods ni de codes de triche proposés pour les jeux Game Boy et Game Boy Advance. mGBA garde ses propres codes (menu Outils, Codes de triche).".into());
    view
}

/// Mods proposés pour un jeu, avec leur état.
#[tauri::command]
pub async fn mods_list(target: ModTarget, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || view_for(&app, &target)).await
}

/// Télécharge et installe un mod (`disable_conflicts` : retire d'abord les mods incompatibles).
#[tauri::command]
pub async fn mods_install(target: ModTarget, id: String, disable_conflicts: bool, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("mod-install", Progress { id: id.clone(), step, done, total });
        };
        match target.platform.as_str() {
            "switch" => install_switch(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &id, disable_conflicts, &emit)?,
            "3ds" => install_ctr(&app, parse_tid(target.title_id.as_deref())?, &id, &emit)?,
            "nds" => install_nds(&app, Path::new(target.rom.as_deref().ok_or("ROM inconnue")?), &emit)?,
            _ => return Err("console inconnue".into()),
        }
        view_for(&app, &target)
    })
    .await
}

#[tauri::command]
pub async fn mods_uninstall(target: ModTarget, id: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        match target.platform.as_str() {
            "switch" => uninstall_switch(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &id)?,
            "3ds" => uninstall_ctr(&app, parse_tid(target.title_id.as_deref())?, &id)?,
            "nds" => {
                let rom = PathBuf::from(target.rom.as_deref().ok_or("ROM inconnue")?);
                fs::remove_file(mch_path(&rom)).map_err(|e| e.to_string())?;
                let backup = rom.with_extension("mch.avant-kaleido");
                if backup.is_file() {
                    fs::rename(&backup, mch_path(&rom)).map_err(|e| e.to_string())?;
                }
            }
            _ => return Err("console inconnue".into()),
        }
        view_for(&app, &target)
    })
    .await
}

/// Switch : met de côté ou remet en place un mod installé hors de Kaleido.
#[tauri::command]
pub async fn mods_toggle_other(target: ModTarget, name: String, enabled: bool, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        toggle_other(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &name, enabled)?;
        view_for(&app, &target)
    })
    .await
}

#[tauri::command]
pub async fn cheats_list(target: ModTarget, app: AppHandle) -> Result<Vec<Cheat>, String> {
    crate::blocking(move || {
        let (file, mch) = cheats_file(&app, &target)?;
        let text = fs::read_to_string(&file).map_err(|_| "codes non installés".to_string())?;
        Ok(if mch { parse_mch(&text) } else { parse_azahar(&text) })
    })
    .await
}

/// Active exactement les codes nommés (les autres sont désactivés).
#[tauri::command]
pub async fn cheats_set(target: ModTarget, enabled: Vec<String>, app: AppHandle) -> Result<Vec<Cheat>, String> {
    crate::blocking(move || {
        let (file, mch) = cheats_file(&app, &target)?;
        let text = fs::read_to_string(&file).map_err(|_| "codes non installés".to_string())?;
        let out = if mch { set_mch(&text, &enabled) } else { set_azahar(&text, &enabled) };
        fs::write(&file, &out).map_err(|e| e.to_string())?;
        Ok(if mch { parse_mch(&out) } else { parse_azahar(&out) })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_keys() {
        assert_eq!(split_version("[60 FPS v1.3.0]"), ("60 FPS".into(), Some("1.3.0".into())));
        assert_eq!(split_version("[Dynamic FPS Scarlet v4.0.0][ASM] Final Release"), ("Dynamic FPS Scarlet ASM Final Release".into(), Some("4.0.0".into())));
        assert_eq!(split_version("Ash"), ("Ash".into(), None));
        assert_eq!(mod_key("60 FPS Static version"), "60-fps-static-version");
    }

    #[test]
    fn classification() {
        let k = classify("[60 FPS Static version v1.1.1]", Tier::Ultra);
        assert_eq!((k.category, k.group, k.name.as_str(), k.rank), ("fps", Some("fps"), "60 FPS", 3));
        let d = classify("[Dynamic FPS v1.1.1]", Tier::Ultra);
        assert_eq!((d.name.as_str(), d.rank), ("60 FPS adaptatif", 1));
        assert_eq!(classify("[Dynamic FPS v1.1.1]", Tier::Medium).rank, 3);
        assert!(classify("[120 FPS v1.3.0]", Tier::Ultra).warning.is_some());
        assert!(classify("[21.9 Ultrawide v1.1.1 for Ryujinx]", Tier::Ultra).hidden);
        assert_eq!(classify("[3440x1440 Ultrawide v1.1.1]", Tier::Ultra).name, "Écran ultra-large (3440×1440)");
        assert_eq!(classify("[3840x2160 v1.1.1]", Tier::Ultra).group, Some("resolution"));
        assert_eq!(classify("[4K Shadows v4.0.0]", Tier::Ultra).rank, 2);
        assert_eq!(classify("[2K Shadows v4.0.0]", Tier::Ultra).rank, 0);
        assert_eq!(classify("[Level of Detail v1.3.0]", Tier::Ultra).rank, 2);
        assert!(classify("[Level of Detail + QOL mods v1.1.1]", Tier::Ultra).warning.is_some());
        assert_eq!(classify("06 60 FPS v1.1.1", Tier::Ultra).group, Some("fps"));
        assert_eq!(classify("Ash", Tier::Ultra).category, "other");
    }

    #[test]
    fn zip_layout() {
        let tid = 0x0100ABF008968000;
        let entries: Vec<(String, u64)> = [
            ("Mods notes.txt", 10),
            ("[60 FPS v1.3.2]/", 0),
            ("[60 FPS v1.3.2]/exefs/1.3.2.pchtxt", 500),
            ("Ash/Clothes.txt", 5),
            ("Ash/01008DB008C2C000/romfs/a.gfpak", 100),
            ("Ash/0100ABF008968000/romfs/a.gfpak", 200),
            ("Ash/romfs/a.gfpak", 300),
            ("Notes/readme.txt", 1),
        ]
        .iter()
        .map(|(n, s)| (n.to_string(), *s))
        .collect();
        let mods = zip_mods(&entries, tid);
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0], ZipMod { folder: "Ash".into(), root: "Ash/0100ABF008968000/".into(), size: 200 });
        assert_eq!(mods[1], ZipMod { folder: "[60 FPS v1.3.2]".into(), root: "[60 FPS v1.3.2]/".into(), size: 500 });
    }

    #[test]
    fn real_fl4sh_names() {
        // Tous les jeux Pokémon Switch ont une archive connue.
        for tid in [0x010003F003A34000u64, 0x0100187003A36000, 0x0100ABF008968000, 0x01008DB008C2C000, 0x0100000011D90000, 0x010018E011D92000, 0x01001F5010DFA000, 0x0100A3D008C5C000, 0x01008F6008C5E000, 0x0100F43008C44000] {
            assert!(FL4SH_KNOWN.iter().any(|n| crate::switch::title_id_in_name(n).map(base_title_id) == Some(tid)), "{tid:016X}");
        }
        // Les title ID de mise à jour sont ramenés au jeu de base.
        assert!(FL4SH_KNOWN.iter().any(|n| crate::switch::title_id_in_name(n).map(base_title_id) == Some(0x010055D009F78000)));
        assert_eq!(encode("Pokémon Let's Go, Pikachu! [010003F003A34000][mods].zip"), "Pok%C3%A9mon%20Let%27s%20Go%2C%20Pikachu%21%20%5B010003F003A34000%5D%5Bmods%5D.zip");
    }

    #[test]
    fn ctrpf_conversion() {
        let src = "\u{feff}[++Currency codes++]\n\n[Add money]\nD3000000 00000000\n38c71dc0 00030D40\n{Buy all the things}\n\n[99999 Pokemiles]\n08C8B36C 0001869F\n\n[--]\n\n[Empty folder name]\n\n[99999 Pokemiles]\n08C8B36C 0001869F\n";
        let out = ctrpf_to_azahar(src);
        assert_eq!(out, "[Add money]\n*Buy all the things\nD3000000 00000000\n38C71DC0 00030D40\n\n[99999 Pokemiles]\n08C8B36C 0001869F\n\n[99999 Pokemiles (2)]\n08C8B36C 0001869F\n\n");
        let cheats = parse_azahar(&out);
        assert_eq!(cheats.len(), 3);
        let on = set_azahar(&out, &["99999 Pokemiles".into()]);
        assert!(on.contains("[99999 Pokemiles]\n*citra_enabled\n08C8B36C"));
        assert_eq!(parse_azahar(&on).iter().filter(|c| c.enabled).count(), 1);
        let off = set_azahar(&on, &[]);
        assert_eq!(off, out);
    }

    #[test]
    fn mch_toggle() {
        let src = "CAT Misc\n\nCODE 0 Enable Running Shoes (Press Select)\n94000130 FFFB0000\nD2000000 00000000\n\nCODE 1 Walk Through Walls\n12060CC4 00000200\n";
        let list = parse_mch(src);
        assert_eq!(list, vec![
            Cheat { name: "Enable Running Shoes (Press Select)".into(), group: Some("Misc".into()), enabled: false },
            Cheat { name: "Walk Through Walls".into(), group: Some("Misc".into()), enabled: true },
        ]);
        let out = set_mch(src, &["Enable Running Shoes (Press Select)".into()]);
        assert!(out.contains("CODE 1 Enable Running Shoes") && out.contains("CODE 0 Walk Through Walls"));
    }

    #[test]
    fn mch_names() {
        let c = mch_candidates("CPUF");
        assert_eq!(c[0], "Pokemon - Version Platine (France) [CPUF]");
        assert!(mch_candidates("ADAF").contains(&"Pokemon - Version Diamant (France) (Rev 5) [ADAF]".to_string()));
        assert_eq!(mch_candidates("IRBO")[0], "Pokemon - Black Version (USA, Europe) [IRBO]");
        assert!(mch_candidates("CPUD").is_empty());
        assert!(mch_candidates("AMCE").is_empty());
    }

    #[test]
    fn folder_names_are_valid() {
        assert_eq!(folder_name("Écran ultra-large 21:9"), "Écran ultra-large 21-9");
        assert_eq!(folder_name("Mod. "), "Mod");
    }
}
