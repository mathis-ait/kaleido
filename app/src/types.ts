export type ViewId = "home" | "library" | "randomizer" | "editor" | "saves" | "settings";

// Miroir des types sérialisés par `kaleido_core::detect`.

export type FileKind = "gb_rom" | "gba_rom" | "nds_rom" | "ctr_rom" | "ctr_dump" | "save" | "unknown" | "switch_game";
export type Platform = "gb" | "gba" | "nds" | "3ds" | "switch";

export interface GameInfo {
  id: string;
  name: string;
  generation: number;
  platform: Platform;
}

export type PokeTypeKey =
  | "normal" | "fighting" | "flying" | "poison" | "ground" | "rock" | "bug" | "ghost" | "steel"
  | "mystery" | "fire" | "water" | "grass" | "electric" | "psychic" | "ice" | "dragon" | "dark" | "fairy";

export type MoveCat = "physical" | "special" | "status";

export interface TypeTag {
  key: PokeTypeKey;
  name: string;
}

export interface BaseStats {
  hp: number;
  attack: number;
  defense: number;
  spAttack: number;
  spDefense: number;
  speed: number;
}

export interface Species {
  id: number;
  name: string;
  types: TypeTag[];
  baseStats: BaseStats;
  total: number;
  abilities: string[];
  hiddenAbility: string | null;
  catchRate: number;
}

export interface RomOverview {
  path: string;
  game: GameInfo;
  internalTitle: string;
  gameCode: string;
  fileCount: number;
  verified: boolean;
  canRandomize: boolean;
  canEdit: boolean;
  species: Species[];
}

// Miroir de `kaleido_core::romedit`.

export interface LevelMove {
  level: number;
  move: number;
}

export interface SpeciesEdit {
  id: number;
  /** PV, Att, Déf, Atq Spé, Déf Spé, Vit. */
  stats: number[];
  /** Index des types dans le jeu (identiques = un seul type). */
  types: [number, number];
  /** Talent 1, talent 2, talent caché (0 = aucun). */
  abilities: [number, number, number];
  catchRate: number;
  learnset: LevelMove[];
}

export interface EditorData {
  hiddenAbility: boolean;
  maxLearnset: number;
  types: { index: number; tag: TypeTag }[];
  abilities: { id: number; name: string }[];
  moves: { id: number; name: string }[];
  species: SpeciesEdit[];
}

export interface RandomizerSettings {
  starters: "unchanged" | "random" | "three_stage" | "triangle" | "custom";
  customStarters: number[];
  /** X / Y : Bulbizarre, Salamèche et Carapuce du Professeur Platane (absent = inchangés). */
  kantoStarters: "unchanged" | "random" | "three_stage" | "triangle" | "custom";
  customKantoStarters: number[];
  catchRate: "unchanged" | "doubled" | "max";
  easyEvolutions: boolean;
  randomMovesets: boolean;
  trainerEvolutions: boolean;
  trainerMaxIvs: boolean;
  shinyOdds: number;
  moves: MoveSettings;
  items: ItemSettings;
  statics: StaticSettings;
  wild: "unchanged" | "random" | "area" | "global";
  wildSimilarStrength: boolean;
  wildLevelPercent: number;
  trainers: "unchanged" | "random" | "type_themed";
  trainersSimilarStrength: boolean;
  trainerLevelPercent: number;
  stats: "unchanged" | "shuffle" | "random";
  randomTypes: boolean;
  randomAbilities: boolean;
  noLegendaries: boolean;
  /** 3DS uniquement (absent des anciens codes de partage : LayeredFS). */
  ctrOutput: CtrOutput;
}

export type CompatMode = "unchanged" | "random" | "random_prefer_type" | "full";

export interface MoveSettings {
  randomTms: boolean;
  randomTutors: boolean;
  noGameBreaking: boolean;
  keepFieldMoves: boolean;
  goodDamagingPercent: number;
  tmCompat: CompatMode;
  fullHmCompat: boolean;
  tutorCompat: CompatMode;
  followEvolutions: boolean;
  levelupSanity: boolean;
}

export interface ItemSettings {
  fieldItems: "unchanged" | "shuffle" | "random" | "random_even";
  banBadFieldItems: boolean;
  shops: "unchanged" | "shuffle" | "random";
  banBadShopItems: boolean;
  banRegularShopItems: boolean;
  banOpShopItems: boolean;
  guaranteeEvolutionItems: boolean;
  guaranteeXItems: boolean;
  noRareCandy: boolean;
  noMasterBall: boolean;
}

export interface Preset {
  id: string;
  name: string;
  description: string;
  settings: RandomizerSettings;
}

export interface PokemonRef {
  id: number;
  name: string;
}

export interface Outcome {
  seed: number;
  shareCode: string;
  starters: PokemonRef[];
  wildSlots: number;
  trainerPokemon: number;
  log: string;
}

/** Jeux pris en charge par le randomizer. */
export const RANDOMIZABLE = [
  "red", "blue", "yellow", "gold", "silver", "crystal",
  "ruby", "sapphire", "emerald", "fire_red", "leaf_green",
  "diamond", "pearl", "platinum", "heart_gold", "soul_silver", "black", "white", "black2", "white2",
  "x", "y", "omega_ruby", "alpha_sapphire", "sun", "moon", "ultra_sun", "ultra_moon",
];

/** Fichiers ouvrables dans l'éditeur et le randomizer (ROM GBA, ROM DS, ROM 3DS, dossier 3DS). */
/** Extension d'une ROM GB, GBC, GBA ou DS, gardée pour les copies écrites par Kaleido. */
export const romExt = (path: string) => path.match(/\.(gbc|gb|gba)$/i)?.[1].toLowerCase() ?? "nds";

export const isRom = (d: Detection) => ["gb_rom", "gba_rom", "nds_rom", "ctr_rom", "ctr_dump"].includes(d.kind) && d.game !== null;

export interface CtrOutcome extends Outcome {
  /** Dossier `romfs` du LayeredFS, si demandé. */
  romfs: string | null;
  /** ROM `.3ds` (ou `.cxi`) reconstruite, si demandée. */
  image: string | null;
}

/** Sortie du randomizer 3DS : dossier LayeredFS, ROM complète ou les deux. */
export type CtrOutput = "layered_fs" | "rom3ds" | "both";

// --- Éditeur de sauvegardes (miroir de `kaleido_core::save::session`).

export type Slot = { kind: "party"; index: number } | { kind: "box"; box: number; index: number };

export interface SlotView {
  slot: Slot;
  species: number;
  form: number;
  nickname: string;
  isNicknamed: boolean;
  isEgg: boolean;
  level: number;
  levelEstimated: boolean;
  exp: number;
  shiny: boolean;
  gender: "male" | "female" | "genderless";
  nature: number;
  natureName: string;
  ability: number;
  heldItem: number;
  moves: number[];
  /** PV, Att, Déf, Atq Spé, Déf Spé, Vit */
  ivs: number[];
  evs: number[];
  otName: string;
  tid: number;
  sid: number;
  ball: number;
  friendship: number;
  pid: number;
  checksumValid: boolean;
  speciesName: string;
  abilityName: string;
  itemName: string | null;
  moveNames: string[];
  stats: number[] | null;
  encryptionConstant: number;
  abilityNumber: number;
  pp: number[];
  ppUps: number[];
  otGender: Gender;
  metLocation: number;
  metLevel: number;
  metDate: PkmDate | null;
  eggLocation: number;
  eggDate: PkmDate | null;
  version: number;
  language: number;
  fatefulEncounter: boolean;
  markings: number[];
  pokerusStrain: number;
  pokerusDays: number;
  tsv: number;
  psv: number;
  extras: PokemonExtras;
  knownMoves: KnownMove[];
  metLocationName: string | null;
  eggLocationName: string | null;
  speciesData: SpeciesData | null;
}

/** Souvenir Gen 6/7 : texte, intensité, ressenti et variable (lieu, espèce, attaque ou objet). */
export interface Memory {
  id: number;
  intensity: number;
  feeling: number;
  variable: number;
}

export interface Handler {
  name: string;
  gender: Gender;
  /** 0 : dresseur d'origine, 1 : soigneur. */
  current: number;
  friendship: number;
  otAffection: number;
  htAffection: number;
  country: number;
  region: number;
  consoleRegion: number;
  /** [région, pays] des 5 derniers lieux, du plus récent au plus ancien. */
  geo: [number, number][];
  fullness: number;
  enjoyment: number;
  otMemory: Memory;
  htMemory: Memory;
}

/** Champs secondaires (absents = n'existent pas dans ce format). */
export interface PokemonExtras {
  /** Sang-froid, Beauté, Grâce, Intelligence, Robustesse, Lustre. */
  contest: number[];
  ribbons: { key: string; name: string; on: boolean }[];
  characteristic: number;
  groundTile: number | null;
  shinyLeaf: number | null;
  walkingMood: number | null;
  nSparkle: boolean | null;
  pokestarFame: number | null;
  formArgument: number | null;
  hyperTraining: boolean[] | null;
  handler: Handler | null;
  superTraining: { medals: boolean[]; secretUnlocked: boolean; supremelyTrained: boolean } | null;
}

export interface ExtrasPatch {
  contest?: number[];
  ribbons?: Record<string, boolean>;
  groundTile?: number;
  shinyLeaf?: number;
  walkingMood?: number;
  nSparkle?: boolean;
  pokestarFame?: number;
  formArgument?: number;
  hyperTraining?: boolean[];
  htName?: string;
  htGender?: Gender;
  currentHandler?: number;
  htFriendship?: number;
  otAffection?: number;
  htAffection?: number;
  country?: number;
  region?: number;
  consoleRegion?: number;
  geo?: [number, number][];
  fullness?: number;
  enjoyment?: number;
  otMemory?: Memory;
  htMemory?: Memory;
  medals?: boolean[];
  secretUnlocked?: boolean;
  supremelyTrained?: boolean;
}

export interface Trainer {
  name: string;
  tid: number;
  sid: number;
  displayId: number;
  gender: Gender;
  money: number;
  playTime: { hours: number; minutes: number; seconds: number };
}

export interface SaveView {
  game: string;
  version: string;
  generation: number;
  trainer: Trainer;
  boxNames: string[];
  party: SlotView[];
  warnings: string[];
  needsResign: boolean;
  checksumsValid: boolean;
  boxFill: number[];
  canUndo: boolean;
  canRedo: boolean;
  trainerNameMax: number;
  boxNameMax: number;
  nicknameMax: number;
}


export interface Detail {
  label: string;
  value: string;
}

export interface Detection {
  path: string;
  fileName: string;
  kind: FileKind;
  title: string;
  game: GameInfo | null;
  platform: Platform | null;
  generation: number | null;
  language: string | null;
  isFrench: boolean;
  size: number;
  details: Detail[];
  warnings: string[];
  /** Signature d'une ROM générée par Kaleido. */
  kaleido: { tool: string; version: string; seed: number; shareCode: string } | null;
  /** Empreinte du contenu, pour repérer les doublons. */
  fingerprint: string | null;
}

/** ROM produite par Kaleido (signature, ou seed retrouvée dans le nom du fichier). */
export const isKaleidoRom = (d: Detection) => !!d.kaleido || d.details.some((x) => x.label === "Randomisée par");

export type Gender = "male" | "female" | "genderless";
export type ShinyMode = "none" | "star" | "square" | "keepPid";

export interface PkmDate {
  year: number;
  month: number;
  day: number;
}

export interface PokemonPatch {
  species?: number;
  form?: number;
  nickname?: string;
  isNicknamed?: boolean;
  level?: number;
  exp?: number;
  nature?: number;
  ability?: number;
  abilityNumber?: number;
  gender?: Gender;
  shiny?: ShinyMode;
  pid?: number;
  encryptionConstant?: number;
  heldItem?: number;
  language?: number;
  moves?: number[];
  pp?: number[];
  ppUps?: number[];
  ivs?: number[];
  evs?: number[];
  friendship?: number;
  otName?: string;
  tid?: number;
  sid?: number;
  otGender?: Gender;
  ball?: number;
  metLocation?: number;
  metLevel?: number;
  metDate?: PkmDate | null;
  eggLocation?: number;
  eggDate?: PkmDate | null;
  version?: number;
  fatefulEncounter?: boolean;
  isEgg?: boolean;
  markings?: number[];
  pokerus?: [number, number];
  extras?: ExtrasPatch;
}

export interface TrainerPatch {
  name?: string;
  tid?: number;
  sid?: number;
  gender?: Gender;
  money?: number;
  hours?: number;
  minutes?: number;
  seconds?: number;
}

export type PouchKind =
  | "items" | "key_items" | "tm_hm" | "medicine" | "berries" | "balls" | "battle_items" | "mail" | "z_crystals" | "roto_powers";

export interface InventoryItem {
  id: number;
  count: number;
  isNew: boolean;
  isFavorite: boolean;
}

export interface Pouch {
  kind: PouchKind;
  capacity: number;
  maxCount: number;
  allowed: number[];
  items: InventoryItem[];
}

export interface KnownMove {
  id: number;
  name: string;
  typeId: number;
  category: string;
  power: number | null;
  accuracy: number | null;
  basePp: number;
  maxPp: number;
}

export interface SpeciesData {
  types: number[];
  /** PV, Att, Déf, AtS, DéS, Vit */
  baseStats: number[];
  abilities: number[];
  abilityNames: string[];
  genderRatio: number;
  formName: string | null;
  formNames: string[];
  baseFriendship: number;
}

export interface Named {
  value: number;
  label: string;
}

export interface SaveLists {
  species: Named[];
  moves: Named[];
  items: Named[];
  abilities: Named[];
  locations: Named[];
  balls: Named[];
  types: string[];
  /** [type, catégorie] de chaque attaque (index = attaque). */
  moveMeta: ([number, MoveCat] | null)[];
  characteristics: string[];
  groundTiles: string[];
  superTraining: string[];
  memories: string[];
  intensities: string[];
  feelings: string[];
  generalLocations: string[];
  consoleRegions: string[];
  countries: Named[];
  /** [pays, région, nom] */
  regions: [number, number, string][];
}

export interface StaticSettings {
  mode: "unchanged" | "swap_legendaries" | "similar_strength" | "random";
  /** Variation des niveaux en % (−50 à +50). */
  levelModifier: number;
  trades: "unchanged" | "given" | "given_and_requested";
  tradeRandomItems: boolean;
  tradeRandomIvs: boolean;
}
