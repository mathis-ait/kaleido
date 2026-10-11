//! Romhacks : installation en un clic (téléchargement du patch officiel, application
//! sur la ROM du joueur, traduction française facultative). Données : `kaleido_core::romhack`.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use kaleido_core::romhack::{self, HackDef};
use kaleido_core::formats::nds::NdsRom;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::library::{download_to, USER_AGENT};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HackView {
    id: &'static str,
    name: &'static str,
    version: &'static str,
    author: &'static str,
    summary: &'static str,
    thread_url: &'static str,
    notes: &'static [&'static str],
    base_label: &'static str,
    /// ROM d'origine trouvée parmi les jeux de la bibliothèque (SHA-1 vérifié).
    base: Option<PathBuf>,
    /// ROMs du bon jeu mais pas de la bonne version (SHA-1 différent).
    base_mismatch: Vec<PathBuf>,
    /// Traduction française proposée.
    french: bool,
    reference_label: Option<&'static str>,
    reference: Option<PathBuf>,
    /// ROMs déjà installées (anglais / français).
    installed_en: Option<PathBuf>,
    installed_fr: Option<PathBuf>,
    /// Ajustements Kaleido (ex. taux de chromatiques) et ROMs installées qui ne les ont pas encore.
    tweaks: Option<&'static str>,
    outdated: Vec<PathBuf>,
}

/// SHA-1 déjà calculés : (chemin, taille, date) → empreinte.
static SHA1_CACHE: LazyLock<Mutex<HashMap<(PathBuf, u64, u64), String>>> = LazyLock::new(Default::default);

fn game_code(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut h = [0u8; 0x10];
    f.read_exact(&mut h).ok()?;
    romhack::nds_game_code(&h)
}

fn file_sha1(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let key = (path.to_path_buf(), meta.len(), mtime);
    if let Some(h) = SHA1_CACHE.lock().unwrap().get(&key) {
        return Some(h.clone());
    }
    let h = romhack::sha1_hex(&fs::read(path).ok()?);
    SHA1_CACHE.lock().unwrap().insert(key, h.clone());
    Some(h)
}


fn view(def: &'static HackDef, roms: &[PathBuf]) -> HackView {
    let mut base = None;
    let mut base_mismatch = Vec::new();
    let mut reference = None;
    let t = def.translation;
    for p in roms.iter().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nds"))) {
        let Some(code) = game_code(p) else { continue };
        if code == def.base_code && base.is_none() {
            if file_sha1(p).as_deref() == Some(def.base_sha1) {
                base = Some(p.clone());
            } else {
                base_mismatch.push(p.clone());
            }
        } else if reference.is_none() && t.is_some_and(|t| t.reference_codes.contains(&code.as_str())) {
            reference = Some(p.clone());
        }
    }
    let installed = |french: bool| {
        roms.iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy() == romhack::output_name(def, french))).cloned().or_else(|| {
            let p = base.as_ref()?.with_file_name(romhack::output_name(def, french));
            p.exists().then_some(p)
        })
    };
    let (installed_en, installed_fr) = (installed(false), installed(true));
    let outdated = [&installed_en, &installed_fr].into_iter().flatten().filter(|p| needs_tweaks(def, p)).cloned().collect();
    HackView {
        id: def.id,
        name: def.name,
        version: def.version,
        author: def.author,
        summary: def.summary,
        thread_url: def.thread_url,
        notes: def.notes,
        base_label: def.base_label,
        base_mismatch: if base.is_some() { Vec::new() } else { base_mismatch },
        french: t.is_some(),
        reference_label: t.map(|t| t.reference_label),
        reference,
        installed_en,
        installed_fr,
        tweaks: def.tweaks.map(|t| t.label),
        outdated,
        base,
    }
}

/// ROM installée par une version précédente de Kaleido, sans les ajustements actuels.
fn needs_tweaks(def: &HackDef, rom: &Path) -> bool {
    fs::File::open(rom).ok().is_some_and(|mut f| romhack::tweaks_applied(&mut f, def).is_ok_and(|done| !done))
}

/// ROMs de romhack de la bibliothèque à mettre à jour (vérification légère, au démarrage).
#[tauri::command]
pub async fn romhacks_outdated(roms: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    crate::blocking(move || {
        Ok(roms
            .into_iter()
            .filter(|p| p.file_name().and_then(|n| romhack::from_file_name(&n.to_string_lossy())).is_some_and(|(def, _)| needs_tweaks(def, p)))
            .collect())
    })
    .await
}

/// Applique les ajustements Kaleido à une ROM déjà installée, sur place (la sauvegarde
/// n'est pas touchée).
#[tauri::command]
pub async fn romhack_update(path: PathBuf) -> Result<(), String> {
    crate::blocking(move || {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (def, _) = romhack::from_file_name(&name).ok_or("ROM de romhack non reconnue")?;
        let mut f = fs::OpenOptions::new().read(true).write(true).open(&path).map_err(|e| format!("{} : {e}", path.display()))?;
        romhack::apply_tweaks(&mut f, def).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

/// Romhacks proposés, avec ce qui manque pour les installer. `roms` : jeux de la bibliothèque.
#[tauri::command]
pub async fn romhacks_list(roms: Vec<PathBuf>) -> Result<Vec<HackView>, String> {
    crate::blocking(move || Ok(romhack::HACKS.iter().map(|d| view(d, &roms)).collect())).await
}

#[derive(Clone, Serialize)]
struct Progress {
    id: String,
    step: &'static str,
    done: u64,
    total: u64,
}

/// Lien direct d'une page de téléchargement MediaFire.
fn mediafire_direct(page: &str) -> Result<String, String> {
    let html = ureq::get(page)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("page de téléchargement inaccessible : {e}"))?
        .into_string()
        .map_err(|e| e.to_string())?;
    let start = html.find("href=\"https://download").ok_or("lien de téléchargement introuvable sur la page MediaFire (le fichier a peut-être été retiré)")?;
    let rest = &html[start + 6..];
    Ok(rest[..rest.find('"').ok_or("lien MediaFire illisible")?].to_string())
}

/// Patch officiel : archive téléchargée une fois puis gardée en cache.
fn fetch_patch(app: &AppHandle, def: &HackDef, emit: &dyn Fn(&'static str, u64, u64)) -> Result<Vec<u8>, String> {
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("romhacks");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let archive = dir.join(format!("{}-{}.zip", def.id, def.version));
    if !archive.exists() {
        emit("download", 0, 0);
        let url = if def.download_page.contains("mediafire.com") { mediafire_direct(def.download_page)? } else { def.download_page.to_string() };
        download_to(&url, &archive, 0, |d, t| emit("download", d, t))?;
    }
    emit("extract", 0, 1);
    let read = || -> Result<Vec<u8>, String> {
        let mut zip = zip::ZipArchive::new(fs::File::open(&archive).map_err(|e| e.to_string())?).map_err(|e| format!("archive illisible : {e}"))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.name().to_ascii_lowercase().ends_with(&format!(".{}", def.patch_ext)) {
                let mut data = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
                return Ok(data);
            }
        }
        Err(format!("aucun fichier .{} dans l'archive du hack", def.patch_ext))
    };
    read().inspect_err(|_| {
        // Archive abîmée ou remplacée : on la retéléchargera la prochaine fois.
        let _ = fs::remove_file(&archive);
    })
}

/// Installe un romhack : patch officiel sur `base`, puis traduction française si `reference`
/// (même jeu en français) est fourni. Renvoie le chemin de la ROM créée.
#[tauri::command]
pub async fn romhack_install(id: String, base: PathBuf, reference: Option<PathBuf>, app: AppHandle) -> Result<PathBuf, String> {
    crate::blocking(move || {
        let def = romhack::find(&id).ok_or("romhack inconnu")?;
        let emit = |step: &'static str, done: u64, total: u64| {
            let _ = app.emit("romhack-install", Progress { id: id.clone(), step, done, total });
        };
        let base_bytes = fs::read(&base).map_err(|e| format!("{} : {e}", base.display()))?;
        if romhack::sha1_hex(&base_bytes) != def.base_sha1 {
            return Err(format!("Cette ROM n'est pas une copie intacte de {} : le patch ne peut pas s'y appliquer.", def.base_label));
        }
        let patch = fetch_patch(&app, def, &emit)?;

        emit("patch", 0, 1);
        let patched = romhack::apply_patch(&base_bytes, &patch).map_err(|e| e.to_string())?;
        if romhack::sha1_hex(&patched) != def.patched_sha1 {
            return Err("Le patch a donné un résultat inattendu (version du hack différente ?).".into());
        }

        let (bytes, french) = match (reference, def.translation) {
            (Some(reference), Some(t)) => {
                emit("translate", 0, 1);
                let mut rom = NdsRom::from_bytes(patched).map_err(|e| e.to_string())?;
                let base_rom = NdsRom::from_bytes(base_bytes).map_err(|e| e.to_string())?;
                let reference_rom = NdsRom::open(&reference).map_err(|e| format!("{} : {e}", reference.display()))?;
                let code = reference_rom.header().game_code.clone();
                if !t.reference_codes.contains(&code.as_str()) {
                    return Err(format!("La ROM de référence doit être {}.", t.reference_label));
                }
                let pack = romhack::TextPack::parse(t.pack).map_err(|e| e.to_string())?;
                romhack::translate_gen4(&mut rom, &base_rom, &reference_rom, t.msg_path, &pack).map_err(|e| e.to_string())?;
                (rom.to_bytes().map_err(|e| e.to_string())?, true)
            }
            _ => (patched, false),
        };
        let mut bytes = bytes;
        romhack::apply_tweaks(&mut std::io::Cursor::new(&mut bytes), def).map_err(|e| e.to_string())?;

        emit("write", 0, 1);
        let out = base.with_file_name(romhack::output_name(def, french));
        let part = out.with_extension("nds.part");
        fs::write(&part, &bytes).map_err(|e| format!("{} : {e}", part.display()))?;
        fs::rename(&part, &out).map_err(|e| e.to_string())?;
        Ok(out)
    })
    .await
}

