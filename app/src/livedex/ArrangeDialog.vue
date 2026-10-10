<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Dialog from "../components/Dialog.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import { arrangePlan, type ArrangeStep, type Position } from "./arrange";
import { dex } from "./data";
import { collection, livedex } from "./store";
import type { Specimen } from "./types";
import { livedexUi } from "./ui";

/**
 * Plan de rangement d'une sauvegarde : où mettre chaque Pokémon pour que ses boîtes suivent
 * l'ordre de la Living Dex. Lecture seule : les déplacements se font dans l'éditeur ou en jeu.
 */
const open = defineModel<boolean>({ required: true });

const saves = computed(() => livedex.sources.filter((s) => s.path !== "bank" && s.boxes > 0));
const path = ref("");
watch(
  [open, saves],
  () => {
    if (!saves.value.some((s) => s.path === path.value)) {
      // Par défaut : la sauvegarde qui a le plus de Pokémon.
      path.value = [...saves.value].sort((a, b) => b.specimens.length - a.specimens.length)[0]?.path ?? "";
    }
  },
  { immediate: true },
);
const source = computed(() => saves.value.find((s) => s.path === path.value));

const plan = computed(() => {
  const d = dex.value;
  const c = collection.value;
  const s = source.value;
  if (!d || !c || !s) return null;
  return arrangePlan(d, livedex.saved.rules, c.slots, s.specimens, s.boxes, livedexUi.shiny);
});

type Filter = "move" | "missing" | "all";
const filter = ref<Filter>("move");
const shown = computed(() => {
  const steps = plan.value?.steps ?? [];
  return filter.value === "all" ? steps : steps.filter((s) => s.status === filter.value);
});
/** Les étapes regroupées par boîte visée. */
const groups = computed(() => {
  const map = new Map<number, ArrangeStep[]>();
  for (const s of shown.value) {
    const list = map.get(s.target.box);
    if (list) list.push(s);
    else map.set(s.target.box, [s]);
  }
  return [...map.entries()];
});

const pos = (p: Position) => `Boîte ${p.box + 1} · case ${p.index + 1}`;
const nameOf = (s: Specimen) => s.nickname || dex.value?.form(s.species, s.form)?.full || dex.value?.species(s.species)?.name || `#${s.species}`;
const fmt = (n: number) => n.toLocaleString("fr-FR");
</script>

<template>
  <Dialog v-model="open" title="Plan de rangement" subtitle="Où ranger chaque Pokémon pour que les boîtes suivent l'ordre de la Living Dex" icon="box" :width="820">
    <p v-if="!saves.length" class="dim">Aucune sauvegarde lue pour l'instant.</p>
    <template v-else>
      <div class="bar">
        <select v-model="path" class="sv-select" aria-label="Sauvegarde">
          <option v-for="s in saves" :key="s.path" :value="s.path">{{ s.game }} · {{ s.trainer || "sans nom" }} ({{ fmt(s.specimens.length) }} Pokémon)</option>
        </select>
        <span class="dim">{{ livedexUi.shiny ? "Living Dex chromatique" : "Living Dex normale" }}</span>
      </div>

      <template v-if="plan">
        <dl class="stats">
          <div>
            <dt>Déjà à leur place</dt>
            <dd>{{ fmt(plan.ok) }}</dd>
          </div>
          <div>
            <dt>À déplacer</dt>
            <dd>{{ fmt(plan.moves) }}</dd>
          </div>
          <div>
            <dt>Manquants</dt>
            <dd>{{ fmt(plan.missing) }}</dd>
          </div>
          <div>
            <dt>Boîtes</dt>
            <dd>{{ plan.boxesNeeded }} nécessaires · {{ plan.boxesAvailable }} dans le jeu</dd>
          </div>
        </dl>

        <p v-if="plan.overflow" class="sv-banner warn">
          La Living Dex compte {{ fmt(plan.overflow) }} cases de trop pour cette sauvegarde : le plan s'arrête à la dernière boîte. Avec la règle « Espèces »,
          moins de cases sont nécessaires.
        </p>
        <p v-if="plan.spare.length" class="dim small">
          {{ fmt(plan.spare.length) }} Pokémon de cette sauvegarde ne servent à aucune case (doublons, formes non comptées) : range-les ailleurs, par
          exemple dans la banque, pour libérer leur place.
        </p>

        <Segmented
          v-model="filter"
          :options="[
            { value: 'move', label: `À déplacer (${fmt(plan.moves)})` },
            { value: 'missing', label: `Manquants (${fmt(plan.missing)})` },
            { value: 'all', label: 'Tout' },
          ]"
          label="Étapes affichées"
        />

        <p v-if="!shown.length" class="dim empty">{{ filter === "move" ? "Rien à déplacer : tout est déjà à sa place." : "Rien à afficher." }}</p>
        <section v-for="[box, steps] in groups" :key="box" class="box">
          <h3 class="sv-section-title">Boîte {{ box + 1 }}</h3>
          <ul>
            <li v-for="s in steps" :key="s.slot.key" :class="s.status">
              <span class="where">case {{ s.target.index + 1 }}</span>
              <Sprite :id="s.slot.species" :form="s.slot.form" :shiny="livedexUi.shiny" :size="36" />
              <span class="what">
                <strong>{{ s.specimen ? nameOf(s.specimen) : s.slot.label }}</strong>
                <small v-if="s.status === 'move'" class="dim">
                  depuis {{ s.from ? pos(s.from) : "l'équipe" }}<template v-if="s.occupant"> · prend la place de {{ nameOf(s.occupant) }}</template>
                </small>
                <small v-else-if="s.status === 'missing'" class="dim">à obtenir</small>
                <small v-else class="dim">déjà à sa place</small>
              </span>
            </li>
          </ul>
        </section>
      </template>
    </template>
  </Dialog>
</template>

<style scoped>
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
}

.bar .sv-select {
  width: auto;
  max-width: 70%;
}

.dim {
  color: var(--text-dim);
}

.small {
  font-size: var(--fs-sm);
}

.empty {
  margin-top: var(--sp-4);
  text-align: center;
}

.stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-5);
  margin: var(--sp-4) 0;
}

.stats dt {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.stats dd {
  margin: 2px 0 0;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.sv-banner {
  margin: 0 0 var(--sp-3);
}

.box {
  margin-top: var(--sp-4);
}

.box ul {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px var(--sp-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.box li {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 2px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.box li.missing :deep(img) {
  filter: brightness(0);
  opacity: 0.18;
}

.where {
  width: 58px;
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-variant-numeric: tabular-nums;
}

.what {
  display: grid;
  min-width: 0;
}

.what strong,
.what small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
