import type { Detection } from "../../types";

/**
 * Supports du mode Cartouche : un modèle par support (jamais par jeu), ses cotes
 * réelles, sa zone d'étiquette, ses coques et son clic d'insertion.
 *
 * Ce fichier ne dépend pas de three.js : la fiche Inspecter s'en sert aussi en mode
 * Jaquettes, sans charger la scène.
 */

export type Support = "ds" | "3ds" | "gba" | "gb" | "switch";
export type Finish = "mat" | "brillant" | "translucide";
export type Wear = "neuve" | "jouee";

export interface Shell {
  id: string;
  label: string;
  /** Couleur du plastique (sRGB). */
  color: string;
}

/** Silhouette plate de la console dans laquelle la cartouche s'insère. */
export type Slot = "ds-back" | "3ds-back" | "gba-top" | "gb-top" | "switch-dock";

export interface SupportModel {
  id: Support;
  label: string;
  /** Cotes réelles en mm : largeur, hauteur, épaisseur. */
  size: [number, number, number];
  /** Zone d'étiquette sur la face avant, en mm, origine en haut à gauche du contour SVG. */
  labelZone: { x: number; y: number; w: number; h: number };
  shells: Shell[];
  defaultShell: string;
  slot: Slot;
  /**
   * Partie de la jaquette reprise sur l'étiquette (fractions de l'image) : la moitié haute,
   * sans le bandeau de la plateforme ni les logos d'éditeur du haut de la boîte.
   */
  coverCrop: { x: number; y: number; w: number; h: number };
  /** Clic d'insertion, synthétisé (voir `insertClick` dans audio.ts). */
  click: { f: number[]; d: number; type: OscillatorType; noise: number };
}

const GREY_DS: Shell = { id: "gris", label: "Gris", color: "#3d3e44" };
const BLACK: Shell = { id: "noir", label: "Noir", color: "#26272c" };
const WHITE: Shell = { id: "blanc", label: "Blanc", color: "#e9ebef" };

export const SUPPORTS: Record<Support, SupportModel> = {
  ds: {
    id: "ds",
    label: "Carte DS",
    size: [33, 35, 3.8],
    labelZone: { x: 2.5, y: 8, w: 28, h: 24.5 },
    shells: [GREY_DS, BLACK, WHITE],
    defaultShell: "gris",
    slot: "ds-back",
    // Boîte DS : bandeau « Nintendo DS » à gauche.
    coverCrop: { x: 0.15, y: 0.08, w: 0.85, h: 0.45 },
    click: { f: [2900, 1800], d: 0.022, type: "square", noise: 0.25 },
  },
  "3ds": {
    id: "3ds",
    label: "Carte 3DS",
    size: [35, 33, 3.8],
    labelZone: { x: 2, y: 7, w: 29, h: 23.5 },
    shells: [{ id: "gris-clair", label: "Gris clair", color: "#d4d5d8" }, BLACK, { id: "gris-fonce", label: "Gris foncé", color: "#55585f" }],
    defaultShell: "gris-clair",
    slot: "3ds-back",
    // Boîte 3DS : bandeau « Nintendo 3DS » à droite.
    coverCrop: { x: 0, y: 0.08, w: 0.86, h: 0.45 },
    click: { f: [2500, 1500], d: 0.024, type: "square", noise: 0.25 },
  },
  gba: {
    id: "gba",
    label: "Cartouche GBA",
    size: [58, 35, 7],
    labelZone: { x: 6, y: 5, w: 46, h: 21 },
    shells: [
      { id: "gris", label: "Gris", color: "#8e9097" },
      { id: "noir", label: "Noir", color: "#2a2b2f" },
      { id: "vert", label: "Vert", color: "#2f9a5a" },
      { id: "rouge", label: "Rouge", color: "#c8343a" },
      { id: "bleu", label: "Bleu", color: "#2f5fbf" },
      { id: "violet", label: "Violet", color: "#6b4fb8" },
      { id: "orange", label: "Orange", color: "#e07a2a" },
    ],
    defaultShell: "gris",
    slot: "gba-top",
    coverCrop: { x: 0.2, y: 0.08, w: 0.8, h: 0.45 },
    click: { f: [900, 520], d: 0.05, type: "triangle", noise: 0.6 },
  },
  gb: {
    id: "gb",
    label: "Cartouche GB",
    size: [57, 65, 8],
    labelZone: { x: 5.5, y: 13, w: 46, h: 40 },
    shells: [
      { id: "gris", label: "Gris", color: "#b8b5ad" },
      { id: "rouge", label: "Rouge", color: "#c8343a" },
      { id: "bleu", label: "Bleu", color: "#2f5fbf" },
      { id: "jaune", label: "Jaune", color: "#f0c22e" },
      { id: "argent", label: "Argent", color: "#c6cad1" },
      { id: "or", label: "Or", color: "#c9a23a" },
    ],
    defaultShell: "gris",
    slot: "gb-top",
    coverCrop: { x: 0.13, y: 0.1, w: 0.87, h: 0.5 },
    click: { f: [760, 430], d: 0.06, type: "triangle", noise: 0.7 },
  },
  switch: {
    id: "switch",
    label: "Carte Switch",
    size: [21, 31, 3.3],
    labelZone: { x: 1.5, y: 3.5, w: 18, h: 26 },
    shells: [BLACK, { id: "gris", label: "Gris", color: "#7b7e86" }],
    defaultShell: "noir",
    slot: "switch-dock",
    // Icône officielle du jeu (carrée, sans bandeau) : reprise entière.
    coverCrop: { x: 0, y: 0, w: 1, h: 1 },
    click: { f: [3400], d: 0.014, type: "square", noise: 0.15 },
  },
};

export const FINISHES: { id: Finish; label: string }[] = [
  { id: "mat", label: "Mat" },
  { id: "brillant", label: "Brillant" },
  { id: "translucide", label: "Translucide" },
];

export const WEARS: { id: Wear; label: string }[] = [
  { id: "neuve", label: "Neuve" },
  { id: "jouee", label: "Jouée" },
];

/** Coque et finition déduites du jeu (identifiants de `games.rs`). */
export const GAME_LOOKS: Record<string, { shell: string; finish: Finish }> = {
  ruby: { shell: "rouge", finish: "translucide" },
  sapphire: { shell: "bleu", finish: "translucide" },
  emerald: { shell: "vert", finish: "translucide" },
  fire_red: { shell: "rouge", finish: "mat" },
  leaf_green: { shell: "vert", finish: "mat" },
  gold: { shell: "or", finish: "brillant" },
  silver: { shell: "argent", finish: "brillant" },
  crystal: { shell: "bleu", finish: "translucide" },
  red: { shell: "rouge", finish: "mat" },
  blue: { shell: "bleu", finish: "mat" },
  yellow: { shell: "jaune", finish: "mat" },
};

export function supportOf(d: Pick<Detection, "platform">): Support {
  switch (d.platform) {
    case "3ds":
      return "3ds";
    case "gba":
      return "gba";
    case "gb":
      return "gb";
    case "switch":
      return "switch";
    default:
      return "ds";
  }
}

/** Choix « Inspecter » enregistrés (miroir de `CartridgeLook`, library.rs). */
export interface StoredLook {
  shell?: string;
  finish?: Finish;
  label?: "custom";
  wear?: Wear;
}

export interface Look {
  support: SupportModel;
  shell: Shell;
  finish: Finish;
  wear: Wear;
  customLabel: boolean;
}

/** Apparence finale : choix de l'utilisateur, sinon déduite du jeu, sinon celle du support. */
export function resolveLook(d: Pick<Detection, "platform" | "game">, stored?: StoredLook | null): Look {
  const support = SUPPORTS[supportOf(d)];
  const natural = d.game ? GAME_LOOKS[d.game.id] : undefined;
  const find = (id: string | undefined) => (id ? support.shells.find((s) => s.id === id) : undefined);
  const shell = find(stored?.shell) ?? find(natural?.shell) ?? find(support.defaultShell) ?? support.shells[0];
  return {
    support,
    shell,
    finish: stored?.finish ?? natural?.finish ?? "mat",
    wear: stored?.wear ?? "neuve",
    customLabel: stored?.label === "custom",
  };
}

/** FNV-1a 32 bits, en hexadécimal. */
export function hash32(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16).padStart(8, "0");
}

/** Clé des réglages et du cache d'étiquette : empreinte de la ROM, sinon title ID, sinon chemin. */
export function cartridgeKey(d: Pick<Detection, "fingerprint" | "details" | "path">): string {
  if (d.fingerprint) return d.fingerprint.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 64) || hash32(d.fingerprint);
  const tid = d.details.find((x) => x.label === "Title ID")?.value.replace(/^0x/i, "").toUpperCase();
  if (tid) return `tid-${tid}`;
  return `path-${hash32(d.path.toLowerCase())}`;
}

const REGIONS: Record<string, string> = {
  E: "USA",
  P: "EUR",
  F: "FRA",
  D: "NOE",
  I: "ITA",
  S: "ESP",
  J: "JPN",
  K: "KOR",
  H: "HOL",
  U: "AUS",
  X: "EUR",
  Y: "EUR",
  Z: "EUR",
};

/** Code imprimé sur l'étiquette, d'après l'en-tête de la ROM : `NTR-CPUF-FRA`, `CTR-P-EKJF`, `AGB-BPEE-USA`. */
export function cartridgeCode(d: Pick<Detection, "platform" | "details" | "game">): string | null {
  const detail = (label: string) => d.details.find((x) => x.label === label)?.value.trim() || null;
  if (d.platform === "3ds") return detail("Code produit");
  const code = detail("Code jeu");
  // Noire, Blanche, Noire 2 et Blanche 2 sont des cartes DSi (TWL), comme l'indique leur étiquette.
  const prefix = d.platform === "nds" ? (code?.startsWith("IR") ? "TWL" : "NTR") : d.platform === "gba" ? "AGB" : d.platform === "gb" ? (d.game?.id === "crystal" ? "CGB" : "DMG") : null;
  if (!prefix || !code) return prefix && d.platform === "gb" ? prefix : null;
  const region = code.length === 4 ? REGIONS[code[3]] : undefined;
  return region ? `${prefix}-${code}-${region}` : `${prefix}-${code}`;
}

/**
 * Photo de la vraie carte (GameTDB, servie par `cover://` : voir `cart_urls` dans library.rs),
 * d'après le code de l'en-tête : `photo-ds-CPUF`, `photo-3ds-ECRA`. GameTDB n'a ni GBA, ni GB, ni Switch.
 */
export function cartPhotoKey(d: Pick<Detection, "platform" | "details">): string | null {
  const detail = (label: string) => d.details.find((x) => x.label === label)?.value.trim() ?? "";
  let code = "";
  if (d.platform === "nds") code = detail("Code jeu");
  else if (d.platform === "3ds") code = detail("Code produit").split("-").pop() ?? "";
  if (!/^[A-Z0-9]{4}$/.test(code)) return null;
  return `photo-${d.platform === "nds" ? "ds" : "3ds"}-${code}`;
}

/** Étiquette dans la photo GameTDB d'une carte (fractions de l'image), sans le plastique autour. */
export const PHOTO_CROP: Partial<Record<Support, { x: number; y: number; w: number; h: number }>> = {
  ds: { x: 0.1, y: 0.06, w: 0.8, h: 0.815 },
  "3ds": { x: 0.097, y: 0.093, w: 0.747, h: 0.812 },
};
