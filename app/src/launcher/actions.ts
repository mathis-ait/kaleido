import { reactive } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { allGames, titleIdOf } from "../games";
import { library } from "../library";
import { nav } from "../nav";
import { isKaleidoRom, RANDOMIZABLE, type Detection } from "../types";
import { openGameSave, play, type PlayOptions, type PlayPlatform } from "../play/play";

/** Actions communes à la grille et au lanceur. */

export const platformOf = (d: Detection): PlayPlatform => (d.platform === "3ds" ? "3ds" : d.platform === "switch" ? "switch" : "nds");

/** Jaquette (boîte française en priorité, icône officielle pour la Switch ; voir library.rs). */
export function coverUrl(d: Detection) {
  if (d.platform === "switch") return convertFileSrc(`nx-${titleIdOf(d)}.png`, "cover");
  return d.game ? convertFileSrc(`${d.game.id}.png`, "cover") : null;
}

/** Un dossier 3DS (mod LayeredFS ou jeu extrait) se joue par-dessus le jeu d'origine. */
export function playOptions(d: Detection): PlayOptions {
  const platform = platformOf(d);
  if (d.kind !== "ctr_dump") return { platform, rom: d.path, modRomfs: null, trackKey: d.path };
  const modRomfs = /[\\/]romfs$/i.test(d.path) ? d.path : `${d.path}\\romfs`;
  const base = allGames.value.find((g) => g.kind === "ctr_rom" && g.game?.id === d.game?.id && !isKaleidoRom(g));
  return { platform, rom: base?.path ?? null, modRomfs, trackKey: d.path };
}

const LAST_PLAYED_KEY = "kaleido.library.lastPlayed";

function readLastPlayed(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem(LAST_PLAYED_KEY) ?? "{}");
  } catch {
    return {};
  }
}

/** Dernière partie lancée depuis Kaleido, par chemin (ms). */
export const lastPlayed = reactive<Record<string, number>>(readLastPlayed());

/** Lance le jeu ; renvoie le nom de l'émulateur, ou null. */
export async function launchGame(d: Detection): Promise<string | null> {
  const result = await play(playOptions(d));
  if (!result) return null;
  lastPlayed[d.path] = Date.now();
  try {
    localStorage.setItem(LAST_PLAYED_KEY, JSON.stringify(lastPlayed));
  } catch {
    /* stockage indisponible */
  }
  return result.emulator;
}

export const openSaveOf = (d: Detection) => openGameSave(playOptions(d));

export const canRandomize = (d: Detection) => !isKaleidoRom(d) && RANDOMIZABLE.includes(d.game?.id ?? "");

export function randomize(d: Detection) {
  // Le Randomizer choisit parmi les fichiers ouverts : un jeu de la bibliothèque y est ajouté.
  if (!library.items.some((x) => x.path === d.path)) library.items.unshift(d);
  nav.randomizerRom = d.path;
  nav.view = "randomizer";
}

/** « il y a 3 h », « hier »… */
export function timeAgo(ms: number): string {
  const s = (Date.now() - ms) / 1000;
  if (s < 60) return "à l'instant";
  if (s < 3600) return `il y a ${Math.round(s / 60)} min`;
  if (s < 86400) return `il y a ${Math.round(s / 3600)} h`;
  if (s < 2 * 86400) return "hier";
  if (s < 30 * 86400) return `il y a ${Math.round(s / 86400)} jours`;
  return new Date(ms).toLocaleDateString("fr-FR", { day: "numeric", month: "long" });
}

// --- Sauvegarde et temps de jeu

/** Miroir de `GameStatus` (library.rs). */
export interface GameStatus {
  emulator: string | null;
  savePath: string | null;
  saveExists: boolean;
  trainer: string | null;
  saveSeconds: number | null;
  emulatorSeconds: number | null;
  kaleidoSeconds: number | null;
}

/** État de chaque jeu, par chemin (rempli à la demande). */
export const statuses = reactive<Record<string, GameStatus>>({});
const pending = new Set<string>();

const EMPTY_STATUS: GameStatus = { emulator: null, savePath: null, saveExists: false, trainer: null, saveSeconds: null, emulatorSeconds: null, kaleidoSeconds: null };

export async function refreshStatus(d: Detection) {
  // Switch : Eden range ses sauvegardes lui-même, rien à lire.
  if (d.platform === "switch") {
    statuses[d.path] ??= { ...EMPTY_STATUS };
    return;
  }
  if (pending.has(d.path)) return;
  pending.add(d.path);
  const o = playOptions(d);
  try {
    statuses[d.path] = await invoke<GameStatus>("game_status", { rom: o.rom, modRomfs: o.modRomfs ?? null, ctr: o.platform === "3ds", key: d.path });
  } catch {
    // Jeu illisible : rien à afficher (et on ne redemande pas à chaque rendu).
    statuses[d.path] ??= { emulator: null, savePath: null, saveExists: false, trainer: null, saveSeconds: null, emulatorSeconds: null, kaleidoSeconds: null };
  } finally {
    pending.delete(d.path);
  }
}

/** Charge l'état d'un jeu s'il n'est pas encore connu. */
export function statusOf(d: Detection): GameStatus | null {
  if (!statuses[d.path]) void refreshStatus(d);
  return statuses[d.path] ?? null;
}

// Au retour dans Kaleido (après une partie), les sauvegardes et compteurs ont pu changer.
window.addEventListener("focus", () => {
  for (const path of Object.keys(statuses)) {
    const d = allGames.value.find((g) => g.path === path);
    if (d) void refreshStatus(d);
  }
});

/** Temps de jeu le plus fiable : celui du jeu, sinon celui de l'émulateur, sinon celui de Kaleido. */
export function playTime(s: GameStatus | null): { seconds: number; source: string } | null {
  if (!s) return null;
  if (s.saveSeconds) return { seconds: s.saveSeconds, source: "compté par le jeu (carte de dresseur)" };
  if (s.emulatorSeconds) return { seconds: s.emulatorSeconds, source: `compté par ${s.emulator ?? "l'émulateur"}` };
  if (s.kaleidoSeconds) return { seconds: s.kaleidoSeconds, source: "parties lancées depuis Kaleido" };
  return null;
}

/** « 42 h 17 », « 35 min ». */
export function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h === 0) return `${Math.max(1, m)} min`;
  return `${h} h ${String(m).padStart(2, "0")}`;
}
