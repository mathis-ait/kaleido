import { computed, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { nav } from "./nav";
import type { EditorData, RomOverview, SpeciesEdit } from "./types";

/** ROM ouverte dans l'éditeur. */
export const editor = reactive({
  overview: null as RomOverview | null,
  /** Données modifiables d'origine (null si le jeu n'est pas encore éditable). */
  data: null as EditorData | null,
  /** Copie modifiée de chaque espèce touchée, par n° national. */
  edits: {} as Record<number, SpeciesEdit>,
  loadingPath: null as string | null,
  error: null as string | null,
});

export const editCount = computed(() => Object.keys(editor.edits).length);

/** Valeurs actuelles d'une espèce (modifiées ou d'origine). */
export function speciesEdit(id: number): SpeciesEdit | undefined {
  return editor.edits[id] ?? editor.data?.species[id - 1];
}

/** Abandonne les modifications en cours, après confirmation. */
export function confirmDiscard(): boolean {
  return !editCount.value || confirm(`${editCount.value} Pokémon modifié(s) non enregistré(s). Abandonner ces modifications ?`);
}

/** Dernière ouverture demandée : une réponse plus ancienne est ignorée. */
let openToken = 0;

/**
 * Ouvre une ROM dans l'éditeur (après confirmation s'il y a des modifications non
 * enregistrées sur une autre ROM). `force` recharge sans rien demander. Renvoie vrai si
 * la ROM est ouverte.
 */
export async function openRom(path: string, force = false): Promise<boolean> {
  nav.view = "editor";
  if (editor.overview?.path === path && !force) return true;
  if (!force && !confirmDiscard()) return false;
  const token = ++openToken;
  editor.loadingPath = path;
  editor.error = null;
  try {
    const overview = await invoke<RomOverview>("open_rom", { path });
    const data = overview.canEdit ? await invoke<EditorData>("rom_editor_data", { path }) : null;
    if (token !== openToken) return false;
    editor.data = data;
    editor.edits = {};
    editor.overview = overview;
    return true;
  } catch (e) {
    if (token === openToken) editor.error = String(e);
    return false;
  } finally {
    if (token === openToken) editor.loadingPath = null;
  }
}

export function closeRom() {
  if (!confirmDiscard()) return;
  editor.overview = null;
  editor.data = null;
  editor.edits = {};
}
