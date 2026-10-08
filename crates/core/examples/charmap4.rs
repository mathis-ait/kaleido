//! Table des caractères Gen 4 : code → texte décodé (JSON).
fn main() {
    let map: Vec<String> = (0u16..0x200).map(|c| format!("{:?}", kaleido_core::text::gen4::decode(&[c]))).collect();
    println!("[{}]", map.join(","));
}
