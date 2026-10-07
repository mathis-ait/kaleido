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
  kind: "caught" | "hatched" | "evolved" | "levelUp" | "fainted" | "revived" | "died" | "gone" | "badge" | "encounter";
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
  unassigned: number;
}

export interface NextBattle {
  label: string;
  name: string;
  town: string;
  trainerId: number;
  team: {
    species: number;
    form: number;
    name: string;
    level: number;
    counter: { name: string; species: number; verdict: "win" | "uncertain" | "lose" | "none"; moveName: string | null } | null;
  }[];
}

/** Lecture de la mémoire de l'émulateur (compagnon en direct). */
export interface LiveInfo {
  /** `live` : mémoire lue ; `ingame` : émulateur lancé, mémoire non lue ; `offline` : aucun émulateur. */
  status: "live" | "ingame" | "offline";
  enabled: boolean;
  emulator: string | null;
  game: string | null;
  /** Lecture vérifiée en jeu pour ce jeu (sinon « non vérifiée »). */
  verified: boolean;
  detail: string | null;
}

/** Pokémon adverse vu en mémoire. */
export interface LiveFoe {
  species: number;
  form: number;
  speciesName: string;
  level: number;
  hp: number;
  maxHp: number;
  shiny: boolean;
  gender: Gender;
  status: StatusCondition;
  pid: number;
}

export interface BattleView {
  wild: boolean;
  foes: LiveFoe[];
}

export type EncounterOutcome = "caught" | "missed" | "fled";

/** Rencontre sauvage vue en mémoire, avec son issue. */
export interface Encounter {
  id: number;
  species: number;
  form: number;
  speciesName: string;
  level: number;
  shiny: boolean;
  place: string | null;
  route: string | null;
  outcome: EncounterOutcome | null;
  at: number;
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
  nextBattle: NextBattle | null;
  /** Origine de l'équipe affichée : sauvegarde ou mémoire de l'émulateur. */
  source: "file" | "memory";
  live: LiveInfo | null;
  battle: BattleView | null;
  encounter: Encounter | null;
  /** Déclencheur de l'envoi : sauvegarde relue, ou lecture de la mémoire. */
  reason: "save" | "memory";
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
const recent = new Map<string, number>();
function notice(text: string) {
  // Même annonce vue en mémoire puis à la sauvegarde suivante : une seule fois par minute.
  const seen = recent.get(text);
  if (seen !== undefined && Date.now() - seen < 60_000 && text !== "Sauvegarde lue") return;
  recent.set(text, Date.now());
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
    else if (s.reason === "save") notice("Sauvegarde lue");
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

/** Active ou coupe la lecture de la mémoire de l'émulateur. */
export async function setMemory(on: boolean) {
  if (companion.state?.live) companion.state.live.enabled = on;
  try {
    await invoke("companion_set_memory", { on });
  } catch (e) {
    notice(String(e));
  }
}

/** Issue d'une rencontre : Capturé, Raté (K.O. du sauvage) ou Fui. */
export async function encounterOutcome(id: number, outcome: EncounterOutcome) {
  try {
    await invoke("companion_encounter_outcome", { id, outcome });
  } catch (e) {
    notice(String(e));
  }
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

/** Ouvre la sauvegarde dans la fenêtre principale : page Combat sur ce dresseur, ou l'éditeur. */
export async function openInMain(trainer: number | null = null, page: string | null = null) {
  try {
    await invoke("companion_open_in_main", { trainer, page });
  } catch (e) {
    notice(String(e));
  }
}
