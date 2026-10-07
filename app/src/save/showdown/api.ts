import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { loadBox, notify, sameSlot, saveState } from "../../saveStore";
import type { SaveView, Slot, SlotView } from "../../types";
import { STAT_SHORT } from "../refdata";

/** Langue des noms d'un texte Showdown. */
export type Lang = "en" | "fr";

/** Pokémon tel qu'écrit dans le texte (noms bruts). */
export interface ShowdownSet {
  nickname: string | null;
  species: string;
  gender: "M" | "F" | null;
  item: string | null;
  ability: string | null;
  level: number | null;
  shiny: boolean;
  happiness: number | null;
  ball: string | null;
  nature: string | null;
  evs: number[];
  ivs: number[];
  moves: string[];
  hiddenPower: string | null;
  ignored: string[];
}

/** Pokémon converti pour le jeu de la sauvegarde (noms français). */
export interface ResolvedSet {
  species: number;
  form: number;
  speciesName: string;
  formName: string | null;
  nickname: string | null;
  gender: "male" | "female" | "genderless" | null;
  heldItem: number;
  itemName: string | null;
  ability: number | null;
  abilityNumber: number | null;
  abilityName: string | null;
  level: number;
  shiny: boolean;
  friendship: number | null;
  ball: number | null;
  ballName: string | null;
  nature: number | null;
  natureName: string | null;
  evs: number[];
  ivs: number[];
  moves: number[];
  moveNames: string[];
  warnings: string[];
  error: string | null;
}

export interface ShowdownPreview {
  lang: Lang;
  sets: ResolvedSet[];
}

export type ImportTarget = { kind: "box"; box: number } | { kind: "slot"; slot: Slot } | { kind: "party" };

export interface ImportedSet {
  speciesName: string;
  slot: Slot | null;
  warnings: string[];
  error: string | null;
}

export interface ImportReport {
  imported: number;
  sets: ImportedSet[];
  lang: Lang;
}

export interface SetOptions {
  moves: string[][];
  items: string[];
  abilities: string[];
  natures: string[];
}

export interface SmogonSet {
  format: string;
  formatLabel: string;
  name: string;
  speciesKey: string;
  sameForm: boolean;
  set: ShowdownSet;
  resolved: ResolvedSet;
  options: SetOptions;
}

export interface SmogonSets {
  generation: number;
  sets: SmogonSet[];
  fetchedAt: number | null;
  stale: boolean;
}

/** Fenêtres Showdown / Smogon (montées par les pages Boîtes et Pokémon). */
export const showdownUi = reactive({
  open: false,
  tab: "import" as "import" | "export",
  smogon: false,
  teams: false,
});

export function openShowdown(tab: "import" | "export" = "import") {
  showdownUi.tab = tab;
  showdownUi.open = true;
}

/** Abréviations communes (glossaire « statShort »). */
export const STAT_LABELS = STAT_SHORT;

/** « 252 Att / 4 Déf Spé / 252 Vit » (valeurs différentes de `skip`). */
export function statLine(values: number[], skip: number) {
  return values
    .map((v, i) => (v !== skip ? `${v} ${STAT_LABELS[i]}` : ""))
    .filter(Boolean)
    .join(" / ");
}

export const slotText = (s: Slot, boxNames: string[]) =>
  s.kind === "party" ? `Équipe, place ${s.index + 1}` : `${boxNames[s.box] ?? `Boîte ${s.box + 1}`}, case ${s.index + 1}`;

/** Recharge l'état après un import et sélectionne le premier Pokémon ajouté. */
async function afterChange(select: Slot | null) {
  const view = await invoke<SaveView>("save_view");
  saveState.view = view;
  saveState.dirty = true;
  if (select?.kind === "box" && select.box !== saveState.box) await loadBox(select.box);
  else await loadBox(saveState.box);
  if (select) {
    const all = [...view.party, ...saveState.slots.filter((s): s is SlotView => !!s)];
    saveState.selected = all.find((s) => sameSlot(s.slot, select)) ?? saveState.selected;
  }
}

export function previewShowdown(text: string) {
  return invoke<ShowdownPreview>("showdown_preview", { text });
}

export async function importShowdown(text: string, target: ImportTarget) {
  const report = await invoke<ImportReport>("showdown_import", { text, target });
  if (report.imported) {
    await afterChange(report.sets.find((s) => s.slot)?.slot ?? null);
    notify(report.imported > 1 ? `${report.imported} Pokémon ajoutés depuis Showdown` : "Pokémon ajouté depuis Showdown");
  }
  return report;
}

export function exportShowdown(slots: Slot[], lang: Lang) {
  return invoke<string>("showdown_export", { slots, lang });
}

export function smogonSets(species: number, form: number, refresh = false) {
  return invoke<SmogonSets>("smogon_sets", { species, form, refresh });
}

/** Applique un set au Pokémon de l'emplacement ; renvoie les avertissements. */
export async function applySet(slot: Slot, set: ShowdownSet) {
  const res = await invoke<{ view: SlotView; warnings: string[] }>("showdown_apply", { slot, set });
  saveState.dirty = true;
  saveState.view = await invoke<SaveView>("save_view");
  if (slot.kind === "box" && slot.box === saveState.box) saveState.slots[slot.index] = res.view;
  saveState.selected = res.view;
  return res.warnings;
}

/** Ajoute un set comme nouveau Pokémon. */
export async function addSet(set: ShowdownSet, target: ImportTarget) {
  const report = await invoke<ImportReport>("showdown_add_set", { set, target });
  if (report.imported) {
    const slot = report.sets[0]?.slot ?? null;
    const keep = saveState.selected;
    await afterChange(null);
    saveState.selected = keep;
    if (slot) notify(`${report.sets[0].speciesName} ajouté : ${slotText(slot, saveState.view?.boxNames ?? [])}`);
  }
  return report;
}
