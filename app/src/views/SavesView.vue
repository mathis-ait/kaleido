<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Banner from "../components/Banner.vue";
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import HomePage from "../save/HomePage.vue";
import BoxesPage from "../save/BoxesPage.vue";
import PokemonPage from "../save/PokemonPage.vue";
import ToolsPage from "../save/ToolsPage.vue";
import ManagerPage from "../save/ManagerPage.vue";
import LiveSyncBadge from "../play/LiveSyncBadge.vue";
import SyncBanner from "../play/SyncBanner.vue";
import NuzlockePage from "../save/NuzlockePage.vue";
import BankPage from "../save/BankPage.vue";
import GiftsPage from "../save/gifts/GiftsPage.vue";
import BattlePage from "../save/BattlePage.vue";
import EncountersPage from "../save/EncountersPage.vue";
import { goTo, history, openSave, SAVE_PAGES, saveState, writeSave, type SavePage } from "../saveStore";
import { keyOf, shell, typing } from "../save/shell";

const view = computed(() => saveState.view);
const pageIndex = computed(() => SAVE_PAGES.findIndex((p) => p.id === saveState.page));
const title = computed(() => {
  if (saveState.page === "home") return "Accueil";
  return SAVE_PAGES[pageIndex.value]?.label ?? "";
});

// L'onglet affiché reste visible dans la barre, même quand elle défile.
const tabs = ref<HTMLElement | null>(null);
watch(
  () => saveState.page,
  () => nextTick(() => tabs.value?.querySelector(".on")?.scrollIntoView({ block: "nearest", inline: "nearest" })),
);

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

function isKeyboardFocused(t: EventTarget | null) {
  return t instanceof HTMLElement && t !== document.body && t.matches(":focus-visible");
}

function onKey(e: KeyboardEvent) {
  if (e.defaultPrevented) return;
  const k = keyOf(e);
  const inField = typing(e);
  // Raccourcis de la page affichée d'abord (ceux à une touche sont ignorés pendant la saisie).
  const action = shell.actions.find((a) => a.key === k && !a.disabled);
  // Entrée / Espace sur un bouton choisi au clavier ou à la manette : c'est ce bouton qui s'active.
  const onControl = (k === "Enter" || k === " ") && isKeyboardFocused(e.target);
  if (action && !onControl && (!inField || k.startsWith("Ctrl+"))) {
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

// ---- Actions du bas : jamais de libellé tronqué. Quand la moitié droite de la barre ne suffit pas,
// les dernières actions (les moins importantes) ne gardent que leur touche, libellé en infobulle.
const actionsBox = ref<HTMLElement | null>(null);
const ruler = ref<HTMLElement | null>(null);
/** Nombre d'actions, en partant de la fin, réduites à leur touche. */
const bare = ref(0);
const isBare = (i: number) => i >= shell.actions.length - bare.value;

function fitActions() {
  const box = actionsBox.value;
  const r = ruler.value;
  if (!box || !r) return;
  // La règle cachée affiche toutes les actions en entier : largeurs naturelles, sans dépendre de l'état affiché.
  const acts = Array.from(r.children) as HTMLElement[];
  const gap = parseFloat(getComputedStyle(box).columnGap) || 0;
  const full = acts.map((a) => a.getBoundingClientRect().width);
  const keyOnly = acts.map((a) => {
    const cs = getComputedStyle(a);
    return a.querySelector("kbd")!.getBoundingClientRect().width + parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
  });
  let need = full.reduce((s, w) => s + w, 0) + gap * Math.max(0, acts.length - 1);
  let n = 0;
  while (n < acts.length && need > box.clientWidth + 0.5) {
    const i = acts.length - 1 - n;
    need -= full[i] - keyOnly[i];
    n++;
  }
  bare.value = n;
}

let fitObserver: ResizeObserver | undefined;
onMounted(() => {
  fitObserver = new ResizeObserver(() => fitActions());
  if (actionsBox.value) fitObserver.observe(actionsBox.value);
  document.fonts?.ready.then(fitActions);
});
onBeforeUnmount(() => fitObserver?.disconnect());
watch(
  () => shell.actions.map((a) => `${a.cap ?? a.key}\u0000${a.label}`).join("\u0001"),
  () => nextTick(fitActions),
  { immediate: true },
);

const time = computed(() => now.value.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));
const pages: Record<SavePage, unknown> = { home: HomePage, boxes: BoxesPage, pokemon: PokemonPage, encounters: EncountersPage, tools: ToolsPage, gifts: GiftsPage, nuzlocke: NuzlockePage, manager: ManagerPage, bank: BankPage, battle: BattlePage };
</script>

<template>
  <section class="save-app">
    <!-- Barre du haut : titre, onglets (Q / E), état de la sauvegarde, horloge -->
    <header v-if="saveState.page !== 'home'" class="topbar">
      <div class="band">
        <h1>{{ title }}</h1>
        <template v-if="view">
          <kbd class="cap" title="Page précédente (Q)" @click="step(-1)">Q</kbd>
          <nav ref="tabs" class="tabs" aria-label="Pages de l'éditeur">
            <button
              v-for="(p, i) in SAVE_PAGES"
              :key="p.id"
              :class="{ on: saveState.page === p.id }"
              :title="`${p.label} (Ctrl+${i + 1})`"
              @click="goTo(p.id)"
            >
              <Icon :name="p.icon" :size="15" />
              <span class="lbl">{{ p.label }}</span>
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
      <Banner v-if="saveState.error" :dismiss="() => (saveState.error = null)">{{ saveState.error }}</Banner>
      <SyncBanner />
      <Banner v-if="view?.needsResign && saveState.page !== 'home'" tone="warn">
        Soleil / Lune : la sauvegarde modifiée devra être re-signée (par exemple avec PKHeX) avant d'être chargée sur console.
        <Tip term="memecrypto" />
      </Banner>
    </div>

    <main class="page" :class="`page-${saveState.page}`">
      <Transition name="page" mode="out-in">
        <component :is="pages[saveState.page]" :key="saveState.page" @open="pickSave" @save-as="saveAs" />
      </Transition>
    </main>


    <Transition name="toast">
      <div v-if="saveState.notice" class="toast" role="status"><Icon name="check" :size="16" /> {{ saveState.notice }}</div>
    </Transition>

    <!-- Barre du bas : retour, aide, accueil, actions de la page -->
    <footer class="bottombar">
      <div class="left">
        <button v-if="saveState.page !== 'home' && (view || saveState.page !== 'manager')" class="act" @click="back">
          <kbd>Échap</kbd> <span class="lbl">Retour</span>
        </button>
        <span class="hint">{{ shell.hint }}</span>
      </div>
      <button v-if="view" class="home-btn" :class="{ on: saveState.page === 'home' }" title="Accueil (Ctrl+H)" @click="goTo('home')">
        <Icon name="home" :size="22" />
      </button>
      <div ref="actionsBox" class="right">
        <button
          v-for="(a, i) in shell.actions"
          :key="a.key + a.label"
          class="act"
          :disabled="a.disabled"
          :title="isBare(i) ? `${a.label} (${a.cap ?? a.key})` : undefined"
          :aria-label="isBare(i) ? a.label : undefined"
          @click="a.run()"
        >
          <kbd>{{ a.cap ?? a.key }}</kbd> <span v-if="!isBare(i)" class="lbl">{{ a.label }}</span>
        </button>
      </div>
      <!-- Règle invisible : les actions en entier, pour savoir ce qui tient -->
      <div ref="ruler" class="ruler" aria-hidden="true">
        <span v-for="a in shell.actions" :key="a.key + a.label" class="act">
          <kbd>{{ a.cap ?? a.key }}</kbd> <span class="lbl">{{ a.label }}</span>
        </span>
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

.tabs button:hover:not(.on) {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

/* Fenêtre trop étroite pour neuf libellés : icônes seules, sauf l'onglet affiché (libellé en infobulle). */
@media (max-width: 1640px) {
  .tabs button:not(.on) .lbl {
    display: none;
  }

  .tabs {
    gap: 2px;
  }

  .tabs button:not(.on) {
    padding: 7px 8px;
  }
}

.tabs button.on {
  background: var(--text);
  color: var(--bg);
}

/* Focus sur l'onglet affiché : anneau à l'intérieur de la pastille, pas de second contour. */
.tabs button.on:focus-visible {
  outline-color: var(--bg);
  outline-offset: -4px;
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

/* Racine de <SyncBanner> (« banner sync ») : la mise en page vient d'ici, ses couleurs de son propre fichier.
   Les autres bandeaux passent par <Banner> (.sv-banner). */
.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  font-size: 13px;
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

/* Les actions se tassent vers la droite, jamais sous le bouton Accueil. Elles ne rétrécissent pas :
   quand la place manque, fitActions() réduit les dernières à leur touche. */
.right {
  flex-wrap: nowrap;
  overflow: hidden;
}

.right > :first-child {
  margin-left: auto;
}

/* « Retour » et les actions ne se tassent jamais : c'est l'aide qui se coupe. */
.left > .act,
.right > .act {
  flex-shrink: 0;
}

.ruler {
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  gap: 16px;
  visibility: hidden;
  pointer-events: none;
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
  flex-shrink: 0;
  border-radius: var(--radius-pill);
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
