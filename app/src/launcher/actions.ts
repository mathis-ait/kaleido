import { reactive } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { allGames } from "../games";
import { nav } from "../nav";
import { isKaleidoRom, RANDOMIZABLE, type Detection } from "../types";
import { openGameSave, play, type PlayOptions, type PlayPlatform } from "../play/play";

/** Actions communes à la grille et au lanceur. */

export const platformOf = (d: Detection): PlayPlatform => (d.platform === "3ds" ? "3ds" : "nds");

/** Jaquette (boîte française en priorité, voir library.rs). */
export const coverUrl = (d: Detection) => (d.game ? convertFileSrc(`${d.game.id}.png`, "cover") : null);

/** Un dossier 3DS (mod LayeredFS ou jeu extrait) se joue par-dessus le jeu d'origine. */
export function playOptions(d: Detection): PlayOptions {
  const platform = platformOf(d);
  if (d.kind !== "ctr_dump") return { platform, rom: d.path, modRomfs: null };
  const modRomfs = /[\\/]romfs$/i.test(d.path) ? d.path : `${d.path}\\romfs`;
  const base = allGames.value.find((g) => g.kind === "ctr_rom" && g.game?.id === d.game?.id && !isKaleidoRom(g));
  return { platform, rom: base?.path ?? null, modRomfs };
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
