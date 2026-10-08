<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { titleIdOf } from "../games";
import { spritePrefs, styleInfo, type SpriteStyle } from "../spriteStyle";
import type { Detection } from "../types";

/**
 * Scène de l'écran titre, à droite du lanceur :
 * - la vraie vidéo de l'écran titre quand le jeu en contient une (Soleil / Lune,
 *   Ultra-Soleil / Ultra-Lune), extraite de la ROM ;
 * - sinon le Pokémon de la boîte, animé, dans le style choisi pour les Pokémon (pixels
 *   animés par défaut ; 3D animés si le Pokémon n'existe pas dans ce style).
 */
const props = defineProps<{ game: Detection | null }>();

/** Pokémon de la boîte : n° national et forme (ordre des formes de Showdown). */
const BOX: Record<string, [number, number]> = {
  diamond: [483, 0],
  pearl: [484, 0],
  platinum: [487, 1],
  heart_gold: [250, 0],
  soul_silver: [249, 0],
  black: [643, 0],
  white: [644, 0],
  black2: [646, 2],
  white2: [646, 1],
  x: [716, 0],
  y: [717, 0],
  omega_ruby: [383, 1],
  alpha_sapphire: [382, 1],
  sun: [791, 0],
  moon: [792, 0],
  ultra_sun: [800, 1],
  ultra_moon: [800, 2],
};

/** Jeux Pokémon Switch, par title ID. */
const BOX_SWITCH: Record<string, [number, number]> = {
  "010003F003A34000": [25, 0],
  "0100187003A36000": [133, 0],
  "0100ABF008968000": [888, 1],
  "01008DB008C2C000": [889, 1],
  "0100000011D90000": [483, 0],
  "010018E011D92000": [484, 0],
  "01001F5010DFA000": [493, 0],
  "0100A3D008C5C000": [1007, 0],
  "01008F6008C5E000": [1008, 0],
};

/** Jeux dont l'écran titre est une vidéo rangée dans la ROM. */
const VIDEO_GAMES = ["sun", "moon", "ultra_sun", "ultra_moon"];

const box = computed(() => {
  const g = props.game;
  if (!g) return null;
  if (g.platform === "switch") return BOX_SWITCH[titleIdOf(g) ?? ""] ?? null;
  return g.game ? (BOX[g.game.id] ?? null) : null;
});

/** Styles à essayer dans l'ordre : celui de l'utilisateur (sauf les icônes), puis les autres animés. */
const styles = computed(() => {
  const chosen: SpriteStyle = spritePrefs.style === "icons" || spritePrefs.style === "home" ? "gen5ani" : spritePrefs.style;
  const other: SpriteStyle = chosen === "gen5ani" ? "ani" : "gen5ani";
  return [chosen, other].map(styleInfo);
});
const attempt = ref(0);
const current = computed(() => styles.value[attempt.value] ?? null);
const spriteSrc = computed(() => {
  const b = box.value;
  const s = current.value;
  if (!b || !s?.dir) return null;
  const dir = spritePrefs.reduceMotion && s.still ? s.still : s.dir;
  const ext = spritePrefs.reduceMotion && s.still ? "png" : s.ext;
  return convertFileSrc(`${dir}/${b[0]}${b[1] ? `-${b[1]}` : ""}.${ext}`, "sprite");
});

// --- Vidéo de l'écran titre

const videoSrc = ref<string | null>(null);
/** La vidéo existe mais ffmpeg n'est pas installé : on propose de le télécharger. */
const needsFfmpeg = ref(false);
const converting = ref(false);
const installing = ref<{ step: string; done: number; total: number } | null>(null);
const videos = new Map<string, Promise<string | "missing" | null>>();

function videoFor(g: Detection): Promise<string | "missing" | null> {
  let p = videos.get(g.path);
  if (!p) {
    p = invoke<ArrayBuffer>("title_video", { path: g.path, game: g.game?.id ?? "" })
      .then((bytes) => URL.createObjectURL(new Blob([bytes], { type: "video/webm" })))
      .catch((e) => (String(e).includes("FFMPEG_MISSING") ? "missing" : null));
    videos.set(g.path, p);
  }
  return p;
}

const mb = (n: number) => `${Math.round(n / 1048576)} Mo`;

listen<{ step: string; done: number; total: number }>("tool-install", (e) => {
  if (installing.value) installing.value = e.payload;
}).catch(() => undefined);

async function installFfmpeg() {
  try {
    const info = await invoke<{ file: string; size: number }>("ffmpeg_download_info");
    const ok = await ask(
      `Kaleido peut afficher la vraie vidéo de l'écran titre de Soleil, Lune, Ultra-Soleil et Ultra-Lune, lue dans ta ROM. Il faut pour cela ffmpeg, le seul outil qui lit ce format vidéo de Nintendo : ${mb(info.size)} à télécharger une seule fois depuis sa page officielle.`,
      { title: "Vidéo de l'écran titre", okLabel: "Télécharger", cancelLabel: "Plus tard" },
    );
    if (!ok) return;
    installing.value = { step: "download", done: 0, total: info.size };
    await invoke("ffmpeg_install");
    videos.clear();
    needsFfmpeg.value = false;
    await refreshVideo();
  } catch (e) {
    await message(String(e), { title: "Installation impossible", kind: "error" });
  } finally {
    installing.value = null;
  }
}

async function refreshVideo() {
  const g = props.game;
  videoSrc.value = null;
  needsFfmpeg.value = false;
  if (!g || !g.game || !VIDEO_GAMES.includes(g.game.id) || g.kind === "ctr_dump") return;
  converting.value = !videos.has(g.path);
  const src = await videoFor(g);
  converting.value = false;
  if (props.game?.path !== g.path) return;
  if (src === "missing") needsFfmpeg.value = true;
  else videoSrc.value = src;
}
watch(() => props.game?.path, refreshVideo, { immediate: true });

// --- Taille du sprite : agrandi, mais pas au point de devenir flou.

const natural = ref<{ w: number; h: number } | null>(null);
const failed = ref(false);
watch([box, styles], () => {
  attempt.value = 0;
  failed.value = false;
});
watch(spriteSrc, () => (natural.value = null));
function onError() {
  if (attempt.value + 1 < styles.value.length) attempt.value++;
  else failed.value = true;
}
function onLoad(e: Event) {
  const img = e.target as HTMLImageElement;
  natural.value = { w: img.naturalWidth, h: img.naturalHeight };
}
/** Place disponible (la scène s'adapte à la taille de la fenêtre). */
const root = ref<HTMLElement>();
const room = ref({ w: 0, h: 0 });
let observer: ResizeObserver | null = null;
onMounted(() => {
  observer = new ResizeObserver(([e]) => (room.value = { w: e.contentRect.width, h: e.contentRect.height }));
  if (root.value) observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());

const spriteStyle = computed(() => {
  const n = natural.value;
  if (!n || !room.value.h) return { opacity: 0 };
  const pixel = current.value?.pixel ?? false;
  // Le plus grand agrandissement qui tient dans 85 % de la place ; entier pour le pixel
  // art (pixels nets), plafonné pour les modèles 3D qui deviennent flous.
  const fit = Math.min((room.value.h * 0.85) / n.h, (room.value.w * 0.9) / n.w);
  const scale = pixel ? Math.max(1, Math.min(10, Math.floor(fit))) : Math.max(1, Math.min(3.5, fit));
  return { width: `${n.w * scale}px`, height: `${n.h * scale}px`, imageRendering: pixel ? "pixelated" : "auto" } as Record<string, string>;
});
</script>

<template>
  <div ref="root" class="title-scene">
    <Transition name="scene" mode="out-in">
      <video v-if="videoSrc" :key="'v' + videoSrc" class="video" :src="videoSrc" autoplay loop muted playsinline />
      <div v-else-if="spriteSrc && !failed" :key="spriteSrc" class="legend">
        <img :src="spriteSrc" alt="" :style="spriteStyle" @load="onLoad" @error="onError" />
        <span class="floor" />
      </div>
    </Transition>
    <button v-if="needsFfmpeg || installing" class="offer" :disabled="!!installing" @click="installFfmpeg">
      {{ installing ? (installing.step === "extract" ? "Installation de ffmpeg…" : `Téléchargement de ffmpeg… ${mb(installing.done)} / ${mb(installing.total)}`) : "Afficher la vraie vidéo de l'écran titre" }}
    </button>
    <p v-else-if="converting" class="offer quiet">Préparation de la vidéo de l'écran titre…</p>
  </div>
</template>

<style scoped>
.title-scene {
  position: absolute;
  top: calc(84px * var(--ui, 1));
  right: 4%;
  bottom: calc(440px * var(--stage, 1));
  left: 46%;
  z-index: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  pointer-events: none;
}

.legend {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-end;
  height: 100%;
}

.legend img {
  max-width: 100%;
  object-fit: contain;
  filter: drop-shadow(0 18px 24px rgb(0 0 0 / 0.45));
  transition: opacity 0.3s ease;
}

/* Ombre au sol, sous le Pokémon. */
.floor {
  width: 46%;
  height: 18px;
  margin-top: -10px;
  border-radius: 50%;
  background: radial-gradient(closest-side, rgb(0 0 0 / 0.5), transparent);
}

.offer {
  position: absolute;
  right: 0;
  bottom: -6px;
  padding: 7px 14px;
  border: 1px solid rgb(255 255 255 / 0.25);
  border-radius: 999px;
  background: rgb(0 0 0 / 0.35);
  color: #fff;
  font: inherit;
  font-size: 13px;
  pointer-events: auto;
  cursor: pointer;
  backdrop-filter: blur(8px);
}

.offer:hover:not(:disabled) {
  background: #fff;
  color: #111;
}

.offer.quiet {
  margin: 0;
  border-color: transparent;
  color: rgb(255 255 255 / 0.7);
  cursor: default;
}

/* La boîte épouse l'image (5:3, écran du haut de la 3DS) : ni bandes ni cadre vide. */
.video {
  align-self: center;
  width: auto;
  height: auto;
  max-width: 100%;
  max-height: 100%;
  aspect-ratio: 5 / 3;
  object-fit: cover;
  border-radius: 14px;
  box-shadow: 0 24px 60px rgb(0 0 0 / 0.5);
}

.scene-enter-active,
.scene-leave-active {
  transition:
    opacity 0.35s ease,
    transform 0.35s ease;
}

.scene-enter-from {
  opacity: 0;
  transform: translateY(12px);
}

.scene-leave-to {
  opacity: 0;
}

@media (max-height: 760px) {
  .title-scene {
    display: none;
  }
}
</style>
