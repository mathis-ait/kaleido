<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { lists } from "../../saveStore";
import type { SlotView } from "../../types";
import { hiddenPowerType, ivsForHiddenPower, natureEffect, STAT_LABELS } from "../refdata";
import { apply, num } from "./edit";

const props = defineProps<{ p: SlotView }>();

const ivs = ref([...props.p.ivs]);
const evs = ref([...props.p.evs]);
watch(
  () => props.p,
  (p) => {
    ivs.value = [...p.ivs];
    evs.value = [...p.evs];
  },
);

const effect = computed(() => natureEffect(props.p.nature));
const evTotal = computed(() => evs.value.reduce((a, b) => a + (Number(b) || 0), 0));
const base = computed(() => props.p.speciesData?.baseStats ?? null);
const baseTotal = computed(() => base.value?.reduce((a, b) => a + b, 0) ?? 0);
const maxStat = computed(() => Math.max(...(props.p.stats ?? [1]), 1));
const hpType = computed(() => hiddenPowerType(props.p.ivs));
/** Types possibles de la Puissance Cachée : Combat (1) à Ténèbres (16). */
const hpTypes = computed(() => lists.types.slice(1, 17).map((label, i) => ({ value: i + 1, label })));
function setHiddenPower(type: number) {
  if (type !== hpType.value) apply({ ivs: ivsForHiddenPower(props.p.ivs, type) });
}
const characteristic = computed(() => lists.characteristics[props.p.extras.characteristic] ?? "");

/** Gen 7 : Hyper Training, dans l'ordre des lignes (PV, Att, Déf, Atq Spé, Déf Spé, Vit). */
const hyper = computed(() => props.p.extras.hyperTraining);
function toggleHyper(i: number) {
  const v = [...(hyper.value ?? [])];
  v[i] = !v[i];
  apply({ extras: { hyperTraining: v } });
}

function commit(kind: "ivs" | "evs", i: number) {
  const arr = kind === "ivs" ? ivs.value : evs.value;
  const v = num(arr[i], 0, kind === "ivs" ? 31 : 252);
  if (v === null) return;
  arr[i] = v;
  const current = kind === "ivs" ? props.p.ivs : props.p.evs;
  if (current[i] !== v) apply(kind === "ivs" ? { ivs: arr.map(Number) } : { evs: arr.map(Number) });
}

const PRESETS: { label: string; evs: number[] }[] = [
  { label: "Attaquant physique", evs: [4, 252, 0, 0, 0, 252] },
  { label: "Attaquant spécial", evs: [4, 0, 0, 252, 0, 252] },
  { label: "Mur physique", evs: [252, 0, 252, 0, 4, 0] },
  { label: "Mur spécial", evs: [252, 0, 4, 0, 252, 0] },
];

const randomIvs = () => Array.from({ length: 6 }, () => Math.floor(Math.random() * 32));
</script>

<template>
  <div class="stats">
    <div class="table">
      <div class="row head">
        <span>Statistique <Tip term="stats" /></span>
        <span class="c">Base <Tip term="baseStats" /></span>
        <span class="c">IV <Tip term="iv" /></span>
        <span class="c">EV <Tip term="ev" /></span>
        <span class="c">Total</span>
        <span v-if="hyper" class="c">Hyper <Tip term="hyperTraining" /></span>
        <span v-else />
      </div>
      <div v-for="(label, i) in STAT_LABELS" :key="label" class="row">
        <span class="name" :class="{ up: effect?.up === i, down: effect?.down === i }">
          {{ label }}
          <small v-if="effect?.up === i">▲ nature</small>
          <small v-if="effect?.down === i">▼ nature</small>
        </span>
        <span class="c dim">{{ base ? base[i] : "—" }}</span>
        <input
          v-model="ivs[i]"
          class="sv-input c"
          :class="{ perfect: Number(ivs[i]) === 31 }"
          type="number"
          min="0"
          max="31"
          :aria-label="`IV ${label}`"
          @change="commit('ivs', i)"
        />
        <input v-model="evs[i]" class="sv-input c" type="number" min="0" max="252" step="4" :aria-label="`EV ${label}`" @change="commit('evs', i)" />
        <strong class="c total">{{ p.stats ? p.stats[i] : "—" }}</strong>
        <span class="bar-cell">
          <span class="bar"><span :style="{ width: `${((p.stats?.[i] ?? 0) / maxStat) * 100}%` }" :class="{ up: effect?.up === i, down: effect?.down === i }" /></span>
          <button
            v-if="hyper"
            class="hyper"
            :class="{ on: hyper[i] }"
            :aria-pressed="hyper[i]"
            :title="hyper[i] ? 'Entraînée par l’Hyper Training (compte comme 31)' : 'Entraîner avec l’Hyper Training'"
            @click="toggleHyper(i)"
          >
            HT
          </button>
        </span>
      </div>
      <div class="row foot">
        <span>Total</span>
        <span class="c dim">{{ baseTotal || "—" }}</span>
        <span class="c dim">{{ p.ivs.reduce((a, b) => a + b, 0) }}</span>
        <span class="c" :class="{ over: evTotal > 510 }">{{ evTotal }}/510</span>
        <span />
        <span />
      </div>
    </div>

    <div class="side">
      <h3 class="sv-section-title">IV</h3>
      <div class="sv-row">
        <button class="sv-btn solid" @click="apply({ ivs: [31, 31, 31, 31, 31, 31] })">Tout à 31</button>
        <button class="sv-btn" @click="apply({ ivs: randomIvs() })">Au hasard</button>
        <button class="sv-btn" title="Pour Distorsion" @click="apply({ ivs: [31, 31, 31, 31, 31, 0] })">Vitesse à 0</button>
      </div>
      <label class="sv-field hp">
        <span>Puissance Cachée <Tip term="hiddenPower" /></span>
        <select class="sv-select" :value="hpType" @change="setHiddenPower(Number(($event.target as HTMLSelectElement).value))">
          <option v-for="t in hpTypes" :key="t.value" :value="t.value">{{ t.label }}</option>
        </select>
      </label>
      <p class="sv-help">Choisir un type ajuste la parité des IV (± 1 point) <Tip term="hiddenPowerType" /></p>
      <p class="sv-help">Caractéristique : <strong>{{ characteristic }}</strong> <Tip term="characteristic" /></p>
      <div v-if="hyper" class="sv-row">
        <button class="sv-btn" @click="apply({ extras: { hyperTraining: p.ivs.map((iv) => iv < 31) } })">Hyper Training sur les IV &lt; 31</button>
        <button class="sv-btn" @click="apply({ extras: { hyperTraining: [false, false, false, false, false, false] } })">Retirer l'Hyper Training</button>
      </div>

      <h3 class="sv-section-title">EV</h3>
      <div class="sv-row">
        <button v-for="pr in PRESETS" :key="pr.label" class="sv-btn" @click="apply({ evs: pr.evs })">{{ pr.label }}</button>
        <button class="sv-btn danger" @click="apply({ evs: [0, 0, 0, 0, 0, 0] })">Remettre à 0</button>
      </div>
      <p class="sv-help">Les EV au-delà de 510 au total sont retirés en partant de la Vitesse.</p>
    </div>
  </div>
</template>

<style scoped>
.stats {
  display: grid;
  grid-template-columns: minmax(0, 1.6fr) minmax(240px, 1fr);
  gap: 28px;
}

.table {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.row {
  display: grid;
  grid-template-columns: 1.4fr 0.6fr 0.8fr 0.9fr 0.7fr 1.4fr;
  align-items: center;
  gap: 10px;
}

.head {
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.head span {
  display: flex;
  align-items: center;
}

.head .c {
  justify-content: center;
}

.c {
  text-align: center;
}

.name {
  font-weight: 700;
}

.name small {
  display: block;
  font-size: var(--fs-xs);
  font-weight: 600;
}

/* Stat favorisée / défavorisée par la nature : rouge / bleu lisibles dans les trois thèmes. */
.name.up {
  color: var(--danger);
}

.name.down {
  color: var(--male);
}

.dim {
  color: var(--text-dim);
}

.sv-input.c {
  padding: 7px 6px;
}

.perfect {
  border-color: var(--ok) !important;
}

.total {
  font-size: 16px;
  font-variant-numeric: tabular-nums;
}

.bar-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bar-cell .bar {
  flex: 1;
}

.hyper {
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  font-size: var(--fs-xs);
  font-weight: 800;
  cursor: pointer;
}

.hyper.on {
  border-color: var(--warn);
  background: var(--warn-bg);
  color: var(--warn);
}

.hp {
  margin-bottom: 4px;
}

.bar {
  height: 10px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.bar span {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--accent-2);
  transition: width 0.3s;
}

.bar span.up {
  background: var(--danger);
}

.bar span.down {
  background: var(--male);
}

.foot {
  padding-top: 6px;
  border-top: 1px solid var(--border);
  font-weight: 700;
}

.over {
  color: var(--danger);
}

.side {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.side h3 {
  margin: 6px 0 0;
}
</style>
