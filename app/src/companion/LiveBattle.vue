<script setup lang="ts">
import { computed } from "vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import HpBar from "./HpBar.vue";
import { encounterOutcome, type BattleView, type Encounter, type EncounterOutcome } from "./store";

/**
 * Lecture en direct : rencontre sauvage (avec son issue) et équipe adverse du combat en cours.
 * N'apparaît que si la mémoire de l'émulateur est lue.
 */
const props = defineProps<{ battle: BattleView | null; encounter: Encounter | null }>();

const OUTCOMES: { value: EncounterOutcome; label: string }[] = [
  { value: "caught", label: "Capturé" },
  { value: "missed", label: "Raté" },
  { value: "fled", label: "Fui" },
];
const outcomeLabel = computed(() => OUTCOMES.find((o) => o.value === props.encounter?.outcome)?.label ?? "");
</script>

<template>
  <section v-if="encounter" class="encounter sv-card" aria-live="polite">
    <Sprite :id="encounter.species" :form="encounter.form" :shiny="encounter.shiny" :size="48" />
    <div class="what">
      <span class="sv-label">Rencontre <Tip term="companion.encounter" /></span>
      <strong>{{ encounter.speciesName }}</strong>
      <small>N. {{ encounter.level }}<template v-if="encounter.place"> · {{ encounter.place }}</template></small>
    </div>
    <div v-if="!encounter.outcome" class="actions" role="group" aria-label="Issue de la rencontre">
      <button v-for="o in OUTCOMES" :key="o.value" type="button" class="sv-btn small" @click="encounterOutcome(encounter.id, o.value)">
        {{ o.label }}
      </button>
    </div>
    <span v-else class="sv-chip" :class="encounter.outcome === 'caught' ? 'ok' : 'dim'">{{ outcomeLabel }}</span>
  </section>

  <section v-if="battle && battle.foes.length" class="battle">
    <span class="sv-label">{{ battle.wild ? "Combat sauvage" : "Combat de dresseur" }} <Tip term="companion.battle" /></span>
    <ul>
      <li v-for="f in battle.foes" :key="f.pid" class="foe sv-card" :class="{ ko: f.hp === 0 }">
        <Sprite :id="f.species" :form="f.form" :shiny="f.shiny" :gender="f.gender" :size="40" />
        <span class="who">
          <span class="name">
            <strong>{{ f.speciesName }}</strong>
            <small>N. {{ f.level }}</small>
          </span>
          <HpBar :hp="f.hp" :max="f.maxHp" />
        </span>
        <span v-if="f.hp === 0" class="sv-chip danger">K.O.</span>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.encounter {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-3);
}

.what {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.what strong {
  overflow: hidden;
  font-size: var(--fs-base);
  white-space: nowrap;
  text-overflow: ellipsis;
}

small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.actions {
  display: flex;
  gap: var(--sp-1);
}

.battle {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.battle ul {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.foe {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-1) var(--sp-3);
}

.foe.ko {
  border-color: var(--danger);
}

.foe.ko :deep(.sprite) {
  filter: grayscale(1);
  opacity: 0.6;
}

.who {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.name {
  display: flex;
  align-items: baseline;
  gap: var(--sp-2);
}
</style>
