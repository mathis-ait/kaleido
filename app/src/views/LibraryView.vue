<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import GameTile from "../components/GameTile.vue";
import ConsoleLogo from "../components/ConsoleLogo.vue";
import ConsoleTabs from "../components/ConsoleTabs.vue";
import Dialog from "../components/Dialog.vue";
import Launcher from "../launcher/Launcher.vue";
import { audio } from "../launcher/audio";
import Banner from "../components/Banner.vue";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import ModsDialog from "../play/ModsDialog.vue";
import DiscoverDialog from "../play/DiscoverDialog.vue";
import RomhacksDialog from "../play/RomhacksDialog.vue";
import { romhacksDialog } from "../play/romhacks";
import { modsDialog } from "../play/mods";
import { addFiles, addFolder, allGames, byRelease, games, libraryUi, loadGames, removeFolder, rescan } from "../games";
import { SHELVES, consoleOf, type ShelfFilter } from "../consoles";
import { coverUrl, formatDuration, lastPlayed, launchGame, platformOf, playTime, statusOf, timeAgo } from "../launcher/actions";
import { nav } from "../nav";
import { RECOMMENDED, available, locateEmulator, emus, formatMo, installEmulator, installs, loadEmulators, type EmulatorId, type PlayPlatform } from "../play/play";
import type { Detection, Platform } from "../types";

onMounted(() => {
  if (!games.loaded) loadGames();
  if (!emus.loaded) loadEmulators();
});

const filter = ref<ShelfFilter>("all");
const query = ref("");

/** Ordre du lanceur et de la grille : par date de sortie. */
const sorted = computed(() => [...allGames.value].sort(byRelease));

const matches = computed(() => {
  const q = query.value.trim().toLowerCase();
  return sorted.value
    .filter((g) => filter.value === "all" || g.platform === filter.value)
    .filter((g) => !q || g.title.toLowerCase().includes(q) || g.fileName.toLowerCase().includes(q));
});

/** Rayons : une section par console, dans l'ordre de sortie des consoles. */
const shelves = computed(() =>
  SHELVES.map((platform) => ({ platform, list: matches.value.filter((g) => g.platform === platform) })).filter((s) => s.list.length),
);

// --- En-tête : résumé de la bibliothèque

const totalSeconds = computed(() => allGames.value.reduce((sum, g) => sum + (playTime(statusOf(g))?.seconds ?? 0), 0));
const consoleCount = computed(() => new Set(allGames.value.map((g) => g.platform)).size);
const plural = (n: number, one: string, many: string) => `${n} ${n > 1 ? many : one}`;

/** Dernier jeu lancé depuis Kaleido, pour la carte « Reprendre ». */
const recent = computed<Detection | null>(() => {
  let best: Detection | null = null;
  for (const g of allGames.value) if (lastPlayed[g.path] && (!best || lastPlayed[g.path] > lastPlayed[best.path])) best = g;
  return best;
});
const recentTime = computed(() => (recent.value ? playTime(statusOf(recent.value)) : null));
const resuming = ref(false);

async function resume() {
  if (!recent.value || resuming.value) return;
  resuming.value = true;
  await launchGame(recent.value);
  resuming.value = false;
}

// --- Ajouter des jeux

const addOpen = ref(false);
const discovering = ref(false);
const foldersOpen = ref(false);

async function pickFolder() {
  addOpen.value = false;
  const dir = await open({ directory: true, title: "Dossier où sont rangés tes jeux" });
  if (typeof dir === "string") await addFolder(dir);
}

async function pickFiles() {
  addOpen.value = false;
  const picked = await open({
    multiple: true,
    title: "Ajouter des jeux",
    filters: [{ name: "Jeux Game Boy, GBA, DS, 3DS et Switch", extensions: ["gb", "gbc", "gba", "nds", "3ds", "cci", "cxi", "xci", "nsp", "xcz", "nsz"] }],
  });
  if (picked) await addFiles(Array.isArray(picked) ? picked : [picked]);
}

function discover() {
  addOpen.value = false;
  discovering.value = true;
}

const folderName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

// --- Émulateur de chaque rayon

const ALTERNATIVE: Partial<Record<PlayPlatform, EmulatorId>> = { nds: "desmume" };
const NAMES: Record<EmulatorId, string> = { mgba: "mGBA", melonds: "melonDS", desmume: "DeSmuME", azahar: "Azahar", citra: "Citra", lime3ds: "Lime3DS", eden: "Eden" };

function emulatorOf(platform: Platform) {
  const p = platformOf({ platform } as Detection);
  const ready = available(p);
  const recommended = RECOMMENDED[p];
  const alternative = ALTERNATIVE[p];
  const id = [recommended, alternative].find((e) => e && installs[e]);
  return { ready, recommended, alternative, installing: id ? { id, ...installs[id]! } : null };
}

function progressText(p: { step: string; done: number; total: number }) {
  if (p.step === "info") return "Recherche de la dernière version…";
  if (p.step === "extract") return "Installation…";
  return p.total ? `Téléchargement ${formatMo(p.done)} / ${formatMo(p.total)}` : "Téléchargement…";
}
</script>

<template>
  <Launcher v-if="libraryUi.mode === 'launcher' && sorted.length" :games="sorted" />
  <section v-else class="library">
    <header class="head">
      <div>
        <h1>Bibliothèque</h1>
        <p v-if="allGames.length" class="summary">
          {{ plural(allGames.length, "jeu", "jeux") }} · {{ plural(consoleCount, "console", "consoles") }}<template v-if="totalSeconds"> · {{ formatDuration(totalSeconds) }} de jeu</template>
        </p>
        <p v-else class="summary">Tes jeux, prêts à lancer, avec leurs jaquettes, leurs émulateurs et leurs mods.</p>
      </div>
      <div class="actions">
        <div class="add">
          <button class="sv-btn" :aria-expanded="addOpen" @click="addOpen = !addOpen"><Icon name="plus" :size="16" /> Ajouter</button>
          <div v-if="addOpen" class="add-menu" @mouseleave="addOpen = false">
            <button @click="discover"><Icon name="search" :size="16" /><span><strong>Rechercher sur ce PC</strong><small>Jeux et émulateurs rangés n'importe où</small></span></button>
            <button @click="pickFolder"><Icon name="folder" :size="16" /><span><strong>Ajouter un dossier</strong><small>Relu à chaque ouverture de Kaleido</small></span></button>
            <button @click="pickFiles"><Icon name="file" :size="16" /><span><strong>Ajouter des jeux</strong><small>Un ou plusieurs fichiers</small></span></button>
          </div>
        </div>
        <button class="sv-btn" title="Installer un romhack en un clic (patch officiel, version française)" @click="romhacksDialog.open = true"><Icon name="wand" :size="16" /> Romhacks</button>
        <button class="sv-round" :title="`Dossiers suivis (${games.config.folders.length})`" aria-label="Dossiers suivis" @click="foldersOpen = true"><Icon name="folder-open" :size="16" /></button>
        <button class="sv-round" :disabled="games.scanning" title="Relire les dossiers" aria-label="Relire les dossiers" @click="rescan"><Icon name="refresh" :size="16" :class="{ 'sv-spin': games.scanning }" /></button>
        <button class="sv-round" :class="{ off: !audio.music }" :title="audio.music ? 'Musique au survol : activée' : 'Musique au survol : coupée'" :aria-pressed="audio.music" @click="audio.music = !audio.music">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" />
            <circle cx="18" cy="16" r="3" />
            <path v-if="!audio.music" d="M3 3l18 18" />
          </svg>
        </button>
        <button v-if="allGames.length" class="sv-btn solid" @click="libraryUi.mode = 'launcher'"><Icon name="play" :size="15" /> Mode lanceur</button>
      </div>
    </header>

    <!-- Reprendre : le dernier jeu lancé, en grand. -->
    <article v-if="recent && !query && filter === 'all'" class="resume">
      <img v-if="coverUrl(recent)" class="resume-bg" :src="coverUrl(recent)!" alt="" aria-hidden="true" />
      <img v-if="coverUrl(recent)" class="resume-cover" :src="coverUrl(recent)!" :alt="recent.title" />
      <div class="resume-text">
        <p class="resume-label">Reprendre</p>
        <h2>{{ recent.title }}</h2>
        <p class="resume-meta">
          <ConsoleLogo :id="consoleOf(recent)" :size="13" />
          <span>Dernière partie {{ timeAgo(lastPlayed[recent.path]) }}</span>
          <span v-if="recentTime">{{ formatDuration(recentTime.seconds) }} de jeu</span>
        </p>
      </div>
      <button class="resume-play" :disabled="resuming" @click="resume">
        <Icon name="play" :size="18" /> {{ resuming ? "Lancement…" : "Jouer" }}
      </button>
    </article>

    <div v-if="allGames.length" class="toolbar">
      <ConsoleTabs v-model="filter" :games="allGames" :size="14" counts />
      <SearchField v-model="query" class="search" placeholder="Rechercher un jeu" />
    </div>

    <Banner v-if="games.error" class="error" :retry="rescan">{{ games.error }}</Banner>

    <!-- Recherche : une seule grille, la console écrite sous chaque jeu. -->
    <div v-if="query.trim() && matches.length" class="grid">
      <GameTile v-for="g in matches" :key="g.path" :game="g" show-console />
    </div>

    <template v-else-if="shelves.length">
      <section v-for="s in shelves" :key="s.platform" class="shelf">
        <header class="shelf-head">
          <ConsoleLogo :id="s.list.every((g) => consoleOf(g) === 'gbc') ? 'gbc' : s.platform" :size="20" />
          <span class="count">{{ s.list.length }}</span>
          <span class="grow" />
          <template v-for="e in [emulatorOf(s.platform)]" :key="s.platform">
            <span v-if="e.installing" class="emu">
              {{ NAMES[e.installing.id] }} · {{ progressText(e.installing) }}
              <span class="bar"><span :style="{ width: (e.installing.step === 'extract' ? 100 : e.installing.total ? Math.round((e.installing.done / e.installing.total) * 100) : 0) + '%' }" /></span>
            </span>
            <span v-else-if="e.ready.length" class="emu" :title="`Émulateur prêt : ${e.ready.map((r) => r.name).join(', ')}`">
              <span class="dot" />{{ e.ready[0].name }}<span v-if="e.ready[0].version" class="dim">&nbsp;{{ e.ready[0].version }}</span>
            </span>
            <span v-else-if="emus.loaded" class="emu-actions">
              <span class="missing">Aucun émulateur</span>
              <button class="sv-btn small solid" @click="installEmulator(e.recommended)"><Icon name="download" :size="14" /> Installer {{ NAMES[e.recommended] }}</button>
              <button class="sv-btn small" title="Choisir l'émulateur déjà présent sur ton PC" @click="locateEmulator(e.recommended, NAMES[e.recommended])">Localiser…</button>
            </span>
          </template>
        </header>
        <div class="grid">
          <GameTile v-for="g in s.list" :key="g.path" :game="g" />
        </div>
      </section>
    </template>

    <div v-else-if="games.scanning" class="empty dim">Analyse des dossiers…</div>

    <div v-else-if="!allGames.length" class="empty first">
      <div class="logos" aria-hidden="true">
        <ConsoleLogo id="gb" :size="16" />
        <ConsoleLogo id="gba" :size="16" />
        <ConsoleLogo id="nds" :size="16" />
        <ConsoleLogo id="3ds" :size="16" />
        <ConsoleLogo id="switch" :size="16" />
      </div>
      <h2>Ta bibliothèque est vide</h2>
      <p>Kaleido cherche tes jeux Game Boy, GBA, DS, 3DS et Switch sur ce PC, retrouve leurs jaquettes et installe les émulateurs qui manquent.</p>
      <div class="emu-actions">
        <button class="sv-btn solid" @click="discovering = true"><Icon name="search" :size="16" /> Rechercher mes jeux sur ce PC</button>
        <button class="sv-btn" @click="pickFolder"><Icon name="folder" :size="16" /> Ajouter un dossier</button>
      </div>
    </div>

    <p v-else class="empty dim">Aucun jeu ne correspond.</p>
  </section>

  <Dialog v-model="foldersOpen" title="Dossiers suivis" icon="folder-open" subtitle="Kaleido les relit à chaque ouverture et y retrouve tes nouveaux jeux." :width="560">
    <ul v-if="games.config.folders.length" class="folders">
      <li v-for="f in games.config.folders" :key="f">
        <Icon name="folder" :size="16" />
        <span class="folder-text"><strong>{{ folderName(f) }}</strong><small>{{ f }}</small></span>
        <button class="sv-round sq" title="Ne plus suivre ce dossier" aria-label="Ne plus suivre ce dossier" @click="removeFolder(f)"><Icon name="x" :size="14" /></button>
      </li>
    </ul>
    <p v-else class="dim">Aucun dossier suivi pour l'instant.</p>
    <template #foot>
      <button class="sv-btn" @click="nav.view = 'settings'">Réglages des émulateurs</button>
      <span class="grow" />
      <button class="sv-btn solid" @click="pickFolder"><Icon name="plus" :size="15" /> Ajouter un dossier</button>
    </template>
  </Dialog>

  <ModsDialog v-if="modsDialog.game" :key="modsDialog.game.path" />
  <DiscoverDialog v-if="discovering" @close="discovering = false" />
  <RomhacksDialog v-if="romhacksDialog.open" @close="romhacksDialog.open = false" />
</template>

<style scoped>
.library {
  max-width: 1480px;
  margin: 0 auto;
}

/* --- En-tête */

.head {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-bottom: 28px;
}

h1 {
  font-family: var(--font-display);
  font-size: 34px;
  font-weight: 700;
  letter-spacing: -0.01em;
}

.summary {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: var(--fs-lg);
  font-variant-numeric: tabular-nums;
}

.actions {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.sv-btn,
.emu-actions .sv-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.sv-round.off {
  color: var(--text-dim);
}

.add {
  position: relative;
}

.add-menu {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  z-index: 20;
  display: flex;
  flex-direction: column;
  width: 300px;
  padding: 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  box-shadow: var(--shadow-pop);
}

.add-menu button {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: 10px 12px;
  border: none;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text);
  text-align: left;
}

.add-menu button:hover {
  background: var(--panel-hover);
}

.add-menu span {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.add-menu strong {
  font-size: var(--fs-base);
  font-weight: 600;
}

.add-menu small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

/* --- Reprendre */

.resume {
  position: relative;
  display: flex;
  align-items: center;
  gap: 28px;
  min-height: 190px;
  margin-bottom: 32px;
  padding: 26px 34px;
  overflow: hidden;
  border-radius: var(--radius-panel);
  background: #10121c;
  color: #fff;
  box-shadow: var(--shadow);
  isolation: isolate;
}

.resume-bg {
  position: absolute;
  inset: -40px;
  z-index: -1;
  width: calc(100% + 80px);
  height: calc(100% + 80px);
  object-fit: cover;
  filter: blur(40px) saturate(1.3) brightness(0.45);
}

.resume-cover {
  width: 138px;
  height: 138px;
  object-fit: contain;
  filter: drop-shadow(0 14px 24px rgb(0 0 0 / 0.55));
}

.resume-text {
  flex: 1;
  min-width: 0;
}

.resume-label {
  margin: 0 0 6px;
  color: rgb(255 255 255 / 0.6);
  font-size: var(--fs-xs);
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.resume h2 {
  font-family: var(--font-display);
  font-size: 32px;
  font-weight: 800;
  letter-spacing: -0.01em;
}

.resume-meta {
  display: flex;
  align-items: center;
  gap: 14px;
  margin: 10px 0 0;
  color: rgb(255 255 255 / 0.75);
  font-size: var(--fs-md);
}

.resume-meta span::before {
  content: "";
  display: inline-block;
  width: 3px;
  height: 3px;
  margin: 0 14px 3px 0;
  border-radius: 50%;
  background: rgb(255 255 255 / 0.5);
  vertical-align: middle;
}

.resume-play {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 14px 28px;
  border: none;
  border-radius: var(--radius-pill);
  background: #fff;
  color: #0b0d18;
  font-size: var(--fs-lg);
  font-weight: 800;
  box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  transition: transform 0.15s ease;
}

.resume-play:hover:not(:disabled) {
  transform: scale(1.04);
}

/* --- Barre d'outils */

.toolbar {
  position: sticky;
  top: 0;
  z-index: 10;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: var(--sp-2) 0;
  margin-bottom: var(--sp-2);
  background: color-mix(in srgb, var(--bg) 82%, transparent);
  backdrop-filter: blur(16px);
}

.search {
  width: 280px;
}

/* --- Rayons */

.shelf {
  margin-top: 30px;
}

.shelf-head {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  min-height: 34px;
  padding-bottom: var(--sp-3);
  margin-bottom: 18px;
  border-bottom: 1px solid var(--border);
}

.count {
  color: var(--text-dim);
  font-size: var(--fs-md);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.grow {
  flex: 1;
}

.emu {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--ok);
}

.dim {
  color: var(--text-dim);
}

.missing {
  color: var(--warn);
  font-size: var(--fs-md);
  font-weight: 600;
}

.emu-actions {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
}

.bar {
  display: inline-block;
  width: 120px;
  height: 4px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--panel-hover);
}

.bar span {
  display: block;
  height: 100%;
  background: var(--text);
  transition: width 0.25s ease;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
  gap: 30px 22px;
}

/* --- États vides */

.empty {
  margin-top: 40px;
  text-align: center;
}

.empty.first {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-4);
  padding: 64px 32px;
  border-radius: var(--radius-panel);
  background: var(--panel);
  color: var(--text-dim);
}

.logos {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  align-items: center;
  gap: 28px;
  margin-bottom: var(--sp-2);
  opacity: 0.7;
}

.empty h2 {
  color: var(--text);
  font-size: 22px;
}

.empty p {
  max-width: 540px;
  margin: 0;
}

.error {
  margin-top: 14px;
}

/* --- Dossiers suivis */

.folders {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.folders li {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  color: var(--text-dim);
}

.folders li:hover {
  background: var(--panel-hover);
}

.folder-text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.folder-text strong {
  color: var(--text);
  font-size: var(--fs-base);
  font-weight: 600;
}

.folder-text small {
  overflow: hidden;
  font-size: var(--fs-sm);
  white-space: nowrap;
  text-overflow: ellipsis;
}

@media (max-width: 1100px) {
  .resume-cover {
    display: none;
  }
}
</style>
