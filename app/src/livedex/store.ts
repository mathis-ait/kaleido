import { computed, reactive, shallowRef, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { knownSaves } from "../save/knownSaves";
import { ACHIEVEMENTS } from "./achievements";
import { dex, loadDex, SAVE_VERSION_GAME, today } from "./data";
import { computeCollection, DEFAULT_RULES, type Collection } from "./slots";
import type { CatchEntry, DexRules, LivedexState, ManualEntry, ScannedSource } from "./types";

/** Fichiers qui ressemblent à une sauvegarde : un échec de lecture mérite d'être signalé. */
const SAVE_LIKE = /\.(sav|dsv|main|bin|srm)$|[\\/]main$/i;

function emptyState(): LivedexState {
  return { version: 1, rules: { ...DEFAULT_RULES }, disabledSources: [], bank: true, legalOnly: false, setupDone: false, manual: [], firstSeen: {}, achievements: {} };
}

export const livedex = reactive({
  ready: false,
  error: null as string | null,
  scanning: false,
  /** Heure du dernier scan (ms). */
  scannedAt: 0,
  /** Sauvegardes lues (y compris celles écartées par l'utilisateur), plus la banque. */
  sources: [] as ScannedSource[],
  /** Fichiers qui ressemblent à des sauvegardes mais n'ont pas pu être lus. */
  unreadable: [] as { path: string; error: string }[],
  /** Émulateur qui utilise chaque sauvegarde détectée chez lui. */
  emulatorOf: {} as Record<string, string>,
  saved: emptyState(),
  /** Succès débloqués pendant la session, à annoncer. */
  unlocked: [] as string[],
  /** Dernière suppression, pour « Annuler ». */
  lastRemoved: null as ManualEntry[] | null,
});

let started: Promise<void> | null = null;

/** Charge les données et l'état enregistré, puis lit les sauvegardes si l'installation est faite. */
export function initLivedex(): Promise<void> {
  started ??= (async () => {
    try {
      const [, saved] = await Promise.all([loadDex(), invoke<LivedexState | null>("livedex_load")]);
      if (saved) livedex.saved = { ...emptyState(), ...saved, rules: { ...DEFAULT_RULES, ...saved.rules } };
      livedex.ready = true;
      await scan();
    } catch (e) {
      livedex.error = String(e);
      started = null;
    }
  })();
  return started;
}

/** Relit toutes les sauvegardes connues (et la banque). */
export async function scan(): Promise<void> {
  if (livedex.scanning) return;
  livedex.scanning = true;
  try {
    const known = await knownSaves();
    livedex.emulatorOf = known.emulatorOf;
    const result = await invoke<ScannedSource[]>("livedex_scan", { paths: known.paths, bank: livedex.saved.bank });
    livedex.sources = result.filter((s) => !s.error).sort((a, b) => a.generation - b.generation || a.game.localeCompare(b.game));
    livedex.unreadable = result.filter((s) => s.error && s.path !== "bank" && SAVE_LIKE.test(s.path)).map((s) => ({ path: s.path, error: s.error ?? "" }));
    livedex.scannedAt = Date.now();
    // Date de première détection de chaque Pokémon (sert de date de « capture » dans le journal).
    const day = today();
    let changed = false;
    for (const s of livedex.sources) {
      for (const p of s.specimens) {
        if (!livedex.saved.firstSeen[p.key]) {
          livedex.saved.firstSeen[p.key] = day;
          changed = true;
        }
      }
    }
    if (changed) persist();
  } catch (e) {
    livedex.error = String(e);
  } finally {
    livedex.scanning = false;
  }
}

/** Relit les sauvegardes si le dernier scan date de plus de `ms`. */
export function refreshIfStale(ms = 30_000) {
  if (livedex.ready && !livedex.scanning && Date.now() - livedex.scannedAt > ms) void scan();
}

// --- Enregistrement (regroupé : une écriture au plus toutes les 400 ms).
let timer: number | undefined;
export function persist() {
  clearTimeout(timer);
  timer = window.setTimeout(() => {
    invoke("livedex_store", { data: livedex.saved, today: today() }).catch((e) => (livedex.error = `Enregistrement impossible : ${e}`));
  }, 400);
}

const sourceLabel = (s: ScannedSource) => (s.path === "bank" ? "Banque Kaleido" : s.trainer ? `${s.game} · ${s.trainer}` : s.game);

/** Pokémon lus (sources actives, sans doublon) puis notés à la main. */
export const entries = computed<CatchEntry[]>(() => {
  const d = dex.value;
  if (!d) return [];
  const st = livedex.saved;
  const byKey = new Map<string, CatchEntry>();
  for (const s of livedex.sources) {
    if (st.disabledSources.includes(s.path)) continue;
    if (s.path === "bank" && !st.bank) continue;
    for (const p of s.specimens) {
      if (st.legalOnly && p.legality === "illegal") continue;
      const where = { source: s.path, label: sourceLabel(s), place: p.place };
      const found = byKey.get(p.key);
      if (found) {
        found.where?.push(where);
        continue;
      }
      const game = d.gameByVersion(p.version)?.id ?? (s.version ? SAVE_VERSION_GAME[s.version] : undefined) ?? null;
      byKey.set(p.key, {
        id: `auto:${p.key}`,
        auto: true,
        species: p.species,
        form: p.form,
        gender: p.gender === "female" ? "f" : p.gender === "male" ? "m" : "n",
        shiny: p.shiny,
        game,
        location: p.metLocation ?? undefined,
        ball: p.ball || undefined,
        level: p.level,
        date: st.firstSeen[p.key],
        nickname: p.nickname ?? undefined,
        ot: p.otName,
        where: [where],
        legality: p.legality,
      });
    }
  }
  const manual: CatchEntry[] = st.manual.map((m) => ({ ...m, auto: false }));
  return [...byKey.values(), ...manual];
});

export const collection = computed<Collection | null>(() => (dex.value ? computeCollection(dex.value, entries.value, livedex.saved.rules) : null));

// --- Succès : vérifiés à chaque changement de la collection.
const announced = shallowRef(new Set<string>());
watch([collection, () => livedex.scanning], ([c]) => {
  const d = dex.value;
  if (!c || !d || !livedex.ready || livedex.scanning) return;
  const ctx = { dex: d, collection: c, entries: entries.value };
  const day = today();
  let changed = false;
  for (const a of ACHIEVEMENTS) {
    if (livedex.saved.achievements[a.id]) continue;
    const [cur, goal] = a.progress(ctx);
    if (goal > 0 && cur >= goal) {
      livedex.saved.achievements[a.id] = day;
      changed = true;
      // Les succès déjà mérités au premier lancement ne sont pas annoncés un par un.
      if (livedex.saved.setupDone && !announced.value.has(a.id)) livedex.unlocked.push(a.id);
      announced.value.add(a.id);
    }
  }
  if (changed) persist();
});

// --- Actions.
export function setRules(rules: DexRules) {
  livedex.saved.rules = { ...rules };
  persist();
}

export function toggleSource(path: string, on: boolean) {
  const list = livedex.saved.disabledSources.filter((p) => p !== path);
  if (!on) list.push(path);
  livedex.saved.disabledSources = list;
  persist();
}

export function setBank(on: boolean) {
  livedex.saved.bank = on;
  persist();
  void scan();
}

export function setLegalOnly(on: boolean) {
  livedex.saved.legalOnly = on;
  persist();
}

export function completeSetup() {
  livedex.saved.setupDone = true;
  persist();
}

export function saveManual(entry: Omit<ManualEntry, "id" | "createdAt"> & { id?: string }) {
  const list = livedex.saved.manual;
  if (entry.id) {
    const i = list.findIndex((m) => m.id === entry.id);
    if (i >= 0) list[i] = { ...list[i], ...entry, id: entry.id };
  } else {
    list.push({ ...entry, id: crypto.randomUUID(), createdAt: new Date().toISOString() });
  }
  persist();
}

/** Entrée enregistrée d'un Pokémon noté à la main (pour la modifier). */
export const manualOf = (id: string) => livedex.saved.manual.find((m) => m.id === id);

/** Supprime des Pokémon notés à la main (les Pokémon lus viennent des sauvegardes). */
export function removeManual(ids: string[]) {
  const set = new Set(ids);
  livedex.lastRemoved = livedex.saved.manual.filter((m) => set.has(m.id));
  livedex.saved.manual = livedex.saved.manual.filter((m) => !set.has(m.id));
  persist();
}

export function undoRemove() {
  if (!livedex.lastRemoved) return;
  livedex.saved.manual.push(...livedex.lastRemoved);
  livedex.lastRemoved = null;
  persist();
}
