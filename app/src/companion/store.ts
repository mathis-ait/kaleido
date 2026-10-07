import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Gender, Slot } from "../types";

export interface PlayTimeView {
  hours: number;
  minutes: number;
  seconds: number;
}

/** Problème de statut d'un Pokémon de l'équipe. */
export type StatusCondition = "none" | "sleep" | "poison" | "burn" | "freeze" | "paralysis" | "badPoison";

export interface LiveMon {
  slot: Slot;
  species: number;
  form: number;
  speciesName: string;
  nickname: string;
  isNicknamed: boolean;
  isEgg: boolean;
  shiny: boolean;
  gender: Gender;
  level: number;
  hp: number;
  maxHp: number;
  status: StatusCondition;
  itemName: string | null;
  abilityName: string;
  natureName: string;
  moveNames: string[];
  metLocationName: string | null;
  uid: string;
}

export interface LiveBoxMon {
  index: number;
  species: number;
  form: number;
  speciesName: string;
  nickname: string;
  isEgg: boolean;
  shiny: boolean;
  level: number;
  uid: string;
}

export interface LiveSnapshot {
  game: string;
  version: string;
  generation: number;
  trainer: { name: string; displayId: number };
  playTime: PlayTimeView;
  badges: number | null;
  party: LiveMon[];
  boxes: { name: string; mons: LiveBoxMon[] }[];
}

export interface CompanionState {
  path: string;
  title: string;
  key: string | null;
  exists: boolean;
  modified: number | null;
  snapshot: LiveSnapshot | null;
  error: string | null;
  onTop: boolean;
  autoOpen: boolean;
}

/** Évènement discret affiché quelques secondes (nouvelle capture, montée de niveau…). */
export interface CompanionNotice {
  id: number;
  text: string;
}

export const companion = reactive({
  loaded: false,
  state: null as CompanionState | null,
  /** Dernier instantané lu correctement : gardé si une lecture échoue. */
  snapshot: null as LiveSnapshot | null,
  /** Heure de la dernière lecture réussie (ms). */
  readAt: null as number | null,
  running: [] as string[],
  notices: [] as CompanionNotice[],
  compact: false,
});

let noticeId = 0;
function notice(text: string) {
  const id = ++noticeId;
  companion.notices.push({ id, text });
  window.setTimeout(() => {
    companion.notices = companion.notices.filter((n) => n.id !== id);
  }, 6000);
}

export const monName = (m: { nickname: string; speciesName: string; isEgg: boolean }) => (m.isEgg ? "Œuf" : m.nickname || m.speciesName);

/** Ce qui a changé entre deux sauvegardes, en phrases courtes (le journal complet viendra avec le suivi Nuzlocke). */
export function changes(before: LiveSnapshot, after: LiveSnapshot): string[] {
  const out: string[] = [];
  const all = (s: LiveSnapshot) => [...s.party, ...s.boxes.flatMap((b) => b.mons)];
  const old = new Map(all(before).map((m) => [m.uid, m]));
  for (const m of all(after)) {
    const prev = old.get(m.uid);
    if (!prev) out.push(m.isEgg ? "Nouvel œuf" : `Nouveau Pokémon : ${monName(m)} (N. ${m.level})`);
    else if (prev.isEgg && !m.isEgg) out.push(`${m.speciesName} est sorti de l'œuf`);
    else if (prev.species !== m.species) out.push(`${prev.speciesName} a évolué en ${m.speciesName}`);
    else if (m.level > prev.level && after.party.some((p) => p.uid === m.uid)) out.push(`${monName(m)} passe au N. ${m.level}`);
  }
  if ((after.badges ?? 0) !== (before.badges ?? 0) && after.badges !== null) {
    const count = (b: number) => [...b.toString(2)].filter((c) => c === "1").length;
    const n = count(after.badges) - count(before.badges ?? 0);
    if (n > 0) out.push(after.generation === 7 ? "Nouvelle épreuve terminée" : "Nouveau badge !");
  }
  return out;
}

function apply(s: CompanionState | null, announce: boolean) {
  companion.state = s;
  companion.loaded = true;
  if (!s?.snapshot) return;
  if (announce && companion.snapshot) {
    const found = changes(companion.snapshot, s.snapshot);
    if (found.length) found.slice(0, 4).forEach(notice);
    else notice("Sauvegarde lue");
  }
  companion.snapshot = s.snapshot;
  companion.readAt = Date.now();
}

export async function refresh() {
  apply(await invoke<CompanionState | null>("companion_state"), false);
}

export async function setOnTop(on: boolean) {
  if (companion.state) companion.state.onTop = on;
  await invoke("companion_set_on_top", { on });
}

export async function setAutoOpen(on: boolean) {
  const key = companion.state?.key;
  if (!key || !companion.state) return;
  companion.state.autoOpen = on;
  await invoke("companion_set_auto_open", { key, on });
}

export async function setCompact(on: boolean) {
  companion.compact = on;
  await invoke("companion_set_compact", { compact: on });
}

export function initCompanion() {
  void refresh();
  listen<CompanionState>("companion-update", (e) => apply(e.payload, true)).catch(() => undefined);
  invoke<string[]>("emulators_running")
    .then((r) => (companion.running = r))
    .catch(() => undefined);
  listen<string[]>("emulators-running", (e) => (companion.running = e.payload)).catch(() => undefined);
}
