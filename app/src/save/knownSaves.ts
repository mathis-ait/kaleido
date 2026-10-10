import { invoke } from "@tauri-apps/api/core";
import { library } from "../library";
import { recentSaves, saveState } from "../saveStore";

/** Dossier de sauvegardes choisi dans le gestionnaire (mémorisé localement). */
export const SAVES_FOLDER_KEY = "kaleido.savesFolder";

export function savesFolder(): string | null {
  try {
    return localStorage.getItem(SAVES_FOLDER_KEY);
  } catch {
    return null;
  }
}

export interface KnownSaves {
  paths: string[];
  /** Émulateur qui utilise chaque sauvegarde détectée automatiquement. */
  emulatorOf: Record<string, string>;
}

/**
 * Toutes les sauvegardes que Kaleido connaît : celles de la bibliothèque, du dossier choisi,
 * la sauvegarde ouverte, les récentes, celles des émulateurs (Azahar, Citra, melonDS, DeSmuME)
 * et les `.sav` rangés à côté des ROMs. Les chemins peuvent désigner des fichiers non pris en
 * charge : c'est la lecture qui tranche.
 */
export async function knownSaves(): Promise<KnownSaves> {
  const paths = new Set<string>(library.items.filter((d) => d.kind === "save").map((d) => d.path));
  const folder = savesFolder();
  if (folder) {
    const found = await invoke<string[]>("expand_paths", { paths: [folder] }).catch(() => []);
    found.forEach((p) => paths.add(p));
  }
  if (saveState.path) paths.add(saveState.path);
  recentSaves().forEach((p) => paths.add(p));
  const romDirs = [...new Set(library.items.filter((d) => d.kind !== "save").map((d) => d.path.replace(/[\\/][^\\/]*$/, "")))];
  const fromEmus = await invoke<{ path: string; emulator: string; game: string | null }[]>("emulator_saves", { romDirs }).catch(() => []);
  fromEmus.forEach((s) => paths.add(s.path));
  return { paths: [...paths], emulatorOf: Object.fromEntries(fromEmus.map((s) => [s.path, s.emulator])) };
}
