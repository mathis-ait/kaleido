//! Échanges en jeu : Diamant / Perle / Platine (`fld_trade.narc`, sous `resource/fra/`
//! dans les ROMs françaises), HeartGold / SoulSilver (`a/1/1/2`),
//! Noire/Blanche (`a/1/6/5`) et Noire 2 / Blanche 2 (`a/1/6/3`). Même format en Gen 4.
//! Portage de `getInGameTrades` / `setInGameTrades` (Gen4RomHandler, Gen5RomHandler)
//! et de `TradeRandomizer` de l'Universal Pokémon Randomizer.

use kaleido_formats::narc::Narc;

use super::access::Files;
use super::tables::{self, Loc};
use crate::games::Game;
use crate::rom::{GameRom, RomError};
use crate::text::{gen4, gen5};

const PT_TRADES: &str = "fielddata/pokemon_trade/fld_trade.narc";
const BW_TRADES: &str = "a/1/6/5";
const B2W2_TRADES: &str = "a/1/6/3";
const HGSS_TRADES: &str = "a/1/1/2";

/// Position des champs (u32) dans une entrée d'échange.
struct Fields {
    given: usize,
    ivs: usize,
    ot_id: usize,
    item: usize,
    requested: usize,
    /// Taille minimale d'une entrée.
    len: usize,
}

const GEN4: Fields = Fields { given: 0, ivs: 4, ot_id: 0x20, item: 0x3C, requested: 0x4C, len: 0x50 };
const GEN5: Fields = Fields { given: 4, ivs: 0x10, ot_id: 0x34, item: 0x4C, requested: 0x5C, len: 0x60 };
/// Gen 4 : talent imposé (u32). Gen 5 : forme (u32) et nature (u32, 0xFF = au hasard).
const GEN4_ABILITY: usize = 0x1C;
const GEN5_FORM: usize = 0x08;
const GEN5_NATURE: usize = 0x2C;

fn fields(gen: u8) -> &'static Fields {
    if gen <= 4 {
        &GEN4
    } else {
        &GEN5
    }
}

fn rd32(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn wr32(d: &mut [u8], at: usize, v: u32) {
    if let Some(b) = d.get_mut(at..at + 4) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

/// Un échange : Pokémon donné au joueur contre le Pokémon demandé.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    /// Entrée dans l'archive des échanges.
    pub entry: usize,
    pub given: u16,
    pub requested: u16,
    pub ivs: [u8; 6],
    pub item: u16,
    pub ot_id: u16,
    pub nickname: String,
    pub ot_name: String,
}

pub(super) struct Trades {
    path: String,
    gen: u8,
    narc: Narc,
    pub list: Vec<Trade>,
}

/// Fichier de textes des échanges : (index, position du surnom, position du dresseur).
fn text_slots(gen: u8, entry: usize, total: usize) -> (usize, usize) {
    if gen <= 4 {
        (entry, entry + total)
    } else {
        (entry * 2, entry * 2 + 1)
    }
}

fn text_file_index(game: Game) -> usize {
    match game {
        Game::Black2 | Game::White2 => tables::B2W2_TRADE_TEXT,
        Game::Diamond | Game::Pearl => tables::DP_TRADE_TEXT,
        Game::HeartGold | Game::SoulSilver => tables::HGSS_TRADE_TEXT,
        Game::Platinum => tables::PT_TRADE_TEXT,
        _ => tables::BW_TRADE_TEXT,
    }
}

/// Dialogues des personnes qui proposent les échanges (0 = aucun), dans l'ordre des échanges lus.
fn person_texts(game: Game) -> &'static [usize] {
    match game {
        Game::Platinum => &tables::PT_TRADE_PERSON_TEXTS,
        Game::Diamond | Game::Pearl => &tables::DP_TRADE_PERSON_TEXTS,
        Game::HeartGold | Game::SoulSilver => &tables::HGSS_TRADE_PERSON_TEXTS,
        _ => &[],
    }
}

/// Lit les échanges du jeu (`None` si le jeu n'est pas pris en charge).
pub(super) fn load(game: &GameRom) -> Result<Option<Trades>, RomError> {
    let (path, unused): (String, &[usize]) = match game.game {
        Game::Platinum | Game::Diamond | Game::Pearl => {
            // Les ROMs françaises rangent certaines archives sous `resource/fra/…`.
            let path = if game.rom().file_id(PT_TRADES).is_some() {
                PT_TRADES.to_string()
            } else {
                match game.rom().files().find(|(_, p)| p.ends_with("fld_trade.narc")) {
                    Some((_, p)) => p.to_string(),
                    None => return Ok(None),
                }
            };
            (path, &[])
        }
        // Shuckie et Kenya (entrées 6 et 7) sont des dons scriptés pour l'UPR : ignorés.
        Game::HeartGold | Game::SoulSilver => (HGSS_TRADES.into(), &tables::HGSS_TRADES_UNUSED),
        Game::Black => (BW_TRADES.into(), &tables::BLACK_TRADES_UNUSED),
        Game::White => (BW_TRADES.into(), &tables::WHITE_TRADES_UNUSED),
        Game::Black2 => (B2W2_TRADES.into(), &tables::BLACK2_TRADES_UNUSED),
        Game::White2 => (B2W2_TRADES.into(), &tables::WHITE2_TRADES_UNUSED),
        _ => return Ok(None),
    };
    let gen = game.generation();
    let narc = game.narc(&path)?;
    let f = fields(gen);
    let strings = game.text_file(text_file_index(game.game))?;
    let total = narc.files.len();
    let mut list = Vec::new();
    for (entry, data) in narc.files.iter().enumerate() {
        if unused.contains(&entry) {
            continue;
        }
        if data.len() < f.len {
            return Err(RomError::Layout(format!("échange n°{entry} trop court ({} octets)", data.len())));
        }
        let (nick, ot) = text_slots(gen, entry, total);
        let mut ivs = [0u8; 6];
        for (i, iv) in ivs.iter_mut().enumerate() {
            *iv = rd32(data, f.ivs + i * 4).min(31) as u8;
        }
        list.push(Trade {
            entry,
            given: rd32(data, f.given) as u16,
            requested: rd32(data, f.requested) as u16,
            ivs,
            item: rd32(data, f.item) as u16,
            ot_id: rd32(data, f.ot_id) as u16,
            nickname: strings.get(nick).cloned().unwrap_or_default(),
            ot_name: strings.get(ot).cloned().unwrap_or_default(),
        });
    }
    Ok(Some(Trades { path, gen, narc, list }))
}

/// Modifie un fichier de l'archive de textes principale.
fn edit_text(game: &mut GameRom, index: usize, edit: impl FnOnce(&mut Vec<String>) -> bool) -> Result<(), RomError> {
    edit_archive_text(game, game.layout.text_archive, index, edit)
}

/// Modifie un fichier d'une archive de textes (principale ou textes de l'histoire).
fn edit_archive_text(game: &mut GameRom, archive: &str, index: usize, edit: impl FnOnce(&mut Vec<String>) -> bool) -> Result<(), RomError> {
    let mut narc = game.narc(archive)?;
    let Some(file) = narc.files.get_mut(index) else { return Ok(()) };
    if game.generation() <= 4 {
        let msg = gen4::MsgFile::parse(file)?;
        let mut lines = msg.strings();
        if !edit(&mut lines) {
            return Ok(());
        }
        *file = gen4::MsgFile::from_strings(msg.seed, &lines)?.to_bytes();
    } else {
        let mut msg = gen5::MsgFile::parse(file)?;
        let mut lines = msg.strings();
        if !edit(&mut lines) {
            return Ok(());
        }
        msg.set_strings(&lines)?;
        *file = msg.to_bytes();
    }
    game.replace_narc(archive, &narc)
}

/// Remplace des mots entiers (pas à l'intérieur d'un autre mot), sans enchaîner
/// les remplacements. Renvoie `None` si rien n'a changé.
pub(super) fn replace_words(text: &str, pairs: &[(String, String)]) -> Option<String> {
    let is_word = |c: char| c.is_alphanumeric();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    let mut i = 0;
    'outer: while i < text.len() {
        let before = text[..i].chars().next_back();
        if before.is_none_or(|c| !is_word(c)) {
            for (old, new) in pairs.iter().filter(|(o, n)| !o.is_empty() && o != n) {
                if text[i..].starts_with(old.as_str()) && text[i + old.len()..].chars().next().is_none_or(|c| !is_word(c)) {
                    out.push_str(new);
                    i += old.len();
                    changed = true;
                    continue 'outer;
                }
            }
        }
        let c = text[i..].chars().next().expect("position valide");
        out.push(c);
        i += c.len_utf8();
    }
    changed.then_some(out)
}

/// Réécrit les échanges (archive, textes, scripts d'affichage de Noire/Blanche).
/// `names` : noms des espèces ; `ability` : premier talent d'une espèce (Gen 4).
/// Renvoie les remarques (emplacements non reconnus).
pub(super) fn write(
    game: &mut GameRom,
    files: &mut Files,
    mut trades: Trades,
    old: &[Trade],
    names: &[String],
    ability: impl Fn(u16) -> u16,
) -> Result<Vec<String>, RomError> {
    let mut notes = Vec::new();
    let gen = trades.gen;
    let f = fields(gen);
    let total = trades.narc.files.len();
    for t in &trades.list {
        let Some(data) = trades.narc.files.get_mut(t.entry) else { continue };
        wr32(data, f.given, t.given as u32);
        for (i, &iv) in t.ivs.iter().enumerate() {
            wr32(data, f.ivs + i * 4, iv as u32);
        }
        wr32(data, f.item, t.item as u32);
        wr32(data, f.requested, t.requested as u32);
        if gen <= 4 {
            // L'UPR ne l'écrit que pour Kenya (HGSS) ; on évite ainsi un talent impossible.
            wr32(data, GEN4_ABILITY, ability(t.given) as u32);
            if data.len() > 0x50 {
                wr32(data, 0x50, 0); // sexe imposé désactivé
            }
        } else {
            wr32(data, GEN5_FORM, 0);
            wr32(data, GEN5_NATURE, 0xFF);
        }
    }
    game.replace_narc(&trades.path, &trades.narc)?;

    // Surnoms (et dresseurs, inchangés).
    let list = std::mem::take(&mut trades.list);
    edit_text(game, text_file_index(game.game), |lines| {
        for t in &list {
            let (nick, _) = text_slots(gen, t.entry, total);
            if let Some(line) = lines.get_mut(nick) {
                *line = t.nickname.clone();
            }
        }
        true
    })?;

    let name = |id: u16| names.get(id as usize).cloned().unwrap_or_default();
    match game.game {
        Game::Platinum | Game::Diamond | Game::Pearl | Game::HeartGold | Game::SoulSilver => {
            // Dialogues des personnes (`IngameTradePersonTextOffsets`).
            let hgss = matches!(game.game, Game::HeartGold | Game::SoulSilver);
            for (i, ((&text, old), new)) in person_texts(game.game).iter().zip(old).zip(&list).enumerate() {
                if text == 0 {
                    continue;
                }
                let mut pairs = vec![(name(old.given), name(new.given))];
                if old.requested != new.requested {
                    pairs.push((name(old.requested), name(new.requested)));
                }
                // HGSS : le dialogue du 7e échange existe en double (fichier suivant), comme dans l'UPR.
                let files: &[usize] = if hgss && i == 6 { &[text, text + 1] } else { &[text] };
                for &file in files {
                    edit_text(game, file, |lines| {
                        let mut changed = false;
                        for line in lines.iter_mut() {
                            if let Some(n) = replace_words(line, &pairs) {
                                *line = n;
                                changed = true;
                            }
                        }
                        changed
                    })?;
                }
            }
        }
        Game::Black | Game::White => {
            // Scripts d'affichage (`TradeScript[]`) : chaque valeur est remplacée selon
            // l'espèce qu'elle contient, ce qui vaut pour les deux versions.
            for (((file, pairs), old), new) in tables::BW_TRADE_SCRIPTS.iter().zip(old).zip(&list) {
                for &(req_at, giv_at) in pairs.iter() {
                    for at in [req_at, giv_at] {
                        let loc = Loc::Script(*file, at);
                        let value = match files.u16_at(loc) {
                            Some(v) if v == old.requested => new.requested,
                            Some(v) if v == old.given => new.given,
                            other => {
                                notes.push(format!("script d'échange {file} @ {at:#X} : valeur inattendue {other:?}"));
                                continue;
                            }
                        };
                        files.set_u16(loc, value);
                    }
                }
            }
        }
        Game::Black2 | Game::White2 => {
            // Dialogues des personnes (textes de l'histoire), regroupés par fichier : un même
            // fichier sert à deux échanges (Capidextre, Alakazam) et un seul passage évite
            // d'enchaîner les remplacements.
            let mut by_file: std::collections::BTreeMap<usize, Vec<(String, String)>> = std::collections::BTreeMap::new();
            for (o, n) in old.iter().zip(&list) {
                let Some(&(_, text)) = tables::B2W2_TRADE_PERSON_TEXTS.iter().find(|(entry, _)| *entry == o.entry) else { continue };
                let pairs = by_file.entry(text).or_default();
                pairs.push((name(o.given), name(n.given)));
                if o.requested != n.requested {
                    pairs.push((name(o.requested), name(n.requested)));
                }
            }
            for (text, pairs) in by_file {
                edit_archive_text(game, tables::B2W2_STORY_TEXT, text, |lines| {
                    let mut changed = false;
                    for line in lines.iter_mut() {
                        if let Some(n) = replace_words(line, &pairs) {
                            *line = n;
                            changed = true;
                        }
                    }
                    changed
                })?;
            }
        }
        _ => {}
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_only() {
        let pairs = vec![("ABRA".to_string(), "PIKACHU".to_string()), ("MACHOP".to_string(), "ABRA".to_string())];
        let s = "Trade your\nMACHOP for my ABRA? ABRACADABRA";
        assert_eq!(replace_words(s, &pairs).as_deref(), Some("Trade your\nABRA for my PIKACHU? ABRACADABRA"));
        assert_eq!(replace_words("rien", &pairs), None);
        assert_eq!(replace_words("Évoli !", &[("Évoli".into(), "Mew".into())]).as_deref(), Some("Mew !"));
    }

    #[test]
    fn trade_fields_roundtrip() {
        let mut d = vec![0u8; 0x60];
        wr32(&mut d, GEN5.given, 546);
        wr32(&mut d, GEN5.requested, 548);
        assert_eq!(rd32(&d, GEN5.given), 546);
        assert_eq!(rd32(&d, GEN5.requested), 548);
        assert_eq!(rd32(&d, 0x100), 0);
        wr32(&mut d, 0x5E, 1); // hors limites : ignoré
        assert_eq!(text_slots(5, 3, 13), (6, 7));
        assert_eq!(text_slots(4, 3, 4), (3, 7));
    }
}
