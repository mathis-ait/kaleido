import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** Résultat de la commande `check_update` (dernière release GitHub). */
export interface UpdateInfo {
  current: string;
  latest: string | null;
  newer: boolean;
  url: string;
  name: string | null;
  notes: string | null;
}

const AUTO_KEY = "kaleido.updates.auto";

function readAuto(): boolean {
  try {
    return localStorage.getItem(AUTO_KEY) !== "0";
  } catch {
    return true;
  }
}

export const updates = reactive({
  /** Vérifier au démarrage (mémorisé). */
  auto: readAuto(),
  checking: false,
  info: null as UpdateInfo | null,
  error: null as string | null,
});

watch(
  () => updates.auto,
  (v) => {
    try {
      localStorage.setItem(AUTO_KEY, v ? "1" : "0");
    } catch {
      /* stockage indisponible : le choix ne sera pas mémorisé */
    }
  },
);

export async function checkUpdate() {
  updates.checking = true;
  updates.error = null;
  try {
    updates.info = await invoke<UpdateInfo>("check_update");
  } catch (e) {
    updates.error = String(e);
  } finally {
    updates.checking = false;
  }
}

/** Vérification silencieuse au lancement, si elle est activée. */
export function initUpdates() {
  if (updates.auto && !import.meta.env.DEV) checkUpdate();
}
