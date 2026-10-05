/** Mode Nuzlocke : miroir de `kaleido_core::nuzlocke` et des commandes `nuzlocke_*`. */
import { invoke } from "@tauri-apps/api/core";
import type { Slot } from "../../types";

export interface NuzRules {
  onePerRoute: boolean;
  dupesClause: boolean;
  shinyClause: boolean;
  nicknameClause: boolean;
  levelCaps: boolean;
  noItemsInBattle: boolean;
  setMode: boolean;
}

export interface NuzState {
  romPath: string | null;
  rules: NuzRules;
  graveyardBox: number | null;
  missed: string[];
  dead: string[];
  alive: string[];
  badges: number | null;
}

export type Origin = "route" | "starter" | "egg" | "trade" | "event" | "gift";
export type CatchKind = "counted" | "dupe" | "shinyBonus" | "extra";

export interface NuzMon {
  key: string;
  slot: Slot;
  species: number;
  speciesName: string;
  nickname: string;
  isNicknamed: boolean;
  level: number;
  shiny: boolean;
  place: string;
  metLocation: number;
  metLocationName: string | null;
  metLevel: number;
  metDate: { year: number; month: number; day: number } | null;
  origin: Origin;
  route: string | null;
  catch: CatchKind | null;
  dead: boolean;
  deathCause: string | null;
}

export interface NuzEncounter {
  species: number;
  minLevel: number;
  maxLevel: number;
  methods: string[];
  name: string;
  owned: boolean;
}

export type RouteStatus = "pending" | "caught" | "missed" | "dupeOnly";

export interface NuzRoute {
  key: string;
  name: string;
  order: number;
  locationIds: number[];
  encounters: NuzEncounter[];
  status: RouteStatus;
  capture: NuzMon | null;
  others: NuzMon[];
  markedMissed: boolean;
}

export interface NuzCap {
  kind: "gym" | "elite" | "champion";
  label: string;
  name: string;
  town: string;
  trainerIds: number[];
  aceSpecies: number;
  aceLevel: number;
  className: string;
  verified: boolean;
  beaten: boolean;
  current: boolean;
}

export interface NuzViolation {
  severity: "error" | "warning" | "info";
  rule: string;
  title: string;
  detail: string;
  mon: string | null;
  route: string | null;
}

export interface NuzReport {
  game: string;
  gameName: string;
  seed: number | null;
  stats: {
    routes: number;
    routesCaught: number;
    routesMissed: number;
    captures: number;
    alive: number;
    dead: number;
    badges: number;
    badgesFromSave: boolean;
    levelCap: number | null;
  };
  caps: NuzCap[];
  routes: NuzRoute[];
  graveyard: NuzMon[];
  graveyardBox: { index: number; name: string; auto: boolean } | null;
  party: NuzMon[];
  others: NuzMon[];
  violations: NuzViolation[];
  rules: NuzRules;
  boxNames: string[];
}

export interface NuzlockeView {
  state: NuzState;
  stateFile: string;
  supported: boolean;
  saveGame: string;
  report: NuzReport | null;
  error: string | null;
}

export const nuzlockeView = () => invoke<NuzlockeView>("nuzlocke_view");
export const setNuzlockeState = (state: NuzState) => invoke<NuzlockeView>("nuzlocke_set_state", { state });
export const linkRom = (rom: string | null) => invoke<NuzlockeView>("nuzlocke_link_rom", { rom });

/** Jeux de ROM qui correspondent à une version de sauvegarde (`SaveView.version`). */
export const ROM_GAMES: Record<string, string[]> = {
  platinum: ["platinum"],
  black_white: ["black", "white"],
};

/** Règles, dans l'ordre d'affichage, avec leur bulle d'aide. */
export const RULES: { id: keyof NuzRules; label: string; tip: string; check: boolean }[] = [
  {
    id: "onePerRoute",
    label: "Une capture par route",
    tip: "La règle de base du Nuzlocke : seul le premier Pokémon rencontré sur chaque route (ou grotte, forêt…) peut être capturé. S'il s'enfuit ou tombe K.O., la route est « ratée ». Kaleido compare le lieu de rencontre enregistré dans chaque Pokémon.",
    check: true,
  },
  {
    id: "dupesClause",
    label: "Clause doublons",
    tip: "Si la première rencontre est une espèce déjà capturée (ou de la même famille d'évolution), on peut l'ignorer et attendre la suivante. Les familles viennent de la ROM : elles suivent les évolutions randomisées.",
    check: true,
  },
  {
    id: "shinyClause",
    label: "Clause chromatique",
    tip: "Un Pokémon chromatique (couleurs rares) peut toujours être capturé, même si la route est déjà faite. Il ne compte pas comme la capture de la route.",
    check: true,
  },
  {
    id: "nicknameClause",
    label: "Clause surnom",
    tip: "Chaque Pokémon capturé doit recevoir un surnom, pour s'y attacher davantage. Kaleido signale les captures sans surnom.",
    check: true,
  },
  {
    id: "levelCaps",
    label: "Niveau maximum",
    tip: "Aucun Pokémon de l'équipe ne doit dépasser le niveau du Pokémon le plus fort du prochain champion d'arène (puis du Conseil 4). Ces niveaux sont lus dans ta ROM : ils tiennent compte de la randomisation.",
    check: true,
  },
  {
    id: "noItemsInBattle",
    label: "Pas de soins en combat",
    tip: "Interdit d'utiliser des Potions ou autres objets de soin pendant un combat. Simple rappel : la sauvegarde ne garde pas de trace des combats.",
    check: false,
  },
  {
    id: "setMode",
    label: "Mode « Set »",
    tip: "Quand le dresseur adverse envoie un nouveau Pokémon, le jeu ne propose pas de changer le tien (option « Style de combat : Défini » dans les options du jeu). Simple rappel, non vérifiable dans la sauvegarde.",
    check: false,
  },
];

export const STATUS_LABEL: Record<RouteStatus, string> = {
  pending: "Pas encore",
  caught: "Capturé",
  missed: "Raté",
  dupeOnly: "Doublon",
};

export const ORIGIN_LABEL: Record<Origin, string> = {
  route: "Capture",
  starter: "Starter",
  egg: "Œuf",
  trade: "Échange",
  event: "Événement",
  gift: "Cadeau / fixe",
};

export const CATCH_LABEL: Record<CatchKind, string> = {
  counted: "Compte",
  dupe: "Doublon",
  shinyBonus: "Chromatique en plus",
  extra: "Capture de trop",
};

export const monName = (m: NuzMon) => (m.isNicknamed ? m.nickname : m.speciesName);
