<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Backdrop from "./Backdrop.vue";
import Icon from "../components/Icon.vue";
import { openRom } from "../editor";
import { hideGame, libraryUi } from "../games";
import { removeItem } from "../library";
import { RECOMMENDED, defaultEmulator, emus, formatMo, installs, loadEmulators } from "../play/play";
import type { Detection } from "../types";
import { isKaleidoRom } from "../types";
import { canRandomize, coverUrl, lastPlayed, launchGame, openSaveOf, platformOf, randomize, timeAgo } from "./actions";
import { audio, prefetchMusic, previewMusic, sfx, stopMusic } from "./audio";
import { dominantColor } from "./color";
import { useGamepad, type PadAction } from "./gamepad";

const props = defineProps<{ games: Detection[] }>();

// --- Onglets

type Tab = "all" | "nds" | "3ds";
const TABS: { id: Tab; label: string }[] = [
  { id: "all", label: "Tous" },
  { id: "nds", label: "Nintendo DS" },
  { id: "3ds", label: "Nintendo 3DS" },
];
const tab = ref<Tab>("all");
const list = computed(() => props.games.filter((g) => tab.value === "all" || g.platform === tab.value));

// --- Sélection

const index = ref(Math.max(0, list.value.findIndex((g) => g.path === libraryUi.selected)));
const game = computed<Detection | null>(() => list.value[index.value] ?? null);

watch(list, (l) => {
  const keep = l.findIndex((g) => g.path === libraryUi.selected);
  index.value = keep >= 0 ? keep : Math.min(index.value, Math.max(0, l.length - 1));
});

function move(delta: number) {
  const next = Math.min(Math.max(index.value + delta, 0), list.value.length - 1);
  if (next === index.value) {
    sfx("edge");
    return;
  }
  index.value = next;
  sfx("move");
}

function switchTab(delta: number) {
  const i = (TABS.findIndex((t) => t.id === tab.value) + delta + TABS.length) % TABS.length;
  tab.value = TABS[i].id;
  sfx("back");
}

// Couleur du jeu, musique et préchargement des voisins.
const color = ref("hsl(250 80% 66%)");
watch(
  game,
  async (g) => {
    libraryUi.selected = g?.path ?? null;
    previewMusic(g);
    prefetchMusic(list.value[index.value + 1]);
    prefetchMusic(list.value[index.value - 1]);
    const url = g ? coverUrl(g) : null;
    const c = url ? await dominantColor(url) : null;
    if (g === game.value && c) color.value = c;
  },
  { immediate: true },
);

watch(
  () => audio.music,
  (on) => (on ? previewMusic(game.value, 0) : stopMusic()),
);

// --- Carrousel

function itemStyle(i: number) {
  const k = i - index.value;
  const abs = Math.abs(k);
  const sign = Math.sign(k);
  const x = k === 0 ? 0 : sign * (250 + (abs - 1) * 168);
  const rot = k === 0 ? 0 : -sign * 44;
  const z = k === 0 ? 40 : -140 - abs * 26;
  const scale = k === 0 ? 1.16 : 0.9;
  return {
    transform: `translateX(${x}px) translateZ(${z}px) rotateY(${rot}deg) scale(${scale})`,
    zIndex: 100 - abs,
    opacity: abs > 6 ? 0 : 1 - Math.max(0, abs - 2) * 0.18,
    filter: k === 0 ? "none" : `brightness(${Math.max(0.45, 0.85 - abs * 0.08)})`,
  };
}

/** Seuls les jeux proches sont dessinés. */
const visible = computed(() => list.value.map((g, i) => ({ g, i })).filter(({ i }) => Math.abs(i - index.value) <= 7));

function clickCover(i: number) {
  if (i === index.value) play();
  else {
    index.value = i;
    sfx("move");
  }
}

let wheelAt = 0;
function onWheel(e: WheelEvent) {
  const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
  const now = performance.now();
  if (Math.abs(d) < 4 || now - wheelAt < 110) return;
  wheelAt = now;
  move(d > 0 ? 1 : -1);
}

// Inclinaison de la jaquette sélectionnée sous la souris.
const tilt = ref({ x: 0, y: 0 });
function onTilt(e: PointerEvent) {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  tilt.value = { x: ((e.clientY - r.top) / r.height - 0.5) * -14, y: ((e.clientX - r.left) / r.width - 0.5) * 18 };
}

// --- Actions

const emulator = computed(() => (game.value && emus.loaded ? defaultEmulator(platformOf(game.value)) : null));
const installing = computed(() => (game.value ? installs[RECOMMENDED[platformOf(game.value)]] ?? null : null));
const playLabel = computed(() => {
  if (installing.value) {
    const p = installing.value;
    return p.step === "download" && p.total ? `Téléchargement ${formatMo(p.done)} / ${formatMo(p.total)}` : "Installation…";
  }
  if (emus.loaded && !emulator.value && game.value) return platformOf(game.value) === "nds" ? "Installer melonDS et jouer" : "Installer Azahar et jouer";
  return "Jouer";
});

const launching = ref<{ cover: string | null; title: string; status: string } | null>(null);
const notice = ref<string | null>(null);

async function play() {
  const g = game.value;
  if (!g || launching.value || installing.value) return;
  sfx("select");
  stopMusic();
  launching.value = { cover: coverUrl(g), title: g.title, status: "Lancement…" };
  const emu = await launchGame(g);
  if (emu) {
    launching.value.status = `Bon jeu ! (${emu})`;
    setTimeout(() => (launching.value = null), 1600);
  } else {
    launching.value = null;
    previewMusic(game.value);
  }
}

async function openSave() {
  const g = game.value;
  if (!g) return;
  stopMusic();
  notice.value = await openSaveOf(g);
}

// Menu « Plus d'actions ».
const menuOpen = ref(false);
const menuIndex = ref(0);
const menu = computed(() => {
  const g = game.value;
  if (!g) return [];
  const items: { label: string; icon: string; run: () => void; danger?: boolean }[] = [
    { label: "Ouvrir sa sauvegarde", icon: "save", run: openSave },
  ];
  if (canRandomize(g)) items.push({ label: "Randomiser", icon: "dice", run: () => randomize(g) });
  items.push({ label: "Éditer la ROM", icon: "pencil", run: () => openRom(g.path) });
  items.push({ label: "Afficher le fichier", icon: "folder", run: () => revealItemInDir(g.path) });
  items.push({
    label: "Retirer de la bibliothèque",
    icon: "x",
    danger: true,
    run: async () => {
      removeItem(g.path);
      await hideGame(g.path);
    },
  });
  return items;
});

function openMenu() {
  menuIndex.value = 0;
  menuOpen.value = true;
  sfx("select");
}

function runMenu(i: number) {
  const item = menu.value[i];
  menuOpen.value = false;
  if (!item) return;
  if (item.label !== "Ouvrir sa sauvegarde" && item.label !== "Retirer de la bibliothèque") stopMusic();
  item.run();
}

// --- Plein écran

async function setImmersive(on: boolean) {
  libraryUi.immersive = on;
  await getCurrentWindow().setFullscreen(on).catch(() => undefined);
}

// --- Clavier et manette

function action(a: PadAction) {
  if (launching.value) return;
  if (menuOpen.value) {
    if (a === "up") menuIndex.value = (menuIndex.value - 1 + menu.value.length) % menu.value.length;
    else if (a === "down") menuIndex.value = (menuIndex.value + 1) % menu.value.length;
    else if (a === "accept") runMenu(menuIndex.value);
    else if (a === "back" || a === "menu") menuOpen.value = false;
    if (a === "up" || a === "down") sfx("move");
    if (a === "back") sfx("back");
    return;
  }
  if (a === "left") move(-1);
  else if (a === "right") move(1);
  else if (a === "accept") play();
  else if (a === "menu" || a === "down") openMenu();
  else if (a === "prevTab") switchTab(-1);
  else if (a === "nextTab") switchTab(1);
  else if (a === "back" && libraryUi.immersive) {
    sfx("back");
    setImmersive(false);
  }
}

const { connected: padConnected } = useGamepad(action);

const KEYS: Record<string, PadAction> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Enter: "accept",
  " ": "accept",
  Escape: "back",
  Backspace: "back",
  q: "prevTab",
  e: "nextTab",
  PageUp: "prevTab",
  PageDown: "nextTab",
  m: "menu",
  ContextMenu: "menu",
};

function onKey(e: KeyboardEvent) {
  const target = e.target as HTMLElement | null;
  if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
  if (e.key === "Home" || e.key === "End") {
    index.value = e.key === "Home" ? 0 : list.value.length - 1;
    sfx("move");
    e.preventDefault();
    return;
  }
  const a = KEYS[e.key];
  if (!a) return;
  e.preventDefault();
  action(a);
}

// --- Horloge

const now = ref(new Date());
let clock: number | undefined;

onMounted(() => {
  if (!emus.loaded) loadEmulators();
  window.addEventListener("keydown", onKey);
  clock = window.setInterval(() => (now.value = new Date()), 15000);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  clearInterval(clock);
  stopMusic();
  if (libraryUi.immersive) setImmersive(false);
});

const time = computed(() => now.value.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));
const date = computed(() => now.value.toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long" }));

const meta = computed(() => {
  const g = game.value;
  if (!g) return [];
  const parts = [platformOf(g) === "nds" ? "Nintendo DS" : "Nintendo 3DS"];
  if (g.generation) parts.push(`${g.generation}ᵉ génération`);
  if (g.language) parts.push(g.language.replace("Multilingue (français inclus)", "Multilingue"));
  return parts;
});
</script>

<template>
  <section class="launcher" :style="{ '--tint': color }" @wheel.passive="onWheel">
    <Backdrop :image="game ? coverUrl(game) : null" :color="color" />

    <header class="topbar">
      <nav class="tabs">
        <span class="key" title="Q ou LB">{{ padConnected ? "LB" : "Q" }}</span>
        <button v-for="t in TABS" :key="t.id" :class="{ active: tab === t.id }" @click="tab = t.id">{{ t.label }}</button>
        <span class="key" title="E ou RB">{{ padConnected ? "RB" : "E" }}</span>
      </nav>
      <div class="status">
        <button class="round" :class="{ off: !audio.music }" :title="audio.music ? 'Couper la musique' : 'Activer la musique'" @click="audio.music = !audio.music">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" />
            <circle cx="18" cy="16" r="3" />
            <path v-if="!audio.music" d="M3 3l18 18" />
          </svg>
        </button>
        <input v-if="audio.music" v-model.number="audio.volume" class="volume" type="range" min="0" max="1" step="0.05" title="Volume" />
        <button class="round" title="Affichage en grille" @click="libraryUi.mode = 'grid'"><Icon name="grid" :size="15" /></button>
        <button class="round" :title="libraryUi.immersive ? 'Quitter le plein écran (Échap)' : 'Plein écran'" @click="setImmersive(!libraryUi.immersive)">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path v-if="!libraryUi.immersive" d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
            <path v-else d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
          </svg>
        </button>
        <span class="clock" :title="date">{{ time }}</span>
      </div>
    </header>

    <div v-if="game" class="hero">
      <Transition name="hero" mode="out-in">
        <div :key="game.path" class="hero-inner">
          <p class="meta">
            <span v-for="m in meta" :key="m">{{ m }}</span>
          </p>
          <h1>{{ game.title }}</h1>
          <p class="sub">
            <span v-if="isKaleidoRom(game)" class="prism">✨ Randomisée par Kaleido<template v-if="game.kaleido"> · seed {{ game.kaleido.seed }}</template></span>
            <span v-if="game.kind === 'ctr_dump'">Mod joué par-dessus le jeu d'origine</span>
            <span v-if="lastPlayed[game.path]">Dernière partie {{ timeAgo(lastPlayed[game.path]) }}</span>
            <span v-else class="dim">{{ game.fileName }}</span>
          </p>
          <div class="cta">
            <button class="play" :disabled="!!installing" @click="play">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M7 4l13 8-13 8z" /></svg>
              <span>{{ playLabel }}</span>
              <span class="key light">{{ padConnected ? "A" : "Entrée" }}</span>
            </button>
            <button class="ghost" @click="openSave"><Icon name="save" :size="16" /> Sauvegarde</button>
            <button class="ghost icon" title="Plus d'actions" @click="openMenu">⋯</button>
          </div>
          <p v-if="notice" class="notice">{{ notice }}</p>
        </div>
      </Transition>
      <div class="now-playing" :class="{ on: audio.playing === game.path, loading: audio.loading === game.path }">
        <span class="eq"><i /><i /><i /><i /></span>
        <span>{{ audio.loading === game.path ? "Chargement de la musique…" : "Thème de l'écran titre" }}</span>
      </div>
    </div>

    <div class="stage">
      <div class="ring">
        <button
          v-for="{ g, i } in visible"
          :key="g.path"
          class="cover"
          :class="{ selected: i === index, kaleido: isKaleidoRom(g) }"
          :style="itemStyle(i)"
          :aria-label="g.title"
          @click="clickCover(i)"
          @pointermove="i === index && onTilt($event)"
          @pointerleave="tilt = { x: 0, y: 0 }"
        >
          <div class="card" :style="i === index ? { transform: `rotateX(${tilt.x}deg) rotateY(${tilt.y}deg)` } : undefined">
            <img v-if="coverUrl(g)" :src="coverUrl(g)!" :alt="g.title" draggable="false" />
            <span v-else class="fallback">{{ g.title }}</span>
            <span class="badge">{{ platformOf(g) === "nds" ? "DS" : "3DS" }}</span>
            <span v-if="isKaleidoRom(g)" class="badge spark">✨</span>
            <span class="shine" />
          </div>
        </button>
      </div>
      <p class="counter">{{ index + 1 }} / {{ list.length }}</p>
    </div>

    <footer class="hints">
      <span><span class="key">◀ ▶</span> Choisir</span>
      <span><span class="key">{{ padConnected ? "A" : "Entrée" }}</span> Jouer</span>
      <span><span class="key">{{ padConnected ? "Start" : "M" }}</span> Plus</span>
      <span v-if="libraryUi.immersive"><span class="key">{{ padConnected ? "B" : "Échap" }}</span> Quitter le plein écran</span>
      <span v-if="padConnected" class="pad"><Icon name="check" :size="13" /> Manette connectée</span>
    </footer>

    <Transition name="fade">
      <div v-if="menuOpen" class="menu-layer" @click.self="menuOpen = false">
        <div class="menu">
          <h3>{{ game?.title }}</h3>
          <button v-for="(m, i) in menu" :key="m.label" :class="{ active: i === menuIndex, danger: m.danger }" @mouseenter="menuIndex = i" @click="runMenu(i)">
            <Icon :name="m.icon" :size="17" /> {{ m.label }}
          </button>
        </div>
      </div>
    </Transition>

    <Transition name="fade">
      <div v-if="launching" class="launch-layer">
        <img v-if="launching.cover" :src="launching.cover" alt="" />
        <h2>{{ launching.title }}</h2>
        <p>{{ launching.status }}</p>
      </div>
    </Transition>
  </section>
</template>

<style scoped>
.launcher {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-rows: auto 1fr auto auto;
  overflow: hidden;
  color: #fff;
  font-family: var(--font);
  user-select: none;
}

.launcher > :not(.menu-layer):not(.launch-layer):not(.backdrop) {
  position: relative;
}

/* --- Barre du haut */

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 22px 34px 0;
}

.tabs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tabs button {
  padding: 7px 16px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  font-weight: 600;
  transition: background 0.2s, color 0.2s;
}

.tabs button:hover {
  color: #fff;
}

.tabs button.active {
  background: rgba(255, 255, 255, 0.95);
  color: #0b0d18;
}

.key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  padding: 2px 7px;
  border: 1px solid rgba(255, 255, 255, 0.35);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 11px;
  font-weight: 700;
}

.key.light {
  border-color: rgba(0, 0, 0, 0.25);
  color: inherit;
  opacity: 0.7;
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 6px 5px 6px;
  border-radius: 999px;
  background: rgba(10, 12, 28, 0.55);
  backdrop-filter: blur(14px);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
}

.round {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: #fff;
}

.round:hover {
  background: rgba(255, 255, 255, 0.12);
}

.round.off {
  color: rgba(255, 255, 255, 0.45);
}

.volume {
  width: 80px;
  accent-color: var(--tint);
}

.clock {
  padding: 0 12px 0 4px;
  font-size: 15px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

/* --- Fiche du jeu */

.hero {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 18px;
  min-height: 0;
  padding: 0 64px;
}

.hero-inner {
  max-width: 760px;
}

.meta {
  display: flex;
  gap: 14px;
  margin: 0 0 10px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.meta span + span::before {
  content: "•";
  margin-right: 14px;
  color: var(--tint);
}

h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(36px, 5.2vw, 68px);
  font-weight: 800;
  line-height: 1.02;
  letter-spacing: -0.02em;
  text-shadow: 0 4px 30px rgba(0, 0, 0, 0.5);
}

.sub {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin: 12px 0 0;
  color: rgba(255, 255, 255, 0.78);
  font-size: 14px;
}

.sub .dim {
  color: rgba(255, 255, 255, 0.5);
}

.prism {
  background: linear-gradient(90deg, #fff, var(--tint));
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  font-weight: 700;
}

.cta {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 26px;
}

.play {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  padding: 15px 22px 15px 26px;
  border: none;
  border-radius: 999px;
  background: #fff;
  color: #0b0d18;
  font-size: 17px;
  font-weight: 800;
  box-shadow: 0 0 0 0 var(--tint), 0 10px 40px color-mix(in srgb, var(--tint) 55%, transparent);
  transition: transform 0.15s ease, box-shadow 0.4s ease;
  animation: breathe 2.6s ease-in-out infinite;
}

.play:hover:not(:disabled) {
  transform: scale(1.04);
}

.play:disabled {
  animation: none;
  cursor: progress;
}

@keyframes breathe {
  50% {
    box-shadow: 0 0 0 6px color-mix(in srgb, var(--tint) 35%, transparent), 0 10px 50px color-mix(in srgb, var(--tint) 70%, transparent);
  }
}

.ghost {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 13px 20px;
  border: none;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  font-size: 15px;
  font-weight: 600;
  backdrop-filter: blur(12px);
  transition: background 0.2s;
}

.ghost:hover {
  background: rgba(255, 255, 255, 0.22);
}

.ghost.icon {
  width: 48px;
  justify-content: center;
  padding: 13px 0;
  font-size: 20px;
  line-height: 1;
}

.notice {
  margin: 14px 0 0;
  color: rgba(255, 255, 255, 0.8);
  font-size: 13px;
}

.now-playing {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  align-self: flex-start;
  padding: 6px 14px 6px 10px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.35);
  color: rgba(255, 255, 255, 0.85);
  font-size: 12px;
  opacity: 0;
  transform: translateY(6px);
  transition: opacity 0.4s, transform 0.4s;
}

.now-playing.on,
.now-playing.loading {
  opacity: 1;
  transform: none;
}

.eq {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 14px;
}

.eq i {
  width: 3px;
  height: 30%;
  border-radius: 2px;
  background: var(--tint);
}

.now-playing.on .eq i {
  animation: eq 0.9s ease-in-out infinite alternate;
}

.eq i:nth-child(2) {
  animation-delay: -0.3s !important;
}

.eq i:nth-child(3) {
  animation-delay: -0.6s !important;
}

.eq i:nth-child(4) {
  animation-delay: -0.15s !important;
}

@keyframes eq {
  from {
    height: 20%;
  }
  to {
    height: 100%;
  }
}

.hero-enter-active {
  transition: opacity 0.35s ease, transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.hero-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.hero-enter-from {
  opacity: 0;
  transform: translateX(-24px);
}

.hero-leave-to {
  opacity: 0;
  transform: translateX(12px);
}

/* --- Carrousel */

.stage {
  height: 430px;
  perspective: 1100px;
}

.ring {
  position: absolute;
  left: 50%;
  top: 47%;
  transform-style: preserve-3d;
}

.cover {
  position: absolute;
  left: -150px;
  top: -133px;
  width: 300px;
  height: 266px;
  padding: 0;
  border: none;
  background: none;
  transform-style: preserve-3d;
  transition: transform 0.5s cubic-bezier(0.2, 0.85, 0.25, 1), opacity 0.4s, filter 0.4s;
  -webkit-box-reflect: below 10px linear-gradient(transparent 62%, rgba(255, 255, 255, 0.16));
}

.card {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 34px rgba(0, 0, 0, 0.5);
  transition: transform 0.2s ease-out, box-shadow 0.45s ease;
}

.selected .card {
  box-shadow:
    0 0 0 3px #fff,
    0 0 0 7px color-mix(in srgb, var(--tint) 85%, transparent),
    0 0 50px 8px color-mix(in srgb, var(--tint) 70%, transparent),
    0 24px 50px rgba(0, 0, 0, 0.55);
  animation: halo 2.6s ease-in-out infinite;
}

@keyframes halo {
  50% {
    box-shadow:
      0 0 0 3px #fff,
      0 0 0 9px color-mix(in srgb, var(--tint) 60%, transparent),
      0 0 80px 14px color-mix(in srgb, var(--tint) 75%, transparent),
      0 24px 50px rgba(0, 0, 0, 0.55);
  }
}

.card img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.fallback {
  display: flex;
  align-items: flex-end;
  height: 100%;
  padding: 16px;
  background: linear-gradient(135deg, var(--tint), #1b1f3b);
  font-size: 18px;
  font-weight: 800;
  text-align: left;
}

.badge {
  position: absolute;
  top: 8px;
  left: 8px;
  padding: 1px 7px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.65);
  font-size: 10px;
  font-weight: 800;
}

.badge.spark {
  left: auto;
  right: 8px;
}

/* Reflet qui balaie la jaquette sélectionnée. */
.shine {
  position: absolute;
  inset: 0;
  background: linear-gradient(115deg, transparent 40%, rgba(255, 255, 255, 0.28) 50%, transparent 60%);
  background-size: 250% 100%;
  background-position: 150% 0;
  pointer-events: none;
}

.selected .shine {
  animation: shine 4.5s ease-in-out infinite;
}

@keyframes shine {
  0%,
  60% {
    background-position: 150% 0;
  }
  100% {
    background-position: -100% 0;
  }
}

.counter {
  position: absolute;
  right: 40px;
  bottom: 10px;
  margin: 0;
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* --- Aide */

.hints {
  display: flex;
  justify-content: center;
  gap: 26px;
  padding: 12px 24px 20px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
}

.hints > span {
  display: inline-flex;
  align-items: center;
  gap: 7px;
}

.pad {
  color: var(--tint);
}

/* --- Menu et lancement */

.menu-layer,
.launch-layer {
  position: absolute;
  inset: 0;
  z-index: 200;
  display: grid;
  place-items: center;
  background: rgba(3, 4, 12, 0.6);
  backdrop-filter: blur(10px);
}

.menu {
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 360px;
  padding: 18px;
  border-radius: 20px;
  background: rgba(16, 18, 36, 0.92);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08), 0 30px 80px rgba(0, 0, 0, 0.6);
}

.menu h3 {
  margin: 0 0 10px;
  padding: 0 10px;
  font-size: 15px;
}

.menu button {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: #fff;
  font-size: 14px;
  text-align: left;
}

.menu button.active {
  background: rgba(255, 255, 255, 0.12);
  box-shadow: inset 3px 0 0 var(--tint);
}

.menu button.danger {
  color: #ff8a8a;
}

.launch-layer {
  align-content: center;
  gap: 14px;
  text-align: center;
}

.launch-layer img {
  width: 300px;
  border-radius: 18px;
  box-shadow: 0 0 0 3px #fff, 0 0 90px 20px color-mix(in srgb, var(--tint) 70%, transparent);
  animation: launch 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
}

.launch-layer h2 {
  margin: 10px 0 0;
  font-size: 26px;
}

.launch-layer p {
  margin: 0;
  color: rgba(255, 255, 255, 0.75);
}

@keyframes launch {
  from {
    transform: scale(0.7) rotateY(-25deg);
    opacity: 0;
  }
  to {
    transform: none;
    opacity: 1;
  }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@media (max-height: 820px) {
  .stage {
    zoom: 0.78;
  }

  h1 {
    font-size: clamp(30px, 4.4vw, 54px);
  }
}
</style>
