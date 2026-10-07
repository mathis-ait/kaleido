import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { PokeTypeKey, TypeTag } from "./types";

// --- Miroir de `kaleido_core::battle` et des commandes `battle_*`.

/** Types dans l'ordre canonique du moteur (0 Normal … 17 Fée). */
const TYPE_KEYS: PokeTypeKey[] = [
  "normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel",
  "fire", "water", "grass", "electric", "psychic", "ice", "dragon", "dark", "fairy",
];
const TYPE_NAMES = [
  "Normal", "Combat", "Vol", "Poison", "Sol", "Roche", "Insecte", "Spectre", "Acier",
  "Feu", "Eau", "Plante", "Électrik", "Psy", "Glace", "Dragon", "Ténèbres", "Fée",
];
export const typeTag = (id: number): TypeTag => ({ key: TYPE_KEYS[id] ?? "normal", name: TYPE_NAMES[id] ?? "?" });

export type Role = "rival" | "gym" | "admin" | "boss" | "eliteFour" | "champion";
export type Weather = "none" | "sun" | "rain" | "sand" | "hail";
export type Status = "none" | "burn" | "paralysis" | "poison" | "sleep";
export type Verdict = "win" | "uncertain" | "lose" | "none";

export interface TeamSlot {
  species: number;
  form: number;
  level: number;
  name: string;
}

export interface TrainerSummary {
  id: number;
  className: string;
  name: string;
  role: Role | null;
  roleLabel: string | null;
  double: boolean;
  maxLevel: number;
  team: TeamSlot[];
}

export interface LinkInfo {
  path: string;
  game: string;
  verified: boolean;
  trainers: TrainerSummary[];
}

export interface Combatant {
  species: number;
  form: number;
  name: string;
  level: number;
  types: number[];
  /** PV, Att, Déf, Atq Spé, Déf Spé, Vit. */
  stats: number[];
  ability: number;
  item: number;
  moves: number[];
  nature: number;
  ivs: number[];
  evs: number[];
  friendship: number;
  weight: number;
  notes: string[];
}

export interface KoInfo {
  hits: number;
  chance: number;
  guaranteed: number | null;
  label: string;
}

export type Damage =
  | { kind: "none"; reason: string }
  | { kind: "fixed"; amount: number }
  | { kind: "rolls"; hits: number[][] };

export interface MoveLine {
  moveId: number;
  name: string;
  typeId: number;
  category: "physical" | "special" | "status";
  power: number | null;
  priority: number;
  hits: number;
  effectiveness: number;
  critical: boolean;
  damage: Damage;
  modifiers: string[];
  min: number;
  max: number;
  minPercent: number;
  maxPercent: number;
  critMin: number;
  critMax: number;
  critMinPercent: number;
  critMaxPercent: number;
  rolls: number[];
  ko: KoInfo;
}

export interface Duel {
  attackerSpeed: number;
  defenderSpeed: number;
  defenderHp: number;
  defenderMaxHp: number;
  moves: MoveLine[];
  notes: string[];
}

export interface BestMove {
  moveId: number;
  name: string;
  typeId: number;
  priority: number;
  minPercent: number;
  maxPercent: number;
  ko: KoInfo;
}

export interface Cell {
  mine: BestMove | null;
  theirs: BestMove | null;
  mySpeed: number;
  theirSpeed: number;
  first: "me" | "them" | "tie";
  verdict: Verdict;
}

export interface MatrixView {
  trainer: TrainerSummary | null;
  mine: Combatant[];
  theirs: Combatant[];
  cells: Cell[][];
}

export interface DuelView {
  me: Combatant;
  them: Combatant;
  attack: Duel;
  defense: Duel;
}

export interface SideOptions {
  /** Indices 1 Att, 2 Déf, 3 Atq Spé, 4 Déf Spé, 5 Vit. */
  boosts: number[];
  status: Status;
  hpPercent: number;
}

export interface Options {
  field: { weather: Weather; intimidate: boolean };
  mine: SideOptions;
  theirs: SideOptions;
}

const side = (): SideOptions => ({ boosts: [0, 0, 0, 0, 0, 0], status: "none", hpPercent: 100 });

/** État de la page « Combat ». */
export const battle = reactive({
  link: null as LinkInfo | null,
  linking: false,
  error: null as string | null,
  trainerId: null as number | null,
  matrix: null as MatrixView | null,
  computing: false,
  selected: null as { mine: number; theirs: number } | null,
  duel: null as DuelView | null,
  options: { field: { weather: "none", intimidate: true }, mine: side(), theirs: side() } as Options,
});

export function resetOptions() {
  battle.options = { field: { weather: "none", intimidate: true }, mine: side(), theirs: side() };
}

const LINK_KEY = "kaleido.battleRoms";

/** ROM liée à chaque sauvegarde, pour la relier automatiquement. */
function rememberedLinks(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(LINK_KEY) ?? "{}");
  } catch {
    return {};
  }
}

function remember(savePath: string | null, romPath: string | null) {
  if (!savePath) return;
  try {
    const links = rememberedLinks();
    if (romPath) links[savePath] = romPath;
    else delete links[savePath];
    localStorage.setItem(LINK_KEY, JSON.stringify(links));
  } catch {
    /* stockage indisponible : le lien n'est pas mémorisé */
  }
}

function clearTrainer() {
  battle.trainerId = null;
  battle.matrix = null;
  battle.selected = null;
  battle.duel = null;
}

export async function linkRom(romPath: string, savePath: string | null) {
  battle.linking = true;
  battle.error = null;
  try {
    battle.link = await invoke<LinkInfo>("battle_link_rom", { path: romPath });
    remember(savePath, romPath);
    clearTrainer();
  } catch (e) {
    battle.error = String(e);
  } finally {
    battle.linking = false;
  }
}

/** Au chargement de la page : ROM déjà liée, ou celle retenue pour cette sauvegarde. */
export async function restoreLink(savePath: string | null) {
  battle.error = null;
  try {
    const current = await invoke<LinkInfo | null>("battle_linked");
    if (current) {
      battle.link = current;
      return;
    }
  } catch {
    /* pas de sauvegarde ouverte */
  }
  battle.link = null;
  clearTrainer();
  const remembered = savePath ? rememberedLinks()[savePath] : undefined;
  if (remembered) {
    await linkRom(remembered, savePath);
    // Une ROM déplacée ou supprimée : on oublie le lien sans afficher d'erreur.
    if (!battle.link) {
      battle.error = null;
      remember(savePath, null);
    }
  }
}

export async function unlinkRom(savePath: string | null) {
  await invoke("battle_unlink").catch(() => undefined);
  battle.link = null;
  remember(savePath, null);
  clearTrainer();
}

export async function selectTrainer(id: number) {
  battle.trainerId = id;
  battle.selected = null;
  battle.duel = null;
  await computeMatrix();
}

export async function computeMatrix() {
  if (battle.trainerId === null) return;
  battle.computing = true;
  battle.error = null;
  try {
    battle.matrix = await invoke<MatrixView>("battle_matrix", { trainer: battle.trainerId, options: battle.options });
    if (battle.selected) await openDuel(battle.selected.mine, battle.selected.theirs);
  } catch (e) {
    battle.error = String(e);
  } finally {
    battle.computing = false;
  }
}

export async function openDuel(mine: number, theirs: number) {
  if (battle.trainerId === null) return;
  battle.selected = { mine, theirs };
  try {
    battle.duel = await invoke<DuelView>("battle_duel", { trainer: battle.trainerId, mine, theirs, options: battle.options });
  } catch (e) {
    battle.error = String(e);
  }
}

/** « 45,2 » à la française. */
export const pct = (x: number) => x.toLocaleString("fr-FR", { maximumFractionDigits: 1 });
