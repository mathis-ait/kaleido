import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Detection } from "./types";

/** Fichiers ouverts pendant la session, partagés entre toutes les vues. */
export const library = reactive({
  items: [] as Detection[],
  pending: 0,
  errors: [] as string[],
});

export async function addPaths(dropped: string[]) {
  // Un dossier ordinaire déposé apporte toutes les ROMs et sauvegardes qu'il contient.
  const paths = await invoke<string[]>("expand_paths", { paths: dropped }).catch(() => dropped);
  if (!paths.length) {
    library.errors.push("Aucune ROM ni sauvegarde trouvée dans ce dossier.");
    return;
  }
  const fresh = paths.filter((p) => !library.items.some((d) => d.path === p));
  library.pending += fresh.length;
  await Promise.all(
    fresh.map(async (path) => {
      try {
        library.items.unshift(await invoke<Detection>("detect_file", { path }));
      } catch (e) {
        library.errors.push(String(e));
      } finally {
        library.pending--;
      }
    }),
  );
}

export function removeItem(path: string) {
  library.items = library.items.filter((d) => d.path !== path);
}
