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

/** Une modification de « Rendre légal », reliée à un terme du glossaire. */
export interface LegalityChange {
  text: string;
  term: string;
}

/** Rencontre retenue ou proposée par « Rendre légal ». */
export interface EncounterOption {
  id: number;
  kindLabel: string;
  family: string;
  species: number;
  speciesName: string;
  location: number;
  locationName: string;
  levelMin: number;
  levelMax: number;
  version: number;
  versionName: string;
  generation: number;
}

export interface LegalizeResult {
  view: SlotView;
  changes: LegalityChange[];
  report: LegalityReport;
  success: boolean;
  encounter: EncounterOption | null;
}

/** Aperçu de « Rendre légal » (rien n'est écrit). */
export interface LegalizePreview {
  before: SlotView;
  after: SlotView;
  changes: LegalityChange[];
  report: LegalityReport;
  success: boolean;
  /** `null` : rencontre actuelle gardée. */
  encounter: EncounterOption | null;
  options: EncounterOption[];
}

/** Une ligne de l'aperçu groupé. */
export interface GroupItem {
  slot: Slot;
  before: SlotView;
  after: SlotView;
  changes: LegalityChange[];
  success: boolean;
  encounter: EncounterOption | null;
}

export interface LegalizeAllResult {
  view: SaveView | null;
  fixed: number;
  failed: number;
  untouched: number;
  items: GroupItem[];
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

/** Aperçu de « Rendre légal » (rien n'est écrit). `choice` : `id` d'une rencontre proposée. */
export function previewLegalize(slot: Slot, choice: number | null = null) {
  return invoke<LegalizePreview>("legality_legalize_preview", { slot, choice });
}

/** Applique « Rendre légal » (même résultat que l'aperçu) ; met à jour la sélection. */
export async function applyLegalize(slot: Slot, choice: number | null = null): Promise<LegalizeResult | null> {
  saveState.error = null;
  try {
    const r = await invoke<LegalizeResult>("legality_legalize_apply", { slot, choice });
    await refreshAfter(null);
    clearReports();
    saveState.selected = r.view;
    if (!r.changes.length) notify(r.success ? "Déjà légal : rien à changer" : "Impossible de le rendre légal automatiquement");
    else notify(r.success ? `Rendu légal (${r.changes.length} modification${r.changes.length > 1 ? "s" : ""}) · Ctrl+Z pour annuler` : "Corrigé en partie : il reste des problèmes");
    return r;
  } catch (e) {
    saveState.error = String(e);
    return null;
  }
}

/** « Rendre légal » sans aperçu. */
export function legalizeSlot(slot: Slot) {
  return applyLegalize(slot, null);
}

/** Aperçu groupé de « Tout rendre légal » (rien n'est écrit). */
export function previewLegalizeAll(slots?: Slot[]) {
  return invoke<LegalizeAllResult>("legality_legalize_all", { slots: slots ?? null, preview: true });
}

/** « Tout rendre légal » (une seule étape d'annulation). */
export async function legalizeAll(slots?: Slot[]) {
  saveState.error = null;
  try {
    const r = await invoke<LegalizeAllResult>("legality_legalize_all", { slots: slots ?? null, preview: false });
    await refreshAfter(r.view);
    clearReports();
    notify(`${r.fixed} Pokémon rendu${r.fixed > 1 ? "s" : ""} légal${r.fixed > 1 ? "ux" : ""}${r.failed ? ` · ${r.failed} impossible${r.failed > 1 ? "s" : ""} à corriger` : ""} · Ctrl+Z pour annuler`);
    return r;
  } catch (e) {
    saveState.error = String(e);
    return null;
  }
}

/** Ligne du tableau avant / après. */
export interface DiffRow {
  label: string;
  term: string;
  before: string;
  after: string;
  changed: boolean;
}

const dateText = (d: { year: number; month: number; day: number } | null) =>
  d ? `${String(d.day).padStart(2, "0")}/${String(d.month).padStart(2, "0")}/${d.year}` : "—";
const hex = (n: number) => (n >>> 0).toString(16).toUpperCase().padStart(8, "0");
const yesNo = (b: boolean) => (b ? "Oui" : "Non");

/**
 * Champs comparés par l'aperçu, avec leur terme du glossaire. `names` : noms des Balls,
 * des versions et des langues (fournis par la page, pour ne pas dépendre des listes ici).
 */
export function diffRows(
  a: SlotView,
  b: SlotView,
  names: { ball: (id: number) => string; version: (id: number) => string; language: (id: number) => string; gender: (g: SlotView["gender"]) => string },
): DiffRow[] {
  const ribbons = (p: SlotView) => p.extras?.ribbons?.filter((r) => r.on).length ?? 0;
  const fields: [string, string, (p: SlotView) => string][] = [
    ["Forme", "form", (p) => p.speciesData?.formName || "normale"],
    ["Surnom", "nickname", (p) => (p.isNicknamed ? p.nickname : "—")],
    ["Niveau", "level", (p) => `N. ${p.level}`],
    ["Jeu d'origine", "version", (p) => names.version(p.version)],
    ["Lieu de rencontre", "metLocation", (p) => p.metLocationName ?? String(p.metLocation)],
    ["Niveau de rencontre", "metLevel", (p) => String(p.metLevel)],
    ["Date de rencontre", "metDate", (p) => dateText(p.metDate)],
    ["Lieu de l'œuf", "eggLocation", (p) => (p.eggLocation ? (p.eggLocationName ?? String(p.eggLocation)) : "—")],
    ["Ball", "ball", (p) => names.ball(p.ball)],
    ["Rencontre fatidique", "fateful", (p) => yesNo(p.fatefulEncounter)],
    ["Dresseur d'origine", "ot", (p) => `${p.otName} (ID ${p.tid})`],
    ["Langue", "language", (p) => names.language(p.language)],
    ["Talent", "ability", (p) => p.abilityName],
    ["Nature", "nature", (p) => p.natureName],
    ["Sexe", "gender", (p) => names.gender(p.gender)],
    ["Chromatique", "shiny", (p) => yesNo(p.shiny)],
    ["PID", "pid", (p) => hex(p.pid)],
    ["Constante de chiffrement", "ec", (p) => hex(p.encryptionConstant)],
    ["IV", "iv", (p) => p.ivs.join(" / ")],
    ["EV", "ev", (p) => p.evs.join(" / ")],
    ["Attaques", "moves", (p) => p.moveNames.filter(Boolean).join(", ") || "—"],
    ["Objet tenu", "heldItem", (p) => p.itemName ?? "—"],
    ["Rubans", "ribbons", (p) => String(ribbons(p))],
    ["Concours", "contest", (p) => (p.extras?.contest ?? []).join(" / ")],
  ];
  return fields.map(([label, term, f]) => {
    const before = f(a);
    const after = f(b);
    return { label, term, before, after, changed: before !== after };
  });
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
