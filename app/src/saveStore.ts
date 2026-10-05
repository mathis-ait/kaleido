import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { nav } from "./nav";
import type { PokemonPatch, SaveView, Slot, SlotView } from "./types";

/** Sauvegarde ouverte dans l'éditeur. */
export const saveState = reactive({
  path: null as string | null,
  view: null as SaveView | null,
  box: 0,
  slots: [] as (SlotView | null)[],
  selected: null as SlotView | null,
  dirty: false,
  loading: false,
  error: null as string | null,
  notice: null as string | null,
});

const sameSlot = (a: Slot, b: Slot) => JSON.stringify(a) === JSON.stringify(b);

async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
  saveState.error = null;
  try {
    return await action();
  } catch (e) {
    saveState.error = String(e);
    return undefined;
  }
}

export async function openSave(path: string) {
  nav.view = "saves";
  saveState.loading = true;
  saveState.selected = null;
  const view = await run(() => invoke<SaveView>("open_save", { path }));
  saveState.loading = false;
  if (!view) return;
  Object.assign(saveState, { path, view, box: 0, dirty: false, notice: null });
  await loadBox(0);
  saveState.selected = view.party[0] ?? null;
}

export async function loadBox(index: number) {
  const slots = await run(() => invoke<(SlotView | null)[]>("save_box", { index }));
  if (slots) {
    saveState.box = index;
    saveState.slots = slots;
  }
}

/** Recharge l'emplacement sélectionné après une modification. */
function reselect(slot: Slot | null) {
  if (!slot) return;
  const all = [...(saveState.view?.party ?? []), ...saveState.slots.filter((s): s is SlotView => !!s)];
  saveState.selected = all.find((s) => sameSlot(s.slot, slot)) ?? null;
}

export async function movePokemon(from: Slot, to: Slot) {
  if (sameSlot(from, to)) return;
  const view = await run(() => invoke<SaveView>("save_move", { from, to }));
  if (!view) return;
  saveState.view = view;
  saveState.dirty = true;
  await loadBox(saveState.box);
  reselect(to);
}

export async function patchPokemon(slot: Slot, patch: PokemonPatch) {
  const updated = await run(() => invoke<SlotView>("save_patch", { slot, patch }));
  if (!updated) return false;
  saveState.dirty = true;
  if (slot.kind === "party" && saveState.view) {
    saveState.view.party[slot.index] = updated;
  } else {
    await loadBox(saveState.box);
  }
  saveState.selected = updated;
  return true;
}

export async function deletePokemon(slot: Slot) {
  const view = await run(() => invoke<SaveView>("save_delete", { slot }));
  if (!view) return;
  saveState.view = view;
  saveState.dirty = true;
  saveState.selected = null;
  await loadBox(saveState.box);
}

export async function importPokemon(slot: Slot, file: string) {
  const v = await run(() => invoke<SlotView>("save_import_pokemon", { slot, file }));
  if (!v) return;
  saveState.dirty = true;
  saveState.view = (await run(() => invoke<SaveView>("save_view"))) ?? saveState.view;
  await loadBox(saveState.box);
  saveState.selected = v;
}

export async function exportPokemon(slot: Slot, output: string) {
  await run(() => invoke("save_export_pokemon", { slot, output }));
  saveState.notice = `Pokémon exporté : ${output}`;
}

export async function writeSave(output?: string) {
  const backup = await run(() => invoke<string | null>("save_write", { output: output ?? null }));
  if (backup === undefined) return;
  saveState.dirty = false;
  saveState.notice = backup ? `Sauvegarde enregistrée. Copie de sécurité de l'original : ${backup}` : "Sauvegarde enregistrée.";
}
