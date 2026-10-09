import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Résultat de la commande `check_update` (dernière release GitHub). */
export interface UpdateInfo {
  current: string;
  latest: string | null;
  newer: boolean;
  /** Kaleido peut télécharger et lancer l'installeur lui-même. */
  installable: boolean;
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
  /** Fenêtre « nouvelle version » ouverte. */
  prompt: false,
  installing: false,
  /** Téléchargement de l'installeur (octets). */
  progress: null as { done: number; total: number } | null,
  installError: null as string | null,
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

/**
 * Télécharge et lance l'installeur de la nouvelle version : Kaleido se ferme, l'installeur
 * fait la mise à jour puis le rouvre.
 */
export async function installUpdate() {
  if (updates.installing) return;
  updates.prompt = true;
  updates.installing = true;
  updates.installError = null;
  updates.progress = null;
  const unlisten = await listen<{ done: number; total: number }>("update-progress", (e) => (updates.progress = e.payload));
  try {
    await invoke("update_install");
  } catch (e) {
    updates.installError = String(e);
    updates.installing = false;
  } finally {
    unlisten();
  }
}

/** Vérification silencieuse au lancement, si elle est activée ; propose la mise à jour s'il y en a une. */
export async function initUpdates() {
  if (!updates.auto || import.meta.env.DEV) return;
  await checkUpdate();
  if (updates.info?.newer) updates.prompt = true;
}
