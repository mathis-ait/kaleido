import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { loadBox, notify, saveState } from "../saveStore";
import type { Gender, SaveView, Slot, SlotView } from "../types";

/** Résultat d'une vérification du moteur (`kaleido_core::legality`). */
export interface LegalityCheck {
  severity: "valid" | "fishy" | "invalid";
  code: string;
  title: string;
  detail: string;
  tab: string | null;
}

export interface EncounterSummary {
  kind: string;
  kindLabel: string;
  species: number;
  speciesName: string;
  location: number;
  locationName: string | null;
  levelMin: number;
  levelMax: number;
  versions: string[];
}

export type Verdict = "legal" | "fishy" | "illegal";

export interface LegalityReport {
  verdict: Verdict;
  verdictLabel: string;
  checks: LegalityCheck[];
  encounter: EncounterSummary | null;
  origin: string;
  pidType: string | null;
  errors: number;
  warnings: number;
}

export interface SlotReport extends LegalityReport {
  slot: Slot;
}

export interface LegalizeResult {
  view: SlotView;
  changes: string[];
  report: LegalityReport;
  success: boolean;
}

/** Ligne de la base « Rencontres ». */
export interface EncounterEntry {
  game: string;
  gameName: string;
  index: number;
  species: number;
  form: number;
  speciesName: string;
  formName?: string;
  kind: string;
  kindLabel: string;
  family: string;
  levelMin: number;
  levelMax: number;
  location: number;
  locationName: string;
  versions: number[];
  versionNames: string[];
  ball?: string;
  ability: string;
  hiddenAbility: boolean;
  shinyLock: boolean;
  shinyAlways: boolean;
  flawlessIvs: number;
  fixedIvs?: number[];
  gender?: number;
  nature?: string;
  moves?: string[];
  heldItem?: string;
  trainer?: string;
  fateful: boolean;
  egg: boolean;
  title?: string;
}

export interface SpeciesEncounters {
  species: number;
  name: string;
  families: string[];
  count: number;
  levelMin: number;
  games: string[];
}

export interface GenerateRequest {
  species: number;
  form?: number;
  level: number;
  shiny?: boolean;
  nature?: number;
  gender?: Gender;
  abilityNumber?: number;
  ball?: number;
  encounterIndex?: number;
  encounterGame?: string;
}

/** Glossaire des termes de légalité par code de vérification (bulles « i »). */
export const CHECK_TERMS: Record<string, string> = {
  encounter: "encounter",
  "encounter-none": "encounter",
  event: "fateful",
  "fateful-extra": "fateful",
  "fateful-missing": "fateful",
  pidiv: "pidiv",
  "pid-gen5": "pidGen5",
  "pid-transfer": "transfer",
  "pid-ec": "ec",
  "shiny-lock": "shinyLock",
  "shiny-grotto": "shinyLock",
  "relearn-transfer": "relearn",
  "relearn-egg": "relearn",
  "relearn-fixed": "relearn",
  "relearn-extra": "relearn",
  "hatch-level": "eggOrigin",
  "egg-location": "eggOrigin",
  "egg-loc": "eggOrigin",
  "ball-egg": "eggOrigin",
  "origin-old": "transfer",
  "transfer-location": "transfer",
  "ability-hidden": "hiddenAbility",
  "ability-hidden-required": "hiddenAbility",
  "iv-flawless": "flawlessIvs",
  "move-illegal": "learnset",
  "move-unverified": "learnset",
  "ev-untrained": "ev",
  "vc-species": "virtualConsole",
  "vc-language": "virtualConsole",
  "vc-contest": "virtualConsole",
  "vc-ivs": "virtualConsole",
  "event-none": "fateful",
  "event-language": "fateful",
  "event-ec": "ec",
};

/** Rapports en cache, par Pokémon (clé = données affichées : toute modification invalide). */
const cache = reactive(new Map<string, LegalityReport>());
const pending = new Set<string>();

export function reportKey(p: SlotView) {
  return JSON.stringify(p);
}

/** Rapport en cache ; lance l'analyse s'il manque (le résultat arrive de façon réactive). */
export function cachedReport(p: SlotView): LegalityReport | null {
  const key = reportKey(p);
  const hit = cache.get(key);
  if (hit) return hit;
  if (!pending.has(key)) {
    pending.add(key);
    invoke<LegalityReport>("legality_check", { slot: p.slot })
      .then((r) => {
        if (cache.size > 400) cache.clear();
        cache.set(key, r);
      })
      .catch(() => undefined)
      .finally(() => pending.delete(key));
  }
  return null;
}

export function clearReports() {
  cache.clear();
}

/** Après une modification qui renvoie l'état général. */
async function refreshAfter(view: SaveView | null) {
  if (view) saveState.view = view;
  else {
    const v = await invoke<SaveView>("save_view").catch(() => null);
    if (v) saveState.view = v;
  }
  saveState.dirty = true;
  await loadBox(saveState.box);
}

/** « Rendre légal » un emplacement ; met à jour la sélection. */
export async function legalizeSlot(slot: Slot): Promise<LegalizeResult | null> {
  saveState.error = null;
  try {
    const r = await invoke<LegalizeResult>("legality_legalize", { slot });
    await refreshAfter(null);
    saveState.selected = r.view;
    if (!r.changes.length) notify(r.success ? "Déjà légal : rien à changer" : "Impossible de le rendre légal automatiquement");
    else notify(r.success ? `Rendu légal (${r.changes.length} modification${r.changes.length > 1 ? "s" : ""}) · Ctrl+Z pour annuler` : "Corrigé en partie : il reste des problèmes");
    return r;
  } catch (e) {
    saveState.error = String(e);
    return null;
  }
}

/** « Tout rendre légal » (une seule étape d'annulation). */
export async function legalizeAll(slots?: Slot[]) {
  saveState.error = null;
  try {
    const r = await invoke<{ view: SaveView; fixed: number; failed: number; untouched: number }>("legality_legalize_all", { slots: slots ?? null });
    await refreshAfter(r.view);
    clearReports();
    notify(`${r.fixed} Pokémon rendu${r.fixed > 1 ? "s" : ""} légal${r.fixed > 1 ? "ux" : ""}${r.failed ? ` · ${r.failed} impossible${r.failed > 1 ? "s" : ""} à corriger` : ""} · Ctrl+Z pour annuler`);
    return r;
  } catch (e) {
    saveState.error = String(e);
    return null;
  }
}

/** Crée un Pokémon légal (dans `slot` ou le premier emplacement libre à partir de la boîte affichée). */
export async function generateLegal(request: GenerateRequest, slot: Slot | null = null) {
  saveState.error = null;
  try {
    const r = await invoke<LegalizeResult>("legality_generate", { slot, fromBox: saveState.box, request });
    await refreshAfter(null);
    const where = r.view.slot.kind === "party" ? `équipe, place ${r.view.slot.index + 1}` : `${saveState.view?.boxNames[r.view.slot.box] ?? "boîte"}, case ${r.view.slot.index + 1}`;
    notify(`${r.view.speciesName} créé (${where})${r.success ? "" : " — à vérifier"}`);
    return r;
  } catch (e) {
    saveState.error = String(e);
    return null;
  }
}

export const FAMILIES: { id: string; label: string; color: string }[] = [
  { id: "herbes", label: "Herbes", color: "#4ade80" },
  { id: "surf", label: "Surf", color: "#60a5fa" },
  { id: "peche", label: "Pêche", color: "#38bdf8" },
  { id: "special", label: "Spécial", color: "#c084fc" },
  { id: "fixe", label: "Fixe", color: "#f87171" },
  { id: "don", label: "Don", color: "#fbbf24" },
  { id: "oeuf", label: "Œuf", color: "#fde68a" },
  { id: "echange", label: "Échange", color: "#f472b6" },
  { id: "evenement", label: "Événement", color: "#a78bfa" },
];

export const familyOf = (id: string) => FAMILIES.find((f) => f.id === id);
