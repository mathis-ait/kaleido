<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Combo from "../../components/Combo.vue";
import Tip from "../../components/Tip.vue";
import { lists, saveState } from "../../saveStore";
import type { PkmDate, SlotView } from "../../types";
import { BALLS, pad, VERSIONS } from "../refdata";
import { apply, num } from "./edit";

const props = defineProps<{ p: SlotView }>();
const gen = computed(() => saveState.view!.generation);

const metLevel = ref(props.p.metLevel);
watch(
  () => props.p.metLevel,
  (v) => (metLevel.value = v),
);

const location = computed({ get: () => props.p.metLocation, set: (v) => v !== props.p.metLocation && apply({ metLocation: v }) });
const eggLocation = computed({ get: () => props.p.eggLocation, set: (v) => v !== props.p.eggLocation && apply({ eggLocation: v }) });
const locationOptions = computed(() => lists.locations.map((o) => ({ ...o, hint: String(o.value) })));
const versions = computed(() => VERSIONS.filter((v) => v.gen <= gen.value));
const balls = computed(() =>
  BALLS.filter((b) => b.since <= gen.value && (!b.until || b.until >= gen.value)).map((b) => ({
    ...b,
    name: lists.balls.find((x) => x.value === b.id)?.label ?? b.name,
  })),
);

const toInput = (d: PkmDate | null) => (d ? `${d.year}-${pad(d.month)}-${pad(d.day)}` : "");
function fromInput(v: string): PkmDate | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(v);
  return m ? { year: Number(m[1]), month: Number(m[2]), day: Number(m[3]) } : null;
}
function today(): PkmDate {
  const d = new Date();
  return { year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() };
}

function commitMetLevel() {
  const n = num(metLevel.value, 0, 100);
  if (n !== null && n !== props.p.metLevel) apply({ metLevel: n });
}

const hatched = computed(() => props.p.eggLocation !== 0 || !!props.p.eggDate);
function setHatched(on: boolean) {
  if (on) apply({ eggLocation: eggDefault.value, eggDate: props.p.metDate ?? today() });
  else apply({ eggLocation: 0, eggDate: null });
}
/** Lieu « Pension » par défaut, selon la génération (identifiants PKHeX). */
const eggDefault = computed(() => ({ 4: 2000, 5: 60002, 6: 60002, 7: 60002 })[gen.value] ?? 0);
</script>

<template>
  <div class="sv-grid">
    <div class="sv-field">
      <span class="sv-label">Jeu d'origine <Tip term="version" /></span>
      <select class="sv-select" :value="p.version" @change="apply({ version: Number(($event.target as HTMLSelectElement).value) })">
        <option v-if="!versions.some((v) => v.id === p.version)" :value="p.version">Version n°{{ p.version }}</option>
        <optgroup v-for="g in [3, 4, 5, 6, 7].filter((g) => g <= gen)" :key="g" :label="`Génération ${g}`">
          <option v-for="v in versions.filter((v) => v.gen === g)" :key="v.id" :value="v.id">{{ v.name }}</option>
        </optgroup>
      </select>
    </div>
    <div class="sv-field">
      <span class="sv-label">Lieu de rencontre <Tip term="metLocation" /></span>
      <Combo v-model="location" :options="locationOptions" placeholder="Chercher un lieu…" />
      <p v-if="!p.metLocationName && p.metLocation" class="sv-help">Lieu n°{{ p.metLocation }} (inconnu dans ce jeu)</p>
    </div>
    <div class="sv-field">
      <span class="sv-label">Niveau de rencontre <Tip term="metLevel" /></span>
      <input v-model="metLevel" class="sv-input" type="number" min="0" max="100" @change="commitMetLevel" />
    </div>
    <div class="sv-field">
      <span class="sv-label">Date de rencontre <Tip term="metDate" /></span>
      <div class="sv-row nowrap">
        <input class="sv-input" type="date" :value="toInput(p.metDate)" @change="apply({ metDate: fromInput(($event.target as HTMLInputElement).value) })" />
        <button class="sv-btn" title="Date du jour" @click="apply({ metDate: today() })">Aujourd'hui</button>
      </div>
    </div>

    <div class="sv-field wide">
      <span class="sv-label">Poké Ball <Tip term="ball" /></span>
      <div class="balls">
        <button
          v-for="b in balls"
          :key="b.id"
          class="ball-btn"
          :class="{ on: p.ball === b.id }"
          :title="b.name"
          @click="apply({ ball: b.id })"
        >
          <span class="ball" :style="{ '--ball': b.color }" />
          <small>{{ b.name }}</small>
        </button>
      </div>
    </div>

    <div class="sv-field">
      <span class="sv-label">Rencontre fatidique <Tip term="fateful" /></span>
      <label class="sv-switch">
        <input type="checkbox" :checked="p.fatefulEncounter" @change="apply({ fatefulEncounter: !p.fatefulEncounter })" />
        <span class="track" />
        {{ p.fatefulEncounter ? "Oui (événement)" : "Non" }}
      </label>
    </div>
    <div class="sv-field">
      <span class="sv-label">Éclos d'un œuf <Tip term="eggLocation" /></span>
      <label class="sv-switch">
        <input type="checkbox" :checked="hatched" @change="setHatched(!hatched)" />
        <span class="track" />
        {{ hatched ? "Oui" : "Non, capturé ou reçu" }}
      </label>
    </div>
    <template v-if="hatched">
      <div class="sv-field">
        <span class="sv-label">Lieu de l'œuf</span>
        <Combo v-model="eggLocation" :options="locationOptions" placeholder="Chercher un lieu…" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Date de l'œuf</span>
        <input class="sv-input" type="date" :value="toInput(p.eggDate)" @change="apply({ eggDate: fromInput(($event.target as HTMLInputElement).value) })" />
      </div>
    </template>
  </div>
</template>

<style scoped>
.nowrap {
  flex-wrap: nowrap;
}

.balls {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(118px, 1fr));
  gap: 6px;
}

.ball-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  text-align: left;
}

.ball-btn:hover {
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.ball-btn.on {
  border-color: var(--text);
  background: color-mix(in srgb, var(--text) 18%, transparent);
}

.ball-btn small {
  overflow: hidden;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.ball {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: linear-gradient(var(--ball) 0 44%, #222 44% 56%, #fff 56%);
  box-shadow: inset 0 0 0 1.5px #222;
}
</style>
