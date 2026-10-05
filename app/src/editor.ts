import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { nav } from "./nav";
import type { RomOverview } from "./types";

/** ROM ouverte dans l'éditeur. */
export const editor = reactive({
  overview: null as RomOverview | null,
  loadingPath: null as string | null,
  error: null as string | null,
});

export async function openRom(path: string) {
  nav.view = "editor";
  if (editor.overview?.path === path) return;
  editor.loadingPath = path;
  editor.error = null;
  try {
    editor.overview = await invoke<RomOverview>("open_rom", { path });
  } catch (e) {
    editor.error = String(e);
  } finally {
    editor.loadingPath = null;
  }
}
