import { invoke } from "@tauri-apps/api/core";
import type { RandomizerSettings } from "../types";

/**
 * Presets de « Nouvelle aventure » : un fichier JSON versionné par preset dans ce dossier,
 * plus ceux que l'utilisateur enregistre depuis le randomizer détaillé. Les réglages ne
 * listent que ce qui change par rapport au moteur (tout inchangé) ; `nds` et `ctr`
 * surchargent pour une plateforme (taux de chromatiques…).
 */
export interface AdventurePreset {
  version: 1;
  id: string;
  order?: number;
  name: string;
  tagline: string;
  /** Jeux concernés (identifiants), sinon tous. */
  games?: string[];
  /** Trois changements, chacun avec une icône. */
  changes: { icon: string; label: string }[];
  /** « Nuzlocke prêt » : la partie est suivie par le compagnon dès le lancement. */
  companion?: { nuzlocke?: boolean };
  settings: Record<string, unknown>;
  nds?: Record<string, unknown>;
  ctr?: Record<string, unknown>;
  /** Enregistré par l'utilisateur (modifiable, supprimable). */
  user?: boolean;
}

const files = import.meta.glob<{ default: AdventurePreset }>("./*.json", { eager: true });

export const BUILTIN_PRESETS: AdventurePreset[] = Object.values(files)
  .map((m) => m.default)
  .sort((a, b) => (a.order ?? 99) - (b.order ?? 99));

type Plain = Record<string, unknown>;
const isPlain = (v: unknown): v is Plain => typeof v === "object" && v !== null && !Array.isArray(v);

/** Fusion récursive : chaque `over` remplace les clés de `base` (copie, rien n'est modifié). */
export function deepMerge<T>(base: T, ...overs: (Plain | undefined)[]): T {
  const out = JSON.parse(JSON.stringify(base)) as Plain;
  for (const over of overs) {
    if (!over) continue;
    for (const [k, v] of Object.entries(over)) {
      out[k] = isPlain(v) && isPlain(out[k]) ? deepMerge(out[k], v) : JSON.parse(JSON.stringify(v));
    }
  }
  return out as T;
}

let defaults: Promise<RandomizerSettings> | null = null;

/** Réglages du moteur quand rien n'est demandé. */
export function engineDefaults(): Promise<RandomizerSettings> {
  defaults ??= invoke<RandomizerSettings>("randomizer_defaults");
  return defaults;
}

/** Réglages complets d'un preset pour une plateforme. */
export async function settingsOf(preset: AdventurePreset, platform: "nds" | "3ds"): Promise<RandomizerSettings> {
  return deepMerge(await engineDefaults(), preset.settings, platform === "3ds" ? preset.ctr : preset.nds);
}

/** Presets proposés pour un jeu. */
export function presetsFor(all: AdventurePreset[], gameId: string | undefined) {
  return all.filter((p) => !p.games || (!!gameId && p.games.includes(gameId)));
}

export async function userPresets(): Promise<AdventurePreset[]> {
  const list = await invoke<AdventurePreset[]>("user_presets").catch(() => []);
  return list.map((p) => ({ ...p, user: true }));
}

/** Les trois changements affichés sur la carte d'un preset enregistré par l'utilisateur. */
function summarize(s: RandomizerSettings): AdventurePreset["changes"] {
  return [
    { icon: "ball", label: s.starters === "unchanged" ? "Starters d'origine" : "Starters tirés au sort" },
    { icon: "map", label: s.wild === "unchanged" ? "Rencontres d'origine" : "Rencontres randomisées" },
    { icon: "swords", label: s.trainers === "unchanged" ? "Dresseurs d'origine" : "Dresseurs randomisés" },
  ];
}

const stripUser = ({ user: _user, ...p }: AdventurePreset) => p;

/** Enregistre les réglages détaillés comme preset personnel (carte de plus dans « Nouvelle aventure »). */
export async function saveUserPreset(name: string, settings: RandomizerSettings) {
  const list = (await userPresets()).map(stripUser);
  list.push({
    version: 1,
    id: `perso-${Date.now()}`,
    order: 100 + list.length,
    name,
    tagline: "Tes réglages, enregistrés depuis le randomizer.",
    changes: summarize(settings),
    settings: JSON.parse(JSON.stringify(settings)),
  });
  await invoke("save_user_presets", { presets: list });
}

export async function deleteUserPreset(id: string) {
  const list = (await userPresets()).filter((p) => p.id !== id).map(stripUser);
  await invoke("save_user_presets", { presets: list });
}
