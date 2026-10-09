import { describe, expect, it } from "vitest";
import { RANDOMIZABLE } from "../../types";
import ds from "./outlines/ds.svg?raw";
import ctr from "./outlines/3ds.svg?raw";
import gba from "./outlines/gba.svg?raw";
import gb from "./outlines/gb.svg?raw";
import nx from "./outlines/switch.svg?raw";
import { GAME_LOOKS, SUPPORTS, cartPhotoKey, cartridgeCode, cartridgeKey, resolveLook, supportOf, type Support } from "./models";

const PLATFORM_OF: Record<string, "gb" | "gba" | "nds" | "3ds"> = {};
for (const id of ["red", "blue", "yellow", "gold", "silver", "crystal"]) PLATFORM_OF[id] = "gb";
for (const id of ["ruby", "sapphire", "emerald", "fire_red", "leaf_green"]) PLATFORM_OF[id] = "gba";
for (const id of ["diamond", "pearl", "platinum", "heart_gold", "soul_silver", "black", "white", "black2", "white2"]) PLATFORM_OF[id] = "nds";
for (const id of ["x", "y", "omega_ruby", "alpha_sapphire", "sun", "moon", "ultra_sun", "ultra_moon"]) PLATFORM_OF[id] = "3ds";

const fake = (id: string) => ({ platform: PLATFORM_OF[id], game: { id, name: id, generation: 1, platform: PLATFORM_OF[id] } });

describe("models", () => {
  it("chaque jeu a un support et une coque de ce support", () => {
    for (const id of RANDOMIZABLE) {
      expect(PLATFORM_OF[id], id).toBeDefined();
      const look = resolveLook(fake(id));
      expect(look.support.shells).toContain(look.shell);
    }
  });

  it("les couleurs déduites existent dans la palette du support", () => {
    for (const [id, natural] of Object.entries(GAME_LOOKS)) {
      const look = resolveLook(fake(id));
      expect(look.shell.id, id).toBe(natural.shell);
      expect(look.finish, id).toBe(natural.finish);
    }
    expect(resolveLook(fake("emerald")).finish).toBe("translucide");
    expect(resolveLook(fake("platinum")).shell.id).toBe("gris");
  });

  it("le choix de l'utilisateur prime, une coque inconnue retombe sur la valeur déduite", () => {
    expect(resolveLook(fake("ruby"), { shell: "bleu", finish: "mat" }).shell.id).toBe("bleu");
    expect(resolveLook(fake("ruby"), { shell: "inconnue" }).shell.id).toBe("rouge");
  });

  it("la zone d'étiquette correspond au contour SVG", () => {
    for (const support of Object.keys(SUPPORTS) as Support[]) {
      const svg = { ds, "3ds": ctr, gba, gb, switch: nx }[support];
      const rect = svg.match(/<rect id="label" x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"/);
      const view = svg.match(/viewBox="0 0 ([\d.]+) ([\d.]+)"/);
      expect(rect, support).not.toBeNull();
      const { x, y, w, h } = SUPPORTS[support].labelZone;
      expect(rect!.slice(1).map(Number)).toEqual([x, y, w, h]);
      expect(view!.slice(1).map(Number)).toEqual(SUPPORTS[support].size.slice(0, 2));
    }
  });

  it("supports par plateforme", () => {
    expect(supportOf({ platform: "switch" })).toBe("switch");
    expect(supportOf({ platform: null })).toBe("ds");
  });

  it("code imprimé d'après l'en-tête", () => {
    const d = (platform: "nds" | "gba" | "3ds" | "gb", details: [string, string][], id = "x") => ({
      platform,
      game: { id, name: id, generation: 4, platform },
      details: details.map(([label, value]) => ({ label, value })),
    });
    expect(cartridgeCode(d("nds", [["Code jeu", "CPUF"]]))).toBe("NTR-CPUF-FRA");
    expect(cartridgeCode(d("gba", [["Code jeu", "BPEE"]]))).toBe("AGB-BPEE-USA");
    expect(cartridgeCode(d("nds", [["Code jeu", "IRAF"]]))).toBe("TWL-IRAF-FRA");
    expect(cartridgeCode(d("3ds", [["Code produit", "CTR-P-EKJF"]]))).toBe("CTR-P-EKJF");
    expect(cartridgeCode(d("gb", [], "crystal"))).toBe("CGB");
  });

  it("clé de jeu sûre pour un nom de fichier", () => {
    expect(cartridgeKey({ fingerprint: "ab:cd/ef", details: [], path: "x" })).toBe("abcdef");
    expect(cartridgeKey({ fingerprint: null, details: [{ label: "Title ID", value: "0x01001F5010DFA000" }], path: "x" })).toBe("tid-01001F5010DFA000");
    expect(cartridgeKey({ fingerprint: null, details: [], path: "C:\a.nds" })).toMatch(/^path-[0-9a-f]{8}$/);
  });
});

describe("photo de la vraie carte", () => {
  const game = (id: string, platform: "nds" | "3ds" | "gba" | "gb" | "switch") => ({ id, name: id, generation: 4, platform: platform as "nds" });
  it("clé d'après le jeu et le code de l'en-tête", () => {
    expect(cartPhotoKey({ platform: "nds", game: game("platinum", "nds"), details: [{ label: "Code jeu", value: "CPUF" }] })).toBe("photo-ds-platinum-CPUF");
    expect(cartPhotoKey({ platform: "3ds", game: game("omega_ruby", "3ds"), details: [{ label: "Code produit", value: "CTR-P-ECRA" }] })).toBe("photo-3ds-omega_ruby-ECRA");
    expect(cartPhotoKey({ platform: "gb", game: game("red", "gb"), details: [] })).toBe("photo-gb-red");
    expect(cartPhotoKey({ platform: "switch", game: null, details: [] })).toBeNull();
    expect(cartPhotoKey({ platform: "gba", game: null, details: [{ label: "Code jeu", value: "B24E" }] })).toBe("photo-gba-jeu-B24E");
    expect(cartPhotoKey({ platform: "switch", game: null, details: [{ label: "Title ID", value: "0100ABF008968000" }] })).toBe("photo-switch-nx_0100abf008968000");
  });
});
