import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { addPaths, library } from "../library";
import { nav } from "../nav";
import { defaultEmulator, planFor, type PlayOptions } from "../play/play";
import { BUILTIN_PRESETS, deleteUserPreset, presetsFor, settingsOf, userPresets, type AdventurePreset } from "../presets";
import type { CtrOutcome, Detection, Outcome, RandomizerSettings } from "../types";

/**
 * « Nouvelle aventure » en trois clics depuis le lanceur : la jaquette (déjà choisie dans le
 * carrousel), le preset, puis l'aperçu de la partie tirée et « Jouer ».
 */

export interface PreviewMon {
  species: number;
  name: string;
  minLevel: number;
  maxLevel: number;
}

export interface AdventurePreview {
  seed: number;
  shareCode: string;
  starters: { id: number; name: string }[];
  firstRoute: { name: string; encounters: PreviewMon[] } | null;
  firstLeader: { name: string; label: string; team: PreviewMon[] } | null;
  elapsedMs: number;
}

export const adventure = reactive({
  open: false,
  step: "preset" as "preset" | "preview",
  game: null as Detection | null,
  presets: [] as AdventurePreset[],
  /** Carte en surbrillance (clavier, manette). */
  index: 0,
  preset: null as AdventurePreset | null,
  settings: null as RandomizerSettings | null,
  seed: 0,
  preview: null as AdventurePreview | null,
  loading: false,
  error: null as string | null,
});

const newSeed = () => Math.floor(Math.random() * 4_294_967_295);
const platformOf = (d: Detection): "nds" | "3ds" => (d.platform === "3ds" ? "3ds" : "nds");
const parentDir = (p: string) => p.replace(/[\\/][^\\/]+$/, "");
const separator = (p: string) => (p.includes("\\") ? "\\" : "/");

export async function openAdventure(d: Detection) {
  Object.assign(adventure, { open: true, step: "preset", game: d, index: 0, preset: null, settings: null, preview: null, error: null });
  adventure.presets = presetsFor([...BUILTIN_PRESETS, ...(await userPresets())], d.game?.id);
}

export function closeAdventure() {
  adventure.open = false;
}

async function computePreview() {
  const g = adventure.game;
  if (!g || !adventure.settings) return;
  adventure.loading = true;
  adventure.error = null;
  const seed = adventure.seed;
  try {
    const p = await invoke<AdventurePreview>("adventure_preview", { path: g.path, settings: adventure.settings, seed });
    // Un autre tirage a pu être demandé entre-temps.
    if (seed === adventure.seed) adventure.preview = p;
  } catch (e) {
    if (seed === adventure.seed) adventure.error = String(e);
  } finally {
    if (seed === adventure.seed) adventure.loading = false;
  }
}

export async function choosePreset(p: AdventurePreset) {
  if (!adventure.game) return;
  adventure.preset = p;
  adventure.settings = await settingsOf(p, platformOf(adventure.game));
  adventure.seed = newSeed();
  adventure.preview = null;
  adventure.step = "preview";
  await computePreview();
}

/** « Relancer le tirage » : même preset, autre seed. */
export function reroll() {
  adventure.seed = newSeed();
  return computePreview();
}

/**
 * Seed ou code de partage collé : un nombre garde le preset choisi, un code « KLD1-… »
 * reprend aussi les réglages de l'ami qui l'a partagé. Renvoie false si le texte est illisible.
 */
export async function applySeedText(text: string): Promise<boolean> {
  const t = text.trim();
  if (/^\d+$/.test(t)) {
    adventure.seed = Number(t) % 4_294_967_296;
    await computePreview();
    return true;
  }
  const parsed = await invoke<[number, RandomizerSettings] | null>("parse_share_code", { code: t }).catch(() => null);
  if (!parsed) return false;
  adventure.seed = parsed[0];
  adventure.settings = parsed[1];
  adventure.preset = { version: 1, id: "partage", name: "Code partagé", tagline: "Réglages repris du code collé.", changes: [], settings: {} };
  await computePreview();
  return true;
}

export async function removePreset(p: AdventurePreset) {
  await deleteUserPreset(p.id);
  adventure.presets = adventure.presets.filter((x) => x.id !== p.id);
  adventure.index = Math.min(adventure.index, Math.max(adventure.presets.length - 1, 0));
}

/** Où la partie sera écrite, et avec quel émulateur (affiché en bas de l'aperçu). */
export function destination() {
  const g = adventure.game;
  if (!g) return null;
  const platform = platformOf(g);
  const sep = separator(g.path);
  const where = platform === "nds" ? parentDir(g.path) : `${parentDir(g.path)}${sep}Kaleido`;
  return { where, emulator: defaultEmulator(platform)?.name ?? "à installer" };
}

/** Écrit la partie (ROM DS ou mod 3DS) et renvoie de quoi la lancer. */
export async function writeAdventure(): Promise<PlayOptions> {
  const g = adventure.game;
  const settings = adventure.settings;
  if (!g || !settings) throw new Error("aucune aventure préparée");
  const seed = adventure.seed;
  const title = g.game?.name ?? g.title;
  let options: PlayOptions;
  let romForRun: string;
  if (platformOf(g) === "nds") {
    const output = `${g.path.replace(/\.nds$/i, "")} - Kaleido ${seed}.nds`;
    await invoke<Outcome>("randomize_rom", { path: g.path, settings, seed, output });
    await addPaths([output]);
    options = { platform: "nds", rom: output, trackKey: output, title };
    romForRun = output;
  } else {
    const output = destination()!.where;
    const res = await invoke<CtrOutcome>("randomize_ctr", {
      path: g.path,
      settings: { ...settings, ctrOutput: "layered_fs" },
      seed,
      output,
      target: "emulator",
    });
    if (!res.romfs) throw new Error("le mod n'a pas été écrit");
    options = { platform: "3ds", rom: g.path, modRomfs: res.romfs, trackKey: res.romfs, title };
    romForRun = res.romfs;
  }
  // « Nuzlocke prêt » : la sauvegarde de la partie est suivie avant même la première sauvegarde.
  if (adventure.preset?.companion?.nuzlocke) {
    const plan = await planFor(options).catch(() => null);
    if (plan?.savePath) await invoke("nuzlocke_track_save", { save: plan.savePath, rom: romForRun }).catch(() => undefined);
  }
  return options;
}

/** « Personnaliser » : le randomizer détaillé, pré-rempli avec le preset et la seed. */
export function customize() {
  const g = adventure.game;
  if (!g || !adventure.settings) return;
  if (!library.items.some((x) => x.path === g.path)) library.items.unshift(g);
  nav.randomizerRom = g.path;
  nav.randomizerSettings = { settings: JSON.parse(JSON.stringify(adventure.settings)), seed: adventure.seed };
  adventure.open = false;
  nav.view = "randomizer";
}
