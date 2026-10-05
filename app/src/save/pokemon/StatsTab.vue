<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { lists } from "../../saveStore";
import type { SlotView } from "../../types";
import { hiddenPowerType, natureEffect, STAT_LABELS } from "../refdata";
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
const hp = computed(() => lists.types[hiddenPowerType(props.p.ivs)] ?? "");

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
        <span />
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
        <span class="bar"><span :style="{ width: `${((p.stats?.[i] ?? 0) / maxStat) * 100}%` }" :class="{ up: effect?.up === i, down: effect?.down === i }" /></span>
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
      <p class="sv-help">Puissance Cachée : <strong>{{ hp }}</strong> <Tip term="hiddenPower" /></p>

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
  font-size: 11px;
  font-weight: 600;
}

.name.up {
  color: #ff9a8a;
}

.name.down {
  color: #8ac2ff;
}

.dim {
  color: var(--text-dim);
}

.sv-input.c {
  padding: 7px 6px;
}

.perfect {
  border-color: #7cf0a4 !important;
}

.total {
  font-size: 16px;
  font-variant-numeric: tabular-nums;
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
  background: #ff9a8a;
}

.bar span.down {
  background: #8ac2ff;
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
