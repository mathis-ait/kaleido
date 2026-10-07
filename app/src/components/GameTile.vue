<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Icon from "./Icon.vue";
import { openRom } from "../editor";
import { hideGame } from "../games";
import { removeItem } from "../library";
import { isKaleidoRom, type Detection } from "../types";
import { PLATFORM_LABEL, PLATFORM_SHORT, RECOMMENDED, defaultEmulator, emus, installs, type PlayPlatform } from "../play/play";
import { platformOf, canRandomize as canRandomizeGame, coverUrl, formatDuration, launchGame, openSaveOf, playTime, randomize as randomizeGame, statusOf } from "../launcher/actions";
import { audio, previewMusic, stopMusic } from "../launcher/audio";
import { openMods } from "../play/mods";

const props = defineProps<{ game: Detection }>();

const platform = computed<PlayPlatform>(() => platformOf(props.game));
const isSwitch = computed(() => platform.value === "switch");
const randomized = computed(() => isKaleidoRom(props.game));
/** Boîte française en priorité, quelle que soit la langue de la ROM. */
const coverSrc = computed(() => coverUrl(props.game));
const coverFailed = ref(false);
const coverLoaded = ref(false);

const busy = ref(false);
const status = ref<string | null>(null);
const menuOpen = ref(false);

const emulator = computed(() => (emus.loaded ? defaultEmulator(platform.value) : null));
const installing = computed(() => !!installs[RECOMMENDED[platform.value]]);
const playHint = computed(() => {
  if (!emus.loaded) return "";
  return emulator.value ? `Lancer dans ${emulator.value.name}` : "Aucun émulateur trouvé : Kaleido te propose de l'installer";
});

async function launch() {
  busy.value = true;
  status.value = null;
  stopMusic();
  const emulator = await launchGame(props.game);
  busy.value = false;
  if (emulator) status.value = `Lancé dans ${emulator}`;
}

async function openSave() {
  menuOpen.value = false;
  stopMusic();
  status.value = await openSaveOf(props.game);
}

const randomize = () => randomizeGame(props.game);

// Musique de l'écran titre au survol.
const hovered = ref(false);
function enter() {
  hovered.value = true;
  previewMusic(props.game, 700);
}
function leave() {
  hovered.value = false;
  menuOpen.value = false;
  if (audio.playing === props.game.path || audio.loading === props.game.path) stopMusic();
  else previewMusic(null);
}
onUnmounted(() => {
  if (hovered.value) stopMusic();
});

async function remove() {
  menuOpen.value = false;
  removeItem(props.game.path);
  await hideGame(props.game.path);
}

const canRandomize = computed(() => canRandomizeGame(props.game));
const gameStatus = computed(() => statusOf(props.game));
const time = computed(() => playTime(gameStatus.value));
const shortTitle = computed(() => props.game.title.replace(/^Pokémon\s+/, ""));
</script>

<template>
  <article class="tile" :class="{ randomized, singing: audio.playing === game.path }" @mouseenter="enter" @mouseleave="leave">
    <div class="cover" :class="platform">
      <template v-if="coverSrc && !coverFailed">
        <img class="backdrop" :src="coverSrc" alt="" aria-hidden="true" />
        <img class="art" :class="{ loaded: coverLoaded }" :src="coverSrc" :alt="game.title" loading="lazy" @load="coverLoaded = true" @error="coverFailed = true" />
      </template>
      <div v-else class="placeholder">
        <span class="ph-platform">{{ PLATFORM_LABEL[platform] }}</span>
        <span class="ph-title">{{ shortTitle }}</span>
      </div>

      <div class="badges">
        <span class="badge">{{ PLATFORM_SHORT[platform] }}</span>
        <span v-if="randomized" class="badge prism" :title="game.kaleido ? `Seed ${game.kaleido.seed}` : 'ROM générée par Kaleido'">Randomisée</span>
        <span v-if="game.kind === 'ctr_dump'" class="badge" title="Dossier : joué comme mod par-dessus le jeu d'origine">Mod</span>
      </div>

      <button class="play" :disabled="busy || installing" :title="playHint" @click="launch">
        <Icon name="play" :size="18" />
        <span>{{ installing ? "Installation…" : busy ? "Lancement…" : "Jouer" }}</span>
      </button>
    </div>

    <div class="meta">
      <div class="text">
        <h3 :title="game.title">{{ game.title }}</h3>
        <p :title="time ? `Temps de jeu ${time.source}` : game.path">{{ time ? `${formatDuration(time.seconds)} de jeu` : (game.language ?? game.fileName) }}</p>
      </div>
      <div class="more">
        <button class="icon-btn" aria-label="Mods et réglages" title="Mods et réglages (60 FPS, textures HD, réglages optimaux…)" @click="openMods(game)">
          <Icon name="wand" :size="16" />
        </button>
        <button class="icon-btn" aria-label="Plus d'actions" title="Plus d'actions" @click="menuOpen = !menuOpen">⋯</button>
        <div v-if="menuOpen" class="menu panel" @click="menuOpen = false">
          <button @click="openMods(game)"><Icon name="wand" :size="15" /> Mods et réglages</button>
          <button v-if="gameStatus?.saveExists" @click="openSave"><Icon name="save" :size="15" /> Ouvrir sa sauvegarde</button>
          <button v-if="canRandomize" @click="randomize"><Icon name="dice" :size="15" /> Randomiser</button>
          <button v-if="!isSwitch" @click="openRom(game.path)"><Icon name="pencil" :size="15" /> Éditer la ROM</button>
          <button @click="revealItemInDir(game.path)"><Icon name="folder" :size="15" /> Afficher le fichier</button>
          <button class="danger" @click.stop="remove"><Icon name="x" :size="15" /> Retirer de la bibliothèque</button>
        </div>
      </div>
    </div>
    <p v-if="status" class="status">{{ status }}</p>
  </article>
</template>

<style scoped>
.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.cover {
  position: relative;
  aspect-ratio: 1 / 1;
  overflow: hidden;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--panel);
  box-shadow: var(--shadow);
  transition: transform 0.2s ease, box-shadow 0.2s ease;
}

.tile:hover .cover {
  transform: translateY(-3px);
}

.randomized .cover {
  border-color: var(--accent);
  box-shadow: var(--shadow), 0 0 0 1px var(--accent);
}

.backdrop {
  position: absolute;
  inset: -20px;
  width: calc(100% + 40px);
  height: calc(100% + 40px);
  object-fit: cover;
  filter: blur(22px) saturate(1.3) brightness(0.65);
}

.art {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  padding: 10px;
  object-fit: contain;
  filter: drop-shadow(0 6px 14px rgba(0, 0, 0, 0.45));
  opacity: 0;
  transition: opacity 0.3s ease;
}

.art.loaded {
  opacity: 1;
}

.placeholder {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 4px;
  padding: 18px;
  background: color-mix(in srgb, var(--text) 8%, var(--surface));
  color: var(--text);
}

.ph-platform {
  font-size: 12px;
  opacity: 0.85;
}

.ph-title {
  font-family: var(--font-display);
  font-size: 22px;
  font-weight: 700;
  line-height: 1.15;
}

.badges {
  position: absolute;
  top: 10px;
  left: 10px;
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.badge {
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.6);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  backdrop-filter: blur(6px);
}

/* Pastille « Randomisée » : inversée (claire) au lieu d'un dégradé, posée sur la jaquette. */
.badge.prism {
  background: rgba(255, 255, 255, 0.92);
  color: #16181f;
}

.play {
  position: absolute;
  left: 50%;
  bottom: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 22px;
  border: none;
  border-radius: var(--radius-pill);
  background: var(--text);
  color: var(--bg);
  font-weight: 700;
  font-size: 15px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.4);
  transform: translate(-50%, 6px);
  opacity: 0;
  transition: opacity 0.18s ease, transform 0.18s ease, background-color 0.15s;
}

.tile:hover .play,
.play:focus-visible,
.play:disabled {
  opacity: 1;
  transform: translate(-50%, 0);
}

.play:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text) 86%, var(--bg));
}

.play:disabled {
  cursor: progress;
}

.meta {
  display: flex;
  align-items: flex-start;
  gap: 6px;
}

.text {
  flex: 1;
  min-width: 0;
}

h3 {
  overflow: hidden;
  font-size: 15px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.text p,
.status {
  margin: 2px 0 0;
  overflow: hidden;
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.status {
  margin: -4px 0 0;
  color: var(--ok);
  white-space: normal;
}

.more {
  position: relative;
  display: flex;
  gap: 2px;
}

.icon-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  font-size: 18px;
  line-height: 1;
  transition: background-color 0.15s, color 0.15s;
}

.icon-btn:hover {
  background: color-mix(in srgb, var(--text) 12%, transparent);
  color: var(--text);
}

.menu {
  position: absolute;
  right: 0;
  bottom: 34px;
  z-index: 5;
  display: flex;
  flex-direction: column;
  min-width: 220px;
  padding: 6px;
  background: var(--surface);
  box-shadow: var(--shadow-pop);
}

.menu button {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  text-align: left;
}

.menu button:hover {
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.menu .danger {
  color: var(--danger);
}
</style>
