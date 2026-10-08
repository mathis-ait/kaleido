<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { editPokemon, loadBox, saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { checksFromReport, type Check } from "../checks";
import type { SlotReport } from "../legality";
import GroupLegalize from "./GroupLegalize.vue";

const all = ref<SlotView[] | null>(null);
const reports = ref<SlotReport[]>([]);
const onlyProblems = ref(true);
/** Aperçu groupé de « Tout rendre légal ». */
const groupOpen = ref(false);

async function scan() {
  all.value = null;
  const [views, reps] = await Promise.all([
    invoke<SlotView[]>("save_all").catch(() => [] as SlotView[]),
    invoke<SlotReport[]>("legality_check_all").catch(() => [] as SlotReport[]),
  ]);
  reports.value = reps;
  all.value = views;
}
onMounted(scan);

const key = (s: SlotView["slot"]) => JSON.stringify(s);

const results = computed(() => {
  const byslot = new Map(reports.value.map((r) => [key(r.slot), r]));
  return (all.value ?? []).map((p) => {
    const r = byslot.get(key(p.slot));
    const checks: Check[] = r ? checksFromReport(r).filter((c) => c.level !== "ok") : [];
    return { p, r, checks, errors: checks.filter((c) => c.level === "error").length };
  });
});
const shown = computed(() => results.value.filter((r) => !onlyProblems.value || r.checks.length).sort((a, b) => b.errors - a.errors));
const count = computed(() => ({
  total: results.value.length,
  errors: results.value.filter((r) => r.errors).length,
  warns: results.value.filter((r) => !r.errors && r.checks.length).length,
}));

async function open(p: SlotView) {
  if (p.slot.kind === "box") await loadBox(p.slot.box);
  editPokemon(p);
}

function fixAll() {
  if (count.value.errors) groupOpen.value = true;
}
const illegalSlots = computed(() => results.value.filter((r) => r.errors).map((r) => r.p.slot));

const where = (p: SlotView) => (p.slot.kind === "party" ? `Équipe ${p.slot.index + 1}` : `${saveState.view!.boxNames[p.slot.box]} · ${p.slot.index + 1}`);
</script>

<template>
  <div class="sv-panel wrap">
    <div class="sv-row head">
      <div class="stats">
        <span><strong>{{ count.total }}</strong> Pokémon</span>
        <span class="bad"><strong>{{ count.errors }}</strong> illégaux</span>
        <span class="warn"><strong>{{ count.warns }}</strong> douteux</span>
        <Tip term="legality" />
      </div>
      <label class="sv-switch">
        <input v-model="onlyProblems" type="checkbox" />
        <span class="track" />
        Seulement les problèmes
      </label>
      <button class="sv-btn" :disabled="!all" @click="scan"><Icon name="refresh" :size="15" /> Relancer</button>
      <button class="sv-btn solid" :disabled="!count.errors || groupOpen" @click="fixAll"><Icon name="wand" :size="15" /> Tout rendre légal</button>
      <Tip term="legalizeTeam" />
    </div>
    <p v-if="!all" class="sv-help">Analyse en cours…</p>
    <p v-else-if="!shown.length" class="sv-help">Aucun problème trouvé.</p>
    <div class="list">
      <button v-for="r in shown" :key="key(r.p.slot)" class="row" @click="open(r.p)">
        <Sprite :id="r.p.species" :shiny="r.p.shiny" :size="48" />
        <div class="who">
          <strong>{{ r.p.nickname || r.p.speciesName }}</strong>
          <small>{{ where(r.p) }} · N. {{ r.p.level }}</small>
          <small v-if="r.r" :class="['verdict', r.r.verdict]">{{ r.r.verdictLabel }}</small>
        </div>
        <ul>
          <li v-for="c in r.checks" :key="c.title + c.detail" :class="c.level" :title="c.detail">{{ c.title }}</li>
          <li v-if="!r.checks.length" class="ok">Légal</li>
        </ul>
        <Icon name="chevron-right" />
      </button>
    </div>
    <GroupLegalize v-model="groupOpen" :slots="illegalSlots" title="Tout rendre légal" @applied="scan" />
  </div>
</template>

<style scoped>
.wrap {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px;
  overflow: hidden;
}

.head {
  gap: 18px;
}

.stats {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-right: auto;
}

.bad {
  color: var(--danger);
}

.warn {
  color: var(--warn);
}

.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 12px;
  border: none;
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  text-align: left;
}

.row:hover {
  background: color-mix(in srgb, var(--text) 13%, transparent);
}

.who {
  display: flex;
  flex-direction: column;
  min-width: 180px;
}

.who small {
  color: var(--text-dim);
}

.who small.verdict.illegal {
  color: var(--danger);
  font-weight: 600;
}

.who small.verdict.fishy {
  color: var(--warn);
  font-weight: 600;
}

.row ul {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row li {
  padding: 2px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

li.error {
  background: color-mix(in srgb, var(--danger) 22%, transparent);
  color: var(--danger);
}

li.warn {
  background: var(--warn-bg);
  color: var(--warn);
}

li.ok {
  color: var(--ok);
}
</style>
