//! Pokédex des sauvegardes Gen 4 à 7 : vu / capturé au niveau de l'espèce.
//!
//! Portage de `Saves/Substructures/PokeDex/Zukan4.cs`, `Zukan5.cs`, `Zukan6.cs` et
//! `Zukan7.cs` de PKHeX (kwsch/PKHeX, GPLv3). Pour que le Pokédex reste cohérent,
//! marquer une espèce comme vue ou capturée pose aussi, comme les fonctions
//! « tout voir / tout capturer » de PKHeX :
//!
//! - les drapeaux de sexe vus (les deux sexes pour une espèce mixte, sinon le sexe
//!   imposé ; asexué = mâle) et le drapeau « affiché » s'il n'y en a aucun ;
//! - la forme par défaut (forme 0) pour les espèces à formes de la Gen 4 à 6
//!   (en Gen 7, les formes partagent les drapeaux d'espèce : seule la forme 0 est posée) ;
//! - pour une capture, le drapeau de langue de la partie (langue du dresseur) ;
//! - X/Y : le drapeau « obtenu dans un autre jeu » des espèces 1 à 649 ;
//!   ROSA : le compteur de rencontres à 1.
//!
//! Effacer une espèce retire tous ses drapeaux (vu, capturé, sexes, affiché, formes,
//! langues). Le motif de Spinda et les entrées chromatiques ne sont pas touchés.
//!
//! | | Données | Capturé | Vu (×4) | Affiché (×4) | Formes | Langues |
//! |---|---|---|---|---|---|---|
//! | Gen 4 | général + 0x12DC / 0x1328 / 0x12B8 | +0x4 | +0x44 (+ sexes 0x84, 0xC4) | — | +0x108… | +0x128 / +0x144 |
//! | Gen 5 | bloc 0x21600 / 0x21400 | +0x8 | +0x5C | +0x1AC | +0x2FC | +0x320 / +0x328 |
//! | Gen 6 | bloc 0x15000 | +0x8 | +0x68 | +0x1E8 | +0x368 | +0x3C8 / +0x400 |
//! | Gen 7 | bloc 0x2A00 / 0x2C00 | +0x88 | +0xF0 | +0x320 | (dans « vu ») | +0x550 |

use serde::Serialize;

use super::{rd_u8, SaveError, SaveFile, SaveVersion};

/// État d'une espèce dans le Pokédex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexEntry {
    pub species: u16,
    pub seen: bool,
    pub caught: bool,
}

/// Répartition des sexes de chaque espèce (1 à 807) : `D` mixte, `M` mâle seulement,
/// `F` femelle seulement, `N` asexué. Extrait de `personal_uu` (USUL) de PKHeX ; les
/// répartitions n'ont pas changé entre la Gen 4 et la Gen 7.
const GENDERS: &str = concat!(
    "DDDDDDDDDDDDDDDDDDDDDDDDDDDDFFFMMMDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDNNDDDDDDDDDDDDDDDDDN",
    "NDDDDMMDDDDDFDFDDDDNNDDFDDDMDDDNDDDDNDDDDDDNNNDDDNNDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
    "NDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDNDDMMFDDFFNNNDDDNNNDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDNDDDDDDDD",
    "DDDDDDDDDDDDMFDDDDDDDDDDDDDDDDDDDDDDNNDDDDNNDDDDDDDDDDDDDDDDDDDDDDDDDDDDDNNNNNNFMNNNNNDDDDDDDDDDDDDD",
    "DDDDDDDDDDDDFMDFDDDDDDDDDDDDDDDDDDDNNDDFDDDDDDDDDDDDDDDDDDDDDNDDDDDDDDDDDNMDDFNNNNNNDNNFNNNNNNDDDDDD",
    "DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDMMDDDDDDDDFFDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDNN",
    "NDDDDDDDDDDDDDNDDDDDDNNDDDMMFFDDDDDDDNNNMMNNMNNNNDDDDDDDDDDDDDDDDDDDFFFDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
    "DDNDDDDDDDDDDDDNNNNNNDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDFDDFFFDDDDDDDDNNNDDDDDDNDDDNNNNNNNNNNNNNNNN",
    "NNNNNNN",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenderKind {
    Dual,
    Male,
    Female,
    Genderless,
}

fn gender_kind(species: u16) -> GenderKind {
    match GENDERS.as_bytes().get(species as usize - 1) {
        Some(b'M') => GenderKind::Male,
        Some(b'F') => GenderKind::Female,
        Some(b'N') => GenderKind::Genderless,
        _ => GenderKind::Dual,
    }
}

fn only_female(species: u16) -> bool {
    gender_kind(species) == GenderKind::Female
}

/// Sexe « vu » d'une espèce à sexe imposé (`FixedGender() & 1` de PKHeX) : 1 pour femelle.
fn fixed_gender_bit(species: u16) -> bool {
    only_female(species)
}

// --- Drapeaux (bit `i` de l'octet `ofs + i/8`, comme `FlagUtil` de PKHeX).

fn get_flag(d: &[u8], ofs: usize, bit: usize) -> bool {
    d.get(ofs + (bit >> 3)).is_some_and(|b| b >> (bit & 7) & 1 != 0)
}

fn set_flag(d: &mut [u8], ofs: usize, bit: usize, value: bool) {
    if let Some(b) = d.get_mut(ofs + (bit >> 3)) {
        let mask = 1 << (bit & 7);
        *b = if value { *b | mask } else { *b & !mask };
    }
}

/// Variante du Pokédex, avec la taille des données.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Gen 3 : `mirror` = second et troisième exemplaires des drapeaux « vu », relatifs au Pokédex.
    Gen3 { mirror: [usize; 2] },
    Gen4 { dp: bool, hgss: bool },
    Gen5 { b2w2: bool },
    Gen6 { ao: bool },
    Gen7 { usum: bool },
}

impl Kind {
    fn of(version: SaveVersion) -> Self {
        // Vérifié : PKHeX SAV3 (PokeDex = petit bloc + 0x18 ; SeenOffset2 / SeenOffset3 du grand bloc).
        let g3 = |a: usize, b: usize| Kind::Gen3 { mirror: [super::gen3::LARGE + a - 0x18, super::gen3::LARGE + b - 0x18] };
        match version {
            SaveVersion::RubySapphire => g3(0x938, 0x3A8C),
            SaveVersion::Emerald => g3(0x988, 0x3B24),
            SaveVersion::FireRedLeafGreen => g3(0x5F8, 0x3A18),
            SaveVersion::DiamondPearl => Kind::Gen4 { dp: true, hgss: false },
            SaveVersion::Platinum => Kind::Gen4 { dp: false, hgss: false },
            SaveVersion::HeartGoldSoulSilver => Kind::Gen4 { dp: false, hgss: true },
            SaveVersion::BlackWhite => Kind::Gen5 { b2w2: false },
            SaveVersion::Black2White2 => Kind::Gen5 { b2w2: true },
            SaveVersion::XY => Kind::Gen6 { ao: false },
            SaveVersion::OmegaRubyAlphaSapphire => Kind::Gen6 { ao: true },
            SaveVersion::SunMoon => Kind::Gen7 { usum: false },
            SaveVersion::UltraSunUltraMoon => Kind::Gen7 { usum: true },
        }
    }

    // Vérifié : PKHeX Legal.MaxSpeciesID_4/5/6, SAV7SM / SAV7USUM (MaxSpeciesID 802 / 807).
    fn max_species(self) -> u16 {
        match self {
            Kind::Gen3 { .. } => 386,
            Kind::Gen4 { .. } => 493,
            Kind::Gen5 { .. } => 649,
            Kind::Gen6 { .. } => 721,
            Kind::Gen7 { usum: false } => 802,
            Kind::Gen7 { usum: true } => 807,
        }
    }

    /// Octets lus ou écrits à partir du début du Pokédex.
    // Vérifié : PKHeX SaveBlockAccessor5BW/B2W2 (0x4D4 / 0x4DC), 6XY/6AO (0x6A0 / 0x11CC),
    // 7SM/7USUM (0xF78) ; Gen 4 : dernier octet utilisé par Zukan4 (formes HGSS de Pichu).
    fn size(self) -> usize {
        match self {
            Kind::Gen3 { mirror } => mirror[1] + gen3::FLAGS,
            Kind::Gen4 { hgss, .. } => gen4::form2(hgss) + 7,
            Kind::Gen5 { b2w2: false } => 0x4D4,
            Kind::Gen5 { b2w2: true } => 0x4DC,
            Kind::Gen6 { ao: false } => 0x6A0,
            Kind::Gen6 { ao: true } => 0x11CC,
            Kind::Gen7 { .. } => 0xF78,
        }
    }
}

impl SaveFile {
    /// Dernier numéro national du Pokédex de ce jeu (493, 649, 721, 802 ou 807).
    pub fn dex_max_species(&self) -> u16 {
        Kind::of(self.version).max_species()
    }

    fn dex_data(&self) -> Result<&[u8], SaveError> {
        let size = Kind::of(self.version).size();
        self.data.get(self.layout.dex..self.layout.dex + size).ok_or(SaveError::Truncated("Pokédex"))
    }

    /// Langue de la partie (`LanguageID` de PKHeX), pour les drapeaux de langue.
    fn dex_language(&self) -> u8 {
        rd_u8(&self.data, self.layout.trainer.language)
    }

    /// Toutes les espèces, de 1 à [`Self::dex_max_species`].
    pub fn pokedex(&self) -> Result<Vec<DexEntry>, SaveError> {
        let kind = Kind::of(self.version);
        let d = self.dex_data()?;
        Ok((1..=kind.max_species())
            .map(|species| DexEntry { species, seen: get_seen(kind, d, species), caught: get_caught(kind, d, species) })
            .collect())
    }

    /// Marque une espèce. `caught` implique `seen` ; `seen = false` efface l'espèce.
    pub fn set_dex_entry(&mut self, species: u16, seen: bool, caught: bool) -> Result<(), SaveError> {
        let kind = Kind::of(self.version);
        if species == 0 || species > kind.max_species() {
            return Err(SaveError::Invalid(format!("espèce n° {species} absente du Pokédex de ce jeu")));
        }
        self.dex_apply(kind, &[species], seen, caught)
    }

    /// Applique le même état à toutes les espèces du Pokédex.
    pub fn dex_set_all(&mut self, seen: bool, caught: bool) -> Result<(), SaveError> {
        let kind = Kind::of(self.version);
        let all: Vec<u16> = (1..=kind.max_species()).collect();
        self.dex_apply(kind, &all, seen, caught)
    }

    fn dex_apply(&mut self, kind: Kind, species: &[u16], seen: bool, caught: bool) -> Result<(), SaveError> {
        let lang = self.dex_language();
        let (start, size) = (self.layout.dex, kind.size());
        let d = self.data.get_mut(start..start + size).ok_or(SaveError::Truncated("Pokédex"))?;
        for &s in species {
            set_entry(kind, d, s, seen || caught, caught, lang);
        }
        Ok(())
    }
}

fn get_seen(kind: Kind, d: &[u8], species: u16) -> bool {
    match kind {
        Kind::Gen3 { .. } => gen3::flag(d, gen3::SEEN, species),
        Kind::Gen4 { .. } => gen4::get_seen(d, species),
        Kind::Gen5 { .. } => gen56::get_seen(&gen56::Layout::gen5(kind), d, species),
        Kind::Gen6 { .. } => gen56::get_seen(&gen56::Layout::gen6(kind), d, species),
        Kind::Gen7 { .. } => gen7::get_seen(d, species),
    }
}

fn get_caught(kind: Kind, d: &[u8], species: u16) -> bool {
    match kind {
        Kind::Gen3 { .. } => gen3::flag(d, gen3::CAUGHT, species),
        Kind::Gen4 { .. } => gen4::get_caught(d, species),
        Kind::Gen5 { .. } => gen56::get_caught(d, species),
        Kind::Gen6 { .. } => gen56::get_caught(d, species),
        Kind::Gen7 { .. } => gen7::get_caught(d, species),
    }
}

fn set_entry(kind: Kind, d: &mut [u8], species: u16, seen: bool, caught: bool, lang: u8) {
    match kind {
        Kind::Gen3 { mirror } => {
            gen3::set_flag(d, gen3::CAUGHT, species, caught);
            for at in [gen3::SEEN, mirror[0], mirror[1]] {
                gen3::set_flag(d, at, species, seen);
            }
            let _ = lang;
        }
        Kind::Gen4 { dp, hgss } => gen4::set_entry(d, dp, hgss, species, seen, caught, lang),
        Kind::Gen5 { .. } => gen56::set_entry(&gen56::Layout::gen5(kind), d, species, seen, caught, lang),
        Kind::Gen6 { .. } => gen56::set_entry(&gen56::Layout::gen6(kind), d, species, seen, caught, lang),
        Kind::Gen7 { .. } => gen7::set_entry(d, species, seen, caught, lang),
    }
}

/// Gen 3 (`SAV3` de PKHeX) : drapeaux « capturé » en +0x10 et « vu » en +0x44 du Pokédex
/// (un bit par espèce), « vu » recopié deux fois dans le grand bloc.
mod gen3 {
    pub(super) const CAUGHT: usize = 0x10;
    pub(super) const SEEN: usize = 0x44;
    /// 386 bits, arrondis à l'octet.
    pub(super) const FLAGS: usize = 0x31;

    pub(super) fn flag(d: &[u8], at: usize, species: u16) -> bool {
        let bit = species as usize - 1;
        d.get(at + bit / 8).is_some_and(|b| b >> (bit % 8) & 1 != 0)
    }

    pub(super) fn set_flag(d: &mut [u8], at: usize, species: u16, on: bool) {
        let bit = species as usize - 1;
        if let Some(b) = d.get_mut(at + bit / 8) {
            *b = (*b & !(1 << (bit % 8))) | (on as u8) << (bit % 8);
        }
    }
}

/// Gen 4 (`Zukan4` de PKHeX) : `u32` magique, quatre régions de 0x40 octets (capturé, vu,
/// premier sexe vu, second sexe vu), motif de Spinda, formes, langues, autres formes.
mod gen4 {
    use super::{fixed_gender_bit, gender_kind, get_flag, set_flag, GenderKind};

    const SIZE_REGION: usize = 0x40;
    const FORM1: usize = 4 + 4 * SIZE_REGION + 4; // 0x108
    const FORM_NONE: u8 = 0xFF;
    const UNOWN: usize = FORM1 + 4;
    const UNOWN_LEN: usize = 0x1C;

    // Vérifié : PKHeX Zukan4.cs (PokeDexLanguageFlags = OFS_FORM1 + 0x3C en HGSS, + 0x20 sinon).
    pub(super) fn languages(hgss: bool) -> usize {
        FORM1 + if hgss { 0x3C } else { 0x20 }
    }

    // Vérifié : PKHeX Zukan4.cs (FormOffset2 = PokeDexLanguageFlags + 0x1F4).
    pub(super) fn form2(hgss: bool) -> usize {
        languages(hgss) + 0x1F4
    }

    fn region(d: &[u8], region: usize, species: u16) -> bool {
        get_flag(d, 4 + region * SIZE_REGION, species as usize - 1)
    }

    fn set_region(d: &mut [u8], region: usize, species: u16, value: bool) {
        set_flag(d, 4 + region * SIZE_REGION, species as usize - 1, value);
    }

    pub(super) fn get_caught(d: &[u8], species: u16) -> bool {
        region(d, 0, species)
    }

    pub(super) fn get_seen(d: &[u8], species: u16) -> bool {
        region(d, 1, species)
    }

    /// Valeurs des formes vues (`GetDexFormValues`) : `FORM_NONE` = case libre.
    fn decode_forms(value: u32, bits: usize, count: usize) -> Vec<u8> {
        let mask = (0xFFu32 >> (8 - bits)) as u8;
        let mut forms: Vec<u8> = (0..count)
            .map(|i| {
                let v = (value >> (i * bits)) as u8 & mask;
                if v == mask && bits > 1 {
                    FORM_NONE
                } else {
                    v
                }
            })
            .collect();
        if bits == 1 && forms[0] == forms[1] && forms[0] == 1 {
            forms[0] = FORM_NONE;
            forms[1] = FORM_NONE;
        }
        forms
    }

    /// Encodage inverse (`SetDexFormValues`) : les cases au-delà de la liste restent à 1.
    fn encode_forms(forms: &[u8], bits: usize, count: usize) -> u32 {
        let mask = 0xFFu32 >> (8 - bits);
        let mut value = 0xFFFF_FFFFu32.checked_shl((count * bits) as u32).unwrap_or(0);
        for (i, &f) in forms.iter().enumerate().take(count) {
            let v = if f == FORM_NONE { mask } else { f as u32 & mask };
            value |= v << (bits * i);
        }
        value
    }

    /// Emplacement des formes d'une espèce : (offset, bits par forme, nombre de cases).
    // Vérifié : PKHeX Zukan4.cs (GetForms / SetForms : Shellos, Gastrodon, Burmy, Wormadam
    // dans FormOffset1 ; Rotom, Shaymin, Giratina, Pichu HGSS dans FormOffset2 hors DP ;
    // Deoxys dans les derniers octets des régions 0 et 1).
    fn form_slot(dp: bool, hgss: bool, species: u16) -> Option<(usize, usize, usize)> {
        let f2 = form2(hgss);
        match species {
            422 => Some((FORM1, 1, 2)),
            423 => Some((FORM1 + 1, 1, 2)),
            412 => Some((FORM1 + 2, 2, 3)),
            413 => Some((FORM1 + 3, 2, 3)),
            479 if !dp => Some((f2, 3, 6)),
            492 if !dp => Some((f2 + 4, 1, 2)),
            487 if !dp => Some((f2 + 5, 1, 2)),
            172 if hgss => Some((f2 + 6, 2, 3)),
            _ => None,
        }
    }

    fn read_value(d: &[u8], at: usize, bits: usize, count: usize) -> u32 {
        let bytes = (bits * count).div_ceil(8);
        (0..bytes).fold(0u32, |acc, i| acc | (d.get(at + i).copied().unwrap_or(0) as u32) << (8 * i))
    }

    fn write_value(d: &mut [u8], at: usize, bits: usize, count: usize, value: u32) {
        let bytes = (bits * count).div_ceil(8);
        for i in 0..bytes {
            if let Some(b) = d.get_mut(at + i) {
                *b = (value >> (8 * i)) as u8;
            }
        }
    }

    // Deoxys : 4 formes de 4 bits, réparties sur le dernier octet des régions 0 et 1.
    const DEOXYS_LO: usize = 4 + SIZE_REGION - 1;
    const DEOXYS_HI: usize = 4 + 2 * SIZE_REGION - 1;

    fn get_forms(d: &[u8], dp: bool, hgss: bool, species: u16) -> Option<Vec<u8>> {
        if species == 386 {
            let v = d[DEOXYS_LO] as u32 | (d[DEOXYS_HI] as u32) << 8;
            return Some(decode_forms(v, 4, 4));
        }
        let (at, bits, count) = form_slot(dp, hgss, species)?;
        Some(decode_forms(read_value(d, at, bits, count), bits, count))
    }

    fn set_forms(d: &mut [u8], dp: bool, hgss: bool, species: u16, forms: &[u8]) {
        match species {
            386 => {
                let v = encode_forms(forms, 4, 4);
                d[DEOXYS_LO] = v as u8;
                d[DEOXYS_HI] = (v >> 8) as u8;
            }
            201 => {
                let unown = &mut d[UNOWN..UNOWN + UNOWN_LEN];
                unown.fill(FORM_NONE);
                unown[..forms.len().min(UNOWN_LEN)].copy_from_slice(&forms[..forms.len().min(UNOWN_LEN)]);
            }
            _ => {
                if let Some((at, bits, count)) = form_slot(dp, hgss, species) {
                    write_value(d, at, bits, count, encode_forms(forms, bits, count));
                }
            }
        }
    }

    /// Ajoute la forme par défaut (`SetForms(species, form, gender)` de PKHeX, forme 0).
    fn add_default_form(d: &mut [u8], dp: bool, hgss: bool, species: u16, gender: u8) {
        if species == 201 {
            // AddUnownForm(0) : première case libre, sauf si déjà présente.
            let unown = &mut d[UNOWN..UNOWN + UNOWN_LEN];
            if let Some(i) = unown.iter().position(|&v| v == 0 || v == FORM_NONE) {
                unown[i] = 0;
            }
            return;
        }
        let Some(mut forms) = get_forms(d, dp, hgss, species) else { return };
        // Pichu HGSS : les « formes » 0 et 1 sont les sexes, 2 = Pichu Troizépi.
        let form = if species == 172 && hgss { gender } else { 0 };
        if forms.contains(&form) {
            return;
        }
        if let Some(i) = forms.iter().position(|&v| v == FORM_NONE) {
            forms[i] = form;
            set_forms(d, dp, hgss, species, &forms);
        }
    }

    // Vérifié : PKHeX Zukan4.cs (DPLangSpecies : seules ces espèces ont des drapeaux de
    // langue en DP, à l'index 1 + position dans la liste).
    const DP_LANG_SPECIES: [u16; 14] = [23, 25, 54, 77, 120, 129, 202, 214, 215, 216, 228, 278, 287, 315];

    fn language_byte(dp: bool, hgss: bool, species: u16) -> Option<usize> {
        let base = languages(hgss);
        if dp {
            DP_LANG_SPECIES.iter().position(|&s| s == species).map(|i| base + 1 + i)
        } else {
            Some(base + species as usize)
        }
    }

    /// Bit de langue Gen 4 (`GetGen4LanguageBitIndex`) : japonais, anglais, français,
    /// allemand, italien, espagnol.
    ///
    /// Écart volontaire : PKHeX envoie l'espagnol (7) sur le bit du japonais ; on le
    /// range sur le bit 5, le seul qui reste (PKHeX pose bien six bits pour « toutes
    /// les langues »).
    fn language_bit(lang: u8) -> usize {
        match lang {
            0 => 1,
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 4, // italien et allemand sont inversés
            5 => 3,
            7 => 5,
            _ => 0,
        }
    }

    pub(super) fn set_entry(d: &mut [u8], dp: bool, hgss: bool, species: u16, seen: bool, caught: bool, lang: u8) {
        let lang_at = language_byte(dp, hgss, species);
        if !seen {
            // ClearSeen : capturé, vu, sexes, formes et langues.
            for r in 0..4 {
                set_region(d, r, species, false);
            }
            // Écart volontaire : PKHeX appelle SetForms(species, []), qui met à 0 les bits
            // des cases (soit « forme 0 vue » partout) ; on y met des 1, la valeur « case
            // libre » que relit GetDexFormValues.
            set_forms(d, dp, hgss, species, &[FORM_NONE; UNOWN_LEN]);
            if let Some(at) = lang_at {
                d[at] = 0;
            }
            return;
        }
        if !get_seen(d, species) {
            // CompleteSeen : les deux sexes pour une espèce mixte (premier mâle, second
            // femelle), sinon le sexe imposé dans les deux régions.
            if gender_kind(species) == GenderKind::Dual {
                set_region(d, 2, species, false);
                set_region(d, 3, species, true);
            } else {
                let g = fixed_gender_bit(species);
                set_region(d, 2, species, g);
                set_region(d, 3, species, g);
            }
            set_region(d, 1, species, true);
            add_default_form(d, dp, hgss, species, fixed_gender_bit(species) as u8);
        }
        set_region(d, 0, species, caught);
        if let Some(at) = lang_at {
            if caught {
                d[at] |= 1 << language_bit(lang);
            } else {
                d[at] = 0;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn forms_of(d: &[u8], dp: bool, hgss: bool, species: u16) -> Option<Vec<u8>> {
        if species == 201 {
            return Some(d[UNOWN..UNOWN + UNOWN_LEN].to_vec());
        }
        get_forms(d, dp, hgss, species)
    }
}

/// Gen 5 et 6 (`Zukan5` et `Zukan6` de PKHeX) : `u32` magique, `u32` d'options, puis
/// neuf régions de bits (capturé ; vus mâle, femelle, mâle chromatique, femelle
/// chromatique ; affichés idem), quatre régions de formes, les langues.
mod gen56 {
    use super::{gender_kind, get_flag, only_female, set_flag, GenderKind, Kind};

    const OFS_CAUGHT: usize = 0x8;
    const LANG_COUNT: usize = 7;

    pub(super) struct Layout {
        region: usize,
        form_len: usize,
        max_species_language: u16,
        forms: fn(u16) -> (u16, u8),
        /// X/Y : drapeaux « obtenu dans un autre jeu » (espèces 1 à 649).
        foreign: Option<usize>,
        /// ROSA : compteurs de rencontres (u16 par espèce).
        seen_counts: Option<(usize, usize)>,
    }

    impl Layout {
        // Vérifié : PKHeX Zukan5.cs (BitSeenSize 0x54, FormLen 9 / 0xB, langues pour
        // les espèces ≤ 493 seulement).
        pub(super) fn gen5(kind: Kind) -> Self {
            let b2w2 = kind == Kind::Gen5 { b2w2: true };
            Layout {
                region: 0x54,
                form_len: if b2w2 { 0xB } else { 9 },
                max_species_language: 493,
                forms: if b2w2 { form_index_b2w2 } else { form_index_bw },
                foreign: None,
                seen_counts: None,
            }
        }

        // Vérifié : PKHeX Zukan6.cs (BitSeenSize 0x60, FormLen 0x18 / 0x26, Zukan6XY :
        // drapeaux étrangers en 0x64C ; Zukan6AO : compteurs vus en 0x684, obtenus en 0xC28).
        pub(super) fn gen6(kind: Kind) -> Self {
            let ao = kind == Kind::Gen6 { ao: true };
            Layout {
                region: 0x60,
                form_len: if ao { 0x26 } else { 0x18 },
                max_species_language: 721,
                forms: if ao { form_index_ao } else { form_index_xy },
                foreign: (!ao).then_some(0x64C),
                seen_counts: ao.then_some((0x684, 0xC28)),
            }
        }

        fn seen(&self) -> usize {
            OFS_CAUGHT + self.region
        }

        fn displayed(&self) -> usize {
            self.seen() + 4 * self.region
        }

        fn form_dex(&self) -> usize {
            self.displayed() + 4 * self.region
        }

        fn languages(&self) -> usize {
            self.form_dex() + 4 * self.form_len
        }
    }

    pub(super) fn get_caught(d: &[u8], species: u16) -> bool {
        get_flag(d, OFS_CAUGHT, species as usize - 1)
    }

    pub(super) fn get_seen(l: &Layout, d: &[u8], species: u16) -> bool {
        (0..4).any(|r| get_flag(d, l.seen() + r * l.region, species as usize - 1))
    }

    fn any_displayed(l: &Layout, d: &[u8], species: u16) -> bool {
        (0..4).any(|r| get_flag(d, l.displayed() + r * l.region, species as usize - 1))
    }

    fn set_seen_region(l: &Layout, d: &mut [u8], species: u16, region: usize, value: bool) {
        set_flag(d, l.seen() + region * l.region, species as usize - 1, value);
        // Zukan6AO.SetSeen : compteur de rencontres à 1 s'il était nul.
        if let (true, Some((seen_at, _))) = (value, l.seen_counts) {
            let at = seen_at + 2 * species as usize;
            if d.get(at..at + 2).is_some_and(|c| c == [0, 0]) {
                d[at] = 1;
            }
        }
    }

    /// Bit de langue (`GetLanguageIndex`) : japonais, anglais, français, italien, allemand,
    /// espagnol ; le coréen n'a pas de case (−1 dans PKHeX).
    fn language_index(lang: u8) -> Option<usize> {
        let mut index = lang as i32 - 1;
        if index > 5 {
            index -= 1;
        }
        (0..=5).contains(&index).then_some(index as usize)
    }

    fn set_languages(l: &Layout, d: &mut [u8], species: u16, value: bool) {
        if species > l.max_species_language {
            return;
        }
        for i in 0..LANG_COUNT {
            set_flag(d, l.languages(), (species as usize - 1) * LANG_COUNT + i, value);
        }
    }

    fn form_flag(l: &Layout, region: usize) -> usize {
        l.form_dex() + region * l.form_len
    }

    pub(super) fn set_entry(l: &Layout, d: &mut [u8], species: u16, seen: bool, caught: bool, lang: u8) {
        let bit = species as usize - 1;
        let (form_index, form_count) = (l.forms)(species);
        if !seen {
            // ClearSeen + CaughtNone, plus les drapeaux « affiché » et de formes.
            for r in 0..4 {
                set_flag(d, l.seen() + r * l.region, bit, false);
                set_flag(d, l.displayed() + r * l.region, bit, false);
                for f in 0..form_count as usize {
                    set_flag(d, form_flag(l, r), form_index as usize + f, false);
                }
            }
            set_flag(d, OFS_CAUGHT, bit, false);
            set_languages(l, d, species, false);
            if let Some(foreign) = l.foreign.filter(|_| species <= 649) {
                set_flag(d, foreign, bit, false);
            }
            if let Some((seen_at, obtained_at)) = l.seen_counts {
                for at in [seen_at + 2 * bit + 2, obtained_at + 2 * bit + 2] {
                    if let Some(c) = d.get_mut(at..at + 2) {
                        c.fill(0);
                    }
                }
            }
            return;
        }
        if !get_seen(l, d, species) {
            // CompleteSeen (sans les chromatiques) : sexes vus, affiché si aucun.
            let kind = gender_kind(species);
            if kind != GenderKind::Female {
                set_seen_region(l, d, species, 0, true);
            }
            if kind == GenderKind::Dual || kind == GenderKind::Female {
                set_seen_region(l, d, species, 1, true);
            }
            if !any_displayed(l, d, species) {
                set_flag(d, l.displayed() + if only_female(species) { l.region } else { 0 }, bit, true);
            }
            // CompleteForms(firstFormOnly) : forme 0 vue, affichée si aucune forme ne l'est.
            if form_count > 0 {
                let first = form_index as usize;
                set_flag(d, form_flag(l, 0), first, true);
                let shown = (0..form_count as usize).any(|f| get_flag(d, form_flag(l, 2), first + f) || get_flag(d, form_flag(l, 3), first + f));
                if !shown {
                    set_flag(d, form_flag(l, 2), first, true);
                }
            }
        }
        set_flag(d, OFS_CAUGHT, bit, caught);
        if caught {
            if species <= l.max_species_language {
                if let Some(i) = language_index(lang) {
                    set_flag(d, l.languages(), bit * LANG_COUNT + i, true);
                }
            }
            // Zukan6XY.CompleteObtained : drapeau « obtenu ailleurs » des espèces ≤ 649.
            if let Some(foreign) = l.foreign.filter(|_| species <= 649) {
                set_flag(d, foreign, bit, true);
            }
        } else {
            set_languages(l, d, species, false);
        }
    }

    // Vérifié : PKHeX Zukan5.cs (GetFormIndexBW / GetFormIndexB2W2) et Zukan6.cs
    // (GetFormIndexXY / GetFormIndexAO) : (premier bit, nombre de formes).
    fn form_index_bw(species: u16) -> (u16, u8) {
        match species {
            201 => (0, 28),
            386 => (28, 4),
            492 => (32, 2),
            487 => (34, 2),
            479 => (36, 6),
            422 => (42, 2),
            423 => (44, 2),
            412 => (46, 3),
            413 => (49, 3),
            351 => (52, 4),
            421 => (56, 2),
            585 => (58, 4),
            586 => (62, 4),
            648 => (66, 2),
            555 => (68, 2),
            550 => (70, 2),
            _ => (0, 0),
        }
    }

    fn form_index_b2w2(species: u16) -> (u16, u8) {
        match species {
            646 => (72, 3),
            647 => (75, 2),
            642 => (77, 2),
            641 => (79, 2),
            645 => (81, 2),
            _ => form_index_bw(species),
        }
    }

    fn form_index_xy(species: u16) -> (u16, u8) {
        match species {
            666 => (83, 20),
            669 => (103, 5),
            670 => (108, 6),
            671 => (114, 5),
            710 => (119, 4),
            711 => (123, 4),
            681 => (127, 2),
            716 => (129, 2),
            3 => (131, 2),
            6 => (133, 3),
            9 => (136, 2),
            65 => (138, 2),
            94 => (140, 2),
            115 => (142, 2),
            127 => (144, 2),
            130 => (146, 2),
            142 => (148, 2),
            150 => (150, 3),
            181 => (153, 2),
            212 => (155, 2),
            214 => (157, 2),
            229 => (159, 2),
            248 => (161, 2),
            257 => (163, 2),
            282 => (165, 2),
            303 => (167, 2),
            306 => (169, 2),
            308 => (171, 2),
            310 => (173, 2),
            354 => (175, 2),
            359 => (177, 2),
            380 => (179, 2),
            381 => (181, 2),
            445 => (183, 2),
            448 => (185, 2),
            460 => (187, 2),
            _ => form_index_b2w2(species),
        }
    }

    fn form_index_ao(species: u16) -> (u16, u8) {
        match species {
            25 => (189, 7),
            720 => (196, 2),
            15 => (198, 2),
            18 => (200, 2),
            80 => (202, 2),
            208 => (204, 2),
            254 => (206, 2),
            260 => (208, 2),
            302 => (210, 2),
            319 => (212, 2),
            323 => (214, 2),
            334 => (216, 2),
            362 => (218, 2),
            373 => (220, 2),
            376 => (222, 2),
            384 => (224, 2),
            428 => (226, 2),
            475 => (228, 2),
            531 => (230, 2),
            719 => (232, 2),
            382 => (234, 2),
            383 => (236, 2),
            493 => (238, 18),
            649 => (256, 5),
            676 => (261, 1),
            _ => form_index_xy(species),
        }
    }

    #[cfg(test)]
    pub(super) fn offsets(l: &Layout) -> (usize, usize, usize, usize) {
        (l.seen(), l.displayed(), l.form_dex(), l.languages())
    }
}

/// Gen 7 (`Zukan7` de PKHeX) : `u32` magique, options, 0x80 octets divers, capturé
/// (0x68 octets), quatre régions « vu » puis quatre « affiché » de 0x8C octets (les
/// formes suivent les espèces dans les mêmes régions), langues à 0x550.
mod gen7 {
    use super::{gender_kind, get_flag, set_flag, GenderKind};

    // Vérifié : PKHeX Zukan7.cs (OFS_CAUGHT = 4 + 4 + 0x80, OFS_SEEN = OFS_CAUGHT + 0x68,
    // BitSeenSize = 0x8C, DexLangIDCount = 9, DexLangFlagByteCount = 920) et
    // SaveBlockAccessor7SM/USUM (langues à 0x550 dans le bloc).
    const OFS_CAUGHT: usize = 0x88;
    const OFS_SEEN: usize = OFS_CAUGHT + 0x68;
    const REGION: usize = 0x8C;
    const LANGUAGES: usize = 0x550;
    const LANG_COUNT: usize = 9;
    const LANG_BYTES: usize = 920;

    pub(super) fn get_caught(d: &[u8], species: u16) -> bool {
        get_flag(d, OFS_CAUGHT, species as usize - 1)
    }

    pub(super) fn get_seen(d: &[u8], species: u16) -> bool {
        (0..4).any(|r| get_flag(d, OFS_SEEN + r * REGION, species as usize - 1))
    }

    fn displayed(r: usize) -> usize {
        OFS_SEEN + (4 + r) * REGION
    }

    /// `GetDexLangFlag` : identifiants 1 à 10 sans le 6 (inutilisé).
    fn language_index(lang: u8) -> Option<usize> {
        match lang {
            0 | 6 | 11.. => None,
            7.. => Some(lang as usize - 2),
            _ => Some(lang as usize - 1),
        }
    }

    fn set_language(d: &mut [u8], bit: usize, index: usize, value: bool) {
        let lbit = bit * LANG_COUNT + index;
        if lbit < LANG_BYTES * 8 {
            set_flag(d, LANGUAGES, lbit, value);
        }
    }

    pub(super) fn set_entry(d: &mut [u8], species: u16, seen: bool, caught: bool, lang: u8) {
        let bit = species as usize - 1;
        if !seen {
            // ClearSeen + SetSeenSingle(false) + SetCaughtSingle(false), forme 0.
            for r in 0..4 {
                set_flag(d, OFS_SEEN + r * REGION, bit, false);
                set_flag(d, displayed(r), bit, false);
            }
            set_flag(d, OFS_CAUGHT, bit, false);
            for i in 0..LANG_COUNT {
                set_language(d, bit, i, false);
            }
            return;
        }
        if !get_seen(d, species) {
            // SetSeenSingle (forme 0, sans chromatique) : SetDexFlags pour chaque sexe
            // possible ; le premier pose aussi le drapeau « affiché » si aucun ne l'est.
            let kind = gender_kind(species);
            let mut shifts = Vec::with_capacity(2);
            if kind != GenderKind::Female {
                shifts.push(0);
            }
            if kind == GenderKind::Dual || kind == GenderKind::Female {
                shifts.push(1);
            }
            for shift in shifts {
                set_flag(d, OFS_SEEN + shift * REGION, bit, true);
                if !(0..4).any(|r| get_flag(d, displayed(r), bit)) {
                    set_flag(d, displayed(shift), bit, true);
                }
            }
        }
        set_flag(d, OFS_CAUGHT, bit, caught);
        if caught {
            if let Some(i) = language_index(lang) {
                set_language(d, bit, i, true);
            }
        } else {
            for i in 0..LANG_COUNT {
                set_language(d, bit, i, false);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn displayed_offset(r: usize) -> usize {
        displayed(r)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{gen4 as save4, gen5 as save5, gen6 as save6, gen7 as save7};
    use super::*;

    /// Sauvegarde synthétique vierge de chaque jeu.
    fn blank(version: SaveVersion) -> Vec<u8> {
        match version {
            SaveVersion::RubySapphire | SaveVersion::Emerald | SaveVersion::FireRedLeafGreen => crate::save::gen3_blank(version),
            SaveVersion::DiamondPearl | SaveVersion::Platinum | SaveVersion::HeartGoldSoulSilver => save4::blank(version, 0, 1),
            SaveVersion::BlackWhite | SaveVersion::Black2White2 => save5::blank(version),
            SaveVersion::XY => save6::blank(0x65600, &save6::synthetic_lengths(version)).0,
            SaveVersion::OmegaRubyAlphaSapphire => save6::blank(0x76000, &save6::synthetic_lengths(version)).0,
            SaveVersion::SunMoon => save6::blank(0x6BE00, &save7::synthetic_lengths(version)).0,
            SaveVersion::UltraSunUltraMoon => save6::blank(0x6CC00, &save7::synthetic_lengths(version)).0,
        }
    }

    const ALL: [SaveVersion; 9] = [
        SaveVersion::DiamondPearl,
        SaveVersion::Platinum,
        SaveVersion::HeartGoldSoulSilver,
        SaveVersion::BlackWhite,
        SaveVersion::Black2White2,
        SaveVersion::XY,
        SaveVersion::OmegaRubyAlphaSapphire,
        SaveVersion::SunMoon,
        SaveVersion::UltraSunUltraMoon,
    ];

    fn reopen(save: &SaveFile) -> SaveFile {
        let re = SaveFile::from_bytes(&save.to_bytes()).unwrap();
        let bad: Vec<_> = re.checksums().into_iter().filter(|c| !c.valid).collect();
        assert!(bad.is_empty(), "{:?} : {bad:?}", save.version());
        re
    }

    #[test]
    fn genders_table() {
        assert_eq!(GENDERS.len(), 807);
        assert_eq!(gender_kind(1), GenderKind::Dual);
        assert_eq!(gender_kind(29), GenderKind::Female); // Nidoran♀
        assert_eq!(gender_kind(32), GenderKind::Male); // Nidoran♂
        assert_eq!(gender_kind(81), GenderKind::Genderless); // Magnéti
        assert_eq!(gender_kind(113), GenderKind::Female); // Leveinard
        assert_eq!(gender_kind(132), GenderKind::Genderless); // Métamorph
        assert_eq!(gender_kind(150), GenderKind::Genderless); // Mewtwo
        assert_eq!(gender_kind(380), GenderKind::Female); // Latias
        assert_eq!(gender_kind(381), GenderKind::Male); // Latios
        assert_eq!(gender_kind(807), GenderKind::Genderless); // Zeraora
    }

    #[test]
    fn roundtrip_every_game() {
        for version in ALL {
            let mut save = SaveFile::from_bytes(&blank(version)).unwrap();
            let max = save.dex_max_species();
            let dex = save.pokedex().unwrap();
            assert_eq!(dex.len(), max as usize, "{version:?}");
            assert!(dex.iter().all(|e| !e.seen && !e.caught), "{version:?}");

            save.set_dex_entry(25, true, false).unwrap();
            save.set_dex_entry(445, true, true).unwrap();
            save.set_dex_entry(max, false, true).unwrap(); // capturé ⇒ vu
            save.set_dex_entry(1, true, true).unwrap();
            save.set_dex_entry(1, false, false).unwrap(); // effacé
            assert!(save.set_dex_entry(0, true, true).is_err());
            assert!(save.set_dex_entry(max + 1, true, true).is_err());

            let re = reopen(&save);
            let dex = re.pokedex().unwrap();
            let get = |s: u16| dex[s as usize - 1];
            assert_eq!(get(25), DexEntry { species: 25, seen: true, caught: false }, "{version:?}");
            assert_eq!(get(445), DexEntry { species: 445, seen: true, caught: true }, "{version:?}");
            assert_eq!(get(max), DexEntry { species: max, seen: true, caught: true }, "{version:?}");
            assert_eq!(get(1), DexEntry { species: 1, seen: false, caught: false }, "{version:?}");
            assert_eq!(dex.iter().filter(|e| e.seen).count(), 3, "{version:?}");

            // Tout capturer, puis tout effacer.
            let mut all = re;
            all.dex_set_all(true, true).unwrap();
            let all = reopen(&all);
            assert!(all.pokedex().unwrap().iter().all(|e| e.seen && e.caught), "{version:?}");
            let mut none = all;
            none.dex_set_all(false, false).unwrap();
            let none = reopen(&none);
            assert!(none.pokedex().unwrap().iter().all(|e| !e.seen && !e.caught), "{version:?}");
            // Effacer le Pokédex ne laisse aucun drapeau.
            let start = none.layout.dex;
            let d = &none.data[start..start + Kind::of(version).size()];
            if !matches!(version, SaveVersion::DiamondPearl | SaveVersion::Platinum | SaveVersion::HeartGoldSoulSilver) {
                assert!(d.iter().all(|&b| b == 0), "{version:?}");
            }
        }
    }

    #[test]
    fn uncatch_keeps_seen() {
        for version in ALL {
            let mut save = SaveFile::from_bytes(&blank(version)).unwrap();
            save.set_dex_entry(133, true, true).unwrap();
            save.set_dex_entry(133, true, false).unwrap();
            let e = save.pokedex().unwrap()[132];
            assert!(e.seen && !e.caught, "{version:?}");
        }
    }

    /// Mêmes octets que `PKHeX.Core.Tests/Saves/PokeDex.cs` : drapeaux capturé, vu
    /// (mâle) et affiché d'une espèce mixte en N2B2.
    #[test]
    fn gen5_flags_like_pkhex_test() {
        let mut save = SaveFile::from_bytes(&blank(SaveVersion::Black2White2)).unwrap();
        for species in [1u16, 100, 649] {
            save.set_dex_entry(species, true, true).unwrap();
            let d = &save.data[save.layout.dex..];
            let bit = species as usize - 1;
            let (value, offset) = (1u8 << (bit & 7), bit >> 3);
            let span = &d[0x08..];
            assert_eq!(span[offset] & value, value, "capturé {species}");
            assert_eq!(span[offset + 0x54] & value, value, "vu {species}");
            assert_eq!(span[offset + 0x54 + 0x54 * 4] & value, value, "affiché {species}");
        }
        // Landorus (forme 0) : drapeau de forme 81 dans la 1re région de formes.
        save.set_dex_entry(645, true, true).unwrap();
        let d = &save.data[save.layout.dex..];
        let form_dex = 0x8 + 0x54 * 9;
        assert_eq!(form_dex, 0x2FC);
        assert_ne!(d[form_dex + 81 / 8] & (1 << (81 % 8)), 0);
        // Affichée (région 2).
        assert_ne!(d[form_dex + 2 * 0xB + 81 / 8] & (1 << (81 % 8)), 0);
    }

    #[test]
    fn gen56_offsets_match_pkhex_comments() {
        let o = gen56::offsets(&gen56::Layout::gen5(Kind::Gen5 { b2w2: false }));
        assert_eq!(o, (0x5C, 0x1AC, 0x2FC, 0x320));
        assert_eq!(gen56::offsets(&gen56::Layout::gen5(Kind::Gen5 { b2w2: true })).3, 0x328);
        assert_eq!(gen56::offsets(&gen56::Layout::gen6(Kind::Gen6 { ao: false })), (0x68, 0x1E8, 0x368, 0x3C8));
        assert_eq!(gen56::offsets(&gen56::Layout::gen6(Kind::Gen6 { ao: true })).3, 0x400);
        // Spinda juste après les langues : 0x648 (XY), 0x680 (ROSA), dans le bloc.
        assert!(0x3C8 + 640 + 4 <= Kind::Gen6 { ao: false }.size());
        assert_eq!(gen7::displayed_offset(0), 0xF0 + 4 * 0x8C);
    }

    #[test]
    fn gen4_genders_forms_languages() {
        // HGSS : Pichu (formes = sexes), Zarbi, Sancoki.
        let mut save = SaveFile::from_bytes(&blank(SaveVersion::HeartGoldSoulSilver)).unwrap();
        let lang_at = save.layout.trainer.language;
        save.data[lang_at] = 3; // français
        for s in [172u16, 201, 422, 386, 113] {
            // Effacer d'abord : les cases de formes passent à « libre ».
            save.set_dex_entry(s, false, false).unwrap();
            let d = save.data[save.layout.dex..].to_vec();
            assert!(gen4::forms_of(&d, false, true, s).is_none_or(|f| f.iter().all(|&v| v == 0xFF)), "{s}");
            save.set_dex_entry(s, true, true).unwrap();
        }
        let d = save.data[save.layout.dex..].to_vec();
        assert_eq!(gen4::forms_of(&d, false, true, 172).unwrap(), [0, 0xFF, 0xFF]);
        assert_eq!(gen4::forms_of(&d, false, true, 201).unwrap()[..2], [0, 0xFF]);
        assert_eq!(gen4::forms_of(&d, false, true, 386).unwrap(), [0, 0xFF, 0xFF, 0xFF]);
        // Sancoki : 1 bit par forme, [0, 1] = forme 0 puis case libre (réglée à 1).
        assert_eq!(gen4::forms_of(&d, false, true, 422).unwrap(), [0, 1]);
        // Leveinard (femelle seulement) : sexe 1 dans les deux régions de sexe.
        let bit = 112;
        assert_ne!(d[4 + 2 * 0x40 + bit / 8] & (1 << (bit % 8)), 0);
        assert_ne!(d[4 + 3 * 0x40 + bit / 8] & (1 << (bit % 8)), 0);
        // Pikachu mixte : premier sexe mâle, second femelle.
        save.set_dex_entry(25, true, false).unwrap();
        let d = &save.data[save.layout.dex..];
        assert_eq!(d[4 + 2 * 0x40 + 24 / 8] & 1, 0);
        assert_eq!(d[4 + 3 * 0x40 + 24 / 8] & 1, 1);
        // Langue française (bit 2) pour Pichu.
        assert_eq!(d[gen4::languages(true) + 172], 1 << 2);
        // Effacer Zarbi vide sa liste de formes.
        save.set_dex_entry(201, false, false).unwrap();
        let d = save.data[save.layout.dex..].to_vec();
        assert!(gen4::forms_of(&d, false, true, 201).unwrap().iter().all(|&f| f == 0xFF));

        // DP : langues seulement pour quelques espèces.
        let mut dp = SaveFile::from_bytes(&blank(SaveVersion::DiamondPearl)).unwrap();
        dp.data[dp.layout.trainer.language] = 2;
        dp.set_dex_entry(25, true, true).unwrap();
        dp.set_dex_entry(26, true, true).unwrap();
        let d = &dp.data[dp.layout.dex..];
        assert_eq!(d[gen4::languages(false) + 2], 1 << 1); // Pikachu, 2e de la liste
        assert_eq!(d[gen4::languages(false) + 26], 0);
    }

    #[test]
    fn gen6_xy_foreign_and_oras_counts() {
        let mut xy = SaveFile::from_bytes(&blank(SaveVersion::XY)).unwrap();
        xy.set_dex_entry(25, true, true).unwrap();
        xy.set_dex_entry(700, true, true).unwrap();
        let d = &xy.data[xy.layout.dex..];
        assert_ne!(d[0x64C + 24 / 8] & (1 << (24 % 8)), 0);
        xy.set_dex_entry(25, false, false).unwrap();
        let d = &xy.data[xy.layout.dex..];
        assert_eq!(d[0x64C + 24 / 8] & (1 << (24 % 8)), 0);

        let mut ao = SaveFile::from_bytes(&blank(SaveVersion::OmegaRubyAlphaSapphire)).unwrap();
        ao.set_dex_entry(25, true, false).unwrap();
        let d = &ao.data[ao.layout.dex..];
        assert_eq!(u16::from_le_bytes([d[0x684 + 50], d[0x685 + 50]]), 1);
    }

    #[test]
    fn gen7_languages_and_display() {
        let mut save = SaveFile::from_bytes(&blank(SaveVersion::SunMoon)).unwrap();
        save.data[save.layout.trainer.language] = 8; // coréen → index 6
        save.set_dex_entry(29, true, true).unwrap(); // Nidoran♀
        let d = &save.data[save.layout.dex..];
        let bit = 28;
        // Vu femelle (région 1), affiché femelle, pas mâle.
        assert_ne!(d[0xF0 + 0x8C + bit / 8] & (1 << (bit % 8)), 0);
        assert_eq!(d[0xF0 + bit / 8] & (1 << (bit % 8)), 0);
        assert_ne!(d[gen7::displayed_offset(1) + bit / 8] & (1 << (bit % 8)), 0);
        let lbit = bit * 9 + 6;
        assert_ne!(d[0x550 + lbit / 8] & (1 << (lbit % 8)), 0);
    }
}
