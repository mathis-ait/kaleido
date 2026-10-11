import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { allGames } from "../games";

/** Fenêtre « Romhacks » (ouverte depuis la grille ou le lanceur de la Bibliothèque). */
export const romhacksDialog = reactive({ open: false });

/**
 * Au démarrage : un romhack installé par une version précédente de Kaleido n'a pas les
 * ajustements actuels (ex. taux de chromatiques) → on ouvre la fenêtre pour proposer la mise à jour.
 */
export async function checkRomhackUpdates() {
  const roms = allGames.value.map((g) => g.path).filter((p) => /\.nds$/i.test(p));
  if (!roms.length) return;
  const outdated = await invoke<string[]>("romhacks_outdated", { roms }).catch(() => []);
  if (outdated.length) romhacksDialog.open = true;
}
