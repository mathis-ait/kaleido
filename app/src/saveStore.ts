import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { nav } from "./nav";
import type { Named, PokemonPatch, Pouch, SaveLists, SaveView, Slot, SlotView, TrainerPatch } from "./types";

export type SavePage = "home" | "boxes" | "pokemon" | "tools" | "gifts" | "nuzlocke" | "manager" | "bank" | "battle";
export type SaveTool = "trainer" | "items" | "dex" | "boxes" | "checks";

/** Pages de l'éditeur, dans l'ordre des onglets (Q / E pour passer de l'une à l'autre). */
export const SAVE_PAGES: { id: SavePage; label: string; icon: string }[] = [
  { id: "boxes", label: "Boîtes", icon: "grid" },
  { id: "pokemon", label: "Pokémon", icon: "ball" },
  { id: "tools", label: "Outils", icon: "sliders" },
  { id: "nuzlocke", label: "Nuzlocke", icon: "swords" },
  { id: "gifts", label: "Cadeaux", icon: "gift" },
  { id: "manager", label: "Sauvegardes", icon: "folder" },
  { id: "bank", label: "Banque", icon: "bank" },
  { id: "battle", label: "Combat", icon: "swords" },
];

/** Sauvegarde ouverte dans l'éditeur. */
export const saveState = reactive({
  path: null as string | null,
  view: null as SaveView | null,
  page: "home" as SavePage,
  tool: null as SaveTool | null,
  box: 0,
  slots: [] as (SlotView | null)[],
  selected: null as SlotView | null,
  dirty: false,
  loading: false,
  error: null as string | null,
  notice: null as string | null,
});

export const sameSlot = (a: Slot, b: Slot) => JSON.stringify(a) === JSON.stringify(b);

let noticeTimer: number | undefined;
export function notify(text: string) {
  saveState.notice = text;
  clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => (saveState.notice = null), 4000);
}

/** Listes du jeu de la sauvegarde ouverte (espèces, attaques, objets, lieux…). */
export const lists = reactive({
  loaded: false,
  species: [] as Named[],
  moves: [] as Named[],
  items: [] as Named[],
  abilities: [] as Named[],
  locations: [] as Named[],
  balls: [] as Named[],
  types: [] as string[],
  /** Noms par identifiant. */
  itemName: {} as Record<number, string>,
  moveName: {} as Record<number, string>,
});

export async function loadLists() {
  const l = await run(() => invoke<SaveLists>("save_lists"));
  if (!l) return;
  Object.assign(lists, l, {
    loaded: true,
    itemName: Object.fromEntries(l.items.map((o) => [o.value, o.label])),
    moveName: Object.fromEntries(l.moves.map((o) => [o.value, o.label])),
  });
}
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
  Object.assign(saveState, { path, view, box: 0, dirty: false, notice: null, page: "home", tool: null });
  rememberSave(path);
  await loadBox(0);
  saveState.selected = view.party[0] ?? null;
  loadLists();
}

export function closeSave() {
  Object.assign(saveState, { path: null, view: null, slots: [], selected: null, dirty: false, page: "manager", tool: null });
}

export async function loadBox(index: number) {
  const slots = await run(() => invoke<(SlotView | null)[]>("save_box", { index }));
  if (slots) {
    saveState.box = index;
    saveState.slots = slots;
  }
}

/** Recharge l'emplacement sélectionné après une modification. */
function reselect(slot: Slot | null | undefined) {
  if (!slot) return;
  const all = [...(saveState.view?.party ?? []), ...saveState.slots.filter((s): s is SlotView => !!s)];
  saveState.selected = all.find((s) => sameSlot(s.slot, slot)) ?? null;
}

/** Après une modification qui renvoie l'état général. */
async function refresh(view: SaveView, select?: Slot | null) {
  saveState.view = view;
  saveState.dirty = true;
  await loadBox(saveState.box);
  reselect(select === undefined ? saveState.selected?.slot : select);
}

export async function movePokemon(from: Slot, to: Slot) {
  if (sameSlot(from, to)) return;
  const view = await run(() => invoke<SaveView>("save_move", { from, to }));
  if (view) await refresh(view, to);
}

/** Copie (Maj + glisser) ou déplacement en écrasant la cible (Alt + glisser). */
export async function copyPokemon(from: Slot, to: Slot, overwrite: boolean) {
  if (sameSlot(from, to)) return;
  const view = await run(() => invoke<SaveView>("save_copy", { from, to, overwrite }));
  if (view) {
    await refresh(view, to);
    notify(overwrite ? "Pokémon déplacé (cible écrasée)" : "Pokémon copié");
  }
}

export async function patchPokemon(slot: Slot, patch: PokemonPatch) {
  const updated = await run(() => invoke<SlotView>("save_patch", { slot, patch }));
  if (!updated) return false;
  saveState.dirty = true;
  const view = await run(() => invoke<SaveView>("save_view"));
  if (view) saveState.view = view;
  if (slot.kind === "box" && slot.box === saveState.box) saveState.slots[slot.index] = updated;
  saveState.selected = updated;
  return true;
}

export async function createPokemon(slot: Slot, species: number, level: number) {
  const created = await run(() => invoke<SlotView>("save_create", { slot, species, level }));
  if (!created) return;
  const view = await run(() => invoke<SaveView>("save_view"));
  if (view) await refresh(view, created.slot);
  notify(`${created.speciesName} ajouté`);
}

export async function deletePokemon(slot: Slot) {
  const view = await run(() => invoke<SaveView>("save_delete", { slot }));
  if (!view) return;
  await refresh(view, null);
  saveState.selected = null;
}

export async function importPokemon(slot: Slot, file: string) {
  const v = await run(() => invoke<SlotView>("save_import_pokemon", { slot, file }));
  if (!v) return;
  const view = await run(() => invoke<SaveView>("save_view"));
  if (view) await refresh(view, v.slot);
}

export async function exportPokemon(slot: Slot, output: string) {
  const ok = await run(() => invoke("save_export_pokemon", { slot, output }));
  if (ok !== undefined) notify(`Pokémon exporté : ${output}`);
}

export async function history(redo: boolean) {
  const view = await run(() => invoke<SaveView>("save_history", { redo }));
  if (!view) return;
  await refresh(view);
  notify(redo ? "Modification rétablie" : "Modification annulée");
}

export async function setTrainer(patch: TrainerPatch) {
  const view = await run(() => invoke<SaveView>("save_set_trainer", { patch }));
  if (view) await refresh(view);
  return !!view;
}

export async function setBoxName(index: number, name: string) {
  const view = await run(() => invoke<SaveView>("save_set_box_name", { index, name }));
  if (view) await refresh(view);
  return !!view;
}

export async function getInventory() {
  return run(() => invoke<Pouch[]>("save_inventory"));
}

export async function setInventory(pouches: Pouch[]) {
  const view = await run(() => invoke<SaveView>("save_set_inventory", { pouches }));
  if (view) await refresh(view);
  return !!view;
}

export async function writeSave(output?: string) {
  const backup = await run(() => invoke<string | null>("save_write", { output: output ?? null }));
  if (backup === undefined) return;
  saveState.dirty = false;
  notify(backup ? `Sauvegarde enregistrée (copie de l'original : ${backup.split(/[\\/]/).pop()})` : "Sauvegarde enregistrée");
}

export function goTo(page: SavePage, tool: SaveTool | null = null) {
  saveState.page = page;
  saveState.tool = tool;
}

export function editPokemon(p: SlotView) {
  saveState.selected = p;
  goTo("pokemon");
}

const RECENT_KEY = "kaleido.recentSaves";

/** Sauvegardes ouvertes récemment (les plus récentes d'abord). */
export function recentSaves(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
  } catch {
    return [];
  }
}

function rememberSave(path: string) {
  try {
    const list = [path, ...recentSaves().filter((p) => p !== path)].slice(0, 30);
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* stockage indisponible : liste non mémorisée */
  }
}
