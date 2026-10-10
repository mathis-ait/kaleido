<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import { goTo, openSave, saveState } from "../saveStore";
import { knownSaves, SAVES_FOLDER_KEY } from "./knownSaves";
import type { Gender } from "../types";
import { useShell } from "./shell";

const emit = defineEmits<{ open: [] }>();

interface Peek {
  path: string;
  fileName: string;
  game: string;
  version: string;
  generation: number;
  trainer: { name: string; tid: number; sid: number; displayId: number; gender: Gender; money: number; playTime: { hours: number; minutes: number; seconds: number } };
  party: [number, boolean][];
  caught: number;
  seen: number;
  dexMax: number;
  stored: number;
  modified: number | null;
}

const FOLDER_KEY = SAVES_FOLDER_KEY;
const folder = ref<string | null>(readFolder());
const saves = ref<Peek[]>([]);
const failed = ref<string[]>([]);
const loading = ref(false);
const search = ref("");
const genFilter = ref<number | null>(null);
const selected = ref<string | null>(null);
/** Émulateur qui utilise chaque sauvegarde détectée automatiquement. */
const emulatorOf = ref<Record<string, string>>({});

function readFolder() {
  try {
    return localStorage.getItem(FOLDER_KEY);
  } catch {
    return null;
  }
}

async function chooseFolder() {
  const dir = await open({ title: "Dossier de tes sauvegardes", directory: true });
  if (typeof dir !== "string") return;
  folder.value = dir;
  try {
    localStorage.setItem(FOLDER_KEY, dir);
  } catch {
    /* non mémorisé */
  }
  refresh();
}

async function refresh() {
  loading.value = true;
  failed.value = [];
  const known = await knownSaves();
  emulatorOf.value = known.emulatorOf;
  const paths = known.paths;
  const results = await Promise.all(
    paths.map((path) =>
      invoke<Peek>("peek_save", { path }).catch(() => {
        // ROM ou fichier non pris en charge : ignoré, sauf s'il ressemble à une sauvegarde.
        if (/\.(sav|dsv|main|bin)$/i.test(path)) failed.value.push(path);
        return null;
      }),
    ),
  );
  saves.value = results.filter((r): r is Peek => !!r).sort((a, b) => a.generation - b.generation || a.game.localeCompare(b.game));
  loading.value = false;
}
onMounted(refresh);

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const shown = computed(() => {
  const q = fold(search.value.trim());
  return saves.value.filter(
    (s) => (genFilter.value === null || s.generation === genFilter.value) && (!q || fold(`${s.game} ${s.trainer.name} ${s.fileName}`).includes(q)),
  );
});
const gens = computed(() => [...new Set(saves.value.map((s) => s.generation))].sort());
const groups = computed(() => {
  const ds = shown.value.filter((s) => s.generation <= 5);
  const ctr = shown.value.filter((s) => s.generation >= 6);
  return [
    { title: "Nintendo DS", list: ds },
    { title: "Nintendo 3DS", list: ctr },
  ].filter((g) => g.list.length);
});

/** Couleurs de la jaquette selon le jeu. */
const COVER: Record<string, [string, string, string]> = {
  diamond_pearl: ["#7aa7ff", "#d9a0d8", "DP"],
  platinum: ["#9aa4b8", "#3b3f52", "Pt"],
  heart_gold_soul_silver: ["#f2c94c", "#b8c2d6", "HGSS"],
  black_white: ["#2d2d2d", "#e8e8e8", "NB"],
  black2_white2: ["#1f3b73", "#e6edf7", "N2B2"],
  x_y: ["#3b82f6", "#ef4444", "XY"],
  omega_ruby_alpha_sapphire: ["#dc2626", "#2563eb", "ROSA"],
  sun_moon: ["#f59e0b", "#6d28d9", "SL"],
  ultra_sun_ultra_moon: ["#ea580c", "#4c1d95", "USUL"],
};
const cover = (v: string) => COVER[v] ?? ["#64748b", "#334155", "?"];

const date = (s: number | null) => (s ? new Date(s * 1000).toLocaleDateString("fr-FR", { day: "numeric", month: "short", year: "numeric" }) : "");
const pad = (n: number) => String(n).padStart(2, "0");
const isOpen = (s: Peek) => s.path === saveState.path;

function go(s: Peek) {
  if (!isOpen(s)) openSave(s.path);
  else saveState.page = "home";
}

useShell(() => ({
  hint: "Double-clic sur une sauvegarde pour l'ouvrir",
  actions: [
    { key: "Ctrl+o", cap: "Ctrl+O", label: "Ouvrir un fichier", run: () => emit("open") },
    { key: "r", cap: "R", label: "Actualiser", run: refresh },
    {
      key: "Enter",
      cap: "Entrée",
      label: "Ouvrir",
      run: () => {
        const s = saves.value.find((x) => x.path === selected.value);
        if (s) go(s);
      },
      disabled: !selected.value,
    },
  ],
}));
</script>

<template>
  <div class="manager sv-panel">
    <div class="bar">
      <input v-model="search" class="sv-input search" placeholder="Chercher une sauvegarde…" />
      <div class="chips">
        <button class="sv-chip" :class="{ on: genFilter === null }" @click="genFilter = null">Toutes</button>
        <button v-for="g in gens" :key="g" class="sv-chip" :class="{ on: genFilter === g }" @click="genFilter = g">Gen {{ g }}</button>
      </div>
      <span class="count">{{ shown.length }} sauvegarde{{ shown.length > 1 ? "s" : "" }}</span>
      <button class="sv-btn" @click="refresh"><Icon name="refresh" :size="15" /> Actualiser</button>
      <button class="sv-btn" @click="chooseFolder"><Icon name="folder" :size="15" /> {{ folder ? "Changer de dossier" : "Choisir un dossier" }}</button>
      <button v-if="folder" class="sv-btn" title="Ouvrir le dossier dans l'explorateur" @click="openPath(folder)"><Icon name="folder-open" :size="15" /></button>
      <button class="sv-btn" title="Banque Kaleido : PC commun à toutes tes sauvegardes" @click="goTo('bank')"><Icon name="bank" :size="15" /> Banque</button>
    </div>
    <p v-if="folder" class="sv-help">Dossier : {{ folder }}</p>

    <div class="scroll">
      <p v-if="loading" class="sv-help">Lecture des sauvegardes…</p>
      <div v-else-if="!saves.length" class="empty">
        <Icon name="save" :size="44" />
        <h2>Aucune sauvegarde pour l'instant</h2>
        <p>Range tes sauvegardes dans un dossier et choisis-le ici : Kaleido les affichera toutes, avec ton équipe et ta progression. Tu peux aussi en ouvrir une directement.</p>
        <div class="sv-row">
          <button class="sv-btn solid" @click="chooseFolder"><Icon name="folder" :size="15" /> Choisir un dossier</button>
          <button class="sv-btn" @click="emit('open')"><Icon name="folder-open" :size="15" /> Ouvrir un fichier</button>
        </div>
        <p class="sv-help">Formats : .sav / .dsv (DS, émulateurs), main (3DS, extrait avec Checkpoint ou JKSM).</p>
      </div>

      <section v-for="g in groups" :key="g.title">
        <h3 class="sv-label">{{ g.title }} · {{ g.list.length }}</h3>
        <div class="cards">
          <button
            v-for="s in g.list"
            :key="s.path"
            class="card"
            :class="{ sel: selected === s.path, open: isOpen(s) }"
            @click="selected = s.path"
            @dblclick="go(s)"
          >
            <span class="cover" :style="{ background: `linear-gradient(135deg, ${cover(s.version)[0]} 0 50%, ${cover(s.version)[1]} 50%)` }">
              {{ cover(s.version)[2] }}
            </span>
            <div class="info">
              <div class="title">
                <strong>{{ s.game.replace("Pokémon ", "") }}</strong>
                <span v-if="isOpen(s)" class="sv-chip on">Ouverte</span>
                <span class="sv-chip dim">Gen {{ s.generation }}</span>
                <span v-if="emulatorOf[s.path]" class="sv-chip accent" :title="`Trouvée automatiquement chez ${emulatorOf[s.path]} : ${s.path}`">{{ emulatorOf[s.path] }}</span>
              </div>
              <div class="who">
                <span :class="s.trainer.gender">{{ s.trainer.gender === "female" ? "♀" : "♂" }}</span>
                <strong>{{ s.trainer.name }}</strong>
                <span class="dim">ID {{ String(s.trainer.displayId).padStart(s.generation >= 7 ? 6 : 5, "0") }}</span>
              </div>
              <div class="facts">
                <span><Icon name="clock" :size="13" /> {{ s.trainer.playTime.hours }} h {{ pad(s.trainer.playTime.minutes) }}</span>
                <span>₽ {{ s.trainer.money.toLocaleString("fr-FR") }}</span>
                <span><Icon name="ball" :size="13" /> {{ s.caught }} capturés</span>
                <span><Icon name="box" :size="13" /> {{ s.stored }} en boîte</span>
              </div>
              <div class="party">
                <Sprite v-for="([sp, sh], i) in s.party" :key="i" :id="sp" :shiny="sh" :size="34" />
              </div>
              <small class="dim file">{{ s.fileName }}<template v-if="s.modified"> · modifiée le {{ date(s.modified) }}</template></small>
            </div>
          </button>
        </div>
      </section>

      <p v-if="failed.length" class="sv-help warn">
        {{ failed.length }} fichier(s) non reconnu(s) : {{ failed.map((f) => f.split(/[\\/]/).pop()).join(", ") }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.manager {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  padding: 18px 20px;
  overflow: hidden;
}

.bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.search {
  width: 260px;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chips .sv-chip {
  padding: 5px 13px;
}

.count {
  margin-left: auto;
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.scroll {
  flex: 1;
  min-height: 0;
  /* Marge intérieure compensée : l'anneau de focus des cartes n'est jamais rogné par le défilement. */
  margin: -6px;
  padding: 6px;
  overflow-y: auto;
}

section h3 {
  margin: 14px 0 10px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(440px, 1fr));
  gap: 14px;
}

.card {
  display: flex;
  gap: 16px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-panel);
  background: color-mix(in srgb, var(--text) 7%, transparent);
  text-align: left;
  transition: background 0.15s, border-color 0.15s;
}

.card:hover:not(.sel) {
  border-color: color-mix(in srgb, var(--text) 35%, transparent);
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

/* Sélection : un seul contour de 2 px, tracé à l'intérieur de la carte (jamais rogné). */
.card.sel {
  border-color: var(--text);
  background: color-mix(in srgb, var(--text) 14%, transparent);
  box-shadow: inset 0 0 0 1px var(--text);
}

.cover {
  display: grid;
  flex-shrink: 0;
  place-items: center;
  width: 92px;
  height: 92px;
  border: 3px solid #fff;
  border-radius: var(--radius-card);
  color: #fff;
  font: 800 20px var(--font-display);
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  box-shadow: 0 6px 14px rgba(0, 0, 0, 0.25);
}

.info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 8px;
}

.title strong {
  font-size: var(--fs-lg);
}

.who {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
}

.who .male {
  color: var(--male);
}

.who .female {
  color: var(--female);
}

.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 14px;
  font-size: 13px;
}

.facts span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.party {
  display: flex;
  gap: 2px;
  min-height: 28px;
}

.dim {
  color: var(--text-dim);
}

.file {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-width: 560px;
  margin: 40px auto;
  text-align: center;
}

.empty p {
  margin: 0;
  color: var(--text-dim);
}

.warn {
  margin-top: 14px;
  color: var(--warn);
}
</style>
