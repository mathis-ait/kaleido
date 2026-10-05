import type { SlotView } from "../types";
import { BALLS, VERSIONS } from "./refdata";

/** Résultat d'une vérification de cohérence. */
export interface Check {
  level: "error" | "warn" | "ok";
  title: string;
  detail: string;
  /** Onglet de la fiche où corriger. */
  tab?: string;
}

/**
 * Vérifications de cohérence d'un Pokémon, sans base de rencontres :
 * elles repèrent les erreurs évidentes, pas toutes les illégalités (voir la bulle « Vérifications »).
 */
export function checkPokemon(p: SlotView, saveGen: number, itemNames: string[]): Check[] {
  const out: Check[] = [];
  const err = (title: string, detail: string, tab?: string) => out.push({ level: "error", title, detail, tab });
  const warn = (title: string, detail: string, tab?: string) => out.push({ level: "warn", title, detail, tab });

  if (!p.checksumValid) err("Données abîmées", "La somme de contrôle du Pokémon ne correspond pas : le jeu le verra comme un « Œuf corrompu ».");

  const evTotal = p.evs.reduce((a, b) => a + b, 0);
  if (evTotal > 510) err("Trop d'EV", `${evTotal} EV au total, 510 au maximum.`, "stats");
  if (p.evs.some((e) => e > 252)) warn("EV au-delà de 252", "Au-delà de 252, les points d'une statistique ne servent plus.", "stats");

  const moves = p.moves.filter((m) => m !== 0);
  if (!moves.length) err("Aucune attaque", "Un Pokémon doit connaître au moins une attaque.", "moves");
  if (new Set(moves).size !== moves.length) err("Attaque en double", "La même attaque apparaît deux fois.", "moves");

  if (!p.isEgg && p.metLevel > p.level) err("Niveau de rencontre trop haut", `Rencontré au niveau ${p.metLevel}, mais il est niveau ${p.level}.`, "met");

  const version = VERSIONS.find((v) => v.id === p.version);
  if (!version) warn("Jeu d'origine inconnu", `Version n°${p.version}.`, "met");
  else if (version.gen > saveGen) err("Jeu d'origine trop récent", `${version.name} est un jeu de Gen ${version.gen}, plus récent que cette sauvegarde.`, "met");

  const ball = BALLS.find((b) => b.id === p.ball);
  if (!ball) err("Ball inconnue", `Ball n°${p.ball}.`, "met");
  else if (ball.since > saveGen) err("Ball inexistante dans ce jeu", `${ball.name} n'existe qu'à partir de la Gen ${ball.since}.`, "met");
  else if (ball.until && saveGen > ball.until && version && version.gen > ball.until)
    err("Ball impossible", `${ball.name} n'existe qu'en Gen ${ball.until}.`, "met");

  if (saveGen === 4 && p.abilityNumber === 4) err("Talent caché en Gen 4", "Les talents cachés n'existent qu'à partir de la Gen 5.");

  if (p.heldItem && !itemNames[p.heldItem]) warn("Objet inconnu", `Objet n°${p.heldItem} : il n'existe peut-être pas dans ce jeu.`);

  if (p.shiny && p.fatefulEncounter) warn("Événement chromatique", "Beaucoup de Pokémon d'événement ne peuvent pas être chromatiques (« shiny lock »). Vérifie la distribution.", "trainer");

  if (p.language === 0 || p.language === 6 || p.language > 10) err("Langue invalide", `Langue n°${p.language}.`);

  if (p.pokerusStrain === 0 && p.pokerusDays > 0) err("Pokérus incohérent", "Des jours d'infection sans souche de virus.", "extras");

  if (!p.metDate && !p.isEgg) warn("Date de rencontre absente", "Les Pokémon capturés ont normalement une date.", "met");

  if (!out.length) out.push({ level: "ok", title: "Aucun problème trouvé", detail: "Les données sont cohérentes." });
  return out;
}

export function checkSummary(list: Check[]) {
  const errors = list.filter((c) => c.level === "error").length;
  const warns = list.filter((c) => c.level === "warn").length;
  if (errors) return { level: "error" as const, text: `${errors} problème${errors > 1 ? "s" : ""}${warns ? ` · ${warns} remarque${warns > 1 ? "s" : ""}` : ""}` };
  if (warns) return { level: "warn" as const, text: `${warns} remarque${warns > 1 ? "s" : ""}` };
  return { level: "ok" as const, text: "Cohérent" };
}
