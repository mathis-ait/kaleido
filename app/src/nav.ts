import { reactive } from "vue";
import type { RandomizerSettings, ViewId } from "./types";

/** Vue affichée, accessible depuis n'importe quel composant. */
export const nav = reactive({
  // Kaleido s'ouvre sur la bibliothèque (remplie automatiquement au démarrage).
  view: "library" as ViewId,
  /** ROM à présélectionner en ouvrant le randomizer. */
  randomizerRom: null as string | null,
  /** Réglages et seed à reprendre dans le randomizer (« Personnaliser » de Nouvelle aventure). */
  randomizerSettings: null as { settings: RandomizerSettings; seed: number } | null,
});
