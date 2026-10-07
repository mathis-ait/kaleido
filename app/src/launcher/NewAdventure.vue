<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import Banner from "../components/Banner.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import type { PadAction } from "./gamepad";
import type { AdventurePreset } from "../presets";
import {
  adventure,
  applySeedText,
  choosePreset,
  closeAdventure,
  customize,
  destination,
  removePreset,
  reroll,
} from "./adventure";

/**
 * « Nouvelle aventure » : choix du preset, puis aperçu de la partie (starters, première
 * route, premier champion) avant de l'écrire et de jouer. Aucune case à cocher ici :
 * les réglages fins sont derrière « Personnaliser ».
 */
const props = defineProps<{ cover: string | null; padConnected: boolean }>();
const emit = defineEmits<{ play: [] }>();

const COLUMNS = 3;
const seedText = ref("");
const seedError = ref(false);
const copied = ref(false);
const cards = ref<HTMLElement[]>([]);
const MAX_ROUTE = 12;

const where = computed(() => destination());
const route = computed(() => adventure.preview?.firstRoute ?? null);
const shownRoute = computed(() => route.value?.encounters.slice(0, MAX_ROUTE) ?? []);
const levels = (m: { minLevel: number; maxLevel: number }) => (m.minLevel === m.maxLevel ? `N. ${m.minLevel}` : `N. ${m.minLevel}–${m.maxLevel}`);

watch(
  () => adventure.index,
  (i) => nextTick(() => cards.value[i]?.focus({ preventScroll: false })),
);
watch(
  () => adventure.step,
  (s) => {
    seedText.value = "";
    seedError.value = false;
    if (s === "preset") nextTick(() => cards.value[adventure.index]?.focus());
  },
);

async function pasteSeed() {
  if (!seedText.value.trim()) return;
  seedError.value = !(await applySeedText(seedText.value));
  if (!seedError.value) seedText.value = "";
}

async function copyCode() {
  if (!adventure.preview) return;
  await navigator.clipboard.writeText(adventure.preview.shareCode).catch(() => undefined);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1800);
}

/** Supprime un preset personnel puis rend le focus à la carte sélectionnée. */
async function remove(p: AdventurePreset) {
  await removePreset(p);
  await nextTick();
  cards.value[adventure.index]?.focus();
}

/** Manette et clavier, transmis par le lanceur. */
function handle(a: PadAction) {
  if (adventure.step === "preset") {
    const n = adventure.presets.length;
    if (!n) return a === "back" && closeAdventure();
    // Corbeille d'un preset personnel : Menu (X / M) y amène le focus, A / Entrée supprime, B / Échap revient à la carte.
    const trash = document.activeElement instanceof HTMLElement && document.activeElement.classList.contains("remove") ? document.activeElement : null;
    if (trash && a === "accept") return trash.click();
    if (trash && a === "back") return cards.value[adventure.index]?.focus();
    if (a === "menu") return cards.value[adventure.index]?.parentElement?.querySelector<HTMLElement>(".remove")?.focus();
    if (a === "left") adventure.index = Math.max(adventure.index - 1, 0);
    else if (a === "right") adventure.index = Math.min(adventure.index + 1, n - 1);
    else if (a === "up") adventure.index = Math.max(adventure.index - COLUMNS, 0);
    else if (a === "down") adventure.index = Math.min(adventure.index + COLUMNS, n - 1);
    else if (a === "accept") void choosePreset(adventure.presets[adventure.index]);
    else if (a === "back") closeAdventure();
    return;
  }
  if (a === "accept" && adventure.preview && !adventure.loading) emit("play");
  else if (a === "menu") void reroll();
  else if (a === "back") adventure.step = "preset";
}

defineExpose({ handle });
</script>

<template>
  <div class="adventure" role="dialog" aria-modal="true" aria-label="Nouvelle aventure">
    <header class="head">
      <img v-if="props.cover" :src="props.cover" alt="" class="cover" />
      <div class="titles">
        <span class="step">{{ adventure.step === "preset" ? "2 / 3 · Le preset" : "3 / 3 · L'aperçu" }}</span>
        <h2>Nouvelle aventure<Tip term="adventure.adventure" /></h2>
        <p>{{ adventure.game?.game?.name ?? adventure.game?.title }}</p>
      </div>
      <button v-if="adventure.step === 'preview'" type="button" class="sv-btn" @click="adventure.step = 'preset'">
        <Icon name="chevron-left" :size="15" /> Presets
      </button>
      <button type="button" class="sv-round sq" aria-label="Fermer" title="Fermer (Échap)" @click="closeAdventure">
        <Icon name="x" :size="16" />
      </button>
    </header>

    <!-- 2. Le preset -->
    <section v-if="adventure.step === 'preset'" class="presets">
      <!-- La corbeille est une sœur de la carte (pas de bouton dans un bouton) : atteignable au clavier. -->
      <div v-for="(p, i) in adventure.presets" :key="p.id" class="slot" @mouseenter="adventure.index = i">
        <button
          :ref="(el) => (cards[i] = el as HTMLElement)"
          type="button"
          class="card sv-card"
          :class="{ on: i === adventure.index }"
          @click="choosePreset(p)"
        >
          <span class="card-head" :class="{ removable: p.user }">
            <strong>{{ p.name }}</strong>
            <span v-if="p.user" class="sv-chip dim">Ton preset</span>
            <span v-if="p.companion?.nuzlocke" class="sv-chip accent">Compagnon</span>
          </span>
          <span class="tagline">{{ p.tagline }}</span>
          <ul>
            <li v-for="c in p.changes" :key="c.label"><Icon :name="c.icon" :size="15" /> {{ c.label }}</li>
          </ul>
        </button>
        <button
          v-if="p.user"
          type="button"
          class="remove"
          :aria-label="`Supprimer le preset ${p.name}`"
          title="Supprimer ce preset"
          @click="remove(p)"
        >
          <Icon name="trash" :size="14" />
        </button>
      </div>
      <p class="hint">
        {{ padConnected ? "Croix pour choisir · A pour valider · B pour revenir" : "Flèches pour choisir · Entrée pour valider · Échap pour revenir" }}
      </p>
    </section>

    <!-- 3. L'aperçu -->
    <section v-else class="preview">
      <Banner v-if="adventure.error" :retry="reroll">Aperçu impossible : {{ adventure.error }}</Banner>
      <EmptyState v-if="adventure.loading && !adventure.preview" loading title="Tirage de la partie…">
        Kaleido randomise le jeu en mémoire pour te montrer le début de l'aventure.
      </EmptyState>
      <div v-else-if="adventure.preview" class="grid" :class="{ busy: adventure.loading }">
        <div class="block sv-panel starters">
          <span class="sv-label">Tes starters</span>
          <ul>
            <li v-for="s in adventure.preview.starters" :key="s.id">
              <Sprite :id="s.id" variant="model" :size="96" />
              <strong>{{ s.name }}</strong>
            </li>
          </ul>
        </div>
        <div v-if="route" class="block sv-panel">
          <span class="sv-label">Première route · {{ route.name }}</span>
          <ul class="mons">
            <li v-for="m in shownRoute" :key="m.species" :title="`${m.name} · ${levels(m)}`">
              <Sprite :id="m.species" :size="44" />
              <small>{{ m.name }}</small>
            </li>
            <li v-if="route.encounters.length > MAX_ROUTE" class="more">+{{ route.encounters.length - MAX_ROUTE }}</li>
          </ul>
        </div>
        <div v-if="adventure.preview.firstLeader" class="block sv-panel">
          <span class="sv-label">Premier champion · {{ adventure.preview.firstLeader.name }} ({{ adventure.preview.firstLeader.label }})</span>
          <ul class="mons">
            <li v-for="(m, i) in adventure.preview.firstLeader.team" :key="i">
              <Sprite :id="m.species" :size="44" />
              <small>{{ m.name }}</small>
              <small>N. {{ m.minLevel }}</small>
            </li>
          </ul>
        </div>
      </div>

      <div class="seed-row">
        <span class="seed">
          <span class="sv-label">Seed <Tip term="adventure.seed" /></span>
          <strong>{{ adventure.seed }}</strong>
        </span>
        <button type="button" class="sv-btn small" :disabled="!adventure.preview" @click="copyCode">
          <Icon :name="copied ? 'check' : 'copy'" :size="14" /> {{ copied ? "Code copié" : "Copier le code" }}
        </button>
        <input
          v-model="seedText"
          class="sv-input compact paste"
          :class="{ invalid: seedError }"
          placeholder="Coller une seed ou un code KLD1-…"
          aria-label="Seed ou code de partage"
          @keydown.enter.prevent="pasteSeed"
          @blur="pasteSeed"
        />
        <button type="button" class="sv-btn small" :disabled="adventure.loading" @click="reroll">
          <Icon name="dice" :size="14" /> Relancer le tirage
        </button>
      </div>

      <footer class="foot">
        <small class="dim">
          {{ adventure.preset?.name }}<template v-if="where"> · Écrite dans {{ where.where }} · {{ where.emulator }}</template>
          · <button type="button" class="sv-link" @click="customize">Changer</button>
        </small>
        <span class="grow" />
        <button type="button" class="sv-link" @click="customize">Personnaliser</button>
        <button type="button" class="play" :disabled="!adventure.preview || adventure.loading" @click="emit('play')">
          <Icon name="play" :size="18" /> Jouer
          <span class="key">{{ padConnected ? "A" : "Entrée" }}</span>
        </button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.adventure {
  position: absolute;
  inset: 0;
  z-index: 150;
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  padding: var(--sp-6) 34px;
  overflow: auto;
  /* Le lanceur écrit en blanc : la fenêtre reprend la couleur du thème (lisible en Jour). */
  color: var(--text);
  background: color-mix(in srgb, var(--bg) 82%, transparent);
  backdrop-filter: blur(14px);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--sp-4);
}

.cover {
  width: 64px;
  height: 64px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  box-shadow: var(--shadow);
}

.titles {
  flex: 1;
  min-width: 0;
}

.step {
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.titles h2 {
  display: flex;
  align-items: center;
  font-size: 26px;
}

.titles p {
  margin: 2px 0 0;
  color: var(--text-dim);
}

/* --- Presets */

.presets {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--sp-4);
  align-content: start;
}

/* La carte choisie suit le focus : son anneau de sélection suffit, pas de second contour. */
.card.on:focus-visible {
  outline: none;
}

.slot {
  position: relative;
  display: flex;
}

.card {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  min-height: 190px;
  padding: var(--sp-4) var(--sp-5);
}

.card-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
}

/* Place de la corbeille (presets personnels), en haut à droite. */
.card-head.removable {
  padding-right: var(--sp-6);
}

.card-head strong {
  font-size: var(--fs-xl);
}

.tagline {
  color: var(--text-dim);
  font-size: var(--fs-base);
}

.card ul {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin: auto 0 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-md);
}

.card li {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.remove {
  position: absolute;
  top: var(--sp-2);
  right: var(--sp-2);
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.remove:hover,
.remove:focus-visible {
  color: var(--danger);
}

.hint {
  grid-column: 1 / -1;
  margin: 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
  text-align: center;
}

/* --- Aperçu */

.preview {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-4);
}

.grid {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--sp-4);
  transition: opacity 0.2s;
}

.grid.busy {
  opacity: 0.5;
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-4);
}

.starters ul {
  display: flex;
  justify-content: space-around;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.starters li {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-1);
}

.mons {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.mons li {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  min-width: 0;
  text-align: center;
}

.mons small {
  max-width: 100%;
  overflow: hidden;
  color: var(--text-dim);
  font-size: var(--fs-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.more {
  justify-content: center;
  color: var(--text-dim);
  font-weight: 700;
}

.seed-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
}

.seed {
  display: flex;
  flex-direction: column;
}

.seed strong {
  font-size: var(--fs-lg);
  font-variant-numeric: tabular-nums;
}

.paste {
  width: min(320px, 100%);
}

.foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-4);
  margin-top: auto;
}

.dim {
  color: var(--text-dim);
}

.grow {
  flex: 1;
}

.play {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 12px 26px;
  border: none;
  border-radius: var(--radius-pill);
  background: var(--text);
  color: var(--bg);
  font-size: var(--fs-lg);
  font-weight: 700;
  transition: background-color 0.15s;
}

.play:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text) 86%, var(--bg));
}

.play:disabled {
  opacity: 0.45;
  cursor: default;
}

.key {
  padding: 1px 7px;
  border: 1px solid color-mix(in srgb, var(--bg) 40%, transparent);
  border-radius: var(--radius-xs);
  font-size: var(--fs-xs);
}

@media (max-width: 1100px) {
  .presets {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .grid {
    grid-template-columns: 1fr;
  }
}
</style>
