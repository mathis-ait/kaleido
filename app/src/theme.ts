import { ref, watch } from "vue";

export interface Theme {
  id: string;
  name: string;
  description: string;
  swatch: string[];
}

export const THEMES: Theme[] = [
  { id: "nuit", name: "Prisme Nuit", description: "Sombre, reflets de kaléidoscope", swatch: ["#0c0e1a", "#8b5cf6", "#22d3ee", "#f472b6"] },
  { id: "jour", name: "Prisme Jour", description: "Clair et lumineux", swatch: ["#f5f6fb", "#7c3aed", "#0891b2", "#db2777"] },
  { id: "ds", name: "Console DS", description: "Gris argent et bleu Nintendo DS", swatch: ["#d9dde3", "#2f6fd6", "#5aa0ff", "#1d2a3a"] },
];

const STORAGE_KEY = "kaleido.theme";

function readStoredTheme(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? "nuit";
  } catch {
    return "nuit";
  }
}

export const currentTheme = ref(readStoredTheme());

export function initTheme() {
  const apply = (id: string) => {
    document.documentElement.dataset.theme = id;
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      /* stockage indisponible : le thème ne sera simplement pas mémorisé */
    }
  };
  apply(currentTheme.value);
  watch(currentTheme, apply);
}
