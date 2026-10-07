<script setup lang="ts">
import Segmented from "../../components/Segmented.vue";
import Tip from "../../components/Tip.vue";
import Toggle from "../../components/Toggle.vue";
import { battle, resetOptions, type SideOptions, type Status, type Weather } from "../../battle";

const WEATHERS: { value: Weather; label: string }[] = [
  { value: "none", label: "Aucune" },
  { value: "sun", label: "Soleil" },
  { value: "rain", label: "Pluie" },
  { value: "sand", label: "Sable" },
  { value: "hail", label: "Grêle" },
];
const STATUSES: { id: Status; label: string }[] = [
  { id: "none", label: "Aucun" },
  { id: "burn", label: "Brûlure" },
  { id: "paralysis", label: "Paralysie" },
  { id: "poison", label: "Poison" },
  { id: "sleep", label: "Sommeil" },
];
const STAGES = [
  { i: 1, label: "Att" },
  { i: 2, label: "Déf" },
  { i: 3, label: "AtS" },
  { i: 4, label: "DéS" },
  { i: 5, label: "Vit" },
];
const LEVELS = [-6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6];

const sides: { key: "mine" | "theirs"; title: string }[] = [
  { key: "mine", title: "Mon équipe" },
  { key: "theirs", title: "Équipe adverse" },
];

function setStage(s: SideOptions, i: number, v: string) {
  const boosts = [...s.boosts];
  boosts[i] = Number(v);
  s.boosts = boosts;
}
</script>

<template>
  <div class="settings sv-panel">
    <div class="block">
      <span class="sv-label">Météo <Tip term="battle.weather" /></span>
      <Segmented v-model="battle.options.field.weather" :options="WEATHERS" label="Météo" />
    </div>
    <div class="block">
      <span class="sv-label">Intimidation <Tip term="battle.intimidate" /></span>
      <Toggle v-model="battle.options.field.intimidate" :label="battle.options.field.intimidate ? 'Appliquée' : 'Ignorée'" />
    </div>
    <div v-for="s in sides" :key="s.key" class="block side">
      <span class="sv-label">{{ s.title }} <Tip term="battle.stages" /></span>
      <div class="stages">
        <label v-for="st in STAGES" :key="st.i">
          <small>{{ st.label }}<Tip v-if="s.key === 'mine' && st.i === STAGES[STAGES.length - 1].i" term="statShort" /></small>
          <select
            class="sv-select"
            :aria-label="`${s.title} : niveau de ${st.label}`"
            :class="{ up: battle.options[s.key].boosts[st.i] > 0, down: battle.options[s.key].boosts[st.i] < 0 }"
            :value="battle.options[s.key].boosts[st.i]"
            @change="setStage(battle.options[s.key], st.i, ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="l in LEVELS" :key="l" :value="l">{{ l > 0 ? `+${l}` : l }}</option>
          </select>
        </label>
        <label>
          <small>Statut <Tip term="battle.status" /></small>
          <select v-model="battle.options[s.key].status" class="sv-select">
            <option v-for="o in STATUSES" :key="o.id" :value="o.id">{{ o.label }}</option>
          </select>
        </label>
        <label>
          <small>PV % <Tip term="battle.hp" /></small>
          <input v-model.number="battle.options[s.key].hpPercent" class="sv-input" type="number" min="1" max="100" />
        </label>
      </div>
    </div>
    <button type="button" class="sv-btn reset" @click="resetOptions()">Réinitialiser</button>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: var(--sp-3) var(--sp-5);
  padding: var(--sp-3) var(--sp-4);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.stages {
  display: flex;
  gap: var(--sp-2);
}

.stages label {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
}

.stages small {
  display: flex;
  align-items: center;
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.stages .sv-select,
.stages .sv-input {
  width: auto;
  min-width: 58px;
  padding: 5px 6px;
  font-size: var(--fs-md);
}

.stages .sv-input {
  width: 64px;
}

.sv-select.up {
  color: var(--ok);
}

.sv-select.down {
  color: var(--danger);
}

.reset {
  margin-left: auto;
}
</style>
