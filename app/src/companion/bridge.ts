import { listen } from "@tauri-apps/api/event";
import { linkRom, selectTrainer } from "../battle";
import { nav } from "../nav";
import { goTo, openSave, saveState, type SavePage } from "../saveStore";

/**
 * Fenêtre principale : demandes du compagnon (« Modifier », « Préparer ce combat »).
 * La sauvegarde de la partie s'ouvre dans l'éditeur, éventuellement sur la page Combat.
 */
export function initCompanionBridge() {
  listen<{ path: string; rom: string | null; trainer: number | null; page: SavePage | null }>("companion-open-in-main", async (e) => {
    const { path, rom, trainer, page } = e.payload;
    nav.view = "saves";
    if (saveState.path !== path) await openSave(path);
    if (trainer !== null && rom) {
      await linkRom(rom, path);
      await selectTrainer(trainer);
      goTo("battle");
    } else {
      goTo(page ?? "home");
    }
  }).catch(() => undefined);
}
