/**
 * Modèle de la Living Dex : quelles cases existent selon les règles choisies, et quelle case
 * remplit chaque Pokémon. Porté de Pelagix (`src/renderer/src/domain/slots.ts`, HydrosPlays,
 * GPLv3) ; toutes les fonctions sont pures.
 *
 * Clés des cases
 *   "25"          forme de base de l'espèce 25
 *   "26-1"        forme 1 de l'espèce 26 (sa catégorie a une case)
 *   "25:m" "25:f" case séparée par sexe (règle `genderDiffs`)
 *   "869-3:v2"    une variante d'une forme (règle `alcremieSweets`)
 *   "25:gmax"     case Gigamax d'une forme (règle `gmax`)
 */

import type { Dex } from "./data";
import type { CatchEntry, DexRules, FormCategory, FormSummary, SpeciesSummary } from "./types";

export interface LivingSlot {
  key: string;
  species: number;
  /** Forme affichée par la case (index PKHeX). */
  form: number;
  variant?: number;
  gender?: "m" | "f";
  gmax?: boolean;
  label: string;
  cat: FormCategory;
}

export interface Collection {
  rules: DexRules;
  slots: LivingSlot[];
  /** Pokémon par case, dans l'ordre reçu. Seules les cases remplies ont une clé. */
  bySlot: Map<string, CatchEntry[]>;
  caught: Set<string>;
  caughtShiny: Set<string>;
  speciesCaught: Set<number>;
  speciesShiny: Set<number>;
  slotByKey: Map<string, LivingSlot>;
  slotsBySpecies: Map<number, LivingSlot[]>;
  slotOfEntry: Map<string, string>;
  totals: { slots: number; caught: number; shiny: number; species: number; speciesCaught: number };
}

export const DEFAULT_RULES: DexRules = {
  regional: true,
  genderForms: true,
  genderDiffs: true,
  cosmetic: true,
  changeable: true,
  heldItem: true,
  fusion: false,
  event: false,
  partner: false,
  alcremieSweets: false,
  mega: false,
  battle: false,
  gmax: false,
};

export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof DexRules)[];

export const RULE_INFO: Record<keyof DexRules, { label: string; description: string }> = {
  regional: { label: "Formes régionales", description: "Les formes d'Alola, de Galar, de Hisui et de Paldea ont leur propre case." },
  genderForms: { label: "Formes liées au sexe", description: "Mistigrix, Wimessir, Paragruel et Fragroin mâle et femelle sont deux cases." },
  genderDiffs: { label: "Différences mâle / femelle", description: "Une case ♂ et une case ♀ pour les espèces dont les deux sexes se distinguent (Pikachu, Hippodocus…)." },
  cosmetic: { label: "Apparences", description: "Lettres de Zarbi, motifs de Prismillon, crèmes de Charmilly et autres apparences fixes." },
  changeable: { label: "Formes modifiables", description: "Formes que l'on change à volonté : Motisma, Deoxys, Shaymin, Plumeline…" },
  heldItem: { label: "Formes liées à un objet", description: "Formes gardées seulement en tenant un objet : plaques d'Arceus, ROM de Silvallié, modules de Genesect, masques d'Ogerpon, Forme Originelle." },
  fusion: { label: "Fusions", description: "Fusions de Kyurem, Necrozma et Sylveroy." },
  event: { label: "Formes d'évènement", description: "Formes distribuées seulement, comme Pikachu à casquette ou Prismillon Poké Ball." },
  partner: { label: "Partenaires", description: "Pikachu et Évoli partenaires de Let's Go, qui ne quittent pas leur jeu." },
  alcremieSweets: { label: "Toutes les Charmilly", description: "Les 63 combinaisons de crème et de décoration au lieu des 9 crèmes." },
  mega: { label: "Méga-Évolutions", description: "Méga-Évolutions et Primo-Résurgences (elles ne restent pas en boîte)." },
  battle: { label: "Formes de combat", description: "Autres formes qui n'existent qu'en combat, et Pokémon Dominants." },
  gmax: { label: "Gigamax", description: "Une case de plus pour chaque forme capable de se Gigamaxer." },
};

const allRules = (on: boolean): DexRules => Object.fromEntries(RULE_KEYS.map((k) => [k, on])) as unknown as DexRules;

export type RulePresetId = "species" | "forms" | "completionist";

export const RULE_PRESETS: Record<RulePresetId, { id: RulePresetId; label: string; description: string; rules: DexRules }> = {
  species: { id: "species", label: "Espèces", description: "Une case par espèce : n'importe quelle forme la remplit.", rules: allRules(false) },
  forms: { id: "forms", label: "Formes", description: "Toutes les formes qui restent en boîte : régionales, liées au sexe, apparences, formes modifiables.", rules: { ...DEFAULT_RULES } },
  completionist: {
    id: "completionist",
    label: "Complétionniste",
    description: "Tout : formes d'évènement et partenaires, fusions, Méga-Évolutions, formes de combat, Gigamax et les 63 Charmilly.",
    rules: allRules(true),
  },
};

export function matchRulePreset(rules: DexRules): RulePresetId | null {
  for (const p of Object.values(RULE_PRESETS)) if (RULE_KEYS.every((k) => p.rules[k] === rules[k])) return p.id;
  return null;
}

const CATEGORY_RULE: Record<FormCategory, keyof DexRules | boolean> = {
  base: true,
  regional: "regional",
  gender: "genderForms",
  cosmetic: "cosmetic",
  changeable: "changeable",
  fusion: "fusion",
  event: "event",
  partner: "partner",
  mega: "mega",
  battle: "battle",
  hidden: false,
};

/** Espèces dont les formes modifiables ne tiennent qu'avec un objet (règle `heldItem`). */
export const HELD_ITEM_FORM_SPECIES = new Set([483, 484, 487, 493, 649, 773, 1017]);

function formRule(species: SpeciesSummary, form: FormSummary): keyof DexRules | boolean {
  if (form.cat === "changeable" && HELD_ITEM_FORM_SPECIES.has(species.id)) return "heldItem";
  return CATEGORY_RULE[form.cat];
}

const isBase = (species: SpeciesSummary, form: FormSummary) => species.forms[0] === form;

export function isFormSlotted(species: SpeciesSummary, form: FormSummary, rules: DexRules): boolean {
  if (isBase(species, form)) return true;
  if (form.present.length === 0) return false;
  const rule = formRule(species, form);
  return typeof rule === "boolean" ? rule : rules[rule];
}

const formKey = (species: SpeciesSummary, form: FormSummary) => (isBase(species, form) ? String(species.id) : `${species.id}-${form.f}`);
const splitsByVariant = (form: FormSummary, rules: DexRules) => rules.alcremieSweets && !!form.variants?.length;

function splitsByGender(species: SpeciesSummary, form: FormSummary, rules: DexRules): boolean {
  if (!rules.genderDiffs || !species.genderDiff || !form.female || form.cat === "gender") return false;
  if (splitsByVariant(form, rules)) return false;
  return !(rules.genderForms && species.forms.some((f) => f.cat === "gender"));
}

function speciesSlots(species: SpeciesSummary, rules: DexRules): LivingSlot[] {
  const slotted = species.forms.filter((form) => isFormSlotted(species, form, rules));
  // Une forme de base seule représente toute l'espèce : elle porte le nom de l'espèce.
  const lone = slotted.length === 1;
  const out: LivingSlot[] = [];
  for (const form of slotted) {
    const key = formKey(species, form);
    const generic = lone && isBase(species, form);
    const name = generic ? species.name : form.full;
    const make = (suffix: string, label: string, extra: { variant?: number; gender?: "m" | "f"; gmax?: boolean; shown?: FormSummary }) => {
      const shown = extra.shown ?? form;
      const gender = extra.gender ?? (generic ? undefined : form.gender);
      out.push({
        key: key + suffix,
        species: species.id,
        form: generic ? 0 : shown.f,
        ...(extra.variant !== undefined && { variant: extra.variant }),
        ...(gender !== undefined && { gender }),
        ...(extra.gmax && { gmax: true }),
        label,
        cat: form.cat,
      });
    };
    if (splitsByVariant(form, rules)) {
      for (const v of form.variants ?? []) make(`:v${v.id}`, `${name} · ${v.name}`, { variant: v.id });
    } else if (splitsByGender(species, form, rules)) {
      for (const gender of ["m", "f"] as const) {
        const shown = species.forms.find((f) => f.gender === gender && (f === form || f.cat === "gender"));
        const label = shown && !generic ? shown.full : `${name} ${gender === "m" ? "♂" : "♀"}`;
        make(`:${gender}`, label, { gender, shown });
      }
    } else {
      make("", name, {});
    }
    if (rules.gmax && form.gmax) make(":gmax", `${name} Gigamax`, { gmax: true });
  }
  return out;
}

const rulesKey = (rules: DexRules) => RULE_KEYS.map((k) => (rules[k] ? "1" : "0")).join("");
const slotCache = new WeakMap<Dex, Map<string, LivingSlot[]>>();

/** Toutes les cases, dans l'ordre national puis l'ordre des formes (mémorisé par jeu de règles). */
export function buildSlots(dex: Dex, rules: DexRules): LivingSlot[] {
  let perDex = slotCache.get(dex);
  if (!perDex) slotCache.set(dex, (perDex = new Map()));
  const k = rulesKey(rules);
  let slots = perDex.get(k);
  if (!slots) {
    slots = dex.speciesList.flatMap((s) => speciesSlots(s, rules));
    perDex.set(k, slots);
  }
  return slots;
}

/**
 * Case la plus précise remplie par un Pokémon, `null` si l'espèce est inconnue. Une forme sans
 * case à elle compte pour la forme de base ; un sexe inconnu ou asexué va dans la case ♂.
 */
export function slotKeyFor(entry: Pick<CatchEntry, "species" | "form" | "variant" | "gender" | "gmax">, dex: Dex, rules: DexRules): string | null {
  const species = dex.species(entry.species);
  const base = species?.forms[0];
  if (!species || !base) return null;
  const own = dex.form(entry.species, entry.form);
  const form = own && isFormSlotted(species, own, rules) ? own : base;
  const key = formKey(species, form);
  if (entry.gmax && rules.gmax && form.gmax) return `${key}:gmax`;
  if (splitsByVariant(form, rules)) {
    const variants = form.variants ?? [];
    const v = variants.find((x) => x.id === entry.variant) ?? variants[0];
    if (v) return `${key}:v${v.id}`;
  }
  if (splitsByGender(species, form, rules)) return `${key}:${own?.gender ?? (entry.gender === "f" ? "f" : "m")}`;
  return key;
}

export function computeCollection(dex: Dex, entries: readonly CatchEntry[], rules: DexRules): Collection {
  const slots = buildSlots(dex, rules);
  const slotByKey = new Map<string, LivingSlot>();
  const slotsBySpecies = new Map<number, LivingSlot[]>();
  for (const s of slots) {
    slotByKey.set(s.key, s);
    const list = slotsBySpecies.get(s.species);
    if (list) list.push(s);
    else slotsBySpecies.set(s.species, [s]);
  }
  const bySlot = new Map<string, CatchEntry[]>();
  const caughtShiny = new Set<string>();
  const speciesCaught = new Set<number>();
  const speciesShiny = new Set<number>();
  const slotOfEntry = new Map<string, string>();
  for (const e of entries) {
    const key = slotKeyFor(e, dex, rules);
    if (key === null || !slotByKey.has(key)) continue;
    const list = bySlot.get(key);
    if (list) list.push(e);
    else bySlot.set(key, [e]);
    slotOfEntry.set(e.id, key);
    speciesCaught.add(e.species);
    if (e.shiny) {
      caughtShiny.add(key);
      speciesShiny.add(e.species);
    }
  }
  const caught = new Set(bySlot.keys());
  return {
    rules,
    slots,
    bySlot,
    caught,
    caughtShiny,
    speciesCaught,
    speciesShiny,
    slotByKey,
    slotsBySpecies,
    slotOfEntry,
    totals: { slots: slots.length, caught: caught.size, shiny: caughtShiny.size, species: dex.speciesList.length, speciesCaught: speciesCaught.size },
  };
}
