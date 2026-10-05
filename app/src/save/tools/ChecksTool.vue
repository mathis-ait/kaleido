<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { editPokemon, lists, loadBox, saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { checkPokemon } from "../checks";

const all = ref<SlotView[] | null>(null);
const onlyProblems = ref(true);

async function scan() {
  all.value = null;
  all.value = await invoke<SlotView[]>("save_all").catch(() => []);
}
onMounted(scan);

const results = computed(() =>
  (all.value ?? []).map((p) => {
    const checks = checkPokemon(p, saveState.view!.generation, lists.itemName).filter((c) => c.level !== "ok");
    return { p, checks, errors: checks.filter((c) => c.level === "error").length };
  }),
);
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

const where = (p: SlotView) => (p.slot.kind === "party" ? `Équipe ${p.slot.index + 1}` : `${saveState.view!.boxNames[p.slot.box]} · ${p.slot.index + 1}`);
</script>

<template>
  <div class="sv-panel wrap">
    <div class="sv-row head">
      <div class="stats">
        <span><strong>{{ count.total }}</strong> Pokémon</span>
        <span class="bad"><strong>{{ count.errors }}</strong> avec problème</span>
        <span class="warn"><strong>{{ count.warns }}</strong> à vérifier</span>
        <Tip term="legality" />
      </div>
      <label class="sv-switch">
        <input v-model="onlyProblems" type="checkbox" />
        <span class="track" />
        Seulement les problèmes
      </label>
      <button class="sv-btn" @click="scan"><Icon name="refresh" :size="15" /> Relancer</button>
    </div>
    <p v-if="!all" class="sv-help">Analyse en cours…</p>
    <p v-else-if="!shown.length" class="sv-help">Aucun problème trouvé.</p>
    <div class="list">
      <button v-for="r in shown" :key="JSON.stringify(r.p.slot)" class="row" @click="open(r.p)">
        <Sprite :id="r.p.species" :shiny="r.p.shiny" :size="48" />
        <div class="who">
          <strong>{{ r.p.nickname || r.p.speciesName }}</strong>
          <small>{{ where(r.p) }} · N. {{ r.p.level }}</small>
        </div>
        <ul>
          <li v-for="c in r.checks" :key="c.title" :class="c.level">{{ c.title }}</li>
          <li v-if="!r.checks.length" class="ok">Cohérent</li>
        </ul>
        <Icon name="chevron-right" />
      </button>
    </div>
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
  color: #8ff0b5;
}
</style>
