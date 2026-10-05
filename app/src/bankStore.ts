import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { loadBox, notify, saveState } from "./saveStore";
import type { SaveView, Slot, SlotView } from "./types";

/** Case de la Banque Kaleido. */
export interface BankSlot {
  box: number;
  index: number;
}

/** Pokémon rangé dans la banque (informations gardées en cache dans l'index). */
export interface BankSlotView {
  slot: BankSlot;
  id: string;
  file: string;
  generation: number;
  species: number;
  form: number;
  nickname: string;
  level: number;
  shiny: boolean;
  isEgg: boolean;
  gender: string;
  otName: string;
  originGame: string | null;
  originSave: string | null;
  added: string;
  speciesName: string;
  /** « PK4 » … « PK7 » */
  format: string;
}

export interface Problem {
  kind: "species" | "form" | "move" | "item" | "ability";
  id: number;
  message: string;
  fixable: boolean;
}

export interface Compatibility {
  ok: boolean;
  fixable: boolean;
  converts: boolean;
  from: string;
  to: string;
  blocker: string | null;
  problems: Problem[];
  changes: string[];
}

export interface BankDetail extends BankSlotView {
  pokemon: SlotView;
  handler: string | null;
  compatibility: Compatibility | null;
}

export interface BankBoxInfo {
  name: string;
  count: number;
}

export interface BankInfo {
  path: string;
  defaultPath: string;
  trashPath: string;
  count: number;
  boxes: BankBoxInfo[];
}

interface TransferResult {
  view: SaveView;
  bank: BankInfo;
  bankSlot: BankSlot | null;
  saveSlot: Slot | null;
}

/** État de la banque affichée. */
export const bankState = reactive({
  info: null as BankInfo | null,
  box: 0,
  slots: [] as (BankSlotView | null)[],
  /** Compatibilité de chaque case avec la sauvegarde ouverte. */
  compat: [] as (boolean | null)[],
  selected: null as BankSlot | null,
  detail: null as BankDetail | null,
  error: null as string | null,
});

export const sameBankSlot = (a: BankSlot | null, b: BankSlot | null) => !!a && !!b && a.box === b.box && a.index === b.index;

async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
  saveState.error = null;
  try {
    return await action();
  } catch (e) {
    saveState.error = String(e);
    return undefined;
  }
}

export async function loadBankInfo() {
  const info = await run(() => invoke<BankInfo>("bank_info"));
  if (info) bankState.info = info;
  return info;
}

function setInfo(info: BankInfo) {
  bankState.info = info;
  if (bankState.box >= info.boxes.length) bankState.box = info.boxes.length - 1;
}

export async function loadBankBox(index = bankState.box) {
  const slots = await run(() => invoke<(BankSlotView | null)[]>("bank_box", { index }));
  if (!slots) return;
  bankState.box = index;
  bankState.slots = slots;
  bankState.compat = (await run(() => invoke<(boolean | null)[]>("bank_box_compat", { index }))) ?? [];
}

export async function selectBank(slot: BankSlot | null) {
  bankState.selected = slot;
  bankState.detail = slot ? ((await run(() => invoke<BankDetail>("bank_detail", { slot }))) ?? null) : null;
}

/** Après un dépôt ou un retrait : la sauvegarde a changé (annulable, à enregistrer). */
async function applyTransfer(r: TransferResult) {
  setInfo(r.bank);
  saveState.view = r.view;
  saveState.dirty = true;
  await Promise.all([loadBox(saveState.box), loadBankBox()]);
  const sel = bankState.selected;
  await selectBank(sel && sel.box === bankState.box && bankState.slots[sel.index] ? sel : null);
}

export async function bankMove(from: BankSlot, to: BankSlot) {
  if (sameBankSlot(from, to)) return;
  const ok = await run(() => invoke("bank_move", { from, to }));
  if (ok === undefined) return;
  await loadBankInfo();
  await loadBankBox();
  await selectBank(to);
}

export async function bankDeposit(from: Slot, to: BankSlot | null, copy: boolean) {
  const r = await run(() => invoke<TransferResult>("bank_deposit", { from, to, copy }));
  if (!r) return false;
  await applyTransfer(r);
  if (r.bankSlot) {
    if (r.bankSlot.box !== bankState.box) await loadBankBox(r.bankSlot.box);
    await selectBank(r.bankSlot);
  }
  notify(copy ? "Copie déposée dans la banque" : "Pokémon déposé dans la banque");
  return true;
}

export async function bankWithdraw(from: BankSlot, to: Slot, copy: boolean, strip: boolean) {
  const r = await run(() => invoke<TransferResult>("bank_withdraw", { from, to, copy, strip }));
  if (!r) return false;
  if (!copy) bankState.selected = null;
  await applyTransfer(r);
  notify(copy ? "Copie envoyée dans la sauvegarde" : "Pokémon envoyé dans la sauvegarde");
  return true;
}

export async function bankDelete(slot: BankSlot) {
  const info = await run(() => invoke<BankInfo>("bank_delete", { slot }));
  if (!info) return;
  setInfo(info);
  await loadBankBox();
  await selectBank(null);
  notify("Pokémon retiré de la banque (fichier gardé dans la corbeille)");
}

export async function bankAddBox() {
  const info = await run(() => invoke<BankInfo>("bank_add_box", { name: null }));
  if (!info) return;
  setInfo(info);
  await loadBankBox(info.boxes.length - 1);
}

export async function bankRenameBox(index: number, name: string) {
  const info = await run(() => invoke<BankInfo>("bank_rename_box", { index, name }));
  if (info) setInfo(info);
}

export async function bankDeleteBox(index: number) {
  const info = await run(() => invoke<BankInfo>("bank_delete_box", { index }));
  if (!info) return;
  setInfo(info);
  await loadBankBox(Math.min(index, info.boxes.length - 1));
}

export async function bankImport(files: string[], to: BankSlot | null) {
  const slot = await run(() => invoke<BankSlot>("bank_import", { files, to }));
  if (!slot) return;
  await loadBankInfo();
  await loadBankBox(slot.box);
  await selectBank(slot);
  notify(files.length > 1 ? `${files.length} Pokémon importés` : "Pokémon importé dans la banque");
}

export async function bankExport(slot: BankSlot, output: string) {
  const ok = await run(() => invoke("bank_export", { slot, output }));
  if (ok !== undefined) notify(`Pokémon exporté : ${output}`);
}

export async function bankSearch(text: string, shiny: boolean | null, generation: number | null) {
  return (await run(() => invoke<BankSlotView[]>("bank_search", { query: { text, shiny, generation } }))) ?? [];
}

export async function bankSetPath(path: string | null) {
  const info = await run(() => invoke<BankInfo>("bank_set_path", { path }));
  if (!info) return;
  bankState.info = info;
  bankState.box = 0;
  await loadBankBox(0);
  await selectBank(null);
  notify(`Banque : ${info.path}`);
}
