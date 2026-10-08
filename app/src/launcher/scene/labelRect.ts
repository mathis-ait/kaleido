import type { Support } from "./models";

/**
 * Position de l'étiquette dans la photo d'une cartouche détourée (LaunchBox, GameTDB).
 * Le cadrage change d'une photo à l'autre : on part du contour de la cartouche (pixels
 * opaques), puis, pour les cartes DS et 3DS, on cherche les bandeaux blancs du haut et du
 * bas de l'étiquette. Sans détection sûre, proportions mesurées sur de vraies photos.
 *
 * Fonction pure (pixels RGBA en entrée), sans canvas : testable.
 */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Étiquette dans le contour de la cartouche (fractions), mesurée sur des photos LaunchBox et GameTDB. */
export const LABEL_IN_CART: Record<Support, Rect> = {
  ds: { x: 0.087, y: 0.048, w: 0.828, h: 0.808 },
  "3ds": { x: 0.09, y: 0.084, w: 0.76, h: 0.834 },
  gba: { x: 0.133, y: 0.223, w: 0.732, h: 0.653 },
  gb: { x: 0.131, y: 0.29, w: 0.738, h: 0.575 },
  switch: { x: 0.073, y: 0.084, w: 0.854, h: 0.78 },
};

/**
 * Jeux dont la photo n'a pas les proportions du support (mesurées sur la photo retenue dans
 * `cart_photos.json`) : cartouche Game Boy Color de Cristal, étiquette plus large d'Argent.
 */
export const GAME_LABEL_IN_CART: Record<string, Rect> = {
  crystal: { x: 0.142, y: 0.347, w: 0.719, h: 0.569 },
  silver: { x: 0.126, y: 0.29, w: 0.763, h: 0.59 },
};

/** Écart toléré entre un bord détecté et le bord attendu (fraction de la cartouche). */
const TOLERANCE = 0.06;

/** Contour des pixels opaques (toute l'image si elle n'a pas de transparence). */
export function opaqueBounds(px: Uint8ClampedArray, width: number, height: number): Rect {
  let x0 = width;
  let y0 = height;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (px[(y * width + x) * 4 + 3] < 128) continue;
      if (x < x0) x0 = x;
      if (x > x1) x1 = x;
      if (y < y0) y0 = y;
      if (y > y1) y1 = y;
    }
  }
  if (x1 < 0) return { x: 0, y: 0, w: width, h: height };
  return { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
}

const bright = (px: Uint8ClampedArray, i: number) => px[i + 3] >= 128 && Math.min(px[i], px[i + 1], px[i + 2]) >= 225;

/** Part de pixels blancs sur un segment de ligne ou de colonne. */
function rowShare(px: Uint8ClampedArray, width: number, y: number, xa: number, xb: number) {
  let n = 0;
  for (let x = xa; x < xb; x++) if (bright(px, (y * width + x) * 4)) n++;
  return n / Math.max(1, xb - xa);
}

function colShare(px: Uint8ClampedArray, width: number, x: number, ya: number, yb: number) {
  let n = 0;
  for (let y = ya; y < yb; y++) if (bright(px, (y * width + x) * 4)) n++;
  return n / Math.max(1, yb - ya);
}

/**
 * Étiquette dans l'image (fractions de l'image entière). DS et 3DS : bandeau blanc du haut
 * (bord haut, puis bords gauche et droit le long de ce bandeau) et bandeau blanc du bas.
 */
export function findLabel(px: Uint8ClampedArray, width: number, height: number, support: Support, game?: string | null): Rect {
  const cart = opaqueBounds(px, width, height);
  const ref = (game && GAME_LABEL_IN_CART[game]) || LABEL_IN_CART[support];
  const expected = { x0: cart.x + ref.x * cart.w, y0: cart.y + ref.y * cart.h, x1: cart.x + (ref.x + ref.w) * cart.w, y1: cart.y + (ref.y + ref.h) * cart.h };
  const near = (value: number, target: number, span: number) => Math.abs(value - target) <= TOLERANCE * span;
  let { x0, y0, x1, y1 } = expected;

  if (support === "ds" || support === "3ds") {
    const midA = Math.round(cart.x + cart.w * 0.3);
    const midB = Math.round(cart.x + cart.w * 0.7);
    // Bord haut : première ligne presque entièrement blanche, en descendant.
    let top = -1;
    for (let y = cart.y; y < cart.y + cart.h * 0.3; y++) {
      if (rowShare(px, width, y, midA, midB) >= 0.7) {
        top = y;
        break;
      }
    }
    // Bord bas : première ligne blanche en remontant.
    let bottom = -1;
    for (let y = cart.y + cart.h - 1; y > cart.y + cart.h * 0.6; y--) {
      if (rowShare(px, width, y, midA, midB) >= 0.7) {
        bottom = y;
        break;
      }
    }
    if (top >= 0 && near(top, expected.y0, cart.h)) y0 = top;
    if (bottom >= 0 && near(bottom, expected.y1, cart.h)) y1 = bottom + 1;
    if (top >= 0) {
      // Bords gauche et droit le long du bandeau du haut.
      const bandA = top + 1;
      const bandB = Math.min(height, Math.round(top + cart.h * 0.035));
      let left = -1;
      for (let x = cart.x; x < cart.x + cart.w * 0.3; x++) {
        if (colShare(px, width, x, bandA, bandB) >= 0.7) {
          left = x;
          break;
        }
      }
      let right = -1;
      for (let x = cart.x + cart.w - 1; x > cart.x + cart.w * 0.6; x--) {
        if (colShare(px, width, x, bandA, bandB) >= 0.7) {
          right = x;
          break;
        }
      }
      if (left >= 0 && near(left, expected.x0, cart.w)) x0 = left;
      if (right >= 0 && near(right, expected.x1, cart.w)) x1 = right + 1;
    }
  }
  return { x: x0 / width, y: y0 / height, w: (x1 - x0) / width, h: (y1 - y0) / height };
}
