import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { Detection } from "../types";

/**
 * Son du lanceur : musique de l'écran titre du jeu sélectionné (extraite de la
 * ROM par le backend) et petits bruitages d'interface, synthétisés.
 */

const PREFS_KEY = "kaleido.launcher.audio";

function readPrefs() {
  try {
    return { music: true, sfx: true, volume: 0.6, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
  } catch {
    return { music: true, sfx: true, volume: 0.6 };
  }
}

export const audio = reactive({
  ...(readPrefs() as { music: boolean; sfx: boolean; volume: number }),
  /** Jeu dont la musique joue. */
  playing: null as string | null,
  loading: null as string | null,
});

watch(
  () => [audio.music, audio.sfx, audio.volume],
  () => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({ music: audio.music, sfx: audio.sfx, volume: audio.volume }));
    } catch {
      /* stockage indisponible */
    }
    if (master) master.gain.setTargetAtTime(audio.music ? audio.volume : 0, ctx!.currentTime, 0.15);
  },
);

let ctx: AudioContext | null = null;
let master: GainNode | null = null;

function context() {
  if (!ctx) {
    ctx = new AudioContext();
    master = ctx.createGain();
    master.gain.value = audio.music ? audio.volume : 0;
    master.connect(ctx.destination);
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => undefined);
  return ctx;
}

// --- Musique

/** Morceaux décodés (null : pas de musique pour ce jeu). */
const buffers = new Map<string, Promise<AudioBuffer | null>>();
const MAX_BUFFERS = 8;

function load(d: Detection): Promise<AudioBuffer | null> {
  let p = buffers.get(d.path);
  if (!p) {
    p = invoke<ArrayBuffer>("music_title_theme", { path: d.path, game: d.game?.id ?? "" })
      .then((bytes) => context().decodeAudioData(bytes))
      .catch(() => null);
    buffers.set(d.path, p);
    if (buffers.size > MAX_BUFFERS) buffers.delete(buffers.keys().next().value!);
  }
  return p;
}

let current: { path: string; source: AudioBufferSourceNode; gain: GainNode } | null = null;
let request = 0;
let delay: number | undefined;

function stopCurrent(fade = 0.5) {
  if (!current || !ctx) return;
  const { source, gain } = current;
  gain.gain.cancelScheduledValues(ctx.currentTime);
  gain.gain.setValueAtTime(gain.gain.value, ctx.currentTime);
  gain.gain.linearRampToValueAtTime(0, ctx.currentTime + fade);
  source.stop(ctx.currentTime + fade + 0.05);
  current = null;
  audio.playing = null;
}

/** Joue la musique de `d` après un court délai (on ne lance rien en faisant défiler vite). */
export function previewMusic(d: Detection | null, wait = 650) {
  const id = ++request;
  clearTimeout(delay);
  if (current && current.path === d?.path) return;
  stopCurrent();
  audio.loading = null;
  if (!d?.game || !audio.music) return;
  delay = window.setTimeout(async () => {
    audio.loading = d.path;
    const buffer = await load(d);
    if (id !== request) return;
    audio.loading = null;
    if (!buffer) return;
    const c = context();
    const source = c.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    const gain = c.createGain();
    gain.gain.setValueAtTime(0, c.currentTime);
    gain.gain.linearRampToValueAtTime(1, c.currentTime + 1.2);
    source.connect(gain).connect(master!);
    source.start();
    current = { path: d.path, source, gain };
    audio.playing = d.path;
  }, wait);
}

export function stopMusic() {
  request++;
  clearTimeout(delay);
  audio.loading = null;
  stopCurrent(0.4);
}

/** Précharge la musique (rendu côté Rust) sans la jouer. */
export const prefetchMusic = (d: Detection | undefined) => {
  if (d?.game && audio.music) void load(d);
};

// --- Bruitages

type Sfx = "move" | "select" | "back" | "edge";

const TONES: Record<Sfx, { f: number[]; d: number; type: OscillatorType; v: number }> = {
  move: { f: [1320], d: 0.045, type: "triangle", v: 0.05 },
  edge: { f: [330], d: 0.06, type: "square", v: 0.025 },
  select: { f: [880, 1320, 1760], d: 0.07, type: "triangle", v: 0.07 },
  back: { f: [990, 660], d: 0.06, type: "triangle", v: 0.05 },
};

export function sfx(kind: Sfx) {
  if (!audio.sfx) return;
  const c = context();
  const t = TONES[kind];
  t.f.forEach((f, i) => {
    const start = c.currentTime + i * t.d * 0.8;
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = t.type;
    osc.frequency.value = f;
    gain.gain.setValueAtTime(t.v, start);
    gain.gain.exponentialRampToValueAtTime(0.0001, start + t.d);
    osc.connect(gain).connect(c.destination);
    osc.start(start);
    osc.stop(start + t.d + 0.02);
  });
}
