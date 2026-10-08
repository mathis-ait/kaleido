import {
  BufferAttribute,
  BufferGeometry,
  Box3,
  Group,
  Matrix4,
  Mesh,
  MeshStandardMaterial,
  Vector2,
  Vector3,
  type Material,
  type Object3D,
  type Texture,
} from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { SUPPORTS, type Support } from "./models";

/**
 * Modèles 3D de vraies cartes et cartouches (Sketchfab, CC-BY 4.0, crédits dans
 * `public/models/<support>/LICENSE.txt`), préparés par `tools/cartridges/` :
 * textures réduites, une seule cartouche gardée pour la Switch.
 *
 * Chaque modèle est réorienté automatiquement à partir de son étiquette (face avant
 * vers +z, haut de l'étiquette vers +y), centré et mis à l'échelle en millimètres.
 * L'étiquette d'origine est recouverte par un quad au même endroit, qui reçoit
 * l'étiquette du jeu (photo de la vraie carte, sinon étiquette composée).
 */

type UV = [number, number];

interface RealModelSpec {
  url: string;
  /** Coins de l'étiquette dans la texture, tels qu'on la lit : haut-gauche, haut-droite, bas-gauche. */
  label: { tl: UV; tr: UV; bl: UV };
  /** Nom du matériau qui porte l'étiquette (sinon le premier matériau texturé). */
  labelMaterial?: string;
  /** Région de la texture occupée par la carte électronique interne (reste opaque en translucide). */
  board?: { u0: number; v0: number; u1: number; v1: number };
  /** Nœuds à masquer. */
  hide?: string[];
  /** Coque d'origine : sa couleur n'est pas retouchée (la texture photographiée est gardée). */
  nativeShell: string;
}

export const REAL_MODELS: Partial<Record<Support, RealModelSpec>> = {
  ds: {
    url: "models/ds/scene.gltf",
    label: { tl: [0.2969, 0.748], tr: [0.6133, 0.748], bl: [0.2969, 0.4043] },
    nativeShell: "gris",
  },
  "3ds": {
    url: "models/3ds/scene.gltf",
    label: { tl: [0.4395, 0.93], tr: [0.8281, 0.93], bl: [0.4395, 0.5137] },
    labelMaterial: "lambert2",
    nativeShell: "gris-clair",
  },
  gba: {
    url: "models/gba/scene.gltf",
    label: { tl: [0.11, 0.6], tr: [0.502, 0.6], bl: [0.11, 0.399] },
    board: { u0: 0.655, v0: 0.36, u1: 0.97, v1: 0.975 },
    nativeShell: "noir",
  },
  switch: {
    url: "models/switch/scene.gltf",
    label: { tl: [0.07, 0.055], tr: [0.93, 0.055], bl: [0.07, 0.845] },
    labelMaterial: "Material.001",
    // Plan blanc de la mise en scène Sketchfab, pas une pièce de la carte.
    hide: ["Plane_2"],
    nativeShell: "noir",
  },
};

export interface RealTemplate {
  /** Modèle orienté, centré, en mm. */
  root: Group;
  /** Quad de l'étiquette (UV 0-1, haut de l'image en v = 1). */
  label: BufferGeometry;
  /** Taille de l'étiquette en mm (largeur, hauteur). */
  labelSize: [number, number];
  /** Taille du modèle en mm (largeur, hauteur, épaisseur). */
  size: [number, number, number];
  spec: RealModelSpec;
}

const templates = new Map<Support, Promise<RealTemplate | null>>();

/** Position 3D d'un point (u, v) de la texture : triangle qui le contient (ou le plus proche, extrapolé). */
function uvToPosition(meshes: Mesh[], target: UV, exact = false): Vector3 | null {
  let best: { score: number; p: Vector3 } | null = null;
  const a = new Vector2();
  const b = new Vector2();
  const c = new Vector2();
  const t = new Vector2(...target);
  for (const mesh of meshes) {
    const geo = mesh.geometry;
    const uv = geo.getAttribute("uv") as BufferAttribute;
    const pos = geo.getAttribute("position") as BufferAttribute;
    if (!uv) continue;
    const index = geo.index;
    const count = index ? index.count : pos.count;
    for (let i = 0; i < count; i += 3) {
      const [i0, i1, i2] = index ? [index.getX(i), index.getX(i + 1), index.getX(i + 2)] : [i, i + 1, i + 2];
      a.fromBufferAttribute(uv, i0);
      b.fromBufferAttribute(uv, i1);
      c.fromBufferAttribute(uv, i2);
      const d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
      if (Math.abs(d) < 1e-12) continue;
      const w0 = ((b.y - c.y) * (t.x - c.x) + (c.x - b.x) * (t.y - c.y)) / d;
      const w1 = ((c.y - a.y) * (t.x - c.x) + (a.x - c.x) * (t.y - c.y)) / d;
      const w2 = 1 - w0 - w1;
      const score = Math.min(w0, w1, w2);
      if (best && score <= best.score) continue;
      const p = new Vector3()
        .addScaledVector(new Vector3().fromBufferAttribute(pos, i0), w0)
        .addScaledVector(new Vector3().fromBufferAttribute(pos, i1), w1)
        .addScaledVector(new Vector3().fromBufferAttribute(pos, i2), w2)
        .applyMatrix4(mesh.matrixWorld);
      best = { score, p };
      if (score >= 0) return p;
    }
  }
  return exact ? null : (best?.p ?? null);
}

/** Sépare les triangles de la carte électronique (région UV `board`) dans un maillage à part. */
function splitBoard(mesh: Mesh, board: NonNullable<RealModelSpec["board"]>): Mesh | null {
  const geo = mesh.geometry;
  const uv = geo.getAttribute("uv");
  if (!uv || !geo.index) return null;
  const keep: number[] = [];
  const moved: number[] = [];
  const index = geo.index;
  for (let i = 0; i < index.count; i += 3) {
    const ids = [index.getX(i), index.getX(i + 1), index.getX(i + 2)];
    const u = ids.reduce((s, k) => s + uv.getX(k), 0) / 3;
    const v = ids.reduce((s, k) => s + uv.getY(k), 0) / 3;
    const inBoard = u >= board.u0 && u <= board.u1 && v >= board.v0 && v <= board.v1;
    (inBoard ? moved : keep).push(...ids);
  }
  if (!moved.length) return null;
  geo.setIndex(keep);
  const boardGeo = geo.clone();
  boardGeo.setIndex(moved);
  const boardMesh = new Mesh(boardGeo, (mesh.material as Material).clone());
  boardMesh.name = `${mesh.name}-board`;
  boardMesh.userData.part = "board";
  mesh.parent!.add(boardMesh);
  boardMesh.position.copy(mesh.position);
  boardMesh.quaternion.copy(mesh.quaternion);
  boardMesh.scale.copy(mesh.scale);
  return boardMesh;
}

async function build(support: Support, spec: RealModelSpec): Promise<RealTemplate | null> {
  const gltf = await new GLTFLoader().loadAsync(spec.url);
  const scene = gltf.scene;
  for (const name of spec.hide ?? []) {
    const node = scene.getObjectByName(name);
    if (node) node.visible = false;
  }
  scene.updateMatrixWorld(true);

  const meshes: Mesh[] = [];
  scene.traverse((o) => {
    if (o instanceof Mesh && o.visible) meshes.push(o);
  });
  const labelMeshes = meshes.filter((m) => {
    const mat = m.material as MeshStandardMaterial;
    return spec.labelMaterial ? mat.name === spec.labelMaterial : !!mat.map;
  });
  // Coins mesurés un peu à l'intérieur de l'étiquette (sûrement dans ses triangles), puis
  // extrapolés jusqu'aux vrais coins : l'étiquette est plane.
  const INSET = 0.12;
  const at = (s: number, t: number): UV => {
    const { tl, tr, bl } = spec.label;
    return [tl[0] + s * (tr[0] - tl[0]) + t * (bl[0] - tl[0]), tl[1] + s * (tr[1] - tl[1]) + t * (bl[1] - tl[1])];
  };
  const p00 = uvToPosition(labelMeshes, at(INSET, INSET), true);
  const p10 = uvToPosition(labelMeshes, at(1 - INSET, INSET), true);
  const p01 = uvToPosition(labelMeshes, at(INSET, 1 - INSET), true);
  if (!p00 || !p10 || !p01) return null;
  const ex = p10.clone().sub(p00).divideScalar(1 - 2 * INSET);
  const ey = p01.clone().sub(p00).divideScalar(1 - 2 * INSET);
  const tl = p00.clone().addScaledVector(ex, -INSET).addScaledVector(ey, -INSET);
  const tr = tl.clone().add(ex);
  const bl = tl.clone().add(ey);

  // Repère de l'étiquette : droite → +x, haut → +y, face avant → +z.
  const right = tr.clone().sub(tl).normalize();
  const up = tl.clone().sub(bl);
  up.addScaledVector(right, -up.dot(right)).normalize();
  const out = new Vector3().crossVectors(right, up).normalize();
  const basis = new Matrix4().makeBasis(right, up, out);
  const toLabel = basis.clone().invert();

  const fix = new Group();
  fix.add(scene);
  fix.quaternion.setFromRotationMatrix(toLabel);
  fix.updateMatrixWorld(true);

  // Pièces internes à part (opaques), avant le calcul de la boîte.
  if (spec.board) for (const m of meshes) splitBoard(m, spec.board);
  for (const m of meshes) m.userData.part ??= labelMeshes.includes(m) && spec.labelMaterial ? "labelBase" : "shell";
  fix.updateMatrixWorld(true);

  const box = new Box3().setFromObject(fix);
  const dims = box.getSize(new Vector3());
  // Mise à l'échelle en mm d'après la largeur réelle du support.
  const scale = SUPPORTS[support].size[0] / dims.x;
  const center = box.getCenter(new Vector3());
  const root = new Group();
  root.add(fix);
  fix.position.copy(center).multiplyScalar(-1);
  root.scale.setScalar(scale);
  fix.updateMatrix();
  root.updateMatrixWorld(true);

  // Quad de l'étiquette dans le repère de `root` (mm), décollé de 0,03 mm.
  const toRoot = (p: Vector3) => p.clone().applyMatrix4(fix.matrix).multiplyScalar(scale);
  const [a, b, c] = [toRoot(tl), toRoot(tr), toRoot(bl)];
  const d = b.clone().add(c).sub(a);
  for (const p of [a, b, c, d]) p.z += 0.03;
  const label = new BufferGeometry();
  label.setAttribute("position", new BufferAttribute(new Float32Array([...c.toArray(), ...d.toArray(), ...b.toArray(), ...c.toArray(), ...b.toArray(), ...a.toArray()]), 3));
  label.setAttribute("uv", new BufferAttribute(new Float32Array([0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1]), 2));
  label.computeVertexNormals();

  // Le modèle est déjà centré dans `root` : on renvoie un groupe neutre qui l'enveloppe.
  const holder = new Group();
  holder.add(root);
  return {
    root: holder,
    label,
    labelSize: [a.distanceTo(b), a.distanceTo(c)],
    size: [dims.x * scale, dims.y * scale, dims.z * scale],
    spec,
  };
}

/** Modèle réel d'un support (chargé une fois), ou null s'il n'y en a pas ou s'il ne charge pas. */
export function realTemplate(support: Support): Promise<RealTemplate | null> {
  const spec = REAL_MODELS[support];
  if (!spec) return Promise.resolve(null);
  let p = templates.get(support);
  if (!p) {
    p = build(support, spec).catch((e) => {
      console.warn(`[cartouches] modèle ${support} illisible`, e);
      return null;
    });
    templates.set(support, p);
  }
  return p;
}

/** Copie d'un modèle pour une cartouche : géométries et textures partagées, matériaux propres. */
export function instantiate(t: RealTemplate): { root: Object3D; shells: MeshStandardMaterial[]; others: MeshStandardMaterial[] } {
  const root = t.root.clone(true);
  const shells: MeshStandardMaterial[] = [];
  const others: MeshStandardMaterial[] = [];
  root.traverse((o) => {
    if (!(o instanceof Mesh)) return;
    const mat = (o.material as MeshStandardMaterial).clone();
    mat.userData.map = mat.map;
    // Couleur d'origine (pièces sans texture : plastique noir de la Switch, contacts dorés).
    mat.userData.color = mat.color.clone();
    o.material = mat;
    (o.userData.part === "shell" ? shells : others).push(mat);
  });
  return { root, shells, others };
}

export function disposeTemplates() {
  for (const p of templates.values()) {
    void p.then((t) => {
      t?.label.dispose();
      t?.root.traverse((o) => {
        if (!(o instanceof Mesh)) return;
        o.geometry.dispose();
        const mat = o.material as MeshStandardMaterial;
        for (const tex of [mat.map, mat.normalMap, mat.roughnessMap, mat.metalnessMap] as (Texture | null)[]) tex?.dispose();
        mat.dispose();
      });
    });
  }
  templates.clear();
}
