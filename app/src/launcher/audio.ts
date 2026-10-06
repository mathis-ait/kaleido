import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Detection } from "../types";
import { titleIdOf } from "../games";

/** Jeux dont on sait lire la musique : Pokémon DS et 3DS, et jeux Switch (selon le jeu). */
const hasMusic = (d: Detection | null | undefined): d is Detection => !!d && (!!d.game || d.platform === "switch");

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
  /** Émulateurs ouverts : pas de musique pendant qu'on joue. */
  running: [] as string[],
});

// --- Silence pendant le jeu et quand Kaleido n'est pas au premier plan

/** Dernier jeu dont on voulait la musique, pour la reprendre au retour. */
let wanted: Detection | null = null;
const silenced = () => audio.running.length > 0 || !document.hasFocus();

invoke<string[]>("emulators_running")
  .then((r) => (audio.running = r))
  .catch(() => undefined);
listen<string[]>("emulators-running", (e) => {
  audio.running = e.payload;
  if (e.payload.length) stopMusic(false);
  else if (wanted) previewMusic(wanted, 800);
}).catch(() => undefined);

window.addEventListener("blur", () => stopMusic(false));
window.addEventListener("focus", () => {
  if (wanted && !audio.running.length) previewMusic(wanted, 500);
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

/**
 * Volume homogène d'un jeu à l'autre : certains morceaux sont mixés bas (Wwise applique
 * son volume pendant le jeu). Gain vers un niveau moyen commun, sans jamais saturer.
 */
function normalize(buffer: AudioBuffer): AudioBuffer {
  const TARGET_RMS = 0.1;
  let sum = 0;
  let peak = 0;
  let count = 0;
  for (let c = 0; c < buffer.numberOfChannels; c++) {
    const data = buffer.getChannelData(c);
    for (let i = 0; i < data.length; i += 7) {
      sum += data[i] * data[i];
      peak = Math.max(peak, Math.abs(data[i]));
      count++;
    }
  }
  const rms = Math.sqrt(sum / Math.max(1, count));
  if (rms < 1e-4) return buffer;
  const gain = Math.min(TARGET_RMS / rms, 0.95 / Math.max(peak, 1e-4), 6);
  if (gain < 1.15 && gain > 0.87) return buffer;
  for (let c = 0; c < buffer.numberOfChannels; c++) {
    const data = buffer.getChannelData(c);
    for (let i = 0; i < data.length; i++) data[i] *= gain;
  }
  return buffer;
}

function load(d: Detection): Promise<AudioBuffer | null> {
  let p = buffers.get(d.path);
  if (!p) {
    const request =
      d.platform === "switch"
        ? invoke<ArrayBuffer>("music_switch_theme", { path: d.path, titleId: titleIdOf(d) ?? "" })
        : invoke<ArrayBuffer>("music_title_theme", { path: d.path, game: d.game?.id ?? "" });
    p = request
      .then((bytes) => context().decodeAudioData(bytes))
      .then(normalize)
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
  wanted = d;
  const id = ++request;
  clearTimeout(delay);
  if (current && current.path === d?.path) return;
  stopCurrent();
  audio.loading = null;
  if (!hasMusic(d) || !audio.music || silenced()) return;
  delay = window.setTimeout(async () => {
    audio.loading = d.path;
    const buffer = await load(d);
    if (id !== request) return;
    audio.loading = null;
    if (!buffer || silenced()) return;
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

/** Coupe la musique ; `forget` : ne pas la reprendre au retour dans Kaleido. */
export function stopMusic(forget = true) {
  if (forget) wanted = null;
  request++;
  clearTimeout(delay);
  audio.loading = null;
  stopCurrent(0.4);
}

/** Précharge la musique (rendu côté Rust) sans la jouer. */
export const prefetchMusic = (d: Detection | undefined) => {
  if (hasMusic(d) && audio.music) void load(d);
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
