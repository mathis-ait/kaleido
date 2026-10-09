import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { titleIdOf, UPDATE_LABEL } from "../games";
import type { Detection } from "../types";
import { defaultEmulator, type EmulatorId, type PlayPlatform } from "./play";

// Miroir des types de `mods.rs` et `tuning.rs`.

export interface ModTarget {
  platform: PlayPlatform;
  titleId: string | null;
  rom: string | null;
  /** Version du jeu installée (mise à jour Switch). */
  gameVersion: string | null;
  /** Switch : fichier de la mise à jour (`rom` = jeu de base). */
  updateFile: string | null;
}

export type ModCategory = "fps" | "graphics" | "textures" | "style" | "qol" | "gameplay" | "romhack" | "audio" | "ui" | "cheats" | "resolution" | "display" | "other";

export interface ModEntry {
  id: string;
  name: string;
  description: string;
  category: ModCategory;
  group: string | null;
  recommended: boolean;
  /** « auto », « gamebanana » / « manual » (catalogue), « explorer » / « local » (installé hors catalogue). */
  source: string;
  author: string;
  page: string;
  size: number | null;
  gameVersion: string | null;
  installed: boolean;
  enabled: boolean;
  updateAvailable: boolean;
  conflicts: string[];
  overlaps: string[];
  requires: string[];
  exclusive: boolean;
  warning: string | null;
  thumbnail: string | null;
  gb: number | null;
  popularity: number | null;
  kind: string | null;
  /** Correctifs ExeFS : « ok », « base » (jeu sans mise à jour), « other » (autre version). */
  exefs: "ok" | "base" | "other" | null;
  /** Touche à la partie : la sauvegarde est copiée avant de l'activer. */
  affectsSave: boolean;
  /** Fichier GameBanana d'avant la dernière mise à jour (retour possible). */
  previousFile: number | null;
  /** Dossier du mod (Switch). */
  folder: string | null;
}

export interface OtherMod {
  name: string;
  enabled: boolean;
  category: ModCategory;
  overlaps: string[];
  canToggle: boolean;
  exefs: "ok" | "base" | "other" | null;
}

export interface ModsView {
  emulator: string | null;
  emulatorFound: boolean;
  location: string | null;
  mods: ModEntry[];
  others: OtherMod[];
  notes: string[];
  errors: string[];
  gamebanana: number | null;
  installedGb: number[];
  canImport: boolean;
  /** Switch : exécutable lancé par Eden. */
  executable: { source: string; buildId: string } | null;
  saves: SaveBackup[];
  lastBackup: SaveBackup | null;
  restoreOffer: SaveBackup | null;
  profiles: GameProfiles | null;
  missing: string[];
}

export interface SaveBackup {
  id: string;
  reason: string;
  modId: string | null;
  created: number;
  size: number;
}

export interface ModProfile {
  name: string;
  mods: string[];
  ownSave: boolean;
}

export interface GameProfiles {
  active: string | null;
  profiles: ModProfile[];
}

/** Profil sans mod, toujours proposé (miroir de `mods_saves::ORIGIN`). */
export const ORIGIN_PROFILE = "Jeu d'origine";

export interface InstallOptions {
  disableConflicts?: boolean;
  file?: number | null;
  variant?: string | null;
  local?: string | null;
}

export interface InstallResult {
  view: ModsView | null;
  variants: string[];
  output: string | null;
}

export interface GbItem {
  id: number;
  name: string;
  thumbnail: string | null;
  author: string;
  category: string;
  likes: number;
  views: number;
  added: number;
  updated: number;
  hasFiles: boolean;
  obsolete: boolean;
  version: string;
}

export interface GbPage {
  total: number;
  complete: boolean;
  items: GbItem[];
  hidden: number;
}

export interface GbCategory {
  id: number;
  name: string;
  count: number;
}

export interface GbProfile {
  id: number;
  name: string;
  text: string;
  files: { id: number; name: string; size: number; date: number; description: string; downloads: number }[];
  images: string[];
  thumbnail: string | null;
  version: string;
  obsolete: boolean;
  likes: number;
  views: number;
  downloads: number;
  updated: number;
  author: string;
  category: string;
}

export interface Cheat {
  name: string;
  group: string | null;
  enabled: boolean;
}

export type Tier = "low" | "medium" | "high" | "ultra";

export interface TunePlan {
  emulator: string;
  gpu: { name: string; vramMb: number; tier: Tier } | null;
  tier: Tier;
  tierLabel: string;
  file: string | null;
  perGame: boolean;
  settings: { label: string; value: string }[];
  applied: boolean;
  canRestore: boolean;
  error: string | null;
}

export const CATEGORY_LABEL: Record<ModCategory, string> = {
  fps: "Fluidité : vrais 60 FPS",
  graphics: "Graphismes",
  textures: "Textures HD",
  style: "Apparence des personnages et des Pokémon",
  qol: "Confort de jeu",
  gameplay: "Gameplay et difficulté",
  romhack: "Romhacks",
  audio: "Musiques et sons",
  ui: "Interface et manettes",
  cheats: "Codes de triche",
  resolution: "Résolution calculée par le jeu",
  display: "Écrans ultra-larges",
  other: "Autres",
};

/** Libellés courts (filtres). */
export const CATEGORY_SHORT: Record<ModCategory, string> = {
  fps: "Fluidité",
  graphics: "Graphismes",
  textures: "Textures",
  style: "Apparence",
  qol: "Confort",
  gameplay: "Gameplay",
  romhack: "Romhacks",
  audio: "Audio",
  ui: "Interface",
  cheats: "Triche",
  resolution: "Résolution",
  display: "Ultra-large",
  other: "Autres",
};

export const CATEGORY_ORDER: ModCategory[] = ["fps", "graphics", "textures", "style", "qol", "gameplay", "romhack", "audio", "ui", "cheats", "resolution", "display", "other"];

export function targetOf(d: Detection): ModTarget {
  const platform: PlayPlatform = d.platform === "switch" ? "switch" : d.platform === "3ds" ? "3ds" : d.platform === "gba" || d.platform === "gb" ? "gba" : "nds";
  const gameVersion = d.details.find((x) => x.label === UPDATE_LABEL)?.value ?? null;
  const rom = platform === "nds" || platform === "gba" || platform === "switch" ? d.path : null;
  return { platform, titleId: titleIdOf(d), rom, gameVersion: gameVersion && /^\d/.test(gameVersion) ? gameVersion : null, updateFile: platform === "switch" ? (d.updatePath ?? null) : null };
}

/** Émulateur dont on règle la configuration pour ce jeu. */
export function tuneEmulator(platform: PlayPlatform): EmulatorId {
  if (platform === "switch") return "eden";
  return defaultEmulator(platform)?.id ?? (platform === "3ds" ? "azahar" : platform === "gba" ? "mgba" : "melonds");
}

/** Fenêtre « Mods et réglages » ouverte pour ce jeu. */
export const modsDialog = reactive({ game: null as Detection | null });

export const openMods = (game: Detection) => (modsDialog.game = game);

export interface ModProgress {
  step: "download" | "verify" | "extract" | "install";
  done: number;
  total: number;
}

/** Installations en cours, par identifiant de mod. */
export const modProgress = reactive<Record<string, ModProgress>>({});

listen<ModProgress & { id: string }>("mod-install", (e) => {
  const { id, ...p } = e.payload;
  if (id in modProgress) modProgress[id] = p;
}).catch(() => undefined);

export const listMods = (target: ModTarget) => invoke<ModsView>("mods_list", { target });

export async function installMod(target: ModTarget, id: string, options: InstallOptions) {
  modProgress[id] = { step: "download", done: 0, total: 0 };
  try {
    return await invoke<InstallResult>("mods_install", { target, id, options });
  } finally {
    delete modProgress[id];
  }
}

export const uninstallMod = (target: ModTarget, id: string) => invoke<ModsView>("mods_uninstall", { target, id });
export const toggleMod = (target: ModTarget, id: string, enabled: boolean) => invoke<ModsView>("mods_toggle", { target, id, enabled });
/** Mods à mettre à jour par jeu (title ID), affichés sur les tuiles de la bibliothèque. */
export const modUpdates = reactive<{ counts: Record<string, number>; checked: boolean }>({ counts: {}, checked: false });

export async function refreshModUpdates() {
  try {
    modUpdates.counts = await invoke<Record<string, number>>("mods_pending_updates");
  } catch {
    // Hors ligne ou émulateur absent : pas d'étiquette.
  } finally {
    modUpdates.checked = true;
  }
}

/** Nombre de mods à mettre à jour pour un jeu. */
export function modUpdatesOf(d: Detection): number {
  const tid = titleIdOf(d);
  if (!tid) return 0;
  // Les mises à jour Switch ont le même title ID de base, à 0x800 près.
  const base = d.platform === "switch" ? tid.slice(0, 13) + "000" : tid;
  return modUpdates.counts[base] ?? 0;
}

export const reorderMods = (target: ModTarget, order: string[]) => invoke<ModsView>("mods_reorder", { target, order });
export const restoreSave = (target: ModTarget, id: string) => invoke<ModsView>("mods_save_restore", { target, id });
export const saveProfile = (target: ModTarget, name: string) => invoke<ModsView>("mods_profile_save", { target, name });
export const deleteProfile = (target: ModTarget, name: string) => invoke<ModsView>("mods_profile_delete", { target, name });
export const applyProfile = (target: ModTarget, name: string) => invoke<ModsView>("mods_profile_apply", { target, name });

/** « 9 oct. 2026 à 13:07 ». */
export const formatDate = (secs: number) => new Date(secs * 1000).toLocaleString("fr-FR", { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
export const browseMods = (game: number, query: string, sort: string, category: number | null, page: number) => invoke<GbPage>("mods_browse", { game, query, sort, category, page });
export const modCategories = (game: number) => invoke<GbCategory[]>("mods_categories", { game });
export const modDetails = (id: number) => invoke<GbProfile>("mods_details", { id });
export const downloadsDir = () => invoke<string | null>("mods_downloads_dir");
export const watchDownloads = (id: string, extensions: string[]) => invoke<void>("mods_watch_downloads", { id, extensions });
export const stopWatch = () => invoke<void>("mods_watch_stop");
export const toggleOther = (target: ModTarget, name: string, enabled: boolean) => invoke<ModsView>("mods_toggle_other", { target, name, enabled });
export const listCheats = (target: ModTarget) => invoke<Cheat[]>("cheats_list", { target });
export const setCheats = (target: ModTarget, enabled: string[]) => invoke<Cheat[]>("cheats_set", { target, enabled });

const tuneTarget = (target: ModTarget) => ({ emulator: tuneEmulator(target.platform), titleId: target.titleId });
export const tunePlan = (target: ModTarget) => invoke<TunePlan>("tune_plan", { target: tuneTarget(target) });
export const tuneApply = (target: ModTarget) => invoke<TunePlan>("tune_apply", { target: tuneTarget(target) });
export const tuneRestore = (target: ModTarget) => invoke<TunePlan>("tune_restore", { target: tuneTarget(target) });

/** « 12,3 k » pour les compteurs de vues. */
export function formatCount(n: number) {
  if (n < 1000) return String(n);
  if (n < 1e6) return `${(n / 1000).toFixed(n < 1e4 ? 1 : 0).replace(".", ",")} k`;
  return `${(n / 1e6).toFixed(1).replace(".", ",")} M`;
}

export function formatSize(bytes: number | null) {
  if (bytes === null) return null;
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} Ko`;
  if (bytes < 1024 ** 3) return `${Math.round(bytes / 1024 ** 2)} Mo`;
  return `${(bytes / 1024 ** 3).toFixed(1).replace(".", ",")} Go`;
}
