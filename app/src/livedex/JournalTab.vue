<script setup lang="ts">
import { computed, ref } from "vue";
import SearchField from "../components/SearchField.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import { BALLS, dex, fold, frDate } from "./data";
import { entries, livedex, manualOf, removeManual, undoRemove } from "./store";
import type { CatchEntry } from "./types";
import { openEntry, openSpecies } from "./ui";

type Origin = "all" | "auto" | "manual";
type Sort = "recent" | "number" | "name";

const query = ref("");
const origin = ref<Origin>("all");
const game = ref("");
const sort = ref<Sort>("recent");
const selected = ref(new Set<string>());

const name = (e: CatchEntry) => dex.value?.form(e.species, e.form)?.full ?? dex.value?.species(e.species)?.name ?? `#${e.species}`;
const gameName = (id: string | null) => (id ? (dex.value?.game(id)?.short ?? id) : "—");

const gamesUsed = computed(() => {
  const ids = new Set(entries.value.map((e) => e.game).filter((g): g is string => !!g));
  return (dex.value?.games ?? []).filter((g) => ids.has(g.id));
});

const list = computed(() => {
  const q = fold(query.value.trim());
  const out = entries.value.filter((e) => {
    if (origin.value === "auto" && !e.auto) return false;
    if (origin.value === "manual" && e.auto) return false;
    if (game.value && e.game !== game.value) return false;
    if (q && !fold(`${name(e)} ${e.nickname ?? ""} ${e.ot ?? ""} ${e.location ?? ""}`).includes(q)) return false;
    return true;
  });
  if (sort.value === "number") out.sort((a, b) => a.species - b.species || a.form - b.form);
  else if (sort.value === "name") out.sort((a, b) => name(a).localeCompare(name(b), "fr"));
  else out.sort((a, b) => (b.date ?? "").localeCompare(a.date ?? "") || a.species - b.species);
  return out;
});

function toggle(id: string, on: boolean) {
  const s = new Set(selected.value);
  if (on) s.add(id);
  else s.delete(id);
  selected.value = s;
}

const manualShown = computed(() => list.value.filter((e) => !e.auto));
const allSelected = computed(() => manualShown.value.length > 0 && manualShown.value.every((e) => selected.value.has(e.id)));
function toggleAll(on: boolean) {
  selected.value = on ? new Set(manualShown.value.map((e) => e.id)) : new Set();
}

function removeSelected() {
  removeManual([...selected.value]);
  selected.value = new Set();
}

function open(e: CatchEntry) {
  if (e.auto) openSpecies(e.species, e.form);
  else openEntry({ ...manualOf(e.id) });
}
</script>

<template>
  <div class="journal">
    <div class="filters">
      <SearchField v-model="query" placeholder="Pokémon, surnom, dresseur, lieu…" />
      <Segmented
        v-model="origin"
        :options="[
          { value: 'all', label: 'Tous' },
          { value: 'auto', label: 'Lus', hint: 'Trouvés dans tes sauvegardes' },
          { value: 'manual', label: 'Notés', hint: 'Notés à la main' },
        ]"
        label="Origine"
      />
      <select v-model="game" class="sv-select" aria-label="Jeu">
        <option value="">Tous les jeux</option>
        <option v-for="g in gamesUsed" :key="g.id" :value="g.id">{{ g.name }}</option>
      </select>
      <select v-model="sort" class="sv-select" aria-label="Tri">
        <option value="recent">Plus récents</option>
        <option value="number">Numéro</option>
        <option value="name">Nom</option>
      </select>
    </div>

    <div v-if="livedex.lastRemoved?.length" class="undo sv-panel" role="status">
      <span>{{ livedex.lastRemoved.length }} Pokémon supprimé{{ livedex.lastRemoved.length > 1 ? "s" : "" }}.</span>
      <button type="button" class="sv-btn small" @click="undoRemove">Annuler</button>
    </div>

    <div class="actions">
      <label v-if="manualShown.length" class="check">
        <input type="checkbox" :checked="allSelected" @change="toggleAll(($event.target as HTMLInputElement).checked)" />
        Tout sélectionner (notés à la main)
      </label>
      <button v-if="selected.size" type="button" class="sv-btn small danger" @click="removeSelected">Supprimer la sélection ({{ selected.size }})</button>
      <span class="dim count">{{ list.length.toLocaleString("fr-FR") }} Pokémon</span>
    </div>

    <p v-if="!list.length" class="dim empty">Rien à afficher.</p>

    <table v-else class="table">
      <thead>
        <tr>
          <th class="sel" />
          <th>Pokémon</th>
          <th>Jeu</th>
          <th>Lieu</th>
          <th>Ball</th>
          <th class="n">Niv.</th>
          <th>Depuis</th>
          <th>Origine</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="e in list" :key="e.id" @click="open(e)">
          <td class="sel" @click.stop>
            <input v-if="!e.auto" type="checkbox" :checked="selected.has(e.id)" :aria-label="`Sélectionner ${name(e)}`" @change="toggle(e.id, ($event.target as HTMLInputElement).checked)" />
          </td>
          <td class="poke">
            <Sprite :id="e.species" :form="e.form" :shiny="e.shiny" :size="36" />
            <span>
              <strong>{{ e.nickname || name(e) }}</strong><span v-if="e.shiny" class="shiny" title="Chromatique"> ★</span>
              <small v-if="e.nickname" class="dim"> {{ name(e) }}</small>
            </span>
          </td>
          <td>{{ gameName(e.game) }}</td>
          <td class="dim">{{ e.location ?? "" }}</td>
          <td class="dim">{{ e.ball ? BALLS[e.ball] : "" }}</td>
          <td class="n">{{ e.level ?? "" }}</td>
          <td class="dim">{{ frDate(e.date) }}</td>
          <td>
            <span v-if="e.auto" class="sv-chip dim" :title="e.where?.map((w) => `${w.label} · ${w.place}`).join('\n')">Lu</span>
            <span v-else class="sv-chip accent">Noté</span>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.filters,
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2) var(--sp-3);
  margin-bottom: var(--sp-3);
}

.filters :deep(.sv-search) {
  flex: 1;
  min-width: 240px;
}

.filters .sv-select {
  width: auto;
}

.check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-sm);
}

.count {
  margin-left: auto;
  font-size: var(--fs-sm);
}

.dim {
  color: var(--text-dim);
}

.empty {
  margin-top: var(--sp-5);
  text-align: center;
}

.undo {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--sp-3);
  padding: 8px 12px;
}

.table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-sm);
}

th {
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-weight: 600;
  text-align: left;
}

td {
  padding: 4px 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
}

tbody tr {
  cursor: pointer;
}

tbody tr:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.sel {
  width: 28px;
}

.n {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.poke {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.shiny {
  color: var(--shiny);
}
</style>
