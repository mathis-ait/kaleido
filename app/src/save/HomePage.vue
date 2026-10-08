<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref } from "vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import { nav } from "../nav";
import { closeSave, editPokemon, goTo, history, saveState, writeSave } from "../saveStore";
import { useShell } from "./shell";
import LiveSyncBadge from "../play/LiveSyncBadge.vue";
import { playOpenSave, sendToGame } from "../play/play";
import { bankState, loadBankInfo } from "../bankStore";

const emit = defineEmits<{ open: []; "save-as": [] }>();

const view = computed(() => saveState.view!);
const lead = computed(() => saveState.selected ?? view.value.party[0] ?? null);
const stored = computed(() => view.value.boxFill.reduce((a, b) => a + b, 0));
const boxPreview = computed(() => saveState.slots.filter((s) => !!s).slice(0, 6));
/** Six cases toujours affichées, comme une rangée de boîte : les vides restent en creux. */
const boxCells = computed(() => Array.from({ length: 6 }, (_, i) => boxPreview.value[i] ?? null));
const t = computed(() => view.value.trainer);

interface Tile {
  id: string;
  title: string;
  sub: string;
  band: string;
  icon?: string;
  gradient: string;
  go?: () => void;
  soon?: boolean;
}

const tiles = computed<Tile[]>(() => [
  {
    id: "pokemon",
    title: "Pokémon",
    sub: lead.value ? `${lead.value.nickname || lead.value.speciesName} · N. ${lead.value.level}` : "Aucun Pokémon sélectionné",
    band: "Modifier le Pokémon sélectionné : identité, statistiques, attaques, rencontre",
    gradient: "linear-gradient(150deg, #ff9a5c, #ff5c8a 55%, #d63d9a)",
    go: () => (lead.value ? editPokemon(lead.value) : goTo("boxes")),
  },
  {
    id: "boxes",
    title: "Boîtes",
    sub: `${stored.value} Pokémon stockés · ${view.value.boxNames.length} boîtes`,
    band: "Ranger, copier, importer et exporter tes Pokémon",
    gradient: "linear-gradient(150deg, #5ad1ff, #2f7fe6 60%, #2457c9)",
    go: () => goTo("boxes"),
  },
  {
    id: "trainer",
    title: "Dresseur",
    sub: `${t.value.name} · ${t.value.money.toLocaleString("fr-FR")} ₽`,
    band: "Nom, ID, argent et temps de jeu",
    icon: "user",
    gradient: "linear-gradient(150deg, #b18cff, #7b5cf0 55%, #5b3fd1)",
    go: () => goTo("tools", "trainer"),
  },
  {
    id: "items",
    title: "Sac",
    sub: "Objets, CT, Baies, Poké Balls…",
    band: "Donner ou retirer des objets, poche par poche",
    icon: "bag",
    gradient: "linear-gradient(150deg, #ffd45c, #ffa23d 55%, #f2733a)",
    go: () => goTo("tools", "items"),
  },
  {
    id: "dex",
    title: "Pokédex",
    sub: "Vus et capturés",
    band: "Compléter ou corriger le Pokédex",
    icon: "book",
    gradient: "linear-gradient(150deg, #7cf0a4, #2fc27a 55%, #179a63)",
    go: () => goTo("tools", "dex"),
  },
  {
    id: "nuzlocke",
    title: "Nuzlocke",
    sub: "Routes, morts, niveau maximum",
    band: "Suivre un défi Nuzlocke avec la ROM (randomisée) de ta partie",
    icon: "swords",
    gradient: "linear-gradient(150deg, #ff7b7b, #c2365a 55%, #7a1f4a)",
    go: () => goTo("nuzlocke"),
  },
  {
    id: "battle",
    title: "Combat",
    sub: "Préparer un combat contre un dresseur",
    band: "Dégâts, K.O. et Vitesse de ton équipe contre les dresseurs de la ROM (randomisée ou non)",
    icon: "swords",
    gradient: "linear-gradient(150deg, #ff7a6b, #e0457b 55%, #9d2f8f)",
    go: () => goTo("battle"),
  },
  {
    id: "manager",
    title: "Sauvegardes",
    sub: "Toutes tes parties",
    band: "Passer d'une sauvegarde à l'autre",
    icon: "folder",
    gradient: "linear-gradient(150deg, #e9f3ff, #b9d4f5 60%, #92b5e6)",
    go: () => goTo("manager"),
  },
  {
    id: "open",
    title: "Ouvrir",
    sub: "Une autre sauvegarde (Ctrl+O)",
    band: "Ouvrir un fichier de sauvegarde",
    icon: "folder-open",
    gradient: "linear-gradient(150deg, #8fa3c0, #5f7396 60%, #46587a)",
    go: () => emit("open"),
  },
  {
    id: "encounters",
    title: "Rencontres",
    sub: "Où trouver chaque Pokémon",
    band: "Hautes herbes, dons, échanges, œufs… et créer un Pokémon légal",
    icon: "map",
    gradient: "linear-gradient(150deg, #5fe0c2, #22a59a)",
    go: () => goTo("encounters"),
  },
  {
    id: "gifts",
    title: "Cadeaux mystère",
    sub: "Distributions Gen 4 à 7",
    band: "Recevoir les Pokémon et objets des événements officiels, ouvrir ou enregistrer des cartes cadeau",
    icon: "gift",
    gradient: "linear-gradient(150deg, #ffb36b, #f0703a)",
    go: () => goTo("gifts"),
  },
  {
    id: "bank",
    title: "Banque",
    sub: bankState.info ? `${bankState.info.count} Pokémon à l'abri` : "PC commun à tes sauvegardes",
    band: "Ranger tes Pokémon hors des sauvegardes et les transférer vers un jeu plus récent",
    icon: "bank",
    gradient: "linear-gradient(150deg, #9db4ff, #5a6fe0)",
    go: () => goTo("bank"),
  },
]);

const focus = ref(0);
const strip = ref<HTMLElement | null>(null);
const focused = computed(() => tiles.value[focus.value]);

function scrollToFocus() {
  // Défilement horizontal de la bande seulement (scrollIntoView ferait défiler toute la page).
  nextTick(() => {
    const s = strip.value;
    const el = s?.querySelector<HTMLElement>(`[data-i="${focus.value}"]`);
    if (!s || !el) return;
    const left = el.offsetLeft - 48;
    const right = el.offsetLeft + el.offsetWidth + 48 - s.clientWidth;
    if (s.scrollLeft > left) s.scrollTo({ left, behavior: "smooth" });
    else if (s.scrollLeft < right) s.scrollTo({ left: right, behavior: "smooth" });
  });
}

function move(d: number) {
  focus.value = Math.min(tiles.value.length - 1, Math.max(0, focus.value + d));
  scrollToFocus();
}

function activate(i = focus.value) {
  focus.value = i;
  const tile = tiles.value[i];
  if (!tile.soon) tile.go?.();
}

/*
 * Glisser la bande à la souris (le tactile défile nativement), avec un peu d'élan au lâcher.
 * Au-delà de 6 px, le geste devient un glissement et le clic sur la tuile est annulé.
 */
const dragging = ref(false);
let drag: { x: number; left: number; moved: boolean; lastX: number; lastT: number; v: number } | null = null;

function onPointerDown(e: PointerEvent) {
  if (e.pointerType !== "mouse" || e.button !== 0 || !strip.value) return;
  drag = { x: e.clientX, left: strip.value.scrollLeft, moved: false, lastX: e.clientX, lastT: e.timeStamp, v: 0 };
}

function onPointerMove(e: PointerEvent) {
  const s = strip.value;
  if (!drag || !s) return;
  const dx = e.clientX - drag.x;
  if (!drag.moved) {
    if (Math.abs(dx) < 6) return;
    drag.moved = true;
    dragging.value = true;
    s.setPointerCapture(e.pointerId);
  }
  s.scrollLeft = drag.left - dx;
  const dt = e.timeStamp - drag.lastT;
  if (dt > 0) drag.v = (e.clientX - drag.lastX) / dt;
  drag.lastX = e.clientX;
  drag.lastT = e.timeStamp;
}

function onPointerUp(e: PointerEvent) {
  const s = strip.value;
  if (!drag || !s) return;
  if (drag.moved) {
    s.releasePointerCapture?.(e.pointerId);
    // Élan : vitesse du dernier mouvement (px/ms) prolongée ; l'aimantation choisit ensuite la tuile.
    const fling = Math.max(-900, Math.min(900, -drag.v * 280));
    dragging.value = false;
    s.scrollBy({ left: fling, behavior: "smooth" });
  }
  drag = null;
}

// Un glissement ne doit pas ouvrir la tuile sur laquelle il s'est terminé.
function onClickCapture(e: MouseEvent) {
  if (!suppressClick) return;
  suppressClick = false;
  e.stopPropagation();
  e.preventDefault();
}
let suppressClick = false;
function onPointerUpCapture() {
  suppressClick = !!drag?.moved;
}

// Molette verticale → défilement horizontal de la bande.
function onWheel(e: WheelEvent) {
  const s = strip.value;
  if (!s || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
  e.preventDefault();
  s.scrollBy({ left: e.deltaY * (e.deltaMode === 1 ? 40 : 1.2), behavior: "smooth" });
}

/*
 * Fin de défilement (souris, molette, tactile) : si la tuile choisie est sortie de l'écran,
 * la sélection passe sur la tuile entièrement visible la plus proche, pour qu'Entrée et
 * les flèches repartent de ce qu'on voit.
 */
let scrollEnd = 0;
/** Tuiles cachées de chaque côté : les flèches n'apparaissent que s'il reste quelque chose à voir. */
const edges = ref({ start: false, end: false });
function updateEdges() {
  const s = strip.value;
  if (!s) return;
  edges.value = { start: s.scrollLeft > 4, end: s.scrollLeft + s.clientWidth < s.scrollWidth - 4 };
}

// Flèches : une page de tuiles à la fois, la sélection suit à la fin du défilement.
function page(dir: number) {
  const s = strip.value;
  if (s) s.scrollBy({ left: dir * s.clientWidth * 0.8, behavior: "smooth" });
}

function onScroll() {
  updateEdges();
  clearTimeout(scrollEnd);
  scrollEnd = window.setTimeout(syncFocusToView, 140);
}

function syncFocusToView() {
  const s = strip.value;
  if (!s || dragging.value) return;
  const view = s.getBoundingClientRect();
  const visible = (el: HTMLElement) => {
    const b = el.getBoundingClientRect();
    return b.left >= view.left + 8 && b.right <= view.right - 8;
  };
  const els = [...s.querySelectorAll<HTMLElement>(".tile")];
  if (!els[focus.value] || visible(els[focus.value])) return;
  const shown = els.map((el, i) => (visible(el) ? i : -1)).filter((i) => i >= 0);
  if (!shown.length) return;
  focus.value = shown.reduce((best, i) => (Math.abs(i - focus.value) < Math.abs(best - focus.value) ? i : best));
}

function hoverTile(i: number) {
  if (!dragging.value) focus.value = i;
}

function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement)?.tagName === "INPUT") return;
  if (e.key === "ArrowRight") move(1);
  else if (e.key === "ArrowLeft") move(-1);
  else return;
  e.preventDefault();
}

let resize: ResizeObserver | undefined;
onMounted(() => {
  window.addEventListener("keydown", onKey);
  resize = new ResizeObserver(updateEdges);
  if (strip.value) resize.observe(strip.value);
  loadBankInfo();
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  clearTimeout(scrollEnd);
  resize?.disconnect();
});

useShell(() => ({
  hint: focused.value.band,
  actions: [
    { key: "Ctrl+s", cap: "Ctrl+S", label: "Enregistrer", run: () => writeSave(), disabled: !saveState.dirty },
    { key: "Enter", cap: "Entrée", label: focused.value.soon ? "Bientôt" : "Ouvrir", run: () => activate(), disabled: focused.value.soon },
  ],
}));

const time = ref(new Date().toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));

// Icônes monochromes : des teintes en dur seraient illisibles sur le fond clair du thème Jour.
const dock: { icon: string; label: string; run: () => void }[] = [
  { icon: "folder-open", label: "Ouvrir une sauvegarde (Ctrl+O)", run: () => emit("open") },
  { icon: "save", label: "Enregistrer (Ctrl+S)", run: () => writeSave() },
  { icon: "upload", label: "Enregistrer sous…", run: () => emit("save-as") },
  { icon: "play", label: "Jouer : lancer le jeu avec cette sauvegarde", run: () => playOpenSave() },
  { icon: "send", label: "Envoyer au jeu (ferme le jeu avant)", run: () => sendToGame() },
  { icon: "grid", label: "Boîtes (Ctrl+1)", run: () => goTo("boxes") },
  { icon: "undo", label: "Annuler (Ctrl+Z)", run: () => history(false) },
  { icon: "settings", label: "Apparence de Kaleido", run: () => (nav.view = "settings") },
  { icon: "power", label: "Fermer la sauvegarde", run: () => closeSave() },
];

</script>

<template>
  <div class="home">
    <!-- Équipe en haut, comme les icônes de profil d'une console -->
    <div class="top">
      <div class="bubbles">
        <button class="bubble trainer" :title="`${t.name} — carte de dresseur`" @click="goTo('tools', 'trainer')">
          {{ t.name.slice(0, 1).toUpperCase() || "?" }}
        </button>
        <button
          v-for="p in view.party"
          :key="JSON.stringify(p.slot)"
          class="bubble"
          :class="{ on: lead && JSON.stringify(lead.slot) === JSON.stringify(p.slot) }"
          :title="`${p.nickname || p.speciesName} · N. ${p.level}`"
          @click="editPokemon(p)"
        >
          <Sprite :id="p.species" :shiny="p.shiny" :size="54" />
        </button>
      </div>
      <div class="corner">
        <LiveSyncBadge :send="false" />
        <span class="game">{{ view.game }}</span>
        <Icon name="save" :size="20" :class="{ dirty: saveState.dirty }" />
        <span class="clock">{{ time }}</span>
      </div>
    </div>

    <div class="band">
      <h2>{{ focused.title }}</h2>
      <span>{{ focused.sub }}</span>
    </div>

    <!-- Tuiles -->
    <div class="strip-wrap">
      <button v-if="edges.start" class="arrow left" aria-label="Tuiles précédentes" @click="page(-1)"><Icon name="chevron-left" /></button>
      <div
        ref="strip"
        class="strip"
        :class="{ dragging }"
        @pointerdown="onPointerDown"
        @pointermove="onPointerMove"
        @pointerup.capture="onPointerUpCapture"
        @pointerup="onPointerUp"
        @pointercancel="onPointerUp"
        @click.capture="onClickCapture"
        @wheel="onWheel"
        @scroll.passive="onScroll"
        @dragstart.prevent
      >
        <button
          v-for="(tile, i) in tiles"
          :key="tile.id"
          :data-i="i"
          class="tile"
          :class="{ focus: i === focus, soon: tile.soon, [`t-${tile.id}`]: true }"
          :style="{ background: tile.gradient }"
          @mouseenter="hoverTile(i)"
          @focus="focus = i"
          @click="activate(i)"
        >
          <div class="art">
            <template v-if="tile.id === 'pokemon' && lead">
              <Sprite :id="lead.species" :shiny="lead.shiny" :form="lead.form" :gender="lead.gender" variant="model" :size="150" />
            </template>
            <div v-else-if="tile.id === 'boxes'" class="mini">
              <span v-for="(p, i) in boxCells" :key="p ? JSON.stringify(p.slot) : `empty-${i}`" :class="{ empty: !p }">
                <Sprite v-if="p" :id="p.species" :shiny="p.shiny" :size="56" />
              </span>
            </div>
            <Icon v-else :name="tile.icon ?? 'info'" :size="64" />
          </div>
          <div class="label">
            <strong>{{ tile.title }}</strong>
            <small>{{ tile.sub }}</small>
          </div>
          <span v-if="tile.soon" class="soon-chip">Bientôt</span>
        </button>
      </div>
      <button v-if="edges.end" class="arrow right" aria-label="Tuiles suivantes" @click="page(1)"><Icon name="chevron-right" /></button>
    </div>

    <!-- Dock d'actions rapides -->
    <div class="dock">
      <button v-for="d in dock" :key="d.label" class="dock-btn" :title="d.label" :aria-label="d.label" @click="d.run">
        <Icon :name="d.icon" :size="24" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 26px 40px 18px;
}

.bubbles {
  display: flex;
  gap: 10px;
}

.bubble {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  padding: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--surface);
  color: var(--text);
  transition: transform 0.15s;
}

.bubble:hover {
  transform: translateY(-2px);
}

/* Pokémon sélectionné : un seul contour net, couleur du texte. */
.bubble.on {
  outline: 3px solid var(--text);
  outline-offset: 3px;
}

.bubble.trainer {
  font: 700 26px var(--font-display);
}

.corner {
  display: flex;
  align-items: center;
  gap: 14px;
}

.game {
  color: var(--text-dim);
  font-size: 13px;
}

.corner .dirty {
  color: var(--warn);
}

.clock {
  font-size: 26px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
}

/* Titre de la tuile choisie : simple ligne de texte, sans bandeau. */
.band {
  display: flex;
  align-items: baseline;
  gap: var(--sp-4);
  min-width: 0;
  padding: var(--sp-2) 48px 0;
}

.band h2 {
  margin: 0;
  font: 600 24px var(--font-display);
  color: var(--text);
  white-space: nowrap;
}

.band span {
  overflow: hidden;
  color: var(--text-dim);
  font-size: var(--fs-base);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.strip-wrap {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  min-height: 0;
}

.strip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 22px;
  width: 100%;
  padding: 30px 48px;
  overflow-x: auto;
  scroll-snap-type: x proximity;
  scroll-padding-inline: 48px;
  scrollbar-width: none;
  cursor: grab;
  /* Les tuiles qui sortent de la bande s'estompent au lieu d'être coupées net. */
  mask-image: linear-gradient(to right, transparent, #000 40px, #000 calc(100% - 40px), transparent);
}

.strip::-webkit-scrollbar {
  display: none;
}

/* Pendant le glissement : la bande suit la souris, sans aimantation ni effet de survol. */
.strip.dragging {
  scroll-snap-type: none;
  cursor: grabbing;
}

.strip.dragging .tile {
  pointer-events: none;
}

.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  flex: 0 0 272px;
  height: 272px;
  padding: 18px 20px;
  border: none;
  border-radius: var(--radius-xs);
  color: #fff;
  text-align: left;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.22);
  scroll-snap-align: start;
  cursor: inherit;
  user-select: none;
  transition: transform 0.18s, filter 0.18s;
}

.tile :deep(img) {
  -webkit-user-drag: none;
  pointer-events: none;
}

/* Tuile choisie : un seul contour net (couleur du texte), comme sur console.
   Le padding de la bande (30 px) laisse la place au contour et à l'agrandissement. */
.tile.focus {
  transform: scale(1.03);
  outline: 3px solid var(--text);
  outline-offset: 4px;
}

.tile.soon {
  filter: saturate(0.55);
  cursor: default;
}

.t-manager {
  color: #1c3a66;
}

.art {
  position: absolute;
  inset: 16px 16px 72px;
  display: grid;
  place-items: center;
}

.art :deep(.icon) {
  stroke-width: 1.5;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.2));
}

.mini {
  display: grid;
  grid-template-columns: repeat(3, 64px);
  gap: 8px;
}

.mini span {
  display: grid;
  place-items: center;
  height: 64px;
  border-radius: var(--radius-xs);
  background: rgba(255, 255, 255, 0.18);
}

.mini span.empty {
  background: rgba(255, 255, 255, 0.08);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2);
}

.label strong {
  display: block;
  font-size: var(--fs-xl);
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
}

.label small {
  display: block;
  margin-top: 2px;
  font-size: var(--fs-md);
  opacity: 0.92;
}

.soon-chip {
  position: absolute;
  top: 12px;
  right: 12px;
  padding: 3px 10px;
  border-radius: var(--radius-pill);
  background: rgba(0, 0, 0, 0.25);
  font-size: var(--fs-xs);
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.arrow {
  position: absolute;
  z-index: 2;
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow);
}

.arrow:hover {
  background: var(--panel-hover);
}

.arrow.left {
  left: 10px;
}

.arrow.right {
  right: 10px;
}

.dock {
  display: flex;
  justify-content: center;
  gap: 22px;
  align-self: center;
  margin-bottom: 18px;
  padding: 18px 40px;
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  background: var(--panel);
  backdrop-filter: blur(10px);
}

.dock-btn {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--surface);
  color: var(--text);
  transition: transform 0.15s, background-color 0.15s;
}

.dock-btn:hover {
  transform: translateY(-2px);
  background: var(--panel-hover);
}
</style>
