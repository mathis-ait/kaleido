<script setup lang="ts">
/**
 * Mode Cartouche : la scène three.js qui dessine les cartouches du carrousel.
 *
 * Elle ne fait que dessiner : Launcher.vue garde l'état (jeu sélectionné, glisser,
 * inclinaison, manette) et les boutons du carrousel restent dans le DOM, invisibles,
 * pour les clics, le glisser et le lecteur d'écran. Même géométrie que le carrousel
 * CSS (décalages 250 px puis 168 px, rotation 44°, perspective 1100 px) : passer d'un
 * mode à l'autre ne déplace rien.
 *
 * Rendu à la demande : une image quand une prop change ou pendant une animation,
 * plus l'oscillation lente de la cartouche centrale à 30 images par seconde, coupée
 * quand la fenêtre n'a pas le focus ou qu'un émulateur tourne.
 */
import { onMounted, onUnmounted, ref, watch } from "vue";
import {
  AmbientLight,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  CanvasTexture,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  NeutralToneMapping,
  PerspectiveCamera,
  RepeatWrapping,
  Plane,
  PMREMGenerator,
  Scene,
  Shape,
  ShapeGeometry,
  SRGBColorSpace,
  Vector2,
  Vector3,
  WebGLRenderer,
  type Texture,
} from "three";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import type { Detection } from "../../types";
import { isKaleidoRom } from "../../types";
import { currentTheme } from "../../theme";
import { audio, insertClick } from "../audio";
import { PHOTO_CROP, cartPhotoKey, cartridgeKey, resolveLook, type Look, type Slot, type StoredLook } from "./models";
import { disposeGeometries, supportGeometry } from "./geometry";
import { forgetLabel, labelTexture } from "./label";
import { REAL_MODELS, disposeTemplates, instantiate, realTemplate, type RealTemplate } from "./realModels";
import { convertFileSrc } from "@tauri-apps/api/core";
import { INITIAL, startInsertion, type InsertState, type Insertion } from "./insert";

const props = defineProps<{
  games: Detection[];
  index: number;
  dragShift: number;
  dragging: boolean;
  /** Inclinaison à la souris (degrés). */
  tilt: { x: number; y: number };
  /** Stick droit de la manette (-1 à 1). */
  stick: { x: number; y: number };
  /** Choix « Inspecter » par clé de jeu. */
  looks: Record<string, StoredLook>;
  /** Couleur dominante du jeu, pour la silhouette de la console. */
  tint: string;
  /** Carrousel DOM : la scène se cale sur sa position et sa perspective. */
  stage: HTMLElement | null;
  ring: HTMLElement | null;
}>();

const emit = defineEmits<{ fade: [value: number]; failed: [reason: string] }>();

const canvas = ref<HTMLCanvasElement | null>(null);

// --- Géométrie du carrousel (mêmes valeurs que `itemStyle` dans Launcher.vue)

interface Pose {
  x: number;
  z: number;
  rot: number;
  scale: number;
  dim: number;
  opacity: number;
}

function poseOf(i: number): Pose {
  const k = i - props.index;
  const abs = Math.abs(k);
  const sign = Math.sign(k);
  return {
    x: (k === 0 ? 0 : sign * (250 + (abs - 1) * 168)) + props.dragShift,
    z: k === 0 ? 40 : -140 - abs * 26,
    rot: k === 0 ? 0 : -sign * 44,
    scale: k === 0 ? 1.16 : 0.9,
    dim: k === 0 ? 1 : Math.max(0.45, 0.85 - abs * 0.08),
    opacity: abs > 6 ? 0 : 1 - Math.max(0, abs - 2) * 0.18,
  };
}

/** Taille d'une cartouche à l'écran : tient dans la boîte d'une jaquette (300 × 266 px). */
function pxPerMm(size: [number, number, number]) {
  return Math.min(280 / size[0], 250 / size[1]);
}

/** cubic-bezier(0.2, 0.85, 0.25, 1), la courbe des transitions CSS du carrousel. */
function bezier(x1: number, y1: number, x2: number, y2: number) {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const sx = (t: number) => ((ax * t + bx) * t + cx) * t;
  const dx = (t: number) => (3 * ax * t + 2 * bx) * t + cx;
  return (x: number) => {
    let t = x;
    for (let i = 0; i < 6; i++) {
      const d = dx(t);
      if (Math.abs(d) < 1e-6) break;
      t -= (sx(t) - x) / d;
    }
    t = Math.min(1, Math.max(0, t));
    return ((ay * t + by) * t + cy) * t;
  };
}
const ease = bezier(0.2, 0.85, 0.25, 1);

// --- Scène

interface Item {
  game: Detection;
  key: string;
  look: Look;
  group: Group;
  body: Group;
  /** Matériaux de la coque (recoloriés, translucides) ; un seul pour un modèle procédural. */
  shells: MeshStandardMaterial[];
  /** Autres pièces (carte électronique, contacts) : jamais translucides. */
  others: MeshStandardMaterial[];
  labelMat: MeshStandardMaterial;
  labelMesh: Mesh;
  labelSig: string;
  /** Modèle réel (Sketchfab) ou procédural (`geometry.ts`). */
  real: RealTemplate | null;
  /** Modèle réel pas encore chargé : la cartouche reste invisible. */
  ready: boolean;
  from: Pose;
  to: Pose;
  cur: Pose;
  t0: number;
  dur: number;
  sizePx: [number, number];
}

let renderer: WebGLRenderer | null = null;
let scene: Scene;
let camera: PerspectiveCamera;
let root: Group;
let ambient: AmbientLight;
let envTexture: Texture | null = null;
const items = new Map<string, Item>();

/** Silhouette de la console et sa fente (insertion). */
let slot: { group: Group; kind: Slot; width: number; body: MeshBasicMaterial; hole: MeshBasicMaterial } | null = null;
/** Haut de la fente, en px sous le centre du carrousel. */
const SLOT_Y = -190;
const SLOT_Z = 42;
const clip = new Plane(new Vector3(0, 1, 0), 0);

let insertion: Insertion | null = null;
let insert: InsertState = { ...INITIAL };
let insertPath: string | null = null;

/** Inclinaison lissée de la cartouche centrale (comme la transition CSS de 0,2 s). */
const tiltNow = { x: 0, y: 0 };
let zoom = 1;
let firstFrame = true;

function hasFocus() {
  return document.hasFocus() && document.visibilityState === "visible";
}

/** Oscillation permanente de la cartouche centrale : seulement au premier plan, hors jeu. */
const swaying = () => !insertion && hasFocus() && audio.running.length === 0;

function themeLight() {
  const theme = currentTheme.value;
  ambient.intensity = theme === "jour" ? 1.0 : theme === "graphite" ? 0.7 : 0.55;
  invalidate();
}

function buildEnvironment(r: WebGLRenderer) {
  // Reflets d'un studio photo (boîtes à lumière, murs neutres) : ce sont eux qui font lire
  // les arrondis, la jointure et le rebord du plastique. Calculé une fois, à l'ouverture.
  const pmrem = new PMREMGenerator(r);
  const room = new RoomEnvironment();
  envTexture = pmrem.fromScene(room, 0.03).texture;
  room.dispose();
  pmrem.dispose();
  return envTexture;
}

/**
 * Grain « peau d'orange » du plastique moulé : bruit fin, flouté, en relief très léger.
 * Une seule texture partagée par toutes les coques (UV de l'extrusion en mm).
 */
function plasticGrain(): CanvasTexture {
  const size = 256;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const g = c.getContext("2d")!;
  const img = g.createImageData(size, size);
  let seed = 7;
  const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < img.data.length; i += 4) {
    const v = 128 + (rand() - 0.5) * 90;
    img.data[i] = img.data[i + 1] = img.data[i + 2] = v;
    img.data[i + 3] = 255;
  }
  g.putImageData(img, 0, 0);
  g.filter = "blur(2px)";
  g.drawImage(c, 0, 0);
  const t = new CanvasTexture(c);
  t.wrapS = t.wrapT = RepeatWrapping;
  t.repeat.set(0.35, 0.35);
  return t;
}
let grain: CanvasTexture | null = null;

function init() {
  const el = canvas.value!;
  try {
    renderer = new WebGLRenderer({ canvas: el, antialias: true, alpha: true, powerPreference: "high-performance" });
  } catch (e) {
    emit("failed", String(e));
    return;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = NeutralToneMapping;
  renderer.localClippingEnabled = true;
  renderer.setClearColor(0x000000, 0);

  scene = new Scene();
  scene.environment = buildEnvironment(renderer);
  scene.environmentIntensity = 0.85;
  grain = plasticGrain();
  camera = new PerspectiveCamera(40, 1, 10, 6000);
  root = new Group();
  scene.add(root);

  ambient = new AmbientLight(0xffffff, 0.55);
  scene.add(ambient);
  const key = new DirectionalLight(0xffffff, 1.6);
  key.position.set(-400, 600, 900);
  scene.add(key);
  const fill = new DirectionalLight(0xffffff, 0.5);
  fill.position.set(600, -100, 500);
  scene.add(fill);
  themeLight();

  // @ts-expect-error mesure en développement (`renderer.info.memory`)
  if (import.meta.env.DEV) window.__kaleidoScene = { renderer, items };
}

// --- Cartouches

function lookSig(look: Look) {
  return `${look.support.id}|${look.shell.id}|${look.finish}|${look.wear}|${look.customLabel}`;
}

const WHITE = new Color(1, 1, 1);

/** Textures de coque recoloriées, par modèle et couleur (partagées entre cartouches). */
const recolors = new Map<string, CanvasTexture>();

/**
 * Coque d'un modèle réel dans une autre couleur : la luminosité de la photo (rayures,
 * ombres, grain) est gardée, la teinte remplacée ; l'étiquette et la carte électronique
 * ne sont pas touchées.
 */
function recolored(t: RealTemplate, source: Texture, color: string): CanvasTexture | null {
  const key = `${t.spec.url}|${color}`;
  let tex = recolors.get(key);
  if (tex) return tex;
  const image = source.image as CanvasImageSource & { width: number; height: number };
  if (!image?.width) return null;
  const size = Math.min(1024, image.width);
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(image, 0, 0, size, size);
  const data = g.getImageData(0, 0, size, size);
  const px = data.data;
  const [r, gg, b] = new Color(color).convertLinearToSRGB().toArray().map((v) => v * 255);
  const { tl, tr, bl } = t.spec.label;
  const lu0 = Math.min(tl[0], tr[0], bl[0]);
  const lu1 = Math.max(tl[0], tr[0], bl[0]);
  const lv0 = Math.min(tl[1], tr[1], bl[1]);
  const lv1 = Math.max(tl[1], tr[1], bl[1]);
  const board = t.spec.board;
  const skip = (u: number, v: number) => (u >= lu0 && u <= lu1 && v >= lv0 && v <= lv1) || (!!board && u >= board.u0 && u <= board.u1 && v >= board.v0 && v <= board.v1);
  // Luminosité de référence du plastique : médiane des pixels de coque.
  const lums: number[] = [];
  for (let p = 0; p < size * size; p += 97) {
    if (!skip((p % size) / size, Math.floor(p / size) / size)) lums.push(px[p * 4] + px[p * 4 + 1] + px[p * 4 + 2]);
  }
  lums.sort((x, y) => x - y);
  const ref = Math.max(1, lums[lums.length >> 1] ?? 1);
  for (let p = 0; p < size * size; p++) {
    if (skip((p % size) / size, Math.floor(p / size) / size)) continue;
    const i = p * 4;
    const k = Math.min(1.8, (px[i] + px[i + 1] + px[i + 2]) / ref);
    px[i] = Math.min(255, r * k);
    px[i + 1] = Math.min(255, gg * k);
    px[i + 2] = Math.min(255, b * k);
  }
  g.putImageData(data, 0, 0);
  tex = new CanvasTexture(c);
  tex.flipY = source.flipY;
  tex.colorSpace = SRGBColorSpace;
  tex.anisotropy = 4;
  recolors.set(key, tex);
  return tex;
}

function applyMaterial(item: Item, center: boolean) {
  const { look } = item;
  const translucent = look.finish === "translucide";
  const worn = look.wear === "jouee" ? 0.15 : 0;
  const metal = look.shell.id === "argent" || look.shell.id === "or";
  for (const m of item.shells) {
    if (item.real) {
      // Modèle réel : sa texture photographiée, recoloriée si une autre coque est choisie.
      const native = look.shell.id === item.real.spec.nativeShell;
      const original = (m.userData.map as Texture | null) ?? null;
      const map = native || !original ? original : (recolored(item.real, original, look.shell.color) ?? original);
      if (m.map !== map) {
        m.map = map;
        m.needsUpdate = true;
      }
      m.userData.base = native || !original ? (m.userData.color as Color) : WHITE;
    } else {
      m.roughness = (look.finish === "brillant" ? 0.22 : translucent ? 0.28 : 0.46) + worn;
      m.metalness = metal ? 0.35 : 0;
      m.userData.base = new Color(look.shell.color);
    }
    if (m instanceof MeshPhysicalMaterial) {
      m.clearcoat = translucent ? 0.6 : look.finish === "brillant" ? 0.5 : 0;
      m.clearcoatRoughness = 0.2 + worn;
    }
    // Translucide : vraie transparence plutôt que `transmission`, qui ne verrait que la scène
    // three.js (vide) et pas le fond du lanceur, dessiné dans le DOM sous le canvas. Plus
    // léger aussi : aucune passe de rendu supplémentaire, même pour les voisines.
    m.userData.alpha = translucent ? (center ? 0.6 : 0.72) : 1;
  }
  for (const m of item.others) m.userData.base = (m.userData.color as Color | undefined) ?? WHITE;
}

function labelInput(item: Item) {
  const d = item.game;
  const seedDetail = d.details.find((x) => x.label === "Seed")?.value;
  const photo = item.real ? cartPhotoKey(d) : null;
  return {
    key: item.key,
    title: d.game?.name ?? d.title,
    support: item.look.support,
    wear: item.look.wear,
    seed: d.kaleido?.seed ?? (seedDetail ? Number(seedDetail) : null),
    randomized: isKaleidoRom(d),
    custom: item.look.customLabel,
    photo: photo ? convertFileSrc(`${photo}.png`, "cover") : null,
    photoCrop: PHOTO_CROP[item.look.support.id] ?? null,
    size: item.real?.labelSize,
  };
}

function loadLabel(item: Item) {
  const sig = lookSig(item.look);
  item.labelSig = sig;
  void labelTexture(labelInput(item))
    .then((texture) => {
      if (items.get(item.game.path) !== item || item.labelSig !== sig) return texture.dispose();
      item.labelMat.map?.dispose();
      item.labelMat.map = texture;
      item.labelMat.needsUpdate = true;
      item.labelMesh.material = item.labelMat;
      invalidate();
    })
    .catch(() => undefined);
}

/**
 * Étiquette en cours de chargement : papier vierge balayé par un reflet lent (la texture
 * défile), partagée par toutes les cartouches en attente.
 */
let loading: { mat: MeshStandardMaterial; tex: CanvasTexture } | null = null;
function loadingMaterial() {
  if (loading) return loading.mat;
  const c = document.createElement("canvas");
  c.width = 512;
  c.height = 8;
  const g = c.getContext("2d")!;
  const sheen = g.createLinearGradient(0, 0, 512, 0);
  sheen.addColorStop(0, "#e2e1dc");
  sheen.addColorStop(0.42, "#e2e1dc");
  sheen.addColorStop(0.5, "#f7f6f2");
  sheen.addColorStop(0.58, "#e2e1dc");
  sheen.addColorStop(1, "#e2e1dc");
  g.fillStyle = sheen;
  g.fillRect(0, 0, 512, 8);
  const tex = new CanvasTexture(c);
  tex.colorSpace = SRGBColorSpace;
  tex.wrapS = RepeatWrapping;
  tex.repeat.set(0.7, 1);
  tex.center.set(0.5, 0.5);
  tex.rotation = -0.5;
  const mat = new MeshStandardMaterial({ map: tex, roughness: 0.55, metalness: 0 });
  loading = { mat, tex };
  return mat;
}

/** Étiquette imprimée : papier couché, un peu satiné ; coins arrondis par transparence. */
const newLabelMaterial = () => new MeshPhysicalMaterial({ color: 0xffffff, roughness: 0.45, metalness: 0, clearcoat: 0.3, clearcoatRoughness: 0.35, alphaTest: 0.5 });

/** Modèle procédural (cartouche GB, ou repli si un modèle réel ne charge pas). */
function buildProcedural(item: Item) {
  const geo = supportGeometry(item.look.support.id);
  const k = pxPerMm(geo.size);
  const shellMat = new MeshPhysicalMaterial({ color: item.look.shell.color, bumpMap: grain, bumpScale: 0.12 });
  item.labelMesh = new Mesh(geo.label, item.labelMat.map ? item.labelMat : loadingMaterial());
  item.body.add(new Mesh(geo.shell, shellMat), item.labelMesh);
  item.body.scale.setScalar(k);
  item.shells = [shellMat];
  item.sizePx = [geo.size[0] * k, geo.size[1] * k];
}

function buildReal(item: Item, t: RealTemplate) {
  const k = pxPerMm(t.size);
  const inst = instantiate(t);
  item.labelMesh = new Mesh(t.label, item.labelMat.map ? item.labelMat : loadingMaterial());
  item.body.add(inst.root, item.labelMesh);
  item.body.scale.setScalar(k);
  item.shells = inst.shells;
  item.others = inst.others;
  item.real = t;
  item.sizePx = [t.size[0] * k, t.size[1] * k];
}

function createItem(game: Detection, i: number): Item {
  const key = cartridgeKey(game);
  const look = resolveLook(game, props.looks[key]);
  const body = new Group();
  const group = new Group();
  group.add(body);
  root.add(group);
  const pose = poseOf(i);
  const item: Item = {
    game,
    key,
    look,
    group,
    body,
    shells: [],
    others: [],
    labelMat: newLabelMaterial(),
    labelMesh: new Mesh(),
    labelSig: "",
    real: null,
    ready: false,
    from: pose,
    to: pose,
    cur: { ...pose },
    t0: 0,
    dur: 0,
    sizePx: [0, 0],
  };
  const finish = (t: RealTemplate | null) => {
    if (items.get(game.path) !== item) return;
    if (t) buildReal(item, t);
    else buildProcedural(item);
    item.ready = true;
    applyMaterial(item, props.games[props.index]?.path === game.path);
    loadLabel(item);
    invalidate();
  };
  if (REAL_MODELS[look.support.id]) void realTemplate(look.support.id).then(finish);
  else queueMicrotask(() => finish(null));
  return item;
}

function disposeItem(item: Item) {
  root.remove(item.group);
  // Géométries et textures des modèles réels : partagées, gardées ; seuls les matériaux sont propres.
  for (const m of [...item.shells, ...item.others]) m.dispose();
  item.labelMat.map?.dispose();
  item.labelMat.dispose();
}

/** Instancie les jeux proches (7 de chaque côté, comme les jaquettes) et vise leur nouvelle pose. */
function sync() {
  if (!renderer) return;
  const now = performance.now();
  const wanted = new Set<string>();
  props.games.forEach((g, i) => {
    if (Math.abs(i - props.index) > 7) return;
    wanted.add(g.path);
    let item = items.get(g.path);
    if (!item) {
      item = createItem(g, i);
      items.set(g.path, item);
      return;
    }
    item.game = g;
    const to = poseOf(i);
    if (Object.keys(to).some((k) => to[k as keyof Pose] !== item!.to[k as keyof Pose])) {
      item.from = { ...item.cur };
      item.to = to;
      item.t0 = now;
      item.dur = props.dragging ? 120 : 500;
    }
    applyMaterial(item, i === props.index);
  });
  for (const [path, item] of items) {
    if (!wanted.has(path)) {
      disposeItem(item);
      items.delete(path);
    }
  }
  invalidate();
}

function refreshLooks() {
  for (const item of items.values()) {
    const look = resolveLook(item.game, props.looks[item.key]);
    const before = item.labelSig;
    item.look = look;
    applyMaterial(item, props.games[props.index]?.path === item.game.path);
    const sig = lookSig(look);
    const labelChanged = before.split("|").slice(3).join("|") !== sig.split("|").slice(3).join("|");
    if (labelChanged) loadLabel(item);
    else item.labelSig = sig;
  }
  invalidate();
}

/** Recharge l'étiquette d'un jeu (image personnalisée changée). */
function reloadLabel(path: string) {
  const item = items.get(path);
  if (!item) return;
  forgetLabel(item.key);
  loadLabel(item);
}

// --- Silhouette de la console

function roundedRect(w: number, h: number, r: number) {
  const s = new Shape();
  s.moveTo(-w / 2 + r, 0);
  s.lineTo(w / 2 - r, 0);
  s.quadraticCurveTo(w / 2, 0, w / 2, -r);
  s.lineTo(w / 2, -h + r);
  s.quadraticCurveTo(w / 2, -h, w / 2 - r, -h);
  s.lineTo(-w / 2 + r, -h);
  s.quadraticCurveTo(-w / 2, -h, -w / 2, -h + r);
  s.lineTo(-w / 2, -r);
  s.quadraticCurveTo(-w / 2, 0, -w / 2 + r, 0);
  return s;
}

function consoleShape(kind: Slot): Shape {
  switch (kind) {
    case "ds-back":
      return roundedRect(560, 420, 46);
    case "3ds-back":
      return roundedRect(600, 420, 60);
    case "gba-top":
      return roundedRect(760, 420, 150);
    case "gb-top":
      return roundedRect(470, 520, 22);
    case "switch-dock": {
      const s = new Shape();
      s.moveTo(-330, 0);
      s.lineTo(330, 0);
      s.lineTo(380, -420);
      s.lineTo(-380, -420);
      s.lineTo(-330, 0);
      return s;
    }
  }
}

function buildSlot(kind: Slot, cartWidth: number) {
  if (slot && slot.kind === kind && Math.abs(slot.width - cartWidth) < 1) return slot;
  if (slot) {
    root.remove(slot.group);
    slot.group.traverse((o) => o instanceof Mesh && o.geometry.dispose());
    slot.body.dispose();
    slot.hole.dispose();
  }
  const group = new Group();
  const body = new MeshBasicMaterial({ color: 0x0d0f18, transparent: true, opacity: 0 });
  const hole = new MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0 });
  const silhouette = new Mesh(new ShapeGeometry(consoleShape(kind), 12), body);
  const opening = new Mesh(new ShapeGeometry(roundedRect(cartWidth + 18, 12, 4), 4), hole);
  opening.position.z = 0.5;
  group.add(silhouette, opening);
  // Juste derrière la cartouche insérée (z 60) : la fente et la coupe tombent au même endroit à l'écran.
  group.position.set(0, SLOT_Y, SLOT_Z);
  group.visible = false;
  root.add(group);
  slot = { group, kind, width: cartWidth, body, hole };
  return slot;
}

// --- Animation et rendu

let raf = 0;
let timer: number | undefined;

function invalidate() {
  clearTimeout(timer);
  timer = undefined;
  if (!raf && renderer) raf = requestAnimationFrame(frame);
}

function placeCamera() {
  const el = canvas.value!;
  const w = el.clientWidth;
  const h = el.clientHeight;
  if (!w || !h) return;
  const size = renderer!.getSize(new Vector2());
  if (size.x !== w || size.y !== h) renderer!.setSize(w, h, false);
  const c = el.getBoundingClientRect();
  const s = props.stage?.getBoundingClientRect();
  const r = props.ring?.getBoundingClientRect();
  // Même règle que la feuille de style du lanceur : carrousel réduit sur les petites fenêtres.
  zoom = window.matchMedia("(max-height: 820px)").matches ? 0.78 : 1;
  const cx = s ? s.left + s.width / 2 - c.left : w / 2;
  const cy = s ? s.top + s.height / 2 - c.top : h / 2;
  const d = 1100 * zoom;
  camera.fov = (2 * Math.atan(h / 2 / d) * 180) / Math.PI;
  camera.aspect = w / h;
  camera.position.set(0, 0, d);
  camera.setViewOffset(w, h, w / 2 - cx, h / 2 - cy, w, h);
  camera.updateProjectionMatrix();
  root.scale.setScalar(zoom);
  root.position.set(r ? r.left - c.left - cx : 0, r ? -(r.top - c.top - cy) : 0, 0);
}

function step(now: number): boolean {
  let busy = false;
  const center = props.games[props.index]?.path;
  // Inclinaison : souris et stick droit, rattrapés en douceur.
  const target = { x: props.tilt.x - props.stick.y * 12, y: props.tilt.y + props.stick.x * 16 };
  tiltNow.x += (target.x - tiltNow.x) * 0.25;
  tiltNow.y += (target.y - tiltNow.y) * 0.25;
  if (Math.abs(target.x - tiltNow.x) > 0.05 || Math.abs(target.y - tiltNow.y) > 0.05) busy = true;
  const sway = swaying() ? Math.sin((now / 6000) * Math.PI * 2) * 2 : 0;
  const deg = Math.PI / 180;

  let waiting = false;
  for (const item of items.values()) {
    const p = item.dur ? Math.min(1, (now - item.t0) / item.dur) : 1;
    if (p < 1) busy = true;
    const e = ease(p);
    for (const k of Object.keys(item.cur) as (keyof Pose)[]) item.cur[k] = item.from[k] + (item.to[k] - item.from[k]) * e;
    const { cur } = item;
    const isCenter = item.game.path === center;
    const inserting = insertPath === item.game.path;
    let x = cur.x;
    let y = 0;
    let z = cur.z;
    let scale = cur.scale;
    let opacity = cur.opacity;
    let rotX = 0;
    let rotY = 0;
    if (inserting) {
      // Face à la fente, descend jusqu'à elle, puis s'enfonce (coupée au ras de la fente).
      const [, hPx] = item.sizePx;
      const touch = SLOT_Y + hPx / 2;
      scale = cur.scale + (1 - cur.scale) * insert.descend;
      y = touch * insert.descend - (hPx + 8) * insert.sink;
      z = cur.z + 20 * insert.descend;
      rotX = -Math.sin(insert.descend * Math.PI) * 10 * deg;
      rotY = 0;
    } else {
      if (insertPath && !isCenter) {
        x += Math.sign(cur.x || 1) * 260 * insert.spread;
        opacity *= 1 - insert.spread;
      }
      if (isCenter) {
        rotX = tiltNow.x * deg;
        rotY = (tiltNow.y + sway) * deg;
      }
    }
    item.group.position.set(x, y, z);
    item.group.rotation.set(0, cur.rot * deg, 0);
    item.group.scale.setScalar(scale);
    item.body.rotation.set(rotX, rotY, 0);
    item.group.visible = item.ready && opacity > 0.01;
    const transparent = opacity < 0.999;
    for (const m of [...item.shells, ...item.others, item.labelMat]) {
      const alpha = opacity * ((m.userData.alpha as number | undefined) ?? 1);
      const blend = transparent || alpha < 0.999;
      if (m.transparent !== blend) {
        m.transparent = blend;
        m.needsUpdate = true;
      }
      m.opacity = alpha;
      m.clippingPlanes = inserting ? [clip] : null;
    }
    for (const m of [...item.shells, ...item.others]) {
      const base = m.userData.base as Color | undefined;
      if (base) m.color.copy(base).multiplyScalar(cur.dim);
    }
    item.labelMat.color.setScalar(cur.dim);
    if (item.ready && !item.labelMat.map && opacity > 0.01) waiting = true;
  }

  // Reflet des étiquettes en attente : il défile tant qu'une étiquette charge.
  if (waiting && loading) {
    loading.tex.offset.x = -((now / 1600) % 1);
    busy = true;
  }

  if (slot) {
    slot.group.visible = insert.slot > 0.001;
    slot.group.position.y = SLOT_Y - (1 - insert.slot) * 240;
    slot.body.color.set(props.tint).lerp(new Color(0x1b1e2b), 0.78);
    slot.body.opacity = insert.slot;
    slot.hole.opacity = insert.slot * 0.95;
    // Plan de coupe au ras de la fente, en coordonnées du monde.
    clip.constant = -(root.position.y + SLOT_Y * zoom);
  }
  return busy;
}

function frame(now: number) {
  raf = 0;
  if (!renderer) return;
  placeCamera();
  const busy = step(now);
  renderer.render(scene, camera);
  if (firstFrame) {
    firstFrame = false;
    try {
      performance.mark("cartridge-first-frame");
      const m = performance.measure("cartridge-open", "launcher-open", "cartridge-first-frame");
      if (import.meta.env.DEV) console.debug(`[cartouches] première image en ${Math.round(m.duration)} ms`);
    } catch {
      /* repère d'ouverture absent */
    }
  }
  if (busy || insertion) invalidate();
  else if (swaying()) timer = window.setTimeout(invalidate, 33);
}

// --- Insertion

/** Lance la séquence d'insertion de la cartouche centrale. */
function startInsert(hooks: { launch(): void; done(): void; cancelled(): void }): boolean {
  const g = props.games[props.index];
  const item = g ? items.get(g.path) : undefined;
  if (!renderer || !item || insertion) return false;
  insertPath = g.path;
  buildSlot(item.look.support.slot, item.sizePx[0]);
  insertion = startInsertion({
    frame: (s) => {
      insert = s;
      emit("fade", s.fade);
      invalidate();
    },
    launch: hooks.launch,
    click: () => insertClick(item.look.support.click),
    done: () => {
      insertion = null;
      hooks.done();
    },
    cancelled: () => {
      insertion = null;
      insertPath = null;
      insert = { ...INITIAL };
      invalidate();
      hooks.cancelled();
    },
  });
  return true;
}

function cancelInsert(): boolean {
  return insertion?.cancel() ?? false;
}

/** Remet la cartouche en place derrière l'écran noir, après le lancement. */
function resetInsert() {
  insertion?.stop();
  insertion = null;
  insertPath = null;
  insert = { ...INITIAL };
  emit("fade", 0);
  invalidate();
}

defineExpose({ startInsert, cancelInsert, resetInsert, reloadLabel });

// --- Cycle de vie

watch(
  () => [props.games, props.index, props.dragShift],
  () => sync(),
);
watch(() => props.looks, refreshLooks, { deep: true });
watch(() => [props.tilt, props.stick, props.tint], () => invalidate());
watch(currentTheme, () => renderer && themeLight());
watch(
  () => audio.running.length,
  () => invalidate(),
);

let resize: ResizeObserver | null = null;
const wake = () => invalidate();

onMounted(() => {
  init();
  if (!renderer) return;
  sync();
  resize = new ResizeObserver(() => invalidate());
  resize.observe(canvas.value!);
  window.addEventListener("focus", wake);
  document.addEventListener("visibilitychange", wake);
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  clearTimeout(timer);
  insertion?.stop();
  resize?.disconnect();
  window.removeEventListener("focus", wake);
  document.removeEventListener("visibilitychange", wake);
  for (const item of items.values()) disposeItem(item);
  items.clear();
  if (slot) {
    slot.group.traverse((o) => o instanceof Mesh && o.geometry.dispose());
    slot.body.dispose();
    slot.hole.dispose();
    slot = null;
  }
  disposeGeometries();
  disposeTemplates();
  for (const t of recolors.values()) t.dispose();
  recolors.clear();
  grain?.dispose();
  loading?.tex.dispose();
  loading?.mat.dispose();
  loading = null;
  grain = null;
  envTexture?.dispose();
  renderer?.dispose();
  renderer = null;
});
</script>

<template>
  <canvas ref="canvas" class="cartridge-scene" aria-hidden="true" />
</template>

<style scoped>
.cartridge-scene {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}
</style>
