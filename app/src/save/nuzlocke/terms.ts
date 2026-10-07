import type { GlossaryEntry } from "../../glossary";
import { RULES } from "./types";

/** Bulles « i » du Nuzlocke (glossaire central, préfixe « nuzlocke. ») : une par règle, puis les notions de suivi. */
export const NUZLOCKE_TERMS: Record<string, GlossaryEntry> = {
  ...Object.fromEntries(RULES.map((r) => [r.id, { title: r.label, text: r.tip }])),
};
