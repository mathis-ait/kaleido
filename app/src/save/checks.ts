import type { SlotView } from "../types";
import { cachedReport, CHECK_TERMS, type LegalityReport } from "./legality";
import { BALLS, VERSIONS } from "./refdata";

/** Résultat d'une vérification de légalité. */
export interface Check {
  level: "error" | "warn" | "ok";
  title: string;
  detail: string;
  /** Onglet de la fiche où corriger. */
  tab?: string;
  /** Terme du glossaire expliqué par une bulle « i ». */
  term?: string;
  code?: string;
}

const LEVEL = { invalid: "error", fishy: "warn", valid: "ok" } as const;

/** Vérifications du moteur de légalité, converties pour l'affichage. */
export function checksFromReport(r: LegalityReport): Check[] {
  const out: Check[] = r.checks.map((c) => ({
    level: LEVEL[c.severity],
    title: c.title,
    detail: c.detail,
    tab: c.tab ?? undefined,
    term: CHECK_TERMS[c.code],
    code: c.code,
  }));
  if (!out.some((c) => c.level !== "ok")) out.push({ level: "ok", title: "Aucun problème trouvé", detail: "Le Pokémon correspond à une rencontre possible et ses données sont cohérentes." });
  return out;
}

/** Rapport complet du moteur s'il est déjà connu (sinon l'analyse est lancée). */
export function legalityOf(p: SlotView): LegalityReport | null {
  return cachedReport(p);
}

/**
 * Vérifications d'un Pokémon : analyse de légalité du moteur (rencontres, attaques, PID…),
 * ou, le temps qu'elle arrive, contrôles rapides de cohérence.
 */
export function checkPokemon(p: SlotView, saveGen: number, itemNames: Record<number, string>): Check[] {
  const r = cachedReport(p);
  return r ? checksFromReport(r) : quickChecks(p, saveGen, itemNames);
}

/** Contrôles rapides, sans base de rencontres (affichés pendant l'analyse). */
function quickChecks(p: SlotView, saveGen: number, itemNames: Record<number, string>): Check[] {
  const out: Check[] = [];
  const err = (title: string, detail: string, tab?: string) => out.push({ level: "error", title, detail, tab });
  const warn = (title: string, detail: string, tab?: string) => out.push({ level: "warn", title, detail, tab });

  if (!p.checksumValid) err("Données abîmées", "La somme de contrôle du Pokémon ne correspond pas : le jeu le verra comme un « Œuf corrompu ».");

  const evTotal = p.evs.reduce((a, b) => a + b, 0);
  if (evTotal > 510) err("Trop d'EV", `${evTotal} EV au total, 510 au maximum.`, "stats");

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

  if (saveGen === 4 && p.abilityNumber === 4) err("Talent caché en Gen 4", "Les talents cachés n'existent qu'à partir de la Gen 5.");
  if (p.heldItem && !itemNames[p.heldItem]) warn("Objet inconnu", `Objet n°${p.heldItem} : il n'existe peut-être pas dans ce jeu.`);
  if (p.language === 0 || p.language === 6 || p.language > 10) err("Langue invalide", `Langue n°${p.language}.`);
  if (p.pokerusStrain === 0 && p.pokerusDays > 0) err("Pokérus incohérent", "Des jours d'infection sans souche de virus.", "extras");

  if (!out.length) out.push({ level: "ok", title: "Analyse en cours…", detail: "Recherche des rencontres possibles." });
  return out;
}

/** Résumé : verdict comme PKHeX (Légal / Douteux / Illégal) et nombre de problèmes. */
export function checkSummary(list: Check[]) {
  const errors = list.filter((c) => c.level === "error").length;
  const warns = list.filter((c) => c.level === "warn").length;
  const plural = (n: number, w: string) => `${n} ${w}${n > 1 ? "s" : ""}`;
  if (errors) return { level: "error" as const, label: "Illégal", text: `${plural(errors, "problème")}${warns ? ` · ${plural(warns, "remarque")}` : ""}` };
  if (warns) return { level: "warn" as const, label: "Douteux", text: plural(warns, "remarque") };
  return { level: "ok" as const, label: "Légal", text: "Aucun problème" };
}
