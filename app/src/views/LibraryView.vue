<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import GameTile from "../components/GameTile.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import ModsDialog from "../play/ModsDialog.vue";
import { modsDialog } from "../play/mods";
import { addFiles, addFolder, allGames, games, loadGames, removeFolder, rescan } from "../games";
import { nav } from "../nav";
import { PLATFORM_LABEL, RECOMMENDED, available, emus, formatMo, installEmulator, installs, loadEmulators, type EmulatorId, type PlayPlatform } from "../play/play";

onMounted(() => {
  if (!games.loaded) loadGames();
  if (!emus.loaded) loadEmulators();
});

type Filter = "all" | PlayPlatform;
const filter = ref<Filter>("all");
const query = ref("");

const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  return allGames.value
    .filter((g) => filter.value === "all" || g.platform === filter.value)
    .filter((g) => !q || g.title.toLowerCase().includes(q) || g.fileName.toLowerCase().includes(q))
    .sort((a, b) => (a.generation ?? 10) - (b.generation ?? 10) || a.title.localeCompare(b.title, "fr"));
});

async function pickFolder() {
  const dir = await open({ directory: true, title: "Dossier où sont rangés tes jeux" });
  if (typeof dir === "string") await addFolder(dir);
}

async function pickFiles() {
  const picked = await open({
    multiple: true,
    title: "Ajouter des jeux",
    filters: [{ name: "Jeux DS, 3DS et Switch", extensions: ["nds", "3ds", "cci", "cxi", "xci", "nsp", "xcz", "nsz"] }],
  });
  if (picked) await addFiles(Array.isArray(picked) ? picked : [picked]);
}

const folderName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

// --- Émulateurs

const ALTERNATIVE: Partial<Record<PlayPlatform, EmulatorId>> = { nds: "desmume" };
const NAMES: Record<EmulatorId, string> = { melonds: "melonDS", desmume: "DeSmuME", azahar: "Azahar", citra: "Citra", lime3ds: "Lime3DS", eden: "Eden" };

const consoles = computed(() =>
  (["nds", "3ds", "switch"] as PlayPlatform[]).map((platform) => {
    const ready = available(platform);
    const recommended = RECOMMENDED[platform];
    const alternative = ALTERNATIVE[platform];
    const installing = [recommended, alternative].find((id) => id && installs[id]);
    return { platform, ready, recommended, alternative, installing: installing ? { id: installing, ...installs[installing]! } : null };
  }),
);

function progressText(p: { step: string; done: number; total: number }) {
  if (p.step === "info") return "Recherche de la dernière version…";
  if (p.step === "extract") return "Installation…";
  return p.total ? `Téléchargement… ${formatMo(p.done)} / ${formatMo(p.total)}` : "Téléchargement…";
}

const percent = (p: { done: number; total: number }) => (p.total ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0);
</script>

<template>
  <section class="library">
    <header>
      <div>
        <h1>Bibliothèque</h1>
        <p class="lead">Tes jeux, prêts à lancer. Kaleido retrouve leurs jaquettes, s'occupe des émulateurs et installe les mods en un clic.</p>
      </div>
      <div class="actions">
        <button class="btn btn-primary" @click="pickFolder"><Icon name="folder" :size="16" /> Ajouter un dossier</button>
        <button class="btn" @click="pickFiles"><Icon name="plus" :size="16" /> Ajouter des jeux</button>
        <button class="btn" :disabled="games.scanning" title="Relire les dossiers" @click="rescan"><Icon name="refresh" :size="16" /></button>
      </div>
    </header>

    <div class="emulators">
      <div v-for="c in consoles" :key="c.platform" class="emu panel">
        <div class="emu-head">
          <span class="console">{{ PLATFORM_LABEL[c.platform] }}</span>
          <span v-if="c.ready.length" class="ok"><Icon name="check" :size="14" /> Prêt</span>
          <span v-else-if="emus.loaded" class="missing">Aucun émulateur</span>
        </div>

        <template v-if="c.installing">
          <p class="emu-line">{{ NAMES[c.installing.id] }} : {{ progressText(c.installing) }}</p>
          <div class="bar"><div :style="{ width: (c.installing.step === 'extract' ? 100 : percent(c.installing)) + '%' }" /></div>
        </template>
        <template v-else-if="c.ready.length">
          <p class="emu-line">
            <span v-for="(e, i) in c.ready" :key="e.id">
              <strong>{{ e.name }}</strong><span v-if="e.version" class="dim">&nbsp;{{ e.version }}</span><span v-if="i < c.ready.length - 1"> · </span>
            </span>
          </p>
        </template>
        <template v-else-if="emus.loaded">
          <div class="emu-actions">
            <button class="btn btn-primary" @click="installEmulator(c.recommended)"><Icon name="download" :size="15" /> Installer {{ NAMES[c.recommended] }}</button>
            <button v-if="c.alternative" class="btn" @click="installEmulator(c.alternative)">ou {{ NAMES[c.alternative] }}</button>
          </div>
        </template>
        <p v-else class="emu-line dim">Recherche des émulateurs…</p>
      </div>
      <button class="link" @click="nav.view = 'settings'">Réglages des émulateurs</button>
    </div>

    <div v-if="games.config.folders.length" class="folders">
      <span class="dim">Dossiers suivis :</span>
      <span v-for="f in games.config.folders" :key="f" class="folder chip" :title="f">
        <Icon name="folder" :size="13" /> {{ folderName(f) }}
        <button aria-label="Ne plus suivre ce dossier" title="Ne plus suivre ce dossier" @click="removeFolder(f)">×</button>
      </span>
    </div>

    <div v-if="allGames.length" class="toolbar">
      <Segmented
        v-model="filter"
        :options="[
          { value: 'all', label: `Tous (${allGames.length})` },
          { value: 'nds', label: 'DS' },
          { value: '3ds', label: '3DS' },
          { value: 'switch', label: 'Switch' },
        ]"
      />
      <label class="search">
        <Icon name="search" :size="15" />
        <input v-model="query" type="search" placeholder="Rechercher un jeu" />
      </label>
      <span v-if="games.scanning" class="dim">Analyse des dossiers…</span>
    </div>

    <p v-if="games.error" class="error">{{ games.error }}</p>

    <div v-if="shown.length" class="grid">
      <GameTile v-for="g in shown" :key="g.path" :game="g" />
    </div>

    <div v-else-if="games.scanning" class="empty dim">Analyse des dossiers…</div>

    <div v-else-if="!allGames.length" class="empty panel">
      <Icon name="grid" :size="36" />
      <h2>Ta bibliothèque est vide</h2>
      <p>Ajoute le dossier où tu ranges tes jeux DS, 3DS et Switch : Kaleido le relira à chaque ouverture et affichera tes jeux avec leur jaquette.</p>
      <button class="btn btn-primary" @click="pickFolder"><Icon name="folder" :size="16" /> Ajouter un dossier de jeux</button>
    </div>

    <p v-else class="empty dim">Aucun jeu ne correspond.</p>

    <ModsDialog v-if="modsDialog.game" :key="modsDialog.game.path" />
  </section>
</template>

<style scoped>
.library {
  max-width: 1200px;
  margin: 0 auto;
}

header {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}

h1 {
  font-size: 34px;
  font-weight: 700;
}

.lead {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: 15px;
}

.actions,
.emu-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.actions .btn,
.emu-actions .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.emulators {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr auto;
  align-items: stretch;
  gap: 14px;
}

.emu {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
}

.emu-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.console {
  font-weight: 600;
}

.ok,
.missing {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 600;
}

.ok {
  color: var(--ok);
}

.missing {
  color: var(--warn);
}

.emu-line {
  margin: 0;
  font-size: 13px;
}

.dim {
  color: var(--text-dim);
}

.bar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-hover);
}

.bar div {
  height: 100%;
  border-radius: inherit;
  background: var(--prism);
  transition: width 0.25s ease;
}

.link {
  align-self: center;
  border: none;
  background: none;
  color: var(--text-dim);
  font-size: 13px;
  text-decoration: underline;
}

.link:hover {
  color: var(--text);
}

.folders {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 18px;
  font-size: 13px;
}

.folder {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.folder button {
  border: none;
  background: none;
  color: var(--text-dim);
  font-size: 15px;
  line-height: 1;
}

.folder button:hover {
  color: var(--danger);
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 14px;
  margin-top: 22px;
}

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  color: var(--text-dim);
}

.search input {
  width: 200px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 13px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 26px 22px;
  margin-top: 22px;
}

.empty {
  margin-top: 26px;
  text-align: center;
}

.empty.panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 48px 32px;
  border-style: dashed;
  border-width: 2px;
  color: var(--text-dim);
}

.empty h2 {
  color: var(--text);
  font-size: 20px;
}

.empty p {
  max-width: 520px;
  margin: 0;
}

.error {
  margin-top: 14px;
  color: var(--danger);
}

@media (max-width: 1100px) {
  .emulators {
    grid-template-columns: 1fr 1fr;
  }

  .link {
    grid-column: 1 / -1;
    justify-self: start;
  }
}
</style>
