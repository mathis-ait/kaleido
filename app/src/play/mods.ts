import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { titleIdOf } from "../games";
import type { Detection } from "../types";
import { defaultEmulator, type EmulatorId, type PlayPlatform } from "./play";

// Miroir des types de `mods.rs` et `tuning.rs`.

export interface ModTarget {
  platform: PlayPlatform;
  titleId: string | null;
  rom: string | null;
}

export type ModCategory = "fps" | "graphics" | "resolution" | "display" | "textures" | "cheats" | "other";

export interface ModEntry {
  id: string;
  name: string;
  description: string;
  category: ModCategory;
  group: string | null;
  recommended: boolean;
  author: string;
  page: string;
  size: number | null;
  gameVersion: string | null;
  installed: boolean;
  updateAvailable: boolean;
  conflicts: string[];
  warning: string | null;
}

export interface OtherMod {
  name: string;
  enabled: boolean;
  category: ModCategory;
}

export interface ModsView {
  emulator: string | null;
  emulatorFound: boolean;
  location: string | null;
  mods: ModEntry[];
  others: OtherMod[];
  notes: string[];
  errors: string[];
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
  resolution: "Résolution calculée par le jeu",
  display: "Écrans ultra-larges",
  cheats: "Codes de triche",
  other: "Autres",
};

export const CATEGORY_ORDER: ModCategory[] = ["fps", "graphics", "textures", "cheats", "resolution", "display", "other"];

export function targetOf(d: Detection): ModTarget {
  const platform: PlayPlatform = d.platform === "switch" ? "switch" : d.platform === "3ds" ? "3ds" : d.platform === "gba" ? "gba" : "nds";
  return { platform, titleId: titleIdOf(d), rom: platform === "nds" || platform === "gba" ? d.path : null };
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

export async function installMod(target: ModTarget, id: string, disableConflicts: boolean) {
  modProgress[id] = { step: "download", done: 0, total: 0 };
  try {
    return await invoke<ModsView>("mods_install", { target, id, disableConflicts });
  } finally {
    delete modProgress[id];
  }
}

export const uninstallMod = (target: ModTarget, id: string) => invoke<ModsView>("mods_uninstall", { target, id });
export const toggleOther = (target: ModTarget, name: string, enabled: boolean) => invoke<ModsView>("mods_toggle_other", { target, name, enabled });
export const listCheats = (target: ModTarget) => invoke<Cheat[]>("cheats_list", { target });
export const setCheats = (target: ModTarget, enabled: string[]) => invoke<Cheat[]>("cheats_set", { target, enabled });

const tuneTarget = (target: ModTarget) => ({ emulator: tuneEmulator(target.platform), titleId: target.titleId });
export const tunePlan = (target: ModTarget) => invoke<TunePlan>("tune_plan", { target: tuneTarget(target) });
export const tuneApply = (target: ModTarget) => invoke<TunePlan>("tune_apply", { target: tuneTarget(target) });
export const tuneRestore = (target: ModTarget) => invoke<TunePlan>("tune_restore", { target: tuneTarget(target) });

export function formatSize(bytes: number | null) {
  if (bytes === null) return null;
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} Ko`;
  if (bytes < 1024 ** 3) return `${Math.round(bytes / 1024 ** 2)} Mo`;
  return `${(bytes / 1024 ** 3).toFixed(1).replace(".", ",")} Go`;
}
