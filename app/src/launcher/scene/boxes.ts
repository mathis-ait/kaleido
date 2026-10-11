import {
  BoxGeometry,
  BufferGeometry,
  CanvasTexture,
  Color,
  Group,
  Mesh,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  PlaneGeometry,
  SRGBColorSpace,
  Vector3,
  type Material,
  type Texture,
} from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { SUPPORTS, type Support } from "./models";

/**
 * Boîtes des jeux en mode Jaquettes : boîtier plastique (DS transparent, 3DS blanc, Switch
 * rouge) avec la jaquette sous la pochette, le livret et la carte dans son logement ; boîte
 * en carton à rabat pour la GB et la GBA, la cartouche debout à l'intérieur. Au lancement,
 * la boîte s'ouvre et la carte en sort (voir `openPlan` dans insert.ts).
 *
 * Tout est en millimètres, centré sur la boîte ; la scène met le groupe à l'échelle.
 * Géométries partagées entre boîtes du même support, matériaux propres à chacune (assombri
 * et fondu des voisines).
 */

interface CaseSpec {
  kind: "case";
  /** Largeur, hauteur, épaisseur (mm). */
  size: [number, number, number];
  /** Centre de la carte dans son logement (x, y), sur le fond (disposition des vrais boîtiers). */
  seat: [number, number];
  /** Support de cartouche GBA au-dessus du logement (boîtiers DS). */
  bracket?: boolean;
  /** Pattes du fond près de la charnière (x, y) : boîtiers Switch. */
  tabs?: [number, number][];
  plastic: { color: number; alpha: number; roughness: number };
  booklet: boolean;
  /** Tranche : bande grise « DS » / « 3DS », ou bande rouge Switch. */
  spine: "ds" | "3ds" | "switch";
}

interface CartonSpec {
  kind: "carton";
  size: [number, number, number];
}

type BoxSpec = CaseSpec | CartonSpec;

const CLEAR = { color: 0xf2f6ff, alpha: 0.2, roughness: 0.06 };

export const BOX_SPECS: Record<Support, BoxSpec> = {
  // Carte au centre du volet droit, un peu sous le milieu ; support GBA au-dessus (DS).
  ds: { kind: "case", size: [136, 124, 14], seat: [6, -14], bracket: true, plastic: CLEAR, booklet: true, spine: "ds" },
  "3ds": { kind: "case", size: [136, 124, 14], seat: [6, -10], plastic: { color: 0xf3f3f1, alpha: 1, roughness: 0.38 }, booklet: true, spine: "3ds" },
  // Carte en bas à droite, deux pattes près de la charnière.
  switch: {
    kind: "case",
    size: [105, 170, 11],
    seat: [16, -46],
    tabs: [
      [-30, 36],
      [-30, -4],
    ],
    plastic: { color: 0xb3101a, alpha: 1, roughness: 0.45 },
    booklet: false,
    spine: "switch",
  },
  gba: { kind: "carton", size: [127, 127, 30] },
  gb: { kind: "carton", size: [127, 127, 30] },
};

/** Épaisseur du plastique. */
const T = 1.3;

// --- Géométries partagées

interface CaseGeo {
  back: BufferGeometry;
  lid: BufferGeometry;
  sheet: PlaneGeometry;
  spine: PlaneGeometry;
  booklet: BoxGeometry;
}
interface CartonGeo {
  body: BoxGeometry;
  flap: PlaneGeometry;
  tab: PlaneGeometry;
  inside: PlaneGeometry;
}
const caseGeos = new Map<Support, CaseGeo>();
const cartonGeos = new Map<Support, CartonGeo>();

function rounded(w: number, h: number, d: number, r: number, x: number, y: number, z: number) {
  const g = new RoundedBoxGeometry(w, h, d, 2, Math.min(r, Math.min(w, h, d) / 2 - 0.01));
  g.translate(x, y, z);
  return g;
}

function merge(parts: BufferGeometry[]) {
  const g = mergeGeometries(parts.map((p) => p.toNonIndexed()));
  for (const p of parts) p.dispose();
  return g;
}

function caseGeometry(support: Support, spec: CaseSpec): CaseGeo {
  const cached = caseGeos.get(support);
  if (cached) return cached;
  const [W, H, D] = spec.size;
  const [cw, ch] = SUPPORTS[support].size;
  // Plaque et rebords d'une moitié (pas de rebord côté charnière). `dir` : +1 couvercle, -1 fond.
  const half = (dir: 1 | -1) => {
    const wallH = D / 2 - 0.25;
    const plateZ = dir * (D / 2 - T / 2);
    const wallZ = dir * (D / 2 - wallH / 2);
    return [
      rounded(W, H, T, 2.2, 0, 0, plateZ),
      rounded(W - 2, T, wallH, 0.5, 1, H / 2 - T / 2, wallZ),
      rounded(W - 2, T, wallH, 0.5, 1, -H / 2 + T / 2, wallZ),
      rounded(T, H - 2, wallH, 0.5, W / 2 - T / 2, 0, wallZ),
    ];
  };
  // Fond : charnière, cadre du logement de la carte (ouvert en bas pour le pouce), ergots.
  const back = half(-1);
  back.push(rounded(3.2, H, D, 1.4, -W / 2 + 1.6, 0, 0));
  const [sx, sy] = spec.seat;
  const gw = cw + 1.2;
  const gh = ch + 1.2;
  const bar = 1.6;
  const fz = -D / 2 + T + 1.7;
  const frame = (w: number, h: number, x: number, y: number) => back.push(rounded(w, h, 3.4, 0.5, sx + x, sy + y, fz));
  frame(gw + bar * 2, bar, 0, gh / 2 + bar / 2);
  frame(bar, gh, -gw / 2 - bar / 2, 0);
  frame(bar, gh, gw / 2 + bar / 2, 0);
  const notch = Math.min(9, gw / 3);
  frame(notch, bar, -gw / 2 + notch / 2, -gh / 2 - bar / 2);
  frame(notch, bar, gw / 2 - notch / 2, -gh / 2 - bar / 2);
  if (spec.bracket) {
    // Support GBA : barre en haut et deux bras qui descendent.
    const by = sy + gh / 2 + 30;
    back.push(rounded(62, bar, 3.4, 0.5, sx, by, fz));
    for (const x of [-30, 30]) back.push(rounded(bar, 14, 3.4, 0.5, sx + x, by - 6, fz));
  }
  for (const [x, y] of spec.tabs ?? []) back.push(rounded(8, 9, 3, 1, x, y, fz));
  // Couvercle (repère : charnière à gauche, en x = 0) et pattes du livret.
  const lid = half(1).map((g) => g.translate(W / 2, 0, 0));
  if (spec.booklet) for (const y of [-H / 3.2, H / 3.2]) lid.push(rounded(5, 8, 3.2, 0.8, W - 6, y, D / 2 - T - 1.6));
  const geo = {
    back: merge(back),
    lid: merge(lid),
    sheet: new PlaneGeometry(W - 3, H - 3),
    spine: new PlaneGeometry(H - 4, D - 1.2),
    booklet: new BoxGeometry(W - 18, H - 18, 1.6),
  };
  caseGeos.set(support, geo);
  return geo;
}

function cartonGeometry(support: Support, size: [number, number, number]): CartonGeo {
  const cached = cartonGeos.get(support);
  if (cached) return cached;
  const [W, H, D] = size;
  const geo = {
    body: new BoxGeometry(W, H, D),
    flap: new PlaneGeometry(W - 0.4, D - 0.4),
    tab: new PlaneGeometry(W - 3, 12),
    inside: new PlaneGeometry(W - 1.5, D - 1.5),
  };
  cartonGeos.set(support, geo);
  return geo;
}

export function disposeBoxGeometries() {
  for (const g of [...caseGeos.values(), ...cartonGeos.values()]) for (const x of Object.values(g)) (x as BufferGeometry).dispose();
  caseGeos.clear();
  cartonGeos.clear();
}

// --- Textures

function canvas(w: number, h: number) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return [c, c.getContext("2d")!] as const;
}

function canvasTexture(c: HTMLCanvasElement) {
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.anisotropy = 4;
  return t;
}

/** Couleur moyenne d'une bande de l'image (bord gauche) : flancs des boîtes en carton. */
function edgeColor(img: HTMLImageElement | null): string {
  if (!img) return "#d9d6cf";
  const [, g] = canvas(8, 32);
  g.drawImage(img, 0, 0, img.naturalWidth * 0.08, img.naturalHeight, 0, 0, 8, 32);
  const px = g.getImageData(0, 0, 8, 32).data;
  let r = 0;
  let gg = 0;
  let b = 0;
  for (let i = 0; i < px.length; i += 4) {
    r += px[i];
    gg += px[i + 1];
    b += px[i + 2];
  }
  const n = px.length / 4;
  return `rgb(${Math.round(r / n)}, ${Math.round(gg / n)}, ${Math.round(b / n)})`;
}

const textOn = (color: string) => {
  const m = color.match(/\d+/g)?.map(Number) ?? [255, 255, 255];
  return 0.299 * m[0] + 0.587 * m[1] + 0.114 * m[2] > 150 ? "#1d1d22" : "#f4f4f2";
};

/** Tranche d'un boîtier : bande de la console puis le titre. */
function spineTexture(style: CaseSpec["spine"], title: string) {
  const [c, g] = canvas(1024, 112);
  if (style === "switch") {
    g.fillStyle = "#f4f4f2";
    g.fillRect(0, 0, c.width, c.height);
    g.fillStyle = "#e60012";
    g.fillRect(0, 0, 190, c.height);
    g.fillStyle = "#fff";
    g.font = "700 34px system-ui, sans-serif";
    g.fillText("SWITCH", 30, 70);
  } else {
    g.fillStyle = style === "3ds" ? "#ffffff" : "#f4f4f2";
    g.fillRect(0, 0, c.width, c.height);
    g.fillStyle = style === "3ds" ? "#d8131b" : "#c9c9cf";
    g.fillRect(0, 0, 150, c.height);
    g.fillStyle = style === "3ds" ? "#fff" : "#222";
    g.font = "700 46px system-ui, sans-serif";
    g.fillText(style === "3ds" ? "3DS" : "DS", 30, 72);
  }
  g.fillStyle = "#222";
  g.font = "600 40px system-ui, sans-serif";
  g.fillText(title.toUpperCase(), 220, 72, c.width - 250);
  return canvasTexture(c);
}

/**
 * Tranche scannée (portrait, haut de la boîte en haut) tournée dans le sens de la tranche
 * générée : le bas de la boîte à gauche.
 */
function scannedSpine(img: HTMLImageElement) {
  const [c, g] = canvas(1024, 112);
  g.translate(c.width, 0);
  g.rotate(Math.PI / 2);
  g.drawImage(img, 0, 0, c.height, c.width);
  return canvasTexture(c);
}

/** Dos sans scan : l'illustration floutée et assombrie. */
function backTexture(img: HTMLImageElement | null, w = 512, h = 468) {
  const [c, g] = canvas(w, h);
  g.fillStyle = "#2a2d38";
  g.fillRect(0, 0, w, h);
  if (img) {
    g.filter = "blur(18px) saturate(1.15)";
    g.drawImage(img, -40, -40, w + 80, h + 80);
    g.filter = "none";
    g.fillStyle = "rgba(18, 20, 30, 0.5)";
    g.fillRect(0, 0, w, h);
  }
  return canvasTexture(c);
}

/** Jaquette absente : papier neutre avec le titre. */
function plainCover(title: string, w = 512, h = 468) {
  const [c, g] = canvas(w, h);
  g.fillStyle = "#e9e8e4";
  g.fillRect(0, 0, w, h);
  g.fillStyle = "#222";
  g.font = "600 34px system-ui, sans-serif";
  g.fillText(title, 40, 80, w - 80);
  return canvasTexture(c);
}

/**
 * Jaquette Switch : la source est une icône carrée. Bandeau rouge en haut (comme les vraies
 * jaquettes), l'icône en pleine largeur sur un fond fait d'elle-même, floutée.
 */
function switchCover(img: HTMLImageElement | null, title: string) {
  const [c, g] = canvas(600, 972);
  const band = 92;
  if (img) {
    g.filter = "blur(30px) saturate(1.2)";
    g.drawImage(img, -300, band - 100, 1200, 1200);
    g.filter = "none";
    g.fillStyle = "rgba(0, 0, 0, 0.18)";
    g.fillRect(0, 0, c.width, c.height);
    g.drawImage(img, 0, band + (c.height - band - 600) / 2, 600, 600);
  } else {
    g.fillStyle = "#e9e8e4";
    g.fillRect(0, 0, c.width, c.height);
    g.fillStyle = "#222";
    g.font = "600 40px system-ui, sans-serif";
    g.fillText(title, 40, 200, 520);
  }
  g.fillStyle = "#e60012";
  g.fillRect(0, 0, c.width, band);
  g.fillStyle = "#fff";
  g.font = "700 40px system-ui, sans-serif";
  g.fillText("NINTENDO SWITCH", 40, 60);
  return canvasTexture(c);
}

/** Flanc d'une boîte en carton : couleur prise sur la jaquette, titre en travers. */
function sideTexture(color: string, title: string) {
  const [c, g] = canvas(1024, 240);
  g.fillStyle = color;
  g.fillRect(0, 0, c.width, c.height);
  g.fillStyle = textOn(color);
  g.font = "700 72px system-ui, sans-serif";
  g.fillText(title.toUpperCase(), 60, 145, c.width - 120);
  return canvasTexture(c);
}

function loadImage(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

async function firstImage(urls: (string | null)[]) {
  for (const url of urls) {
    const img = url ? await loadImage(url) : null;
    if (img) return img;
  }
  return null;
}

function imageTexture(img: HTMLImageElement) {
  const [c, g] = canvas(img.naturalWidth, img.naturalHeight);
  g.drawImage(img, 0, 0);
  return canvasTexture(c);
}

// --- Boîtes

export interface GameBox {
  group: Group;
  size: [number, number, number];
  /** Carte rangée, puis sortie de la boîte (mm, repère de la boîte). */
  seat: Vector3;
  exit: Vector3;
  /** Ouverte : inclinaison (rad, autour de x) et décalage pour rester centrée (mm). */
  openTilt: number;
  openShift: number;
  setOpen(o: number): void;
  /** Tous les matériaux : `userData.base` (couleur) et `userData.alpha` (opacité propre). */
  mats: MeshStandardMaterial[];
  dispose(): void;
}

interface Options {
  support: Support;
  /** Jaquettes à essayer dans l'ordre (romhack sans jaquette → celle du jeu de base). */
  covers: (string | null)[];
  /** Scans LaunchBox : dos, tranche, face avant (Switch), s'ils existent. */
  scans: { back: string | null; spine: string | null; front: string | null };
  title: string;
  /** ROM randomisée par Kaleido : pochette ou carton irisé. */
  randomized: boolean;
  onChange: () => void;
}

class Materials {
  list: MeshStandardMaterial[] = [];
  textures: Texture[] = [];
  add<M extends MeshStandardMaterial>(m: M, alpha = 1): M {
    m.transparent = true;
    m.userData.base = m.color.clone();
    m.userData.alpha = alpha;
    this.list.push(m);
    return m;
  }
  texture(t: Texture) {
    this.textures.push(t);
    return t;
  }
  setMap(m: MeshStandardMaterial, map: Texture) {
    m.map = this.texture(map);
    m.color.set(0xffffff);
    m.userData.base = m.color.clone();
    m.needsUpdate = true;
  }
  dispose() {
    for (const m of this.list) m.dispose();
    for (const t of this.textures) t.dispose();
  }
}

/** Impression sous pochette (boîtier) ou sur carton vernis ; irisée pour une ROM randomisée. */
const printed = (randomized: boolean, sleeve: boolean) =>
  new MeshPhysicalMaterial({
    roughness: sleeve ? 0.6 : 0.5,
    clearcoat: sleeve ? 1 : 0.35,
    clearcoatRoughness: sleeve ? 0.04 : 0.3,
    iridescence: randomized ? 0.9 : sleeve ? 0.2 : 0,
    iridescenceIOR: 1.35,
    iridescenceThicknessRange: [250, 600],
  });

function add(parent: Group, g: BufferGeometry, m: Material | Material[], order: number, x = 0, y = 0, z = 0) {
  const mesh = new Mesh(g, m);
  mesh.position.set(x, y, z);
  mesh.renderOrder = order;
  parent.add(mesh);
  return mesh;
}

function buildCase(o: Options, spec: CaseSpec): GameBox {
  const geo = caseGeometry(o.support, spec);
  const [W, , D] = spec.size;
  const mats = new Materials();
  const p = spec.plastic;
  // Plastique transparent : vraie transparence (le fond du lanceur est dans le DOM, sous le
  // canvas : `transmission` ne le verrait pas). Les reflets du studio dessinent les arêtes.
  const plastic = () =>
    mats.add(
      new MeshPhysicalMaterial({
        color: p.color,
        roughness: p.roughness,
        clearcoat: p.alpha < 1 ? 1 : 0,
        clearcoatRoughness: 0.05,
        iridescence: p.alpha < 1 ? 0.15 : 0,
        iridescenceIOR: 1.3,
        iridescenceThicknessRange: [200, 500],
        // Plastique opaque coloré : peu de reflet blanc, sinon les grandes faces virent au rose.
        specularIntensity: p.alpha < 1 ? 1 : 0.25,
        depthWrite: p.alpha >= 1,
      }),
      p.alpha,
    );
  const plasticOrder = p.alpha < 1 ? 2 : -2;
  const paper = mats.add(new MeshStandardMaterial({ color: 0xdedfe2, roughness: 0.9 }));
  const front = mats.add(printed(o.randomized, true));
  const backArt = mats.add(printed(false, true));
  const bookletArt = mats.add(new MeshStandardMaterial({ roughness: 0.75 }));
  const spine = mats.add(new MeshStandardMaterial({ map: mats.texture(spineTexture(spec.spine, o.title)), roughness: 0.55 }));

  const group = new Group();
  add(group, geo.back, plastic(), plasticOrder);
  add(group, geo.sheet, backArt, -2, 0.5, 0, -D / 2 - 0.06).rotation.y = Math.PI;
  add(group, geo.sheet, paper, -2, 0.5, 0, -D / 2 - 0.04);
  add(group, geo.spine, spine, -2, -W / 2 - 0.08, 0, 0).rotation.set(0, -Math.PI / 2, Math.PI / 2);

  const lid = new Group();
  lid.position.set(-W / 2, 0, 0);
  group.add(lid);
  add(lid, geo.lid, plastic(), plasticOrder);
  add(lid, geo.sheet, front, -2, W / 2 + 0.5, 0, D / 2 + 0.06);
  add(lid, geo.sheet, paper, -2, W / 2 + 0.5, 0, D / 2 + 0.04).rotation.y = Math.PI;
  // Livret sous ses pattes ; sa couverture (face -z) se retrouve vers le haut, couvercle ouvert.
  if (spec.booklet) add(lid, geo.booklet, [paper, paper, paper, paper, paper, bookletArt], -2, W / 2 + 4, 0, D / 2 - T - 0.8);

  let disposed = false;
  void Promise.all([firstImage(o.covers), firstImage([o.scans.front]), firstImage([o.scans.back]), firstImage([o.scans.spine])]).then(([img, frontScan, backScan, spineScan]) => {
    if (disposed) return;
    // Switch : la jaquette du lanceur est une icône carrée ; le scan de la vraie boîte passe avant.
    const art = frontScan ? imageTexture(frontScan) : o.support === "switch" ? switchCover(img, o.title) : img ? imageTexture(img) : plainCover(o.title);
    mats.setMap(front, art);
    mats.setMap(bookletArt, art.clone());
    mats.setMap(backArt, backScan ? imageTexture(backScan) : backTexture(img));
    if (spineScan) mats.setMap(spine, scannedSpine(spineScan));
    o.onChange();
  });

  const [, , cd] = SUPPORTS[o.support].size;
  const seat = new Vector3(spec.seat[0], spec.seat[1], -D / 2 + T + cd / 2 + 0.2);
  return {
    group,
    size: spec.size,
    seat,
    exit: seat.clone().add(new Vector3(0, 0, 14)),
    openTilt: -0.38,
    openShift: W / 2,
    setOpen: (v) => (lid.rotation.y = -Math.PI * v),
    mats: mats.list,
    dispose() {
      disposed = true;
      mats.dispose();
    },
  };
}

function buildCarton(o: Options, spec: CartonSpec): GameBox {
  const geo = cartonGeometry(o.support, spec.size);
  const [, H, D] = spec.size;
  const mats = new Materials();
  const card = () => mats.add(new MeshStandardMaterial({ color: 0xb59d78, roughness: 0.95 }));
  const front = mats.add(printed(o.randomized, false));
  const back = mats.add(printed(false, false));
  const side = mats.add(new MeshPhysicalMaterial({ color: 0xd9d6cf, roughness: 0.5, clearcoat: 0.35, clearcoatRoughness: 0.3 }));
  const flapOut = mats.add(new MeshPhysicalMaterial({ color: 0xd9d6cf, roughness: 0.5, clearcoat: 0.35, clearcoatRoughness: 0.3 }));
  const inner = card();
  // Le dessus du corps n'est pas dessiné : c'est le rabat qui le ferme.
  const none = new MeshStandardMaterial({ visible: false });

  const group = new Group();
  // Faces de BoxGeometry : +x, -x, +y, -y, +z, -z.
  const left = mats.add(new MeshPhysicalMaterial({ color: 0xd9d6cf, roughness: 0.5, clearcoat: 0.35, clearcoatRoughness: 0.3 }));
  add(group, geo.body, [side, left, none, side, front, back], 0);
  // Fond intérieur visible par l'ouverture, un peu sous le bord.
  const inside = add(group, geo.inside, inner, -1, 0, H / 2 - 4, 0);
  inside.rotation.x = -Math.PI / 2;

  // Rabat : charnière sur l'arête arrière du dessus, languette qui rentre devant.
  const hinge = new Group();
  hinge.position.set(0, H / 2, -D / 2);
  group.add(hinge);
  const flap = new Group();
  hinge.add(flap);
  add(flap, geo.flap, flapOut, 0, 0, 0.05, D / 2).rotation.x = -Math.PI / 2;
  add(flap, geo.flap, card(), 0, 0, -0.05, D / 2).rotation.x = Math.PI / 2;
  const tab = add(flap, geo.tab, card(), 0, 0, -6, D - 0.6);
  tab.rotation.y = Math.PI;

  let disposed = false;
  void Promise.all([firstImage(o.covers), firstImage([o.scans.back]), firstImage([o.scans.spine])]).then(([img, backScan, spineScan]) => {
    if (disposed) return;
    mats.setMap(front, img ? imageTexture(img) : plainCover(o.title, 512, 512));
    mats.setMap(back, backScan ? imageTexture(backScan) : backTexture(img, 512, 512));
    const sideTex = sideTexture(edgeColor(img), o.title);
    mats.setMap(side, sideTex);
    mats.setMap(flapOut, sideTex.clone());
    // Flanc gauche : la tranche scannée, debout (face -x : avant à droite, haut en haut).
    mats.setMap(left, spineScan ? imageTexture(spineScan) : sideTex.clone());
    o.onChange();
  });

  const [, ch] = SUPPORTS[o.support].size;
  return {
    group,
    size: spec.size,
    seat: new Vector3(0, -H / 2 + ch / 2 + 3, 0),
    exit: new Vector3(0, H / 2 + ch / 2 + 8, 0),
    openTilt: 0.32,
    openShift: 0,
    setOpen: (v) => (hinge.rotation.x = -1.95 * v),
    mats: mats.list,
    dispose() {
      disposed = true;
      mats.dispose();
      none.dispose();
    },
  };
}

export function buildBox(o: Options): GameBox {
  const spec = BOX_SPECS[o.support];
  return spec.kind === "case" ? buildCase(o, spec) : buildCarton(o, spec);
}

/** Couleur de base assombrie (voisines du carrousel) et opacité. */
export function shadeBox(b: GameBox, dim: number, opacity: number) {
  for (const m of b.mats) {
    m.color.copy(m.userData.base as Color).multiplyScalar(dim);
    m.opacity = opacity * ((m.userData.alpha as number | undefined) ?? 1);
  }
}
