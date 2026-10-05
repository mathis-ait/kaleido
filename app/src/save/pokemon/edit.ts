import { patchPokemon, saveState } from "../../saveStore";
import type { PokemonPatch } from "../../types";

/** Applique une modification au Pokémon affiché dans la fiche (annulable avec Ctrl+Z). */
export async function apply(patch: PokemonPatch) {
  const p = saveState.selected;
  if (!p) return false;
  return patchPokemon(p.slot, patch);
}

/** Valeur numérique bornée d'un champ, `null` si invalide. */
export function num(v: unknown, min: number, max: number): number | null {
  const n = Number(v);
  if (!Number.isFinite(n) || String(v).trim() === "") return null;
  return Math.min(max, Math.max(min, Math.round(n)));
}

export const hex = (n: number) => (n >>> 0).toString(16).toUpperCase().padStart(8, "0");
