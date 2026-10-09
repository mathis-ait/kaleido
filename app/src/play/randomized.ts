import { reactive } from "vue";
import type { Detection, RandomizerSettings } from "../types";

/** Paramètres appliqués à une ROM générée par Kaleido (relus dans sa signature). */
export interface RandomizedInfo {
  seed: number;
  /** Version de Kaleido qui a généré la ROM. */
  version: string;
  shareCode: string;
  settings: RandomizerSettings;
  /** Journal écrit à la génération, s'il est toujours là. */
  journal: string | null;
}

export const randomizedDialog = reactive({ game: null as Detection | null });

export const openRandomized = (game: Detection) => (randomizedDialog.game = game);
