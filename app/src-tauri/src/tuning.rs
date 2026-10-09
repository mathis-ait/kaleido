//! « Réglages optimaux » : profil graphique adapté à la carte graphique du PC, écrit
//! dans la configuration de l'émulateur (par jeu quand l'émulateur le permet).
//!
//! - **Eden** et **Azahar / Citra / Lime3DS** : configuration par jeu
//!   `<dossier utilisateur>/config/custom/<title ID>.ini` (format Qt : chaque clé est
//!   accompagnée de `clé\use_global=false` et `clé\default=false`, sinon la valeur
//!   par défaut est utilisée à la place de la nôtre). Valeurs des énumérations d'Eden :
//!   src/common/settings_enums.h (`resolution_setup` 8 = ×4, `anti_aliasing` 2 = SMAA,
//!   `scaling_filter` 4 = Lanczos, `max_anisotropy` 5 = ×16…).
//! - **melonDS** : `melonDS.toml` (à côté de l'exécutable, réglages communs à tous les jeux).
//! - **DeSmuME** : `desmume.ini`, section `[3D]`.
//!
//! Avant la première modification, le fichier est copié en `<fichier>.kaleido-backup`
//! (ou noté comme créé par Kaleido) : « Restaurer » remet exactement l'ancien fichier.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::play::{self, EmulatorId, Env, Resolved};

// ---------------------------------------------------------------------------
// Carte graphique

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Low,
    Medium,
    High,
    Ultra,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Low => "Économie",
            Tier::Medium => "Équilibré",
            Tier::High => "Qualité",
            Tier::Ultra => "Qualité maximale",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    /// Mémoire vidéo dédiée, en Mo.
    pub vram_mb: u64,
    pub tier: Tier,
}

/// Puces graphiques intégrées : peu de mémoire dédiée, réglages prudents.
fn is_integrated(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("intel") && !n.contains("arc")) || n.contains("radeon(tm) graphics") || n.contains("radeon graphics") || n.contains("vega")
}

pub fn tier_of(name: &str, vram_mb: u64) -> Tier {
    if is_integrated(name) {
        return Tier::Low;
    }
    match vram_mb {
        v if v >= 12 * 1024 => Tier::Ultra,
        v if v >= 8 * 1024 => Tier::High,
        v if v >= 4 * 1024 => Tier::Medium,
        _ => Tier::Low,
    }
}

/// Analyse la sortie de `reg query … /s /v <valeur>` : (clé, valeur) pour chaque adaptateur.
fn parse_reg(output: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut key = String::new();
    for line in output.lines() {
        if line.starts_with("HKEY_") {
            key = line.trim().to_string();
        } else if let Some(pos) = line.find("REG_") {
            let rest = &line[pos..];
            let value = rest.split_once(char::is_whitespace).map_or("", |(_, v)| v).trim();
            out.push((key.clone(), value.to_string()));
        }
    }
    out
}

/// Cartes graphiques déclarées dans le registre ; la plus puissante d'abord.
pub fn parse_gpus(names: &str, memories: &str) -> Vec<Gpu> {
    let memories = parse_reg(memories);
    let mut gpus: Vec<Gpu> = parse_reg(names)
        .into_iter()
        .filter(|(_, n)| !n.is_empty() && !n.to_ascii_lowercase().contains("basic display") && !n.to_ascii_lowercase().contains("virtual"))
        .map(|(key, name)| {
            let vram = memories
                .iter()
                .find(|(k, _)| *k == key)
                .and_then(|(_, v)| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                .unwrap_or(0);
            let vram_mb = vram / (1024 * 1024);
            Gpu { tier: tier_of(&name, vram_mb), name, vram_mb }
        })
        .collect();
    gpus.sort_by(|a, b| b.tier.cmp(&a.tier).then(b.vram_mb.cmp(&a.vram_mb)));
    gpus
}

#[cfg(windows)]
fn reg_query(value: &str) -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .args(["query", r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}", "/s", "/v", value])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn reg_query(_: &str) -> String {
    String::new()
}

pub fn detect_gpu() -> Option<Gpu> {
    static GPU: std::sync::OnceLock<Option<Gpu>> = std::sync::OnceLock::new();
    GPU.get_or_init(|| parse_gpus(&reg_query("DriverDesc"), &reg_query("HardwareInformation.qwMemorySize")).into_iter().next()).clone()
}

/// Niveau retenu : celui de la carte graphique, « Équilibré » si elle est inconnue.
pub fn current_tier() -> Tier {
    detect_gpu().map_or(Tier::Medium, |g| g.tier)
}

// ---------------------------------------------------------------------------
// Édition des fichiers de configuration

/// Lignes d'une section `[nom]` : (début du contenu, fin) en indices de lignes.
fn section_bounds(lines: &[String], section: &str) -> Option<(usize, usize)> {
    let header = format!("[{section}]");
    let start = lines.iter().position(|l| l.trim() == header)? + 1;
    let end = lines[start..].iter().position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |p| start + p);
    Some((start, end))
}

/// Fin de ligne du fichier (Eden et Azahar écrivent en CRLF sous Windows).
fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Remplace ou ajoute des groupes de lignes `clé=valeur` dans une section (format INI / TOML simple).
/// `owner` dit à quel groupe appartient une clé existante : le groupe est écrit à la place de sa
/// première occurrence (l'ordre du fichier est gardé), sinon en fin de section.
fn set_lines(text: &str, section: &str, groups: &[Vec<(String, String)>], owner: &dyn Fn(&str) -> Option<usize>, sep: &str) -> String {
    let nl = newline(text);
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let (start, end) = match section_bounds(&lines, section) {
        Some(b) => b,
        None => {
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            (lines.len(), lines.len())
        }
    };
    let render = |g: &Vec<(String, String)>| g.iter().map(|(k, v)| format!("{k}{sep}{v}")).collect::<Vec<_>>();
    let mut written = vec![false; groups.len()];
    let mut body: Vec<String> = Vec::new();
    for line in &lines[start..end] {
        let key = line.split('=').next().unwrap_or("").trim();
        match owner(key) {
            Some(i) => {
                if !written[i] {
                    written[i] = true;
                    body.extend(render(&groups[i]));
                }
            }
            None => body.push(line.clone()),
        }
    }
    // Les groupes nouveaux vont avant les lignes vides de fin de section.
    let mut insert_at = body.len();
    while insert_at > 0 && body[insert_at - 1].trim().is_empty() {
        insert_at -= 1;
    }
    let new: Vec<String> = groups.iter().zip(&written).filter(|(_, w)| !**w).flat_map(|(g, _)| render(g)).collect();
    body.splice(insert_at..insert_at, new);
    lines.splice(start..end, body);
    let mut out = lines.join(nl);
    out.push_str(nl);
    out
}

/// Clé Qt : `clé`, `clé\default`, `clé\use_global` (réglages par jeu).
pub fn set_qt(text: &str, section: &str, values: &[(&str, String)], per_game: bool) -> String {
    let groups: Vec<Vec<(String, String)>> = values
        .iter()
        .map(|(k, v)| {
            let mut g = Vec::new();
            if per_game {
                g.push((format!("{k}\\use_global"), "false".to_string()));
            }
            g.push((format!("{k}\\default"), "false".to_string()));
            g.push((k.to_string(), v.clone()));
            g
        })
        .collect();
    let owner = |key: &str| values.iter().position(|(k, _)| key == *k || key.strip_prefix(k).is_some_and(|r| r.starts_with('\\')));
    set_lines(text, section, &groups, &owner, "=")
}

pub fn get_qt(text: &str, section: &str, key: &str) -> Option<String> {
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let (start, end) = section_bounds(&lines, section)?;
    lines[start..end].iter().find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string()))
}

/// Clés TOML `table.clé` (melonDS écrit une table par niveau : `[3D.GL]`).
pub fn set_toml(text: &str, values: &[(&str, String)]) -> String {
    let mut out = text.to_string();
    for (path, v) in values {
        let (table, key) = path.rsplit_once('.').unwrap_or(("", path));
        out = if table.is_empty() {
            // Clés à la racine : avant la première table.
            let nl = newline(&out);
            let mut lines: Vec<String> = out.lines().filter(|l| l.split('=').next().unwrap_or("").trim() != key || l.trim_start().starts_with('[')).map(str::to_string).collect();
            let first_table = lines.iter().position(|l| l.trim_start().starts_with('[')).unwrap_or(lines.len());
            lines.insert(first_table, format!("{key} = {v}"));
            lines.join(nl) + nl
        } else {
            let owner = |k: &str| (k == key).then_some(0);
            set_lines(&out, table, &[vec![(key.to_string(), v.clone())]], &owner, " = ")
        };
    }
    out
}

pub fn set_ini(text: &str, section: &str, values: &[(&str, String)]) -> String {
    let groups: Vec<Vec<(String, String)>> = values.iter().map(|(k, v)| vec![(k.to_string(), v.clone())]).collect();
    let owner = |key: &str| values.iter().position(|(k, _)| key.eq_ignore_ascii_case(k));
    set_lines(text, section, &groups, &owner, "=")
}

// ---------------------------------------------------------------------------
// Profils

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    /// Libellé lisible (« Résolution interne »).
    pub label: &'static str,
    /// Valeur lisible (« ×4 »).
    pub value: String,
}

/// Une modification : fichier, section, clé, valeur.
#[derive(Debug, Clone)]
struct Change {
    section: &'static str,
    key: &'static str,
    value: String,
}

fn ch(section: &'static str, key: &'static str, value: impl ToString) -> Change {
    Change { section, key, value: value.to_string() }
}

/// Réglages d'Eden pour un niveau. Valeurs : src/common/settings_enums.h.
fn eden_profile(tier: Tier) -> (Vec<Change>, Vec<Setting>) {
    let (res, res_label) = match tier {
        Tier::Ultra => (8, "×4 (jusqu'à 4K)"),
        Tier::High => (7, "×3 (1440p à 4K)"),
        Tier::Medium => (6, "×2 (1080p)"),
        Tier::Low => (3, "×1 (720p)"),
    };
    let strong = tier >= Tier::High;
    let changes = vec![
        ch("Renderer", "backend", 1),
        ch("Renderer", "resolution_setup", res),
        ch("Renderer", "anti_aliasing", if tier >= Tier::Medium { 2 } else { 1 }),
        ch("Renderer", "scaling_filter", if tier >= Tier::Medium { 4 } else { 6 }),
        ch("Renderer", "max_anisotropy", if strong { 5 } else { 3 }),
        ch("Renderer", "gpu_accuracy", if strong { 1 } else { 0 }),
        ch("Renderer", "accelerate_astc", 1),
        ch("Renderer", "astc_recompression", if strong { 0 } else { 2 }),
        ch("Renderer", "use_disk_shader_cache", "true"),
        ch("Renderer", "use_vulkan_driver_pipeline_cache", "true"),
        ch("Renderer", "use_vsync", 2),
        ch("System", "use_docked_mode", 1),
        ch("System", "language_index", 2),
        ch("System", "region_index", 2),
    ];
    let settings = vec![
        Setting { label: "Moteur de rendu", value: "Vulkan".into() },
        Setting { label: "Résolution interne", value: res_label.into() },
        Setting { label: "Anticrénelage", value: if tier >= Tier::Medium { "SMAA" } else { "FXAA" }.into() },
        Setting { label: "Filtre de mise à l'échelle", value: if tier >= Tier::Medium { "Lanczos" } else { "AMD FSR" }.into() },
        Setting { label: "Filtrage anisotrope", value: if strong { "×16" } else { "×4" }.into() },
        Setting { label: "Précision GPU", value: if strong { "Élevée" } else { "Normale" }.into() },
        Setting { label: "Textures ASTC", value: if strong { "Décodées sur le GPU, sans perte" } else { "Décodées sur le GPU, compressées (moins de mémoire)" }.into() },
        Setting { label: "Cache des shaders", value: "Activé (moins de saccades)".into() },
        Setting { label: "Synchronisation verticale", value: "FIFO (sans déchirure)".into() },
        Setting { label: "Mode", value: "Console sur la télé (docké)".into() },
        Setting { label: "Langue et région", value: "Français, Europe".into() },
    ];
    (changes, settings)
}

fn azahar_profile(tier: Tier) -> (Vec<Change>, Vec<Setting>) {
    let factor = match tier {
        Tier::Ultra => 6,
        Tier::High => 4,
        Tier::Medium => 3,
        Tier::Low => 2,
    };
    let changes = vec![
        ch("Renderer", "graphics_api", 2),
        ch("Renderer", "resolution_factor", factor),
        ch("Renderer", "use_hw_shader", "true"),
        ch("Renderer", "shaders_accurate_mul", if tier >= Tier::High { "true" } else { "false" }),
        ch("Renderer", "async_shader_compilation", "true"),
        ch("Renderer", "use_disk_shader_cache", "true"),
        ch("Renderer", "use_vsync", "true"),
        ch("Renderer", "texture_filter", 0),
    ];
    let settings = vec![
        Setting { label: "Moteur de rendu", value: "Vulkan".into() },
        Setting { label: "Résolution interne", value: format!("×{factor} ({}×{} pour l'écran du haut)", 400 * factor, 240 * factor) },
        Setting { label: "Multiplication précise des shaders", value: if tier >= Tier::High { "Activée (évite des défauts d'ombrage)" } else { "Désactivée (plus rapide)" }.into() },
        Setting { label: "Compilation des shaders", value: "En arrière-plan, avec cache (moins de saccades)".into() },
        Setting { label: "Synchronisation verticale", value: "Activée".into() },
        Setting { label: "Filtre de textures", value: "Aucun (rendu fidèle)".into() },
    ];
    (changes, settings)
}

fn melonds_profile(tier: Tier) -> (Vec<Change>, Vec<Setting>) {
    let scale = match tier {
        Tier::Ultra => 8,
        Tier::High => 6,
        Tier::Medium => 4,
        Tier::Low => 2,
    };
    // Le rendu « OpenGL Compute » (nécessite OpenGL 4.3) gère mieux l'agrandissement.
    let renderer = if tier >= Tier::Medium { 2 } else { 1 };
    let changes = vec![
        ch("3D", "Renderer", renderer),
        ch("3D.GL", "ScaleFactor", scale),
        ch("3D.GL", "BetterPolygons", "true"),
        ch("3D.GL", "HiresCoordinates", "true"),
        ch("Screen", "UseGL", "true"),
        ch("Screen", "VSync", "true"),
        ch("JIT", "Enable", "true"),
        ch("Audio", "Interpolation", 3),
    ];
    let settings = vec![
        Setting { label: "Rendu 3D", value: if renderer == 2 { "OpenGL Compute" } else { "OpenGL" }.into() },
        Setting { label: "Résolution interne", value: format!("×{scale} ({}×{})", 256 * scale, 192 * scale) },
        Setting { label: "Polygones améliorés", value: "Activés".into() },
        Setting { label: "Affichage", value: "OpenGL, synchronisation verticale".into() },
        Setting { label: "Recompilateur (JIT)", value: "Activé (plus rapide)".into() },
        Setting { label: "Son", value: "Interpolation cubique (plus doux)".into() },
    ];
    (changes, settings)
}

fn desmume_profile(tier: Tier) -> (Vec<Change>, Vec<Setting>) {
    let scale = match tier {
        Tier::Ultra | Tier::High => 4,
        Tier::Medium => 3,
        Tier::Low => 2,
    };
    let changes = vec![ch("3D", "Renderer", 1), ch("3D", "PrescaleHD", scale), ch("3D", "MultisampleSize", if tier >= Tier::High { 4 } else { 0 })];
    let settings = vec![
        Setting { label: "Rendu 3D", value: "OpenGL".into() },
        Setting { label: "Résolution interne", value: format!("×{scale}") },
        Setting { label: "Anticrénelage", value: if tier >= Tier::High { "MSAA ×4" } else { "Aucun" }.into() },
    ];
    (changes, settings)
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    QtPerGame,
    Toml,
    Ini,
}

/// Fichier à modifier pour cet émulateur (et ce jeu).
fn target_file(r: &Resolved, title_id: Option<&str>) -> Result<(PathBuf, Format), String> {
    let id = r.id;
    if id.is_switch() || id.is_ctr() {
        let user = r.ctr_user_dir().ok_or("dossier de l'émulateur introuvable")?;
        let tid = title_id.filter(|t| t.len() == 16 && t.bytes().all(|b| b.is_ascii_hexdigit())).ok_or("title ID du jeu inconnu")?;
        return Ok((user.join("config").join("custom").join(format!("{}.ini", tid.to_ascii_uppercase())), Format::QtPerGame));
    }
    let dir = r.exe.as_deref().and_then(Path::parent).ok_or_else(|| format!("{} est introuvable", id.name()))?;
    match id {
        EmulatorId::Melonds => {
            let portable = dir.join("portable").join("melonDS.toml");
            Ok((if portable.is_file() { portable } else { dir.join("melonDS.toml") }, Format::Toml))
        }
        EmulatorId::Desmume => Ok((dir.join("desmume.ini"), Format::Ini)),
        _ => Err("émulateur non pris en charge".into()),
    }
}

fn profile(id: EmulatorId, tier: Tier) -> (Vec<Change>, Vec<Setting>) {
    match id {
        EmulatorId::Eden => eden_profile(tier),
        EmulatorId::Melonds => melonds_profile(tier),
        EmulatorId::Desmume => desmume_profile(tier),
        _ => azahar_profile(tier),
    }
}

fn apply_changes(text: &str, format: Format, changes: &[Change]) -> String {
    let mut out = text.to_string();
    match format {
        Format::Toml => {
            let values: Vec<(String, String)> = changes.iter().map(|c| (format!("{}.{}", c.section, c.key), c.value.clone())).collect();
            let refs: Vec<(&str, String)> = values.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            out = set_toml(&out, &refs);
        }
        _ => {
            let mut sections: Vec<&str> = changes.iter().map(|c| c.section).collect();
            sections.dedup();
            for s in sections {
                let values: Vec<(&str, String)> = changes.iter().filter(|c| c.section == s).map(|c| (c.key, c.value.clone())).collect();
                out = match format {
                    Format::Ini => set_ini(&out, s, &values),
                    f => set_qt(&out, s, &values, f == Format::QtPerGame),
                };
            }
        }
    }
    out
}

/// Les valeurs voulues sont-elles déjà en place ?
fn is_applied(text: &str, format: Format, changes: &[Change]) -> bool {
    changes.iter().all(|c| match format {
        Format::Toml => {
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            section_bounds(&lines, c.section).is_some_and(|(s, e)| {
                lines[s..e].iter().any(|l| l.split_once('=').is_some_and(|(k, v)| k.trim() == c.key && v.trim() == c.value))
            })
        }
        _ => get_qt(text, c.section, c.key).is_some_and(|v| v == c.value),
    })
}

fn backup_path(file: &Path) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(".kaleido-backup");
    file.with_file_name(name)
}

fn created_marker(file: &Path) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(".kaleido-created");
    file.with_file_name(name)
}

// ---------------------------------------------------------------------------
// Commandes

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TuneTarget {
    pub emulator: EmulatorId,
    /// Title ID (Switch, 3DS) : réglages propres à ce jeu.
    pub title_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunePlan {
    emulator: &'static str,
    gpu: Option<Gpu>,
    tier: Tier,
    tier_label: &'static str,
    file: Option<String>,
    /// Réglages propres à ce jeu (sinon : communs à tous les jeux de l'émulateur).
    per_game: bool,
    settings: Vec<Setting>,
    applied: bool,
    can_restore: bool,
    error: Option<String>,
}

fn resolve(id: EmulatorId, app: &AppHandle) -> Resolved {
    let config = play::load_config(app);
    play::resolve(id, &config, &Env::system(&config.search_dirs))
}

pub fn plan(r: &Resolved, title_id: Option<&str>, gpu: Option<Gpu>) -> TunePlan {
    let tier = gpu.as_ref().map_or(Tier::Medium, |g| g.tier);
    let (changes, settings) = profile(r.id, tier);
    let mut p = TunePlan {
        emulator: r.id.name(),
        gpu,
        tier,
        tier_label: tier.label(),
        file: None,
        per_game: false,
        settings,
        applied: false,
        can_restore: false,
        error: None,
    };
    if r.exe.is_none() {
        p.error = Some(format!("{} n'est pas installé.", r.id.name()));
    }
    match target_file(r, title_id) {
        Ok((file, format)) => {
            p.per_game = format == Format::QtPerGame;
            p.applied = fs::read_to_string(&file).is_ok_and(|t| is_applied(&t, format, &changes));
            p.can_restore = backup_path(&file).is_file() || created_marker(&file).is_file();
            p.file = Some(file.display().to_string());
        }
        Err(e) => p.error = Some(e),
    }
    p
}

pub fn apply(r: &Resolved, title_id: Option<&str>, tier: Tier) -> Result<(), String> {
    let (file, format) = target_file(r, title_id)?;
    let (changes, _) = profile(r.id, tier);
    let existing = fs::read_to_string(&file).ok();
    match &existing {
        Some(_) if !backup_path(&file).exists() && !created_marker(&file).exists() => {
            fs::copy(&file, backup_path(&file)).map_err(|e| format!("copie de sécurité impossible : {e}"))?;
        }
        None => {
            if let Some(dir) = file.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            if !backup_path(&file).exists() {
                fs::write(created_marker(&file), b"").map_err(|e| e.to_string())?;
            }
        }
        _ => {}
    }
    let text = apply_changes(existing.as_deref().unwrap_or(""), format, &changes);
    fs::write(&file, text).map_err(|e| format!("{} : {e}", file.display()))
}

pub fn restore(r: &Resolved, title_id: Option<&str>) -> Result<(), String> {
    let (file, _) = target_file(r, title_id)?;
    let backup = backup_path(&file);
    let created = created_marker(&file);
    if backup.is_file() {
        fs::copy(&backup, &file).map_err(|e| e.to_string())?;
        fs::remove_file(&backup).map_err(|e| e.to_string())?;
    } else if created.is_file() {
        let _ = fs::remove_file(&file);
        fs::remove_file(&created).map_err(|e| e.to_string())?;
    } else {
        return Err("aucune copie de sécurité : les réglages n'ont pas été modifiés par Kaleido".into());
    }
    Ok(())
}

/// Ce que « Réglages optimaux » changerait (et si c'est déjà fait).
#[tauri::command]
pub async fn tune_plan(target: TuneTarget, app: AppHandle) -> Result<TunePlan, String> {
    crate::blocking(move || Ok(plan(&resolve(target.emulator, &app), target.title_id.as_deref(), detect_gpu()))).await
}

#[tauri::command]
pub async fn tune_apply(target: TuneTarget, app: AppHandle) -> Result<TunePlan, String> {
    crate::blocking(move || {
        let r = resolve(target.emulator, &app);
        apply(&r, target.title_id.as_deref(), current_tier())?;
        Ok(plan(&r, target.title_id.as_deref(), detect_gpu()))
    })
    .await
}

#[tauri::command]
pub async fn tune_restore(target: TuneTarget, app: AppHandle) -> Result<TunePlan, String> {
    crate::blocking(move || {
        let r = resolve(target.emulator, &app);
        restore(&r, target.title_id.as_deref())?;
        Ok(plan(&r, target.title_id.as_deref(), detect_gpu()))
    })
    .await
}

/// Active ou non les textures personnalisées d'Azahar (réglage global, section `[Utility]`).
pub fn set_azahar_custom_textures(user: &Path, enabled: bool, preload: bool) -> Result<(), String> {
    let file = user.join("config").join("qt-config.ini");
    let text = fs::read_to_string(&file).unwrap_or_default();
    let values = [("custom_textures", enabled.to_string()), ("preload_textures", preload.to_string()), ("async_custom_loading", "true".to_string())];
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&file, set_qt(&text, "Utility", &values, false)).map_err(|e| e.to_string())
}

/// Active ou non le chargeur de plugins 3GX d'Azahar (réglage global `plugin_loader`, section `[System]`).
pub fn set_azahar_plugin_loader(user: &Path, enabled: bool) -> Result<(), String> {
    let file = user.join("config").join("qt-config.ini");
    let text = fs::read_to_string(&file).unwrap_or_default();
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&file, set_qt(&text, "System", &[("plugin_loader", enabled.to_string())], false)).map_err(|e| e.to_string())
}

/// Active les codes de triche de melonDS (`Instance0.EnableCheats`).
pub fn enable_melonds_cheats(exe: &Path) -> Result<(), String> {
    let dir = exe.parent().ok_or("dossier de melonDS introuvable")?;
    let portable = dir.join("portable").join("melonDS.toml");
    let file = if portable.is_file() { portable } else { dir.join("melonDS.toml") };
    let text = fs::read_to_string(&file).unwrap_or_default();
    fs::write(&file, set_toml(&text, &[("Instance0.EnableCheats", "true".into())])).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_parsing() {
        let names = "\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968}\\0000\r\n    DriverDesc    REG_SZ    NVIDIA GeForce RTX 4080 SUPER\r\n\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968}\\0001\r\n    DriverDesc    REG_SZ    AMD Radeon(TM) Graphics\r\n\r\nFin de la recherche : 2 correspondance(s) trouvée(s).\r\n";
        let mem = "HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968}\\0000\r\n    HardwareInformation.qwMemorySize    REG_QWORD    0x3ff800000\r\nHKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968}\\0001\r\n    HardwareInformation.qwMemorySize    REG_QWORD    0x20000000\r\n";
        let gpus = parse_gpus(names, mem);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 4080 SUPER");
        assert_eq!(gpus[0].vram_mb, 16376);
        assert_eq!(gpus[0].tier, Tier::Ultra);
        assert_eq!(gpus[1].tier, Tier::Low);
        assert_eq!(tier_of("NVIDIA GeForce GTX 1060 6GB", 6144), Tier::Medium);
        assert_eq!(tier_of("Intel(R) UHD Graphics 770", 128), Tier::Low);
        assert_eq!(tier_of("Intel(R) Arc(TM) A770 Graphics", 16384), Tier::Ultra);
    }

    #[test]
    fn qt_per_game() {
        let text = "[Renderer]\nresolution_setup\\use_global=true\nbackend\\use_global=true\n\n[System]\nlanguage_index\\use_global=true\n";
        let out = set_qt(text, "Renderer", &[("resolution_setup", "8".into())], true);
        // La clé est remplacée à sa place d'origine.
        assert!(out.starts_with("[Renderer]\nresolution_setup\\use_global=false\nresolution_setup\\default=false\nresolution_setup=8\nbackend"));
        assert!(!out.contains("resolution_setup\\use_global=true"));
        assert!(out.contains("backend\\use_global=true"));
        assert_eq!(get_qt(&out, "Renderer", "resolution_setup").as_deref(), Some("8"));
        // Fins de ligne Windows conservées.
        let crlf = set_qt("[Renderer]\r\nbackend=0\r\n", "Renderer", &[("backend", "1".into())], false);
        assert_eq!(crlf, "[Renderer]\r\nbackend\\default=false\r\nbackend=1\r\n");
        // Section absente : ajoutée à la fin.
        let out = set_qt("", "Utility", &[("custom_textures", "true".into())], false);
        assert_eq!(out, "[Utility]\ncustom_textures\\default=false\ncustom_textures=true\n");
        // Une clé dont le nom en prolonge une autre n'est pas touchée.
        let out = set_qt("[Renderer]\nuse_vsync_x=1\nuse_vsync=0\n", "Renderer", &[("use_vsync", "2".into())], false);
        assert!(out.contains("use_vsync_x=1") && out.contains("use_vsync=2") && !out.contains("use_vsync=0"));
    }

    #[test]
    fn toml_tables() {
        let text = "LimitFPS = true\n\n[3D]\nRenderer = 0\n\n[3D.GL]\nScaleFactor = 1\n\n[Instance0]\nSaveFilePath = \"\"\n";
        let out = set_toml(text, &[("3D.Renderer", "2".into()), ("3D.GL.ScaleFactor", "6".into()), ("JIT.Enable", "true".into()), ("Instance0.EnableCheats", "true".into())]);
        assert!(out.contains("[3D]\nRenderer = 2\n"));
        assert!(out.contains("[3D.GL]\nScaleFactor = 6\n"));
        assert!(out.contains("[JIT]\nEnable = true"));
        assert!(out.contains("SaveFilePath = \"\"\nEnableCheats = true"));
        assert!(out.starts_with("LimitFPS = true"));
    }

    #[test]
    fn apply_and_restore() {
        let dir = std::env::temp_dir().join(format!("kaleido-tune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let user = dir.join("eden");
        fs::create_dir_all(user.join("config").join("custom")).unwrap();
        let r = Resolved {
            id: EmulatorId::Eden,
            exe: Some(dir.join("eden.exe")),
            detected: false,
            profile: play::Profile { user_dir: Some(user.clone()), ..Default::default() },
            env: Env::default(),
        };
        let tid = Some("01001F5010DFA000");
        let file = user.join("config").join("custom").join("01001F5010DFA000.ini");
        fs::write(&file, "[Renderer]\nresolution_setup\\use_global=true\n").unwrap();

        let gpu = Some(Gpu { name: "RTX".into(), vram_mb: 16000, tier: Tier::Ultra });
        assert!(!plan(&r, tid, gpu.clone()).applied);
        apply(&r, tid, Tier::Ultra).unwrap();
        let p = plan(&r, tid, gpu);
        assert!(p.applied && p.can_restore && p.per_game);
        assert!(fs::read_to_string(&file).unwrap().contains("resolution_setup=8"));
        // Une deuxième application garde la copie d'origine.
        apply(&r, tid, Tier::High).unwrap();
        restore(&r, tid).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "[Renderer]\nresolution_setup\\use_global=true\n");

        // Fichier inexistant : restaurer le supprime.
        let other = Some("0100A3D008C5C000");
        apply(&r, other, Tier::Low).unwrap();
        restore(&r, other).unwrap();
        assert!(!user.join("config").join("custom").join("0100A3D008C5C000.ini").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
