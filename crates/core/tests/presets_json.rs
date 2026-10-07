//! Les presets de « Nouvelle aventure » (`app/src/presets/*.json`) correspondent bien aux
//! réglages du randomizer : chaque clé écrite est reconnue (une faute de frappe serait
//! sinon ignorée sans bruit), pour la partie commune comme pour les surcharges DS / 3DS.

use std::path::PathBuf;

use kaleido_core::randomizer::Settings;
use serde_json::{Map, Value};

/// Fusion récursive : `over` remplace les clés de `base`.
fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                merge(b.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (b, o) => *b = o.clone(),
    }
}

/// Chaque clé de `wanted` existe dans `got` avec la même valeur.
fn check(wanted: &Value, got: &Value, at: &str) {
    match (wanted, got) {
        (Value::Object(w), Value::Object(g)) => {
            for (k, v) in w {
                let path = format!("{at}.{k}");
                let g = g.get(k).unwrap_or_else(|| panic!("{path} : réglage inconnu"));
                check(v, g, &path);
            }
        }
        (w, g) => assert_eq!(w, g, "{at}"),
    }
}

#[test]
fn presets_valides() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/src/presets");
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let preset: Map<String, Value> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{name} : {e}"));
        assert_eq!(preset["version"], 1, "{name}");
        for key in ["id", "name", "tagline"] {
            assert!(preset[key].as_str().is_some_and(|s| !s.is_empty()), "{name} : {key}");
        }
        assert_eq!(preset["changes"].as_array().map(Vec::len), Some(3), "{name} : trois changements");
        ids.push(preset["id"].as_str().unwrap().to_string());
        for platform in ["nds", "ctr"] {
            let mut settings = preset["settings"].clone();
            if let Some(over) = preset.get(platform) {
                merge(&mut settings, over);
            }
            let parsed: Settings = serde_json::from_value(settings.clone()).unwrap_or_else(|e| panic!("{name} ({platform}) : {e}"));
            check(&settings, &serde_json::to_value(&parsed).unwrap(), &format!("{name} ({platform})"));
        }
    }
    ids.sort();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n, "identifiants uniques");
    assert!(n >= 4, "au moins quatre presets");
}
