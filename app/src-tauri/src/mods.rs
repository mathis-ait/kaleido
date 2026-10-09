//! Mods en un clic pour les jeux de la bibliothèque.
//!
//! - **Switch (Eden)** : mods de Fl4sh9174/Switch-Emulator-Ultrawide-FPS-Mods (une
//!   archive par jeu, `Nom [TITLEID][mods].zip`, un dossier `[Mod vX.Y.Z]/exefs|romfs`
//!   par mod). Les « 60 FPS » de Fl4sh sont de vrais correctifs de cadence : le jeu
//!   tourne à vitesse normale (animations, physique, musique), contrairement à une
//!   accélération de l'émulateur. Pour Légendes Arceus, quelques mods GameBanana
//!   (textures HD, ciel, distance d'affichage), vérifiés par MD5.
//!   Installation : `<load>/<TITLEID>/<nom du mod>/` (dossier `load_directory` d'Eden),
//!   avec un `kaleido-mod.json`. Les mods (de Kaleido ou non) peuvent être mis de
//!   côté (déplacés dans `<données>/mods-disabled`) et remis en place.
//! - **Catalogue** (`mods_catalog.rs`) : les meilleurs mods de chaque jeu, choisis à la
//!   main (GameBanana, ou pages à télécharger soi-même comme Nexus Mods), et
//!   **explorateur** de tous les mods GameBanana du jeu (`gamebanana.rs`). Une archive
//!   téléchargée à la main (.zip, .7z, .rar) peut aussi être installée.
//! - **3DS (Azahar)** : codes de triche (iSharingan/CTRPF-AR-CHEAT-CODES, convertis au
//!   format d'Azahar : `cheats/<TITLEID>.txt`) et packs de textures HD
//!   (Gray-Rice/PokeTex-3DS, `load/textures/<TITLEID>/`).
//!   Mods LayeredFS du catalogue et de GameBanana : fusionnés dans `load/mods/<TITLEID>/`
//!   (Azahar n'en lit qu'un par jeu), fichiers de chaque mod notés dans `kaleido-mods.json`.
//! - **DS (melonDS)** : codes de triche au format `.mch` (Lyrx997/MelonDS-Desktop-Cheats),
//!   placés à côté de la ROM, là où melonDS les cherche ; romhacks du catalogue livrés en
//!   patch (xdelta, BPS, IPS) appliqués sur la ROM, à côté d'elle.
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
use kaleido_core::data::fps60_ctr;

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
    /// Version du jeu installée (mise à jour Switch), pour signaler les mods prévus pour une autre.
    #[serde(default)]
    pub game_version: Option<String>,
    /// Switch : fichier de la mise à jour installée (`rom` = jeu de base).
    #[serde(default)]
    pub update_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Voir `mods_catalog::CATEGORIES`.
    pub category: String,
    /// Mods incompatibles entre eux (un seul à la fois).
    pub group: Option<String>,
    pub recommended: bool,
    /// « auto » (Fl4sh, codes, PokeTex), « gamebanana » / « manual » (catalogue, téléchargé par
    /// Kaleido ou par l'utilisateur), « explorer » / « local » (installé hors catalogue).
    pub source: String,
    pub author: String,
    pub page: String,
    pub size: Option<u64>,
    /// Version du jeu visée par le mod.
    pub game_version: Option<String>,
    pub installed: bool,
    /// Installé mais mis de côté.
    pub enabled: bool,
    /// Version installée différente de celle proposée.
    pub update_available: bool,
    /// Mods déjà installés qui entrent en conflit avec celui-ci.
    pub conflicts: Vec<String>,
    /// Mods actifs qui remplacent les mêmes fichiers du jeu.
    pub overlaps: Vec<String>,
    /// Mods du catalogue nécessaires (noms), pas encore installés.
    pub requires: Vec<String>,
    #[serde(skip)]
    pub requires_ids: Vec<String>,
    pub exclusive: bool,
    pub warning: Option<String>,
    pub thumbnail: Option<String>,
    pub gb: Option<u32>,
    pub popularity: Option<u64>,
    /// « layeredfs », « textures », « patch » (DS), « cheats ».
    pub kind: Option<String>,
    /// Correctifs ExeFS (Switch) : « ok » (visent l'exécutable lancé), « base » (le jeu sans
    /// mise à jour), « other » (une autre version) ; absent quand le mod n'en a pas ou que
    /// l'exécutable n'a pas pu être lu.
    pub exefs: Option<String>,
    /// Touche à la partie (romhack, rééquilibrage, mod exclusif) : sauvegarde copiée avant.
    pub affects_save: bool,
    /// Fichier GameBanana d'avant la dernière mise à jour (encore en cache) : retour possible.
    pub previous_file: Option<u64>,
    /// Dossier du mod (Switch), pour l'ordre de priorité.
    pub folder: Option<String>,
    /// Le mod ne peut pas s'appliquer à ce jeu (ex. patch DS pour une autre version) : la raison.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtherMod {
    pub name: String,
    pub enabled: bool,
    pub category: String,
    pub overlaps: Vec<String>,
    pub can_toggle: bool,
    pub exefs: Option<String>,
    /// Généré par Kaleido (Randomizer, éditeur de ROM) mais pas suivi par le manifeste des mods.
    #[serde(default)]
    pub from_kaleido: bool,
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
    /// Jeu GameBanana (explorateur).
    pub gamebanana: Option<u32>,
    /// Mods GameBanana déjà installés (pour l'explorateur).
    pub installed_gb: Vec<u32>,
    /// Une archive téléchargée à la main peut être installée.
    pub can_import: bool,
    /// Switch : exécutable lancé par Eden, vérifié pour les correctifs ExeFS.
    pub executable: Option<Executable>,
    /// Switch : mise à jour séparée qu'Eden ne lance pas (nom du fichier) : Kaleido peut
    /// l'installer dans sa NAND.
    pub install_update: Option<String>,
    /// Copies de la sauvegarde du jeu (les plus récentes d'abord).
    pub saves: Vec<crate::mods_saves::SaveBackup>,
    /// Copie faite juste avant ce changement.
    pub last_backup: Option<crate::mods_saves::SaveBackup>,
    /// Copie à proposer de remettre (mod de partie désactivé ou retiré).
    pub restore_offer: Option<crate::mods_saves::SaveBackup>,
    /// Profils de mods du jeu (Switch, 3DS).
    pub profiles: Option<crate::mods_saves::GameProfiles>,
    /// Mods d'un profil qui ne sont plus installés (après un changement de profil).
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Executable {
    /// « la mise à jour 1.1.1 », « le jeu de base ».
    pub source: String,
    pub build_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Marker {
    id: String,
    version: Option<String>,
    source: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    gb: Option<u32>,
    #[serde(default)]
    page: Option<String>,
    /// Fichier GameBanana installé, et celui d'avant une mise à jour (pour revenir en arrière).
    #[serde(default)]
    file: Option<u64>,
    #[serde(default)]
    previous_file: Option<u64>,
    /// Version Nexus Mods au moment de l'installation.
    #[serde(default)]
    nexus_version: Option<String>,
}

/// Options d'installation.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    /// Retire d'abord les mods incompatibles.
    #[serde(default)]
    pub disable_conflicts: bool,
    /// Fichier GameBanana choisi (sinon celui du catalogue, sinon le plus récent).
    #[serde(default)]
    pub file: Option<u64>,
    /// Variante choisie quand l'archive en contient plusieurs.
    #[serde(default)]
    pub variant: Option<String>,
    /// Archive ou patch téléchargé à la main.
    #[serde(default)]
    pub local: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub view: Option<ModsView>,
    /// L'archive contient plusieurs variantes : à choisir, puis relancer l'installation.
    pub variants: Vec<String>,
    /// ROM créée (romhack DS).
    pub output: Option<String>,
}

/// Résultat interne d'une installation.
enum Installed {
    Done,
    Variants(Vec<String>),
    Output(PathBuf),
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
pub(crate) fn cached(app: &AppHandle, name: &str, url: &str, max_age: Duration, browser: bool) -> Result<Vec<u8>, String> {
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

pub(crate) fn encode(s: &str) -> String {
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

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
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
    if magic.starts_with(b"Rar!") {
        return extract_rar(archive, dest, progress);
    }
    Err("format d'archive non pris en charge (seuls .zip, .7z et .rar le sont)".into())
}

fn extract_rar(archive: &Path, dest: &Path, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    let total = fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
    let mut done = 0u64;
    let err = |e: unrar::error::UnrarError| format!("archive RAR illisible : {e}");
    let mut open = unrar::Archive::new(archive).open_for_processing().map_err(err)?;
    while let Some(header) = open.read_header().map_err(err)? {
        let entry = header.entry();
        let name = entry.filename.to_string_lossy().into_owned();
        let size = entry.unpacked_size;
        let target = (!entry.is_directory()).then(|| safe_entry_path(&name)).flatten().map(|rel| dest.join(rel));
        open = match target {
            Some(target) => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let next = header.extract_to(&target).map_err(err)?;
                done += size;
                progress(done.min(total.max(done)), total.max(done));
                next
            }
            None => header.skip().map_err(err)?,
        };
    }
    Ok(())
}

/// Fichiers (chemins relatifs, `/`, minuscules) sous `dir`.
fn files_under(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/").to_lowercase());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// Fichier GameBanana gardé en cache (`<cache>/gb-files/<id>-<nom>`), vérifié par MD5.
fn gb_download(app: &AppHandle, file: &crate::gamebanana::RawFile, emit: &dyn Fn(&'static str, u64, u64)) -> Result<PathBuf, String> {
    let dir = cache_dir(app)?.join("gb-files");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}-{}", file._idRow, folder_name(&file._sFile)));
    if path.is_file() && fs::metadata(&path).map(|m| m.len()).ok() == Some(file._nFilesize) {
        return Ok(path);
    }
    let part = path.with_extension("part");
    download_to(&file._sDownloadUrl, &part, file._nFilesize, |d, t| emit("download", d, t))?;
    if !file._sMd5Checksum.is_empty() {
        emit("verify", 0, 1);
        let sum = md5_hex(&part)?;
        if !sum.eq_ignore_ascii_case(&file._sMd5Checksum) {
            let _ = fs::remove_file(&part);
            return Err("le fichier téléchargé est abîmé (somme MD5 différente de celle de GameBanana) : réessaie".into());
        }
    }
    fs::rename(&part, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Choix du fichier d'un mod GameBanana : celui demandé, sinon le plus récent qui commence
/// par `prefix`, sinon le plus récent (hors fichiers pour Ryujinx quand il y a le choix).
fn pick_gb_file<'a>(files: &'a [crate::gamebanana::RawFile], wanted: Option<u64>, prefix: Option<&str>) -> Option<&'a crate::gamebanana::RawFile> {
    if let Some(id) = wanted {
        return files.iter().find(|f| f._idRow == id);
    }
    if let Some(p) = prefix.map(str::to_lowercase) {
        if let Some(f) = files.iter().filter(|f| f._sFile.to_lowercase().starts_with(&p)).max_by_key(|f| f._tsDateAdded) {
            return Some(f);
        }
    }
    let usable: Vec<&crate::gamebanana::RawFile> = files.iter().filter(|f| !f._sFile.to_lowercase().contains("ryujinx") && !f._sDescription.to_lowercase().contains("ryujinx only")).collect();
    usable.into_iter().max_by_key(|f| f._tsDateAdded).or_else(|| files.iter().max_by_key(|f| f._tsDateAdded))
}

/// Dossiers d'une archive décompressée qui contiennent directement `romfs` ou `exefs`
/// (variantes d'un même mod quand il y en a plusieurs), chemins relatifs.
fn mod_roots(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, depth: u8, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let subs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !is_junk(p)).collect();
        if subs.iter().any(|p| p.file_name().is_some_and(|n| matches!(n.to_string_lossy().to_ascii_lowercase().as_str(), "romfs" | "exefs"))) {
            out.push(dir.strip_prefix(base).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default());
            return;
        }
        if depth > 0 {
            for s in subs {
                walk(base, &s, depth - 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, 6, &mut out);
    out.sort();
    out
}

/// Dossiers et fichiers parasites des archives faites sur Mac.
fn is_junk(p: &Path) -> bool {
    p.file_name().is_some_and(|n| {
        let n = n.to_string_lossy();
        n == "__MACOSX" || n == ".DS_Store" || n.starts_with("._")
    })
}

/// Dossiers de premier niveau du romfs des jeux Pokémon Switch : un mod livré sans
/// dossier `romfs` (fichiers du jeu à la racine de l'archive) est reconnu grâce à eux.
const ROMFS_TOP: &[&str] = &["arc", "bin", "world", "pokemon", "appli", "field", "system_resource", "demo", "event", "ui", "effect", "sound", "font", "message", "data", "streamingassets", "ai", "chara", "script"];

/// Dossier dont le contenu est un romfs « en vrac » (au moins un dossier connu du jeu).
fn loose_romfs(dir: &Path, depth: u8) -> Option<PathBuf> {
    let subs: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !is_junk(p)).collect();
    if subs.iter().any(|p| p.file_name().is_some_and(|n| ROMFS_TOP.contains(&n.to_string_lossy().to_ascii_lowercase().as_str()))) {
        return Some(dir.to_path_buf());
    }
    if depth == 0 || subs.len() != 1 {
        return None;
    }
    loose_romfs(&subs[0], depth - 1)
}

/// Retire les fichiers parasites (`__MACOSX`, `.DS_Store`) d'un dossier de mod.
fn remove_junk(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if is_junk(&p) {
            let _ = if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
        } else if p.is_dir() {
            remove_junk(&p);
        }
    }
}

/// Variante à installer : celle demandée, celle du catalogue, celle au title ID du jeu,
/// sinon la seule. `Err(liste)` quand l'utilisateur doit choisir.
fn choose_root(roots: &[String], wanted: Option<&str>, hint: Option<&str>, tid: u64) -> Result<String, Vec<String>> {
    if let Some(w) = wanted {
        if let Some(r) = roots.iter().find(|r| r.as_str() == w) {
            return Ok(r.clone());
        }
    }
    if roots.len() == 1 {
        return Ok(roots[0].clone());
    }
    if let Some(h) = hint.map(str::to_lowercase) {
        let found: Vec<&String> = roots.iter().filter(|r| r.to_lowercase().starts_with(&h) || r.to_lowercase().contains(&h)).collect();
        if found.len() == 1 {
            return Ok(found[0].clone());
        }
    }
    let tid_hex = format!("{tid:016x}");
    let by_tid: Vec<&String> = roots.iter().filter(|r| r.to_lowercase().contains(&tid_hex)).collect();
    if by_tid.len() == 1 {
        return Ok(by_tid[0].clone());
    }
    // Mods qui visent un autre jeu de la même paire (ex. Perle) : écartés.
    let others: Vec<&String> = roots.iter().filter(|r| r.split('/').find_map(crate::switch::title_id_in_name).is_none_or(|t| crate::switch::base_title_id(t) == tid)).collect();
    if others.len() == 1 {
        return Ok(others[0].clone());
    }
    Err(roots.to_vec())
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

pub(crate) fn resolve(id: EmulatorId, app: &AppHandle) -> Resolved {
    let config = play::load_config(app);
    play::resolve(id, &config, &Env::system(&config.search_dirs))
}

/// Émulateur 3DS à utiliser : celui choisi, sinon le premier trouvé, sinon Azahar.
pub(crate) fn ctr_emulator(app: &AppHandle) -> Resolved {
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

pub(crate) fn fxhash(s: &str) -> u64 {
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

/// Clé de catalogue d'un title ID.
fn tid_key(tid: u64) -> String {
    format!("{tid:016X}")
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

/// Catégorie d'un mod installé à la main, d'après son nom.
fn other_category(name: &str, tier: Tier) -> String {
    let lower = name.to_lowercase();
    let k = classify(name, tier);
    if k.category != "other" {
        return k.category.into();
    }
    let guess = [
        ("textur", "textures"),
        ("hd", "textures"),
        ("scale", "style"),
        ("resatur", "style"),
        ("trade", "qol"),
        ("faster", "qol"),
        ("music", "audio"),
        ("sound", "audio"),
        ("button", "ui"),
        ("icon", "ui"),
        ("luminescent", "romhack"),
        ("randomizer", "gameplay"),
        ("shiny", "gameplay"),
    ];
    guess.iter().find(|(w, _)| lower.contains(w)).map_or("other", |(_, c)| c).to_string()
}

fn version_parts(v: &str) -> Vec<u32> {
    v.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.trim_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0)).collect()
}

/// Avertissement quand le mod vise une autre version du jeu que celle installée.
/// `wanted` : « 1.3.0 », « 1.1.3/1.2.0/1.3.0 », « 4.0.0+ ».
pub fn version_warning(wanted: Option<&str>, have: Option<&str>) -> Option<String> {
    let (w, h) = (wanted?.trim(), have?.trim());
    if w.is_empty() || h.is_empty() || !h.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let ok = w.split(['/', ',', ' ']).filter(|x| !x.is_empty() && x.chars().any(|c| c.is_ascii_digit())).any(|x| match x.strip_suffix('+') {
        Some(min) => version_parts(h) >= version_parts(min),
        None => version_parts(x) == version_parts(h),
    });
    (!ok && w.chars().any(|c| c.is_ascii_digit())).then(|| format!("Prévu pour la version {w} du jeu, et tu as la {h} : il peut ne pas marcher."))
}

/// Pour chaque mod actif, les autres mods actifs qui remplacent des fichiers du jeu en commun.
fn overlap_map(dirs: &[(String, Vec<String>)]) -> BTreeMap<String, Vec<String>> {
    let mut owners: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
    for (i, (_, files)) in dirs.iter().enumerate() {
        for f in files {
            owners.entry(f.as_str()).or_default().push(i);
        }
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for list in owners.values().filter(|l| l.len() > 1) {
        for &i in list {
            let names = out.entry(dirs[i].0.clone()).or_default();
            for &j in list {
                if j != i && !names.contains(&dirs[j].0) {
                    names.push(dirs[j].0.clone());
                }
            }
        }
    }
    out
}

/// Fichiers du romfs d'un dossier de mod (les correctifs d'exefs se cumulent sans conflit ;
/// l'index Trinity `arc/data.trpfd` est refait par Kaleido).
fn romfs_files(dir: &Path) -> Vec<String> {
    files_under(dir).into_iter().filter(|f| f.starts_with("romfs/") && !f.starts_with("romfs/arc/data.trpfd")).collect()
}

// ---------------------------------------------------------------------------
// Moteur Trinity (Écarlate / Violet, Légendes Z-A) : index `data.trpfd` fusionné

/// Jeux dont les fichiers en vrac ne sont lus que s'ils sont retirés de `arc/data.trpfd`.
const TRINITY: &[u64] = &[0x0100A3D008C5C000, 0x01008F6008C5E000, 0x0100F43008C44000];

/// Mods « Trinity Bypass » du catalogue : inutiles avec l'index fusionné.
const TRINITY_BYPASS: &[&str] = &["gb:640721", "gb:631935"];

const TRINITY_INDEX_ID: &str = "kaleido:trinity";
const TRINITY_INDEX_DIR: &str = "Kaleido - index Trinity";
/// Index apporté par un mod, mis de côté : celui de Kaleido le remplace.
const TRPFD_OFF: &str = "data.trpfd.kaleido-off";

pub fn is_trinity(tid: u64) -> bool {
    TRINITY.contains(&tid)
}

/// Dossier `romfs` d'un dossier de mod (casse quelconque).
fn romfs_dir(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("romfs")))
}

/// Index d'origine de la version installée (gardé en cache par build ID de l'exécutable).
fn vanilla_trpfd(app: &AppHandle, target: &ModTarget) -> Result<Vec<u8>, String> {
    let keys_path = crate::switch::prod_keys(app).ok_or("clés de la console (prod.keys) introuvables")?;
    let keys = kaleido_core::nx::Keys::load(&keys_path).map_err(|e| e.to_string())?;
    let base = target.rom.as_deref().ok_or("fichier du jeu inconnu")?;
    let exe = target.update_file.as_deref().unwrap_or(base);
    let id = file_build_id(Path::new(exe), &keys)?;
    let cache = cache_dir(app)?.join("trinity");
    let file = cache.join(format!("{id}.trpfd"));
    if let Ok(data) = fs::read(&file) {
        return Ok(data);
    }
    let data = kaleido_core::nx::read_game_file(Path::new(base), target.update_file.as_deref().map(Path::new), &keys, "arc/data.trpfd").map_err(|e| e.to_string())?;
    let _ = fs::create_dir_all(&cache);
    let _ = fs::write(&file, &data);
    Ok(data)
}

/// Refait l'index Trinity d'un jeu à partir des mods actifs. Rend une phrase pour la vue.
/// En cas d'échec, chaque mod retrouve son propre index (comportement d'avant Kaleido).
fn refresh_trinity(app: &AppHandle, target: &ModTarget, tid: u64) -> Result<Option<String>, String> {
    if !is_trinity(tid) {
        return Ok(None);
    }
    let r = resolve(EmulatorId::Eden, app);
    let game_dir = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?.join(format!("{tid:016X}"));
    let index_dir = game_dir.join(TRINITY_INDEX_DIR);
    let arcs: Vec<(String, PathBuf, Vec<String>)> = installed_dirs(&game_dir)
        .into_iter()
        .filter(|(_, m)| !m.as_ref().is_some_and(|m| m.id == TRINITY_INDEX_ID))
        .filter_map(|(name, _)| {
            let romfs = romfs_dir(&game_dir.join(&name))?;
            let files: Vec<String> = files_rel(&romfs).into_iter().filter(|f| !f.to_ascii_lowercase().starts_with("arc/data.trpfd")).collect();
            Some((name, romfs.join("arc"), files))
        })
        .collect();
    let built = build_trinity_index(app, target, &arcs);
    match built {
        Ok(None) => {
            restore_mod_indexes(&arcs);
            if index_dir.exists() {
                fs::remove_dir_all(&index_dir).map_err(|e| e.to_string())?;
            }
            Ok(None)
        }
        Ok(Some((bytes, mods, files, source))) => {
            let arc = index_dir.join("romfs").join("arc");
            fs::create_dir_all(&arc).map_err(|e| e.to_string())?;
            fs::write(arc.join("data.trpfd"), bytes).map_err(|e| e.to_string())?;
            write_marker(&index_dir, &Marker { id: TRINITY_INDEX_ID.into(), version: Some(format!("{mods} mods, {files} fichiers")), source: source.into(), name: Some("Index Trinity".into()), ..Default::default() })?;
            // L'index est en place : ceux des mods sont mis de côté.
            for (name, arc, _) in &arcs {
                let own = arc.join("data.trpfd");
                if own.is_file() {
                    let off = arc.join(TRPFD_OFF);
                    let _ = fs::remove_file(&off);
                    fs::rename(&own, &off).map_err(|e| format!("index de « {name} » impossible à mettre de côté : {e}"))?;
                }
            }
            let s = |n: usize| if n > 1 { "s" } else { "" };
            Ok(Some(format!("Index Trinity refait d'après {source} : {mods} mod{} cumulé{}, {files} fichier{} lu{} en vrac.", s(mods), s(mods), s(files), s(files))))
        }
        Err(e) => {
            restore_mod_indexes(&arcs);
            if index_dir.exists() {
                let _ = fs::remove_dir_all(&index_dir);
            }
            Err(e)
        }
    }
}

/// Remet l'index propre à chaque mod (mis de côté par une fusion précédente).
fn restore_mod_indexes(arcs: &[(String, PathBuf, Vec<String>)]) {
    for (_, arc, _) in arcs {
        let off = arc.join(TRPFD_OFF);
        if off.is_file() && !arc.join("data.trpfd").exists() {
            let _ = fs::rename(&off, arc.join("data.trpfd"));
        }
    }
}

/// Index fusionné : (octets, mods cumulés, fichiers en vrac, source), ou `None` sans fichier de mod.
fn build_trinity_index(app: &AppHandle, target: &ModTarget, arcs: &[(String, PathBuf, Vec<String>)]) -> Result<Option<(Vec<u8>, usize, usize, &'static str)>, String> {
    use kaleido_core::trinity::{path_hash, FileDescriptor};
    let hashes: std::collections::HashSet<u64> = arcs.iter().flat_map(|(_, _, files)| files.iter().map(|f| path_hash(f))).collect();
    let mods = arcs.iter().filter(|(_, _, files)| !files.is_empty()).count();
    if hashes.is_empty() {
        return Ok(None);
    }
    let (mut index, source) = match vanilla_trpfd(app, target) {
        Ok(data) => (FileDescriptor::parse(&data).map_err(|e| e.to_string())?, "ton jeu"),
        Err(e) => {
            // Secours : l'union des index apportés par les mods, s'ils viennent exactement de la
            // même version du jeu (chacun n'en retire que ses propres fichiers).
            let parsed: Vec<FileDescriptor> = arcs
                .iter()
                .filter_map(|(_, arc, _)| fs::read(arc.join("data.trpfd")).or_else(|_| fs::read(arc.join(TRPFD_OFF))).ok())
                .filter_map(|d| FileDescriptor::parse(&d).ok())
                .collect();
            let same = parsed.windows(2).all(|w| w[0].pack_names == w[1].pack_names && w[0].packs == w[1].packs);
            let Some(first) = parsed.first().filter(|_| same) else {
                return Err(format!("l'index d'origine du jeu est illisible ({e}), et ceux des mods viennent de versions différentes : ces mods ne peuvent pas être cumulés"));
            };
            let mut all: Vec<(u64, kaleido_core::trinity::FileInfo)> = parsed.iter().flat_map(|d| d.hashes.iter().copied().zip(d.files.iter().copied())).collect();
            all.sort_by_key(|(h, _)| *h);
            all.dedup_by_key(|(h, _)| *h);
            let mut d = first.clone();
            d.hashes = all.iter().map(|(h, _)| *h).collect();
            d.files = all.iter().map(|(_, f)| *f).collect();
            (d, "les index des mods")
        }
    };
    index.remove(&hashes);
    Ok(Some((index.to_bytes(), mods, hashes.len(), source)))
}

/// Après un changement de mods : refait l'index Trinity si besoin, et le signale.
fn after_change(app: &AppHandle, target: &ModTarget, view: &mut ModsView) {
    if target.platform != "switch" {
        return;
    }
    let Some(tid) = parse_tid(target.title_id.as_deref()).ok().map(base_title_id) else { return };
    match refresh_trinity(app, target, tid) {
        Ok(Some(note)) => view.notes.insert(0, note),
        Ok(None) => {}
        Err(e) => view.errors.push(format!("Index Trinity : {e}")),
    }
}

/// Profils GameBanana de plusieurs mods, récupérés en parallèle.
fn gb_profiles(app: &AppHandle, ids: &[u32]) -> BTreeMap<u32, Result<(crate::gamebanana::GbProfile, Vec<crate::gamebanana::RawFile>), String>> {
    std::thread::scope(|s| {
        let handles: Vec<_> = ids.iter().map(|&id| (id, s.spawn(move || crate::gamebanana::profile_raw(app, id)))).collect();
        handles.into_iter().map(|(id, h)| (id, h.join().unwrap_or_else(|_| Err("erreur interne".into())))).collect()
    })
}

/// Entrées du catalogue d'un jeu (fiches GameBanana comprises).
fn catalog_entries(app: &AppHandle, key: &str, target: &ModTarget, mine: &dyn Fn(&str) -> Option<(String, Marker, bool)>, view: &mut ModsView) -> Vec<ModEntry> {
    let Some(catalog) = crate::mods_catalog::for_game(app, key) else { return vec![] };
    let ids: Vec<u32> = catalog.mods.iter().filter(|m| m.source == "gamebanana").filter_map(|m| m.gb).collect();
    let profiles = gb_profiles(app, &ids);
    let nexus_ids: Vec<(String, u32)> = catalog.mods.iter().filter_map(|m| crate::nexus::parse_id(&m.id)).collect();
    let nexus = crate::nexus::mods(app, &nexus_ids);
    let mut failed = 0;
    let mut out = Vec::new();
    for m in &catalog.mods {
        let current = mine(&m.id);
        let profile = m.gb.and_then(|id| profiles.get(&id)).and_then(|p| p.as_ref().ok());
        let nx = crate::nexus::parse_id(&m.id).and_then(|k| nexus.get(&k));
        if m.source == "gamebanana" && profile.is_none() {
            failed += 1;
        }
        let file = profile.and_then(|(_, files)| pick_gb_file(files, None, m.file.as_deref()));
        let requires: Vec<String> = m.requires.iter().filter(|r| mine(r).is_none_or(|(_, _, on)| !on)).filter_map(|r| catalog.mods.iter().find(|o| &o.id == r)).map(|o| o.name.clone()).collect();
        // L'écart de version est montré discrètement par l'interface : un mod de romfs fait sur
        // une version antérieure marche le plus souvent. Les correctifs ExeFS, eux, sont vérifiés.
        let _ = target;
        let warning = m.warning.clone().unwrap_or_default();
        out.push(ModEntry {
            id: m.id.clone(),
            name: m.name.clone(),
            description: m.description.clone(),
            category: m.category.clone(),
            group: m.group.clone(),
            recommended: m.recommended,
            source: m.source.clone(),
            author: m.author.clone().or_else(|| profile.map(|(p, _)| p.author.clone())).unwrap_or_default(),
            page: m.page.clone().or_else(|| m.gb.map(|id| format!("https://gamebanana.com/mods/{id}"))).unwrap_or_default(),
            size: file.map(|f| f._nFilesize),
            game_version: m.game_version.clone(),
            installed: current.is_some(),
            enabled: current.as_ref().is_some_and(|c| c.2),
            update_available: match (&current, file) {
                (Some((_, marker, _)), Some(f)) => marker.version.as_ref().is_some_and(|v| *v != f._sFile),
                _ => false,
            },
            requires,
            requires_ids: m.requires.clone(),
            exclusive: m.exclusive,
            warning: (!warning.is_empty()).then_some(warning),
            thumbnail: profile.and_then(|(p, _)| p.thumbnail.clone()).or_else(|| nx.and_then(|n| n.thumbnail_url.clone())),
            gb: m.gb,
            popularity: nx.and_then(|n| n.downloads).or(profile.map(|(p, _)| p.views)).or(m.popularity),
            kind: m.kind.clone(),
            ..Default::default()
        });
    }
    if failed > 0 {
        view.errors.push(format!("GameBanana ne répond pas : {failed} fiche{} de mod sans aperçu (l'installation reste possible plus tard).", if failed > 1 { "s" } else { "" }));
    }
    out
}

/// Entrée d'un mod installé par Kaleido hors catalogue (explorateur, fichier).
fn entry_from_marker(folder: &str, m: &Marker, enabled: bool) -> ModEntry {
    let from_gb = m.gb.is_some();
    ModEntry {
        id: m.id.clone(),
        name: m.name.clone().unwrap_or_else(|| folder.to_string()),
        description: if from_gb { "Installé depuis l'explorateur GameBanana.".into() } else { "Installé depuis un fichier téléchargé.".into() },
        category: m.category.clone().unwrap_or_else(|| "other".into()),
        source: if from_gb { "explorer".into() } else { "local".into() },
        page: m.page.clone().or_else(|| m.gb.map(|id| format!("https://gamebanana.com/mods/{id}"))).unwrap_or_default(),
        game_version: None,
        installed: true,
        enabled,
        gb: m.gb,
        ..Default::default()
    }
}

/// Conflits : même groupe déjà en place, ou mod qui doit rester seul.
fn resolve_conflicts(entries: &mut [ModEntry], others: &[(String, Kind)], active_folders: &[String]) {
    let installed_groups: Vec<(String, String)> = entries
        .iter()
        .filter(|e| e.installed && e.enabled)
        .filter_map(|e| e.group.clone().map(|g| (e.name.clone(), g)))
        .chain(others.iter().filter_map(|(n, k)| k.group.map(|g| (n.clone(), g.to_string()))))
        .collect();
    let exclusive: Vec<(String, String)> = entries.iter().filter(|e| e.exclusive && e.installed && e.enabled).map(|e| (e.id.clone(), e.name.clone())).collect();
    // Mods qui dépendent d'un mod exclusif (ses extensions) : compatibles avec lui.
    let entries_requiring: Vec<String> = entries.iter().filter(|e| !e.requires_ids.is_empty()).map(|e| e.name.clone()).collect();
    let active_names: Vec<String> = entries.iter().filter(|e| e.installed && e.enabled).map(|e| e.name.clone()).chain(active_folders.iter().cloned()).collect();
    for e in entries.iter_mut() {
        if !e.installed {
            if let Some(g) = &e.group {
                e.conflicts = installed_groups.iter().filter(|(_, ig)| ig == g).map(|(n, _)| n.clone()).collect();
            }
            if e.exclusive {
                e.conflicts = active_names.iter().filter(|n| !entries_requiring.contains(n)).cloned().collect();
            }
        }
        for (ex_id, name) in &exclusive {
            if &e.name == name || e.requires_ids.contains(ex_id) {
                continue;
            }
            if e.installed && e.enabled {
                let w = format!("{name} doit rester seul : ce mod risque de ne pas marcher avec.");
                e.warning = Some(e.warning.take().map_or(w.clone(), |old| format!("{old} {w}")));
            }
        }
    }
}

/// Build IDs des exécutables d'un fichier Switch : (title ID du contenu, build ID), gardés
/// en mémoire par chemin, taille et date.
fn file_build_ids(path: &Path, keys: &kaleido_core::nx::Keys) -> Result<Vec<(u64, String)>, String> {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<BTreeMap<(PathBuf, u64, u64), Vec<(u64, String)>>>> = OnceLock::new();
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let stamp = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    let key = (path.to_path_buf(), meta.len(), stamp);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(ids) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(ids.clone());
    }
    let ids: Vec<(u64, String)> = kaleido_core::nx::program_build_ids(path, keys).map_err(|e| e.to_string())?.into_iter().map(|(t, b)| (t, kaleido_core::nx::build_id_hex(&b))).collect();
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, ids.clone());
    Ok(ids)
}

/// Build ID de l'exécutable principal d'un fichier (le jeu de base s'il y en a un).
fn file_build_id(path: &Path, keys: &kaleido_core::nx::Keys) -> Result<String, String> {
    let ids = file_build_ids(path, keys)?;
    ids.iter().find(|(t, _)| t & 0xFFF == 0).or(ids.first()).map(|(_, id)| id.clone()).ok_or_else(|| "aucun exécutable".to_string())
}

/// Build ID de l'exécutable lancé par Eden (la mise à jour si présente) et du jeu de base.
struct ExeIds {
    running: String,
    base: Option<String>,
    has_update: bool,
    /// Mise à jour contenue dans le fichier du jeu lui-même (.xci « + Expansion Pass »…) :
    /// c'est elle qu'Eden lance, même quand une mise à jour plus récente est à côté.
    embedded: Option<String>,
    /// Mise à jour trouvée à côté, en fichier séparé.
    separate: Option<String>,
    /// Mise à jour installée dans la NAND d'Eden : (build ID, numéro de version).
    nand: Option<(String, u32)>,
}

/// Dossiers `nand/user/Contents/registered` et `keys` d'Eden.
fn eden_nand(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    let user = resolve(EmulatorId::Eden, app).ctr_user_dir()?;
    let configured = fs::read_to_string(user.join("config").join("qt-config.ini"))
        .ok()
        .and_then(|t| tuning::get_qt(&t, "Data%20Storage", "nand_directory"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let nand = configured.unwrap_or_else(|| user.join("nand"));
    Some((nand.join("user").join("Contents").join("registered"), user.join("keys")))
}

fn exe_ids(app: &AppHandle, target: &ModTarget) -> Result<ExeIds, String> {
    let keys_path = crate::switch::prod_keys(app).ok_or("clés de la console (prod.keys) introuvables")?;
    let keys = kaleido_core::nx::Keys::load(&keys_path).map_err(|e| e.to_string())?;
    let in_game = match target.rom.as_deref() {
        Some(p) => file_build_ids(Path::new(p), &keys).unwrap_or_default(),
        None => vec![],
    };
    let base = in_game.iter().find(|(t, _)| t & 0xFFF == 0).map(|(_, id)| id.clone());
    let embedded = in_game.iter().find(|(t, _)| t & 0xFFF == 0x800).map(|(_, id)| id.clone());
    let separate = match target.update_file.as_deref() {
        Some(u) => Some(file_build_id(Path::new(u), &keys)?),
        None => None,
    };
    let base_tid = parse_tid(target.title_id.as_deref()).map(base_title_id).ok();
    let nand = eden_nand(app)
        .zip(base_tid)
        .and_then(|((reg, keys_dir), tid)| kaleido_core::nx::nand_update(&reg, &keys_dir, &keys, tid))
        .map(|(b, v)| (kaleido_core::nx::build_id_hex(&b), v));
    // Constaté chez l'utilisateur : une mise à jour installée dans la NAND passe avant celle
    // contenue dans le .xci, qui passe elle-même avant le fichier séparé.
    let running = nand.as_ref().map(|(b, _)| b.clone()).or(embedded.clone()).or(separate.clone()).or(base.clone()).ok_or("fichier du jeu inconnu")?;
    Ok(ExeIds { running, base, has_update: embedded.is_some() || separate.is_some() || nand.is_some(), embedded, separate, nand })
}

/// Verdict des correctifs ExeFS d'un mod (build IDs visés) face à l'exécutable lancé.
pub fn exefs_verdict(patches: &[String], running: &str, base: Option<&str>) -> Option<&'static str> {
    use kaleido_core::nx::build_id_matches;
    if patches.is_empty() {
        return None;
    }
    if patches.iter().any(|p| build_id_matches(p, running)) {
        return Some("ok");
    }
    if base.is_some_and(|b| patches.iter().any(|p| build_id_matches(p, b))) {
        return Some("base");
    }
    Some("other")
}

/// Build IDs visés par les correctifs `exefs/` d'un dossier de mod.
fn dir_patch_ids(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let Some(exefs) = entries.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("exefs"))) else { return vec![] };
    let Ok(files) = fs::read_dir(exefs) else { return vec![] };
    files
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let content = if name.to_ascii_lowercase().ends_with(".pchtxt") { fs::read(e.path()).unwrap_or_default() } else { Vec::new() };
            kaleido_core::nx::patch_build_id(&name, &content)
        })
        .collect()
}

/// Build IDs visés par les correctifs d'une archive Fl4sh : (dossier qui contient `exefs/`, build ID).
fn zip_patch_ids(zip_path: &Path) -> Vec<(String, String)> {
    let Ok(file) = fs::File::open(zip_path) else { return vec![] };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return vec![] };
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else { continue };
        let path = entry.name().replace('\\', "/");
        let lower = path.to_ascii_lowercase();
        let Some(pos) = lower.rfind("/exefs/") else { continue };
        let name = path[pos + 7..].to_string();
        if name.contains('/') || name.is_empty() {
            continue;
        }
        let mut content = Vec::new();
        if lower.ends_with(".pchtxt") {
            let _ = (&mut entry).take(1 << 20).read_to_end(&mut content);
        }
        if let Some(id) = kaleido_core::nx::patch_build_id(&name, &content) {
            out.push((path[..pos + 1].to_string(), id));
        }
    }
    out
}

/// Phrase d'avertissement pour un verdict ExeFS.
fn exefs_warning(verdict: &str, exe: &Executable) -> Option<String> {
    match verdict {
        "base" => Some(format!("Ce correctif vise le jeu sans mise à jour, alors qu'Eden lance {} : il ne s'appliquera pas.", exe.source)),
        "other" => Some(format!("Ce correctif vise une autre version du jeu que {} : Eden l'ignorera sans rien dire.", exe.source)),
        "shadowed" => Some(format!("Ce correctif vise ta mise à jour séparée, mais Eden lance {} : il ne s'appliquera pas tant que la mise à jour séparée n'est pas installée dans Eden (bouton en haut de la sélection).", exe.source)),
        _ => None,
    }
}

fn switch_view(app: &AppHandle, target: &ModTarget, tid: u64) -> ModsView {
    let r = resolve(EmulatorId::Eden, app);
    let mut view = ModsView { emulator: Some("Eden"), emulator_found: r.exe.is_some(), gamebanana: crate::gamebanana::game_of(tid), can_import: true, ..Default::default() };
    let Some(load) = eden_load_dir(&r) else {
        view.errors.push("Dossier d'Eden introuvable.".into());
        return view;
    };
    let game_dir = load.join(format!("{tid:016X}"));
    view.location = Some(game_dir.display().to_string());
    let ids = match exe_ids(app, target) {
        Ok(ids) => Some(ids),
        Err(e) => {
            view.notes.push(format!("Kaleido n'a pas pu vérifier si les correctifs 60 FPS conviennent à ta version du jeu ({e})."));
            None
        }
    };
    if let Some(ids) = &ids {
        let source = if let Some((nand_id, _)) = &ids.nand {
            match target.game_version.as_deref().filter(|_| ids.separate.as_ref() == Some(nand_id)) {
                Some(v) => format!("la mise à jour {v} installée dans Eden"),
                None => "la mise à jour installée dans Eden".to_string(),
            }
        } else if ids.embedded.is_some() {
            "la mise à jour intégrée à ton fichier de jeu".to_string()
        } else {
            match (ids.has_update, target.game_version.as_deref()) {
                (true, Some(v)) => format!("la mise à jour {v}"),
                (true, None) => "la mise à jour installée".to_string(),
                (false, _) => "le jeu de base".to_string(),
            }
        };
        view.executable = Some(Executable { source, build_id: ids.running.clone() });
        let separate_shadowed = ids.separate.is_some() && ids.separate.as_deref() != Some(ids.running.as_str()) && (ids.embedded.is_some() || ids.nand.is_some());
        if separate_shadowed {
            view.install_update = target.update_file.as_deref().and_then(|u| Path::new(u).file_name()).map(|n| n.to_string_lossy().into_owned());
        }
        if separate_shadowed && ids.nand.is_none() {
            let file = target.update_file.as_deref().and_then(|u| Path::new(u).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            view.notes.insert(0, format!("Ton fichier de jeu contient sa propre mise à jour, différente de « {file} » : Eden lance celle du fichier de jeu, et les correctifs prévus pour l'autre ne s'appliquent pas. Kaleido peut l'installer dans Eden pour qu'il la lance (bouton ci-dessous)."));
        }
    }
    let verdict = |patches: &[String]| {
        let i = ids.as_ref()?;
        let v = exefs_verdict(patches, &i.running, i.base.as_deref())?;
        // Bon pour la mise à jour séparée, mais Eden lance celle du fichier de jeu.
        if v != "ok" && (i.embedded.is_some() || i.nand.is_some()) && i.separate.as_deref().is_some_and(|s| patches.iter().any(|p| kaleido_core::nx::build_id_matches(p, s))) {
            return Some("shadowed".to_string());
        }
        Some(v.to_string())
    };
    let tier = tuning::current_tier();
    let installed = installed_dirs(&game_dir);
    let disabled = disabled_dir(app, tid).map(|d| installed_dirs(&d)).unwrap_or_default();
    let display = |n: &String, m: &Option<Marker>| m.as_ref().and_then(|m| m.name.clone()).unwrap_or_else(|| n.clone());

    // Fichiers remplacés en commun par les mods actifs.
    let active: Vec<(String, Vec<String>)> = installed.iter().map(|(n, m)| (display(n, m), romfs_files(&game_dir.join(n)))).collect();
    let overlaps = overlap_map(&active);

    // Mods présents qui ne viennent pas de Kaleido (ou mis de côté par Kaleido).
    for (name, marker) in &installed {
        if marker.is_none() && name != TRINITY_INDEX_DIR {
            view.others.push(OtherMod { name: name.clone(), enabled: true, category: other_category(name, tier), overlaps: overlaps.get(name).cloned().unwrap_or_default(), can_toggle: true, exefs: verdict(&dir_patch_ids(&game_dir.join(name))), from_kaleido: false });
        }
    }
    for (name, marker) in &disabled {
        if marker.is_none() {
            view.others.push(OtherMod { name: name.clone(), enabled: false, category: other_category(name, tier), overlaps: vec![], can_toggle: true, exefs: None, from_kaleido: false });
        }
    }
    let active_others: Vec<(String, Kind)> = installed.iter().filter(|(_, m)| m.is_none()).map(|(n, _)| (n.clone(), classify(n, tier))).collect();
    let mine = |id: &str| -> Option<(String, Marker, bool)> {
        installed
            .iter()
            .map(|(n, m)| (n, m, true))
            .chain(disabled.iter().map(|(n, m)| (n, m, false)))
            .find_map(|(n, m, on)| m.as_ref().filter(|m| m.id == id).map(|m| (n.clone(), m.clone(), on)))
    };

    // Fl4sh.
    let mut entries: Vec<ModEntry> = Vec::new();
    match fl4sh_archive(app, tid) {
        Some((archive, sha)) => match fl4sh_zip(app, &archive, &sha).and_then(|z| zip_listing(&z).map(|l| (z, l))) {
            Ok((zip_path, listing)) => {
                let mods = zip_mods(&listing, tid);
                let zip_ids = if ids.is_some() { zip_patch_ids(&zip_path) } else { vec![] };
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
                    // Sans exécutable lisible, l'écart de version reste le seul indice pour un 60 FPS.
                    let mismatch = if ids.is_none() { version_warning(version.as_deref(), target.game_version.as_deref()) } else { None };
                    let warning = [k.warning, mismatch].into_iter().flatten().collect::<Vec<_>>().join(" ");
                    entries.push(ModEntry {
                        installed: current.is_some(),
                        enabled: current.as_ref().is_some_and(|c| c.2),
                        update_available: current.as_ref().is_some_and(|(_, mk, _)| mk.version != version),
                        id,
                        name: k.name,
                        description: k.description,
                        category: k.category.into(),
                        group: k.group.map(str::to_string),
                        recommended,
                        source: "auto".into(),
                        author: "Fl4sh9174".into(),
                        page: format!("https://github.com/{FL4SH_REPO}"),
                        size: Some(m.size),
                        game_version: version,
                        warning: (!warning.is_empty()).then_some(warning),
                        exefs: verdict(&zip_ids.iter().filter(|(root, _)| root.starts_with(&m.root)).map(|(_, id)| id.clone()).collect::<Vec<_>>()),
                        ..Default::default()
                    });
                }
            }
            Err(e) => view.errors.push(format!("Mods de Fl4sh indisponibles : {e}")),
        },
        None => view.notes.push("Fl4sh ne propose pas encore de mods FPS pour ce jeu.".into()),
    }

    // Catalogue.
    let mut catalog = catalog_entries(app, &tid_key(tid), target, &mine, &mut view);
    // Jeux Trinity : l'index fusionné remplace le groupe « trpfd » et le Trinity Bypass, à
    // condition de pouvoir lire l'index d'origine dans le jeu (fichier connu et clés présentes).
    if is_trinity(tid) && target.rom.is_some() && crate::switch::prod_keys(app).is_some() {
        for e in catalog.iter_mut() {
            if e.group.as_deref() == Some("trpfd") {
                e.group = None;
            }
            e.requires.retain(|r| !r.contains("Bypass"));
            e.requires_ids.retain(|r| !TRINITY_BYPASS.contains(&r.as_str()));
            if TRINITY_BYPASS.contains(&e.id.as_str()) {
                e.recommended = false;
                let w = "Inutile avec Kaleido : il refait lui-même l'index des fichiers (data.trpfd) pour cumuler les mods.".to_string();
                e.warning = Some(e.warning.take().map_or(w.clone(), |old| format!("{w} {old}")));
            }
        }
    }
    // Un mod du catalogue du même groupe qu'un mod Fl4sh (ex. 60 FPS) n'est pas recommandé deux fois.
    let fl4sh_groups: Vec<String> = entries.iter().filter(|e| e.recommended).filter_map(|e| e.group.clone()).collect();
    for mut e in catalog {
        if e.recommended && e.group.as_ref().is_some_and(|g| fl4sh_groups.contains(g)) {
            e.recommended = false;
        }
        entries.push(e);
    }

    // Mods installés par Kaleido hors catalogue (explorateur, fichier).
    for (folder, marker, on) in installed.iter().map(|(n, m)| (n, m, true)).chain(disabled.iter().map(|(n, m)| (n, m, false))) {
        if let Some(m) = marker.as_ref().filter(|m| !m.id.starts_with("kaleido:")) {
            if !entries.iter().any(|e| e.id == m.id) {
                entries.push(entry_from_marker(folder, m, on));
            }
        }
    }

    // Mises à jour : mods installés depuis l'explorateur (fichier plus récent sur GameBanana)
    // et mods Nexus (version plus récente), et retour possible à la version d'avant.
    let explorer: Vec<u32> = entries.iter().filter(|e| e.installed && e.source == "explorer").filter_map(|e| e.gb).collect();
    let profiles = gb_profiles(app, &explorer);
    let nexus_keys: Vec<(String, u32)> = entries.iter().filter(|e| e.installed).filter_map(|e| crate::nexus::parse_id(&e.id)).collect();
    let nexus = crate::nexus::mods(app, &nexus_keys);
    for e in entries.iter_mut().filter(|e| e.installed) {
        let Some((folder, marker, _)) = mine(&e.id) else { continue };
        e.folder = Some(folder);
        e.previous_file = marker.previous_file;
        if e.source == "explorer" {
            if let Some(Ok((_, files))) = e.gb.and_then(|g| profiles.get(&g)) {
                let newest = pick_gb_file(files, None, None);
                e.update_available = newest.is_some_and(|f| marker.version.as_deref() != Some(f._sFile.as_str()) && marker.file != Some(f._idRow));
            }
        }
        if let (Some(have), Some(now)) = (marker.nexus_version.as_deref(), crate::nexus::parse_id(&e.id).and_then(|k| nexus.get(&k)).and_then(|m| m.version.as_deref())) {
            if have != now {
                e.update_available = true;
                let w = format!("Nouvelle version sur Nexus Mods ({now}, tu as la {have}) : télécharge-la, Kaleido la mettra à la place.");
                e.warning = Some(e.warning.take().map_or(w.clone(), |old| format!("{w} {old}")));
            }
        }
    }

    let other_names: Vec<String> = installed.iter().filter(|(_, m)| m.is_none()).map(|(n, _)| n.clone()).collect();
    resolve_conflicts(&mut entries, &active_others, &other_names);
    for e in entries.iter_mut().filter(|e| e.installed && e.enabled) {
        e.overlaps = overlaps.get(&e.name).cloned().unwrap_or_default();
    }
    // Correctifs ExeFS des mods installés : vérifiés sur les fichiers en place.
    if let Ok(parked) = disabled_dir(app, tid) {
        for e in entries.iter_mut().filter(|e| e.installed) {
            if let Some((folder, _, on)) = mine(&e.id) {
                let dir = if on { game_dir.join(&folder) } else { parked.join(&folder) };
                if let Some(v) = verdict(&dir_patch_ids(&dir)) {
                    e.exefs = Some(v);
                }
            }
        }
    }
    if let Some(exe) = &view.executable {
        for e in entries.iter_mut() {
            if let Some(w) = e.exefs.as_deref().and_then(|v| exefs_warning(v, exe)) {
                e.warning = Some(e.warning.take().map_or(w.clone(), |old| format!("{w} {old}")));
            }
        }
    }
    view.installed_gb = entries.iter().filter(|e| e.installed).filter_map(|e| e.gb).collect();
    if entries.iter().any(|e| e.category == "fps") && view.executable.is_none() {
        view.notes.push("Les mods 60 FPS ne s'appliquent qu'à la version du jeu indiquée : installe la dernière mise à jour du jeu dans Eden.".into());
    }
    view.mods = entries;
    view
}

fn write_marker(dir: &Path, marker: &Marker) -> Result<(), String> {
    fs::write(dir.join(MARKER), serde_json::to_vec_pretty(marker).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Mod prêt à être mis en place : dossier qui contient `romfs` / `exefs`, et son marqueur.
struct Prepared {
    root: PathBuf,
    marker: Marker,
}

/// Télécharge (ou prend le fichier donné), décompresse et repère le dossier du mod.
/// `Ok(Err(variantes))` : l'archive contient plusieurs variantes, à choisir.
#[allow(clippy::too_many_arguments)]
fn prepare_archive(
    app: &AppHandle,
    key: &str,
    tid: u64,
    id: &str,
    opts: &InstallOptions,
    work: &Path,
    emit: &dyn Fn(&'static str, u64, u64),
    find_roots: &dyn Fn(&Path) -> Vec<String>,
) -> Result<Result<Prepared, Vec<String>>, String> {
    let catalog = crate::mods_catalog::find(app, key, id);
    let out = work.join("x");
    let mut marker = Marker { id: id.to_string(), ..Default::default() };
    if let Some(local) = &opts.local {
        let path = PathBuf::from(local);
        emit("extract", 0, 1);
        if path.is_dir() {
            copy_dir(&path, &out).map_err(|e| e.to_string())?;
        } else {
            extract_any(&path, &out, |d, t| emit("extract", d, t))?;
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Mod".into());
        if marker.id.is_empty() {
            marker.id = format!("file:{:016x}", fxhash(&stem.to_lowercase()));
        }
        marker.name = Some(catalog.as_ref().map(|c| c.name.clone()).unwrap_or(stem.clone()));
        marker.version = Some(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(stem));
        marker.source = if catalog.is_some() { "catalog".into() } else { "local".into() };
        marker.page = catalog.as_ref().and_then(|c| c.page.clone());
        if let Some(key) = crate::nexus::parse_id(id) {
            marker.nexus_version = crate::nexus::mods(app, &[key.clone()]).get(&key).and_then(|m| m.version.clone());
        }
    } else if let Some(gb) = catalog.as_ref().and_then(|c| c.gb).or_else(|| id.strip_prefix("gb:").and_then(|n| n.split(':').next()?.parse::<u32>().ok())) {
        let (profile, files) = crate::gamebanana::profile_raw(app, gb)?;
        let file = pick_gb_file(&files, opts.file, catalog.as_ref().and_then(|c| c.file.as_deref())).ok_or("ce mod n'a pas de fichier téléchargeable")?;
        let archive = gb_download(app, file, emit)?;
        emit("extract", 0, 1);
        extract_any(&archive, &out, |d, t| emit("extract", d, t))?;
        marker.name = Some(catalog.as_ref().map(|c| c.name.clone()).unwrap_or(profile.name.clone()));
        marker.version = Some(file._sFile.clone());
        marker.source = "GameBanana".into();
        marker.gb = Some(gb);
        marker.file = Some(file._idRow);
        marker.category = Some(catalog.as_ref().map(|c| c.category.clone()).unwrap_or_else(|| category_from_gb(&profile.category, &profile.name)));
    } else {
        return Err("ce mod se télécharge à la main : choisis le fichier téléchargé".into());
    }
    if marker.category.is_none() {
        marker.category = catalog.as_ref().map(|c| c.category.clone());
    }
    remove_junk(&out);
    let mut roots = find_roots(&out);
    if roots.is_empty() {
        // Fichiers du jeu à la racine : rangés dans un dossier `romfs`.
        if let Some(loose) = loose_romfs(&out, 3) {
            let wrapped = work.join("w");
            move_dir(&loose, &wrapped.join("romfs"))?;
            return Ok(Ok(Prepared { root: wrapped, marker }));
        }
        roots = vec![];
    }
    if roots.is_empty() {
        return Err("l'archive ne contient pas de dossier romfs ou exefs : ce n'est pas un mod à installer tel quel (lis la page du mod)".into());
    }
    let hint = catalog.as_ref().and_then(|c| c.variant.clone());
    match choose_root(&roots, opts.variant.as_deref(), hint.as_deref(), tid) {
        Ok(rel) => {
            // `textures:<chemin>` (3DS) : pack de textures, sans dossier romfs.
            let rel = rel.strip_prefix("textures:").unwrap_or(&rel).to_string();
            Ok(Ok(Prepared { root: if rel.is_empty() { out } else { out.join(rel) }, marker }))
        }
        Err(list) => Ok(Err(list)),
    }
}

/// Catégorie Kaleido d'un mod GameBanana hors catalogue.
fn category_from_gb(category: &str, name: &str) -> String {
    let c = category.to_lowercase();
    if c.contains("skin") {
        return "style".into();
    }
    if c.contains("sound") {
        return "audio".into();
    }
    if c.contains("gui") {
        return "ui".into();
    }
    if c.contains("texture") || c.contains("map") {
        return "textures".into();
    }
    if c.contains("rom hack") {
        return "romhack".into();
    }
    other_category(name, Tier::High)
}

fn install_switch(app: &AppHandle, target: &ModTarget, tid: u64, id: &str, opts: &InstallOptions, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Installed, String> {
    let r = resolve(EmulatorId::Eden, app);
    let load = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?;
    let game_dir = load.join(format!("{tid:016X}"));
    let parked = disabled_dir(app, tid)?;
    let view = switch_view(app, target, tid);
    let entry = view.mods.iter().find(|m| m.id == id);

    // Conflits : les mods concernés sont mis de côté (réactivables ensuite).
    if let Some(entry) = entry.filter(|e| !e.conflicts.is_empty()) {
        if !opts.disable_conflicts {
            return Err(format!("Conflit avec : {}", entry.conflicts.join(", ")));
        }
        for (name, marker) in installed_dirs(&game_dir) {
            let shown = marker.as_ref().and_then(|m| m.name.clone()).unwrap_or_else(|| name.clone());
            let kind_name = view.mods.iter().find(|e| marker.as_ref().is_some_and(|m| m.id == e.id)).map(|e| e.name.clone());
            if entry.conflicts.contains(&shown) || kind_name.is_some_and(|n| entry.conflicts.contains(&n)) {
                move_dir(&game_dir.join(&name), &parked.join(&name))?;
            }
        }
    }

    let work = work_dir(app)?.join(format!("{tid:016X}-{:x}", fxhash(&format!("{id}{:?}", opts.local))));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = (|| -> Result<Result<Prepared, Vec<String>>, String> {
        if let Some(key) = id.strip_prefix("fl4sh:") {
            let (archive, sha) = fl4sh_archive(app, tid).ok_or("archive de Fl4sh introuvable")?;
            let zip_path = fl4sh_zip(app, &archive, &sha)?;
            let listing = zip_listing(&zip_path)?;
            let m = zip_mods(&listing, tid).into_iter().find(|m| mod_key(&split_version(&m.folder).0) == key).ok_or("mod introuvable dans l'archive")?;
            emit("extract", 0, 1);
            extract_zip(&zip_path, &work, |d, t| emit("extract", d, t))?;
            let root = work.join(m.root.trim_end_matches('/'));
            let name = entry.map(|e| e.name.clone());
            let category = entry.map(|e| e.category.clone());
            Ok(Ok(Prepared { root, marker: Marker { id: id.into(), version: split_version(&m.folder).1, source: FL4SH_REPO.into(), name, category, ..Default::default() } }))
        } else {
            prepare_archive(app, &tid_key(tid), tid, id, opts, &work, emit, &mod_roots)
        }
    })();
    let outcome = result.and_then(|prepared| {
        let p = match prepared {
            Ok(p) => p,
            Err(list) => return Ok(Installed::Variants(list)),
        };
        emit("install", 0, 1);
        // Ancienne version (active ou mise de côté) remplacée : la nouvelle reprend son dossier
        // (et donc son rang de priorité), son état et garde trace du fichier d'avant.
        let mut previous: Option<(PathBuf, Marker)> = None;
        for dir in [&game_dir, &parked] {
            for (name, marker) in installed_dirs(dir) {
                if let Some(m) = marker.filter(|m| m.id == p.marker.id) {
                    fs::remove_dir_all(dir.join(&name)).map_err(|e| e.to_string())?;
                    previous = Some((dir.join(&name), m));
                }
            }
        }
        let mut marker = p.marker.clone();
        if let Some((_, old)) = &previous {
            marker.previous_file = if old.file.is_some() && old.file != marker.file { old.file } else { old.previous_file.filter(|f| Some(*f) != marker.file) };
        }
        let target = match &previous {
            Some((path, _)) => path.clone(),
            None => {
                let base = folder_name(marker.name.as_deref().unwrap_or("Mod"));
                let mut folder = base.clone();
                let mut n = 2;
                while game_dir.join(&folder).exists() || parked.join(&folder).exists() {
                    folder = format!("{base} ({n})");
                    n += 1;
                }
                game_dir.join(&folder)
            }
        };
        move_dir(&p.root, &target)?;
        let _ = fs::remove_file(target.join(MARKER));
        write_marker(&target, &marker)?;
        Ok(Installed::Done)
    });
    let _ = fs::remove_dir_all(&work);
    outcome
}

/// Dossier (actif ou mis de côté) d'un mod installé par Kaleido.
fn switch_folder_of(app: &AppHandle, tid: u64, id: &str) -> Result<PathBuf, String> {
    let r = resolve(EmulatorId::Eden, app);
    let game_dir = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?.join(format!("{tid:016X}"));
    for dir in [game_dir, disabled_dir(app, tid)?] {
        if let Some((name, _)) = installed_dirs(&dir).into_iter().find(|(_, m)| m.as_ref().is_some_and(|m| m.id == id)) {
            return Ok(dir.join(name));
        }
    }
    Err("ce mod n'est pas installé par Kaleido".into())
}

fn uninstall_switch(app: &AppHandle, tid: u64, id: &str) -> Result<(), String> {
    fs::remove_dir_all(switch_folder_of(app, tid, id)?).map_err(|e| e.to_string())
}

/// Met de côté (ou remet en place) un dossier de mod, installé par Kaleido ou non.
fn toggle_folder(app: &AppHandle, tid: u64, name: &str, enabled: bool) -> Result<(), String> {
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

/// Retire un préfixe de rang « 01 » … « 99 » d'un nom de dossier installé à la main
/// (« 01 HD Textures »). « 60 FPS » ou « 30 fps » sont des noms, pas des rangs.
pub fn strip_rank(name: &str) -> &str {
    let b = name.as_bytes();
    let rest = name.get(3..).unwrap_or("");
    let looks_like_name = rest.get(..3).is_some_and(|w| w.eq_ignore_ascii_case("fps")) || rest.starts_with(|c: char| c.is_ascii_digit());
    if b.len() > 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b' ' && !looks_like_name {
        rest
    } else {
        name
    }
}

/// Nouveaux noms pour l'ordre voulu : (dossier, nom de base connu pour un mod de Kaleido).
/// Le premier l'emporte (Eden lit les dossiers par ordre alphabétique et garde le premier
/// fichier trouvé). Seuls les dossiers à renommer sont rendus.
pub fn rank_names(order: &[(String, Option<String>)]) -> Vec<(String, String)> {
    order
        .iter()
        .enumerate()
        .filter_map(|(i, (old, base))| {
            let base = base.clone().unwrap_or_else(|| strip_rank(old).to_string());
            let new = format!("{:02} {base}", i + 1);
            (new != *old).then(|| (old.clone(), new))
        })
        .collect()
}

/// Range les dossiers actifs donnés dans cet ordre de priorité (préfixes « 01 », « 02 »…).
fn reorder_switch(app: &AppHandle, tid: u64, order: &[String]) -> Result<(), String> {
    let r = resolve(EmulatorId::Eden, app);
    let game_dir = eden_load_dir(&r).ok_or("dossier d'Eden introuvable")?.join(format!("{tid:016X}"));
    for name in order {
        if name.is_empty() || name.contains(['/', '\\']) || name == ".." || !game_dir.join(name).is_dir() {
            return Err(format!("dossier « {name} » introuvable"));
        }
    }
    let markers = installed_dirs(&game_dir);
    // Mods de Kaleido : leur nom d'origine vient du marqueur (sans rang éventuel déjà mis).
    let with_base: Vec<(String, Option<String>)> = order
        .iter()
        .map(|n| {
            let base = markers.iter().find(|(d, _)| d == n).and_then(|(_, m)| m.as_ref()).and_then(|m| m.name.as_deref()).map(folder_name);
            (n.clone(), base)
        })
        .collect();
    let renames = rank_names(&with_base);
    // En deux temps, pour ne jamais tomber sur un nom déjà pris par un autre dossier de la liste.
    let mut temp = Vec::new();
    for (i, (old, new)) in renames.iter().enumerate() {
        let t = format!(".kaleido-tri-{i}");
        fs::rename(game_dir.join(old), game_dir.join(&t)).map_err(|e| format!("« {old} » ne peut pas être renommé : {e}"))?;
        temp.push((t, new.clone(), old.clone()));
    }
    for (t, new, old) in &temp {
        if game_dir.join(new).exists() {
            let _ = fs::rename(game_dir.join(t), game_dir.join(old));
            return Err(format!("un dossier « {new} » existe déjà"));
        }
        fs::rename(game_dir.join(t), game_dir.join(new)).map_err(|e| e.to_string())?;
    }
    // Les profils retiennent les mods installés à la main par leur dossier.
    let mut g = crate::mods_saves::game_profiles(app, tid);
    let mut changed = false;
    for p in g.profiles.iter_mut() {
        for k in p.mods.iter_mut() {
            if let Some((_, new)) = renames.iter().find(|(old, _)| k.strip_prefix("folder:") == Some(old.as_str())) {
                *k = format!("folder:{new}");
                changed = true;
            }
        }
    }
    if changed {
        crate::mods_saves::set_game_profiles(app, tid, g)?;
    }
    Ok(())
}

/// Met de côté ou réactive un mod installé par Kaleido.
fn toggle_switch_mod(app: &AppHandle, tid: u64, id: &str, enabled: bool) -> Result<(), String> {
    let folder = switch_folder_of(app, tid, id)?;
    let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("dossier invalide")?;
    toggle_folder(app, tid, &name, enabled)
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

/// Mods LayeredFS installés par Kaleido dans `load/mods/<TITLEID>/` (fusionnés : Azahar
/// n'en lit qu'un par jeu), avec la liste de leurs fichiers.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CtrManifest {
    mods: Vec<CtrMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CtrMod {
    marker: Marker,
    /// Chemins relatifs à `load/mods/<TITLEID>/`.
    files: Vec<String>,
    enabled: bool,
}

const CTR_MANIFEST: &str = "kaleido-mods.json";

fn ctr_mods_dir(user: &Path, tid: u64) -> PathBuf {
    user.join("load").join("mods").join(format!("{tid:016X}"))
}

fn ctr_textures_dir(user: &Path, tid: u64) -> PathBuf {
    user.join("load").join("textures").join(format!("{tid:016X}"))
}

fn read_manifest(dir: &Path) -> CtrManifest {
    fs::read(dir.join(CTR_MANIFEST)).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
}

fn write_manifest(dir: &Path, m: &CtrManifest) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(CTR_MANIFEST), serde_json::to_vec_pretty(m).unwrap_or_default()).map_err(|e| e.to_string())
}

/// Fichiers sous `dir`, chemins relatifs avec `/`, casse d'origine.
fn files_rel(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// Racines d'un mod 3DS : dossiers qui contiennent `romfs`, `exefs` ou `code.ips` ; à défaut
/// un pack de textures (`textures:<chemin>`).
fn ctr_roots(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, depth: u8, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| !is_junk(p)).collect();
        let hit = paths.iter().any(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            (p.is_dir() && matches!(n.as_str(), "romfs" | "exefs")) || (p.is_file() && matches!(n.as_str(), "code.ips" | "code.bps" | "exheader.bin"))
        });
        if hit {
            out.push(dir.strip_prefix(base).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default());
            return;
        }
        if depth > 0 {
            for p in paths.iter().filter(|p| p.is_dir()) {
                walk(base, p, depth - 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, 6, &mut out);
    if out.is_empty() {
        if let Some(t) = texture_root(dir, 5) {
            out.push(format!("textures:{}", t.strip_prefix(dir).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default()));
        }
    }
    out.sort();
    out
}

fn ctr_view(app: &AppHandle, target: &ModTarget, tid: u64) -> ModsView {
    let r = ctr_emulator(app);
    let mut view = ModsView { emulator: Some(r.id.name()), emulator_found: r.exe.is_some(), gamebanana: crate::gamebanana::game_of(tid), can_import: true, ..Default::default() };
    let Some(user) = r.ctr_user_dir() else {
        view.errors.push("Dossier de l'émulateur 3DS introuvable.".into());
        return view;
    };
    view.location = Some(ctr_mods_dir(&user, tid).display().to_string());
    let Some((_, _, fr)) = CTR_GAMES.iter().find(|g| g.0 == tid) else {
        view.notes.push("Aucun mod connu pour ce jeu.".into());
        return view;
    };
    let cheats = user.join("cheats").join(format!("{tid:016X}.txt"));
    view.mods.push(ModEntry {
        id: "cheats".into(),
        name: "Codes de triche".into(),
        description: "Ajoute la base de codes de la communauté (argent, objets, éclosion rapide, chromatiques…). Ils sont installés désactivés : tu choisis ensuite lesquels activer, ici ou dans le menu Triches de l'émulateur.".into(),
        category: "cheats".into(),
        source: "auto".into(),
        author: "iSharingan / JourneyOver (CTRPF-AR-CHEAT-CODES)".into(),
        page: "https://github.com/iSharingan/CTRPF-AR-CHEAT-CODES".into(),
        installed: cheats.is_file(),
        enabled: true,
        warning: Some("Certains codes ne marchent qu'avec une version précise du jeu (indiquée dans leur nom). Sauvegarde avant d'en essayer.".into()),
        kind: Some("cheats".into()),
        ..Default::default()
    });
    if fps60_ctr::supports(tid) {
        view.mods.push(fps60_entry(&user, target, tid));
    }
    let tex_dir = ctr_textures_dir(&user, tid);
    if let Some(pack) = texture_pack(tid) {
        view.mods.push(ModEntry {
            id: "textures".into(),
            name: "Textures HD".into(),
            description: format!(
                "Remplace les textures du jeu par des versions haute définition, affichées par {}. {}",
                r.id.name(),
                if pack.note.is_empty() { "" } else { pack.note }
            ),
            category: "textures".into(),
            recommended: tuning::current_tier() >= Tier::Medium,
            source: "auto".into(),
            author: pack.author.into(),
            page: "https://github.com/Gray-Rice/PokeTex-3DS".into(),
            size: Some(pack.size),
            installed: tex_dir.join(MARKER).is_file(),
            enabled: true,
            warning: Some("Ferme l'émulateur avant d'installer : Kaleido active l'option « textures personnalisées » dans sa configuration.".into()),
            kind: Some("textures".into()),
            ..Default::default()
        });
    }

    // Mods installés par Kaleido : LayeredFS (manifeste) et packs de textures (sous-dossiers).
    let mods_dir = ctr_mods_dir(&user, tid);
    let manifest = read_manifest(&mods_dir);
    let tex_active = installed_dirs(&tex_dir);
    let tex_parked = disabled_dir(app, tid).map(|d| installed_dirs(&d.join("textures"))).unwrap_or_default();
    let mine = |id: &str| -> Option<(String, Marker, bool)> {
        manifest
            .mods
            .iter()
            .find(|m| m.marker.id == id)
            .map(|m| (String::new(), m.marker.clone(), m.enabled))
            .or_else(|| fs::read(plugin_marker(&user, tid, id)).ok().and_then(|d| serde_json::from_slice::<Marker>(&d).ok()).map(|m| (String::new(), m, true)))
            .or_else(|| tex_active.iter().map(|(n, m)| (n, m, true)).chain(tex_parked.iter().map(|(n, m)| (n, m, false))).find_map(|(n, m, on)| m.as_ref().filter(|m| m.id == id).map(|m| (n.clone(), m.clone(), on))))
    };
    let mut entries = catalog_entries(app, &tid_key(tid), target, &mine, &mut view);
    for m in &manifest.mods {
        if !entries.iter().any(|e| e.id == m.marker.id) {
            entries.push(entry_from_marker(&m.marker.id, &m.marker, m.enabled));
        }
    }
    for (folder, marker, on) in tex_active.iter().map(|(n, m)| (n, m, true)).chain(tex_parked.iter().map(|(n, m)| (n, m, false))) {
        if let Some(m) = marker {
            if !entries.iter().any(|e| e.id == m.id) {
                let mut e = entry_from_marker(folder, m, on);
                e.kind = Some("textures".into());
                entries.push(e);
            }
        }
    }

    // Fichiers communs entre mods LayeredFS actifs.
    let active: Vec<(String, Vec<String>)> = manifest.mods.iter().filter(|m| m.enabled).map(|m| (m.marker.name.clone().unwrap_or(m.marker.id.clone()), m.files.iter().map(|f| f.to_lowercase()).collect())).collect();
    let overlaps = overlap_map(&active);
    resolve_conflicts(&mut entries, &[], &[]);
    for e in entries.iter_mut().filter(|e| e.installed && e.enabled) {
        e.overlaps = overlaps.get(&e.name).cloned().unwrap_or_default();
    }
    view.installed_gb = entries.iter().filter(|e| e.installed).filter_map(|e| e.gb).collect();
    view.mods.extend(entries);

    // Fichiers LayeredFS présents qui ne viennent pas des mods de Kaleido.
    let mut known: std::collections::HashSet<String> = manifest.mods.iter().flat_map(|m| m.files.iter().map(|f| f.to_lowercase())).chain([CTR_MANIFEST.to_lowercase()]).collect();
    // Le 60 fps natif : son marqueur, et le code.ips quand il est seul (sans mod du randomiseur).
    if mods_dir.join(FPS60_MARKER).is_file() {
        known.insert(FPS60_MARKER.to_lowercase());
        if !mods_dir.join("romfs").exists() {
            known.insert("code.ips".into());
        }
    }
    let mut rest: Vec<String> = files_under(&mods_dir).into_iter().filter(|f| !known.contains(f)).collect();
    // Mod du Randomizer ou de l'éditeur de ROM, copié par la Bibliothèque au lancement (marqueur
    // kaleido.txt) : RomFS, code.ips (taux de chromatiques) et le marqueur.
    if mods_dir.join(play::MOD_COPY_MARKER).is_file() {
        let before = rest.len();
        rest.retain(|f| f != play::MOD_COPY_MARKER && f != "code.ips" && !f.starts_with("romfs/"));
        if rest.len() < before {
            view.others.push(OtherMod {
                name: "Mod du Randomizer Kaleido".into(),
                enabled: true,
                category: "gameplay".into(),
                overlaps: vec![],
                can_toggle: false,
                exefs: None,
                from_kaleido: true,
            });
        }
    }
    let foreign = rest.len();
    if foreign > 0 {
        view.others.push(OtherMod {
            name: format!("Mod LayeredFS installé à la main ({foreign} fichier{})", if foreign > 1 { "s" } else { "" }),
            enabled: true,
            category: "other".into(),
            overlaps: vec![],
            can_toggle: false,
            exefs: None,
            from_kaleido: false,
        });
    }
    if !fps60_ctr::supports(tid) {
        view.notes.push(format!(
            "Pas encore de 60 fps natif pour Pokémon {fr} : les codes « 60 FPS » qui circulent font tourner tout le jeu deux fois plus vite (sa logique est calée sur 30 images par seconde). Kaleido ne les propose donc pas."
        ));
    }
    view
}

fn install_ctr(app: &AppHandle, target: &ModTarget, tid: u64, id: &str, opts: &InstallOptions, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Installed, String> {
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
            fs::write(&file, out).map_err(|e| e.to_string())?;
            Ok(Installed::Done)
        }
        "textures" => {
            let pack = texture_pack(tid).ok_or("pas de pack de textures pour ce jeu")?;
            let target_dir = ctr_textures_dir(&user, tid);
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
                if target_dir.exists() && !target_dir.join(MARKER).is_file() && installed_dirs(&target_dir).iter().all(|(_, m)| m.is_none()) {
                    let backup = target_dir.with_file_name(format!("{tid:016X}.avant-kaleido"));
                    let _ = fs::remove_dir_all(&backup);
                    fs::rename(&target_dir, &backup).map_err(|e| e.to_string())?;
                }
                merge_dir(&root, &target_dir)?;
                write_marker(&target_dir, &Marker { id: "textures".into(), source: "Gray-Rice/PokeTex-3DS".into(), ..Default::default() })?;
                tuning::set_azahar_custom_textures(&user, true, tuning::current_tier() >= Tier::High)
            })();
            let _ = fs::remove_dir_all(&work);
            result.map(|_| Installed::Done)
        }
        "fps60" => {
            emit("install", 0, 1);
            install_fps60(&user, target, tid).map(|_| Installed::Done)
        }
        _ => install_ctr_mod(app, target, &user, tid, id, opts, emit),
    }
}

// ---------------------------------------------------------------------------
// 60 fps natif (runtime ctr-smooth) : un code.ips précompilé par version du jeu, fusionné avec
// le code.ips déjà présent (taux de chromatiques du randomiseur…).

/// Marqueur du 60 fps natif dans `load/mods/<TID>/`.
const FPS60_MARKER: &str = "kaleido-fps60.json";

fn fps60_entry(user: &Path, target: &ModTarget, tid: u64) -> ModEntry {
    let mods_dir = ctr_mods_dir(user, tid);
    let blocked = if play::ctr_update_installed(user, tid) {
        Some("Une mise à jour du jeu est installée dans l'émulateur : le 60 fps natif ne couvre que la version de la cartouche (1.0), sans mise à jour.".to_string())
    } else if target.rom.is_none() {
        Some("Ouvre ce jeu depuis la Bibliothèque : Kaleido doit lire son programme pour vérifier la version.".to_string())
    } else {
        None
    };
    ModEntry {
        id: "fps60".into(),
        name: "60 fps natif".into(),
        description: "Le jeu affiche 60 images par seconde au lieu de 30 : caméra, personnages et Pokémon bougent entre deux images, en exploration comme en combat. La vitesse du jeu, la musique, les événements et le hasard restent ceux d'origine (la logique tourne toujours à 30 images par seconde). Les menus et effets en 2D restent à 30. L + Select, tenus une seconde en jeu, coupent ou rétablissent le lissage.".into(),
        category: "fps".into(),
        source: "auto".into(),
        author: "Kaleido (ctr-smooth)".into(),
        page: String::new(),
        installed: mods_dir.join(FPS60_MARKER).is_file(),
        enabled: true,
        warning: Some("Expérimental. L'ordinateur dessine deux fois plus d'images : préfère Vulkan dans l'émulateur et baisse la résolution interne s'il ralentit. Un mod relancé depuis le randomiseur remplace le dossier du jeu : réactive alors le 60 fps natif.".into()),
        kind: Some("fps60".into()),
        blocked,
        ..Default::default()
    }
}

/// Programme d'origine (décompressé) et profil 60 fps de la ROM du jeu.
fn fps60_profile(target: &ModTarget, tid: u64) -> Result<(Vec<u8>, &'static fps60_ctr::Profile), String> {
    let rom = target.rom.as_deref().ok_or("fichier du jeu inconnu : ouvre ce jeu depuis la Bibliothèque")?;
    let game = kaleido_core::CtrGameRom::open(Path::new(rom)).map_err(|e| e.to_string())?;
    let code = game.code().map_err(|e| e.to_string())?.code;
    let profile = fps60_ctr::profile_for(tid, &code).ok_or_else(|| {
        format!("Cette version du jeu n'est pas couverte (programme {}…). Le 60 fps natif existe pour Rubis Oméga EUR, cartouche 1.0.", &fps60_ctr::sha256_hex(&code)[..12])
    })?;
    Ok((code, profile))
}

fn install_fps60(user: &Path, target: &ModTarget, tid: u64) -> Result<(), String> {
    if play::ctr_update_installed(user, tid) {
        return Err("Une mise à jour du jeu est installée dans l'émulateur : le 60 fps natif ne couvre que la version 1.0 de la cartouche.".into());
    }
    // Les codes « 60 FPS » de la base de triche doublent la vitesse du jeu par-dessus le lissage.
    let cheats = fs::read_to_string(user.join("cheats").join(format!("{tid:016X}.txt"))).unwrap_or_default();
    if let Some(c) = parse_azahar(&cheats).into_iter().find(|c| c.enabled && c.name.to_lowercase().replace(' ', "").contains("60fps")) {
        return Err(format!("Le code de triche « {} » est activé : il ferait tourner le jeu deux fois plus vite. Désactive-le d'abord (onglet Codes de triche).", c.name));
    }
    let (original, profile) = fps60_profile(target, tid)?;
    let dir = ctr_mods_dir(user, tid);
    let ips_path = dir.join("code.ips");
    let existing = fs::read(&ips_path).ok();
    // Un code.ips qui contient déjà le lissage (réinstallation, ancienne version, copie
    // manuelle) : on repart des autres patchs qu'il contient.
    let previous = match &existing {
        Some(e) => fps60_ctr::strip(profile, &original, e).map_err(|e| e.to_string())?,
        None => None,
    };
    let merged = fps60_ctr::merged_ips(profile, &original, previous.as_deref()).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(&ips_path, merged).map_err(|e| e.to_string())?;
    let marker = serde_json::json!({ "version": profile.version, "code_sha256": profile.code_sha256, "merged_with_existing": previous.is_some() });
    fs::write(dir.join(FPS60_MARKER), serde_json::to_vec_pretty(&marker).unwrap()).map_err(|e| e.to_string())
}

/// Retire le lissage du code.ips ; les autres patchs (taux de chromatiques…) restent.
fn uninstall_fps60(user: &Path, target: &ModTarget, tid: u64) -> Result<(), String> {
    let dir = ctr_mods_dir(user, tid);
    let ips_path = dir.join("code.ips");
    if let Ok(existing) = fs::read(&ips_path) {
        let (original, profile) = fps60_profile(target, tid)?;
        match fps60_ctr::strip(profile, &original, &existing).map_err(|e| e.to_string())? {
            Some(rest) => fs::write(&ips_path, rest).map_err(|e| e.to_string())?,
            None => fs::remove_file(&ips_path).map_err(|e| e.to_string())?,
        }
    }
    let _ = fs::remove_file(dir.join(FPS60_MARKER));
    Ok(())
}

/// Mod du catalogue, de GameBanana ou d'un fichier : LayeredFS ou pack de textures.
fn install_ctr_mod(app: &AppHandle, target: &ModTarget, user: &Path, tid: u64, id: &str, opts: &InstallOptions, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Installed, String> {
    let view = ctr_view(app, target, tid);
    if let Some(entry) = view.mods.iter().find(|m| m.id == id).filter(|e| !e.conflicts.is_empty()) {
        if !opts.disable_conflicts {
            return Err(format!("Conflit avec : {}", entry.conflicts.join(", ")));
        }
        for c in &entry.conflicts {
            if let Some(other) = view.mods.iter().find(|m| &m.name == c && m.installed && m.enabled) {
                toggle_ctr_mod(app, user, tid, &other.id, false)?;
            }
        }
    }
    if crate::mods_catalog::find(app, &tid_key(tid), id).and_then(|c| c.kind).as_deref() == Some("plugin3gx") {
        return install_plugin3gx(app, user, tid, id, opts, emit);
    }
    let work = work_dir(app)?.join(format!("ctr-{tid:016X}-{:x}", fxhash(&format!("{id}{:?}", opts.local))));
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = prepare_archive(app, &tid_key(tid), tid, id, opts, &work, emit, &ctr_roots).and_then(|prepared| {
        let p = match prepared {
            Ok(p) => p,
            Err(list) => return Ok(Installed::Variants(list.into_iter().map(|v| v.trim_start_matches("textures:").to_string()).collect())),
        };
        emit("install", 0, 1);
        let textures = !p.root.join("romfs").is_dir() && !p.root.join("exefs").is_dir() && !p.root.join("code.ips").is_file() && !p.root.join("exheader.bin").is_file();
        if textures {
            // Pack de textures : sous-dossier de `load/textures/<TITLEID>/`.
            let base_dir = ctr_textures_dir(user, tid);
            uninstall_ctr_mod(app, user, tid, &p.marker.id).ok();
            let folder = folder_name(p.marker.name.as_deref().unwrap_or("Textures"));
            let dest = base_dir.join(&folder);
            if dest.exists() {
                fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
            }
            move_dir(&p.root, &dest)?;
            write_marker(&dest, &p.marker)?;
            tuning::set_azahar_custom_textures(user, true, tuning::current_tier() >= Tier::High)?;
            return Ok(Installed::Done);
        }
        // LayeredFS : fichiers fusionnés dans `load/mods/<TITLEID>/`.
        let dir = ctr_mods_dir(user, tid);
        let _ = uninstall_ctr_mod(app, user, tid, &p.marker.id);
        let files = files_rel(&p.root);
        for f in &files {
            let to = dir.join(f);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if to.exists() {
                let _ = fs::remove_file(&to);
            }
            if fs::rename(p.root.join(f), &to).is_err() {
                fs::copy(p.root.join(f), &to).map_err(|e| e.to_string())?;
            }
        }
        let mut manifest = read_manifest(&dir);
        manifest.mods.retain(|m| m.marker.id != p.marker.id);
        manifest.mods.push(CtrMod { marker: p.marker, files, enabled: true });
        write_manifest(&dir, &manifest)?;
        Ok(Installed::Done)
    });
    let _ = fs::remove_dir_all(&work);
    result
}

/// Dossier des plugins 3GX d'un jeu (chargeur de plugins d'Azahar, comme Luma3DS).
fn plugins_dir(user: &Path, tid: u64) -> PathBuf {
    user.join("sdmc").join("luma").join("plugins").join(format!("{tid:016X}"))
}

/// Marqueur d'un plugin installé par Kaleido : `kaleido-<id>.json` (`version` = fichiers .3gx).
fn plugin_marker(user: &Path, tid: u64, id: &str) -> PathBuf {
    plugins_dir(user, tid).join(format!("kaleido-{:016x}.json", fxhash(id)))
}

/// Témoin : le chargeur de plugins d'Azahar a été activé par Kaleido.
fn loader_flag(user: &Path) -> PathBuf {
    user.join("sdmc").join("luma").join("plugins").join("kaleido-loader.txt")
}

/// Plugin 3GX (ex. Pokémon qui suit sur Soleil et Lune) : fichiers `.3gx` copiés dans
/// `sdmc/luma/plugins/<TITLEID>/`.
fn install_plugin3gx(app: &AppHandle, user: &Path, tid: u64, id: &str, opts: &InstallOptions, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Installed, String> {
    let entry = crate::mods_catalog::find(app, &tid_key(tid), id).ok_or("mod inconnu")?;
    let source = match &opts.local {
        Some(l) => PathBuf::from(l),
        None => {
            let gb = entry.gb.ok_or("ce plugin se télécharge à la main : choisis le fichier téléchargé")?;
            let (_, files) = crate::gamebanana::profile_raw(app, gb)?;
            let file = pick_gb_file(&files, opts.file, entry.file.as_deref()).ok_or("ce mod n'a pas de fichier téléchargeable")?;
            gb_download(app, file, emit)?
        }
    };
    let work = work_dir(app)?.join(format!("3gx-{tid:016X}"));
    let _ = fs::remove_dir_all(&work);
    let result = (|| -> Result<Installed, String> {
        let plugins: Vec<PathBuf> = if source.extension().is_some_and(|e| e.eq_ignore_ascii_case("3gx")) {
            vec![source.clone()]
        } else {
            emit("extract", 0, 1);
            extract_any(&source, &work, |d, t| emit("extract", d, t))?;
            files_rel(&work).into_iter().filter(|f| f.to_lowercase().ends_with(".3gx") && !f.contains("__MACOSX")).map(|f| work.join(f)).collect()
        };
        if plugins.is_empty() {
            return Err("aucun plugin .3gx dans l'archive".into());
        }
        emit("install", 0, 1);
        let dir = plugins_dir(user, tid);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut names = Vec::new();
        for p in &plugins {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            fs::copy(p, dir.join(&name)).map_err(|e| e.to_string())?;
            names.push(name);
        }
        let marker = Marker { id: id.into(), version: Some(names.join("|")), source: "GameBanana".into(), name: Some(entry.name.clone()), category: Some(entry.category.clone()), gb: entry.gb, page: entry.page.clone(), ..Default::default() };
        fs::write(plugin_marker(user, tid, id), serde_json::to_vec_pretty(&marker).unwrap_or_default()).map_err(|e| e.to_string())?;
        // Sans le chargeur de plugins (désactivé par défaut), Azahar ignore le .3gx. Kaleido note
        // qu'il l'a activé lui-même, pour ne le couper ensuite que dans ce cas.
        let config = fs::read_to_string(user.join("config").join("qt-config.ini")).unwrap_or_default();
        if tuning::get_qt(&config, "System", "plugin_loader").as_deref() != Some("true") {
            tuning::set_azahar_plugin_loader(user, true)?;
            let _ = fs::write(loader_flag(user), b"Kaleido a active le chargeur de plugins d'Azahar.");
        }
        Ok(Installed::Done)
    })();
    let _ = fs::remove_dir_all(&work);
    result
}

fn uninstall_plugin3gx(user: &Path, tid: u64, id: &str) -> Result<bool, String> {
    let marker_path = plugin_marker(user, tid, id);
    let Some(marker) = fs::read(&marker_path).ok().and_then(|d| serde_json::from_slice::<Marker>(&d).ok()) else { return Ok(false) };
    for name in marker.version.unwrap_or_default().split('|').filter(|n| !n.is_empty() && !n.contains(['/', '\\'])) {
        let _ = fs::remove_file(plugins_dir(user, tid).join(name));
    }
    fs::remove_file(marker_path).map_err(|e| e.to_string())?;
    // Plus aucun plugin installé par Kaleido, pour aucun jeu : on remet le chargeur comme avant.
    let root = user.join("sdmc").join("luma").join("plugins");
    let any_left = fs::read_dir(&root).into_iter().flatten().flatten().any(|d| fs::read_dir(d.path()).into_iter().flatten().flatten().any(|f| f.file_name().to_string_lossy().starts_with("kaleido-")));
    if !any_left && loader_flag(user).is_file() {
        tuning::set_azahar_plugin_loader(user, false)?;
        let _ = fs::remove_file(loader_flag(user));
    }
    Ok(true)
}

/// Fichiers d'un mod LayeredFS qu'aucun autre mod actif n'utilise.
fn ctr_own_files(manifest: &CtrManifest, id: &str) -> Vec<String> {
    let Some(m) = manifest.mods.iter().find(|m| m.marker.id == id) else { return vec![] };
    m.files.iter().filter(|f| !manifest.mods.iter().any(|o| o.marker.id != id && o.enabled && o.files.iter().any(|g| g.eq_ignore_ascii_case(f)))).cloned().collect()
}

fn ctr_parked_dir(app: &AppHandle, tid: u64, id: &str) -> Result<PathBuf, String> {
    Ok(disabled_dir(app, tid)?.join("layeredfs").join(format!("{:016x}", fxhash(id))))
}

fn uninstall_ctr_mod(app: &AppHandle, user: &Path, tid: u64, id: &str) -> Result<(), String> {
    if uninstall_plugin3gx(user, tid, id)? {
        return Ok(());
    }
    let dir = ctr_mods_dir(user, tid);
    let mut manifest = read_manifest(&dir);
    if let Some(m) = manifest.mods.iter().find(|m| m.marker.id == id) {
        if m.enabled {
            for f in ctr_own_files(&manifest, id) {
                let _ = fs::remove_file(dir.join(&f));
            }
            remove_empty_dirs(&dir);
        }
        let _ = fs::remove_dir_all(ctr_parked_dir(app, tid, id)?);
        manifest.mods.retain(|m| m.marker.id != id);
        return write_manifest(&dir, &manifest);
    }
    for base in [ctr_textures_dir(user, tid), disabled_dir(app, tid)?.join("textures")] {
        if let Some((name, _)) = installed_dirs(&base).into_iter().find(|(_, m)| m.as_ref().is_some_and(|m| m.id == id)) {
            return fs::remove_dir_all(base.join(name)).map_err(|e| e.to_string());
        }
    }
    Err("ce mod n'est pas installé par Kaleido".into())
}

fn toggle_ctr_mod(app: &AppHandle, user: &Path, tid: u64, id: &str, enabled: bool) -> Result<(), String> {
    let dir = ctr_mods_dir(user, tid);
    let mut manifest = read_manifest(&dir);
    if let Some(i) = manifest.mods.iter().position(|m| m.marker.id == id) {
        if manifest.mods[i].enabled == enabled {
            return Ok(());
        }
        let parked = ctr_parked_dir(app, tid, id)?;
        if enabled {
            for f in files_rel(&parked) {
                let to = dir.join(&f);
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let _ = fs::remove_file(&to);
                if fs::rename(parked.join(&f), &to).is_err() {
                    fs::copy(parked.join(&f), &to).map_err(|e| e.to_string())?;
                }
            }
            let _ = fs::remove_dir_all(&parked);
        } else {
            for f in ctr_own_files(&manifest, id) {
                let to = parked.join(&f);
                if let Some(parent) = to.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                if fs::rename(dir.join(&f), &to).is_err() {
                    fs::copy(dir.join(&f), &to).map_err(|e| e.to_string())?;
                    let _ = fs::remove_file(dir.join(&f));
                }
            }
            remove_empty_dirs(&dir);
        }
        manifest.mods[i].enabled = enabled;
        return write_manifest(&dir, &manifest);
    }
    // Pack de textures d'un sous-dossier.
    let active = ctr_textures_dir(user, tid);
    let parked = disabled_dir(app, tid)?.join("textures");
    let (from, to) = if enabled { (&parked, &active) } else { (&active, &parked) };
    let (name, _) = installed_dirs(from).into_iter().find(|(_, m)| m.as_ref().is_some_and(|m| m.id == id)).ok_or("ce mod n'est pas installé par Kaleido")?;
    move_dir(&from.join(&name), &to.join(&name))
}

/// Supprime les dossiers vides (après retrait des fichiers d'un mod).
fn remove_empty_dirs(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            remove_empty_dirs(&p);
            let _ = fs::remove_dir(&p);
        }
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

fn uninstall_ctr(app: &AppHandle, target: &ModTarget, tid: u64, id: &str) -> Result<(), String> {
    let r = ctr_emulator(app);
    let user = r.ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
    match id {
        "fps60" => uninstall_fps60(&user, target, tid),
        "cheats" => fs::remove_file(user.join("cheats").join(format!("{tid:016X}.txt"))).map_err(|e| e.to_string()),
        "textures" => {
            let target = ctr_textures_dir(&user, tid);
            // Les packs installés à part (sous-dossiers marqués) sont gardés.
            let keep: Vec<String> = installed_dirs(&target).into_iter().filter(|(_, m)| m.is_some()).map(|(n, _)| n).collect();
            let hold = work_dir(app)?.join(format!("keep-{tid:016X}"));
            for k in &keep {
                move_dir(&target.join(k), &hold.join(k))?;
            }
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            let backup = target.with_file_name(format!("{tid:016X}.avant-kaleido"));
            if backup.is_dir() {
                fs::rename(&backup, &target).map_err(|e| e.to_string())?;
            }
            for k in &keep {
                move_dir(&hold.join(k), &target.join(k))?;
            }
            let _ = fs::remove_dir_all(&hold);
            Ok(())
        }
        _ => uninstall_ctr_mod(app, &user, tid, id),
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

/// Révision d'une ROM DS (octet 0x1E de l'en-tête).
fn nds_revision(rom: &Path) -> Option<u8> {
    let mut f = fs::File::open(rom).ok()?;
    let mut header = [0u8; 0x20];
    f.read_exact(&mut header).ok()?;
    Some(header[0x1E])
}

/// La plupart des romhacks DS se patchent sur la version USA (code en « E », ou « O » pour les
/// Noire / Blanche internationales). La version européenne en anglais de Platine a le même
/// code CPUE mais la révision 10 : son contenu diffère. `None` si la ROM convient.
pub fn us_rom_problem(code: &str, revision: u8) -> Option<String> {
    let region = code.as_bytes().get(3).copied()?;
    if matches!(region, b'E' | b'O') && revision < 10 {
        return None;
    }
    let what = match region {
        b'E' | b'P' | b'O' => "la version européenne en anglais",
        b'F' => "la version française",
        b'D' => "la version allemande",
        b'I' => "la version italienne",
        b'S' => "la version espagnole",
        b'J' => "la version japonaise",
        b'K' => "la version coréenne",
        _ => "une autre version",
    };
    let us = format!("{}E", &code[..3]);
    Some(format!("Ta ROM ne convient pas à ce patch : il faut la version USA du jeu (code {us}, révision 0 ou 1), et celle-ci est {what} (code {code}, révision {revision}). Le patch échouerait : utilise une copie USA de ta cartouche."))
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
        category: "cheats".into(),
        source: "auto".into(),
        author: "DeadSkullzJr, converti par Lyrx997".into(),
        page: "https://github.com/Lyrx997/MelonDS-Desktop-Cheats".into(),
        installed: mch_path(rom).is_file(),
        enabled: true,
        warning: Some("Sauvegarde avant d'essayer un code : certains peuvent bloquer le jeu.".into()),
        kind: Some("cheats".into()),
        ..Default::default()
    });
    // Romhacks livrés en patch : la ROM créée se range à côté de celle-ci.
    if let Some(catalog) = code.as_deref().and_then(|c| c.get(..3)).and_then(|c| crate::mods_catalog::for_game(app, c)) {
        let revision = nds_revision(rom).unwrap_or(0);
        for m in catalog.mods {
            let out = patched_rom_path(rom, &m.name);
            let needs_us = m.game_version.as_deref().is_some_and(|v| v.contains("USA"));
            let blocked = if needs_us { code.as_deref().and_then(|c| us_rom_problem(c, revision)) } else { None };
            view.mods.push(ModEntry {
                blocked,
                installed: out.is_file(),
                enabled: true,
                id: m.id,
                name: m.name,
                description: m.description,
                category: m.category,
                group: m.group,
                source: m.source,
                author: m.author.unwrap_or_default(),
                page: m.page.or_else(|| m.gb.map(|id| format!("https://gamebanana.com/mods/{id}"))).unwrap_or_default(),
                game_version: m.game_version,
                warning: m.warning,
                popularity: m.popularity,
                kind: Some(m.kind.unwrap_or_else(|| "patch".into())),
                ..Default::default()
            });
        }
    }
    let config = play::load_config(app);
    if config.preferred_nds == Some(EmulatorId::Desmume) {
        view.notes.push("Les codes de triche sont lus par melonDS, pas par DeSmuME.".into());
    }
    view.notes.push("Pas de vrai mode 60 FPS pour les Pokémon DS : le jeu est calé sur 30 images par seconde et les codes qui débloquent la cadence l'accélèrent entièrement. Kaleido ne les propose donc pas.".into());
    view
}

/// ROM d'un romhack créée à côté de la ROM d'origine.
fn patched_rom_path(rom: &Path, name: &str) -> PathBuf {
    rom.with_file_name(format!("{}.nds", folder_name(name)))
}

/// CRC-16 (MODBUS) de l'en-tête d'une ROM DS, stocké en 0x15E.
pub fn nds_header_ok(data: &[u8]) -> bool {
    if data.len() < 0x160 {
        return false;
    }
    let mut crc = 0xFFFFu16;
    for &b in &data[..0x15E] {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc == u16::from_le_bytes([data[0x15E], data[0x15F]])
}

fn is_patch_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| matches!(e.to_string_lossy().to_ascii_lowercase().as_str(), "xdelta" | "xdelta3" | "vcdiff" | "xd" | "bps" | "ips"))
}

/// Applique le patch d'un romhack (fichier, ou archive qui le contient) sur la ROM.
fn install_nds_patch(app: &AppHandle, rom: &Path, id: &str, opts: &InstallOptions, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Installed, String> {
    let local = PathBuf::from(opts.local.as_deref().ok_or("choisis le patch téléchargé")?);
    let code = nds_code(rom).ok_or("code du jeu illisible")?;
    let entry = code.get(..3).and_then(|c| crate::mods_catalog::find(app, c, id));
    let name = entry.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| local.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Romhack".into()));
    let work = work_dir(app)?.join(format!("patch-{:x}", fxhash(&local.to_string_lossy())));
    let _ = fs::remove_dir_all(&work);
    let result = (|| -> Result<Installed, String> {
        let patch_path = if is_patch_file(&local) {
            local.clone()
        } else {
            emit("extract", 0, 1);
            extract_any(&local, &work, |d, t| emit("extract", d, t))?;
            let found: Vec<String> = files_rel(&work).into_iter().filter(|f| is_patch_file(Path::new(f))).collect();
            let chosen = match (found.len(), &opts.variant) {
                (0, _) => return Err("aucun patch (.xdelta, .bps, .ips) dans l'archive".into()),
                (_, Some(v)) if found.contains(v) => v.clone(),
                (1, _) => found[0].clone(),
                _ => return Ok(Installed::Variants(found)),
            };
            work.join(chosen)
        };
        emit("install", 0, 1);
        let base = fs::read(rom).map_err(|e| e.to_string())?;
        let patch = fs::read(&patch_path).map_err(|e| e.to_string())?;
        let out = kaleido_core::romhack::apply_patch(&base, &patch).map_err(|e| e.to_string())?;
        if !nds_header_ok(&out) {
            return Err(format!(
                "le patch ne correspond pas à cette ROM ({code}) : il faut la version du jeu indiquée sur la page du romhack (souvent la version américaine non modifiée)"
            ));
        }
        let path = patched_rom_path(rom, &name);
        fs::write(&path, out).map_err(|e| e.to_string())?;
        Ok(Installed::Output(path))
    })();
    let _ = fs::remove_dir_all(&work);
    result
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
    let mut view = match target.platform.as_str() {
        "switch" => switch_view(app, target, base_title_id(parse_tid(target.title_id.as_deref())?)),
        "3ds" => ctr_view(app, target, parse_tid(target.title_id.as_deref())?),
        "nds" => nds_view(app, Path::new(target.rom.as_deref().ok_or("ROM inconnue")?)),
        "gba" => gba_view(app),
        _ => return Err("console inconnue".into()),
    };
    if let Some(tid) = game_tid(target) {
        for m in view.mods.iter_mut() {
            m.affects_save = crate::mods_saves::affects_save(m);
        }
        view.saves = crate::mods_saves::list(app, tid);
        view.profiles = Some(crate::mods_saves::game_profiles(app, tid));
    }
    Ok(view)
}

/// Title ID d'un jeu dont Kaleido gère la sauvegarde (Switch, 3DS).
fn game_tid(target: &ModTarget) -> Option<u64> {
    match target.platform.as_str() {
        "switch" => parse_tid(target.title_id.as_deref()).ok().map(base_title_id),
        "3ds" => parse_tid(target.title_id.as_deref()).ok(),
        _ => None,
    }
}

/// Mod de la vue qui touche à la partie, par identifiant.
fn save_affecting(view: &ModsView, id: &str) -> Option<ModEntry> {
    view.mods.iter().find(|m| m.id == id && crate::mods_saves::affects_save(m)).cloned()
}

/// Active ou met de côté un mod installé (identifiant Kaleido ou dossier installé à la main).
fn toggle_key(app: &AppHandle, target: &ModTarget, tid: u64, key: &str, enabled: bool) -> Result<(), String> {
    if let Some(id) = key.strip_prefix("id:") {
        return match target.platform.as_str() {
            "switch" => toggle_switch_mod(app, tid, id, enabled),
            _ => {
                let user = ctr_emulator(app).ctr_user_dir().ok_or("dossier de l'émulateur 3DS introuvable")?;
                toggle_ctr_mod(app, &user, tid, id, enabled)
            }
        };
    }
    match key.strip_prefix("folder:") {
        Some(name) if target.platform == "switch" => toggle_folder(app, tid, name, enabled),
        _ => Err("mod inconnu".into()),
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

/// Télécharge (ou prend le fichier donné) et installe un mod.
#[tauri::command]
pub async fn mods_install(target: ModTarget, id: String, options: Option<InstallOptions>, app: AppHandle) -> Result<InstallResult, String> {
    crate::blocking(move || {
        let opts = options.unwrap_or_default();
        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("mod-install", Progress { id: id.clone(), step, done, total });
        };
        // Mod qui touche à la partie : la sauvegarde est copiée avant (rien si déjà actif).
        let mut last_backup = None;
        if let Some(tid) = game_tid(&target) {
            let before = view_for(&app, &target)?;
            if let Some(m) = save_affecting(&before, &id).filter(|m| !(m.installed && m.enabled)) {
                last_backup = crate::mods_saves::backup(&app, &target, tid, &format!("avant {}", m.name), Some(&id))?;
            }
        }
        let outcome = match target.platform.as_str() {
            "switch" => install_switch(&app, &target, base_title_id(parse_tid(target.title_id.as_deref())?), &id, &opts, &emit)?,
            "3ds" => install_ctr(&app, &target, parse_tid(target.title_id.as_deref())?, &id, &opts, &emit)?,
            "nds" => {
                let rom = PathBuf::from(target.rom.as_deref().ok_or("ROM inconnue")?);
                if id == "cheats" {
                    install_nds(&app, &rom, &emit)?;
                    Installed::Done
                } else {
                    install_nds_patch(&app, &rom, &id, &opts, &emit)?
                }
            }
            _ => return Err("console inconnue".into()),
        };
        let with_backup = |mut v: ModsView| {
            v.last_backup = last_backup.clone();
            v
        };
        Ok(match outcome {
            Installed::Variants(variants) => InstallResult { variants, ..Default::default() },
            Installed::Done => InstallResult {
                view: Some(with_backup({
                    let mut v = view_for(&app, &target)?;
                    after_change(&app, &target, &mut v);
                    v
                })),
                ..Default::default()
            },
            Installed::Output(path) => InstallResult { view: Some(view_for(&app, &target)?), output: Some(path.display().to_string()), ..Default::default() },
        })
    })
    .await
}

#[tauri::command]
pub async fn mods_uninstall(target: ModTarget, id: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let offer = game_tid(&target).filter(|_| view_for(&app, &target).ok().and_then(|v| save_affecting(&v, &id)).is_some_and(|m| m.enabled)).and_then(|tid| crate::mods_saves::offer_for(&app, tid, &id));
        match target.platform.as_str() {
            "switch" => uninstall_switch(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &id)?,
            "3ds" => uninstall_ctr(&app, &target, parse_tid(target.title_id.as_deref())?, &id)?,
            "nds" => {
                let rom = PathBuf::from(target.rom.as_deref().ok_or("ROM inconnue")?);
                if id == "cheats" {
                    fs::remove_file(mch_path(&rom)).map_err(|e| e.to_string())?;
                    let backup = rom.with_extension("mch.avant-kaleido");
                    if backup.is_file() {
                        fs::rename(&backup, mch_path(&rom)).map_err(|e| e.to_string())?;
                    }
                } else {
                    let name = nds_view(&app, &rom).mods.into_iter().find(|m| m.id == id).map(|m| m.name).ok_or("mod inconnu")?;
                    fs::remove_file(patched_rom_path(&rom, &name)).map_err(|e| e.to_string())?;
                }
            }
            _ => return Err("console inconnue".into()),
        }
        let mut view = view_for(&app, &target)?;
        after_change(&app, &target, &mut view);
        view.restore_offer = offer;
        Ok(view)
    })
    .await
}

/// Met de côté ou réactive un mod installé par Kaleido.
#[tauri::command]
pub async fn mods_toggle(target: ModTarget, id: String, enabled: bool, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let tid = game_tid(&target).ok_or("pas possible pour cette console")?;
        let before = view_for(&app, &target)?;
        let affecting = save_affecting(&before, &id);
        let mut last_backup = None;
        if let Some(m) = affecting.as_ref().filter(|_| enabled) {
            last_backup = crate::mods_saves::backup(&app, &target, tid, &format!("avant {}", m.name), Some(&id))?;
        }
        toggle_key(&app, &target, tid, &format!("id:{id}"), enabled)?;
        let mut view = view_for(&app, &target)?;
        after_change(&app, &target, &mut view);
        view.last_backup = last_backup;
        if !enabled && affecting.is_some() {
            view.restore_offer = crate::mods_saves::offer_for(&app, tid, &id);
        }
        Ok(view)
    })
    .await
}

/// Remet une copie de la sauvegarde (la partie actuelle est copiée avant).
#[tauri::command]
pub async fn mods_save_restore(target: ModTarget, id: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let tid = game_tid(&target).ok_or("pas de sauvegarde gérée pour cette console")?;
        crate::mods_saves::restore(&app, &target, tid, &id)?;
        view_for(&app, &target)
    })
    .await
}

/// Enregistre les mods actifs comme profil `name` (remplacé s'il existe) et le rend actif.
#[tauri::command]
pub async fn mods_profile_save(target: ModTarget, name: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let tid = game_tid(&target).ok_or("pas de profils pour cette console")?;
        let name = name.trim().to_string();
        if name.is_empty() || name == crate::mods_saves::ORIGIN || name.chars().count() > 60 {
            return Err("nom de profil invalide".into());
        }
        let view = view_for(&app, &target)?;
        let mods = crate::mods_saves::active_keys(&view);
        let own_save = view.mods.iter().any(|m| m.installed && m.enabled && crate::mods_saves::affects_save(m));
        let mut g = crate::mods_saves::game_profiles(&app, tid);
        g.profiles.retain(|p| p.name != name);
        g.profiles.push(crate::mods_saves::Profile { name: name.clone(), mods, own_save });
        g.profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        g.active = Some(name);
        crate::mods_saves::set_game_profiles(&app, tid, g)?;
        view_for(&app, &target)
    })
    .await
}

#[tauri::command]
pub async fn mods_profile_delete(target: ModTarget, name: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        let tid = game_tid(&target).ok_or("pas de profils pour cette console")?;
        let mut g = crate::mods_saves::game_profiles(&app, tid);
        g.profiles.retain(|p| p.name != name);
        if g.active.as_deref() == Some(name.as_str()) {
            g.active = None;
        }
        crate::mods_saves::set_game_profiles(&app, tid, g)?;
        view_for(&app, &target)
    })
    .await
}

/// Passe au profil `name` (« Jeu d'origine » = aucun mod) : sauvegardes échangées si l'un
/// des deux profils garde la sienne, puis mods activés ou mis de côté.
#[tauri::command]
pub async fn mods_profile_apply(target: ModTarget, name: String, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        use crate::mods_saves::ORIGIN;
        let tid = game_tid(&target).ok_or("pas de profils pour cette console")?;
        let mut g = crate::mods_saves::game_profiles(&app, tid);
        let from = g.active.clone().unwrap_or_else(|| ORIGIN.to_string());
        let wanted: Vec<String> = if name == ORIGIN { vec![] } else { g.profiles.iter().find(|p| p.name == name).ok_or("profil introuvable")?.mods.clone() };
        let own = |n: &str| g.profiles.iter().any(|p| p.name == n && p.own_save);
        let before = view_for(&app, &target)?;
        let (enable, disable, missing) = crate::mods_saves::plan(&before, &wanted);
        // Mods de partie mis de côté : leur copie d'avant est proposée si le profil choisi n'a
        // pas encore de sauvegarde à lui.
        let leaving_save_mods: Vec<String> = disable.iter().filter_map(|k| k.strip_prefix("id:")).filter(|id| save_affecting(&before, id).is_some()).map(str::to_string).collect();
        let mut swapped = false;
        if from != name && (own(&from) || own(&name) || !leaving_save_mods.is_empty()) {
            swapped = crate::mods_saves::swap_saves(&app, &target, tid, &from, &name)?;
        }
        for k in &disable {
            toggle_key(&app, &target, tid, k, false)?;
        }
        for k in &enable {
            toggle_key(&app, &target, tid, k, true)?;
        }
        g.active = (name != ORIGIN).then_some(name);
        crate::mods_saves::set_game_profiles(&app, tid, g)?;
        let mut view = view_for(&app, &target)?;
        after_change(&app, &target, &mut view);
        view.missing = missing;
        if !swapped {
            view.restore_offer = leaving_save_mods.iter().find_map(|id| crate::mods_saves::offer_for(&app, tid, id));
        }
        Ok(view)
    })
    .await
}

/// Installe la mise à jour séparée d'un jeu dans la NAND d'Eden (progression sous `id`).
fn install_update_for(app: &AppHandle, target: &ModTarget, id: &str) -> Result<(), String> {
    if crate::library::running_emulators().contains(&"Eden") {
        return Err("Ferme Eden d'abord : il ne relit sa NAND qu'au démarrage.".into());
    }
    let update = PathBuf::from(target.update_file.as_deref().ok_or("pas de mise à jour séparée pour ce jeu")?);
    let keys_path = crate::switch::prod_keys(app).ok_or("clés de la console (prod.keys) introuvables")?;
    let keys = kaleido_core::nx::Keys::load(&keys_path).map_err(|e| e.to_string())?;
    let (registered, keys_dir) = eden_nand(app).ok_or("dossier d'Eden introuvable")?;
    let emit = |done: u64, total: u64| {
        let _ = app.emit("mod-install", Progress { id: id.into(), step: "install", done, total });
    };
    kaleido_core::nx::install_update_to_nand(&update, &registered, &keys_dir, &keys, emit).map_err(|e| e.to_string())?;
    Ok(())
}

/// Switch : installe la mise à jour séparée du jeu dans la NAND d'Eden, comme son menu
/// « Installer des fichiers dans la NAND ».
#[tauri::command]
pub async fn mods_install_update(target: ModTarget, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        if target.platform != "switch" {
            return Err("seulement pour les jeux Switch".into());
        }
        install_update_for(&app, &target, "nand-update")?;
        view_for(&app, &target)
    })
    .await
}

/// Jeu Switch de la bibliothèque, tel que l'interface le connaît.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchGameRef {
    pub path: String,
    pub title_id: String,
    pub title: String,
    #[serde(default)]
    pub update_path: Option<String>,
    #[serde(default)]
    pub update_version: Option<String>,
}

impl SwitchGameRef {
    fn target(&self) -> ModTarget {
        ModTarget { platform: "switch".into(), title_id: Some(self.title_id.clone()), rom: Some(self.path.clone()), game_version: self.update_version.clone(), update_file: self.update_path.clone() }
    }
}

/// Jeu dont Eden ne lance pas la dernière mise à jour trouvée.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleUpdate {
    pub title: String,
    pub path: String,
    /// Fichier de la mise à jour à installer.
    pub update_file: String,
    pub update_version: Option<String>,
    /// Ce qu'Eden lance à la place.
    pub running: String,
}

/// La mise à jour séparée d'un jeu est-elle masquée (mise à jour du .xci, ou autre version
/// dans la NAND) ? Rend ce qu'Eden lance à la place.
fn stale_update(app: &AppHandle, target: &ModTarget) -> Option<String> {
    let ids = exe_ids(app, target).ok()?;
    let separate = ids.separate.as_ref()?;
    if *separate == ids.running {
        return None;
    }
    if ids.nand.is_some() {
        Some("une autre version installée dans Eden".into())
    } else if ids.embedded.is_some() {
        Some("la mise à jour contenue dans le fichier du jeu".into())
    } else {
        None
    }
}

/// Bibliothèque : jeux Switch dont Eden ne lance pas la dernière mise à jour.
#[tauri::command]
pub async fn switch_updates_check(games: Vec<SwitchGameRef>, app: AppHandle) -> Result<Vec<StaleUpdate>, String> {
    crate::blocking(move || {
        Ok(games
            .iter()
            .filter(|g| g.update_path.is_some())
            .filter_map(|g| {
                let running = stale_update(&app, &g.target())?;
                let file = g.update_path.as_deref().and_then(|u| Path::new(u).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                Some(StaleUpdate { title: g.title.clone(), path: g.path.clone(), update_file: file, update_version: g.update_version.clone(), running })
            })
            .collect())
    })
    .await
}

/// Bibliothèque : installe dans Eden la dernière mise à jour de chacun de ces jeux.
#[tauri::command]
pub async fn switch_updates_install(games: Vec<SwitchGameRef>, app: AppHandle) -> Result<u32, String> {
    crate::blocking(move || {
        let mut done = 0;
        for g in &games {
            install_update_for(&app, &g.target(), &format!("nand:{}", g.path))?;
            done += 1;
        }
        Ok(done)
    })
    .await
}

/// Switch : ordre de priorité des mods actifs (dossiers, le premier l'emporte).
#[tauri::command]
pub async fn mods_reorder(target: ModTarget, order: Vec<String>, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        if target.platform != "switch" {
            return Err("l'ordre ne se règle que pour les jeux Switch".into());
        }
        reorder_switch(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &order)?;
        let mut view = view_for(&app, &target)?;
        after_change(&app, &target, &mut view);
        Ok(view)
    })
    .await
}

/// Switch : met de côté ou remet en place un mod installé hors de Kaleido.
#[tauri::command]
pub async fn mods_toggle_other(target: ModTarget, name: String, enabled: bool, app: AppHandle) -> Result<ModsView, String> {
    crate::blocking(move || {
        toggle_folder(&app, base_title_id(parse_tid(target.title_id.as_deref())?), &name, enabled)?;
        let mut view = view_for(&app, &target)?;
        after_change(&app, &target, &mut view);
        Ok(view)
    })
    .await
}

/// Explorateur : mods GameBanana d'un jeu.
#[tauri::command]
pub async fn mods_browse(game: u32, query: String, sort: String, category: Option<u32>, page: u32, app: AppHandle) -> Result<crate::gamebanana::GbPage, String> {
    crate::blocking(move || crate::gamebanana::browse(&app, game, &query, &sort, category, page)).await
}

#[tauri::command]
pub async fn mods_categories(game: u32, app: AppHandle) -> Result<Vec<crate::gamebanana::GbCategory>, String> {
    crate::blocking(move || {
        Ok(crate::gamebanana::categories(&app, game)?.into_iter().map(|mut c| {
            c.name = crate::gamebanana::category_fr(&c.name);
            c
        }).collect())
    })
    .await
}

/// Fiche détaillée d'un mod GameBanana (description, fichiers, images).
#[tauri::command]
pub async fn mods_details(id: u32, app: AppHandle) -> Result<crate::gamebanana::GbProfile, String> {
    crate::blocking(move || crate::gamebanana::profile(&app, id)).await
}

// ---------------------------------------------------------------------------
// Mises à jour en attente, pour la bibliothèque

/// Mods installés par Kaleido d'un jeu : (title ID, marqueur).
fn kaleido_markers(app: &AppHandle) -> Vec<(u64, Marker)> {
    let mut out = Vec::new();
    // Switch : dossiers actifs et mis de côté.
    let eden = resolve(EmulatorId::Eden, app);
    if let Some(load) = eden_load_dir(&eden) {
        for game in fs::read_dir(&load).into_iter().flatten().flatten() {
            let Some(tid) = u64::from_str_radix(&game.file_name().to_string_lossy(), 16).ok() else { continue };
            let parked = disabled_dir(app, tid).map(|d| installed_dirs(&d)).unwrap_or_default();
            for (_, m) in installed_dirs(&game.path()).into_iter().chain(parked) {
                if let Some(m) = m.filter(|m| !m.id.starts_with("kaleido:")) {
                    out.push((tid, m));
                }
            }
        }
    }
    // 3DS : mods LayeredFS fusionnés.
    if let Some(user) = ctr_emulator(app).ctr_user_dir() {
        for game in fs::read_dir(user.join("load").join("mods")).into_iter().flatten().flatten() {
            let Some(tid) = u64::from_str_radix(&game.file_name().to_string_lossy(), 16).ok() else { continue };
            for m in read_manifest(&game.path()).mods {
                out.push((tid, m.marker));
            }
        }
    }
    out
}

/// Nombre de mods à mettre à jour par jeu (title ID en hexadécimal). S'appuie sur les caches
/// (fiches GameBanana 6 h, archives Fl4sh 1 jour) : léger au démarrage de la bibliothèque.
fn pending_updates(app: &AppHandle) -> BTreeMap<String, u32> {
    let markers = kaleido_markers(app);
    let gb_ids: Vec<u32> = markers.iter().filter_map(|(_, m)| m.gb).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let profiles = gb_profiles(app, &gb_ids);
    let nexus_keys: Vec<(String, u32)> = markers.iter().filter(|(_, m)| m.nexus_version.is_some()).filter_map(|(_, m)| crate::nexus::parse_id(&m.id)).collect();
    let nexus = crate::nexus::mods(app, &nexus_keys);
    let mut fl4sh: BTreeMap<u64, Vec<ZipMod>> = BTreeMap::new();
    let mut out: BTreeMap<String, u32> = BTreeMap::new();
    for (tid, m) in &markers {
        let outdated = if let Some(gb) = m.gb {
            let prefix = crate::mods_catalog::find(app, &tid_key(*tid), &m.id).and_then(|c| c.file);
            match profiles.get(&gb) {
                Some(Ok((_, files))) => pick_gb_file(files, None, prefix.as_deref()).is_some_and(|f| m.version.as_deref() != Some(f._sFile.as_str()) && m.file != Some(f._idRow)),
                _ => false,
            }
        } else if let Some(key) = m.id.strip_prefix("fl4sh:") {
            let mods = fl4sh.entry(*tid).or_insert_with(|| {
                fl4sh_archive(app, *tid).and_then(|(a, sha)| fl4sh_zip(app, &a, &sha).ok()).and_then(|z| zip_listing(&z).ok()).map(|l| zip_mods(&l, *tid)).unwrap_or_default()
            });
            mods.iter().find(|z| mod_key(&split_version(&z.folder).0) == key).is_some_and(|z| split_version(&z.folder).1 != m.version)
        } else if let (Some(have), Some(k)) = (m.nexus_version.as_deref(), crate::nexus::parse_id(&m.id)) {
            nexus.get(&k).and_then(|n| n.version.as_deref()).is_some_and(|now| now != have)
        } else {
            false
        };
        if outdated {
            *out.entry(format!("{tid:016X}")).or_default() += 1;
        }
    }
    out
}

/// Mods à mettre à jour, par jeu (title ID), pour la bibliothèque.
#[tauri::command]
pub async fn mods_pending_updates(app: AppHandle) -> Result<BTreeMap<String, u32>, String> {
    crate::blocking(move || Ok(pending_updates(&app))).await
}

/// Dossier Téléchargements de l'utilisateur (sélecteur de fichier des mods manuels).
#[tauri::command]
pub fn mods_downloads_dir(app: AppHandle) -> Option<String> {
    app.path().download_dir().ok().map(|p| p.display().to_string())
}

// ---------------------------------------------------------------------------
// Surveillance du dossier Téléchargements (mods à télécharger soi-même)

/// Numéro de la surveillance en cours : en lancer une nouvelle (ou l'arrêter) clôt la précédente.
static WATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadFound {
    id: String,
    /// Fichier terminé, ou `None` à la fin du délai.
    path: Option<String>,
}

/// Fichiers terminés de `dir` apparus ou modifiés après `since`, avec l'une des extensions.
/// Un fichier en cours (`.crdownload`, `.part` à côté, taille nulle) est écarté.
pub fn finished_downloads(dir: &Path, since: SystemTime, extensions: &[String]) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if !extensions.iter().any(|x| x.eq_ignore_ascii_case(&ext)) {
            continue;
        }
        // Firefox écrit dans « nom.part » en laissant un fichier vide au nom final.
        let mut part = p.clone().into_os_string();
        part.push(".part");
        if Path::new(&part).exists() {
            continue;
        }
        let changed = meta.modified().ok().max(meta.created().ok());
        if changed.is_some_and(|t| t >= since) {
            out.push((p, meta.len()));
        }
    }
    out
}

/// Attend qu'un fichier téléchargé arrive dans Téléchargements (30 minutes au plus) et le
/// signale par l'évènement `mods-download`.
#[tauri::command]
pub fn mods_watch_downloads(id: String, extensions: Vec<String>, app: AppHandle) -> Result<(), String> {
    let dir = app.path().download_dir().map_err(|_| "dossier Téléchargements introuvable".to_string())?;
    let ticket = WATCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    // Petite marge : le navigateur peut dater le fichier un peu avant le clic.
    let since = SystemTime::now() - Duration::from_secs(5);
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(30 * 60);
        let mut last: Vec<(PathBuf, u64)> = Vec::new();
        while WATCH.load(std::sync::atomic::Ordering::SeqCst) == ticket {
            if std::time::Instant::now() > deadline {
                let _ = app.emit("mods-download", DownloadFound { id: id.clone(), path: None });
                return;
            }
            let now = finished_downloads(&dir, since, &extensions);
            // Taille identique d'un passage à l'autre : le fichier est terminé.
            if let Some((p, _)) = now.iter().find(|f| last.contains(f)) {
                WATCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = app.emit("mods-download", DownloadFound { id: id.clone(), path: Some(p.display().to_string()) });
                return;
            }
            last = now;
            std::thread::sleep(Duration::from_millis(1500));
        }
    });
    Ok(())
}

#[tauri::command]
pub fn mods_watch_stop() {
    WATCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
    fn variants_and_roots() {
        let dir = std::env::temp_dir().join(format!("kaleido-roots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for p in ["Option A/romfs/a.bin", "Option B/0100000011D90000/romfs/a.bin", "atmosphere/contents/010018E011D92000/exefs/x.ips", "readme/notes.txt"] {
            let f = dir.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, b"x").unwrap();
        }
        let roots = mod_roots(&dir);
        assert_eq!(roots, vec!["Option A", "Option B/0100000011D90000", "atmosphere/contents/010018E011D92000"]);
        let tid = 0x0100000011D90000;
        assert_eq!(choose_root(&roots, Some("Option A"), None, tid), Ok("Option A".to_string()));
        assert_eq!(choose_root(&roots, None, Some("option b"), tid), Ok("Option B/0100000011D90000".to_string()));
        assert_eq!(choose_root(&roots, None, None, tid), Ok("Option B/0100000011D90000".to_string()));
        // Variante d'un autre jeu de la paire écartée.
        let pair = vec![roots[0].clone(), roots[2].clone()];
        assert_eq!(choose_root(&pair, None, None, tid), Ok("Option A".to_string()));
        assert!(choose_root(&["A".to_string(), "B".to_string()], None, None, tid).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn game_versions() {
        assert_eq!(version_warning(Some("1.3.0"), Some("1.3.0")), None);
        assert_eq!(version_warning(Some("1.1.3/1.2.0/1.3.0"), Some("1.3.0")), None);
        assert_eq!(version_warning(Some("4.0.0+"), Some("4.0.1")), None);
        assert!(version_warning(Some("1.1.3"), Some("1.3.0")).is_some());
        assert_eq!(version_warning(Some("1.3.0"), None), None);
        assert_eq!(version_warning(Some("toutes"), Some("1.3.0")), None);
    }

    #[test]
    fn overlapping_files() {
        let dirs = vec![("A".to_string(), vec!["romfs/x".to_string(), "romfs/y".into()]), ("B".into(), vec!["romfs/y".into()]), ("C".into(), vec!["romfs/z".into()])];
        let map = overlap_map(&dirs);
        assert_eq!(map.get("A"), Some(&vec!["B".to_string()]));
        assert_eq!(map.get("B"), Some(&vec!["A".to_string()]));
        assert!(!map.contains_key("C"));
    }

    #[test]
    fn gb_file_choice() {
        let f = |id: u64, name: &str, date: i64| crate::gamebanana::RawFile {
            _idRow: id,
            _sFile: name.into(),
            _nFilesize: 1,
            _tsDateAdded: date,
            _sDownloadUrl: String::new(),
            _sMd5Checksum: String::new(),
            _sDescription: String::new(),
            _nDownloadCount: 0,
            _sAvResult: String::new(),
        };
        let files = vec![f(1, "mod_113.zip", 10), f(2, "mod_130.zip", 20), f(3, "mod_ryujinx.zip", 30)];
        assert_eq!(pick_gb_file(&files, None, None).unwrap()._idRow, 2);
        assert_eq!(pick_gb_file(&files, None, Some("mod_113")).unwrap()._idRow, 1);
        assert_eq!(pick_gb_file(&files, Some(3), None).unwrap()._idRow, 3);
    }

    #[test]
    fn nds_header_crc() {
        let mut h = vec![0u8; 0x200];
        h[..12].copy_from_slice(b"POKEMON PL\0\0");
        assert!(!nds_header_ok(&h));
        let mut crc = 0xFFFFu16;
        for &b in &h[..0x15E] {
            crc ^= b as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
            }
        }
        h[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        assert!(nds_header_ok(&h));
    }

    #[test]
    fn exefs_verdicts() {
        let update = "AEE8F150DDA1B5A838806E1A5EA6827AD9F3C51E";
        let base = "7FCAD279539DE183B25C11834FD4A030591CFE25";
        assert_eq!(exefs_verdict(&[], update, Some(base)), None);
        assert_eq!(exefs_verdict(&[update.into()], update, Some(base)), Some("ok"));
        assert_eq!(exefs_verdict(&[base.into()], update, Some(base)), Some("base"));
        assert_eq!(exefs_verdict(&["0123456789ABCDEF".into()], update, Some(base)), Some("other"));
        // Un dossier qui couvre plusieurs versions est bon dès qu'un correctif correspond.
        assert_eq!(exefs_verdict(&[base.into(), update.into()], update, Some(base)), Some("ok"));

        let dir = std::env::temp_dir().join(format!("kaleido-exefs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("exefs")).unwrap();
        fs::write(dir.join("exefs").join("1.1.1.pchtxt"), format!("@nsobid-{update}\n@enabled\n")).unwrap();
        fs::write(dir.join("exefs").join(format!("{base}.ips")), b"PATCHEOF").unwrap();
        let mut ids = dir_patch_ids(&dir);
        ids.sort();
        assert_eq!(ids, vec![base.to_string(), update.to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugin_loader_setting() {
        let dir = std::env::temp_dir().join(format!("kaleido-plg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("config")).unwrap();
        fs::write(dir.join("config").join("qt-config.ini"), "[System]\r\nplugin_loader\\default=true\r\nplugin_loader=false\r\n").unwrap();
        tuning::set_azahar_plugin_loader(&dir, true).unwrap();
        let text = fs::read_to_string(dir.join("config").join("qt-config.ini")).unwrap();
        assert_eq!(tuning::get_qt(&text, "System", "plugin_loader").as_deref(), Some("true"));
        assert_eq!(tuning::get_qt(&text, "System", "plugin_loader\\default").as_deref(), Some("false"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_detection() {
        let dir = std::env::temp_dir().join(format!("kaleido-dl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let since = SystemTime::now() - Duration::from_secs(5);
        fs::write(dir.join("mod.zip"), b"PK").unwrap();
        fs::write(dir.join("lumi.7z"), b"").unwrap();
        fs::write(dir.join("lumi.7z.part"), b"7z").unwrap();
        fs::write(dir.join("other.crdownload"), b"x").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        let exts: Vec<String> = ["zip", "7z", "rar"].iter().map(|s| s.to_string()).collect();
        let found = finished_downloads(&dir, since, &exts);
        assert_eq!(found.iter().map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>(), vec!["mod.zip"]);
        assert!(finished_downloads(&dir, SystemTime::now() + Duration::from_secs(60), &exts).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn us_rom_check() {
        assert_eq!(us_rom_problem("CPUE", 0), None);
        assert_eq!(us_rom_problem("CPUE", 1), None);
        assert_eq!(us_rom_problem("IRBO", 0), None);
        assert!(us_rom_problem("CPUE", 10).unwrap().contains("européenne en anglais"));
        assert!(us_rom_problem("CPUF", 0).unwrap().contains("française (code CPUF"));
        assert!(us_rom_problem("IPKF", 0).unwrap().contains("code IPKE"));
    }

    #[test]
    fn priority_names() {
        assert_eq!(strip_rank("01 HD Texture Overhaul v0.5"), "HD Texture Overhaul v0.5");
        assert_eq!(strip_rank("60 FPS"), "60 FPS");
        assert_eq!(strip_rank("1 seul"), "1 seul");
        assert_eq!(strip_rank("06 60 FPS v1.1.1"), "06 60 FPS v1.1.1");
        let order = vec![("03 Draw Distance".to_string(), None), ("Water".to_string(), None), ("01 HD Textures".to_string(), None)];
        assert_eq!(rank_names(&order), vec![
            ("03 Draw Distance".to_string(), "01 Draw Distance".to_string()),
            ("Water".to_string(), "02 Water".to_string()),
            ("01 HD Textures".to_string(), "03 HD Textures".to_string()),
        ]);
        // Déjà dans l'ordre : rien à renommer.
        assert!(rank_names(&[("01 A".to_string(), None), ("02 B".to_string(), None)]).is_empty());
        // Mod de Kaleido « 60 FPS » : nom connu par le marqueur, jamais pris pour un rang.
        assert_eq!(rank_names(&[("Water".into(), None), ("60 FPS".into(), Some("60 FPS".into()))]), vec![("Water".to_string(), "01 Water".to_string()), ("60 FPS".to_string(), "02 60 FPS".to_string())]);
        assert!(rank_names(&[("01 60 FPS".into(), Some("60 FPS".into()))]).is_empty());
        // Eden compare les noms octet par octet : un rang « 01 » passe avant toute lettre.
        let mut names = vec!["Water".to_string(), "02 B".to_string(), "01 A".to_string(), "Ash".to_string()];
        names.sort();
        assert_eq!(names, vec!["01 A", "02 B", "Ash", "Water"]);
    }

    #[test]
    fn folder_names_are_valid() {
        assert_eq!(folder_name("Écran ultra-large 21:9"), "Écran ultra-large 21-9");
        assert_eq!(folder_name("Mod. "), "Mod");
    }

    /// 60 fps natif sur un dossier Azahar temporaire et la vraie ROM (ignoré sans ROM) :
    /// installation par-dessus le taux de chromatiques, réinstallation, retrait, garde-fous.
    #[test]
    fn fps60_install_uninstall() {
        let dir = std::env::var_os("KALEIDO_3DS_DIR").map(PathBuf::from).or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents").join("NDS & 3DS")));
        let rom = dir.and_then(|d| fs::read_dir(d).ok()).into_iter().flatten().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains("Omega Ruby") && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("3ds")));
        let Some(rom) = rom else {
            eprintln!("pas de ROM Rubis Oméga : test ignoré");
            return;
        };
        let tid = 0x0004_0000_0011_C400u64;
        let user = std::env::temp_dir().join(format!("kaleido-fps60-{}", std::process::id()));
        let _ = fs::remove_dir_all(&user);
        let mods = ctr_mods_dir(&user, tid);
        fs::create_dir_all(&mods).unwrap();
        let shiny = include_bytes!("../../../ctr-smooth/proto/kaleido-shiny-or-eur-v1.0.ips");
        fs::write(mods.join("code.ips"), shiny).unwrap();
        let target = ModTarget { platform: "3ds".into(), title_id: Some(format!("{tid:016X}")), rom: Some(rom.display().to_string()), game_version: None, update_file: None };
        let game = kaleido_core::CtrGameRom::open(&rom).unwrap();
        let original = game.code().unwrap().code;
        let profile = fps60_ctr::profile_for(tid, &original).unwrap();
        assert!(fps60_entry(&user, &target, tid).blocked.is_none());

        install_fps60(&user, &target, tid).unwrap();
        let ips_now = fs::read(mods.join("code.ips")).unwrap();
        assert!(fps60_ctr::contains(profile, &original, &ips_now).unwrap());
        assert!(fps60_entry(&user, &target, tid).installed);
        // réinstallation : même résultat
        install_fps60(&user, &target, tid).unwrap();
        assert_eq!(fs::read(mods.join("code.ips")).unwrap(), ips_now);
        // retrait : le taux de chromatiques revient tel quel
        uninstall_fps60(&user, &target, tid).unwrap();
        let back = fs::read(mods.join("code.ips")).unwrap();
        let (mut a, mut b) = (original.clone(), original.clone());
        kaleido_core::formats::ips::apply(&mut a, &back).unwrap();
        kaleido_core::formats::ips::apply(&mut b, shiny).unwrap();
        assert_eq!(a, b);
        assert!(!mods.join(FPS60_MARKER).exists());
        // sans autre patch : le code.ips disparaît au retrait
        fs::remove_file(mods.join("code.ips")).unwrap();
        install_fps60(&user, &target, tid).unwrap();
        uninstall_fps60(&user, &target, tid).unwrap();
        assert!(!mods.join("code.ips").exists());
        // code de triche « 60 FPS » actif : refus
        let cheats = user.join("cheats");
        fs::create_dir_all(&cheats).unwrap();
        fs::write(cheats.join(format!("{tid:016X}.txt")), "[60 FPS]
*citra_enabled
D3000000 00000000
").unwrap();
        assert!(install_fps60(&user, &target, tid).unwrap_err().contains("60 FPS"));
        fs::remove_dir_all(&cheats).unwrap();
        // mise à jour installée : bloqué
        let upd = user.join("sdmc/Nintendo 3DS").join("0".repeat(32)).join("0".repeat(32)).join("title/0004000e/0011c400/content");
        fs::create_dir_all(&upd).unwrap();
        assert!(fps60_entry(&user, &target, tid).blocked.is_some());
        assert!(install_fps60(&user, &target, tid).is_err());
        let _ = fs::remove_dir_all(&user);
    }
}
