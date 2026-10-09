<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Icon from "./Icon.vue";
import { openRom } from "../editor";
import { hideGame } from "../games";
import { removeItem } from "../library";
import { CONSOLES, consoleOf } from "../consoles";
import { isKaleidoRom, type Detection } from "../types";
import { RECOMMENDED, defaultEmulator, emus, installs } from "../play/play";
import { platformOf, canRandomize as canRandomizeGame, coverUrl, isPokemonGame, formatDuration, lastPlayed, launchGame, openSaveOf, playTime, randomize as randomizeGame, statusOf, timeAgo } from "../launcher/actions";
import { audio, previewMusic, stopMusic } from "../launcher/audio";
import { modUpdatesOf, openMods } from "../play/mods";

/**
 * Jeu de la grille : la jaquette sur un fond flouté de ses propres couleurs, le titre et
 * une ligne d'état dessous. Jouer et les actions apparaissent au survol (et au clavier).
 * La console n'est pas répétée sur la jaquette : la boîte la montre déjà, la section aussi.
 */
const props = defineProps<{ game: Detection; showConsole?: boolean }>();

const randomized = computed(() => isKaleidoRom(props.game));
const pendingMods = computed(() => modUpdatesOf(props.game));
const incomplete = computed(() => props.game.warnings.find((w) => w.startsWith("Fichier incomplet")) ?? "");
const coverSrc = computed(() => coverUrl(props.game));
const coverFailed = ref(false);
const coverLoaded = ref(false);

const busy = ref(false);
const status = ref<string | null>(null);
const menuOpen = ref(false);

const platform = computed(() => platformOf(props.game));
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
  const name = await launchGame(props.game);
  busy.value = false;
  if (name) status.value = `Lancé dans ${name}`;
}

async function openSave() {
  menuOpen.value = false;
  stopMusic();
  status.value = await openSaveOf(props.game);
}

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

/** Ligne sous le titre : temps de jeu et dernière partie, sinon la langue. */
const line = computed(() => {
  const parts: string[] = [];
  if (props.showConsole) parts.push(CONSOLES[consoleOf(props.game)].label);
  if (time.value) parts.push(formatDuration(time.value.seconds));
  const last = lastPlayed[props.game.path];
  if (last) parts.push(timeAgo(last));
  if (!time.value && !last) parts.push(gameStatus.value?.saveExists ? "Partie en cours" : "Pas encore joué");
  return parts.join(" · ");
});
</script>

<template>
  <article class="tile" :class="{ singing: audio.playing === game.path, open: menuOpen }" @mouseenter="enter" @mouseleave="leave">
    <div class="cover">
      <template v-if="coverSrc && !coverFailed">
        <img class="blur" :src="coverSrc" alt="" aria-hidden="true" />
        <img class="art" :class="{ loaded: coverLoaded }" :src="coverSrc" :alt="game.title" loading="lazy" @load="coverLoaded = true" @error="coverFailed = true" />
      </template>
      <div v-else class="placeholder">
        <span>{{ shortTitle }}</span>
      </div>

      <button class="play" :disabled="busy || installing" :title="playHint" @click="launch">
        <Icon name="play" :size="16" />
        <span>{{ installing ? "Installation…" : busy ? "Lancement…" : "Jouer" }}</span>
      </button>
    </div>

    <div class="meta">
      <div class="text">
        <h3 :title="game.title">{{ game.title }}</h3>
        <p :title="time ? `Temps de jeu ${time.source}` : game.path">
          <span v-if="randomized" class="tag" :title="game.kaleido ? `Seed ${game.kaleido.seed}` : 'ROM générée par Kaleido'">Randomisée</span>
          <span v-if="game.kind === 'ctr_dump'" class="tag" title="Dossier : joué comme mod par-dessus le jeu d'origine">Mod</span>
          <span v-if="incomplete" class="tag tag-warn" :title="incomplete">Fichier incomplet</span>
          <button v-if="pendingMods" class="tag tag-btn" :title="`${pendingMods} mod${pendingMods > 1 ? 's ont' : ' a'} une nouvelle version`" @click="openMods(game)">Mods à mettre à jour</button>
          {{ line }}
        </p>
      </div>
      <div class="more">
        <button class="icon-btn" aria-label="Plus d'actions" title="Plus d'actions" :aria-expanded="menuOpen" @click="menuOpen = !menuOpen">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg>
        </button>
        <div v-if="menuOpen" class="menu" @click="menuOpen = false">
          <button @click="openMods(game)"><Icon name="wand" :size="15" /> Mods et réglages<template v-if="pendingMods"> · {{ pendingMods }} à mettre à jour</template></button>
          <button v-if="gameStatus?.saveExists && isPokemonGame(game)" @click="openSave"><Icon name="save" :size="15" /> Ouvrir sa sauvegarde</button>
          <button v-if="canRandomize" @click="randomizeGame(game)"><Icon name="dice" :size="15" /> Randomiser</button>
          <button v-if="isPokemonGame(game)" @click="openRom(game.path)"><Icon name="pencil" :size="15" /> Éditer la ROM</button>
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
  gap: var(--sp-3);
}

.tile.open {
  z-index: 5;
}

.cover {
  position: relative;
  aspect-ratio: 1 / 1;
  overflow: hidden;
  border-radius: var(--radius-card);
  background: var(--panel);
  box-shadow: var(--shadow);
  transition:
    transform 0.25s cubic-bezier(0.2, 0.85, 0.25, 1),
    box-shadow 0.25s ease;
}

.tile:hover .cover,
.tile:focus-within .cover {
  transform: translateY(-4px);
  box-shadow:
    0 0 0 2px var(--text),
    var(--shadow-pop);
}

.blur {
  position: absolute;
  inset: -24px;
  width: calc(100% + 48px);
  height: calc(100% + 48px);
  object-fit: cover;
  filter: blur(26px) saturate(1.25) brightness(0.62);
}

.art {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  padding: 12%;
  object-fit: contain;
  filter: drop-shadow(0 10px 18px rgb(0 0 0 / 0.5));
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
  align-items: flex-end;
  padding: var(--sp-5);
  background: color-mix(in srgb, var(--text) 7%, var(--surface));
  font-family: var(--font-display);
  font-size: var(--fs-xl);
  font-weight: 700;
  line-height: 1.15;
}

/* Jouer : pastille claire posée en bas de la jaquette, au survol ou au focus. */
.play {
  position: absolute;
  left: 50%;
  bottom: var(--sp-3);
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 9px 20px;
  border: none;
  border-radius: var(--radius-pill);
  background: #fff;
  color: #0b0d18;
  font-size: var(--fs-base);
  font-weight: 700;
  box-shadow: 0 6px 20px rgb(0 0 0 / 0.4);
  transform: translate(-50%, 8px);
  opacity: 0;
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}

.tile:hover .play,
.tile:focus-within .play,
.play:disabled {
  opacity: 1;
  transform: translate(-50%, 0);
}

.play:disabled {
  cursor: progress;
}

.meta {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-1);
}

.text {
  flex: 1;
  min-width: 0;
}

h3 {
  overflow: hidden;
  font-size: var(--fs-base);
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.text p,
.status {
  margin: 3px 0 0;
  overflow: hidden;
  color: var(--text-dim);
  font-size: var(--fs-sm);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.tag {
  margin-right: var(--sp-1);
  padding: 1px 6px;
  border-radius: var(--radius-xs);
  background: var(--text);
  color: var(--bg);
  font-size: var(--fs-xs);
  font-weight: 700;
}

.tag-warn {
  background: var(--warn);
  color: var(--bg);
}

.tag-btn {
  border: none;
  font-family: inherit;
  line-height: inherit;
  cursor: pointer;
}

.tag-btn:hover {
  background: color-mix(in srgb, var(--text) 80%, transparent);
}

.status {
  color: var(--ok);
  white-space: normal;
}

.more {
  position: relative;
}

.icon-btn {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  opacity: 0;
  transition:
    background-color 0.15s,
    color 0.15s,
    opacity 0.15s;
}

.tile:hover .icon-btn,
.tile:focus-within .icon-btn,
.tile.open .icon-btn {
  opacity: 1;
}

.icon-btn:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.menu {
  position: absolute;
  right: 0;
  bottom: 34px;
  display: flex;
  flex-direction: column;
  min-width: 230px;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  box-shadow: var(--shadow-pop);
}

.menu button {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text);
  font-size: var(--fs-md);
  text-align: left;
}

.menu button:hover {
  background: var(--panel-hover);
}

.menu .danger {
  color: var(--danger);
}
</style>
