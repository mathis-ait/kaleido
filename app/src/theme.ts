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
  { id: "graphite", name: "Graphite", description: "Gris neutres, sans reflet ni dégradé", swatch: ["#131416", "#ececee", "#8ab4f8", "#2c2d31"] },
];

const STORAGE_KEY = "kaleido.theme";

function readStoredTheme(): string {
  try {
    const id = localStorage.getItem(STORAGE_KEY);
    // Thème retiré (Lagon, Console DS, Réseau, Pixel) : retour à Prisme Nuit.
    return id && THEMES.some((t) => t.id === id) ? id : "nuit";
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
  // Thème changé dans une autre fenêtre (compagnon ↔ fenêtre principale).
  window.addEventListener("storage", (e) => {
    if (e.key === STORAGE_KEY && e.newValue && e.newValue !== currentTheme.value) currentTheme.value = e.newValue;
  });
}
