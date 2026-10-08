import { invoke } from "@tauri-apps/api/core";
import { CanvasTexture, SRGBColorSpace } from "three";
import { dominantColor } from "../color";
import { hash32, type SupportModel, type Wear } from "./models";

/**
 * Étiquette d'une cartouche, composée dans un canvas hors écran : fond de la couleur
 * de la jaquette avec un grain papier, moitié haute de la jaquette (logo et Pokémon
 * de boîte), bandeau avec le nom, le code jeu et la plateforme. Aucun logo de marque.
 *
 * Cache mémoire par clé, et cache disque en PNG (`label_cache`, labels.rs) pour ne
 * composer chaque étiquette qu'une fois.
 */

/** À changer quand la composition change : les étiquettes en cache sont alors refaites. */
const LABEL_VERSION = 8;

export interface LabelInput {
  /** Clé du jeu (`cartridgeKey`). */
  key: string;
  title: string;
  code: string | null;
  platform: string;
  cover: string | null;
  support: SupportModel;
  /** Couleur de la coque, pour un jeu sans jaquette. */
  shellColor: string;
  wear: Wear;
  /** Seed d'une ROM randomisée par Kaleido. */
  seed: number | null;
  randomized: boolean;
  /** Image fournie par l'utilisateur (Inspecter) : elle prime sur tout le reste. */
  custom: boolean;
  /** Photo de la vraie carte (`cover://`) et position de l'étiquette dans la photo : elle prime sur la composition. */
  photo: string | null;
  photoCrop: { x: number; y: number; w: number; h: number } | null;
  /** Taille de l'étiquette en mm, quand le modèle 3D la donne (sinon la zone du support). */
  size?: [number, number];
}

export const LABEL_WIDTH = 512;

export function labelSize(support: SupportModel, size?: [number, number]): [number, number] {
  const [w, h] = size ?? [support.labelZone.w, support.labelZone.h];
  return [LABEL_WIDTH, Math.round((LABEL_WIDTH * h) / w)];
}

// --- Cache disque

async function readCache(key: string): Promise<Blob | null> {
  try {
    const bytes = await invoke<ArrayBuffer>("label_cache", new Uint8Array(0), { headers: { "x-label-key": key } });
    return bytes && bytes.byteLength ? new Blob([bytes]) : null;
  } catch {
    return null;
  }
}

function writeCache(key: string, canvas: HTMLCanvasElement) {
  canvas.toBlob(async (blob) => {
    if (!blob) return;
    const bytes = new Uint8Array(await blob.arrayBuffer());
    invoke("label_cache", bytes, { headers: { "x-label-key": key } }).catch(() => undefined);
  }, "image/png");
}

// --- Composition

function loadImage(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

/** Générateur pseudo-aléatoire déterministe (mulberry32) : même jeu, même grain. */
function random(seed: string) {
  let a = parseInt(hash32(seed), 16);
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function cssVar(name: string, fallback: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** Couleur CSS → [r, g, b] (0-255), via le canvas. */
function rgb(color: string): [number, number, number] {
  const c = document.createElement("canvas").getContext("2d")!;
  c.fillStyle = color;
  c.fillRect(0, 0, 1, 1);
  const [r, g, b] = c.getImageData(0, 0, 1, 1).data;
  return [r, g, b];
}

function shade([r, g, b]: [number, number, number], k: number) {
  return `rgb(${Math.round(r * k)} ${Math.round(g * k)} ${Math.round(b * k)})`;
}

function paperGrain(g: CanvasRenderingContext2D, w: number, h: number, rand: () => number) {
  const data = g.getImageData(0, 0, w, h);
  const px = data.data;
  for (let i = 0; i < px.length; i += 4) {
    const n = (rand() - 0.5) * 14;
    px[i] += n;
    px[i + 1] += n;
    px[i + 2] += n;
  }
  g.putImageData(data, 0, 0);
}

function fitText(g: CanvasRenderingContext2D, text: string, maxWidth: number, size: number, weight: number, family: string) {
  let s = size;
  g.font = `${weight} ${s}px ${family}`;
  while (s > 10 && g.measureText(text).width > maxWidth) {
    s -= 1;
    g.font = `${weight} ${s}px ${family}`;
  }
  return s;
}

/** Usure « jouée » : coins éclaircis, rayures fines, couleurs un peu passées. */
function wearOut(g: CanvasRenderingContext2D, w: number, h: number, rand: () => number) {
  g.save();
  g.globalCompositeOperation = "saturation";
  g.fillStyle = "rgba(128,128,128,0.22)";
  g.fillRect(0, 0, w, h);
  g.globalCompositeOperation = "source-over";
  for (const [x, y] of [
    [0, 0],
    [w, 0],
    [0, h],
    [w, h],
  ]) {
    const r = g.createRadialGradient(x, y, 0, x, y, w * 0.16);
    r.addColorStop(0, "rgba(255,255,255,0.35)");
    r.addColorStop(1, "rgba(255,255,255,0)");
    g.fillStyle = r;
    g.fillRect(0, 0, w, h);
  }
  g.strokeStyle = "rgba(255,255,255,0.18)";
  g.lineWidth = 1;
  for (let i = 0; i < 26; i++) {
    const x = rand() * w;
    const y = rand() * h;
    const len = 8 + rand() * 40;
    const a = rand() * Math.PI;
    g.beginPath();
    g.moveTo(x, y);
    g.lineTo(x + Math.cos(a) * len, y + Math.sin(a) * len);
    g.stroke();
  }
  g.restore();
}

/** Autocollant KALEIDO et seed courte, posé de travers sur l'étiquette d'une ROM randomisée. */
function drawSticker(g: CanvasRenderingContext2D, w: number, h: number, input: LabelInput, font: string) {
  const label = input.seed !== null ? `KALEIDO · ${String(input.seed).slice(-6)}` : "KALEIDO";
  const size = Math.round(Math.min(w, h) * 0.055);
  g.font = `800 ${size}px ${font}`;
  const tw = g.measureText(label).width + size * 1.4;
  const th = size * 1.9;
  const x = w - tw - w * 0.05;
  const y = h * 0.56;
  g.save();
  g.translate(x + tw / 2, y + th / 2);
  g.rotate(-0.06);
  g.shadowColor = "rgba(0,0,0,0.35)";
  g.shadowBlur = 4;
  g.fillStyle = "#fbfaf6";
  g.beginPath();
  g.roundRect(-tw / 2, -th / 2, tw, th, 4);
  g.fill();
  g.shadowColor = "transparent";
  g.fillStyle = "#16171c";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(label, 0, 1);
  g.restore();
}

/** Illustration de la jaquette (sans bandeau de plateforme), recadrée pour remplir la zone. */
function drawArt(g: CanvasRenderingContext2D, cover: HTMLImageElement, crop: SupportModel["coverCrop"], x: number, y: number, w: number, h: number) {
  const sx = cover.naturalWidth * crop.x;
  const sy = cover.naturalHeight * crop.y;
  const sw = cover.naturalWidth * crop.w;
  const sh = cover.naturalHeight * crop.h;
  const scale = Math.max(w / sw, h / sh);
  const cw = w / scale;
  const ch = h / scale;
  g.drawImage(cover, sx + (sw - cw) / 2, sy, cw, ch, x, y, w, h);
}

/** « NINTENDO DS » / « NINTENDO 3DS » : texte imprimé du bandeau du haut des vraies étiquettes. */
function drawConsoleMark(g: CanvasRenderingContext2D, cx: number, cy: number, size: number, model: "DS" | "3DS", font: string) {
  g.textBaseline = "middle";
  g.font = `500 ${size * 0.62}px ${font}`;
  const left = "NINTENDO";
  g.save();
  // Espacement des lettres du mot « NINTENDO », comme sur les cartes.
  const spacing = size * 0.06;
  const leftWidth = [...left].reduce((s, ch) => s + g.measureText(ch).width + spacing, 0);
  g.font = `900 ${size}px ${font}`;
  const rightWidth = g.measureText(model).width;
  const total = leftWidth + size * 0.18 + rightWidth;
  let x = cx - total / 2;
  g.font = `500 ${size * 0.62}px ${font}`;
  g.fillStyle = "#2a2a2e";
  for (const ch of left) {
    g.fillText(ch, x, cy + size * 0.08);
    x += g.measureText(ch).width + spacing;
  }
  x += size * 0.18;
  g.font = `900 ${size}px ${font}`;
  if (model === "3DS") {
    g.fillStyle = "#d7141a";
    g.fillText("3", x, cy);
    x += g.measureText("3").width;
    g.fillStyle = "#2a2a2e";
    g.fillText("DS", x, cy);
  } else {
    g.fillStyle = "#2a2a2e";
    g.fillText(model, x, cy);
  }
  g.restore();
}

/**
 * Étiquette composée quand il n'y a pas de photo de la vraie carte : même gabarit que
 * les étiquettes officielles du support (bandeau du haut, illustration, code en bas),
 * avec l'illustration de la jaquette française.
 */
export async function composeLabel(input: LabelInput): Promise<{ canvas: HTMLCanvasElement; complete: boolean }> {
  const [w, h] = labelSize(input.support, input.size);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d", { willReadFrequently: true })!;
  const rand = random(`${input.key}:${input.wear}`);
  const font = cssVar("--font-display", "Segoe UI, sans-serif");
  const mono = "Consolas, 'Cascadia Mono', 'Courier New', monospace";
  await document.fonts?.ready;

  const cover = input.cover ? await loadImage(input.cover) : null;
  const tint = cover && input.cover ? await dominantColor(input.cover) : null;
  const base = rgb(tint ?? input.shellColor);
  const support = input.support.id;
  const paper = "#f4f3ef";

  g.save();
  g.beginPath();
  g.roundRect(0, 0, w, h, w * 0.025);
  g.clip();
  g.fillStyle = paper;
  g.fillRect(0, 0, w, h);

  // Zones du gabarit : bandeau du haut, illustration, bandeau du bas (code).
  const top = support === "ds" || support === "3ds" ? h * 0.14 : support === "switch" ? h * 0.12 : 0;
  const bottom = support === "ds" || support === "3ds" ? h * 0.12 : support === "switch" ? h * 0.1 : h * 0.1;
  const art = { x: 0, y: top, w, h: h - top - bottom };

  if (cover) drawArt(g, cover, input.support.coverCrop, art.x, art.y, art.w, art.h);
  else {
    g.fillStyle = shade(base, 0.8);
    g.fillRect(art.x, art.y, art.w, art.h);
    g.fillStyle = "rgba(255,255,255,0.94)";
    g.textBaseline = "middle";
    const size = fitText(g, input.title, w * 0.86, Math.round(h * 0.12), 800, font);
    g.font = `800 ${size}px ${font}`;
    g.fillText(input.title, w * 0.07, art.y + art.h / 2);
  }

  // Bandeau du haut.
  if (support === "ds" || support === "3ds") {
    g.fillStyle = paper;
    g.fillRect(0, 0, w, top);
    drawConsoleMark(g, w / 2, top * 0.54, top * 0.52, support === "ds" ? "DS" : "3DS", font);
  } else if (support === "switch") {
    g.fillStyle = "#e4000f";
    g.fillRect(0, 0, w, top);
    g.fillStyle = "#fff";
    g.textBaseline = "middle";
    g.textAlign = "center";
    g.font = `800 ${Math.round(top * 0.42)}px ${font}`;
    g.fillText("NINTENDO SWITCH", w / 2, top / 2 + 1);
    g.textAlign = "start";
  }

  // Bandeau du bas : code imprimé à droite, comme sur les cartes (titre pour la Switch, sans code connu).
  g.fillStyle = paper;
  g.fillRect(0, h - bottom, w, bottom);
  g.textBaseline = "middle";
  const mid = h - bottom / 2;
  const code = input.code ?? input.title;
  g.fillStyle = "#1f1f22";
  if (support === "ds" || support === "3ds") {
    g.font = `500 ${Math.round(bottom * 0.5)}px ${mono}`;
    g.textAlign = "right";
    g.fillText(code.split("").join(String.fromCharCode(8202)), w * 0.94, mid, w * 0.88);
  } else {
    g.font = `700 ${Math.round(bottom * 0.46)}px ${input.code ? mono : font}`;
    g.textAlign = "left";
    g.fillText(code, w * 0.05, mid, w * 0.9);
  }
  g.textAlign = "start";

  // Grain du papier et léger vernis.
  paperGrain(g, w, h, rand);
  const gloss = g.createLinearGradient(0, 0, w * 0.4, h);
  gloss.addColorStop(0, "rgba(255,255,255,0.06)");
  gloss.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = gloss;
  g.fillRect(0, 0, w, h);
  g.restore();

  if (input.randomized) drawSticker(g, w, h, input, font);
  if (input.wear === "jouee") wearOut(g, w, h, rand);
  return { canvas, complete: !input.cover || !!cover };
}

// --- Texture

const memory = new Map<string, Promise<HTMLCanvasElement>>();
const MAX_MEMORY = 48;

/** Durée de la dernière composition (ms), pour les mesures de performance. */
export let lastComposeMs = 0;

/** PNG du cache → canvas (un ImageBitmap ne serait pas retourné par WebGL : étiquette à l'envers). */
async function toCanvas(blob: Blob): Promise<HTMLCanvasElement | null> {
  const bitmap = await createImageBitmap(blob).catch(() => null);
  if (!bitmap) return null;
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
  bitmap.close();
  return canvas;
}

/**
 * Étiquette tirée de la photo de la vraie carte : la zone de l'étiquette, coins arrondis,
 * plus un petit autocollant KALEIDO sur une ROM randomisée. Null si la photo manque.
 */
async function photoLabel(input: LabelInput): Promise<HTMLCanvasElement | null> {
  if (!input.photo || !input.photoCrop) return null;
  const img = await loadImage(input.photo);
  if (!img) return null;
  const [w, h] = labelSize(input.support, input.size);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d", { willReadFrequently: true })!;
  const c = input.photoCrop;
  g.save();
  g.beginPath();
  g.roundRect(0, 0, w, h, w * 0.025);
  g.clip();
  g.drawImage(img, img.naturalWidth * c.x, img.naturalHeight * c.y, img.naturalWidth * c.w, img.naturalHeight * c.h, 0, 0, w, h);
  g.restore();
  if (input.randomized) drawSticker(g, w, h, input, cssVar("--font-display", "Segoe UI, sans-serif"));
  if (input.wear === "jouee") wearOut(g, w, h, random(`${input.key}:${input.wear}`));
  return canvas;
}

async function source(input: LabelInput): Promise<HTMLCanvasElement> {
  if (input.custom) {
    const blob = await readCache(input.key);
    if (blob) {
      const bitmap = await toCanvas(blob);
      if (bitmap) return bitmap;
    }
  }
  const suffix = `${LABEL_VERSION}-${input.wear}${input.randomized ? "-k" : ""}${input.size ? "-r" : ""}`;
  if (input.photo) {
    const photoKey = `${input.key}-p${suffix}`;
    const cachedPhoto = await readCache(photoKey);
    const fromCache = cachedPhoto ? await toCanvas(cachedPhoto) : null;
    if (fromCache) return fromCache;
    const fromPhoto = await photoLabel(input);
    if (fromPhoto) {
      writeCache(photoKey, fromPhoto);
      return fromPhoto;
    }
  }
  const diskKey = `${input.key}-g${suffix}`;
  const cached = await readCache(diskKey);
  if (cached) {
    const bitmap = await toCanvas(cached);
    if (bitmap) return bitmap;
  }
  const t = performance.now();
  const { canvas, complete } = await composeLabel(input);
  lastComposeMs = performance.now() - t;
  if (complete) writeCache(diskKey, canvas);
  return canvas;
}

function memoryKey(input: LabelInput) {
  return `${input.key}|${input.wear}|${input.custom ? "c" : "g"}|${input.randomized ? "k" : ""}|${input.photo ? "p" : ""}|${input.size ? "r" : ""}`;
}

/** Oublie l'étiquette d'un jeu (après un changement d'image personnalisée). */
export function forgetLabel(key: string) {
  for (const k of [...memory.keys()]) if (k.startsWith(`${key}|`)) memory.delete(k);
}

export async function labelTexture(input: LabelInput): Promise<CanvasTexture> {
  const k = memoryKey(input);
  let p = memory.get(k);
  if (!p) {
    p = source(input);
    memory.set(k, p);
    if (memory.size > MAX_MEMORY) memory.delete(memory.keys().next().value!);
  }
  const image = await p;
  // Une texture par instance (libérée avec elle) ; l'image, elle, est partagée.
  const texture = new CanvasTexture(image);
  texture.colorSpace = SRGBColorSpace;
  texture.anisotropy = 4;
  return texture;
}
