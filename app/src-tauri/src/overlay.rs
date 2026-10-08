//! Overlay de stream : petit serveur HTTP local (127.0.0.1 seulement) qui sert une page
//! à coller dans OBS comme « source navigateur ». La page affiche l'équipe, les morts et
//! les derniers évènements de la partie suivie par le compagnon, mise à jour à chaque
//! `companion-update` grâce aux Server-Sent Events (`/events`).
//!
//! Routes (toutes en GET) :
//! - `/overlay?k=<jeton>&layout=…` : la page autonome (fond transparent) ;
//! - `/api/state?k=<jeton>` : dernier état du compagnon, sans les boîtes ;
//! - `/api/journal?k=<jeton>` : les 100 dernières entrées du journal ;
//! - `/events?k=<jeton>` : flux SSE, un message `data:` par mise à jour ;
//! - `/sprite/<espèce>/<forme>?shiny=1&female=1&style=gen5ani` : sprites du cache `sprite://`.
//!
//! Le jeton aléatoire empêche une page web ouverte dans le navigateur de l'utilisateur de
//! lire l'état ; aucune en-tête CORS n'est envoyée. Rien n'est écouté hors de la machine.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::companion;

pub const DEFAULT_PORT: u16 = 48080;
/// Port déjà pris : on essaie les suivants.
const PORT_TRIES: u16 = 10;
/// Commentaire envoyé sur le flux quand rien ne se passe : OBS garde la connexion ouverte
/// et une source fermée est détectée (écriture en échec) au plus tard après ce délai.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// Page de l'overlay et jetons de thème de l'app (une seule source de vérité pour les couleurs).
const PAGE: &str = include_str!("overlay.html");
const APP_CSS: &str = include_str!("../../src/styles/main.css");

/// Réglages mémorisés dans `companion.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayConfig {
    pub enabled: bool,
    pub port: u16,
    /// Généré à la première activation, puis gardé : l'URL collée dans OBS reste valable.
    pub token: String,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig { enabled: false, port: DEFAULT_PORT, token: String::new() }
    }
}

/// Jeton de 128 bits en hexadécimal. `RandomState` est initialisé au hasard par le système
/// à chaque processus : suffisant pour un secret local, sans dépendance de plus.
pub fn new_token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        h.write_u128(t);
        h.write_u64(i ^ u64::from(std::process::id()));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

struct Running {
    server: Arc<Server>,
    port: u16,
}

/// État partagé entre le compagnon (qui publie) et le serveur (qui diffuse).
#[derive(Default)]
struct Hub {
    /// Dernier état publié, déjà sérialisé (`null` : aucune partie suivie).
    last: Mutex<Option<Arc<str>>>,
    subscribers: Mutex<Vec<Sender<Arc<str>>>>,
    running: Mutex<Option<Running>>,
    /// Jeton exigé par le serveur en cours.
    token: Mutex<String>,
    /// Dossier du cache des sprites (absent dans les tests).
    sprites: Mutex<Option<PathBuf>>,
    error: Mutex<Option<String>>,
}

fn hub() -> &'static Hub {
    static HUB: OnceLock<Hub> = OnceLock::new();
    HUB.get_or_init(Hub::default)
}

/// Ce que reçoit l'overlay : l'état du compagnon sans les boîtes (inutiles ici et lourdes).
fn slim(mut v: Value) -> Value {
    if let Some(snap) = v.get_mut("snapshot").and_then(Value::as_object_mut) {
        snap.remove("boxes");
    }
    v
}

/// Publie un nouvel état (ou `None` quand le compagnon ne suit plus de partie).
pub fn publish_value(state: Option<Value>) {
    let text: Arc<str> = match state {
        Some(v) => serde_json::to_string(&slim(v)).unwrap_or_else(|_| "null".into()).into(),
        None => "null".into(),
    };
    let h = hub();
    if let Ok(mut last) = h.last.lock() {
        if last.as_deref() == Some(&*text) {
            return;
        }
        *last = Some(text.clone());
    }
    if let Ok(mut subs) = h.subscribers.lock() {
        subs.retain(|s| s.send(text.clone()).is_ok());
    }
}

pub fn publish<T: Serialize>(state: Option<&T>) {
    publish_value(state.and_then(|s| serde_json::to_value(s).ok()));
}

fn last_state() -> Arc<str> {
    hub().last.lock().ok().and_then(|l| l.clone()).unwrap_or_else(|| "null".into())
}

// ---------------------------------------------------------------------------
// Serveur

/// Paramètre `name` de la chaîne de requête (valeurs sans caractères spéciaux : pas de décodage).
fn query_param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == name).then_some(v)
    })
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("en-tête valide")
}

fn text(status: u16, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body).with_status_code(status).with_header(header("Content-Type", "text/plain; charset=utf-8"))
}

fn json(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header("Content-Type", "application/json; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
}

/// Jetons de thème de l'app : tout ce qui précède les styles de base de `main.css`.
fn theme_css() -> &'static str {
    APP_CSS.split("/* ---------- Base ---------- */").next().unwrap_or("")
}

fn page() -> String {
    // En développement, la page est relue sur le disque : retouches visibles sans recompiler.
    #[cfg(debug_assertions)]
    if let Ok(p) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/overlay.html")) {
        return p.replace("/*__THEME_TOKENS__*/", theme_css());
    }
    PAGE.replace("/*__THEME_TOKENS__*/", theme_css())
}

/// `/sprite/25/0?shiny=1&female=1&style=gen5ani` → `gen5ani/25-0-f-shiny.gif`.
fn sprite_name(url: &str) -> Option<String> {
    let path = url.split('?').next()?;
    let mut parts = path.trim_start_matches("/sprite/").split('/');
    let species: u16 = parts.next()?.parse().ok().filter(|&n| n > 0)?;
    let form: u16 = parts.next().and_then(|f| f.parse().ok()).unwrap_or(0);
    let style = match query_param(url, "style") {
        Some(s @ ("ani" | "gen5ani" | "dex" | "gen5" | "home")) => s,
        Some("icons") => return Some(format!("{species}{}.png", if query_param(url, "shiny") == Some("1") { "-shiny" } else { "" })),
        _ => "gen5ani",
    };
    let ext = if style.ends_with("ani") { "gif" } else { "png" };
    let mut name = format!("{style}/{species}-{form}");
    if query_param(url, "female") == Some("1") {
        name.push_str("-f");
    }
    if query_param(url, "shiny") == Some("1") {
        name.push_str("-shiny");
    }
    Some(format!("{name}.{ext}"))
}

fn journal() -> String {
    let state: Value = serde_json::from_str(&last_state()).unwrap_or(Value::Null);
    serde_json::to_string(state.get("journal").unwrap_or(&Value::Array(Vec::new()))).unwrap_or_else(|_| "[]".into())
}

/// Flux SSE : la connexion reste ouverte et reçoit chaque nouvel état.
fn stream(request: Request) {
    let (tx, rx): (Sender<Arc<str>>, Receiver<Arc<str>>) = mpsc::channel();
    if let Ok(mut subs) = hub().subscribers.lock() {
        subs.push(tx);
    }
    let mut w = request.into_writer();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n";
    // `retry` : OBS se reconnecte seul 2 s après un redémarrage de Kaleido.
    let first = format!("{head}retry: 2000\n\ndata: {}\n\n", last_state());
    if w.write_all(first.as_bytes()).and_then(|_| w.flush()).is_err() {
        return;
    }
    loop {
        let chunk = match rx.recv_timeout(HEARTBEAT) {
            Ok(state) => format!("data: {state}\n\n"),
            Err(RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if w.write_all(chunk.as_bytes()).and_then(|_| w.flush()).is_err() {
            return;
        }
    }
}

fn handle(request: Request) {
    if request.method() != &Method::Get {
        let _ = request.respond(text(405, "méthode non prise en charge"));
        return;
    }
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    // Les sprites ne révèlent rien de la partie : pas de jeton, l'overlay les charge en <img>.
    if let Some(name) = path.starts_with("/sprite/").then(|| sprite_name(&url)) {
        let cache = hub().sprites.lock().ok().and_then(|c| c.clone());
        let result = match (name, cache) {
            (Some(n), Some(c)) => crate::sprites::image(&c, &n),
            (None, _) => Err("nom de sprite invalide".into()),
            (_, None) => Err("cache des sprites indisponible".into()),
        };
        let _ = match result {
            Ok((data, mime)) => request.respond(
                Response::from_data(data).with_header(header("Content-Type", mime)).with_header(header("Cache-Control", "max-age=31536000, immutable")),
            ),
            Err(e) => request.respond(text(404, &e)),
        };
        return;
    }
    let token = hub().token.lock().map(|t| t.clone()).unwrap_or_default();
    if token.is_empty() || query_param(&url, "k") != Some(token.as_str()) {
        let _ = request.respond(text(403, "jeton manquant ou incorrect : recopie l'URL depuis les réglages de Kaleido"));
        return;
    }
    let _ = match path.as_str() {
        "/overlay" | "/" => request.respond(
            Response::from_string(page())
                .with_header(header("Content-Type", "text/html; charset=utf-8"))
                .with_header(header("Cache-Control", "no-store")),
        ),
        "/api/state" => request.respond(json(&last_state())),
        "/api/journal" => request.respond(json(&journal())),
        "/events" => {
            stream(request);
            Ok(())
        }
        _ => request.respond(text(404, "introuvable")),
    };
}

/// Démarre le serveur sur `port` ou l'un des suivants ; renvoie le port retenu.
fn start_server(port: u16, token: &str) -> Result<u16, String> {
    stop_server();
    if let Ok(mut t) = hub().token.lock() {
        *t = token.to_string();
    }
    let mut last = String::new();
    for p in port..port.saturating_add(PORT_TRIES) {
        match Server::http(("127.0.0.1", p)) {
            Ok(server) => {
                let server = Arc::new(server);
                let s = server.clone();
                std::thread::Builder::new()
                    .name("overlay-http".into())
                    .spawn(move || {
                        for request in s.incoming_requests() {
                            // Un fil par requête : le flux SSE reste ouvert tant qu'OBS l'affiche.
                            let _ = std::thread::Builder::new().name("overlay-req".into()).spawn(move || handle(request));
                        }
                    })
                    .map_err(|e| e.to_string())?;
                if let Ok(mut r) = hub().running.lock() {
                    *r = Some(Running { server, port: p });
                }
                return Ok(p);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("aucun port libre entre {port} et {} ({last})", port.saturating_add(PORT_TRIES - 1)))
}

fn stop_server() {
    if let Some(r) = hub().running.lock().ok().and_then(|mut r| r.take()) {
        r.server.unblock();
    }
    // Les flux ouverts se ferment : leurs canaux sont abandonnés.
    if let Ok(mut subs) = hub().subscribers.lock() {
        subs.clear();
    }
}

fn running_port() -> Option<u16> {
    hub().running.lock().ok()?.as_ref().map(|r| r.port)
}

/// Applique la configuration (démarre, redémarre ou arrête le serveur).
fn apply(c: &OverlayConfig) {
    let error = if c.enabled {
        let same = running_port().is_some() && hub().token.lock().map(|t| *t == c.token).unwrap_or(false);
        let wanted = (c.port..c.port.saturating_add(PORT_TRIES)).contains(&running_port().unwrap_or(0));
        if same && wanted {
            None
        } else {
            start_server(c.port, &c.token).err()
        }
    } else {
        stop_server();
        None
    };
    if let Ok(mut e) = hub().error.lock() {
        *e = error;
    }
}

/// Au démarrage de Kaleido : relance l'overlay s'il était activé.
pub fn init(app: &AppHandle) {
    if let Ok(mut s) = hub().sprites.lock() {
        *s = crate::sprites::cache_dir(app).ok();
    }
    let c = companion::load_config(app).overlay;
    if c.enabled && !c.token.is_empty() {
        apply(&c);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayStatus {
    enabled: bool,
    /// Port demandé.
    port: u16,
    /// Port réellement ouvert (un des suivants si le premier est pris).
    active_port: Option<u16>,
    token: String,
    error: Option<String>,
}

fn status(c: &OverlayConfig) -> OverlayStatus {
    OverlayStatus {
        enabled: c.enabled,
        port: c.port,
        active_port: running_port(),
        token: c.token.clone(),
        error: hub().error.lock().ok().and_then(|e| e.clone()),
    }
}

#[tauri::command]
pub fn overlay_status(app: AppHandle) -> OverlayStatus {
    status(&companion::load_config(&app).overlay)
}

/// Active ou coupe l'overlay ; `port` change le port demandé, `new_token` invalide les anciennes URL.
#[tauri::command]
pub async fn overlay_configure(enabled: bool, port: Option<u16>, new_token: Option<bool>, app: AppHandle) -> Result<OverlayStatus, String> {
    crate::blocking(move || {
        let mut config = companion::load_config(&app);
        let c = &mut config.overlay;
        c.enabled = enabled;
        if let Some(p) = port.filter(|&p| p >= 1024) {
            c.port = p;
        }
        if c.token.is_empty() || new_token == Some(true) {
            c.token = new_token_unique(&c.token);
        }
        companion::save_config(&app, &config)?;
        apply(&config.overlay);
        Ok(status(&config.overlay))
    })
    .await
}

fn new_token_unique(old: &str) -> String {
    loop {
        let t = new_token();
        if t != old {
            return t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpStream;

    fn get(port: u16, path: &str) -> (u16, String) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let status = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        let body = out.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (status, body)
    }

    /// Critère du PRD : démarrer le serveur, pousser un état synthétique, le lire sur
    /// `/api/state` puis recevoir la mise à jour suivante sur le flux SSE.
    #[test]
    fn etat_et_evenements_servis() {
        let port = start_server(48380, "jeton-test").unwrap();
        publish_value(Some(serde_json::json!({
            "title": "Platine",
            "snapshot": { "party": [{ "species": 25, "level": 12 }], "boxes": [{ "name": "Boîte 1" }] },
            "journal": [{ "kind": "caught", "text": "Nouveau Pokémon : Pikachu (N. 12)" }],
        })));

        assert_eq!(get(port, "/api/state").0, 403, "sans jeton");
        assert_eq!(get(port, "/api/state?k=faux").0, 403);
        let (code, body) = get(port, "/api/state?k=jeton-test");
        assert_eq!(code, 200);
        let state: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(state["title"], "Platine");
        assert_eq!(state["snapshot"]["party"][0]["species"], 25);
        assert!(state["snapshot"].get("boxes").is_none(), "les boîtes ne sont pas envoyées");
        let (_, journal) = get(port, "/api/journal?k=jeton-test");
        assert!(journal.contains("Pikachu"));
        let (code, html) = get(port, "/overlay?k=jeton-test&layout=bar");
        assert_eq!(code, 200);
        assert!(html.contains("--sp-1"), "jetons de thème injectés");

        // Flux SSE : l'état actuel d'abord, puis chaque publication.
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(s, "GET /events?k=jeton-test HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut r = BufReader::new(s);
        let mut data = Vec::new();
        let mut line = String::new();
        let mut published = false;
        while data.len() < 2 {
            line.clear();
            r.read_line(&mut line).unwrap();
            if let Some(d) = line.strip_prefix("data: ") {
                data.push(d.trim().to_string());
                if !published {
                    publish_value(Some(serde_json::json!({ "title": "Platine", "snapshot": { "party": [{ "species": 25, "level": 13 }] } })));
                    published = true;
                }
            }
        }
        assert!(data[0].contains("\"level\":12"));
        assert!(data[1].contains("\"level\":13"));

        // Port pris : le serveur suivant prend le port d'après.
        let other = Server::http(("127.0.0.1", 48390)).unwrap();
        let port2 = start_server(48390, "jeton-test").unwrap();
        assert_eq!(port2, 48391);
        drop(other);
        stop_server();
    }

    #[test]
    fn noms_de_sprites() {
        assert_eq!(sprite_name("/sprite/25/0").as_deref(), Some("gen5ani/25-0.gif"));
        assert_eq!(sprite_name("/sprite/479/2?shiny=1&style=home").as_deref(), Some("home/479-2-shiny.png"));
        assert_eq!(sprite_name("/sprite/450/0?female=1&style=ani").as_deref(), Some("ani/450-0-f.gif"));
        assert_eq!(sprite_name("/sprite/6/0?style=icons&shiny=1").as_deref(), Some("6-shiny.png"));
        assert_eq!(sprite_name("/sprite/0/0"), None);
    }

    #[test]
    fn jetons_differents() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
