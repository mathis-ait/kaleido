import type { Dex } from "./data";
import { slotKeyFor, type LivingSlot } from "./slots";
import type { DexRules, Specimen } from "./types";

/** Plan de rangement : où chaque case de la Living Dex doit se trouver dans les boîtes d'une sauvegarde. */

export const BOX_SIZE = 30;

export interface Position {
  box: number;
  index: number;
}

export interface ArrangeStep {
  slot: LivingSlot;
  /** Place visée : la n-ième case de la Living Dex devient la n-ième case des boîtes. */
  target: Position;
  /** `ok` : déjà à sa place ; `move` : à déplacer ; `missing` : la sauvegarde ne l'a pas. */
  status: "ok" | "move" | "missing";
  specimen?: Specimen;
  /** Où il est aujourd'hui (`null` : dans l'équipe). */
  from?: Position | null;
  /** Pokémon qui occupe la place visée aujourd'hui, s'il y en a un autre. */
  occupant?: Specimen;
}

export interface ArrangePlan {
  steps: ArrangeStep[];
  ok: number;
  moves: number;
  missing: number;
  /** Boîtes nécessaires pour toute la Living Dex, et boîtes de la sauvegarde. */
  boxesNeeded: number;
  boxesAvailable: number;
  /** Cases de la Living Dex qui ne tiennent pas dans la sauvegarde. */
  overflow: number;
  /** Pokémon de la sauvegarde qui ne servent à aucune case (doublons…) : à ranger ailleurs. */
  spare: Specimen[];
}

const posKey = (p: Position) => `${p.box}:${p.index}`;
const positionOf = (s: Specimen): Position | null => (s.slot?.kind === "box" ? { box: s.slot.box, index: s.slot.index } : null);

/**
 * Pour chaque case de la Living Dex (dans l'ordre), choisit un Pokémon de la sauvegarde qui la
 * remplit : de préférence celui qui est déjà à sa place, sinon le premier rangé en boîte, puis
 * l'équipe. En mode chromatique, seuls les chromatiques comptent.
 */
export function arrangePlan(dex: Dex, rules: DexRules, slots: LivingSlot[], specimens: Specimen[], boxCount: number, shiny: boolean): ArrangePlan {
  const capacity = boxCount * BOX_SIZE;
  const bySlot = new Map<string, Specimen[]>();
  for (const s of specimens) {
    if (shiny && !s.shiny) continue;
    const key = slotKeyFor({ species: s.species, form: s.form, gender: s.gender === "female" ? "f" : s.gender === "male" ? "m" : "n" }, dex, rules);
    if (!key) continue;
    const list = bySlot.get(key);
    if (list) list.push(s);
    else bySlot.set(key, [s]);
  }
  const at = new Map<string, Specimen>();
  for (const s of specimens) {
    const p = positionOf(s);
    if (p) at.set(posKey(p), s);
  }

  const used = new Set<Specimen>();
  const steps: ArrangeStep[] = [];
  slots.forEach((slot, i) => {
    if (i >= capacity) return;
    const target = { box: Math.floor(i / BOX_SIZE), index: i % BOX_SIZE };
    const candidates = (bySlot.get(slot.key) ?? []).filter((s) => !used.has(s));
    const inPlace = candidates.find((s) => {
      const p = positionOf(s);
      return p !== null && posKey(p) === posKey(target);
    });
    const pick =
      inPlace ??
      [...candidates].sort((a, b) => {
        const pa = positionOf(a);
        const pb = positionOf(b);
        if (!pa || !pb) return pa ? -1 : pb ? 1 : 0;
        return pa.box - pb.box || pa.index - pb.index;
      })[0];
    if (!pick) {
      steps.push({ slot, target, status: "missing" });
      return;
    }
    used.add(pick);
    if (pick === inPlace) {
      steps.push({ slot, target, status: "ok", specimen: pick, from: target });
      return;
    }
    const occupant = at.get(posKey(target));
    steps.push({ slot, target, status: "move", specimen: pick, from: positionOf(pick), occupant: occupant && occupant !== pick ? occupant : undefined });
  });

  return {
    steps,
    ok: steps.filter((s) => s.status === "ok").length,
    moves: steps.filter((s) => s.status === "move").length,
    missing: steps.filter((s) => s.status === "missing").length,
    boxesNeeded: Math.ceil(slots.length / BOX_SIZE),
    boxesAvailable: boxCount,
    overflow: Math.max(0, slots.length - capacity),
    spare: specimens.filter((s) => !used.has(s)),
  };
}
