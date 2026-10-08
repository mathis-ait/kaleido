import { BufferAttribute, ExtrudeGeometry, Path, Shape, ShapeGeometry, Vector2, type BufferGeometry } from "three";
import { SVGLoader } from "three/addons/loaders/SVGLoader.js";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { SUPPORTS, type Support } from "./models";
import ds from "./outlines/ds.svg?raw";
import ctr from "./outlines/3ds.svg?raw";
import gba from "./outlines/gba.svg?raw";
import gb from "./outlines/gb.svg?raw";
import nx from "./outlines/switch.svg?raw";

/**
 * Géométries des supports, construites depuis les contours SVG tracés aux cotes
 * réelles (repli du PRD sans Blender : mêmes SVG que `tools/cartridges/build.py`).
 * Une géométrie par support, partagée par toutes les cartouches de ce support.
 *
 * Comme une vraie cartouche moulée :
 * - deux demi-coques arrondies, d'où la ligne de jointure creusée sur toute la tranche ;
 * - un rebord en relief autour de l'étiquette, posée au fond de son creux ;
 * - des stries de prise et, sur la cartouche GB, la flèche moulée sous l'étiquette.
 *
 * Unités : millimètres, origine au centre de la cartouche, y vers le haut, face
 * avant (étiquette) vers +z.
 */

const OUTLINES: Record<Support, string> = { ds, "3ds": ctr, gba, gb, switch: nx };

/** Arrondi des arêtes des demi-coques (mm) : il dessine aussi la jointure. */
const BEVEL = 0.4;
/** Profondeur du creux de l'étiquette (mm). */
const RECESS = 0.3;
/** Marge du creux autour de l'étiquette (mm). */
const RECESS_MARGIN = 0.45;

/** Détails moulés de la face avant, en mm, origine en haut à gauche du contour SVG. */
interface Features {
  /** Stries horizontales en relief : position du bloc, nombre, épaisseur d'une strie. */
  ribs?: { x: number; y: number; w: number; h: number; count: number; rib: number };
  /** Flèche en relief (pointe vers le bas) : centre et taille. */
  arrow?: { x: number; y: number; size: number };
}

const FEATURES: Record<Support, Features> = {
  ds: { ribs: { x: 6, y: 1.6, w: 21, h: 4.6, count: 5, rib: 0.42 } },
  "3ds": { ribs: { x: 6, y: 1.4, w: 21, h: 4.2, count: 5, rib: 0.42 } },
  gba: { ribs: { x: 14, y: 1.2, w: 30, h: 2.8, count: 3, rib: 0.45 } },
  gb: { ribs: { x: 8, y: 2.5, w: 38, h: 8, count: 6, rib: 0.6 }, arrow: { x: 28.5, y: 58.5, size: 4.2 } },
  switch: {},
};

export interface SupportGeometry {
  shell: BufferGeometry;
  label: BufferGeometry;
  /** Cotes en mm (largeur, hauteur, épaisseur). */
  size: [number, number, number];
}

const cache = new Map<Support, SupportGeometry>();

/** Coordonnées SVG (haut à gauche, y vers le bas) → centre de la cartouche, y vers le haut. */
function toLocal(support: Support) {
  const [w, h] = SUPPORTS[support].size;
  return (x: number, y: number) => new Vector2(x - w / 2, h / 2 - y);
}

function outlinePoints(support: Support): Vector2[] {
  const data = new SVGLoader().parse(OUTLINES[support]);
  const path = data.paths.find((p) => (p.userData?.node as Element | undefined)?.id === "outline") ?? data.paths[0];
  const shape = SVGLoader.createShapes(path)[0];
  const local = toLocal(support);
  return shape.getPoints(16).map((p) => local(p.x, p.y));
}

/** Rectangle à coins arrondis, en coordonnées SVG, converti en points locaux. */
function roundedPoints(support: Support, x: number, y: number, w: number, h: number, r: number): Vector2[] {
  const local = toLocal(support);
  const p = new Path();
  p.moveTo(x + r, y);
  p.lineTo(x + w - r, y);
  p.quadraticCurveTo(x + w, y, x + w, y + r);
  p.lineTo(x + w, y + h - r);
  p.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  p.lineTo(x + r, y + h);
  p.quadraticCurveTo(x, y + h, x, y + h - r);
  p.lineTo(x, y + r);
  p.quadraticCurveTo(x, y, x + r, y);
  return p.getPoints(6).map((q) => local(q.x, q.y));
}

function extrude(shape: Shape, depth: number, bevel: number, z: number, offset = -bevel): BufferGeometry {
  const geo = new ExtrudeGeometry(shape, {
    depth: Math.max(0.01, depth - bevel * 2),
    bevelEnabled: bevel > 0,
    bevelThickness: bevel,
    bevelSize: bevel,
    bevelOffset: offset,
    bevelSegments: bevel > 0 ? 4 : 0,
    curveSegments: 16,
  });
  // L'extrusion part de -bevel : la pièce va de z à z + depth.
  geo.translate(0, 0, z + bevel);
  return geo;
}

function buildShell(support: Support): BufferGeometry {
  const model = SUPPORTS[support];
  const [, , depth] = model.size;
  const outline = new Shape(outlinePoints(support));
  const half = depth / 2;
  const parts: BufferGeometry[] = [];

  // Deux demi-coques : leurs arrondis se rejoignent en V au milieu de la tranche (jointure).
  parts.push(extrude(outline, half, BEVEL, -half));
  parts.push(extrude(outline, half - RECESS, BEVEL, 0));

  // Rebord autour du creux de l'étiquette, posé sur la face avant, en retrait de l'arrondi.
  const z = model.labelZone;
  const rim = new Shape(outlinePoints(support));
  rim.holes.push(new Path(roundedPoints(support, z.x - RECESS_MARGIN, z.y - RECESS_MARGIN, z.w + RECESS_MARGIN * 2, z.h + RECESS_MARGIN * 2, 1).reverse()));
  parts.push(extrude(rim, RECESS + 0.02, 0.12, half - RECESS - 0.02, -(BEVEL + 0.12)));

  const f = FEATURES[support];
  if (f.ribs) {
    const { x, y, w, h, count, rib } = f.ribs;
    const gap = count > 1 ? (h - rib) / (count - 1) : 0;
    for (let i = 0; i < count; i++) {
      const s = new Shape(roundedPoints(support, x, y + i * gap, w, rib, rib / 2));
      parts.push(extrude(s, 0.22, 0.08, half - 0.02, 0));
    }
  }
  if (f.arrow) {
    const { x, y, size } = f.arrow;
    const local = toLocal(support);
    const s = new Shape([local(x - size / 2, y - size / 3), local(x + size / 2, y - size / 3), local(x, y + size / 2)]);
    parts.push(extrude(s, 0.2, 0.06, half - 0.02, 0));
  }

  const merged = mergeGeometries(parts.map((g) => (g.index ? g.toNonIndexed() : g)));
  for (const g of parts) g.dispose();
  return merged!;
}

/** Étiquette : rectangle à coins arrondis au fond du creux, UV de 0 à 1 sur la zone. */
function labelGeometry(support: Support): BufferGeometry {
  const [, , depth] = SUPPORTS[support].size;
  const z = SUPPORTS[support].labelZone;
  const pts = roundedPoints(support, z.x, z.y, z.w, z.h, Math.min(0.8, z.w / 20));
  const geo = new ShapeGeometry(new Shape(pts), 6);
  const local = toLocal(support);
  const origin = local(z.x, z.y + z.h);
  const pos = geo.getAttribute("position");
  const uv = new Float32Array(pos.count * 2);
  for (let i = 0; i < pos.count; i++) {
    uv[i * 2] = (pos.getX(i) - origin.x) / z.w;
    uv[i * 2 + 1] = (pos.getY(i) - origin.y) / z.h;
  }
  geo.setAttribute("uv", new BufferAttribute(uv, 2));
  // Au fond du creux, juste au-dessus pour éviter le scintillement.
  geo.translate(0, 0, depth / 2 - RECESS + 0.03);
  return geo;
}

export function supportGeometry(support: Support): SupportGeometry {
  let g = cache.get(support);
  if (!g) {
    g = { shell: buildShell(support), label: labelGeometry(support), size: SUPPORTS[support].size };
    cache.set(support, g);
  }
  return g;
}

export function disposeGeometries() {
  for (const g of cache.values()) {
    g.shell.dispose();
    g.label.dispose();
  }
  cache.clear();
}
