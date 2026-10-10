<script setup lang="ts">
import { computed, ref } from "vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Icon from "../components/Icon.vue";
import Toggle from "../components/Toggle.vue";
import ArrangeDialog from "./ArrangeDialog.vue";
import { dex, frDate } from "./data";
import type { LivingSlot } from "./slots";
import { collection } from "./store";
import { livedexUi, openSpecies, rememberUi } from "./ui";

const BOX = 30;
const hideFull = ref(false);
const arranging = ref(false);

const shiny = computed({
  get: () => livedexUi.shiny,
  set: (v: boolean) => {
    livedexUi.shiny = v;
    rememberUi();
  },
});

const filled = (s: LivingSlot) => (livedexUi.shiny ? collection.value?.caughtShiny.has(s.key) : collection.value?.caught.has(s.key)) ?? false;

const boxes = computed(() => {
  const c = collection.value;
  if (!c) return [];
  const out: { index: number; slots: LivingSlot[]; count: number }[] = [];
  for (let i = 0; i < c.slots.length; i += BOX) {
    const slots = c.slots.slice(i, i + BOX);
    out.push({ index: i / BOX, slots, count: slots.filter(filled).length });
  }
  return out;
});
const shown = computed(() => (hideFull.value ? boxes.value.filter((b) => b.count < b.slots.length) : boxes.value));

const num = (n: number) => String(n).padStart(4, "0");

function tooltip(s: LivingSlot): string {
  const list = collection.value?.bySlot.get(s.key) ?? [];
  const mine = livedexUi.shiny ? list.filter((e) => e.shiny) : list;
  if (!mine.length) return `${s.label} · manquant`;
  const e = mine[0];
  const from = e.auto ? (e.where?.[0] ? `${e.where[0].label} · ${e.where[0].place}` : "") : `Noté à la main${e.game ? ` · ${dex.value?.game(e.game)?.short ?? ""}` : ""}`;
  const more = mine.length > 1 ? `\n+ ${mine.length - 1} autre${mine.length > 2 ? "s" : ""}` : "";
  return `${s.label}\n${from}${e.date ? `\nDepuis le ${frDate(e.date)}` : ""}${more}`;
}

const spriteGender = (s: LivingSlot) => (s.gender === "f" ? "female" : undefined);
</script>

<template>
  <div class="boxes-tab">
    <div class="bar">
      <Segmented
        v-model="shiny"
        :options="[
          { value: false, label: 'Normale' },
          { value: true, label: 'Chromatique' },
        ]"
        label="Living Dex affichée"
      />
      <span class="sv-row">
        <Toggle v-model="hideFull" label="Masquer les boîtes complètes" />
        <button type="button" class="sv-btn small" title="Où ranger chaque Pokémon dans une sauvegarde pour suivre l'ordre de la Living Dex" @click="arranging = true"><Icon name="box" :size="14" /> Plan de rangement</button>
      </span>
    </div>

    <p v-if="!shown.length" class="dim done">Toutes les boîtes sont complètes.</p>

    <div class="grid">
      <section v-for="b in shown" :key="b.index" class="box sv-panel" :class="{ complete: b.count === b.slots.length }">
        <header>
          <strong>Boîte {{ b.index + 1 }}</strong>
          <span class="dim">n° {{ num(b.slots[0].species) }} – {{ num(b.slots[b.slots.length - 1].species) }}</span>
          <span class="count" :class="{ full: b.count === b.slots.length }">{{ b.count }}/{{ b.slots.length }}</span>
        </header>
        <div class="cells">
          <button
            v-for="s in b.slots"
            :key="s.key"
            type="button"
            class="cell"
            :class="{ have: filled(s) }"
            :title="tooltip(s)"
            :aria-label="tooltip(s)"
            @click="openSpecies(s.species, s.form)"
          >
            <Sprite :id="s.species" :form="s.form" :gender="spriteGender(s)" :shiny="livedexUi.shiny" :size="48" />
          </button>
        </div>
      </section>
    </div>
    <ArrangeDialog v-model="arranging" />
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  margin-bottom: var(--sp-4);
}

.dim {
  color: var(--text-dim);
}

.done {
  text-align: center;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(372px, 1fr));
  gap: var(--sp-4);
}

.box {
  padding: 12px;
  /* Les boîtes hors de l'écran ne sont pas dessinées (1 600 cases). */
  content-visibility: auto;
  contain-intrinsic-size: auto 360px;
}

.box header {
  display: flex;
  align-items: baseline;
  gap: var(--sp-2);
  margin-bottom: 10px;
  font-size: var(--fs-sm);
}

.box header .count {
  margin-left: auto;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.box header .count.full {
  color: var(--ok);
  font-weight: 700;
}

.cells {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 4px;
}

.cell {
  display: grid;
  place-items: center;
  aspect-ratio: 1;
  padding: 0;
  border: 1px solid transparent;
  border-radius: var(--radius-xs);
  background: color-mix(in srgb, var(--text) 5%, transparent);
  cursor: pointer;
  transition: background-color 0.12s, border-color 0.12s;
}

.cell:hover {
  border-color: color-mix(in srgb, var(--text) 35%, transparent);
}

/* Case vide : silhouette discrète. */
.cell:not(.have) :deep(img) {
  filter: brightness(0);
  opacity: 0.16;
}

:root[data-theme="jour"] .cell:not(.have) :deep(img) {
  opacity: 0.12;
}

.cell.have {
  background: color-mix(in srgb, var(--text) 10%, transparent);
}
</style>
