<script setup lang="ts">
import Tip from "../../components/Tip.vue";
import { battle, resetOptions, type SideOptions, type Status, type Weather } from "../../battle";
import { BATTLE_TIPS } from "./glossary";

const WEATHERS: { id: Weather; label: string }[] = [
  { id: "none", label: "Aucune" },
  { id: "sun", label: "Soleil" },
  { id: "rain", label: "Pluie" },
  { id: "sand", label: "Sable" },
  { id: "hail", label: "Grêle" },
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
      <span class="sv-label">Météo <Tip v-bind="BATTLE_TIPS.weather" /></span>
      <div class="sv-seg">
        <button v-for="w in WEATHERS" :key="w.id" :class="{ on: battle.options.field.weather === w.id }" @click="battle.options.field.weather = w.id">
          {{ w.label }}
        </button>
      </div>
    </div>
    <div class="block">
      <span class="sv-label">Intimidation <Tip v-bind="BATTLE_TIPS.intimidate" /></span>
      <label class="sv-switch">
        <input v-model="battle.options.field.intimidate" type="checkbox" />
        <span class="track" />
        {{ battle.options.field.intimidate ? "Appliquée" : "Ignorée" }}
      </label>
    </div>
    <div v-for="s in sides" :key="s.key" class="block side">
      <span class="sv-label">{{ s.title }} <Tip v-bind="BATTLE_TIPS.stages" /></span>
      <div class="stages">
        <label v-for="st in STAGES" :key="st.i" :title="`Niveau de ${st.label}`">
          <small>{{ st.label }}</small>
          <select
            class="sv-select"
            :class="{ up: battle.options[s.key].boosts[st.i] > 0, down: battle.options[s.key].boosts[st.i] < 0 }"
            :value="battle.options[s.key].boosts[st.i]"
            @change="setStage(battle.options[s.key], st.i, ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="l in LEVELS" :key="l" :value="l">{{ l > 0 ? `+${l}` : l }}</option>
          </select>
        </label>
        <label>
          <small>Statut <Tip v-bind="BATTLE_TIPS.status" /></small>
          <select v-model="battle.options[s.key].status" class="sv-select">
            <option v-for="o in STATUSES" :key="o.id" :value="o.id">{{ o.label }}</option>
          </select>
        </label>
        <label>
          <small>PV % <Tip v-bind="BATTLE_TIPS.hp" /></small>
          <input v-model.number="battle.options[s.key].hpPercent" class="sv-input" type="number" min="1" max="100" />
        </label>
      </div>
    </div>
    <button class="sv-btn reset" @click="resetOptions()">Réinitialiser</button>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 14px 22px;
  padding: 12px 16px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.stages {
  display: flex;
  gap: 6px;
}

.stages label {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.stages small {
  display: flex;
  align-items: center;
  color: var(--text-dim);
  font-size: 11px;
}

.stages .sv-select,
.stages .sv-input {
  width: auto;
  min-width: 58px;
  padding: 5px 6px;
  font-size: 13px;
}

.stages .sv-input {
  width: 64px;
}

.sv-select.up {
  color: #22c55e;
}

.sv-select.down {
  color: var(--danger);
}

.reset {
  margin-left: auto;
}
</style>
