import { describe, expect, it } from "vitest";
import raw from "../../public/livedex/dex.json";
import type { Dex } from "./data";
import { buildSlots, computeCollection, RULE_PRESETS, slotKeyFor } from "./slots";
import type { CatchEntry, DexIndex } from "./types";

const index = raw as unknown as DexIndex;
const dex: Dex = {
  index,
  speciesList: index.species,
  games: index.games,
  species: (id) => index.species[id - 1],
  form: (s, f) => index.species[s - 1]?.forms.find((x) => x.f === f),
  game: (id) => index.games.find((g) => g.id === id),
  gameByVersion: (v) => index.games.find((g) => g.version === v),
};

const entry = (species: number, form = 0, extra: Partial<CatchEntry> = {}): CatchEntry => ({ id: `${species}-${form}-${Math.random()}`, auto: true, species, form, shiny: false, game: null, ...extra });

describe("cases de la Living Dex", () => {
  it("compte les cases de chaque préréglage comme Pelagix", () => {
    expect(buildSlots(dex, RULE_PRESETS.species.rules)).toHaveLength(1025);
    expect(buildSlots(dex, RULE_PRESETS.forms.rules)).toHaveLength(1365);
    expect(buildSlots(dex, RULE_PRESETS.completionist.rules)).toHaveLength(1627);
  });

  it("range une forme sans case dans la forme de base", () => {
    expect(slotKeyFor(entry(26, 1), dex, RULE_PRESETS.species.rules)).toBe("26");
    expect(slotKeyFor(entry(26, 1), dex, RULE_PRESETS.forms.rules)).toBe("26-1");
  });

  it("sépare mâles et femelles quand les sexes se distinguent", () => {
    expect(slotKeyFor(entry(25, 0, { gender: "f" }), dex, RULE_PRESETS.forms.rules)).toBe("25:f");
    expect(slotKeyFor(entry(25, 0, { gender: "n" }), dex, RULE_PRESETS.forms.rules)).toBe("25:m");
  });

  it("remplit une case une seule fois et compte les chromatiques", () => {
    const c = computeCollection(dex, [entry(1), entry(1, 0, { shiny: true }), entry(4)], RULE_PRESETS.species.rules);
    expect(c.totals.caught).toBe(2);
    expect(c.totals.shiny).toBe(1);
    expect(c.bySlot.get("1")).toHaveLength(2);
  });

  it("a des noms français", () => {
    expect(dex.species(25)?.name).toBe("Pikachu");
    expect(dex.species(6)?.name).toBe("Dracaufeu");
    expect(dex.form(26, 1)?.full).toBe("Raichu d'Alola");
  });
});
