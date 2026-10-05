export type ViewId = "home" | "randomizer" | "editor" | "saves" | "settings";

// Miroir des types sérialisés par `kaleido_core::detect`.

export type FileKind = "nds_rom" | "ctr_rom" | "ctr_dump" | "save" | "unknown";
export type Platform = "nds" | "3ds";

export interface GameInfo {
  id: string;
  name: string;
  generation: number;
  platform: Platform;
}

export type PokeTypeKey =
  | "normal" | "fighting" | "flying" | "poison" | "ground" | "rock" | "bug" | "ghost" | "steel"
  | "mystery" | "fire" | "water" | "grass" | "electric" | "psychic" | "ice" | "dragon" | "dark";

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
  species: Species[];
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
}
