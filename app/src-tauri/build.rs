use std::path::Path;

fn main() {
    embed_mods_catalog();
    tauri_build::build()
}

/// Liste les fichiers de `data/mods-catalog` pour les intégrer au programme (`mods_catalog.rs`).
fn embed_mods_catalog() {
    let dir = Path::new("data/mods-catalog");
    println!("cargo:rerun-if-changed=data/mods-catalog");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".json")).collect())
        .unwrap_or_default();
    names.sort();
    let mut code = String::from("&[\n");
    for name in &names {
        println!("cargo:rerun-if-changed=data/mods-catalog/{name}");
        let stem = name.trim_end_matches(".json");
        code.push_str(&format!("    ({stem:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/data/mods-catalog/{name}\"))),\n"));
    }
    code.push_str("]\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("mods_catalog.rs");
    std::fs::write(out, code).unwrap();
}
