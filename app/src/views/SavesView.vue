<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import "../save/form.css";
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import HomePage from "../save/HomePage.vue";
import BoxesPage from "../save/BoxesPage.vue";
import PokemonPage from "../save/PokemonPage.vue";
import ToolsPage from "../save/ToolsPage.vue";
import ManagerPage from "../save/ManagerPage.vue";
import LiveSyncBadge from "../play/LiveSyncBadge.vue";
import PlayGuide from "../play/PlayGuide.vue";
import SyncBanner from "../play/SyncBanner.vue";
import NuzlockePage from "../save/NuzlockePage.vue";
import { goTo, history, openSave, SAVE_PAGES, saveState, writeSave, type SavePage } from "../saveStore";
import { keyOf, shell, typing } from "../save/shell";

const view = computed(() => saveState.view);
const pageIndex = computed(() => SAVE_PAGES.findIndex((p) => p.id === saveState.page));
const title = computed(() => {
  if (saveState.page === "home") return "Accueil";
  return SAVE_PAGES[pageIndex.value]?.label ?? "";
});

// Horloge en haut à droite, comme sur console.
const now = ref(new Date());
let clock: number | undefined;

function step(delta: number) {
  const n = SAVE_PAGES.length;
  const i = pageIndex.value < 0 ? (delta > 0 ? -1 : 0) : pageIndex.value;
  goTo(SAVE_PAGES[(i + delta + n) % n].id);
}

async function pickSave() {
  const path = await open({
    title: "Ouvrir une sauvegarde",
    filters: [
      { name: "Sauvegardes", extensions: ["sav", "dsv", "bin", "main"] },
      { name: "Tous les fichiers", extensions: ["*"] },
    ],
  });
  if (typeof path === "string") openSave(path);
}

async function saveAs() {
  const output = await save({ title: "Enregistrer la sauvegarde sous", defaultPath: saveState.path ?? undefined });
  if (output) writeSave(output);
}

function back() {
  if (saveState.page === "tools" && saveState.tool) saveState.tool = null;
  else if (saveState.page !== "home") goTo(view.value ? "home" : "manager");
}

function onKey(e: KeyboardEvent) {
  if (e.defaultPrevented) return;
  const k = keyOf(e);
  const inField = typing(e);
  // Raccourcis de la page affichée d'abord (ceux à une touche sont ignorés pendant la saisie).
  const action = shell.actions.find((a) => a.key === k && !a.disabled);
  if (action && (!inField || k.startsWith("Ctrl+"))) {
    e.preventDefault();
    action.run();
    return;
  }
  if (k === "Ctrl+o") {
    e.preventDefault();
    pickSave();
  } else if (!view.value) {
    return;
  } else if (k === "Ctrl+z" && !inField) {
    e.preventDefault();
    history(false);
  } else if ((k === "Ctrl+y" || k === "Ctrl+Z") && !inField) {
    e.preventDefault();
    history(true);
  } else if (k === "Ctrl+s") {
    e.preventDefault();
    if (saveState.dirty) writeSave();
  } else if (k === "Ctrl+h") {
    e.preventDefault();
    goTo("home");
  } else if (/^Ctrl\+[1-9]$/.test(k) && SAVE_PAGES[Number(k.slice(-1)) - 1]) {
    e.preventDefault();
    goTo(SAVE_PAGES[Number(k.slice(-1)) - 1].id);
  } else if (inField) {
    return;
  } else if (k === "q") {
    step(-1);
  } else if (k === "e") {
    step(1);
  } else if (k === "Escape") {
    back();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKey);
  clock = window.setInterval(() => (now.value = new Date()), 10_000);
  if (!view.value) saveState.page = "manager";
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  clearInterval(clock);
});

const time = computed(() => now.value.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));
const pages: Record<SavePage, unknown> = { home: HomePage, boxes: BoxesPage, pokemon: PokemonPage, tools: ToolsPage, nuzlocke: NuzlockePage, manager: ManagerPage };
</script>

<template>
  <section class="save-app">
    <!-- Barre du haut : titre, onglets (Q / E), état de la sauvegarde, horloge -->
    <header v-if="saveState.page !== 'home'" class="topbar">
      <div class="band">
        <h1>{{ title }}</h1>
        <template v-if="view">
          <kbd class="cap" title="Page précédente (Q)" @click="step(-1)">Q</kbd>
          <nav class="tabs">
            <button
              v-for="(p, i) in SAVE_PAGES"
              :key="p.id"
              :class="{ on: saveState.page === p.id }"
              :title="`${p.label} (Ctrl+${i + 1})`"
              @click="goTo(p.id)"
            >
              <Icon :name="p.icon" :size="15" />
              {{ p.label }}
            </button>
          </nav>
          <kbd class="cap" title="Page suivante (E)" @click="step(1)">E</kbd>
        </template>
      </div>
      <div class="status">
        <template v-if="view">
          <LiveSyncBadge />
          <button class="icon-btn" :disabled="!view.canUndo" title="Annuler (Ctrl+Z)" @click="history(false)"><Icon name="undo" /></button>
          <button class="icon-btn" :disabled="!view.canRedo" title="Rétablir (Ctrl+Y)" @click="history(true)"><Icon name="redo" /></button>
          <button
            class="save-pill"
            :class="{ dirty: saveState.dirty }"
            :disabled="!saveState.dirty"
            :title="saveState.dirty ? 'Enregistrer (Ctrl+S)' : 'Aucune modification à enregistrer'"
            @click="writeSave()"
          >
            <Icon name="save" :size="16" />
            {{ saveState.dirty ? "Enregistrer" : "Enregistré" }}
          </button>
        </template>
        <span class="clock">{{ time }}</span>
      </div>
    </header>

    <!-- Bandeaux -->
    <div class="banners">
      <div v-if="saveState.error" class="banner danger" role="alert">
        <Icon name="alert" :size="16" /> {{ saveState.error }}
        <button class="x" aria-label="Fermer" @click="saveState.error = null"><Icon name="x" :size="14" /></button>
      </div>
      <SyncBanner />
      <div v-if="view?.needsResign && saveState.page !== 'home'" class="banner warn">
        <Icon name="shield-alert" :size="16" /> Soleil / Lune : la sauvegarde modifiée devra être re-signée (par exemple avec PKHeX) avant d'être
        chargée sur console. <Tip term="memecrypto" />
      </div>
    </div>

    <main class="page" :class="`page-${saveState.page}`">
      <Transition name="page" mode="out-in">
        <component :is="pages[saveState.page]" :key="saveState.page" @open="pickSave" @save-as="saveAs" />
      </Transition>
    </main>

    <PlayGuide />

    <Transition name="toast">
      <div v-if="saveState.notice" class="toast" role="status"><Icon name="check" :size="16" /> {{ saveState.notice }}</div>
    </Transition>

    <!-- Barre du bas : retour, aide, accueil, actions de la page -->
    <footer class="bottombar">
      <div class="left">
        <button v-if="saveState.page !== 'home' && (view || saveState.page !== 'manager')" class="act" @click="back">
          <kbd>Échap</kbd> Retour
        </button>
        <span class="hint">{{ shell.hint }}</span>
      </div>
      <button v-if="view" class="home-btn" :class="{ on: saveState.page === 'home' }" title="Accueil (Ctrl+H)" @click="goTo('home')">
        <Icon name="home" :size="22" />
      </button>
      <div class="right">
        <button v-for="a in shell.actions" :key="a.key + a.label" class="act" :disabled="a.disabled" @click="a.run()">
          <kbd>{{ a.cap ?? a.key }}</kbd> {{ a.label }}
        </button>
      </div>
    </footer>
  </section>
</template>

<style scoped>
.save-app {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* ---- Barre du haut ---- */
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 24px 0 0;
}

.band {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 10px 56px 10px 26px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  clip-path: polygon(0 0, 100% 0, calc(100% - 36px) 100%, 0 100%);
  backdrop-filter: blur(12px);
}

.band h1 {
  margin-right: 8px;
  font-size: 24px;
  font-weight: 300;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  white-space: nowrap;
}

.cap,
kbd {
  display: inline-grid;
  place-items: center;
  min-width: 24px;
  height: 22px;
  padding: 0 6px;
  border-radius: 6px;
  background: var(--text);
  color: var(--bg);
  font: 700 11px/1 var(--font);
}

.cap {
  cursor: pointer;
}

.tabs {
  display: flex;
  gap: 4px;
  overflow-x: auto;
  scrollbar-width: none;
}

.tabs button {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 7px 14px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-weight: 600;
  font-size: 13px;
  white-space: nowrap;
  transition: background 0.15s, color 0.15s;
}

.tabs button:hover {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.tabs button.on {
  background: var(--text);
  color: var(--bg);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 60%, transparent);
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.icon-btn {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--panel);
}

.icon-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.save-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  color: var(--text-dim);
  font-weight: 600;
  font-size: 13px;
}

.save-pill.dirty {
  background: var(--text);
  color: var(--bg);
}

.clock {
  margin-left: 8px;
  font-size: 22px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
}

/* ---- Bandeaux ---- */
.banners {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 0 24px;
}

.banners:not(:empty) {
  padding-top: 10px;
}

.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  font-size: 13px;
}

.banner.warn {
  background: var(--warn-bg);
  color: var(--warn);
}

.banner.danger {
  border: 1px solid var(--danger);
  background: color-mix(in srgb, var(--danger) 12%, transparent);
  color: var(--danger);
}

.banner .x {
  margin-left: auto;
  border: none;
  background: none;
}

/* ---- Page ---- */
.page {
  position: relative;
  flex: 1;
  min-height: 0;
  padding: 16px 24px 12px;
  overflow: hidden;
}

.page-home {
  padding: 0;
}

.page-enter-active,
.page-leave-active {
  transition: opacity 0.14s ease, transform 0.14s ease;
}

.page-enter-from {
  opacity: 0;
  transform: translateX(10px);
}

.page-leave-to {
  opacity: 0;
  transform: translateX(-6px);
}

/* ---- Barre du bas ---- */
.bottombar {
  position: relative;
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 12px;
  min-height: 64px;
  margin: 0 24px;
  border-top: 1px solid color-mix(in srgb, var(--text) 35%, transparent);
}

.left,
.right {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.right {
  justify-content: flex-end;
  flex-wrap: nowrap;
  overflow: hidden;
}

.hint {
  overflow: hidden;
  color: var(--text-dim);
  font-size: 13px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.act {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border: none;
  border-radius: 8px;
  background: none;
  font-size: 14px;
  white-space: nowrap;
}

.act kbd {
  border-radius: 999px;
}

.act:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.act:disabled {
  opacity: 0.4;
  cursor: default;
}

.home-btn {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  border: 2px solid var(--text);
  border-radius: 50%;
  background: transparent;
  transition: background 0.15s;
}

.home-btn:hover,
.home-btn.on {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

/* ---- Notification ---- */
.toast {
  position: absolute;
  bottom: 84px;
  left: 50%;
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 8px;
  max-width: 70%;
  padding: 10px 18px;
  border-radius: 999px;
  background: var(--text);
  color: var(--bg);
  font-weight: 600;
  font-size: 13px;
  box-shadow: var(--shadow);
  transform: translateX(-50%);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 10px);
}
</style>
