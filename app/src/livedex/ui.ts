import { reactive } from "vue";
import type { ManualEntry } from "./types";

export type LivedexTab = "boxes" | "pokedex" | "journal" | "achievements";

/** État de l'interface de la Living Dex, partagé par ses onglets et ses fenêtres. */
export const livedexUi = reactive({
  tab: "boxes" as LivedexTab,
  /** Mode chromatique des boîtes. */
  shiny: false,
  /** Fiche espèce ouverte. */
  species: null as number | null,
  /** Forme mise en avant dans la fiche. */
  form: 0,
  /** Fenêtre « Noter un Pokémon » : entrée à modifier, ou valeurs de départ. */
  entry: null as (Partial<ManualEntry> & { species?: number }) | null,
  settings: false,
  /** Incrémenté par Ctrl+K : le Pokédex met le focus sur sa recherche. */
  searchTick: 0,
});

export function openSpecies(species: number, form = 0) {
  livedexUi.species = species;
  livedexUi.form = form;
}

export function openEntry(preset: Partial<ManualEntry> = {}) {
  livedexUi.entry = { ...preset };
}

const KEY = "kaleido.livedex.ui";
try {
  const saved = JSON.parse(localStorage.getItem(KEY) ?? "null") as { tab?: LivedexTab; shiny?: boolean } | null;
  if (saved?.tab) livedexUi.tab = saved.tab;
  if (saved?.shiny) livedexUi.shiny = true;
} catch {
  /* préférences d'affichage non mémorisées */
}

export function rememberUi() {
  try {
    localStorage.setItem(KEY, JSON.stringify({ tab: livedexUi.tab, shiny: livedexUi.shiny }));
  } catch {
    /* tant pis */
  }
}
