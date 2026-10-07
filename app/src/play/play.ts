import { computed, reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
import { library } from "../library";
import { loadBox, notify, openSave, sameSlot, saveState, writeSave } from "../saveStore";
import type { SaveView, SlotView } from "../types";

// Miroir des types de `play.rs`.

export type EmulatorId = "melonds" | "desmume" | "azahar" | "citra" | "lime3ds" | "eden";
export type PlayPlatform = "nds" | "3ds" | "switch";

export interface EmulatorInfo {
  id: EmulatorId;
  name: string;
  platform: PlayPlatform;
  exe: string | null;
  detected: boolean;
  missing: boolean;
  version: string | null;
  /** Dossier des sauvegardes DS (null : à côté de la ROM) ou dossier utilisateur 3DS. */
  dataDir: string | null;
  dataDirExists: boolean;
}

export interface EmulatorProfile {
  exe?: string | null;
  saveDir?: string | null;
  userDir?: string | null;
}

export interface PlayConfig {
  profiles: Partial<Record<EmulatorId, EmulatorProfile>>;
  searchDirs: string[];
  preferredNds: EmulatorId | null;
  preferredCtr: EmulatorId | null;
}

interface PlayRequest {
  emulator: EmulatorId;
  rom: string | null;
  modRomfs?: string | null;
  save?: string | null;
  replaceMod?: boolean;
  trackKey?: string | null;
}

export interface PlayPlan {
  emulator: string;
  exe: string | null;
  savePath: string | null;
  saveExists: boolean;
  saveConflict: boolean;
  titleId: string | null;
  modPath: string | null;
  modExists: boolean;
  needsGame: boolean;
  warnings: string[];
}

export interface PlayResult {
  emulator: string;
  savePath: string | null;
  backups: string[];
  modPath: string | null;
  warnings: string[];
}

/** Partie en cours : jeu lancé depuis Kaleido et sauvegarde suivie. */
export interface PlaySession {
  platform: PlayPlatform;
  emulator: EmulatorId;
  emulatorName: string;
  rom: string | null;
  modRomfs: string | null;
  savePath: string | null;
}

export const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

// --- Profils d'émulateurs

export const emus = reactive({
  loaded: false,
  loading: false,
  list: [] as EmulatorInfo[],
  config: { profiles: {}, searchDirs: [], preferredNds: null, preferredCtr: null } as PlayConfig,
  error: null as string | null,
});

export interface EmulatorsState {
  emulators: EmulatorInfo[];
  config: PlayConfig;
}

export function applyState(s: EmulatorsState) {
  emus.list = s.emulators;
  emus.config = s.config;
  emus.loaded = true;
}

export async function loadEmulators() {
  emus.loading = true;
  emus.error = null;
  try {
    applyState(await invoke<EmulatorsState>("emulators_list"));
  } catch (e) {
    emus.error = String(e);
  } finally {
    emus.loading = false;
  }
}

/** Modifie une copie des réglages, l'enregistre et relit l'état. */
export async function updateConfig(change: (c: PlayConfig) => void) {
  const config: PlayConfig = JSON.parse(JSON.stringify(emus.config));
  change(config);
  emus.error = null;
  try {
    applyState(await invoke<EmulatorsState>("emulators_save", { config }));
  } catch (e) {
    emus.error = String(e);
  }
}

export const available = (platform: PlayPlatform) => emus.list.filter((e) => e.platform === platform && e.exe);

/** Émulateur par défaut pour une plateforme (préféré s'il est trouvé, sinon le premier trouvé). */
export function defaultEmulator(platform: PlayPlatform): EmulatorInfo | null {
  const list = available(platform);
  const preferred = platform === "nds" ? emus.config.preferredNds : platform === "3ds" ? emus.config.preferredCtr : null;
  return list.find((e) => e.id === preferred) ?? list[0] ?? null;
}

// --- Installation d'un émulateur

export interface EmulatorDownload {
  id: EmulatorId;
  name: string;
  version: string;
  file: string;
  size: number;
  page: string;
}

/** Émulateur installé quand on veut jouer sans en avoir. */
export const RECOMMENDED: Record<PlayPlatform, EmulatorId> = { nds: "melonds", "3ds": "azahar", switch: "eden" };
/** Émulateurs que Kaleido sait télécharger et installer. */
export const INSTALLABLE: EmulatorId[] = ["melonds", "azahar", "desmume", "eden"];

export const PLATFORM_LABEL: Record<PlayPlatform, string> = { nds: "Nintendo DS", "3ds": "Nintendo 3DS", switch: "Nintendo Switch" };
export const PLATFORM_SHORT: Record<PlayPlatform, string> = { nds: "DS", "3ds": "3DS", switch: "Switch" };

export interface InstallProgress {
  step: "info" | "download" | "extract";
  done: number;
  total: number;
}

/** Installations en cours. */
export const installs = reactive<Partial<Record<EmulatorId, InstallProgress>>>({});

listen<InstallProgress & { id: EmulatorId }>("emulator-install", (e) => {
  const { id, ...progress } = e.payload;
  if (installs[id]) installs[id] = progress;
}).catch(() => undefined);

export const formatMo = (bytes: number) => `${Math.max(1, Math.round(bytes / 1048576))} Mo`;

/** Propose puis installe un émulateur (dernière version officielle). Renvoie true s'il est installé. */
export async function installEmulator(id: EmulatorId, reason?: string): Promise<boolean> {
  if (installs[id]) return false;
  installs[id] = { step: "info", done: 0, total: 0 };
  try {
    const info = await invoke<EmulatorDownload>("emulator_download_info", { id });
    const ok = await ask(
      `${reason ? reason + "\n\n" : ""}Kaleido peut télécharger ${info.name} ${info.version} (${formatMo(info.size)}) depuis sa page officielle et l'installer dans son propre dossier. Rien d'autre n'est modifié sur ton PC.`,
      { title: `Installer ${info.name}`, okLabel: "Télécharger et installer", cancelLabel: "Annuler" },
    );
    if (!ok) return false;
    installs[id] = { step: "download", done: 0, total: info.size };
    applyState(await invoke<EmulatorsState>("emulator_install", { id }));
    return true;
  } catch (e) {
    await message(String(e), { title: "Installation impossible", kind: "error" });
    return false;
  } finally {
    delete installs[id];
  }
}

/** « Localiser… » : l'utilisateur choisit lui-même l'exécutable d'un émulateur. */
export async function locateEmulator(id: EmulatorId, name: string) {
  const exe = await open({ title: `Où se trouve ${name} ?`, filters: [{ name: `${name} (programme)`, extensions: ["exe"] }] });
  if (typeof exe !== "string") return false;
  try {
    applyState(await invoke<EmulatorsState>("emulator_locate", { id, exe }));
    return true;
  } catch (e) {
    await message(String(e), { title: "Émulateur non enregistré", kind: "error" });
    return false;
  }
}

// --- Guide de première utilisation

const GUIDE_KEY = "kaleido.play.guideSeen";

export const guide = reactive({ open: false, next: null as (() => void) | null });

function guideSeen() {
  try {
    return localStorage.getItem(GUIDE_KEY) === "1";
  } catch {
    return true;
  }
}

export function showGuide(next: (() => void) | null = null) {
  guide.next = next;
  guide.open = true;
}

export function closeGuide(proceed: boolean) {
  try {
    localStorage.setItem(GUIDE_KEY, "1");
  } catch {
    /* stockage indisponible */
  }
  guide.open = false;
  const next = guide.next;
  guide.next = null;
  if (proceed) next?.();
}

// --- Synchro en direct

export type SyncStatus = "idle" | "watching" | "changed" | "reloaded";

export const live = reactive({
  session: null as PlaySession | null,
  /** Fichier surveillé. */
  path: null as string | null,
  status: "idle" as SyncStatus,
  /** Heure du dernier évènement (ms). */
  at: 0,
  /** Le jeu a sauvegardé mais l'éditeur ne peut pas recharger tout seul. */
  pending: null as string | null,
});

const watchTarget = computed(() => live.session?.savePath ?? saveState.path);

watch(
  watchTarget,
  async (path) => {
    live.pending = null;
    live.path = path;
    live.status = path ? "watching" : "idle";
    await invoke(path ? "watch_save" : "unwatch_save", path ? { path } : {}).catch(() => (live.status = "idle"));
  },
  { immediate: true },
);

// Après un enregistrement fait par Kaleido, le fichier a changé : ce n'est pas le jeu.
watch(
  () => saveState.dirty,
  (dirty, was) => {
    if (was && !dirty) resync();
  },
);

export const resync = () => invoke("watch_save_resync").catch(() => undefined);

listen<{ path: string }>("save-changed", (e) => onGameSaved(e.payload.path)).catch(() => undefined);

async function onGameSaved(path: string) {
  if (path !== live.path) return;
  live.at = Date.now();
  live.status = "changed";
  if (!saveState.view) return;
  if (saveState.path === path && !saveState.dirty) {
    if (await reloadFrom(path)) {
      live.status = "reloaded";
      notify("Le jeu a sauvegardé : sauvegarde rechargée");
    }
  } else {
    live.pending = path;
  }
}

/** Recharge une sauvegarde dans l'éditeur sans changer de page. */
export async function reloadFrom(path: string) {
  let view: SaveView;
  try {
    view = await invoke<SaveView>("open_save", { path });
  } catch (e) {
    saveState.error = String(e);
    return false;
  }
  const selected = saveState.selected?.slot;
  Object.assign(saveState, { path, view, dirty: false });
  await loadBox(Math.min(saveState.box, Math.max(0, view.boxNames.length - 1)));
  const all: SlotView[] = [...view.party, ...saveState.slots.filter((s): s is SlotView => !!s)];
  saveState.selected = (selected && all.find((s) => sameSlot(s.slot, selected))) ?? view.party[0] ?? null;
  live.pending = null;
  await resync();
  return true;
}

/** Bandeau : recharger la version du jeu (les modifications de Kaleido sont abandonnées). */
export async function acceptGameSave() {
  const path = live.pending;
  if (!path) return;
  if (saveState.dirty) {
    const ok = await ask("Tes modifications non enregistrées dans Kaleido seront perdues. Recharger quand même la sauvegarde du jeu ?", {
      title: "Recharger la sauvegarde du jeu",
      kind: "warning",
      okLabel: "Recharger",
      cancelLabel: "Annuler",
    });
    if (!ok) return;
  }
  if (await reloadFrom(path)) {
    live.status = "reloaded";
    notify("Sauvegarde du jeu rechargée");
  }
}

/** Bandeau : enregistrer d'abord les modifications de Kaleido dans un autre fichier, puis recharger. */
export async function keepCopyThenReload() {
  const path = live.pending;
  if (!path || !saveState.path) return;
  const output = await save({ title: "Enregistrer une copie de tes modifications", defaultPath: saveState.path.replace(/(\.[^.\\/]+)?$/, " (Kaleido)$1") });
  if (!output) return;
  await writeSave(output);
  if (saveState.dirty) return;
  if (await reloadFrom(path)) {
    live.status = "reloaded";
    notify(`Modifications gardées dans ${fileName(output)}, sauvegarde du jeu rechargée`);
  }
}

/** Bandeau : garder les modifications de Kaleido ; la version du jeu est mise de côté. */
export async function keepMine() {
  const path = live.pending;
  if (!path) return;
  // Seul un enregistrement de Kaleido sur ce même fichier pourrait écraser la progression du jeu.
  if (!saveState.dirty || saveState.path !== path) {
    live.pending = null;
    return;
  }
  try {
    const backup = await invoke<string | null>("play_backup", { path });
    live.pending = null;
    await resync();
    notify(backup ? `Version du jeu mise de côté : ${fileName(backup)}` : "Modifications de Kaleido conservées");
  } catch (e) {
    saveState.error = String(e);
  }
}

/** Fichier où l'émulateur lit la sauvegarde de la partie en cours (ou la sauvegarde ouverte). */
export const sendTarget = computed(() => live.session?.savePath ?? saveState.path);

/** « Envoyer au jeu » : écrit la sauvegarde modifiée là où l'émulateur la lit (copie de sécurité d'abord). */
export async function sendToGame() {
  const target = sendTarget.value;
  if (!target || !saveState.view) return;
  const ok = await ask(
    `La sauvegarde sera écrite dans :\n${target}\n\nFerme d'abord le jeu dans l'émulateur (ou redémarre-le juste après) : un jeu ouvert garde l'ancienne sauvegarde en mémoire et l'écraserait à sa prochaine sauvegarde.\n\nUne copie de sécurité de la sauvegarde actuelle du jeu est faite avant.`,
    { title: "Envoyer au jeu", kind: "warning", okLabel: "Envoyer", cancelLabel: "Annuler" },
  );
  if (!ok) return;
  try {
    const tmp = await invoke<string>("play_temp_save");
    await invoke("save_write", { output: tmp });
    const backup = await invoke<string | null>("play_install_save", { src: tmp, dst: target });
    if (target === saveState.path) saveState.dirty = false;
    live.pending = null;
    await resync();
    notify(backup ? `Envoyé au jeu (ancienne version : ${fileName(backup)})` : "Envoyé au jeu");
  } catch (e) {
    saveState.error = String(e);
  }
}

// --- Lancer un jeu

export interface PlayOptions {
  platform: PlayPlatform;
  emulator?: EmulatorId | null;
  /** ROM DS ou jeu 3DS de base. */
  rom: string | null;
  modRomfs?: string | null;
  /** Sauvegarde à installer avant de lancer. */
  save?: string | null;
  /** Jeu de la bibliothèque dont on compte le temps de jeu. */
  trackKey?: string | null;
  /** Nom du jeu, affiché par le compagnon de partie. */
  title?: string | null;
}

const LAST_GAME_KEY = "kaleido.play.lastGame3ds";

async function pickGame(platform: PlayPlatform) {
  let last: string | undefined;
  try {
    last = localStorage.getItem(LAST_GAME_KEY) ?? undefined;
  } catch {
    last = undefined;
  }
  const picked = await open({
    title: platform === "3ds" ? "Choisis le jeu 3DS d'origine à lancer" : platform === "switch" ? "Choisis le jeu Switch à lancer" : "Choisis la ROM DS à lancer",
    defaultPath: platform === "3ds" ? last : undefined,
    filters:
      platform === "3ds"
        ? [{ name: "Jeu 3DS", extensions: ["3ds", "cci", "cxi", "app"] }]
        : platform === "switch"
          ? [{ name: "Jeu Switch", extensions: ["xci", "nsp", "xcz", "nsz"] }]
          : [{ name: "ROM Nintendo DS", extensions: ["nds"] }],
  });
  if (typeof picked !== "string") return null;
  if (platform === "3ds") {
    try {
      localStorage.setItem(LAST_GAME_KEY, picked);
    } catch {
      /* stockage indisponible */
    }
  }
  return picked;
}

function request(o: PlayOptions, emulator: EmulatorId, replaceMod = false): PlayRequest {
  return { emulator, rom: o.rom, modRomfs: o.modRomfs ?? null, save: o.save ?? null, replaceMod, trackKey: o.trackKey ?? null };
}

/** Ce que l'émulateur choisi fera de cette partie (chemin de la sauvegarde…). */
export async function planFor(o: PlayOptions) {
  const emu = o.emulator ? emus.list.find((e) => e.id === o.emulator) : defaultEmulator(o.platform);
  if (!emu) return null;
  return invoke<PlayPlan>("play_plan", { request: request(o, emu.id) });
}

/**
 * Lance le jeu : guide la première fois, choix du jeu de base (3DS), confirmations
 * (mod déjà installé, sauvegarde existante), puis suivi de la sauvegarde.
 */
export async function play(o: PlayOptions): Promise<PlayResult | null> {
  if (!emus.loaded) await loadEmulators();
  if (!guideSeen()) {
    showGuide(() => play(o));
    return null;
  }
  let emu = o.emulator ? emus.list.find((e) => e.id === o.emulator && e.exe) : defaultEmulator(o.platform);
  if (!emu && !o.emulator) {
    // Aucun émulateur pour cette console : on propose d'installer le recommandé.
    const id = RECOMMENDED[o.platform];
    if (!(await installEmulator(id, `Aucun émulateur ${PLATFORM_SHORT[o.platform]} n'a été trouvé sur ce PC.`))) return null;
    emu = defaultEmulator(o.platform);
  }
  if (!emu) {
    showGuide(null);
    return null;
  }
  const opts = { ...o };
  try {
    let plan = await invoke<PlayPlan>("play_plan", { request: request(opts, emu.id) });
    if (plan.needsGame || !opts.rom) {
      const game = await pickGame(opts.platform);
      if (!game) return null;
      opts.rom = game;
      plan = await invoke<PlayPlan>("play_plan", { request: request(opts, emu.id) });
    }
    if (plan.modExists) {
      const ok = await ask(
        `Un mod est déjà installé pour ce jeu dans ${emu.name} :\n${plan.modPath}\n\nIl sera déplacé dans « load/kaleido-backups » (rien n'est supprimé) puis remplacé par le mod Kaleido.`,
        { title: "Remplacer le mod ?", kind: "warning", okLabel: "Remplacer", cancelLabel: "Annuler" },
      );
      if (!ok) return null;
    }
    if (plan.saveConflict) {
      const ok = await ask(
        `${emu.name} a déjà une sauvegarde pour ce jeu :\n${plan.savePath}\n\nUne copie de sécurité en sera faite, puis elle sera remplacée par la sauvegarde choisie.`,
        { title: "Remplacer la sauvegarde du jeu ?", kind: "warning", okLabel: "Remplacer", cancelLabel: "Annuler" },
      );
      if (!ok) return null;
    }
    const result = await invoke<PlayResult>("play_rom", { request: request(opts, emu.id, plan.modExists) });
    live.session = {
      platform: opts.platform,
      emulator: emu.id,
      emulatorName: emu.name,
      rom: opts.rom,
      modRomfs: opts.modRomfs ?? null,
      savePath: result.savePath,
    };
    // Compagnon de partie (DS / 3DS) : s'ouvre à côté de l'émulateur, sauf si désactivé pour ce jeu.
    if (opts.platform !== "switch" && result.savePath) {
      invoke("companion_launch", {
        path: result.savePath,
        title: opts.title ?? "",
        key: opts.trackKey ?? null,
        // Jeu 3DS randomisé (dossier extrait + mod) : c'est le dossier qui porte les vraies rencontres.
        rom: opts.modRomfs ? (opts.trackKey ?? opts.rom) : opts.rom,
      }).catch(() => undefined);
    }
    if (result.warnings.length) await message(result.warnings.join("\n\n"), { title: "À savoir", kind: "info" });
    return result;
  } catch (e) {
    await message(String(e), { title: "Impossible de lancer le jeu", kind: "error" });
    return null;
  }
}

/** Bouton « Jouer » de l'éditeur : lance le jeu de la sauvegarde ouverte, avec cette sauvegarde. */
export async function playOpenSave() {
  const view = saveState.view;
  const path = saveState.path;
  if (!view || !path) return;
  const platform: PlayPlatform = view.generation >= 6 ? "3ds" : "nds";
  if (saveState.dirty) {
    const ok = await ask("Enregistrer tes modifications avant de lancer le jeu ?", {
      title: "Modifications non enregistrées",
      okLabel: "Enregistrer",
      cancelLabel: "Annuler",
    });
    if (!ok) return;
    await writeSave();
    if (saveState.dirty) return;
  }
  let rom: string | null = null;
  if (platform === "nds") {
    rom = await invoke<string | null>("play_find_rom", { save: path }).catch(() => null);
    if (!rom) {
      const stem = fileName(path).replace(/\.[^.]+$/, "").toLowerCase();
      rom = library.items.find((d) => d.kind === "nds_rom" && d.fileName.replace(/\.[^.]+$/, "").toLowerCase() === stem)?.path ?? null;
    }
  }
  const result = await play({ platform, rom, save: path });
  if (!result?.savePath) return;
  // L'éditeur suit désormais le fichier que l'émulateur utilise.
  if (result.savePath !== path && !result.warnings.length) {
    await reloadFrom(result.savePath);
    notify(`Jeu lancé dans ${result.emulator} — Kaleido suit maintenant ${fileName(result.savePath)}`);
  } else {
    notify(`Jeu lancé dans ${result.emulator}`);
  }
}

/** Ouvre dans l'éditeur la sauvegarde que l'émulateur utilise pour cette partie. */
export async function openGameSave(o: PlayOptions) {
  if (!emus.loaded) await loadEmulators();
  const emu = o.emulator ? emus.list.find((e) => e.id === o.emulator) : defaultEmulator(o.platform);
  if (!emu) {
    showGuide(null);
    return "Aucun émulateur configuré.";
  }
  try {
    const plan = await invoke<PlayPlan>("play_plan", { request: request({ ...o, save: null }, emu.id) });
    if (!plan.savePath) return "Emplacement de la sauvegarde inconnu pour ce jeu.";
    live.session = { platform: o.platform, emulator: emu.id, emulatorName: emu.name, rom: o.rom, modRomfs: o.modRomfs ?? null, savePath: plan.savePath };
    if (!plan.saveExists) return `Pas encore de sauvegarde (${fileName(plan.savePath)}) : sauvegarde une première fois en jeu, puis reviens ici.`;
    await openSave(plan.savePath);
    return null;
  } catch (e) {
    return String(e);
  }
}
