// Formes des données de la Living Dex (`app/public/livedex/`, générées par
// `tools/livedex/build-data.mjs` depuis Pelagix) et de l'état enregistré (`livedex.json`).

export type TypeId =
  | "normal" | "fighting" | "flying" | "poison" | "ground" | "rock" | "bug" | "ghost" | "steel"
  | "fire" | "water" | "grass" | "electric" | "psychic" | "ice" | "dragon" | "dark" | "fairy" | "stellar";

export type SpeciesTag = "legendary" | "mythical" | "baby" | "starter" | "fossil" | "pseudo-legendary" | "ultra-beast" | "paradox";

/**
 * base : forme 0 · regional : Alola, Galar, Hisui, Paldea · gender : la forme EST le sexe (Mistigrix…)
 * · cosmetic : apparence fixe (Zarbi, Prismillon…) · changeable : modifiable (Motisma, Deoxys…)
 * · fusion · event : distribution seulement · partner : Let's Go · mega · battle · hidden : jamais comptée.
 */
export type FormCategory = "base" | "regional" | "gender" | "cosmetic" | "changeable" | "fusion" | "event" | "partner" | "mega" | "battle" | "hidden";

export interface GameDef {
  /** Identifiant stable (celui de Pelagix) : « platinum », « scarlet »… */
  id: string;
  name: string;
  short: string;
  system: "gb" | "gbc" | "n64" | "gba" | "gcn" | "nds" | "3ds" | "switch" | "mobile";
  gen: number;
  /** `GameVersion` de PKHeX, `null` si aucune. */
  version: number | null;
}

export interface FormSummary {
  /** Index de forme PKHeX. */
  f: number;
  name: string;
  /** Nom complet : « Raichu d'Alola », « Zarbi (B) ». */
  full: string;
  cat: FormCategory;
  region?: "alola" | "galar" | "hisui" | "paldea";
  types: TypeId[];
  female?: boolean;
  gmax?: boolean;
  gender?: "m" | "f";
  variants?: { id: number; name: string }[];
  /** Jeux (index dans `DexIndex.games`) où la forme existe. */
  present: number[];
  /** Jeux où on peut l'obtenir sans évènement. */
  obtain: number[];
  /** Jeux où on ne l'obtient que par un évènement. */
  event: number[];
  /** Pokémon GO : 1 = disponible, 2 = aussi en chromatique. */
  go?: 1 | 2;
}

export interface SpeciesSummary {
  id: number;
  name: string;
  /** Nom anglais (recherche). */
  en: string;
  gen: number;
  tags: SpeciesTag[];
  /** Huitièmes de femelles : -1 asexué, 0 mâles seulement, 8 femelles seulement. */
  genderRate: number;
  genderDiff: boolean;
  family: number;
  forms: FormSummary[];
}

export interface DexIndex {
  meta: { builtAt: string; pkhexVersion: string; counts: { species: number; forms: number; rows: number } };
  games: GameDef[];
  gameBalls: number[][];
  species: SpeciesSummary[];
}

export type EncounterKind = "wild" | "static" | "gift" | "egg" | "trade" | "raid" | "tera" | "outbreak" | "shadow" | "walker" | "dream" | "event";

export interface EncounterRow {
  g: number[];
  k: EncounterKind;
  m?: string;
  /** Index dans `SpeciesDetail.strings`. */
  l?: number;
  lv: [number, number];
  c?: string[];
  s?: "locked" | "forced";
  b?: number;
  d?: 0 | 1 | 2;
  n?: string;
  via?: string;
  rf?: 1;
  x?: { ot?: string; from?: string; to?: string; id?: number };
}

export interface SpeciesDetail {
  id: number;
  strings: string[];
  family: { s: number; f: number; from?: [number, number]; how?: string }[];
  forms: Record<string, { rows: EncounterRow[]; evolve: { from: [number, number]; how: string; g: number[] }[]; breed: number[] }>;
}

// --- Pokémon possédés, lus par le moteur (`livedex_scan`).

export interface Specimen {
  key: string;
  species: number;
  form: number;
  gender: "male" | "female" | "genderless";
  shiny: boolean;
  nickname: string | null;
  level: number;
  ball: number;
  otName: string;
  tid: number;
  sid: number;
  version: number;
  metLocation: string | null;
  metLevel: number;
  metDate: { year: number; month: number; day: number } | null;
  place: string;
  legality: "legal" | "fishy" | "illegal" | null;
}

export interface ScannedSource {
  path: string;
  fileName: string;
  game: string;
  version: string | null;
  generation: number;
  trainer: string;
  modified: number | null;
  specimens: Specimen[];
  error: string | null;
}

// --- État enregistré.

export interface DexRules {
  regional: boolean;
  genderForms: boolean;
  genderDiffs: boolean;
  cosmetic: boolean;
  changeable: boolean;
  heldItem: boolean;
  fusion: boolean;
  event: boolean;
  partner: boolean;
  alcremieSweets: boolean;
  mega: boolean;
  battle: boolean;
  gmax: boolean;
}

/** Pokémon noté à la main (jeux que Kaleido ne lit pas : Switch, HOME, GO, cartouche…). */
export interface ManualEntry {
  id: string;
  species: number;
  form: number;
  variant?: number;
  gender?: "m" | "f" | "n";
  shiny: boolean;
  gmax?: boolean;
  /** `GameDef.id`. */
  game: string;
  method?: string;
  location?: string;
  ball?: number;
  level?: number;
  /** AAAA-MM-JJ. */
  date?: string;
  nickname?: string;
  ot?: string;
  notes?: string;
  createdAt: string;
}

export interface LivedexState {
  version: 1;
  rules: DexRules;
  /** Sauvegardes suivies écartées par l'utilisateur. */
  disabledSources: string[];
  /** Compter les Pokémon de la banque Kaleido. */
  bank: boolean;
  /** Ne compter que les Pokémon jugés légaux (ou douteux) par le moteur de légalité. */
  legalOnly: boolean;
  /** Le premier écran (choix des sauvegardes) a été validé. */
  setupDone: boolean;
  manual: ManualEntry[];
  /** Clé d'un Pokémon lu → date (AAAA-MM-JJ) de sa première détection. */
  firstSeen: Record<string, string>;
  /** Succès → date de déblocage. */
  achievements: Record<string, string>;
}

/** Une ligne du journal : un Pokémon lu dans une sauvegarde ou noté à la main. */
export interface CatchEntry {
  id: string;
  auto: boolean;
  species: number;
  form: number;
  variant?: number;
  gender?: "m" | "f" | "n";
  shiny: boolean;
  gmax?: boolean;
  /** `GameDef.id` du jeu d'origine, si connu. */
  game: string | null;
  location?: string;
  ball?: number;
  level?: number;
  date?: string;
  nickname?: string;
  ot?: string;
  notes?: string;
  /** Pokémon lus : sauvegardes où il se trouve (fichier · boîte). */
  where?: { source: string; label: string; place: string }[];
  legality?: Specimen["legality"];
}
