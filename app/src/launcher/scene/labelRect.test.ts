import { describe, expect, it } from "vitest";
import { LABEL_IN_CART, findLabel, opaqueBounds } from "./labelRect";

/** Image synthétique : fond transparent, cartouche grise, étiquette blanche avec un centre coloré. */
function photo(width: number, height: number, cart: [number, number, number, number], label: [number, number, number, number]) {
  const px = new Uint8ClampedArray(width * height * 4);
  const fill = (x0: number, y0: number, x1: number, y1: number, rgba: number[]) => {
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) px.set(rgba, (y * width + x) * 4);
  };
  fill(cart[0], cart[1], cart[2], cart[3], [70, 70, 72, 255]);
  fill(label[0], label[1], label[2], label[3], [250, 250, 250, 255]);
  // Illustration au milieu de l'étiquette, entre les bandeaux blancs.
  const band = Math.round((label[3] - label[1]) * 0.15);
  fill(label[0], label[1] + band, label[2], label[3] - band, [40, 30, 120, 255]);
  return px;
}

describe("étiquette dans la photo", () => {
  it("contour de la cartouche par la transparence", () => {
    const px = photo(100, 120, [10, 5, 90, 115], [20, 12, 80, 100]);
    expect(opaqueBounds(px, 100, 120)).toEqual({ x: 10, y: 5, w: 80, h: 110 });
  });

  it("carte DS : bords de l'étiquette d'après ses bandeaux blancs", () => {
    // Cartouche 300 × 330 ; étiquette décalée de la position attendue (cadrage différent).
    const px = photo(320, 360, [10, 15, 310, 345], [32, 33, 288, 287]);
    const r = findLabel(px, 320, 360, "ds");
    expect(r.x * 320).toBeCloseTo(32, 0);
    expect(r.y * 360).toBeCloseTo(33, 0);
    expect((r.x + r.w) * 320).toBeCloseTo(288, 0);
    expect((r.y + r.h) * 360).toBeCloseTo(287, 0);
  });

  it("sans bandeau blanc détectable : proportions mesurées", () => {
    const px = photo(200, 120, [0, 0, 200, 120], [0, 0, 0, 0]);
    const r = findLabel(px, 200, 120, "gba");
    for (const k of ["x", "y", "w", "h"] as const) expect(r[k]).toBeCloseTo(LABEL_IN_CART.gba[k], 6);
  });
});
