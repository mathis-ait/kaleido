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
const LABEL_VERSION = 4;

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
  /** Image fournie par l'utilisateur (Inspecter) : elle prime sur la composition. */
  custom: boolean;
}

export const LABEL_WIDTH = 512;

export function labelSize(support: SupportModel): [number, number] {
  const { w, h } = support.labelZone;
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

/** Compose l'étiquette (déterministe pour une jaquette donnée). */
export async function composeLabel(input: LabelInput): Promise<{ canvas: HTMLCanvasElement; complete: boolean }> {
  const [w, h] = labelSize(input.support);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d", { willReadFrequently: true })!;
  const rand = random(`${input.key}:${input.wear}`);
  const font = cssVar("--font-display", "Segoe UI, sans-serif");
  await document.fonts?.ready;

  const cover = input.cover ? await loadImage(input.cover) : null;
  const tint = cover && input.cover ? await dominantColor(input.cover) : null;
  const base = rgb(tint ?? input.shellColor);

  // Comme une vraie étiquette : l'illustration à fond perdu (avec le logo du jeu, qui
  // porte déjà son nom), et un fin bandeau clair en bas avec le code imprimé.
  const strip = Math.round(Math.max(30, h * 0.11));
  const pad = Math.round(w * 0.04);

  // 1. Fond : couleur de la jaquette, sous l'illustration (visible si elle ne couvre pas tout).
  g.fillStyle = shade(base, cover ? 0.55 : 0.8);
  g.fillRect(0, 0, w, h);

  if (cover) {
    // 2. Illustration : moitié haute de la jaquette, sans bandeau de plateforme, à fond perdu.
    const area = { x: 0, y: 0, w, h: h - strip };
    const crop = input.support.coverCrop;
    const sx = cover.naturalWidth * crop.x;
    const sy = cover.naturalHeight * crop.y;
    const sw = cover.naturalWidth * crop.w;
    const sh = cover.naturalHeight * crop.h;
    const scale = Math.max(area.w / sw, area.h / sh);
    const cw = area.w / scale;
    const ch = area.h / scale;
    g.drawImage(cover, sx + (sw - cw) / 2, sy, cw, ch, area.x, area.y, area.w, area.h);
  } else {
    // 5. Sans jaquette : le nom en grand sur la couleur du support.
    g.fillStyle = "rgba(255,255,255,0.92)";
    g.textBaseline = "middle";
    const size = fitText(g, input.title, w - pad * 2, Math.round(h * 0.14), 800, font);
    g.font = `800 ${size}px ${font}`;
    g.fillText(input.title, pad, (h - strip) / 2);
  }

  // 3. Bandeau imprimé : papier blanc cassé, code jeu à gauche, plateforme à droite.
  g.fillStyle = "#f3f1ec";
  g.fillRect(0, h - strip, w, strip);
  g.fillStyle = "rgba(0,0,0,0.12)";
  g.fillRect(0, h - strip, w, 1);
  g.textBaseline = "middle";
  g.fillStyle = "#26272c";
  g.font = `700 ${Math.round(strip * 0.42)}px ${font}`;
  const mid = h - strip / 2;
  g.fillText(input.code ?? input.title, pad, mid);

  if (input.randomized) {
    // 4. ROM randomisée : repère KALEIDO et seed courte, imprimés dans le bandeau.
    const label = input.seed !== null ? `KALEIDO · ${String(input.seed).slice(-6)}` : "KALEIDO";
    g.font = `800 ${Math.round(strip * 0.38)}px ${font}`;
    const tw = g.measureText(label).width + strip * 0.6;
    const th = strip * 0.64;
    const tx = w - pad - tw;
    g.fillStyle = "#16171c";
    g.beginPath();
    g.roundRect(tx, mid - th / 2, tw, th, th / 2);
    g.fill();
    g.fillStyle = "#fff";
    g.textAlign = "center";
    g.fillText(label, tx + tw / 2, mid + 1);
    g.textAlign = "start";
  } else {
    g.fillStyle = "rgba(38,39,44,0.7)";
    g.font = `600 ${Math.round(strip * 0.36)}px ${font}`;
    g.textAlign = "right";
    g.fillText(input.platform, w - pad, mid);
    g.textAlign = "start";
  }

  // Grain du papier et léger vernis (plus clair en haut), sur toute l'étiquette.
  paperGrain(g, w, h, rand);
  const gloss = g.createLinearGradient(0, 0, w * 0.4, h);
  gloss.addColorStop(0, "rgba(255,255,255,0.06)");
  gloss.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = gloss;
  g.fillRect(0, 0, w, h);

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

async function source(input: LabelInput): Promise<HTMLCanvasElement> {
  if (input.custom) {
    const blob = await readCache(input.key);
    if (blob) {
      const bitmap = await toCanvas(blob);
      if (bitmap) return bitmap;
    }
  }
  const diskKey = `${input.key}-g${LABEL_VERSION}-${input.wear}${input.randomized ? "-k" : ""}`;
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
  return `${input.key}|${input.wear}|${input.custom ? "c" : "g"}|${input.randomized ? "k" : ""}`;
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
