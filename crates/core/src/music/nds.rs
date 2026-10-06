//! Musique des jeux DS (Gen 4 et 5) : lecteur de séquences SSEQ.
//!
//! La musique des jeux DS n'est pas enregistrée : c'est une partition (SSEQ)
//! jouée par le pilote son Nitro du jeu avec des instruments (SBNK) qui
//! pointent vers des échantillons (SWAV rangés dans des SWAR). Tout est rangé
//! dans un fichier SDAT. Ce module lit le SDAT de la ROM et rejoue la séquence
//! comme le pilote du jeu : séquenceur cadencé à ~192 Hz, enveloppes ADSR en
//! décibels, volumes, panoramique, modulation, portamento, et mixeur 16 voies
//! (PCM 8/16 bits, IMA-ADPCM, PSG carré et bruit).
//!
//! Le cœur du lecteur (séquenceur, pistes, voies, tables de conversion) est un
//! portage en Rust de « SSEQ Player » de Naram Qashat (CyberBotX, in_xsf /
//! in_ncsf), lui-même adapté du FeOS Sound System de fincs
//! (<https://github.com/fincs/FSS>), dont les routines viennent du désassemblage
//! du pilote de Nintendo. Formats : « Nitro Composer File (*.sdat)
//! Specification » (kiwi.ds) et GBATEK pour le mixeur.

use std::collections::HashMap;
use std::path::Path;

use kaleido_formats::nds::NdsRom;
use kaleido_formats::FormatError;

use super::{MusicError, Pcm};
use crate::games::Game;

/// Fréquence de sortie (proche des 32 728 Hz du mixeur de la DS).
pub const SAMPLE_RATE: u32 = 32_768;

const ARM7_CLOCK: f64 = 33_513_982.0;
/// Période du séquenceur du pilote : 64 × 2728 cycles ARM7 (≈ 5,2 ms).
const SECONDS_PER_TIMER: f64 = 64.0 * 2728.0 / ARM7_CLOCK;
const AMPL_K: i32 = 723;
const AMPL_THRESHOLD: i32 = -AMPL_K * 128;

/// Thème de l'écran titre de `game`, lu dans la ROM `.nds`.
pub fn title_theme(path: &Path, game: Game, max_seconds: f32) -> Result<Pcm, MusicError> {
    let rom = NdsRom::open(path)?;
    let sdat_bytes = find_sdat(&rom)?;
    let sdat = Sdat::parse(sdat_bytes)?;
    let seqs = title_sequences(&sdat, game);
    if seqs.is_empty() {
        return Err(MusicError::NotFound("séquence de l'écran titre".into()));
    }
    render_sequences(&sdat, &seqs, max_seconds)
}

/// Le fichier SDAT de la ROM (le plus gros `.sdat` du système de fichiers).
pub fn find_sdat(rom: &NdsRom) -> Result<&[u8], MusicError> {
    rom.files()
        .filter(|(_, p)| p.to_ascii_lowercase().ends_with(".sdat"))
        .filter_map(|(id, _)| rom.file(id))
        .max_by_key(|d| d.len())
        .ok_or_else(|| MusicError::NotFound("fichier SDAT".into()))
}

/// Séquence de l'écran titre, par jeu, d'après les noms du bloc SYMB (présent
/// dans tous les SDAT Pokémon DS). Attention, les séquences « TITLE » sans
/// suffixe sont celles de la cinématique d'ouverture :
/// - Diamant / Perle / Platine : `SEQ_TITLE00` (n° 1172) accompagne l'intro,
///   `SEQ_TITLE01` (n° 1173) l'écran titre (`title_screen.c` de pret/pokeplatinum).
/// - HeartGold / SoulSilver : `SEQ_GS_TITLE` (n° 1004) est l'intro
///   (`intro_movie.c`), l'écran titre joue `SEQ_GS_POKEMON_THEME` (n° 1008,
///   `title_screen.c` de pret/pokeheartgold).
/// - Noir / Blanc (2) : même convention, `SEQ_BGM_POKEMON_THEME` (n° 1007).
const TITLE_NAMES: [&str; 3] = ["SEQ_TITLE01", "SEQ_GS_POKEMON_THEME", "SEQ_BGM_POKEMON_THEME"];

/// Index fixe de repli si le SDAT n'avait pas de noms (ROM modifiée).
fn title_index(game: Game) -> Option<usize> {
    match game.generation() {
        4 if matches!(game, Game::HeartGold | Game::SoulSilver) => Some(1008),
        4 => Some(1173),
        5 => Some(1007),
        _ => None,
    }
}

/// Séquences jouées à l'écran titre, dans l'ordre.
///
/// Noire 2 / Blanche 2 enchaînent `SEQ_BGM_TITLE01` (n° 1004, nouvelle
/// introduction de 41 s, propre à ces versions) puis le thème de Noir / Blanc
/// `SEQ_BGM_POKEMON_THEME` : la piste « Title Screen » de la bande originale
/// (1:53) correspond à cet enchaînement (41,5 s + 76,6 s). Dans Noir / Blanc,
/// `SEQ_BGM_TITLE01` (11 s) est un morceau distinct (« A New Adventure! »).
pub fn title_sequences(sdat: &Sdat, game: Game) -> Vec<usize> {
    let Some(main) = title_sequence(sdat, game) else { return Vec::new() };
    let names = sdat.sequence_names();
    if matches!(game, Game::Black2 | Game::White2) && names.get(main).is_some_and(|n| n.as_deref() == Some("SEQ_BGM_POKEMON_THEME")) {
        if let Some(intro) = names.iter().position(|n| n.as_deref() == Some("SEQ_BGM_TITLE01")) {
            return vec![intro, main];
        }
    }
    vec![main]
}

/// Index de la séquence principale de l'écran titre dans le SDAT.
pub fn title_sequence(sdat: &Sdat, game: Game) -> Option<usize> {
    let names = sdat.sequence_names();
    if let Some(i) = TITLE_NAMES.iter().find_map(|&wanted| names.iter().position(|n| n.as_deref() == Some(wanted))) {
        return Some(i);
    }
    if names.iter().all(Option::is_none) {
        return title_index(game).filter(|&i| sdat.has_sequence(i));
    }
    // Repli : un nom en « POKEMON_THEME », sinon en « TITLE01 ».
    ["POKEMON_THEME", "TITLE01"].iter().find_map(|pat| names.iter().position(|n| n.as_deref().is_some_and(|n| n.ends_with(pat))))
}

/// Rend la séquence n° `index` du SDAT en PCM stéréo, `max_seconds` au plus
/// (une séquence qui boucle est jouée jusqu'à cette durée, une séquence sans
/// boucle s'arrête à sa fin).
pub fn render_sequence(sdat: &Sdat, index: usize, max_seconds: f32) -> Result<Pcm, MusicError> {
    render_sequences(sdat, &[index], max_seconds)
}

/// Rend plusieurs séquences à la suite (chacune jusqu'à sa fin, la dernière
/// en boucle), `max_seconds` au plus en tout.
pub fn render_sequences(sdat: &Sdat, indices: &[usize], max_seconds: f32) -> Result<Pcm, MusicError> {
    let frames = (max_seconds.max(0.0) as f64 * SAMPLE_RATE as f64) as usize;
    let mut mix: Vec<f32> = Vec::with_capacity(frames * 2);
    for &index in indices {
        let info = sdat.seqs.get(index).copied().flatten().ok_or_else(|| MusicError::NotFound(format!("séquence n°{index}")))?;
        let sseq = sdat.file(info.file).ok_or(bad("SSEQ introuvable"))?;
        let data = sseq_data(sseq)?;
        let bank = load_bank(sdat, info.bank)?;
        let mask = sdat.players.get(info.player as usize).copied().flatten().filter(|&m| m != 0).unwrap_or(0xFFFF);
        let mut player = Player::new(data, &bank, mask, cnv_scale(if info.volume == 0 { 0x7F } else { info.volume }));
        player.render_into(&mut mix, frames);
        if mix.len() >= frames * 2 {
            break;
        }
    }

    // Normalisation : crête à -1 dBFS (gain borné pour ne pas gonfler le bruit).
    let peak = mix.iter().fold(0f32, |m, s| m.max(s.abs()));
    let gain = if peak > 0.0 { (29_000.0 / peak).min(4.0) } else { 1.0 };
    let samples = mix.iter().map(|s| (s * gain).round().clamp(-32768.0, 32767.0) as i16).collect();
    Ok(Pcm { sample_rate: SAMPLE_RATE, samples })
}

/// Déroulé temporel d'une séquence (sans rendu audio).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeqTiming {
    /// Instants (s) du premier et du deuxième passage par le saut de boucle de
    /// la piste qui boucle la première : la durée d'une boucle est la différence.
    Loop(f32, f32),
    /// Toutes les pistes se terminent à cet instant (s) : pas de boucle.
    End(f32),
    /// Ni boucle ni fin dans la durée examinée.
    Unknown,
}

/// Analyse le déroulé de la séquence n° `index` sur `max_seconds` au plus.
pub fn sequence_timing(sdat: &Sdat, index: usize, max_seconds: f32) -> Result<SeqTiming, MusicError> {
    let info = sdat.seqs.get(index).copied().flatten().ok_or_else(|| MusicError::NotFound(format!("séquence n°{index}")))?;
    let data = sseq_data(sdat.file(info.file).ok_or(bad("SSEQ introuvable"))?)?;
    let bank = load_bank(sdat, info.bank)?;
    let mut player = Player::new(data, &bank, 0xFFFF, 0);
    let timers = (max_seconds as f64 / SECONDS_PER_TIMER) as u64;
    while player.timer_count < timers {
        player.timer();
        if let Some(&(t, first)) = player.gotos.first() {
            if let Some(&(_, second)) = player.gotos.iter().skip(1).find(|g| g.0 == t) {
                let s = |n: u64| (n as f64 * SECONDS_PER_TIMER) as f32;
                return Ok(SeqTiming::Loop(s(first), s(second)));
            }
        }
        if player.track_ids.iter().all(|&t| player.tracks[t].ended) {
            return Ok(SeqTiming::End((player.timer_count as f64 * SECONDS_PER_TIMER) as f32));
        }
    }
    Ok(SeqTiming::Unknown)
}

fn bad(msg: &'static str) -> MusicError {
    MusicError::Format(FormatError::Invalid(msg))
}

fn rd8(d: &[u8], o: usize) -> Option<u8> {
    d.get(o).copied()
}

fn rd16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn rd32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

// ---------------------------------------------------------------------------
// SDAT
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct SeqInfo {
    pub file: u32,
    pub bank: u16,
    pub volume: u8,
    pub player: u8,
}

#[derive(Debug, Clone, Copy)]
struct BankInfo {
    file: u32,
    wave_arcs: [u16; 4],
}

/// Archive sonore Nitro (SDAT) : tables SYMB, INFO et FAT.
pub struct Sdat<'a> {
    data: &'a [u8],
    names: Vec<Option<String>>,
    seqs: Vec<Option<SeqInfo>>,
    banks: Vec<Option<BankInfo>>,
    wave_arcs: Vec<Option<u32>>,
    players: Vec<Option<u16>>,
    fat: Vec<(usize, usize)>,
}

impl<'a> Sdat<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, MusicError> {
        if data.get(..4) != Some(b"SDAT") {
            return Err(bad("ce n'est pas un SDAT"));
        }
        let symb_off = rd32(data, 0x10).ok_or(bad("SDAT tronqué"))? as usize;
        let info_off = rd32(data, 0x18).ok_or(bad("SDAT tronqué"))? as usize;
        let fat_off = rd32(data, 0x20).ok_or(bad("SDAT tronqué"))? as usize;
        if data.get(info_off..info_off + 4) != Some(b"INFO") || data.get(fat_off..fat_off + 4) != Some(b"FAT ") {
            return Err(bad("SDAT sans bloc INFO ou FAT"));
        }

        // Entrées d'un enregistrement INFO : décalages relatifs au bloc INFO.
        let record = |i: usize| -> Vec<Option<usize>> {
            let Some(rec) = rd32(data, info_off + 8 + 4 * i) else { return Vec::new() };
            let base = info_off + rec as usize;
            let count = rd32(data, base).unwrap_or(0).min(0x10000) as usize;
            (0..count).map(|j| rd32(data, base + 4 + 4 * j).filter(|&o| o != 0).map(|o| info_off + o as usize)).collect()
        };
        let seqs = record(0)
            .into_iter()
            .map(|o| {
                let o = o?;
                Some(SeqInfo { file: rd32(data, o)? & 0xFF_FFFF, bank: rd16(data, o + 4)?, volume: rd8(data, o + 6)?, player: rd8(data, o + 9)? })
            })
            .collect();
        let banks = record(2)
            .into_iter()
            .map(|o| {
                let o = o?;
                let mut wave_arcs = [0xFFFF; 4];
                for (k, w) in wave_arcs.iter_mut().enumerate() {
                    *w = rd16(data, o + 4 + 2 * k)?;
                }
                Some(BankInfo { file: rd32(data, o)? & 0xFF_FFFF, wave_arcs })
            })
            .collect();
        let wave_arcs = record(3).into_iter().map(|o| Some(rd32(data, o?)? & 0xFF_FFFF)).collect();
        let players = record(4).into_iter().map(|o| rd16(data, o? + 2)).collect();

        let count = rd32(data, fat_off + 8).unwrap_or(0) as usize;
        let fat = (0..count.min(0x10000))
            .map_while(|i| {
                let e = fat_off + 12 + 16 * i;
                Some((rd32(data, e)? as usize, rd32(data, e + 4)? as usize))
            })
            .collect();

        let mut names = Vec::new();
        if symb_off != 0 && data.get(symb_off..symb_off + 4) == Some(b"SYMB") {
            if let Some(rec) = rd32(data, symb_off + 8) {
                let base = symb_off + rec as usize;
                let count = rd32(data, base).unwrap_or(0).min(0x10000) as usize;
                names = (0..count)
                    .map(|j| {
                        let o = rd32(data, base + 4 + 4 * j).filter(|&o| o != 0)?;
                        let s = data.get(symb_off + o as usize..)?;
                        let end = s.iter().position(|&b| b == 0)?;
                        Some(String::from_utf8_lossy(&s[..end]).into_owned())
                    })
                    .collect();
            }
        }
        Ok(Self { data, names, seqs, banks, wave_arcs, players, fat })
    }

    /// Fichier n° `id` de la FAT.
    fn file(&self, id: u32) -> Option<&'a [u8]> {
        let &(off, size) = self.fat.get(id as usize)?;
        self.data.get(off..off.checked_add(size)?)
    }

    pub fn sequence_count(&self) -> usize {
        self.seqs.len()
    }

    /// Nom (bloc SYMB) de chaque séquence, `None` si absent.
    pub fn sequence_names(&self) -> Vec<Option<String>> {
        (0..self.seqs.len()).map(|i| self.names.get(i).cloned().flatten()).collect()
    }

    /// La séquence n° `index` existe-t-elle (entrée INFO non vide) ?
    pub fn has_sequence(&self, index: usize) -> bool {
        matches!(self.seqs.get(index), Some(Some(_)))
    }

    pub fn sequence_info(&self, index: usize) -> Option<SeqInfo> {
        self.seqs.get(index).copied().flatten()
    }

    /// Fichier SSEQ brut de la séquence n° `index`.
    pub fn sequence_file(&self, index: usize) -> Option<&'a [u8]> {
        self.file(self.sequence_info(index)?.file)
    }
}

/// Données de séquence d'un SSEQ (après l'en-tête), les sauts y sont relatifs.
fn sseq_data(sseq: &[u8]) -> Result<&[u8], MusicError> {
    if sseq.get(..4) != Some(b"SSEQ") || sseq.get(0x10..0x14) != Some(b"DATA") {
        return Err(bad("SSEQ invalide"));
    }
    let off = rd32(sseq, 0x18).ok_or(bad("SSEQ tronqué"))? as usize;
    sseq.get(off..).ok_or(bad("SSEQ tronqué"))
}

// ---------------------------------------------------------------------------
// SBNK / SWAR / SWAV
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Inst {
    /// 1 = PCM, 2 = PSG carré (`swav` = rapport cyclique), 3 = bruit.
    record: u8,
    swav: u16,
    swar: u16,
    key: u8,
    attack: u8,
    decay: u8,
    sustain: u8,
    release: u8,
    pan: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Region {
    low: u8,
    high: u8,
    inst: Inst,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct BankEntry {
    /// 0 = vide, 1-15 = instrument simple, 16 = batterie, 17 = partage du clavier.
    record: u8,
    regions: Vec<Region>,
}

fn parse_inst(d: &[u8], o: usize, record: u8) -> Option<Inst> {
    Some(Inst {
        record,
        swav: rd16(d, o)?,
        swar: rd16(d, o + 2)?,
        key: rd8(d, o + 4)?,
        attack: rd8(d, o + 5)?,
        decay: rd8(d, o + 6)?,
        sustain: rd8(d, o + 7)?,
        release: rd8(d, o + 8)?,
        pan: rd8(d, o + 9)?,
    })
}

fn parse_sbnk(d: &[u8]) -> Result<Vec<BankEntry>, MusicError> {
    if d.get(..4) != Some(b"SBNK") || d.get(0x10..0x14) != Some(b"DATA") {
        return Err(bad("SBNK invalide"));
    }
    let count = rd32(d, 0x38).ok_or(bad("SBNK tronqué"))? as usize;
    let mut entries = Vec::with_capacity(count.min(1024));
    for i in 0..count.min(0x10000) {
        let e = 0x3C + 4 * i;
        let (Some(record), Some(off)) = (rd8(d, e), rd16(d, e + 1)) else { break };
        let off = off as usize;
        let mut regions = Vec::new();
        match record {
            0 => {}
            16 => {
                let (low, high) = (rd8(d, off).unwrap_or(1), rd8(d, off + 1).unwrap_or(0));
                if low <= high {
                    for (k, key) in (low..=high).enumerate() {
                        let o = off + 2 + 12 * k;
                        let Some(inst) = rd16(d, o).and_then(|r| parse_inst(d, o + 2, r as u8)) else { break };
                        regions.push(Region { low: key, high: key, inst });
                    }
                }
            }
            17 => {
                let mut low = 0u8;
                for k in 0..8 {
                    let high = rd8(d, off + k).unwrap_or(0);
                    if high == 0 {
                        break;
                    }
                    let o = off + 8 + 12 * k;
                    let Some(inst) = rd16(d, o).and_then(|r| parse_inst(d, o + 2, r as u8)) else { break };
                    regions.push(Region { low, high, inst });
                    low = high.saturating_add(1);
                }
            }
            _ => {
                if let Some(inst) = parse_inst(d, off, record) {
                    regions.push(Region { low: 0, high: 127, inst });
                }
            }
        }
        entries.push(BankEntry { record, regions });
    }
    Ok(entries)
}

/// Échantillon décodé en PCM 16 bits.
#[derive(Debug, Clone, PartialEq)]
struct Wave {
    data: Vec<i16>,
    /// Début de la boucle et longueur totale, en échantillons.
    loop_start: usize,
    looped: bool,
    /// Valeur du timer matériel (fréquence = 16 756 991 / timer).
    timer: u16,
}

const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const IMA_STEP: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190,
    209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499,
    2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350,
    22385, 24623, 27086, 29794, 32767,
];

/// IMA-ADPCM de la DS : en-tête de 4 octets (échantillon initial, index de pas),
/// puis deux échantillons par octet, quartet de poids faible d'abord.
fn decode_ima_adpcm(raw: &[u8]) -> Vec<i16> {
    if raw.len() < 4 {
        return Vec::new();
    }
    let mut pred = i16::from_le_bytes([raw[0], raw[1]]) as i32;
    let mut index = (u16::from_le_bytes([raw[2], raw[3]]) as i32).clamp(0, 88);
    let mut out = Vec::with_capacity((raw.len() - 4) * 2);
    let mut nibble = |n: i32, out: &mut Vec<i16>| {
        let step = IMA_STEP[index as usize];
        let mut diff = step >> 3;
        if n & 4 != 0 {
            diff += step;
        }
        if n & 2 != 0 {
            diff += step >> 1;
        }
        if n & 1 != 0 {
            diff += step >> 2;
        }
        pred = if n & 8 != 0 { (pred - diff).max(-0x8000) } else { (pred + diff).min(0x7FFF) };
        index = (index + IMA_INDEX[n as usize]).clamp(0, 88);
        out.push(pred as i16);
    };
    for &b in &raw[4..] {
        nibble((b & 0xF) as i32, &mut out);
        nibble((b >> 4) as i32, &mut out);
    }
    out
}

/// SWAV sans en-tête de fichier (tel que rangé dans un SWAR).
fn parse_swav(d: &[u8]) -> Option<Wave> {
    let kind = rd8(d, 0)?;
    let looped = rd8(d, 1)? != 0;
    let rate = rd16(d, 2)?;
    let mut timer = rd16(d, 4)?;
    if timer == 0 && rate != 0 {
        timer = (ARM7_CLOCK / 2.0 / rate as f64) as u16;
    }
    let loop_words = rd16(d, 6)? as usize;
    let len_words = rd32(d, 8)? as usize;
    let size = (loop_words + len_words) * 4;
    let raw = &d[12..d.len().min(12 + size)];
    let (data, loop_start) = match kind {
        0 => (raw.iter().map(|&b| (b as i8 as i16) << 8).collect(), loop_words * 4),
        1 => (raw.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect(), loop_words * 2),
        2 => (decode_ima_adpcm(raw), loop_words.saturating_sub(1) * 8),
        _ => return None,
    };
    if data.is_empty() || timer == 0 {
        return None;
    }
    let loop_start = if loop_start < data.len() { loop_start } else { 0 };
    Some(Wave { data, loop_start, looped, timer })
}

fn swar_entries(d: &[u8]) -> Vec<Option<&[u8]>> {
    if d.get(..4) != Some(b"SWAR") || d.get(0x10..0x14) != Some(b"DATA") {
        return Vec::new();
    }
    let count = rd32(d, 0x38).unwrap_or(0) as usize;
    (0..count.min(0x10000)).map(|i| rd32(d, 0x3C + 4 * i).filter(|&o| o != 0).and_then(|o| d.get(o as usize..))).collect()
}

/// Banque d'instruments prête à jouer, avec ses échantillons décodés.
struct Bank {
    entries: Vec<BankEntry>,
    waves: Vec<Wave>,
    /// (archive 0-3, n° d'échantillon) → index dans `waves`.
    wave_index: HashMap<(u16, u16), usize>,
}

fn load_bank(sdat: &Sdat, bank: u16) -> Result<Bank, MusicError> {
    let info = sdat.banks.get(bank as usize).copied().flatten().ok_or(bad("banque d'instruments introuvable"))?;
    let entries = parse_sbnk(sdat.file(info.file).ok_or(bad("SBNK introuvable"))?)?;
    let swars: Vec<Vec<Option<&[u8]>>> = info
        .wave_arcs
        .iter()
        .map(|&w| sdat.wave_arcs.get(w as usize).copied().flatten().and_then(|f| sdat.file(f)).map(swar_entries).unwrap_or_default())
        .collect();

    // On ne décode que les échantillons utilisés par la banque.
    let mut waves = Vec::new();
    let mut wave_index = HashMap::new();
    for inst in entries.iter().flat_map(|e| &e.regions).map(|r| r.inst) {
        if !(inst.record == 1 || inst.record >= 4) || wave_index.contains_key(&(inst.swar, inst.swav)) {
            continue;
        }
        let wave = swars.get(inst.swar as usize).and_then(|s| s.get(inst.swav as usize)).copied().flatten().and_then(parse_swav);
        if let Some(w) = wave {
            wave_index.insert((inst.swar, inst.swav), waves.len());
            waves.push(w);
        }
    }
    Ok(Bank { entries, waves, wave_index })
}

// ---------------------------------------------------------------------------
// Tables de conversion du pilote (FSS / SSEQPlayer)
// ---------------------------------------------------------------------------

fn cnv_attack(attk: u8) -> i32 {
    const LUT: [u8; 19] = [0x00, 0x01, 0x05, 0x0E, 0x1A, 0x26, 0x33, 0x3F, 0x49, 0x54, 0x5C, 0x64, 0x6D, 0x74, 0x7B, 0x7F, 0x84, 0x89, 0x8F];
    let attk = if attk & 0x80 != 0 { 0 } else { attk as i32 };
    if attk >= 0x6D {
        LUT[(0x7F - attk) as usize] as i32
    } else {
        0xFF - attk
    }
}

fn cnv_fall(fall: u8) -> i32 {
    let fall = if fall & 0x80 != 0 { 0 } else { fall as i32 };
    match fall {
        0x7F => 0xFFFF,
        0x7E => 0x3C00,
        f if f < 0x32 => (f << 1) + 1,
        f => 0x1E00 / (0x7E - f),
    }
}

/// Volume linéaire (0-127) → atténuation en dixièmes de dB (20·log10).
fn cnv_scale(scale: u8) -> i32 {
    const LUT: [i16; 128] = [
        -32768, -421, -361, -325, -300, -281, -265, -252, -240, -230, -221, -212, -205, -198, -192, -186, -180, -175, -170, -165, -161, -156, -152,
        -148, -145, -141, -138, -134, -131, -128, -125, -122, -120, -117, -114, -112, -110, -107, -105, -103, -100, -98, -96, -94, -92, -90, -88,
        -86, -85, -83, -81, -79, -78, -76, -74, -73, -71, -70, -68, -67, -65, -64, -62, -61, -60, -58, -57, -56, -54, -53, -52, -51, -49, -48, -47,
        -46, -45, -43, -42, -41, -40, -39, -38, -37, -36, -35, -34, -33, -32, -31, -30, -29, -28, -27, -26, -25, -24, -23, -23, -22, -21, -20, -19,
        -18, -17, -17, -16, -15, -14, -13, -12, -12, -11, -10, -9, -9, -8, -7, -6, -6, -5, -4, -3, -3, -2, -1, -1, 0,
    ];
    LUT[if scale & 0x80 != 0 { 0x7F } else { scale as usize }] as i32
}

/// Volume (0-127) → atténuation en dixièmes de dB, courbe quadratique (40·log10).
/// Sert aux volumes de piste, à l'expression, à la vélocité et au sustain.
fn cnv_sust(sust: u8) -> i32 {
    const LUT: [i16; 128] = [
        -32768, -722, -721, -651, -601, -562, -530, -503, -480, -460, -442, -425, -410, -396, -383, -371, -360, -349, -339, -330, -321, -313, -305,
        -297, -289, -282, -276, -269, -263, -257, -251, -245, -239, -234, -229, -224, -219, -214, -210, -205, -201, -196, -192, -188, -184, -180,
        -176, -173, -169, -165, -162, -158, -155, -152, -149, -145, -142, -139, -136, -133, -130, -127, -125, -122, -119, -116, -114, -111, -109,
        -106, -103, -101, -99, -96, -94, -91, -89, -87, -85, -82, -80, -78, -76, -74, -72, -70, -68, -66, -64, -62, -60, -58, -56, -54, -52, -50,
        -49, -47, -45, -43, -42, -40, -38, -36, -35, -33, -31, -30, -28, -27, -25, -23, -22, -20, -19, -17, -16, -14, -13, -11, -10, -8, -7, -6, -4,
        -3, -1, 0,
    ];
    LUT[if sust & 0x80 != 0 { 0x7F } else { sust as usize }] as i32
}

fn cnv_sine(arg: i32) -> i32 {
    const LUT: [i8; 33] = [
        0, 6, 12, 19, 25, 31, 37, 43, 49, 54, 60, 65, 71, 76, 81, 85, 90, 94, 98, 102, 106, 109, 112, 115, 117, 120, 122, 123, 125, 126, 126, 127,
        127,
    ];
    // Quart de période de 32 pas (période de 128), table de 33 valeurs bornes comprises.
    const N: i32 = 32;
    let v = if arg < N {
        LUT[arg as usize]
    } else if arg < 2 * N {
        LUT[(2 * N - arg) as usize]
    } else if arg < 3 * N {
        -LUT[(arg - 2 * N) as usize]
    } else {
        -LUT[(4 * N - arg).clamp(0, N) as usize]
    };
    v as i32
}

/// Ajuste un timer de `pitch` 64es de demi-ton (768 = une octave), comme
/// `Timer_Adjust` du pilote (table `getpitchtbl` = 65536·(2^(i/768) − 1)).
fn timer_adjust(base: u16, pitch: i32) -> u16 {
    let t = base as f64 * (-(pitch as f64) / 768.0).exp2();
    t.clamp(16.0, 65535.0) as u16
}

/// Atténuation totale (dixièmes de dB, -723 à 0) → amplitude linéaire.
/// Équivalent de `getvoltbl` + diviseur de volume du matériel.
fn ampl_to_gain(total: i32) -> f32 {
    if total <= -AMPL_K {
        0.0
    } else {
        10f32.powf(total.min(0) as f32 / 200.0)
    }
}

// ---------------------------------------------------------------------------
// Séquenceur
// ---------------------------------------------------------------------------

const MAX_TRACKS: usize = 32;
const STACK_SIZE: usize = 3;

// Indicateurs de mise à jour posés par une piste.
const T_VOL: u8 = 1;
const T_PAN: u8 = 2;
const T_TIMER: u8 = 4;
const T_MOD: u8 = 8;
const T_LEN: u8 = 16;

// Indicateurs d'une voie.
const C_VOL: u8 = 1;
const C_PAN: u8 = 2;
const C_TIMER: u8 = 4;

#[derive(Debug, Clone, Copy, Default)]
struct Override {
    active: bool,
    cmd: u8,
    value: i32,
    extra: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum StackKind {
    #[default]
    Call,
    Loop,
}

#[derive(Debug, Clone, Default)]
struct Track {
    allocated: bool,
    ended: bool,
    note_wait: bool,
    porta: bool,
    tie: bool,
    pos: usize,
    stack: [(StackKind, usize); STACK_SIZE],
    loop_count: [u8; STACK_SIZE],
    stack_pos: usize,
    ov: Override,
    last_cmp: bool,
    wait: i32,
    patch: u16,
    porta_key: u8,
    porta_time: u8,
    sweep_pitch: i32,
    vol: u8,
    expr: u8,
    pan: i32,
    bend_range: u8,
    bend: i8,
    transpose: i8,
    attack: u8,
    decay: u8,
    sustain: u8,
    release: u8,
    mod_type: u8,
    mod_speed: u8,
    mod_depth: u8,
    mod_range: u8,
    mod_delay: u16,
    prio: u8,
    upd: u8,
}

impl Track {
    fn init(&mut self, start: usize) {
        *self = Track {
            allocated: true,
            note_wait: true,
            pos: start,
            last_cmp: true,
            prio: 64,
            porta_key: 60,
            vol: 127,
            expr: 127,
            bend_range: 2,
            attack: 0xFF,
            decay: 0xFF,
            sustain: 0xFF,
            release: 0xFF,
            mod_range: 1,
            mod_speed: 16,
            ..Track::default()
        };
    }

    fn r8(&mut self, d: &[u8]) -> Option<u8> {
        let v = *d.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    fn r16(&mut self, d: &[u8]) -> Option<u16> {
        Some(self.r8(d)? as u16 | (self.r8(d)? as u16) << 8)
    }

    fn r24(&mut self, d: &[u8]) -> Option<usize> {
        Some(self.r8(d)? as usize | (self.r8(d)? as usize) << 8 | (self.r8(d)? as usize) << 16)
    }

    fn rvl(&mut self, d: &[u8]) -> Option<i32> {
        let mut x = 0i32;
        for _ in 0..5 {
            let b = self.r8(d)?;
            x = (x << 7) | (b & 0x7F) as i32;
            if b & 0x80 == 0 {
                break;
            }
        }
        Some(x)
    }

    /// Argument d'une commande, ou valeur imposée par une commande
    /// « aléatoire » / « depuis une variable » précédente.
    fn v8(&mut self, d: &[u8], extra: bool) -> Option<i32> {
        if self.ov.active {
            Some(if extra { self.ov.extra } else { self.ov.value })
        } else {
            self.r8(d).map(i32::from)
        }
    }

    fn v16(&mut self, d: &[u8]) -> Option<i32> {
        if self.ov.active {
            Some(self.ov.value)
        } else {
            self.r16(d).map(i32::from)
        }
    }

    fn vvl(&mut self, d: &[u8]) -> Option<i32> {
        if self.ov.active {
            Some(self.ov.value)
        } else {
            self.rvl(d)
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
enum ChState {
    #[default]
    None,
    Start,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Source {
    #[default]
    Off,
    Pcm(usize),
    Psg(u8),
    Noise,
}

#[derive(Debug, Clone, Default)]
struct Channel {
    id: usize,
    state: ChState,
    track: Option<usize>,
    prio: u8,
    manual_sweep: bool,
    flags: u8,
    pan: i32,
    ext_ampl: i32,
    velocity: i32,
    ext_pan: i32,
    key: u8,
    org_key: u8,
    ampl: i32,
    ext_tune: i32,
    mod_type: u8,
    mod_speed: u8,
    mod_depth: u8,
    mod_range: u8,
    mod_delay: u16,
    mod_delay_cnt: u16,
    mod_counter: u16,
    sweep_len: i32,
    sweep_cnt: i32,
    sweep_pitch: i32,
    attack_lvl: i32,
    sustain_lvl: u8,
    decay_rate: i32,
    release_rate: i32,
    note_length: i32,
    vol: u16,
    // Registres du matériel.
    source: Source,
    base_timer: u16,
    enabled: bool,
    gain: f32,
    panning: i32,
    gain_l: f32,
    gain_r: f32,
    pos: f64,
    inc: f64,
    psg_x: u16,
    psg_last: f32,
    psg_count: i64,
}

impl Channel {
    fn release(&mut self) {
        self.note_length = -1;
        self.prio = 1;
        self.state = ChState::Release;
    }

    fn kill(&mut self) {
        self.state = ChState::None;
        self.track = None;
        self.prio = 0;
        self.enabled = false;
        self.gain = 0.0;
        self.gain_l = 0.0;
        self.gain_r = 0.0;
        self.vol = 0;
        self.note_length = -1;
    }

    fn update_vol(&mut self, trk: &Track, master_vol: i32, sseq_vol: i32) {
        let v = master_vol + sseq_vol + cnv_sust(trk.vol) + cnv_sust(trk.expr);
        self.ext_ampl = v.max(-AMPL_K);
    }

    fn update_tune(&mut self, trk: &Track) {
        self.ext_tune = (self.key as i32 - self.org_key as i32) * 64 + ((trk.bend as i32 * trk.bend_range as i32) >> 1);
    }

    fn update_mod(&mut self, trk: &Track) {
        self.mod_type = trk.mod_type;
        self.mod_speed = trk.mod_speed;
        self.mod_depth = trk.mod_depth;
        self.mod_range = trk.mod_range;
        self.mod_delay = trk.mod_delay;
    }

    fn update_porta(&mut self, trk: &Track) {
        self.manual_sweep = false;
        self.sweep_pitch = trk.sweep_pitch;
        self.sweep_cnt = 0;
        if !trk.porta {
            self.sweep_len = 0;
            return;
        }
        let diff = (trk.porta_key as i32 - self.key as i32) << 22;
        self.sweep_pitch += diff >> 16;
        if trk.porta_time == 0 {
            self.sweep_len = self.note_length;
            self.manual_sweep = true;
        } else {
            let sq = trk.porta_time as i32 * trk.porta_time as i32;
            self.sweep_len = ((self.sweep_pitch.abs() as i64 * sq as i64) >> 11) as i32;
        }
    }

    /// Mise à jour à chaque période du pilote : enveloppe, modulation,
    /// puis registres de volume / panoramique / timer (`Snd_UpdChannel`).
    fn update(&mut self, sample_rate: f64) {
        if self.state > ChState::Start && !self.enabled {
            self.kill();
            return;
        }
        let not_sustain = self.state != ChState::Sustain;
        let in_start = self.state == ChState::Start;
        let sweep = self.sweep_pitch != 0 && self.sweep_len != 0 && self.sweep_cnt <= self.sweep_len;
        let mut modulation = self.mod_depth != 0;
        let mut vol_upd = self.flags & C_VOL != 0 || not_sustain;
        let mut pan_upd = self.flags & C_PAN != 0 || in_start;
        let mut tmr_upd = self.flags & C_TIMER != 0 || in_start || sweep;
        let mut mod_param = 0i32;

        match self.state {
            ChState::None => return,
            ChState::Start | ChState::Attack => {
                if self.state == ChState::Start {
                    self.enabled = true;
                    self.ampl = AMPL_THRESHOLD;
                    self.state = ChState::Attack;
                }
                let old = self.ampl >> 7;
                let mut new = self.ampl;
                loop {
                    new = new * self.attack_lvl / 256;
                    if new >> 7 != old {
                        break;
                    }
                }
                self.ampl = new;
                if self.ampl == 0 {
                    self.state = ChState::Decay;
                }
            }
            ChState::Decay => {
                self.ampl -= self.decay_rate;
                let sust = cnv_sust(self.sustain_lvl) << 7;
                if self.ampl <= sust {
                    self.ampl = sust;
                    self.state = ChState::Sustain;
                }
            }
            ChState::Sustain => {}
            ChState::Release => {
                self.ampl -= self.release_rate;
                if self.ampl <= AMPL_THRESHOLD {
                    self.kill();
                    return;
                }
            }
        }

        if modulation && self.mod_delay_cnt < self.mod_delay {
            self.mod_delay_cnt += 1;
            modulation = false;
        }
        if modulation {
            match self.mod_type {
                0 => tmr_upd = true,
                1 => vol_upd = true,
                _ => pan_upd = true,
            }
            mod_param = cnv_sine((self.mod_counter >> 8) as i32) * self.mod_range as i32 * self.mod_depth as i32;
            if self.mod_type == 1 {
                mod_param = ((mod_param as i64 * 60) >> 14) as i32;
            } else {
                mod_param >>= 8;
            }
            let mut counter = self.mod_counter as u32 + ((self.mod_speed as u32) << 6);
            while counter >= 0x8000 {
                counter -= 0x8000;
            }
            self.mod_counter = counter as u16;
        }

        if tmr_upd {
            let mut adj = self.ext_tune;
            if modulation && self.mod_type == 0 {
                adj += mod_param;
            }
            if sweep {
                let (len, cnt) = (self.sweep_len as i64, self.sweep_cnt as i64);
                adj += (self.sweep_pitch as i64 * (len - cnt) / len) as i32;
                if !self.manual_sweep {
                    self.sweep_cnt += 1;
                }
            }
            let tmr = if adj != 0 { timer_adjust(self.base_timer, adj) } else { self.base_timer };
            self.inc = ARM7_CLOCK / (sample_rate * 2.0) / tmr.max(1) as f64;
            self.flags &= !C_TIMER;
        }

        if vol_upd || pan_upd {
            if vol_upd {
                let mut total = (self.ampl >> 7) + self.ext_ampl + self.velocity;
                if modulation && self.mod_type == 1 {
                    total += mod_param;
                }
                self.gain = ampl_to_gain(total);
                self.vol = (self.gain * 127.0 * 16.0) as u16;
                self.flags &= !C_VOL;
            }
            if pan_upd {
                let mut pan = self.pan + self.ext_pan;
                if modulation && self.mod_type == 2 {
                    pan += mod_param;
                }
                self.panning = (pan + 64).clamp(0, 127);
                self.flags &= !C_PAN;
            }
            self.gain_l = self.gain * (127 - self.panning) as f32 / 128.0;
            self.gain_r = self.gain * self.panning as f32 / 128.0;
        }
    }

    /// Échantillon courant (interpolation linéaire pour le PCM), puis avance.
    #[inline]
    fn next_sample(&mut self, waves: &[Wave]) -> f32 {
        match self.source {
            Source::Off => 0.0,
            Source::Pcm(w) => {
                let wave = &waves[w];
                let total = wave.data.len();
                let s = if self.pos < 0.0 {
                    0.0
                } else {
                    let i = self.pos as usize;
                    let frac = (self.pos - i as f64) as f32;
                    let a = wave.data[i.min(total - 1)] as f32;
                    let j = i + 1;
                    let b = if j < total {
                        wave.data[j]
                    } else if wave.looped {
                        wave.data[wave.loop_start + (j - total) % (total - wave.loop_start)]
                    } else {
                        wave.data[total - 1]
                    } as f32;
                    a + (b - a) * frac
                };
                self.pos += self.inc;
                if self.pos >= total as f64 {
                    if wave.looped && total > wave.loop_start {
                        let len = (total - wave.loop_start) as f64;
                        while self.pos >= total as f64 {
                            self.pos -= len;
                        }
                    } else {
                        self.kill();
                    }
                }
                s
            }
            Source::Psg(duty) => {
                let s = if self.pos < 0.0 {
                    0.0
                } else {
                    // Rapport cyclique n : n+1 huitièmes à l'état haut (7 = silence).
                    let step = (self.pos as i64 & 7) as u8;
                    if duty >= 7 {
                        -32767.0
                    } else if step >= 7 - duty {
                        32767.0
                    } else {
                        -32767.0
                    }
                };
                self.pos += self.inc;
                s
            }
            Source::Noise => {
                if self.pos >= 0.0 {
                    let target = self.pos as i64;
                    while self.psg_count < target {
                        if self.psg_x & 1 != 0 {
                            self.psg_x = (self.psg_x >> 1) ^ 0x6000;
                            self.psg_last = -32767.0;
                        } else {
                            self.psg_x >>= 1;
                            self.psg_last = 32767.0;
                        }
                        self.psg_count += 1;
                    }
                }
                self.pos += self.inc;
                if self.pos >= 0.0 {
                    self.psg_last
                } else {
                    0.0
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AllocKind {
    Pcm,
    Psg,
    Noise,
}

struct Player<'a> {
    data: &'a [u8],
    bank: &'a Bank,
    tracks: Vec<Track>,
    track_ids: Vec<usize>,
    channels: [Channel; 16],
    allowed: u16,
    tempo: i32,
    tempo_count: i32,
    tempo_rate: i32,
    master_vol: i32,
    sseq_vol: i32,
    variables: [i16; 32],
    random: u32,
    sample_rate: f64,
    /// Nombre d'appels au timer, et instants (en appels) des sauts `goto` de chaque piste.
    timer_count: u64,
    gotos: Vec<(usize, u64)>,
}

impl<'a> Player<'a> {
    fn new(data: &'a [u8], bank: &'a Bank, allowed: u16, sseq_vol: i32) -> Self {
        let mut p = Player {
            data,
            bank,
            tracks: vec![Track::default(); MAX_TRACKS],
            track_ids: Vec::with_capacity(16),
            channels: Default::default(),
            allowed,
            tempo: 120,
            tempo_count: 0,
            tempo_rate: 0x100,
            master_vol: 0,
            sseq_vol,
            variables: [-1; 32],
            random: 0x1234_5678,
            sample_rate: SAMPLE_RATE as f64,
            timer_count: 0,
            gotos: Vec::new(),
        };
        for (i, c) in p.channels.iter_mut().enumerate() {
            c.id = i;
        }
        p.tracks[0].init(0);
        p.track_ids.push(0);
        p
    }

    fn rand(&mut self) -> u16 {
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.random >> 16) as u16
    }

    /// Rendu complet : `Snd_Timer` toutes les ~5,2 ms, mixage entre deux.
    /// Ajoute le rendu à `mix` (stéréo entrelacé) jusqu'à `frames` trames au
    /// total, ou jusqu'à la fin de la séquence si elle ne boucle pas.
    fn render_into(&mut self, mix: &mut Vec<f32>, frames: usize) {
        let samples_per_timer = SECONDS_PER_TIMER * self.sample_rate;
        let mut until_timer = samples_per_timer;
        self.timer();
        while mix.len() < frames * 2 {
            let (mut l, mut r) = (0f32, 0f32);
            for ch in self.channels.iter_mut() {
                if ch.state != ChState::None {
                    let s = ch.next_sample(&self.bank.waves);
                    l += s * ch.gain_l;
                    r += s * ch.gain_r;
                }
            }
            mix.push(l);
            mix.push(r);
            until_timer -= 1.0;
            if until_timer <= 0.0 {
                self.timer();
                until_timer += samples_per_timer;
                // Séquence terminée (sans boucle) et plus rien ne sonne.
                if self.track_ids.iter().all(|&t| self.tracks[t].ended) && self.channels.iter().all(|c| c.state == ChState::None) {
                    break;
                }
            }
        }
    }

    fn timer(&mut self) {
        self.timer_count += 1;
        // Chn_UpdateTracks : propage les changements des pistes aux voies.
        for c in 0..16 {
            self.channel_update_track(c);
        }
        for t in self.tracks.iter_mut() {
            t.upd = 0;
        }
        let sr = self.sample_rate;
        for ch in self.channels.iter_mut() {
            ch.update(sr);
        }
        // Player_Run
        while self.tempo_count >= 240 {
            self.tempo_count -= 240;
            for k in 0..self.track_ids.len() {
                let t = self.track_ids[k];
                self.run_track(t);
            }
        }
        self.tempo_count += (self.tempo * self.tempo_rate) >> 8;
    }

    fn channel_update_track(&mut self, c: usize) {
        let Some(t) = self.channels[c].track else { return };
        let trk = &self.tracks[t];
        let flags = trk.upd;
        if flags == 0 {
            return;
        }
        let ch = &mut self.channels[c];
        if flags & T_LEN != 0 && ch.state > ChState::Start {
            if ch.state < ChState::Release {
                ch.note_length -= 1;
                if ch.note_length == 0 {
                    ch.release();
                }
            }
            if ch.manual_sweep && ch.sweep_cnt < ch.sweep_len {
                ch.sweep_cnt += 1;
            }
        }
        if flags & T_VOL != 0 {
            ch.update_vol(trk, self.master_vol, self.sseq_vol);
            ch.flags |= C_VOL;
        }
        if flags & T_PAN != 0 {
            ch.ext_pan = trk.pan;
            ch.flags |= C_PAN;
        }
        if flags & T_TIMER != 0 {
            ch.update_tune(trk);
            ch.flags |= C_TIMER;
        }
        if flags & T_MOD != 0 {
            let mod_flag = |t: u8| match t {
                0 => C_TIMER,
                2 => C_PAN,
                _ => C_VOL,
            };
            let old = ch.mod_type;
            ch.update_mod(trk);
            if old != trk.mod_type {
                ch.flags |= mod_flag(old) | mod_flag(trk.mod_type);
            }
        }
    }

    fn alloc_channel(&mut self, kind: AllocKind, prio: u8) -> Option<usize> {
        const PCM: [usize; 16] = [4, 5, 6, 7, 2, 0, 3, 1, 8, 9, 10, 11, 14, 12, 15, 13];
        const PSG: [usize; 6] = [8, 9, 10, 11, 12, 13];
        const NOISE: [usize; 2] = [14, 15];
        let list: &[usize] = match kind {
            AllocKind::Pcm => &PCM,
            AllocKind::Psg => &PSG,
            AllocKind::Noise => &NOISE,
        };
        let mut cur: Option<usize> = None;
        for &n in list {
            if self.allowed & (1 << n) == 0 {
                continue;
            }
            if let Some(c) = cur {
                let (this, best) = (&self.channels[n], &self.channels[c]);
                if this.prio >= best.prio && (this.prio != best.prio || best.vol <= this.vol) {
                    continue;
                }
            }
            cur = Some(n);
        }
        let c = cur?;
        if prio < self.channels[c].prio {
            return None;
        }
        self.channels[c].note_length = -1;
        self.channels[c].vol = 0x7FF;
        Some(c)
    }

    /// Note_On : choisit l'instrument (batterie / partage du clavier), alloue une voie.
    fn note_on(&mut self, t: usize, key: u8, vel: i32, len: i32) {
        let bank = self.bank;
        let Some(entry) = bank.entries.get(self.tracks[t].patch as usize) else { return };
        let region = match entry.record {
            16 => {
                let (Some(first), Some(last)) = (entry.regions.first(), entry.regions.last()) else { return };
                if !(first.low <= key && key <= last.high) {
                    return;
                }
                entry.regions.get((key - first.low) as usize)
            }
            17 => entry.regions.iter().find(|r| key <= r.high),
            _ => entry.regions.first(),
        };
        let Some(inst) = region.map(|r| r.inst) else { return };
        let prio = self.tracks[t].prio;
        let (c, source, timer, start) = match inst.record {
            0 => return,
            2 | 3 => {
                let noise = inst.record == 3;
                let Some(c) = self.alloc_channel(if noise { AllocKind::Noise } else { AllocKind::Psg }, prio) else { return };
                let source = if noise { Source::Noise } else { Source::Psg((inst.swav & 7) as u8) };
                // Timer de la note n° 60 (do 4, 262 Hz × 8 pas).
                (c, source, (0x100_0000 / (262 * 8)) as u16, -1.0)
            }
            _ => {
                let Some(&w) = bank.wave_index.get(&(inst.swar, inst.swav)) else { return };
                let Some(c) = self.alloc_channel(AllocKind::Pcm, prio) else { return };
                (c, Source::Pcm(w), bank.waves[w].timer, -3.0)
            }
        };
        let trk = &self.tracks[t];
        let (master, sseq) = (self.master_vol, self.sseq_vol);
        let ch = &mut self.channels[c];
        ch.source = source;
        ch.base_timer = timer;
        ch.pos = start;
        ch.inc = 0.0;
        ch.psg_x = 0x7FFF;
        ch.psg_count = 0;
        ch.psg_last = 0.0;
        ch.state = ChState::Start;
        ch.track = Some(t);
        ch.flags = 0;
        ch.prio = prio;
        ch.key = key;
        ch.org_key = inst.key;
        ch.velocity = cnv_sust(vel as u8);
        ch.pan = inst.pan as i32 - 64;
        ch.mod_delay_cnt = 0;
        ch.mod_counter = 0;
        ch.note_length = len;
        ch.attack_lvl = cnv_attack(if trk.attack == 0xFF { inst.attack } else { trk.attack });
        ch.decay_rate = cnv_fall(if trk.decay == 0xFF { inst.decay } else { trk.decay });
        ch.sustain_lvl = if trk.sustain == 0xFF { inst.sustain } else { trk.sustain };
        ch.release_rate = cnv_fall(if trk.release == 0xFF { inst.release } else { trk.release });
        ch.update_vol(trk, master, sseq);
        ch.ext_pan = trk.pan;
        ch.update_tune(trk);
        ch.update_mod(trk);
        ch.update_porta(trk);
        self.tracks[t].porta_key = key;
    }

    /// Note_On_Tie : réutilise la note en cours de la piste (legato).
    fn note_on_tie(&mut self, t: usize, key: u8, vel: i32) {
        let Some(c) = (0..16)
            .find(|&i| self.channels[i].state > ChState::None && self.channels[i].track == Some(t) && self.channels[i].state != ChState::Release)
        else {
            return self.note_on(t, key, vel, -1);
        };
        let trk = &self.tracks[t];
        let (master, sseq) = (self.master_vol, self.sseq_vol);
        let ch = &mut self.channels[c];
        ch.flags = 0;
        ch.prio = trk.prio;
        ch.key = key;
        ch.velocity = cnv_sust(vel as u8);
        ch.mod_delay_cnt = 0;
        ch.mod_counter = 0;
        ch.update_vol(trk, master, sseq);
        ch.update_tune(trk);
        ch.update_mod(trk);
        ch.update_porta(trk);
        ch.flags |= C_TIMER;
        self.tracks[t].porta_key = key;
    }

    fn release_all_notes(&mut self, t: usize) {
        for ch in self.channels.iter_mut() {
            if ch.state > ChState::None && ch.track == Some(t) && ch.state != ChState::Release {
                ch.release();
            }
        }
    }

    /// Track_Run : un tick de séquence pour la piste `t`.
    fn run_track(&mut self, t: usize) {
        self.tracks[t].upd |= T_LEN;
        if self.tracks[t].ended {
            return;
        }
        if self.tracks[t].wait > 0 {
            self.tracks[t].wait -= 1;
            if self.tracks[t].wait > 0 {
                return;
            }
        }
        if self.run_commands(t).is_none() {
            // Lecture hors des données ou séquence sans attente : on arrête la piste.
            self.tracks[t].ended = true;
        }
    }

    fn run_commands(&mut self, t: usize) -> Option<()> {
        let d = self.data;
        let mut budget = 10_000;
        while self.tracks[t].wait == 0 {
            budget -= 1;
            if budget == 0 {
                return None;
            }
            let cmd = if self.tracks[t].ov.active { self.tracks[t].ov.cmd } else { self.tracks[t].r8(d)? };
            if cmd < 0x80 {
                let trk = &mut self.tracks[t];
                let key = (cmd as i32 + trk.transpose as i32) as u8;
                let vel = trk.v8(d, true)? as u8 as i32;
                let len = trk.vvl(d)?;
                if trk.note_wait {
                    trk.wait = len;
                }
                if trk.tie {
                    self.note_on_tie(t, key, vel);
                } else {
                    self.note_on(t, key & 0x7F, vel, len);
                }
            } else {
                match cmd {
                    0x93 => {
                        let trk = &mut self.tracks[t];
                        let _num = trk.r8(d)?;
                        let start = trk.r24(d)?;
                        if let Some(n) = self.tracks.iter().position(|tr| !tr.allocated) {
                            self.tracks[n].init(start);
                            self.track_ids.push(n);
                        }
                    }
                    0x80 => self.tracks[t].wait = self.tracks[t].vvl(d)?,
                    0x81 => self.tracks[t].patch = self.tracks[t].vvl(d)? as u16,
                    0x94 => {
                        let dest = self.tracks[t].r24(d)?;
                        self.tracks[t].pos = dest;
                        if self.gotos.len() < 256 {
                            self.gotos.push((t, self.timer_count));
                        }
                    }
                    0x95 => {
                        let trk = &mut self.tracks[t];
                        let dest = trk.r24(d)?;
                        if trk.stack_pos < STACK_SIZE {
                            trk.stack[trk.stack_pos] = (StackKind::Call, trk.pos);
                            trk.stack_pos += 1;
                            trk.pos = dest;
                        }
                    }
                    0xFD => {
                        let trk = &mut self.tracks[t];
                        if trk.stack_pos > 0 && trk.stack[trk.stack_pos - 1].0 == StackKind::Call {
                            trk.stack_pos -= 1;
                            trk.pos = trk.stack[trk.stack_pos].1;
                        }
                    }
                    0xC0 => {
                        let trk = &mut self.tracks[t];
                        trk.pan = trk.v8(d, false)? as u8 as i32 - 64;
                        trk.upd |= T_PAN;
                    }
                    0xC1 => {
                        let trk = &mut self.tracks[t];
                        trk.vol = trk.v8(d, false)? as u8;
                        trk.upd |= T_VOL;
                    }
                    0xC2 => {
                        self.master_vol = cnv_sust(self.tracks[t].v8(d, false)? as u8);
                        for &id in &self.track_ids {
                            self.tracks[id].upd |= T_VOL;
                        }
                    }
                    0xC6 => {
                        let p = self.tracks[t].r8(d)?;
                        self.tracks[t].prio = p;
                    }
                    0xC7 => self.tracks[t].note_wait = self.tracks[t].r8(d)? != 0,
                    0xC8 => {
                        self.tracks[t].tie = self.tracks[t].r8(d)? != 0;
                        self.release_all_notes(t);
                    }
                    0xD5 => {
                        let trk = &mut self.tracks[t];
                        trk.expr = trk.v8(d, false)? as u8;
                        trk.upd |= T_VOL;
                    }
                    0xE1 => self.tempo = self.tracks[t].r16(d)? as i32,
                    0xFF => {
                        self.tracks[t].ended = true;
                        return Some(());
                    }
                    0xD4 => {
                        let trk = &mut self.tracks[t];
                        let count = trk.v8(d, false)? as u8;
                        if trk.stack_pos < STACK_SIZE {
                            trk.loop_count[trk.stack_pos] = count;
                            trk.stack[trk.stack_pos] = (StackKind::Loop, trk.pos);
                            trk.stack_pos += 1;
                        }
                    }
                    0xFC => {
                        let trk = &mut self.tracks[t];
                        if trk.stack_pos > 0 && trk.stack[trk.stack_pos - 1].0 == StackKind::Loop {
                            let i = trk.stack_pos - 1;
                            let prev = trk.loop_count[i];
                            if prev == 0 {
                                // Boucle infinie : sert de point de boucle comme un `goto`.
                                trk.pos = trk.stack[i].1;
                                if self.gotos.len() < 256 {
                                    self.gotos.push((t, self.timer_count));
                                }
                            } else {
                                trk.loop_count[i] -= 1;
                                if trk.loop_count[i] != 0 {
                                    trk.pos = trk.stack[i].1;
                                } else {
                                    trk.stack_pos -= 1;
                                }
                            }
                        }
                    }
                    0xC3 => self.tracks[t].transpose = self.tracks[t].v8(d, false)? as i8,
                    0xC4 => {
                        let trk = &mut self.tracks[t];
                        trk.bend = trk.v8(d, false)? as i8;
                        trk.upd |= T_TIMER;
                    }
                    0xC5 => {
                        let trk = &mut self.tracks[t];
                        trk.bend_range = trk.r8(d)?;
                        trk.upd |= T_TIMER;
                    }
                    0xD0 => self.tracks[t].attack = self.tracks[t].v8(d, false)? as u8,
                    0xD1 => self.tracks[t].decay = self.tracks[t].v8(d, false)? as u8,
                    0xD2 => self.tracks[t].sustain = self.tracks[t].v8(d, false)? as u8,
                    0xD3 => self.tracks[t].release = self.tracks[t].v8(d, false)? as u8,
                    0xC9 => {
                        let trk = &mut self.tracks[t];
                        trk.porta_key = (trk.r8(d)? as i32 + trk.transpose as i32) as u8;
                        trk.porta = true;
                    }
                    0xCE => self.tracks[t].porta = self.tracks[t].r8(d)? != 0,
                    0xCF => self.tracks[t].porta_time = self.tracks[t].v8(d, false)? as u8,
                    0xE3 => {
                        let trk = &mut self.tracks[t];
                        trk.sweep_pitch = trk.v16(d)? as i16 as i32;
                        trk.porta = true;
                    }
                    0xCA => {
                        let trk = &mut self.tracks[t];
                        trk.mod_depth = trk.v8(d, false)? as u8;
                        trk.upd |= T_MOD;
                    }
                    0xCB => {
                        let trk = &mut self.tracks[t];
                        trk.mod_speed = trk.v8(d, false)? as u8;
                        trk.upd |= T_MOD;
                    }
                    0xCC => {
                        let trk = &mut self.tracks[t];
                        trk.mod_type = trk.r8(d)?;
                        trk.upd |= T_MOD;
                    }
                    0xCD => {
                        let trk = &mut self.tracks[t];
                        trk.mod_range = trk.r8(d)?;
                        trk.upd |= T_MOD;
                    }
                    0xE0 => {
                        let trk = &mut self.tracks[t];
                        trk.mod_delay = trk.v16(d)? as u16;
                        trk.upd |= T_MOD;
                    }
                    0xA0 => {
                        // Aléatoire : la commande suivante prend une valeur tirée au sort.
                        let trk = &mut self.tracks[t];
                        let sub = trk.r8(d)?;
                        let extra = if (0xB0..=0xBD).contains(&sub) || sub < 0x80 { trk.r8(d)? as i32 } else { 0 };
                        let min = trk.r16(d)? as i16 as i32;
                        let max = trk.r16(d)? as i16 as i32;
                        let range = (max - min + 1).max(1);
                        let r = self.rand() as i32;
                        self.tracks[t].ov = Override { active: true, cmd: sub, value: r % range + min, extra };
                    }
                    0xA1 => {
                        let trk = &mut self.tracks[t];
                        let sub = trk.r8(d)?;
                        let extra = if (0xB0..=0xBD).contains(&sub) || sub < 0x80 { trk.r8(d)? as i32 } else { 0 };
                        let var = trk.r8(d)? as usize;
                        let value = self.variables.get(var).copied().unwrap_or(0) as i32;
                        self.tracks[t].ov = Override { active: true, cmd: sub, value, extra };
                    }
                    0xB0..=0xB6 => {
                        let trk = &mut self.tracks[t];
                        let var = trk.v8(d, true)? as u8 as usize;
                        let value = trk.v16(d)? as i16;
                        if var < 32 {
                            let cur = self.variables[var];
                            self.variables[var] = match cmd {
                                0xB0 => value,
                                0xB1 => cur.wrapping_add(value),
                                0xB2 => cur.wrapping_sub(value),
                                0xB3 => cur.wrapping_mul(value),
                                0xB4 if value != 0 => cur.wrapping_div(value),
                                0xB4 => cur,
                                0xB5 if value < 0 => cur >> (-(value as i32)).min(15),
                                0xB5 => cur.wrapping_shl(value as u32),
                                _ => {
                                    let r = self.rand() as i32;
                                    if value < 0 {
                                        -(r % (-(value as i32) + 1)) as i16
                                    } else {
                                        (r % (value as i32 + 1)) as i16
                                    }
                                }
                            };
                        }
                    }
                    0xB8..=0xBD => {
                        let trk = &mut self.tracks[t];
                        let var = trk.v8(d, true)? as u8 as usize;
                        let value = trk.v16(d)? as i16;
                        let cur = self.variables.get(var).copied().unwrap_or(-1);
                        self.tracks[t].last_cmp = match cmd {
                            0xB8 => cur == value,
                            0xB9 => cur >= value,
                            0xBA => cur > value,
                            0xBB => cur <= value,
                            0xBC => cur < value,
                            _ => cur != value,
                        };
                    }
                    0xA2 => {
                        // Si : saute la commande suivante quand la comparaison a échoué.
                        let trk = &mut self.tracks[t];
                        if !trk.last_cmp {
                            let next = trk.r8(d)?;
                            let (mut bytes, var_len, extra) = command_size(next);
                            if extra {
                                let sub = trk.r8(d)?;
                                if (0xB0..=0xBD).contains(&sub) || sub < 0x80 {
                                    bytes += 1;
                                }
                            }
                            trk.pos += bytes;
                            if var_len {
                                trk.rvl(d)?;
                            }
                        }
                    }
                    _ => {
                        // 0xFE (pistes allouées), 0xD6 (affichage d'une variable),
                        // 0xD7 (sourdine) et commandes inconnues : on saute les arguments.
                        let (bytes, var_len, _) = command_size(cmd);
                        self.tracks[t].pos += bytes;
                        if var_len {
                            self.tracks[t].rvl(d)?;
                        }
                    }
                }
            }
            if cmd != 0xA0 && cmd != 0xA1 {
                self.tracks[t].ov.active = false;
            }
        }
        Some(())
    }
}

/// Taille des arguments d'une commande : (octets fixes, suivi d'une longueur
/// variable, suivi d'un octet supplémentaire selon la sous-commande).
fn command_size(cmd: u8) -> (usize, bool, bool) {
    match cmd {
        0x00..=0x7F => (1, true, false),
        0x80 | 0x81 => (0, true, false),
        0xC0..=0xD7 => (1, false, false),
        0xFE | 0xE1 | 0xE3 | 0xE0 => (2, false, false),
        0x94 | 0x95 | 0xB0..=0xB6 | 0xB8..=0xBD => (3, false, false),
        0x93 => (4, false, false),
        0xA1 => (1, false, true),
        0xA0 => (4, false, true),
        _ => (0, false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ima_adpcm_decodes_known_vector() {
        // Échantillon initial 0, index 0. Quartet 7 : pas 7 → diff = 0 + 7 + 3 + 1 = 11,
        // index 8 (pas 16). Quartet 0xF : diff = 2 + 16 + 8 + 4 = 30 → 11 - 30 = -19,
        // index 16 (pas 34). Quartet 0 : diff = 34 >> 3 = 4 → -15, index 15.
        // Quartet 8 : pas 31, diff = 3 → -18.
        let raw = [0x00, 0x00, 0x00, 0x00, 0xF7, 0x80];
        assert_eq!(decode_ima_adpcm(&raw), vec![11, -19, -15, -18]);
    }

    #[test]
    fn ima_adpcm_clamps() {
        let raw = [0xFF, 0x7F, 88, 0x00, 0x77];
        assert_eq!(decode_ima_adpcm(&raw), vec![0x7FFF, 0x7FFF]);
    }

    #[test]
    fn envelope_conversions_match_driver() {
        assert_eq!(cnv_attack(0x7F), 0x00);
        assert_eq!(cnv_attack(0x6D), 0x8F);
        assert_eq!(cnv_attack(0), 0xFF);
        assert_eq!(cnv_fall(0x7F), 0xFFFF);
        assert_eq!(cnv_fall(0x7E), 0x3C00);
        assert_eq!(cnv_fall(10), 21);
        assert_eq!(cnv_fall(0x64), 0x1E00 / 0x1A);
        assert_eq!(cnv_sust(127), 0);
        assert_eq!(cnv_sust(64), -119);
        assert_eq!(cnv_scale(127), 0);
        assert_eq!(cnv_scale(64), -60);
    }

    #[test]
    fn volume_and_pitch_conversions() {
        assert_eq!(ampl_to_gain(0), 1.0);
        assert_eq!(ampl_to_gain(-AMPL_K), 0.0);
        assert!((ampl_to_gain(-60) - 0.501).abs() < 0.01); // -6 dB
                                                           // Volume 64 en courbe quadratique ≈ (64/127)².
        assert!((ampl_to_gain(cnv_sust(64)) - (64.0f32 / 127.0).powi(2)).abs() < 0.01);
        // 768 = une octave : le timer est divisé par deux.
        assert_eq!(timer_adjust(0x2000, 768), 0x1000);
        assert_eq!(timer_adjust(0x2000, -768), 0x4000);
        // Table getpitchtbl[1] = 0x3B : 0x10000 × (1 + 0x3B / 0x10000) → 0x1003B / 2^16.
        assert_eq!(timer_adjust(0xFFFF, -1), 0xFFFF);
        assert_eq!(cnv_sine(0), 0);
        assert_eq!(cnv_sine(32), 127);
        assert_eq!(cnv_sine(96), -127);
        assert_eq!(cnv_sine(64), 0);
    }

    /// SDAT minimal : une séquence (une note puis fin), une banque à un
    /// instrument PCM8 bouclé, une archive d'un échantillon, un lecteur.
    fn synthetic_sdat() -> Vec<u8> {
        // SSEQ : note 69 (la 4) vélocité 127 durée 48, puis fin.
        let seq_data = [69u8, 127, 48, 0xFF];
        let mut sseq = b"SSEQ\xFF\xFE\x00\x01".to_vec();
        sseq.extend_from_slice(&((0x1C + seq_data.len()) as u32).to_le_bytes());
        sseq.extend_from_slice(&[0x10, 0, 1, 0]);
        sseq.extend_from_slice(b"DATA");
        sseq.extend_from_slice(&((0x0C + seq_data.len()) as u32).to_le_bytes());
        sseq.extend_from_slice(&0x1Cu32.to_le_bytes());
        sseq.extend_from_slice(&seq_data);

        // SBNK : un instrument PCM (archive 0, échantillon 0, note de base 69).
        let mut sbnk = b"SBNK\xFF\xFE\x00\x01".to_vec();
        sbnk.extend_from_slice(&[0; 8]);
        sbnk.extend_from_slice(b"DATA");
        sbnk.extend_from_slice(&[0; 4]);
        sbnk.extend_from_slice(&[0; 32]);
        sbnk.extend_from_slice(&1u32.to_le_bytes());
        sbnk.extend_from_slice(&[1, 0x40, 0, 0]); // type 1, décalage 0x40
        sbnk.extend_from_slice(&[0, 0, 0, 0, 69, 127, 127, 127, 127, 64]);

        // SWAR : un SWAV PCM8 en boucle, une période de carré sur 8 échantillons,
        // timer pour 440 × 8 Hz.
        let timer = (ARM7_CLOCK / 2.0 / (440.0 * 8.0)) as u16;
        let mut swar = b"SWAR\xFF\xFE\x00\x01".to_vec();
        swar.extend_from_slice(&[0; 8]);
        swar.extend_from_slice(b"DATA");
        swar.extend_from_slice(&[0; 4]);
        swar.extend_from_slice(&[0; 32]);
        swar.extend_from_slice(&1u32.to_le_bytes());
        swar.extend_from_slice(&0x40u32.to_le_bytes());
        swar.extend_from_slice(&[0, 1]);
        swar.extend_from_slice(&3520u16.to_le_bytes());
        swar.extend_from_slice(&timer.to_le_bytes());
        swar.extend_from_slice(&0u16.to_le_bytes());
        swar.extend_from_slice(&2u32.to_le_bytes());
        swar.extend_from_slice(&[0x60, 0x60, 0x60, 0x60, 0xA0, 0xA0, 0xA0, 0xA0]);

        let files = [sseq, sbnk, swar];
        // Disposition : en-tête 0x40, SYMB, INFO, FAT, fichiers.
        let mut symb = b"SYMB".to_vec();
        symb.extend_from_slice(&[0; 4]);
        let rec_start = 8 + 4 * 8;
        for i in 0..8u32 {
            symb.extend_from_slice(&(rec_start + 8 * i).to_le_bytes());
        }
        for i in 0..8 {
            symb.extend_from_slice(&(if i == 0 { 1u32 } else { 0 }).to_le_bytes());
            symb.extend_from_slice(&(if i == 0 { rec_start + 64 } else { 0 }).to_le_bytes());
        }
        symb.extend_from_slice(b"SEQ_TEST_TITLE01\0\0");

        let mut info = b"INFO".to_vec();
        info.extend_from_slice(&[0; 4]);
        let mut recs = Vec::new();
        let base = 8 + 4 * 8;
        let entry_sizes = [12usize, 0, 12, 4, 8, 0, 0, 0];
        let mut payload = Vec::new();
        let mut offsets = Vec::new();
        let mut pos = base + 8 * 8;
        for (i, &size) in entry_sizes.iter().enumerate() {
            offsets.push(base + 8 * i);
            if size == 0 {
                recs.extend_from_slice(&0u32.to_le_bytes());
                recs.extend_from_slice(&0u32.to_le_bytes());
            } else {
                recs.extend_from_slice(&1u32.to_le_bytes());
                recs.extend_from_slice(&(pos as u32).to_le_bytes());
                let mut e = vec![0u8; size];
                match i {
                    0 => e[..10].copy_from_slice(&[0, 0, 0, 0, 0, 0, 127, 64, 64, 0]),
                    2 => e.copy_from_slice(&[1, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
                    3 => e.copy_from_slice(&[2, 0, 0, 0]),
                    _ => e.copy_from_slice(&[1, 0, 0xFF, 0xFF, 0, 0, 0, 0]),
                }
                payload.extend_from_slice(&e);
                pos += size;
            }
        }
        for o in offsets {
            info.extend_from_slice(&(o as u32).to_le_bytes());
        }
        info.extend_from_slice(&recs);
        info.extend_from_slice(&payload);

        let symb_off = 0x40;
        let info_off = symb_off + symb.len();
        let fat_off = info_off + info.len();
        let fat_len = 12 + 16 * files.len();
        let mut file_off = fat_off + fat_len;
        let mut fat = b"FAT ".to_vec();
        fat.extend_from_slice(&(fat_len as u32).to_le_bytes());
        fat.extend_from_slice(&(files.len() as u32).to_le_bytes());
        for f in &files {
            fat.extend_from_slice(&(file_off as u32).to_le_bytes());
            fat.extend_from_slice(&(f.len() as u32).to_le_bytes());
            fat.extend_from_slice(&[0; 8]);
            file_off += f.len();
        }

        let mut sdat = b"SDAT\xFF\xFE\x00\x01".to_vec();
        sdat.extend_from_slice(&(file_off as u32).to_le_bytes());
        sdat.extend_from_slice(&[0x40, 0, 4, 0]);
        for (o, l) in [(symb_off, symb.len()), (info_off, info.len()), (fat_off, fat_len), (fat_off + fat_len, file_off - fat_off - fat_len)] {
            sdat.extend_from_slice(&(o as u32).to_le_bytes());
            sdat.extend_from_slice(&(l as u32).to_le_bytes());
        }
        sdat.resize(0x40, 0);
        sdat.extend_from_slice(&symb);
        sdat.extend_from_slice(&info);
        sdat.extend_from_slice(&fat);
        for f in &files {
            sdat.extend_from_slice(f);
        }
        sdat
    }

    #[test]
    fn parses_synthetic_sdat() {
        let data = synthetic_sdat();
        let sdat = Sdat::parse(&data).unwrap();
        assert_eq!(sdat.sequence_names(), vec![Some("SEQ_TEST_TITLE01".to_string())]);
        let info = sdat.sequence_info(0).unwrap();
        assert_eq!((info.file, info.bank, info.volume, info.player), (0, 0, 127, 0));
        assert_eq!(sdat.players, vec![Some(0xFFFF)]);
        let bank = load_bank(&sdat, 0).unwrap();
        assert_eq!(bank.entries.len(), 1);
        assert_eq!(bank.entries[0].regions[0].inst.key, 69);
        assert_eq!(bank.waves.len(), 1);
        assert_eq!(bank.waves[0].data.len(), 8);
        assert!(bank.waves[0].looped);
        // Repli sur la recherche par suffixe.
        assert_eq!(title_sequence(&sdat, Game::X), Some(0));
    }

    #[test]
    fn renders_synthetic_note_at_expected_pitch() {
        let data = synthetic_sdat();
        let sdat = Sdat::parse(&data).unwrap();
        let pcm = render_sequence(&sdat, 0, 2.0).unwrap();
        // 48 ticks à 120 bpm ≈ 0,5 s de note, puis release instantané et fin.
        assert!(pcm.seconds() > 0.5 && pcm.seconds() < 1.5, "{}", pcm.seconds());
        let left: Vec<f32> = pcm.samples.iter().step_by(2).map(|&s| s as f32).collect();
        // Passages par zéro (front montant) sur la partie tenue → fréquence.
        let part = &left[2000..12000];
        let crossings = part.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let freq = crossings as f32 / (part.len() as f32 / SAMPLE_RATE as f32);
        assert!((freq - 440.0).abs() < 5.0, "fréquence {freq}");
    }

    /// Rend le thème de l'écran titre de chaque ROM de test présente
    /// (dossier `KALEIDO_TEST_ROMS`, par défaut celui de l'auteur) et écrit les
    /// WAV dans le dossier temporaire `kaleido-music-nds`.
    #[test]
    #[ignore]
    fn renders_title_theme_of_real_roms() {
        let dir = std::env::var("KALEIDO_TEST_ROMS").unwrap_or_else(|_| r"C:\Users\Thisma\Documents\NDS & 3DS".into());
        let out = std::env::temp_dir().join("kaleido-music-nds");
        std::fs::create_dir_all(&out).unwrap();
        let cases = [
            ("Pokemon - Version Diamant (France) (Rev 5).nds", Game::Diamond, &["SEQ_TITLE01"][..], "diamant"),
            ("Pokemon - Platinum Version (Europe).nds", Game::Platinum, &["SEQ_TITLE01"], "platine"),
            ("Pokemon - Version Argent SoulSilver (France).nds", Game::SoulSilver, &["SEQ_GS_POKEMON_THEME"], "soulsilver"),
            ("Pokemon - Version Blanche (France) (NDSi Enhanced).nds", Game::White, &["SEQ_BGM_POKEMON_THEME"], "blanche"),
            ("Pokemon - Version Noire 2 (France) (NDSi Enhanced).nds", Game::Black2, &["SEQ_BGM_TITLE01", "SEQ_BGM_POKEMON_THEME"], "noire2"),
        ];
        for (file, game, expected, short) in cases {
            let path = Path::new(&dir).join(file);
            if !path.exists() {
                eprintln!("absente : {}", path.display());
                continue;
            }
            let rom = NdsRom::open(&path).unwrap();
            let sdat = Sdat::parse(find_sdat(&rom).unwrap()).unwrap();
            let names = sdat.sequence_names();
            let seqs: Vec<&str> = title_sequences(&sdat, game).iter().map(|&i| names[i].as_deref().unwrap()).collect();
            assert_eq!(seqs, expected, "{file}");

            let start = std::time::Instant::now();
            let pcm = title_theme(&path, game, 60.0).unwrap();
            let elapsed = start.elapsed();
            std::fs::write(out.join(format!("{short}.wav")), pcm.to_wav()).unwrap();
            let peak = pcm.samples.iter().map(|&s| (s as i32).abs()).max().unwrap();
            let rms = (pcm.samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / pcm.samples.len() as f64).sqrt();
            let clipped = pcm.samples.iter().filter(|&&s| s == i16::MAX || s == i16::MIN).count();
            eprintln!("{short} : {:.1} s, crête {peak}, RMS {rms:.0}, écrêtés {clipped}, {elapsed:?} (ouverture de la ROM comprise)", pcm.seconds());
            assert!((pcm.seconds() - 60.0).abs() < 0.01, "{file} : {} s", pcm.seconds());
            assert!(peak > 20_000 && clipped == 0, "{file}");
            assert!(rms > 1_500.0 && rms < 12_000.0, "{file} : RMS {rms}");
            // Pas de trou : chaque seconde contient du son.
            for (i, sec) in pcm.samples.chunks(2 * SAMPLE_RATE as usize).enumerate() {
                let r = (sec.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / sec.len() as f64).sqrt();
                assert!(r > 50.0, "{file} : silence à {i} s");
            }
        }
    }
}
