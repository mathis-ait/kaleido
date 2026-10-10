import { describe, expect, it } from "vitest";
import raw from "../../public/livedex/dex.json";
import type { Dex } from "./data";
import { buildSlots, computeCollection, RULE_PRESETS, slotKeyFor } from "./slots";
import { arrangePlan } from "./arrange";
import type { CatchEntry, DexIndex, Specimen } from "./types";

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

describe("plan de rangement", () => {
  const spec = (species: number, slot: Specimen["slot"], extra: Partial<Specimen> = {}): Specimen => ({
    key: `${species}-${JSON.stringify(slot)}`,
    species,
    form: 0,
    gender: "male",
    shiny: false,
    nickname: null,
    level: 5,
    ball: 4,
    otName: "Moi",
    tid: 1,
    sid: 0,
    version: 30,
    metLocation: null,
    metLevel: 5,
    metDate: null,
    place: "",
    slot,
    legality: "legal",
    ...extra,
  });
  const rules = RULE_PRESETS.species.rules;
  const slots = buildSlots(dex, rules);

  it("garde ce qui est en place, déplace le reste et compte les manquants", () => {
    const list = [spec(1, { kind: "box", box: 0, index: 0 }), spec(3, { kind: "box", box: 0, index: 1 }), spec(2, { kind: "party", index: 0 }), spec(1, { kind: "box", box: 5, index: 3 })];
    const plan = arrangePlan(dex, rules, slots, list, 32, false);
    expect(plan.steps[0].status).toBe("ok");
    expect(plan.steps[1]).toMatchObject({ status: "move", from: null });
    expect(plan.steps[1].occupant?.species).toBe(3);
    expect(plan.steps[2]).toMatchObject({ status: "move", from: { box: 0, index: 1 } });
    expect(plan.missing).toBe(32 * 30 - 3);
    expect(plan.spare.map((s) => s.species)).toEqual([1]);
    expect(plan.overflow).toBe(1025 - 32 * 30);
  });

  it("ne garde que les chromatiques en mode chromatique", () => {
    const plan = arrangePlan(dex, rules, slots, [spec(1, { kind: "box", box: 0, index: 0 })], 40, true);
    expect(plan.steps[0].status).toBe("missing");
  });
});
