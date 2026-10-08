import { invoke } from "@tauri-apps/api/core";
import { CanvasTexture, SRGBColorSpace } from "three";
import { hash32, type SupportModel, type Wear } from "./models";

/**
 * Étiquette d'une cartouche, préparée dans un canvas hors écran : image choisie par
 * l'utilisateur, sinon photo de la vraie carte (zone de l'étiquette), sinon étiquette
 * neutre avec le titre du jeu.
 *
 * Cache mémoire par clé, et cache disque en PNG (`label_cache`, labels.rs) pour les
 * étiquettes tirées des photos.
 */

/** À changer quand la composition change : les étiquettes en cache sont alors refaites. */
const LABEL_VERSION = 8;

export interface LabelInput {
  /** Clé du jeu (`cartridgeKey`). */
  key: string;
  title: string;
  support: SupportModel;
  wear: Wear;
  /** Seed d'une ROM randomisée par Kaleido. */
  seed: number | null;
  randomized: boolean;
  /** Image fournie par l'utilisateur (Inspecter) : elle prime sur tout le reste. */
  custom: boolean;
  /** Photo de la vraie carte (`cover://`) et position de l'étiquette dans la photo : elle prime sur l'étiquette neutre. */
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
function drawSticker(g: CanvasRenderingContext2D, w: number, h: number, input: LabelInput, font: string, top = 0.56) {
  const label = input.seed !== null ? `KALEIDO · ${String(input.seed).slice(-6)}` : "KALEIDO";
  const size = Math.round(Math.min(w, h) * 0.055);
  g.font = `800 ${size}px ${font}`;
  const tw = g.measureText(label).width + size * 1.4;
  const th = size * 1.9;
  const x = w - tw - w * 0.05;
  const y = h * top;
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

/** Coupe un titre en deux lignes au plus, la coupure la plus équilibrée. */
function splitTitle(g: CanvasRenderingContext2D, title: string, maxWidth: number): string[] {
  if (g.measureText(title).width <= maxWidth) return [title];
  const words = title.split(" ");
  let best: string[] = [title];
  let bestWidth = Infinity;
  for (let i = 1; i < words.length; i++) {
    const lines = [words.slice(0, i).join(" "), words.slice(i).join(" ")];
    const width = Math.max(...lines.map((l) => g.measureText(l).width));
    if (width < bestWidth) {
      best = lines;
      bestWidth = width;
    }
  }
  return best;
}

/**
 * Étiquette neutre, sans photo de la vraie étiquette : papier uni et titre du
 * jeu imprimé au centre. Pas de fausse étiquette reconstituée depuis la jaquette.
 */
async function neutralLabel(input: LabelInput): Promise<HTMLCanvasElement> {
  const [w, h] = labelSize(input.support, input.size);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d", { willReadFrequently: true })!;
  const rand = random(`${input.key}:${input.wear}`);
  const font = cssVar("--font-display", "Segoe UI, sans-serif");
  await document.fonts?.ready;

  g.save();
  g.beginPath();
  g.roundRect(0, 0, w, h, w * 0.025);
  g.clip();
  g.fillStyle = "#efeee9";
  g.fillRect(0, 0, w, h);
  paperGrain(g, w, h, rand);
  g.restore();

  // Titre : la plus grande taille qui tient sur deux lignes dans la largeur de l'étiquette.
  const maxWidth = w * 0.84;
  let size = Math.round(Math.min(h * 0.13, w * 0.11));
  let lines: string[] = [];
  for (; size > 12; size--) {
    g.font = `700 ${size}px ${font}`;
    lines = splitTitle(g, input.title, maxWidth);
    if (lines.every((l) => g.measureText(l).width <= maxWidth)) break;
  }
  g.fillStyle = "#26272b";
  g.textAlign = "center";
  g.textBaseline = "middle";
  const lineHeight = size * 1.18;
  const y0 = h / 2 - ((lines.length - 1) * lineHeight) / 2;
  lines.forEach((line, i) => g.fillText(line, w / 2, y0 + i * lineHeight, maxWidth));
  g.textAlign = "start";

  if (input.randomized) drawSticker(g, w, h, input, font, 0.74);
  if (input.wear === "jouee") wearOut(g, w, h, rand);
  return canvas;
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
  const t = performance.now();
  const canvas = await neutralLabel(input);
  lastComposeMs = performance.now() - t;
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
