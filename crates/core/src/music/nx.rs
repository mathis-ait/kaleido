//! Musique des jeux Switch : sons Wwise du jeu de l'utilisateur.
//!
//! Les jeux Pokémon Switch utilisent Wwise. Les musiques sont décrites dans une banque
//! (`.bnk`, objets HIRC : évènement → action → conteneurs → pistes) et leurs sons sont
//! rangés dans une archive AKPK (`.pck`, table des flux : id, taille de bloc, taille,
//! bloc de départ). Un son est un RIFF/WAVE au codec `0x3039` (« Opus NX ») : en-tête
//! `fmt ` (canaux, fréquence, nombre d'échantillons en +0x18, taille de la table des
//! trames en +0x24), puis dans `data` une table d'offsets de trames (u32) suivie de
//! paquets Opus bruts de 20 ms.
//!
//! Plutôt que de décoder l'Opus, les paquets sont simplement réempaquetés dans un
//! fichier Ogg Opus standard (RFC 7845), que le moteur web de l'application décode.

use std::collections::{HashMap, HashSet};

/// Hachage FNV-1 32 bits des noms Wwise (en minuscules).
pub fn wwise_hash(name: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in name.to_ascii_lowercase().bytes() {
        h = h.wrapping_mul(16777619) ^ b as u32;
    }
    h
}

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

// ---------------------------------------------------------------------------
// Banque (.bnk) et archive (.pck)

/// Objets HIRC d'une banque : id → (type, données).
pub fn bank_objects(bank: &[u8]) -> HashMap<u32, (u8, Vec<u8>)> {
    let mut objs = HashMap::new();
    let mut o = 0;
    while let (Some(tag), Some(size)) = (bank.get(o..o + 4), u32le(bank, o + 4)) {
        let size = size as usize;
        if tag == b"HIRC" {
            let n = u32le(bank, o + 8).unwrap_or(0);
            let mut p = o + 12;
            for _ in 0..n {
                let (Some(&t), Some(len), Some(id)) = (bank.get(p), u32le(bank, p + 1), u32le(bank, p + 5)) else { break };
                let end = p + 5 + len as usize;
                let Some(data) = bank.get(p + 9..end) else { break };
                objs.insert(id, (t, data.to_vec()));
                p = end;
            }
        }
        o += 8 + size;
    }
    objs
}

/// Flux d'une archive AKPK : id → (offset, taille). `header` : au moins l'en-tête complet.
pub fn pck_streams(header: &[u8]) -> HashMap<u32, (u64, u64)> {
    let mut out = HashMap::new();
    if header.get(..4) != Some(b"AKPK") {
        return out;
    }
    let (Some(lang), Some(banks)) = (u32le(header, 12), u32le(header, 16)) else { return out };
    let lut = 28 + lang as usize + banks as usize;
    let n = u32le(header, lut).unwrap_or(0) as usize;
    for i in 0..n {
        let p = lut + 4 + i * 20;
        let (Some(id), Some(block), Some(size), Some(start)) = (u32le(header, p), u32le(header, p + 4), u32le(header, p + 8), u32le(header, p + 12)) else { break };
        out.insert(id, (start as u64 * block.max(1) as u64, size as u64));
    }
    out
}

/// Taille de l'en-tête d'une archive AKPK (à lire en entier pour `pck_streams`).
pub fn pck_header_size(first: &[u8]) -> Option<usize> {
    (first.get(..4)? == b"AKPK").then(|| u32le(first, 4).map(|s| s as usize + 8))?
}

/// Sons (ids de flux) joués par un évènement : parcours du graphe des objets en suivant
/// tout identifiant connu (sans dépendre de la version du format des objets).
pub fn event_sources(objs: &HashMap<u32, (u8, Vec<u8>)>, event: u32, streams: &HashMap<u32, (u64, u64)>) -> Vec<u32> {
    const EVENT: u8 = 4;
    const ACTION: u8 = 3;
    const MUSIC_TRACK: u8 = 11;
    let mut seen = HashSet::new();
    let mut stack = vec![event];
    let mut sources = Vec::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some((t, data)) = objs.get(&id) else { continue };
        for p in 0..data.len().saturating_sub(3) {
            let v = u32le(data, p).unwrap();
            if *t == MUSIC_TRACK && streams.contains_key(&v) && !sources.contains(&v) {
                sources.push(v);
            }
            if v != id {
                if let Some((vt, _)) = objs.get(&v) {
                    // Pas de remontée vers d'autres évènements ; les objets musicaux ne
                    // déclenchent pas d'actions.
                    if *vt != EVENT && !(*t >= 10 && *vt == ACTION) {
                        stack.push(v);
                    }
                }
            }
        }
    }
    sources
}

// ---------------------------------------------------------------------------
// Wwise Opus NX → Ogg Opus

#[derive(Debug, Clone)]
pub struct WwiseOpus {
    pub channels: u8,
    pub sample_rate: u32,
    pub samples: u64,
    pub packets: Vec<Vec<u8>>,
}

impl WwiseOpus {
    pub fn seconds(&self) -> f32 {
        self.samples as f32 / self.sample_rate.max(1) as f32
    }
}

/// Lit un son Wwise au codec Opus NX (`0x3039`).
pub fn parse_wwise_opus(wem: &[u8]) -> Result<WwiseOpus, String> {
    if wem.get(..4) != Some(b"RIFF") || wem.get(8..12) != Some(b"WAVE") {
        return Err("son Wwise illisible (pas de RIFF)".into());
    }
    let mut fmt = None;
    let mut data = None;
    let mut o = 12;
    while let (Some(tag), Some(size)) = (wem.get(o..o + 4), u32le(wem, o + 4)) {
        let body = wem.get(o + 8..(o + 8 + size as usize).min(wem.len())).unwrap_or(&[]);
        match tag {
            b"fmt " => fmt = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        o += 8 + size as usize + (size as usize & 1);
    }
    let fmt = fmt.ok_or("son Wwise sans en-tête fmt")?;
    let data = data.ok_or("son Wwise sans données")?;
    let codec = u16le(fmt, 0).unwrap_or(0);
    if codec != 0x3039 {
        return Err(format!("codec Wwise {codec:#06x} non pris en charge"));
    }
    let channels = u16le(fmt, 2).unwrap_or(0) as u8;
    let sample_rate = u32le(fmt, 4).unwrap_or(48000);
    let samples = u32le(fmt, 0x18).unwrap_or(0) as u64;
    let table = u32le(fmt, 0x24).ok_or("en-tête Opus NX incomplet")? as usize;
    if table % 4 != 0 || table > data.len() || channels == 0 || channels > 2 {
        return Err("table des trames Opus invalide".into());
    }
    let offsets: Vec<usize> = (0..table / 4).map(|i| u32le(data, i * 4).unwrap() as usize).collect();
    let body = &data[table..];
    let mut packets = Vec::with_capacity(offsets.len());
    for (i, &start) in offsets.iter().enumerate() {
        let end = offsets.get(i + 1).copied().unwrap_or(body.len());
        let mut p = body.get(start..end).ok_or("trame Opus hors des données")?;
        // Trame « Nintendo » : taille (u32 gros-boutiste) et état final du codeur, puis le paquet Opus.
        if p.len() > 8 && u32::from_be_bytes(p[..4].try_into().unwrap()) as usize == p.len() - 8 {
            p = &p[8..];
        }
        if p.is_empty() {
            return Err("trame Opus vide".into());
        }
        packets.push(p.to_vec());
    }
    Ok(WwiseOpus { channels, sample_rate, samples, packets })
}

fn ogg_crc(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut r = (i as u32) << 24;
            for _ in 0..8 {
                r = if r & 0x8000_0000 != 0 { (r << 1) ^ 0x04C1_1DB7 } else { r << 1 };
            }
            *e = r;
        }
        t
    });
    data.iter().fold(0u32, |crc, &b| (crc << 8) ^ table[((crc >> 24) as u8 ^ b) as usize])
}

fn ogg_page(out: &mut Vec<u8>, packets: &[&[u8]], granule: u64, seq: u32, flags: u8) {
    let mut lacing = Vec::new();
    for p in packets {
        let mut n = p.len();
        while n >= 255 {
            lacing.push(255u8);
            n -= 255;
        }
        lacing.push(n as u8);
    }
    let start = out.len();
    out.extend_from_slice(b"OggS");
    out.push(0);
    out.push(flags);
    out.extend_from_slice(&granule.to_le_bytes());
    out.extend_from_slice(&0x4B41_4C45u32.to_le_bytes()); // numéro de flux
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&[0; 4]); // CRC, calculé ensuite
    out.push(lacing.len() as u8);
    out.extend_from_slice(&lacing);
    for p in packets {
        out.extend_from_slice(p);
    }
    let crc = ogg_crc(&out[start..]);
    out[start + 22..start + 26].copy_from_slice(&crc.to_le_bytes());
}

/// Fichier Ogg Opus avec au plus `max_seconds` de son (0 : tout).
pub fn to_ogg_opus(s: &WwiseOpus, max_seconds: f32) -> Vec<u8> {
    const PRE_SKIP: u16 = 0;
    // Durée d'une trame, lue dans le premier paquet (TOC Opus), 20 ms par défaut.
    let frame = s.packets.first().map_or(960, |p| opus_packet_samples(p)).max(120) as u64;
    let mut count = s.packets.len();
    if max_seconds > 0.0 {
        count = count.min(((max_seconds * 48000.0) as u64 / frame + 1) as usize);
    }
    let total = if count == s.packets.len() && s.samples > 0 { s.samples } else { count as u64 * frame };

    let mut out = Vec::new();
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(s.channels);
    head.extend_from_slice(&PRE_SKIP.to_le_bytes());
    head.extend_from_slice(&s.sample_rate.to_le_bytes());
    head.extend_from_slice(&0u16.to_le_bytes());
    head.push(0);
    ogg_page(&mut out, &[&head], 0, 0, 0x02);
    let mut tags = b"OpusTags".to_vec();
    let vendor = b"Kaleido";
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    ogg_page(&mut out, &[&tags], 0, 1, 0);

    let mut seq = 2;
    let mut done = 0usize;
    // 50 paquets (une seconde) par page au plus, et jamais plus de 255 segments.
    while done < count {
        let mut page: Vec<&[u8]> = Vec::new();
        let mut segments = 0;
        while done + page.len() < count && page.len() < 50 {
            let p = &s.packets[done + page.len()];
            let segs = p.len() / 255 + 1;
            if segments + segs > 255 {
                break;
            }
            segments += segs;
            page.push(p);
        }
        done += page.len();
        let last = done >= count;
        let granule = if last { total + PRE_SKIP as u64 } else { (done as u64 * frame).min(total) + PRE_SKIP as u64 };
        ogg_page(&mut out, &page, granule, seq, if last { 0x04 } else { 0 });
        seq += 1;
    }
    out
}

/// Nombre d'échantillons (à 48 kHz) d'un paquet Opus, d'après son octet TOC.
pub fn opus_packet_samples(p: &[u8]) -> u32 {
    let Some(&toc) = p.first() else { return 0 };
    let config = toc >> 3;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][(config & 3) as usize],
        12..=15 => [480, 960][(config & 1) as usize],
        _ => [120, 240, 480, 960][(config & 3) as usize],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => p.get(1).map_or(1, |b| (b & 0x3F) as u32),
    };
    frame * frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes() {
        // Noms retrouvés dans Légendes Arceus (BGM.bnk).
        assert_eq!(wwise_hash("PLAY_BGM_HA_SYS_ENDING"), 467728242);
        assert_eq!(wwise_hash("play_bgm_ha_fi_area00"), 54688062);
    }

    #[test]
    fn ogg_crc_vector() {
        // CRC Ogg (polynôme 0x04C11DB7, sans réflexion) de « 123456789 ».
        assert_eq!(ogg_crc(b"123456789"), 0x89A1_897F);
    }

    fn fake_wem(packets: &[&[u8]]) -> Vec<u8> {
        let mut table = Vec::new();
        let mut body = Vec::new();
        for p in packets {
            table.extend_from_slice(&(body.len() as u32).to_le_bytes());
            body.extend_from_slice(p);
        }
        let mut fmt = vec![0u8; 0x28];
        fmt[0..2].copy_from_slice(&0x3039u16.to_le_bytes());
        fmt[2..4].copy_from_slice(&2u16.to_le_bytes());
        fmt[4..8].copy_from_slice(&48000u32.to_le_bytes());
        fmt[0x18..0x1C].copy_from_slice(&((packets.len() * 960) as u32).to_le_bytes());
        fmt[0x24..0x28].copy_from_slice(&(table.len() as u32).to_le_bytes());
        let mut data = table;
        data.extend(body);
        let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        out.extend(fmt);
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend(data);
        out
    }

    #[test]
    fn wem_to_ogg() {
        // TOC 0xFC : CELT plein bande 20 ms, stéréo, une trame.
        let p1 = [0xFC, 1, 2, 3];
        let p2 = vec![0xFC; 300];
        // Le second paquet avec l'en-tête Nintendo de 8 octets, retiré à la lecture.
        let mut framed = (300u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&[9, 9, 9, 9]);
        framed.extend_from_slice(&p2);
        let w = parse_wwise_opus(&fake_wem(&[&p1, &framed])).unwrap();
        assert_eq!((w.channels, w.packets.len(), w.packets[1].len()), (2, 2, 300));
        assert_eq!(opus_packet_samples(&p1), 960);
        let ogg = to_ogg_opus(&w, 0.0);
        assert_eq!(&ogg[..4], b"OggS");
        assert_eq!(ogg.windows(8).filter(|w| *w == b"OpusHead").count(), 1);
        // Trois pages, la dernière marquée fin de flux, granule = 1920.
        let pages: Vec<usize> = ogg.windows(4).enumerate().filter(|(_, w)| *w == b"OggS").map(|(i, _)| i).collect();
        assert_eq!(pages.len(), 3);
        let last = pages[2];
        assert_eq!(ogg[last + 5], 0x04);
        assert_eq!(u64::from_le_bytes(ogg[last + 6..last + 14].try_into().unwrap()), 1920);
        // Le paquet de 300 octets est découpé en segments 255 + 45.
        assert_eq!(&ogg[last + 27..last + 30], &[4, 255, 45]);
    }
}

/// Conversion d'un son extrait : `KALEIDO_WEM=<fichier.wem> cargo test -p kaleido-core real_wem -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn real_wem() {
    let path = std::env::var("KALEIDO_WEM").unwrap();
    let w = parse_wwise_opus(&std::fs::read(&path).unwrap()).unwrap();
    println!("{} canaux, {} Hz, {:.1} s, {} paquets, TOC {:#04x}", w.channels, w.sample_rate, w.seconds(), w.packets.len(), w.packets[0][0]);
    std::fs::write(format!("{path}.ogg"), to_ogg_opus(&w, 0.0)).unwrap();
}

// ---------------------------------------------------------------------------
// Thème de l'écran titre

/// Évènement Wwise du thème de chaque jeu (title ID du jeu de base).
pub fn title_event(title_id: u64) -> Option<&'static str> {
    Some(match title_id {
        // L'écran titre normal ne joue que des ambiances ; ce thème accompagne l'écran
        // titre une fois l'histoire terminée (séquence demo/sequence/sd9010_title.bseq).
        0x01001F5010DFA000 => "PLAY_BGM_HA_SYS_TITLE_CLEAR",
        _ => return None,
    })
}

/// Thème de l'écran titre d'un jeu Switch : fichier audio (Ogg Opus pour les jeux Wwise,
/// WAV sinon), au plus `max_seconds`.
///
/// - Jeux Wwise connus (`title_event`) : parmi les sons de l'évènement du thème (couches,
///   variantes), le plus long est le morceau complet.
/// - Autres jeux : flux audio dont le nom contient « title » (portages qui gardent leurs
///   formats d'origine, comme les flux AST de Super Mario Galaxy 1 et 2).
pub fn title_theme(game: &std::path::Path, keys: &crate::nx::Keys, title_id: u64, max_seconds: f32) -> Result<Vec<u8>, String> {
    let mut nca = crate::nx::open_program(game, keys).map_err(|e| e.to_string())?;
    let mut romfs = nca.romfs().map_err(|e| e.to_string())?;
    let files = romfs.list();
    match title_event(title_id) {
        Some(event) => wwise_theme(&mut romfs, &files, wwise_hash(event), max_seconds),
        None => named_theme(&mut romfs, &files, max_seconds),
    }
}

fn wwise_theme(romfs: &mut crate::nx::RomFs<'_>, files: &[crate::nx::RomFile], event: u32, max_seconds: f32) -> Result<Vec<u8>, String> {
    let in_sound = |f: &&crate::nx::RomFile, ext: &str| f.path.contains("/sound/") && f.path.ends_with(ext);

    // Banque qui contient l'évènement.
    let mut objects = None;
    for f in files.iter().filter(|f| in_sound(f, ".bnk")) {
        let bank = romfs.read_all(f).map_err(|e| e.to_string())?;
        let objs = bank_objects(&bank);
        if objs.contains_key(&event) {
            objects = Some(objs);
            break;
        }
    }
    let objects = objects.ok_or("évènement du thème introuvable dans les banques de sons")?;

    // Sons de l'évènement, cherchés dans chaque archive .pck.
    let mut best: Option<(crate::nx::RomFile, u64, u64)> = None;
    for f in files.iter().filter(|f| in_sound(f, ".pck")) {
        let first = romfs.read(f, 0, 16).map_err(|e| e.to_string())?;
        let Some(size) = pck_header_size(&first) else { continue };
        let header = romfs.read(f, 0, size).map_err(|e| e.to_string())?;
        let streams = pck_streams(&header);
        for id in event_sources(&objects, event, &streams) {
            let (offset, len) = streams[&id];
            if best.as_ref().is_none_or(|b| len > b.2) {
                best = Some((f.clone(), offset, len));
            }
        }
    }
    let (file, offset, len) = best.ok_or("sons du thème introuvables")?;
    let wem = romfs.read(&file, offset, len as usize).map_err(|e| e.to_string())?;
    Ok(to_ogg_opus(&parse_wwise_opus(&wem)?, max_seconds))
}

/// Flux « titre » d'un jeu qui n'utilise pas Wwise (le plus gros si plusieurs).
pub fn title_stream_candidates(files: &[crate::nx::RomFile]) -> Vec<&crate::nx::RomFile> {
    let mut list: Vec<&crate::nx::RomFile> = files
        .iter()
        .filter(|f| {
            let lower = f.path.to_lowercase();
            let name = lower.rsplit('/').next().unwrap_or("");
            name.contains("title") && lower.ends_with(".ast")
        })
        .collect();
    list.sort_by(|a, b| b.size.cmp(&a.size));
    list
}

fn named_theme(romfs: &mut crate::nx::RomFs<'_>, files: &[crate::nx::RomFile], max_seconds: f32) -> Result<Vec<u8>, String> {
    let file = title_stream_candidates(files).first().copied().cloned().ok_or("pas de musique d'écran titre reconnue dans ce jeu")?;
    let data = romfs.read_all(&file).map_err(|e| e.to_string())?;
    let mut pcm = parse_ast(&data)?;
    if max_seconds > 0.0 {
        let frames = (max_seconds * pcm.sample_rate as f32) as usize;
        pcm.samples.truncate(frames * 2);
        pcm.fade_out(1.5);
    }
    Ok(pcm.to_wav())
}

// ---------------------------------------------------------------------------
// AST (flux Nintendo de la GameCube / Wii, repris tels quels par certains portages)

/// Flux AST en PCM 16 bits. En-tête `STRM` (gros-boutiste, Wii) ou `MRTS` (en-tête
/// petit-boutiste des portages Switch, échantillons restés gros-boutistes) : format en
/// 0x09 (1 = PCM16), bits en 0x0A, canaux en 0x0C, fréquence en 0x10, nombre
/// d'échantillons en 0x14. Puis des blocs `BLCK` (0x20 octets d'en-tête, taille par
/// canal en +4) contenant les canaux l'un après l'autre.
pub fn parse_ast(data: &[u8]) -> Result<super::Pcm, String> {
    let magic = data.get(..4).ok_or("flux AST vide")?;
    let le = match magic {
        b"STRM" => false,
        b"MRTS" => true,
        _ => return Err("ce n'est pas un flux AST".into()),
    };
    let rd16 = |o: usize| data.get(o..o + 2).map(|b| if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) });
    let rd32 = |o: usize| data.get(o..o + 4).map(|b| if le { u32::from_le_bytes(b.try_into().unwrap()) } else { u32::from_be_bytes(b.try_into().unwrap()) });
    // Format sur 16 bits en 0x08 : seul l'octet 0x09 compte (0x0100 en petit-boutiste, 1 en gros).
    let format = *data.get(9).ok_or("en-tête AST incomplet")?;
    let bits = rd16(0x0A).ok_or("en-tête AST incomplet")?;
    let channels = rd16(0x0C).ok_or("en-tête AST incomplet")? as usize;
    let sample_rate = rd32(0x10).ok_or("en-tête AST incomplet")?;
    let total = rd32(0x14).ok_or("en-tête AST incomplet")? as usize;
    if format != 1 || bits != 16 {
        return Err("flux AST compressé (AFC) non pris en charge".into());
    }
    if channels == 0 || channels > 8 || sample_rate == 0 {
        return Err("en-tête AST invalide".into());
    }
    let mut chans: Vec<Vec<i16>> = vec![Vec::with_capacity(total); channels];
    let mut o = 0x40;
    while o + 0x20 <= data.len() {
        let tag = &data[o..o + 4];
        if tag != b"BLCK" && tag != b"KCLB" {
            break;
        }
        let size = rd32(o + 4).unwrap_or(0) as usize;
        let start = o + 0x20;
        for (c, out) in chans.iter_mut().enumerate() {
            let block = data.get(start + c * size..start + (c + 1) * size).ok_or("bloc AST tronqué")?;
            // Échantillons gros-boutistes dans les deux variantes.
            out.extend(block.chunks_exact(2).map(|b| i16::from_be_bytes([b[0], b[1]])));
        }
        o = start + channels * size;
    }
    let frames = chans.iter().map(Vec::len).min().unwrap_or(0).min(if total > 0 { total } else { usize::MAX });
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let l = chans[0][i];
        let r = chans.get(1).map_or(l, |c| c[i]);
        samples.push(l);
        samples.push(r);
    }
    Ok(super::Pcm { sample_rate, samples })
}

/// Thème d'un vrai jeu : `KALEIDO_NX_GAME=<xci|nsp> KALEIDO_NX_KEYS=<prod.keys> KALEIDO_NX_OGG=<sortie.ogg> cargo test -p kaleido-core real_title_theme -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn real_title_theme() {
    let game = std::path::PathBuf::from(std::env::var("KALEIDO_NX_GAME").unwrap());
    let keys = crate::nx::Keys::load(std::path::Path::new(&std::env::var("KALEIDO_NX_KEYS").unwrap())).unwrap();
    let t = std::time::Instant::now();
    let tid = u64::from_str_radix(&std::env::var("KALEIDO_NX_TID").unwrap_or("01001F5010DFA000".into()), 16).unwrap();
    let ogg = title_theme(&game, &keys, tid, 50.0).unwrap();
    println!("{} Ko en {:?}", ogg.len() / 1024, t.elapsed());
    std::fs::write(std::env::var("KALEIDO_NX_OGG").unwrap(), ogg).unwrap();
}

#[cfg(test)]
mod ast_tests {
    use super::*;

    fn ast(le: bool, left: &[i16], right: &[i16]) -> Vec<u8> {
        let mut h = vec![0u8; 0x40];
        let w16 = |v: u16| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
        h[..4].copy_from_slice(if le { b"MRTS" } else { b"STRM" });
        // Comme les vrais fichiers : 00 01 dans les deux variantes (0x0100 lu en petit-boutiste).
        h[8..10].copy_from_slice(&[0, 1]);
        h[0x0A..0x0C].copy_from_slice(&w16(16));
        h[0x0C..0x0E].copy_from_slice(&w16(2));
        h[0x10..0x14].copy_from_slice(&w32(32000));
        h[0x14..0x18].copy_from_slice(&w32(left.len() as u32));
        let mut out = h;
        let mut block = vec![0u8; 0x20];
        block[..4].copy_from_slice(if le { b"KCLB" } else { b"BLCK" });
        block[4..8].copy_from_slice(&w32((left.len() * 2) as u32));
        out.extend(block);
        for s in left.iter().chain(right) {
            out.extend_from_slice(&s.to_be_bytes());
        }
        out
    }

    #[test]
    fn both_variants() {
        for le in [false, true] {
            let pcm = parse_ast(&ast(le, &[1, -2, 3], &[10, 20, 30])).unwrap();
            assert_eq!(pcm.sample_rate, 32000);
            assert_eq!(pcm.samples, vec![1, 10, -2, 20, 3, 30]);
        }
        assert!(parse_ast(b"RIFF....").is_err());
    }

    #[test]
    fn title_candidates() {
        let f = |p: &str, size| crate::nx::RomFile { path: p.into(), offset: 0, size };
        let files = vec![f("/AudioRes/Stream/SMG_title_strm.ast", 5), f("/LayoutData/TitleLogo.arc", 9), f("/AudioRes/Stream/SMG_boss01a_strm.ast", 7)];
        let c = title_stream_candidates(&files);
        assert_eq!(c.len(), 1);
        assert!(c[0].path.ends_with("SMG_title_strm.ast"));
    }
}
