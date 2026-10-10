import type { Dex } from "./data";
import type { Collection } from "./slots";
import type { CatchEntry, SpeciesTag, TypeId } from "./types";

/** Ce que les succès regardent. */
export interface AchievementContext {
  dex: Dex;
  collection: Collection;
  entries: CatchEntry[];
}

export type AchievementCategory = "Living Dex" | "Espèces" | "Générations" | "Chromatiques" | "Balls" | "Types" | "Raretés" | "Jeux" | "Kaleido";

export interface Achievement {
  id: string;
  category: AchievementCategory;
  title: string;
  description: string;
  /** Progression [actuel, objectif]. */
  progress(ctx: AchievementContext): [number, number];
}

const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"];
const GEN_REGIONS = ["", "Kanto", "Johto", "Hoenn", "Sinnoh", "Unys", "Kalos", "Alola", "Galar", "Paldea"];
const TYPES: TypeId[] = ["normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel", "fire", "water", "grass", "electric", "psychic", "ice", "dragon", "dark", "fairy"];

const caughtSlots = (ctx: AchievementContext) => ctx.collection.totals.caught;
const speciesWithTag = (ctx: AchievementContext, tag: SpeciesTag) => ctx.dex.speciesList.filter((s) => s.tags.includes(tag));
const caughtOf = (ctx: AchievementContext, ids: number[]): [number, number] => [ids.filter((id) => ctx.collection.speciesCaught.has(id)).length, ids.length];

function slotsAchievement(n: number, title: string): Achievement {
  return { id: `slots-${n}`, category: "Living Dex", title, description: n === 1 ? "Remplir une case de la Living Dex." : `Remplir ${n.toLocaleString("fr-FR")} cases de la Living Dex.`, progress: (c) => [caughtSlots(c), n] };
}

function percentAchievement(p: number, title: string): Achievement {
  return {
    id: `percent-${p}`,
    category: "Living Dex",
    title,
    description: p === 100 ? "Remplir toutes les cases de la Living Dex avec les règles choisies." : `Remplir ${p} % des cases de la Living Dex.`,
    progress: (c) => [caughtSlots(c), Math.ceil((c.collection.totals.slots * p) / 100)],
  };
}

function speciesAchievement(n: number, title: string, description: string): Achievement {
  return { id: `species-${n}`, category: "Espèces", title, description, progress: (c) => [c.collection.totals.speciesCaught, n] };
}

function genAchievement(gen: number): Achievement {
  return {
    id: `gen-${gen}`,
    category: "Générations",
    title: `Pokédex de ${GEN_REGIONS[gen]}`,
    description: `Posséder toutes les espèces apparues en Gen ${ROMAN[gen]}.`,
    progress: (c) => caughtOf(c, c.dex.speciesList.filter((s) => s.gen === gen).map((s) => s.id)),
  };
}

function shinyAchievement(n: number, title: string): Achievement {
  return { id: `shiny-${n}`, category: "Chromatiques", title, description: n === 1 ? "Posséder un Pokémon chromatique." : `Remplir ${n} cases avec un Pokémon chromatique.`, progress: (c) => [c.collection.totals.shiny, n] };
}

function ballsAchievement(n: number, title: string): Achievement {
  return {
    id: `balls-${n}`,
    category: "Balls",
    title,
    description: `Posséder des Pokémon capturés dans ${n} Balls différentes.`,
    progress: (c) => [new Set(c.entries.map((e) => e.ball).filter((b): b is number => !!b)).size, n],
  };
}

function tagAchievement(id: string, tag: SpeciesTag, title: string, description: string): Achievement {
  return { id, category: "Raretés", title, description, progress: (c) => caughtOf(c, speciesWithTag(c, tag).map((s) => s.id)) };
}

function gamesAchievement(n: number, title: string): Achievement {
  return {
    id: `games-${n}`,
    category: "Jeux",
    title,
    description: `Posséder des Pokémon venus de ${n} jeux différents.`,
    progress: (c) => [new Set(c.entries.map((e) => e.game).filter(Boolean)).size, n],
  };
}

export const ACHIEVEMENTS: Achievement[] = [
  slotsAchievement(1, "Première case"),
  slotsAchievement(30, "Une boîte pleine"),
  slotsAchievement(100, "Centurie"),
  slotsAchievement(250, "Collection sérieuse"),
  slotsAchievement(500, "Demi-millier"),
  slotsAchievement(1000, "Millier"),
  percentAchievement(25, "Un quart du chemin"),
  percentAchievement(50, "À mi-chemin"),
  percentAchievement(75, "Dernière ligne droite"),
  percentAchievement(100, "Living Dex complète"),
  speciesAchievement(151, "Les 151", "Posséder 151 espèces différentes."),
  speciesAchievement(386, "Trois régions", "Posséder 386 espèces différentes."),
  speciesAchievement(649, "Cinq régions", "Posséder 649 espèces différentes."),
  speciesAchievement(809, "Sept régions", "Posséder 809 espèces différentes."),
  speciesAchievement(1025, "Tous les Pokémon", "Posséder les 1 025 espèces."),
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map(genAchievement),
  shinyAchievement(1, "Éclat"),
  shinyAchievement(10, "Chasseur de chromatiques"),
  shinyAchievement(50, "Collection scintillante"),
  shinyAchievement(151, "Kanto en couleurs"),
  ballsAchievement(5, "Trousse de Balls"),
  ballsAchievement(12, "Collectionneur de Balls"),
  ballsAchievement(20, "Balls en tout genre"),
  {
    id: "types-all",
    category: "Types",
    title: "Arc-en-ciel",
    description: "Posséder au moins un Pokémon de chacun des 18 types.",
    progress: (c) => {
      const seen = new Set<TypeId>();
      for (const id of c.collection.speciesCaught) for (const t of c.dex.species(id)?.forms[0]?.types ?? []) seen.add(t);
      return [TYPES.filter((t) => seen.has(t)).length, TYPES.length];
    },
  },
  {
    id: "regional-10",
    category: "Types",
    title: "Voyageur",
    description: "Posséder 10 Pokémon en forme régionale.",
    progress: (c) => [new Set(c.entries.filter((e) => c.dex.form(e.species, e.form)?.cat === "regional").map((e) => `${e.species}-${e.form}`)).size, 10],
  },
  tagAchievement("starters", "starter", "Premier choix", "Posséder tous les Pokémon de départ."),
  tagAchievement("legendaries", "legendary", "Panthéon", "Posséder tous les Pokémon légendaires."),
  tagAchievement("mythicals", "mythical", "Mythes et légendes", "Posséder tous les Pokémon fabuleux."),
  tagAchievement("ultra-beasts", "ultra-beast", "Ultra-Dimension", "Posséder toutes les Ultra-Chimères."),
  tagAchievement("paradox", "paradox", "Paradoxe", "Posséder tous les Pokémon Paradoxe."),
  tagAchievement("fossils", "fossil", "Paléontologue", "Posséder tous les Pokémon Fossiles."),
  gamesAchievement(3, "Trois cartouches"),
  gamesAchievement(8, "Grand voyageur"),
  gamesAchievement(15, "Toutes les époques"),
  {
    id: "auto-first",
    category: "Kaleido",
    title: "Lecture automatique",
    description: "Remplir une case grâce à une sauvegarde lue par Kaleido.",
    progress: (c) => [Math.min(1, c.entries.filter((e) => e.auto).length), 1],
  },
  {
    id: "manual-first",
    category: "Kaleido",
    title: "Carnet de terrain",
    description: "Noter à la main un Pokémon d'un jeu que Kaleido ne lit pas.",
    progress: (c) => [Math.min(1, c.entries.filter((e) => !e.auto).length), 1],
  },
];

/** Rangs de collectionneur, selon la part des cases remplies. */
export const RANKS: { min: number; name: string }[] = [
  { min: 0, name: "Débutant" },
  { min: 5, name: "Apprenti" },
  { min: 15, name: "Collectionneur" },
  { min: 30, name: "Expert" },
  { min: 50, name: "Spécialiste" },
  { min: 70, name: "Maître" },
  { min: 90, name: "Champion" },
  { min: 100, name: "Légende" },
];

export function rankOf(percent: number): { index: number; name: string; next: { min: number; name: string } | null } {
  let index = 0;
  RANKS.forEach((r, i) => {
    if (percent >= r.min) index = i;
  });
  return { index, name: RANKS[index].name, next: RANKS[index + 1] ?? null };
}
