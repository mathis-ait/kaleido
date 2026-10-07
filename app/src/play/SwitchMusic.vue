<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { message } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import { titleIdOf } from "../games";
import { forgetMusic, stopMusic } from "../launcher/audio";
import type { Detection } from "../types";
import { formatSize } from "./mods";

/**
 * « Musique du lanceur » d'un jeu Switch : morceau deviné par Kaleido, ou choisi à
 * l'écoute parmi les morceaux du jeu (les plus gros d'abord).
 */

interface Track {
  id: string;
  name: string;
  size: number;
  seconds: number | null;
}
interface TrackList {
  tracks: Track[];
  chosen: string | null;
  auto: string | null;
  autoError: string | null;
  vgmstream: boolean;
}

const props = defineProps<{ game: Detection }>();
const titleId = computed(() => titleIdOf(props.game) ?? "");

const list = ref<TrackList | null>(null);
const error = ref<string | null>(null);
const open = ref(false);
const playing = ref<string | null>(null);
const loadingId = ref<string | null>(null);
const needsDecoder = ref(false);
const installing = ref<{ step: string; done: number; total: number } | null>(null);
const shown = ref(40);

async function load() {
  error.value = null;
  try {
    list.value = await invoke<TrackList>("music_switch_tracks", { path: props.game.path, titleId: titleId.value });
  } catch (e) {
    error.value = String(e);
  }
}
onMounted(load);

const current = computed(() => list.value?.chosen ?? list.value?.auto ?? null);
/** Le morceau utilisé en tête, puis les autres (les plus gros d'abord). */
const ordered = computed(() => {
  const tracks = list.value?.tracks ?? [];
  const cur = tracks.find((t) => t.id === current.value);
  return cur ? [cur, ...tracks.filter((t) => t !== cur)] : tracks;
});
const duration = (s: number | null) => (s === null ? null : `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, "0")}`);
const nameOf = (id: string | null) => (id ? (list.value?.tracks.find((t) => t.id === id)?.name ?? id.split(/[/:]/).pop() ?? id) : null);

// --- Écoute

let ctx: AudioContext | null = null;
let source: AudioBufferSourceNode | null = null;

function stop() {
  try {
    source?.stop();
  } catch {
    /* déjà arrêté */
  }
  source = null;
  playing.value = null;
}

async function listen_(id: string) {
  if (playing.value === id) return stop();
  stop();
  stopMusic();
  loadingId.value = id;
  try {
    const bytes = await invoke<ArrayBuffer>("music_switch_preview", { path: props.game.path, trackId: id });
    ctx ??= new AudioContext();
    const buffer = await ctx.decodeAudioData(bytes);
    source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(ctx.destination);
    source.onended = () => {
      if (playing.value === id) playing.value = null;
    };
    source.start();
    playing.value = id;
  } catch (e) {
    if (String(e).includes("VGMSTREAM_MISSING")) needsDecoder.value = true;
    else await message(String(e), { title: "Écoute impossible", kind: "error" });
  } finally {
    loadingId.value = null;
  }
}

onBeforeUnmount(() => {
  stop();
  ctx?.close().catch(() => undefined);
});

// --- Choix

async function choose(id: string | null) {
  try {
    await invoke("music_switch_choose", { titleId: titleId.value, trackId: id });
    forgetMusic(props.game.path);
    await load();
  } catch (e) {
    await message(String(e), { title: "Choix non enregistré", kind: "error" });
  }
}

// --- Décodeur vgmstream

let unlisten: (() => void) | null = null;
onMounted(async () => {
  unlisten = await listen<{ step: string; done: number; total: number }>("tool-install", (e) => (installing.value = e.payload)).catch(() => null);
});
onBeforeUnmount(() => unlisten?.());

async function installDecoder() {
  installing.value = { step: "download", done: 0, total: 0 };
  try {
    await invoke<string>("vgmstream_install");
    needsDecoder.value = false;
    await load();
  } catch (e) {
    await message(String(e), { title: "Installation impossible", kind: "error" });
  } finally {
    installing.value = null;
  }
}
</script>

<template>
  <section class="block music">
    <div class="head">
      <h3><Icon name="play" :size="16" /> Musique du lanceur</h3>
      <button v-if="list" class="btn small" @click="open = !open">{{ open ? "Fermer la liste" : "Choisir un autre morceau" }}</button>
    </div>

    <p v-if="error" class="error small">{{ error }}</p>
    <template v-else-if="list">
      <p class="small">
        <template v-if="list.chosen">Morceau choisi : <strong>{{ nameOf(list.chosen) }}</strong></template>
        <template v-else-if="list.auto">Morceau trouvé automatiquement : <strong>{{ nameOf(list.auto) }}</strong></template>
        <template v-else>{{ list.autoError ?? "Aucun morceau trouvé automatiquement." }}</template>
        <button v-if="list.chosen" class="link" @click="choose(null)">Revenir au choix automatique</button>
      </p>

      <div v-if="needsDecoder || installing" class="decoder">
        <p class="small">Ce morceau est dans un format que Kaleido confie à <strong>vgmstream</strong>, un décodeur libre qui lit la plupart des formats audio de jeux (4 Mo, téléchargé depuis sa page officielle).</p>
        <button class="btn btn-primary small" :disabled="!!installing" @click="installDecoder">
          <Icon name="download" :size="14" />
          {{ installing ? (installing.step === "extract" ? "Installation…" : `Téléchargement… ${installing.total ? formatSize(installing.done) : ""}`) : "Installer vgmstream" }}
        </button>
      </div>

      <ul v-if="open" class="tracks">
        <li v-for="t in ordered.slice(0, shown)" :key="t.id" :class="{ current: t.id === current }">
          <button class="listen" :aria-label="playing === t.id ? 'Arrêter' : 'Écouter'" :disabled="loadingId === t.id" @click="listen_(t.id)">
            <Icon :name="playing === t.id ? 'x' : 'play'" :size="14" />
          </button>
          <span class="name" :title="t.id">{{ t.name }}</span>
          <span class="dim size">{{ duration(t.seconds) ?? formatSize(t.size) }}</span>
          <span v-if="t.id === current" class="tag">Utilisé</span>
          <button v-else class="btn small" @click="choose(t.id)">Utiliser</button>
        </li>
        <li v-if="list.tracks.length > shown" class="more">
          <button class="link" @click="shown += 60">Afficher plus ({{ list.tracks.length - shown }} restants)</button>
        </li>
      </ul>
      <p v-if="open" class="dim small">Les musiques sont en général les plus gros fichiers. L'écoute dure 50 secondes, comme dans le lanceur.</p>
    </template>
    <p v-else class="dim small">Lecture des morceaux du jeu…</p>
  </section>
</template>

<style scoped>
.music {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

h3 {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 16px;
}

.small {
  margin: 0;
  font-size: 13px;
}

.btn.small {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  font-size: 13px;
}

.decoder {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
}

.tracks {
  display: flex;
  flex-direction: column;
  max-height: 320px;
  margin: 0;
  padding: 4px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  list-style: none;
}

.tracks li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 5px 8px;
  border-radius: 8px;
  font-size: 13px;
}

.tracks li.current {
  background: var(--text);
  color: var(--bg);
}

.tracks li.current .dim {
  color: inherit;
  opacity: 0.7;
}

.listen {
  display: grid;
  flex: none;
  width: 28px;
  height: 28px;
  place-items: center;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.listen:hover {
  background: var(--panel-hover);
}

.tracks li.current .listen {
  border-color: currentColor;
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.size {
  white-space: nowrap;
}

.tag {
  font-weight: 600;
}

.more {
  justify-content: center;
}

.dim {
  color: var(--text-dim);
}

.error {
  color: var(--danger);
}

.link {
  margin-left: 8px;
  padding: 0;
  border: none;
  background: none;
  color: var(--text-dim);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}
</style>
