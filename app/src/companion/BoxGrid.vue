<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import { monName, type LiveSnapshot } from "./store";

/** Boîtes du PC en lecture seule : même grille de 6 × 5 que l'éditeur. */
const props = defineProps<{ snapshot: LiveSnapshot }>();

const box = ref(0);
const selected = ref<number | null>(null);
watch(
  () => props.snapshot.boxes.length,
  (n) => (box.value = Math.min(box.value, Math.max(n - 1, 0))),
);

const current = computed(() => props.snapshot.boxes[box.value]);
const cells = computed(() => {
  const out = Array.from({ length: 30 }, () => null as LiveSnapshot["boxes"][number]["mons"][number] | null);
  for (const m of current.value?.mons ?? []) out[m.index] = m;
  return out;
});
const picked = computed(() => (selected.value === null ? null : cells.value[selected.value]));
const total = computed(() => props.snapshot.boxes.reduce((n, b) => n + b.mons.length, 0));

function step(d: number) {
  const n = props.snapshot.boxes.length;
  box.value = (box.value + d + n) % n;
  selected.value = null;
}
</script>

<template>
  <section class="boxes">
    <header>
      <button type="button" class="sv-round" aria-label="Boîte précédente" @click="step(-1)"><Icon name="chevron-left" /></button>
      <h2>{{ current?.name }}</h2>
      <button type="button" class="sv-round" aria-label="Boîte suivante" @click="step(1)"><Icon name="chevron-right" /></button>
      <small>{{ current?.mons.length ?? 0 }}/30 · {{ total }} au total</small>
    </header>
    <div class="grid" role="group" :aria-label="current?.name">
      <button
        v-for="(m, i) in cells"
        :key="i"
        type="button"
        class="cell sv-card"
        :class="{ on: selected === i && m }"
        :disabled="!m"
        :aria-label="m ? `${monName(m)}, N. ${m.level}` : 'Vide'"
        :title="m ? `${monName(m)} · N. ${m.level}` : undefined"
        @click="selected = i"
      >
        <Sprite v-if="m" :id="m.isEgg ? 0 : m.species" :form="m.form" :shiny="m.shiny" :size="40" />
        <Icon v-if="m?.shiny" name="sparkle" :size="10" class="shiny" />
      </button>
    </div>
    <p class="pick">
      <template v-if="picked">
        <strong>{{ monName(picked) }}</strong>
        <template v-if="!picked.isEgg"><template v-if="picked.nickname && picked.nickname !== picked.speciesName"> · {{ picked.speciesName }}</template> · N. {{ picked.level }}</template>
        <span v-if="picked.shiny" class="sv-chip shiny">Chromatique</span>
      </template>
      <span v-else class="dim">Choisis un Pokémon pour voir son nom et son niveau.</span>
    </p>
  </section>
</template>

<style scoped>
.boxes {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

header {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

h2 {
  flex: 1;
  overflow: hidden;
  font-size: var(--fs-lg);
  text-align: center;
  white-space: nowrap;
  text-overflow: ellipsis;
}

header small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
  white-space: nowrap;
}

.grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: var(--sp-1);
}

.cell {
  position: relative;
  display: grid;
  place-items: center;
  aspect-ratio: 1;
  min-width: 0;
  padding: 0;
  overflow: hidden;
}

/* Anneau de focus posé sur le contour de la case : sans décalage, il ne double pas
   l'anneau de sélection et ne déborde pas sur les cases voisines. */
.cell:focus-visible {
  outline-offset: 0;
}

.cell:disabled {
  opacity: 0.45;
  cursor: default;
}

.cell :deep(.sprite) {
  max-width: 100%;
  max-height: 100%;
}

.shiny {
  position: absolute;
  top: 3px;
  left: 3px;
  color: var(--shiny);
}

.pick {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1);
  min-height: 24px;
  margin: 0;
  font-size: var(--fs-md);
}

.dim {
  color: var(--text-dim);
}
</style>
