import gb from "./assets/consoles/gb.svg";
import gbc from "./assets/consoles/gbc.svg";
import gba from "./assets/consoles/gba.svg";
import nds from "./assets/consoles/nds.svg";
import ctr from "./assets/consoles/3ds.svg";
import nx from "./assets/consoles/switch.svg";
import type { Detection, Platform } from "./types";

/**
 * Consoles telles que la bibliothèque les montre : nom, logo officiel (monochrome, posé en
 * masque pour prendre la couleur du texte) et rapport largeur / hauteur du logo.
 * La Game Boy Color n'est pas une plateforme à part (même émulateur, même onglet) : seul son
 * logo change, pour Or, Argent et Cristal.
 */
export type ConsoleId = "gb" | "gbc" | "gba" | "nds" | "3ds" | "switch";

export interface ConsoleInfo {
  label: string;
  logo: string;
  /** Largeur / hauteur du logo. */
  ratio: number;
  /** Hauteur optique relative : les logos sur deux lignes paraissent plus petits à hauteur égale. */
  scale: number;
}

export const CONSOLES: Record<ConsoleId, ConsoleInfo> = {
  gb: { label: "Game Boy", logo: gb, ratio: 1126.5 / 197.4, scale: 0.92 },
  gbc: { label: "Game Boy Color", logo: gbc, ratio: 577.29 / 235.66, scale: 1.55 },
  gba: { label: "Game Boy Advance", logo: gba, ratio: 401.7 / 46.8, scale: 1.2 },
  nds: { label: "Nintendo DS", logo: nds, ratio: 718 / 109, scale: 1.12 },
  "3ds": { label: "Nintendo 3DS", logo: ctr, ratio: 127.8 / 15.6, scale: 1 },
  switch: { label: "Nintendo Switch", logo: nx, ratio: 1456 / 483, scale: 1.6 },
};

/** Filtre de la bibliothèque : toutes les consoles ou une seule. */
export type ShelfFilter = "all" | Platform;

/** Ordre chronologique des onglets. */
export const SHELVES: Platform[] = ["gb", "gba", "nds", "3ds", "switch"];

/** Jeux Game Boy Color reconnus (les ROM `.gbc` le sont d'office). */
const GBC_GAMES = ["gold", "silver", "crystal"];

export function consoleOf(d: Pick<Detection, "platform" | "fileName" | "game">): ConsoleId {
  if (d.platform === "gb") return GBC_GAMES.includes(d.game?.id ?? "") || /\.gbc$/i.test(d.fileName) ? "gbc" : "gb";
  return d.platform ?? "nds";
}

/** « 1ʳᵉ génération », « 4ᵉ génération ». */
export const generationLabel = (n: number) => `${n}${n === 1 ? "ʳᵉ" : "ᵉ"} génération`;
