import { invoke } from "@tauri-apps/api/core";
import type { Gender, PkmDate, SaveView, Slot, SlotView } from "../../types";

/** Types et commandes de la page « Cadeaux mystère » (voir app/src-tauri/src/gifts.rs). */

export type GiftFormat = "pcd" | "pgt" | "pgf" | "wc6" | "wc6full" | "wc7" | "wc7full";
export type GiftKind = "pokemon" | "egg" | "item" | "other";
export type ShinyRule = "never" | "random" | "always" | "always_star" | "always_square";
export type GiftSort = "default" | "newest" | "species" | "card" | "title";

export interface GameChip {
  id: number;
  name: string;
}

export interface GiftSummary {
  id: number;
  format: GiftFormat;
  formatLabel: string;
  generation: number;
  cardId: number;
  title: string;
  kind: GiftKind;
  kindLabel: string;
  species: number;
  form: number;
  speciesName: string | null;
  level: number;
  shiny: ShinyRule | null;
  egg: boolean;
  games: GameChip[];
  date: PkmDate | null;
  itemNames: string[];
}

export interface GiftPokemon {
  species: number;
  form: number;
  level: number;
  metLevel: number;
  egg: boolean;
  moves: number[];
  relearn: number[];
  heldItem: number;
  ball: number;
  otName: string | null;
  otGender: Gender | null;
  tid: number | null;
  sid: number | null;
  shiny: ShinyRule;
  pid: number | null;
  nature: number | null;
  gender: Gender | null;
  ivs: (number | null)[];
  perfectIvs: number;
  metLocation: number;
  eggLocation: number;
  language: number;
  nickname: string | null;
  originGame: number;
  fateful: boolean;
  ribbons: string[];
  evs: number[];
}

export interface GiftDetails extends GiftSummary {
  pokemon: GiftPokemon | null;
  formName: string | null;
  ot: string;
  trainerId: string;
  otGender: Gender | null;
  ballName: string | null;
  heldItemName: string | null;
  moveNames: string[];
  relearnNames: string[];
  natureName: string;
  genderLabel: string;
  abilityLabel: string;
  ivsLabel: string;
  metLocationName: string | null;
  eggLocationName: string | null;
  languageName: string;
  ribbonNames: string[];
  shinyLabel: string | null;
  notes: string[];
  /** Compatibilité avec la sauvegarde ouverte (null : aucune sauvegarde). */
  compatible: boolean | null;
  incompatibleReason: string | null;
  extension: string;
}

export interface GiftQuery {
  species: number | null;
  text: string;
  shiny: boolean | null;
  egg: boolean | null;
  generations: number[];
  kinds: GiftKind[];
  versions: number[];
  sort: GiftSort;
}

export interface GiftPage {
  total: number;
  offset: number;
  items: GiftSummary[];
}

export interface GiftOverview {
  total: number;
  perGeneration: Record<string, number>;
  species: { value: number; label: string; count: number }[];
}

export interface GiftAdded {
  message: string;
  warning: string | null;
  slot: Slot | null;
  pokemon: SlotView | null;
  view: SaveView;
}

/** Identifiants des cartes ouvertes depuis un fichier (voir IMPORTED_BASE côté Rust). */
export const IMPORTED_BASE = 1_000_000;

export const giftsApi = {
  search: (query: GiftQuery, onlyThisGame: boolean, offset: number, limit: number) =>
    invoke<GiftPage>("gifts_search", { query, onlyThisGame, offset, limit }),
  overview: () => invoke<GiftOverview>("gifts_overview"),
  details: (id: number) => invoke<GiftDetails>("gifts_details", { id }),
  add: (id: number, slot: Slot | null) => invoke<GiftAdded>("gifts_add", { id, slot }),
  exportFile: (id: number, output: string) => invoke<void>("gifts_export", { id, output }),
  importFile: (file: string) => invoke<GiftDetails>("gifts_import", { file }),
};

/** Extensions acceptées à l'ouverture d'un fichier de cadeau. */
export const GIFT_EXTENSIONS = ["pcd", "pgt", "pgf", "wc6", "wc6full", "wc7", "wc7full"];

export function isShiny(rule: ShinyRule | null) {
  return rule === "always" || rule === "always_star" || rule === "always_square";
}

export function formatDate(d: PkmDate | null) {
  if (!d) return null;
  return `${String(d.day).padStart(2, "0")}/${String(d.month).padStart(2, "0")}/${d.year}`;
}
