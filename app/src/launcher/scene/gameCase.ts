import { BoxGeometry, BufferGeometry, CanvasTexture, Color, Group, Mesh, MeshPhysicalMaterial, MeshStandardMaterial, PlaneGeometry, SRGBColorSpace, Vector3, type Material, type Texture } from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

/**
 * Boîtier transparent d'un jeu DS (mode Cartouche) : la jaquette glissée sous la pochette,
 * le livret dans le couvercle et la carte dans son logement. Il s'ouvre au lancement du
 * jeu, la carte en sort puis suit la séquence d'insertion habituelle (`insert.ts`).
 *
 * Tout est en millimètres, centré sur le boîtier ; la scène met le groupe à l'échelle.
 * Les géométries sont partagées entre boîtiers, les matériaux propres à chacun (assombri
 * et fondu des voisines).
 */

/** Cotes d'un boîtier DS (mm) : largeur, hauteur, épaisseur. */
export const CASE_SIZE: [number, number, number] = [136, 124, 14];
const [W, H, D] = CASE_SIZE;
/** Épaisseur du plastique. */
const T = 1.3;
/** Carte DS (mm), pour son logement. */
const CARD = { w: 33, h: 35, d: 3.8 };
/** Centre de la carte posée dans son logement (repère du boîtier). */
export const SEAT = new Vector3(W / 4 + 4, 10, -D / 2 + T + CARD.d / 2 + 0.2);

interface Shared {
  back: BufferGeometry;
  lid: BufferGeometry;
  sheet: PlaneGeometry;
  spine: PlaneGeometry;
  booklet: BoxGeometry;
}
let shared: Shared | null = null;

function rounded(w: number, h: number, d: number, r: number, x: number, y: number, z: number) {
  const g = new RoundedBoxGeometry(w, h, d, 2, Math.min(r, Math.min(w, h, d) / 2 - 0.01));
  g.translate(x, y, z);
  return g;
}

/** Plaque et rebords d'une moitié (pas de rebord côté charnière). `dir` : +1 couvercle, -1 fond. */
function half(dir: 1 | -1): BufferGeometry[] {
  const wallH = D / 2 - 0.25;
  const plateZ = dir * (D / 2 - T / 2);
  const wallZ = dir * (D / 2 - wallH / 2);
  return [
    rounded(W, H, T, 2.2, 0, 0, plateZ),
    rounded(W - 2, T, wallH, 0.5, 1, H / 2 - T / 2, wallZ),
    rounded(W - 2, T, wallH, 0.5, 1, -H / 2 + T / 2, wallZ),
    rounded(T, H - 2, wallH, 0.5, W / 2 - T / 2, 0, wallZ),
  ];
}

function geometries(): Shared {
  if (shared) return shared;
  // Fond : moitié arrière, charnière, cadre du logement de la carte, ergots.
  const back = half(-1);
  back.push(rounded(3.2, H, D, 1.4, -W / 2 + 1.6, 0, 0));
  const gw = CARD.w + 1.2;
  const gh = CARD.h + 1.2;
  const bar = 1.6;
  const fz = -D / 2 + T + 1.7;
  const frame = (w: number, h: number, x: number, y: number) => back.push(rounded(w, h, 3.4, 0.5, SEAT.x + x, SEAT.y + y, fz));
  frame(gw + bar * 2, bar, 0, gh / 2 + bar / 2);
  frame(bar, gh, -gw / 2 - bar / 2, 0);
  frame(bar, gh, gw / 2 + bar / 2, 0);
  // Bas du cadre ouvert au milieu : encoche pour le pouce.
  frame(9, bar, -gw / 2 + 4.5, -gh / 2 - bar / 2);
  frame(9, bar, gw / 2 - 4.5, -gh / 2 - bar / 2);
  for (const y of [-H / 4, H / 4]) back.push(rounded(4, 9, 3, 0.8, -W / 2 + 12, y, -D / 2 + T + 1.5));
  // Couvercle (repère : charnière à gauche, en x = 0) : moitié avant et pattes du livret.
  const lid = half(1).map((g) => g.translate(W / 2, 0, 0));
  for (const y of [-H / 3.2, H / 3.2]) lid.push(rounded(5, 8, 3.2, 0.8, W - 6, y, D / 2 - T - 1.6));
  const merge = (parts: BufferGeometry[]) => {
    const g = mergeGeometries(parts.map((p) => p.toNonIndexed()));
    for (const p of parts) p.dispose();
    return g;
  };
  shared = {
    back: merge(back),
    lid: merge(lid),
    sheet: new PlaneGeometry(W - 3, H - 3),
    spine: new PlaneGeometry(H - 4, D - 1.2),
    booklet: new BoxGeometry(118, 106, 1.6),
  };
  return shared;
}

export function disposeCaseGeometries() {
  if (!shared) return;
  for (const g of Object.values(shared)) g.dispose();
  shared = null;
}

export interface GameCase {
  group: Group;
  /** Pivot du couvercle (rotation y : 0 fermé, -π ouvert). */
  lid: Group;
  /** Tous les matériaux : `userData.base` (couleur) et `userData.alpha` (opacité propre). */
  mats: MeshStandardMaterial[];
  dispose(): void;
}

function canvasTexture(c: HTMLCanvasElement) {
  const t = new CanvasTexture(c);
  t.colorSpace = SRGBColorSpace;
  t.anisotropy = 4;
  return t;
}

/** Tranche : bande grise « DS » puis le titre, comme sur les boîtiers européens. */
function spineTexture(title: string) {
  const c = document.createElement("canvas");
  c.width = 1024;
  c.height = 112;
  const g = c.getContext("2d")!;
  g.fillStyle = "#f4f4f2";
  g.fillRect(0, 0, c.width, c.height);
  g.fillStyle = "#c9c9cf";
  g.fillRect(0, 0, 150, c.height);
  g.fillStyle = "#222";
  g.font = "700 46px system-ui, sans-serif";
  g.fillText("DS", 40, 72);
  g.font = "600 40px system-ui, sans-serif";
  g.fillText(title.toUpperCase(), 190, 72, c.width - 220);
  return canvasTexture(c);
}

/** Dos de jaquette (pas de scan disponible) : l'illustration floutée et assombrie. */
function backTexture(img: HTMLImageElement | null) {
  const c = document.createElement("canvas");
  c.width = 512;
  c.height = 468;
  const g = c.getContext("2d")!;
  g.fillStyle = "#2a2d38";
  g.fillRect(0, 0, c.width, c.height);
  if (img) {
    g.filter = "blur(18px) saturate(1.15)";
    g.drawImage(img, -40, -40, c.width + 80, c.height + 80);
    g.filter = "none";
    g.fillStyle = "rgba(18, 20, 30, 0.5)";
    g.fillRect(0, 0, c.width, c.height);
  }
  return canvasTexture(c);
}

/** Jaquette absente : papier neutre avec le titre. */
function plainCover(title: string) {
  const c = document.createElement("canvas");
  c.width = 512;
  c.height = 468;
  const g = c.getContext("2d")!;
  g.fillStyle = "#e9e8e4";
  g.fillRect(0, 0, c.width, c.height);
  g.fillStyle = "#fff";
  g.fillRect(0, 0, 70, c.height);
  g.fillStyle = "#222";
  g.font = "600 34px system-ui, sans-serif";
  g.fillText(title, 100, 80, c.width - 130);
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

/** Jaquette sous la pochette : vernis net (la pochette) et légère irisation du film. */
const sleeved = () =>
  new MeshPhysicalMaterial({
    roughness: 0.6,
    clearcoat: 1,
    clearcoatRoughness: 0.04,
    iridescence: 0.2,
    iridescenceIOR: 1.35,
    iridescenceThicknessRange: [250, 480],
  });

/** `covers` : jaquettes à essayer dans l'ordre (romhack sans jaquette → celle du jeu de base). */
export function buildCase(covers: (string | null)[], title: string, onChange: () => void): GameCase {
  const geo = geometries();
  const textures: Texture[] = [];
  const mats: MeshStandardMaterial[] = [];
  const mat = <M extends MeshStandardMaterial>(m: M, alpha = 1): M => {
    m.transparent = true;
    m.userData.base = m.color.clone();
    m.userData.alpha = alpha;
    mats.push(m);
    return m;
  };
  // Plastique transparent : vraie transparence (le fond du lanceur est dans le DOM, sous le
  // canvas : `transmission` ne le verrait pas). Les reflets du studio dessinent les arêtes.
  const plastic = () =>
    mat(
      new MeshPhysicalMaterial({
        color: 0xf2f6ff,
        roughness: 0.06,
        metalness: 0,
        clearcoat: 1,
        clearcoatRoughness: 0.03,
        iridescence: 0.15,
        iridescenceIOR: 1.3,
        iridescenceThicknessRange: [200, 500],
        depthWrite: false,
      }),
      0.2,
    );
  const paper = mat(new MeshStandardMaterial({ color: 0xdedfe2, roughness: 0.9 }));
  const front = mat(sleeved());
  const backArt = mat(sleeved());
  const bookletArt = mat(new MeshStandardMaterial({ roughness: 0.75 }));
  const spineTex = spineTexture(title);
  textures.push(spineTex);
  const spine = mat(new MeshStandardMaterial({ map: spineTex, roughness: 0.55 }));

  const group = new Group();
  const add = (parent: Group, g: BufferGeometry, m: Material | Material[], order: number, x = 0, y = 0, z = 0) => {
    const mesh = new Mesh(g, m);
    mesh.position.set(x, y, z);
    mesh.renderOrder = order;
    parent.add(mesh);
    return mesh;
  };

  // Fond
  add(group, geo.back, plastic(), 2);
  add(group, geo.sheet, backArt, -2, 0.5, 0, -D / 2 - 0.06).rotation.y = Math.PI;
  add(group, geo.sheet, paper, -2, 0.5, 0, -D / 2 - 0.04);
  add(group, geo.spine, spine, -2, -W / 2 - 0.08, 0, 0).rotation.set(0, -Math.PI / 2, Math.PI / 2);

  // Couvercle, pivot sur la charnière
  const lid = new Group();
  lid.position.set(-W / 2, 0, 0);
  group.add(lid);
  add(lid, geo.lid, plastic(), 2);
  add(lid, geo.sheet, front, -2, W / 2 + 0.5, 0, D / 2 + 0.06);
  add(lid, geo.sheet, paper, -2, W / 2 + 0.5, 0, D / 2 + 0.04).rotation.y = Math.PI;
  // Livret sous ses pattes ; sa couverture (face -z) se retrouve vers le haut, couvercle ouvert.
  add(lid, geo.booklet, [paper, paper, paper, paper, paper, bookletArt], -2, W / 2 + 4, 0, D / 2 - T - 0.8);

  let disposed = false;
  const apply = (m: MeshStandardMaterial, map: Texture) => {
    m.map = map;
    m.color.set(0xffffff);
    m.userData.base = m.color.clone();
    m.needsUpdate = true;
  };
  const firstCover = async () => {
    for (const url of covers) {
      const img = url ? await loadImage(url) : null;
      if (img) return img;
    }
    return null;
  };
  void firstCover().then((img) => {
    if (disposed) return;
    let art: Texture;
    if (img) {
      const c = document.createElement("canvas");
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      c.getContext("2d")!.drawImage(img, 0, 0);
      art = canvasTexture(c);
    } else art = plainCover(title);
    const back = backTexture(img);
    textures.push(art, back);
    apply(front, art);
    apply(bookletArt, art);
    apply(backArt, back);
    onChange();
  });

  return {
    group,
    lid,
    mats,
    dispose() {
      disposed = true;
      for (const m of mats) m.dispose();
      for (const t of textures) t.dispose();
    },
  };
}

/** Couleur de base d'un matériau, assombrie (voisines du carrousel). */
export function dimCase(c: GameCase, dim: number, opacity: number) {
  for (const m of c.mats) {
    m.color.copy(m.userData.base as Color).multiplyScalar(dim);
    m.opacity = opacity * ((m.userData.alpha as number | undefined) ?? 1);
  }
}
