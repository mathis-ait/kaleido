<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { lists, notify, saveState } from "../../saveStore";
import type { SaveView } from "../../types";

interface DexEntry {
  species: number;
  seen: boolean;
  caught: boolean;
}

const entries = ref<DexEntry[]>([]);
const loading = ref(true);
const search = ref("");
const filter = ref<"all" | "caught" | "seen" | "unseen">("all");

async function load() {
  loading.value = true;
  const raw = await invoke<DexEntry[]>("save_dex").catch((e) => {
    saveState.error = String(e);
    return [];
  });
  // Un Pokémon capturé est forcément vu (certains jeux ne gardent que le drapeau « capturé »).
  entries.value = raw.map((e) => ({ ...e, seen: e.seen || e.caught }));
  loading.value = false;
}
onMounted(load);

const nameById = computed(() => Object.fromEntries(lists.species.map((s) => [s.value, s.label])) as Record<number, string>);
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const shown = computed(() => {
  const q = fold(search.value.trim());
  return entries.value.filter((e) => {
    if (filter.value === "caught" && !e.caught) return false;
    if (filter.value === "seen" && !(e.seen && !e.caught)) return false;
    if (filter.value === "unseen" && e.seen) return false;
    if (!q) return true;
    return String(e.species) === q || fold(nameById.value[e.species] ?? "").includes(q);
  });
});
const seen = computed(() => entries.value.filter((e) => e.seen || e.caught).length);
const caught = computed(() => entries.value.filter((e) => e.caught).length);

async function done(view: SaveView | undefined) {
  if (!view) return;
  saveState.view = view;
  saveState.dirty = true;
  await load();
}

/** Clic : non vu → vu → capturé → non vu. */
async function cycle(e: DexEntry) {
  const next = !e.seen ? { seen: true, caught: false } : !e.caught ? { seen: true, caught: true } : { seen: false, caught: false };
  await done(await invoke<SaveView>("save_set_dex", { entries: [{ species: e.species, ...next }] }).catch((err) => void (saveState.error = String(err))));
}

async function all(seenFlag: boolean, caughtFlag: boolean, label: string) {
  await done(await invoke<SaveView>("save_dex_all", { seen: seenFlag, caught: caughtFlag }).catch((err) => void (saveState.error = String(err))));
  notify(label);
}

/** Marque capturés les Pokémon visibles (après filtre / recherche). */
async function catchShown() {
  const list = shown.value.filter((e) => !e.caught).map((e) => ({ species: e.species, seen: true, caught: true }));
  if (!list.length) return;
  await done(await invoke<SaveView>("save_set_dex", { entries: list }).catch((err) => void (saveState.error = String(err))));
  notify(`${list.length} Pokémon marqués capturés`);
}
</script>

<template>
  <div class="dex sv-panel">
    <div class="top">
      <div class="counter">
        <small>Vus <Tip term="seen" /></small>
        <strong>{{ seen }}</strong><span>/ {{ entries.length }}</span>
        <i :style="{ width: `${(seen / Math.max(1, entries.length)) * 100}%` }" />
      </div>
      <div class="counter red">
        <small>Capturés <Tip term="caught" /></small>
        <strong>{{ caught }}</strong><span>/ {{ entries.length }}</span>
        <i :style="{ width: `${(caught / Math.max(1, entries.length)) * 100}%` }" />
      </div>
      <div class="sv-row actions">
        <button class="sv-btn" @click="all(true, false, 'Tous les Pokémon sont vus')"><Icon name="search" :size="14" /> Tout vu</button>
        <button class="sv-btn" @click="all(true, true, 'Pokédex complété')"><Icon name="star" :size="14" /> Tout capturé</button>
        <button class="sv-btn" :disabled="!shown.length" @click="catchShown">Capturer la sélection</button>
        <button class="sv-btn danger" @click="all(false, false, 'Pokédex vidé')"><Icon name="trash" :size="14" /> Vider</button>
      </div>
    </div>

    <div class="filters">
      <input v-model="search" class="sv-input" placeholder="Chercher par nom ou numéro…" />
      <div class="sv-seg">
        <button :class="{ on: filter === 'all' }" @click="filter = 'all'">Tous</button>
        <button :class="{ on: filter === 'caught' }" @click="filter = 'caught'">Capturés</button>
        <button :class="{ on: filter === 'seen' }" @click="filter = 'seen'">Vus seulement</button>
        <button :class="{ on: filter === 'unseen' }" @click="filter = 'unseen'">Pas vus</button>
      </div>
    </div>

    <p v-if="loading" class="sv-help">Chargement…</p>
    <div v-else class="grid">
      <button
        v-for="e in shown"
        :key="e.species"
        class="entry"
        :class="{ seen: e.seen, caught: e.caught }"
        :title="`${nameById[e.species] ?? e.species} — ${e.caught ? 'capturé' : e.seen ? 'vu' : 'pas vu'} (clic pour changer)`"
        @click="cycle(e)"
      >
        <span class="num">#{{ String(e.species).padStart(4, "0") }}</span>
        <span v-if="e.caught" class="ball" />
        <Sprite :id="e.species" :size="56" :class="{ ghost: !e.seen }" />
        <small>{{ nameById[e.species] ?? `n°${e.species}` }}</small>
      </button>
    </div>
    <p class="sv-help foot">
      <Icon name="info" :size="14" /> Clic sur un Pokémon : pas vu → vu → capturé. Marquer « capturé » remplit aussi le sexe, la forme de base et la
      langue de ta partie, comme PKHeX.
    </p>
  </div>
</template>

<style scoped>
.dex {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 18px;
  overflow: hidden;
}

.top {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 14px;
}

.counter {
  position: relative;
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 10px 16px 14px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.counter small {
  display: flex;
  align-items: center;
  color: var(--text-dim);
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.counter strong {
  font-size: 26px;
}

.counter span {
  color: var(--text-dim);
}

.counter i {
  position: absolute;
  bottom: 0;
  left: 0;
  height: 4px;
  background: var(--accent-2);
}

.counter.red i {
  background: #ff6b6b;
}

.actions {
  margin-left: auto;
}

.filters {
  display: flex;
  gap: 10px;
}

.filters .sv-input {
  flex: 1;
}

.grid {
  display: grid;
  flex: 1;
  grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
  align-content: start;
  gap: 8px;
  min-height: 0;
  overflow-y: auto;
  padding: 2px;
}

.entry {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 18px 4px 8px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 4%, transparent);
  color: var(--text-dim);
}

.entry.seen {
  background: color-mix(in srgb, var(--text) 10%, transparent);
  color: var(--text);
}

.entry.caught {
  border-color: color-mix(in srgb, var(--text) 45%, transparent);
}

.entry small {
  max-width: 100%;
  overflow: hidden;
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.num {
  position: absolute;
  top: 5px;
  left: 8px;
  font-size: 10px;
  font-weight: 700;
  opacity: 0.8;
}

.ball {
  position: absolute;
  top: 6px;
  right: 8px;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: linear-gradient(#ff5a5a 0 45%, #222 45% 55%, #fff 55%);
}

.ghost {
  filter: brightness(0) opacity(0.25);
}

.foot {
  display: flex;
  align-items: center;
  gap: 6px;
}
</style>
