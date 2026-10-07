import { reactive, watch } from "vue";

/** Styles d'affichage des Pokémon (réglage « Style des Pokémon »). */
export type SpriteStyle = "icons" | "ani" | "gen5ani" | "home";

export interface SpriteStyleInfo {
  id: SpriteStyle;
  name: string;
  description: string;
  /** Dossier du moteur (`sprite://…/<dir>/`) ; `null` pour les icônes. */
  dir: string | null;
  /** Équivalent fixe quand les animations sont réduites. */
  still: string | null;
  ext: "png" | "gif";
  /** Pixel art : agrandi sans lissage. */
  pixel: boolean;
}

export const SPRITE_STYLES: SpriteStyleInfo[] = [
  { id: "icons", name: "Icônes", description: "Petites icônes de menu, légères et nettes", dir: null, still: null, ext: "png", pixel: true },
  { id: "ani", name: "3D animés", description: "Modèles 3D animés, façon X/Y à Ultra-Soleil", dir: "ani", still: "dex", ext: "gif", pixel: false },
  { id: "gen5ani", name: "Pixel animés", description: "Sprites animés façon Noir et Blanc", dir: "gen5ani", still: "gen5", ext: "gif", pixel: true },
  { id: "home", name: "Artwork HOME", description: "Rendus haute définition de Pokémon HOME (fixes)", dir: "home", still: "home", ext: "png", pixel: false },
];

export interface SpritePrefs {
  /** Style des grands affichages (fiche Pokémon, accueil, starters, Pokédex). */
  style: SpriteStyle;
  /** Aussi dans les grilles denses (boîtes, équipe, Pokédex) au lieu des icônes. */
  everywhere: boolean;
  /** Image fixe au lieu des GIF animés. */
  reduceMotion: boolean;
}

const STORAGE_KEY = "kaleido.sprites";

function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

function readStored(): SpritePrefs {
  // Par défaut : pixels animés, partout (les plus lisibles, en grand comme en petit).
  const fallback: SpritePrefs = { style: "gen5ani", everywhere: true, reduceMotion: prefersReducedMotion() };
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") as Partial<SpritePrefs> | null;
    if (!raw) return fallback;
    const style = SPRITE_STYLES.some((s) => s.id === raw.style) ? (raw.style as SpriteStyle) : fallback.style;
    return { style, everywhere: !!raw.everywhere, reduceMotion: raw.reduceMotion ?? fallback.reduceMotion };
  } catch {
    return fallback;
  }
}

export const spritePrefs = reactive<SpritePrefs>(readStored());

watch(
  spritePrefs,
  (p) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
    } catch {
      /* stockage indisponible : le réglage ne sera simplement pas mémorisé */
    }
  },
  { deep: true },
);

export const styleInfo = (id: SpriteStyle) => SPRITE_STYLES.find((s) => s.id === id) ?? SPRITE_STYLES[0];
