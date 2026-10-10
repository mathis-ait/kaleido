import { shallowRef } from "vue";
import type { DexIndex, EncounterKind, FormCategory, FormSummary, GameDef, SpeciesDetail, SpeciesSummary, SpeciesTag, TypeId } from "./types";

/**
 * Données de la Living Dex : index (jeux, espèces, formes) chargé une fois, fiches « où le
 * trouver » chargées à la demande. Source : Pelagix (HydrosPlays, GPLv3), d'après PKHeX et PokeAPI.
 */
export interface Dex {
  index: DexIndex;
  speciesList: SpeciesSummary[];
  games: GameDef[];
  species(id: number): SpeciesSummary | undefined;
  form(species: number, form: number): FormSummary | undefined;
  game(id: string): GameDef | undefined;
  /** Jeu d'après la `GameVersion` de PKHeX. */
  gameByVersion(version: number): GameDef | undefined;
}

export const dex = shallowRef<Dex | null>(null);
let loading: Promise<Dex> | null = null;

export function loadDex(): Promise<Dex> {
  loading ??= fetch("/livedex/dex.json")
    .then((r) => {
      if (!r.ok) throw new Error(`données de la Living Dex introuvables (${r.status})`);
      return r.json() as Promise<DexIndex>;
    })
    .then((index) => {
      const forms = new Map<string, FormSummary>();
      for (const s of index.species) for (const f of s.forms) forms.set(`${s.id}-${f.f}`, f);
      const games = new Map(index.games.map((g) => [g.id, g]));
      const byVersion = new Map<number, GameDef>();
      for (const g of index.games) if (g.version !== null && !byVersion.has(g.version)) byVersion.set(g.version, g);
      const d: Dex = {
        index,
        speciesList: index.species,
        games: index.games,
        species: (id) => index.species[id - 1],
        form: (s, f) => forms.get(`${s}-${f}`),
        game: (id) => games.get(id),
        gameByVersion: (v) => byVersion.get(v),
      };
      dex.value = d;
      return d;
    })
    .catch((e) => {
      loading = null;
      throw e;
    });
  return loading;
}

const details = new Map<number, Promise<SpeciesDetail>>();
export function loadSpecies(id: number): Promise<SpeciesDetail> {
  let p = details.get(id);
  if (!p) {
    p = fetch(`/livedex/species/${id}.json`).then((r) => {
      if (!r.ok) throw new Error(`fiche ${id} introuvable`);
      return r.json() as Promise<SpeciesDetail>;
    });
    p.catch(() => details.delete(id));
    details.set(id, p);
  }
  return p;
}

/** Jeu de la sauvegarde (valeur `SaveVersion` du moteur) : pour les Gen 1 et 2, sans jeu d'origine. */
export const SAVE_VERSION_GAME: Record<string, string> = {
  red_blue: "red",
  yellow: "yellow",
  gold_silver: "gold",
  crystal: "crystal",
  ruby_sapphire: "ruby",
  emerald: "emerald",
  fire_red_leaf_green: "firered",
  diamond_pearl: "diamond",
  platinum: "platinum",
  heart_gold_soul_silver: "heartgold",
  black_white: "black",
  black2_white2: "black2",
  x_y: "x",
  omega_ruby_alpha_sapphire: "omegaruby",
  sun_moon: "sun",
  ultra_sun_ultra_moon: "ultrasun",
};

/** Noms des Balls (identifiants PKHeX). */
export const BALLS: string[] = [
  "",
  "Master Ball",
  "Hyper Ball",
  "Super Ball",
  "Poké Ball",
  "Safari Ball",
  "Filet Ball",
  "Scuba Ball",
  "Faiblo Ball",
  "Bis Ball",
  "Chrono Ball",
  "Luxe Ball",
  "Honor Ball",
  "Sombre Ball",
  "Soin Ball",
  "Rapide Ball",
  "Mémoire Ball",
  "Speed Ball",
  "Niveau Ball",
  "Appât Ball",
  "Masse Ball",
  "Love Ball",
  "Copain Ball",
  "Lune Ball",
  "Compét'Ball",
  "Parc Ball",
  "Rêve Ball",
  "Ultra Ball",
  "Étrange Ball",
  "Poké Ball (Hisui)",
  "Super Ball (Hisui)",
  "Hyper Ball (Hisui)",
  "Plume Ball",
  "Aile Ball",
  "Jet Ball",
  "Masse Ball (Hisui)",
  "Mégamasse Ball",
  "Gigamasse Ball",
  "Origine Ball",
];

export const TYPE_NAMES: Record<TypeId, string> = {
  normal: "Normal",
  fighting: "Combat",
  flying: "Vol",
  poison: "Poison",
  ground: "Sol",
  rock: "Roche",
  bug: "Insecte",
  ghost: "Spectre",
  steel: "Acier",
  fire: "Feu",
  water: "Eau",
  grass: "Plante",
  electric: "Électrik",
  psychic: "Psy",
  ice: "Glace",
  dragon: "Dragon",
  dark: "Ténèbres",
  fairy: "Fée",
  stellar: "Stellaire",
};

export const TAG_NAMES: Record<SpeciesTag, string> = {
  legendary: "Légendaire",
  mythical: "Fabuleux",
  baby: "Bébé",
  starter: "Starter",
  fossil: "Fossile",
  "pseudo-legendary": "Pseudo-légendaire",
  "ultra-beast": "Ultra-Chimère",
  paradox: "Paradoxe",
};

export const KIND_NAMES: Record<EncounterKind, string> = {
  wild: "Sauvage",
  static: "Fixe",
  gift: "Don",
  egg: "Œuf",
  trade: "Échange",
  raid: "Raid",
  tera: "Raid Téracristal",
  outbreak: "Apparition massive",
  shadow: "Obscur",
  walker: "Pokéwalker",
  dream: "Monde des Rêves",
  event: "Évènement",
};

export const CATEGORY_NAMES: Record<FormCategory, string> = {
  base: "Forme de base",
  regional: "Forme régionale",
  gender: "Forme liée au sexe",
  cosmetic: "Apparence",
  changeable: "Forme modifiable",
  fusion: "Fusion",
  event: "Forme d'évènement",
  partner: "Partenaire",
  mega: "Méga-Évolution",
  battle: "Forme de combat",
  hidden: "Variante invisible",
};

export const SYSTEM_NAMES: Record<GameDef["system"], string> = {
  gb: "Game Boy",
  gbc: "Game Boy Color",
  n64: "Nintendo 64",
  gba: "Game Boy Advance",
  gcn: "GameCube",
  nds: "Nintendo DS",
  "3ds": "Nintendo 3DS",
  switch: "Nintendo Switch",
  mobile: "Mobile",
};

const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"];
export const genLabel = (g: number) => (g ? `Gen ${ROMAN[g] ?? g}` : "Services");

/** Texte sans accents ni majuscules, pour la recherche. */
export const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Date du jour au format AAAA-MM-JJ (heure locale). */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function frDate(iso: string | undefined): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return iso;
  return new Date(y, m - 1, d).toLocaleDateString("fr-FR", { day: "numeric", month: "short", year: "numeric" });
}
