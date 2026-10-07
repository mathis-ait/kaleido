<script setup lang="ts">
import { computed } from "vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import HpBar from "./HpBar.vue";
import { encounterOutcome, type BattleView, type Encounter, type EncounterOutcome } from "./store";

/**
 * Lecture en direct : une seule carte pour le combat en cours (adversaires et leurs PV) et,
 * pour une rencontre sauvage, son issue pour le Nuzlocke. Visible seulement en mémoire lue.
 */
const props = defineProps<{ battle: BattleView | null; encounter: Encounter | null; place: string | null }>();

const OUTCOMES: { value: EncounterOutcome; label: string }[] = [
  { value: "caught", label: "Capturé" },
  { value: "missed", label: "Raté" },
  { value: "fled", label: "Fui" },
];
const outcomeLabel = computed(() => OUTCOMES.find((o) => o.value === props.encounter?.outcome)?.label ?? "");

const fighting = computed(() => !!props.battle?.foes.length);
const title = computed(() => {
  if (!fighting.value) return "Rencontre";
  return props.battle!.wild ? "Combat sauvage" : "Combat de dresseur";
});
const where = computed(() => props.encounter?.place ?? props.place);
</script>

<template>
  <section class="fight sv-card" aria-live="polite">
    <header>
      <span class="sv-label">{{ title }} <Tip :term="fighting ? 'companion.battle' : 'companion.encounter'" /></span>
      <small v-if="where" class="dim">{{ where }}</small>
    </header>

    <ul v-if="fighting" class="foes">
      <li v-for="f in battle!.foes" :key="f.pid" :class="{ ko: f.hp === 0 }">
        <Sprite :id="f.species" :form="f.form" :shiny="f.shiny" :gender="f.gender" :size="56" />
        <span class="who">
          <span class="name">
            <strong>{{ f.speciesName }}</strong>
            <small>N. {{ f.level }}</small>
            <span v-if="f.hp === 0" class="sv-chip danger">K.O.</span>
          </span>
          <HpBar :hp="f.hp" :max="f.maxHp" />
        </span>
      </li>
    </ul>
    <div v-else-if="encounter" class="foes">
      <span class="one">
        <Sprite :id="encounter.species" :form="encounter.form" :shiny="encounter.shiny" :size="56" />
        <span class="name">
          <strong>{{ encounter.speciesName }}</strong>
          <small>N. {{ encounter.level }}</small>
        </span>
      </span>
    </div>

    <div v-if="encounter" class="outcome">
      <template v-if="!encounter.outcome">
        <span class="dim">Pour le Nuzlocke, cette rencontre est :</span>
        <span class="actions" role="group" aria-label="Issue de la rencontre">
          <button v-for="o in OUTCOMES" :key="o.value" type="button" class="sv-btn small" @click="encounterOutcome(encounter.id, o.value)">
            {{ o.label }}
          </button>
        </span>
      </template>
      <template v-else>
        <span class="dim">Rencontre notée :</span>
        <span class="sv-chip" :class="encounter.outcome === 'caught' ? 'ok' : 'dim'">{{ outcomeLabel }}</span>
      </template>
    </div>
  </section>
</template>

<style scoped>
.fight {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3);
}

header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--sp-2);
}

.dim,
small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.foes {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.foes li,
.one {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.foes li.ko :deep(img) {
  filter: grayscale(1);
  opacity: 0.5;
}

.who {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.name {
  display: flex;
  align-items: baseline;
  gap: var(--sp-2);
  min-width: 0;
}

.name strong {
  overflow: hidden;
  font-size: var(--fs-base);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.outcome {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
  padding-top: var(--sp-2);
  border-top: 1px solid var(--border);
  font-size: var(--fs-md);
}

.actions {
  display: flex;
  gap: var(--sp-1);
}
</style>
