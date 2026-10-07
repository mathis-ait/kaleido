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
  map: number;
  party: LiveMon[];
  boxes: { name: string; mons: LiveBoxMon[] }[];
}

export interface JournalEntry {
  at: number;
  playSeconds: number;
  place: string | null;
  kind: "caught" | "hatched" | "evolved" | "levelUp" | "fainted" | "revived" | "died" | "gone" | "badge";
  text: string;
  key: string | null;
  species: number | null;
}

export interface NuzlockeSummary {
  captures: number;
  routes: number;
  routesCaught: number;
  alive: number;
  dead: number;
  levelCap: number | null;
  next: { label: string; name: string; town: string; aceSpecies: number; aceLevel: number } | null;
  here: { key: string; name: string; status: "pending" | "caught" | "missed" | "dupeOnly"; capture: string | null; markedMissed: boolean } | null;
  warnings: { error: boolean; title: string; detail: string }[];
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
  /** Lieu de la dernière sauvegarde (quand la ROM est connue). */
  place: string | null;
  /** Nouveautés depuis la lecture précédente. */
  events: string[];
  journal: JournalEntry[];
  nuzlocke: NuzlockeSummary | null;
  canTrack: boolean;
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

function apply(s: CompanionState | null, announce: boolean) {
  companion.state = s;
  companion.loaded = true;
  if (!s?.snapshot) return;
  if (announce) {
    if (s.events.length) s.events.slice(0, 4).forEach(notice);
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

/** Active le suivi Nuzlocke avec la ROM lancée (règles par défaut, modifiables dans l'éditeur). */
export async function trackNuzlocke() {
  try {
    await invoke("companion_track_nuzlocke");
  } catch (e) {
    notice(String(e));
  }
}

/** « Rencontre ratée ici » : fuite ou K.O. du sauvage, rien de visible dans la sauvegarde. */
export async function markMissed(route: string, missed: boolean) {
  try {
    await invoke("companion_mark_missed", { route, missed });
  } catch (e) {
    notice(String(e));
  }
}
