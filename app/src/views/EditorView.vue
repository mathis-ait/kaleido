<script setup lang="ts">
import { computed, ref } from "vue";
import TypeBadge from "../components/TypeBadge.vue";
import { editor, openRom } from "../editor";
import { library } from "../library";
import { nav } from "../nav";
import type { BaseStats, PokeTypeKey, Species } from "../types";

const query = ref("");
const typeFilter = ref<PokeTypeKey | "">("");
type SortKey = "id" | "name" | "total" | keyof BaseStats;
const sortKey = ref<SortKey>("id");
const sortDesc = ref(false);

const STATS: { key: keyof BaseStats; label: string }[] = [
  { key: "hp", label: "PV" },
  { key: "attack", label: "Att" },
  { key: "defense", label: "Déf" },
  { key: "spAttack", label: "Atq Spé" },
  { key: "spDefense", label: "Déf Spé" },
  { key: "speed", label: "Vit" },
];

const openableRoms = computed(() => library.items.filter((d) => d.kind === "nds_rom" && d.game));

const types = computed(() => {
  const seen = new Map<PokeTypeKey, string>();
  for (const s of editor.overview?.species ?? []) for (const t of s.types) seen.set(t.key, t.name);
  return [...seen].sort((a, b) => a[1].localeCompare(b[1], "fr"));
});

/** Recherche insensible à la casse et aux accents. */
const normalize = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

const rows = computed(() => {
  const q = normalize(query.value.trim());
  const list = (editor.overview?.species ?? []).filter(
    (s) =>
      (!q || normalize(s.name).includes(q) || String(s.id) === q || s.abilities.some((a) => normalize(a).includes(q))) &&
      (!typeFilter.value || s.types.some((t) => t.key === typeFilter.value)),
  );
  const value = (s: Species): number | string =>
    sortKey.value === "id" ? s.id : sortKey.value === "name" ? s.name : sortKey.value === "total" ? s.total : s.baseStats[sortKey.value];
  const dir = sortDesc.value ? -1 : 1;
  return [...list].sort((a, b) => {
    const [x, y] = [value(a), value(b)];
    return (typeof x === "string" ? x.localeCompare(y as string, "fr") : x - (y as number)) * dir || a.id - b.id;
  });
});

function sortBy(key: SortKey) {
  if (sortKey.value === key) {
    sortDesc.value = !sortDesc.value;
  } else {
    sortKey.value = key;
    // Les statistiques se lisent naturellement de la plus haute à la plus basse.
    sortDesc.value = key !== "id" && key !== "name";
  }
}

const arrow = (key: SortKey) => (sortKey.value === key ? (sortDesc.value ? "↓" : "↑") : "");
/** Couleur de la barre : rouge → jaune → vert → cyan selon la valeur. */
const statHue = (v: number) => Math.min(190, (v / 150) * 190);
</script>

<template>
  <section class="editor">
    <!-- Aucune ROM ouverte -->
    <template v-if="!editor.overview && !editor.loadingPath">
      <h1>Éditeur de ROM</h1>
      <p class="lead">Explore les données d'un jeu DS : Pokédex, statistiques, types et talents.</p>

      <div v-if="editor.error" class="error">{{ editor.error }}</div>

      <div v-if="openableRoms.length" class="picker">
        <button v-for="rom in openableRoms" :key="rom.path" class="pick panel" @click="openRom(rom.path)">
          <span class="chip chip-accent">Gen {{ rom.generation }}</span>
          <strong>{{ rom.title }}</strong>
          <small>{{ rom.fileName }}</small>
        </button>
      </div>
      <div v-else class="empty panel">
        <p>Aucune ROM DS dans la bibliothèque.</p>
        <button class="btn btn-primary" @click="nav.view = 'home'">Ajouter une ROM</button>
      </div>
    </template>

    <!-- Chargement -->
    <div v-else-if="editor.loadingPath" class="loading">
      <div class="spinner" />
      <p>Lecture de la ROM…</p>
    </div>

    <!-- Pokédex -->
    <template v-else-if="editor.overview">
      <header class="head">
        <div>
          <div class="chips">
            <span class="chip chip-accent">Gen {{ editor.overview.game.generation }}</span>
            <span class="chip">{{ editor.overview.gameCode }}</span>
            <span class="chip">{{ editor.overview.fileCount }} fichiers</span>
            <span v-if="!editor.overview.verified" class="chip warn" title="Emplacements des données pas encore vérifiés sur ce jeu">Non vérifié</span>
          </div>
          <h1>{{ editor.overview.game.name }}</h1>
        </div>
        <button class="btn" @click="editor.overview = null">Changer de ROM</button>
      </header>

      <div class="toolbar">
        <input v-model="query" class="search" type="search" placeholder="Rechercher un Pokémon, un numéro, un talent…" />
        <select v-model="typeFilter" class="select">
          <option value="">Tous les types</option>
          <option v-for="[key, name] in types" :key="key" :value="key">{{ name }}</option>
        </select>
        <span class="count">{{ rows.length }} Pokémon</span>
      </div>

      <div class="table panel">
        <div class="row header">
          <button @click="sortBy('id')">N° {{ arrow("id") }}</button>
          <button @click="sortBy('name')">Nom {{ arrow("name") }}</button>
          <span>Types</span>
          <button v-for="s in STATS" :key="s.key" class="num" @click="sortBy(s.key)">{{ s.label }} {{ arrow(s.key) }}</button>
          <button class="num" @click="sortBy('total')">Total {{ arrow("total") }}</button>
          <span>Talents</span>
        </div>
        <div v-for="p in rows" :key="p.id" class="row">
          <span class="id">#{{ String(p.id).padStart(3, "0") }}</span>
          <strong class="name">{{ p.name }}</strong>
          <span class="types"><TypeBadge v-for="t in p.types" :key="t.key" :type="t" /></span>
          <span v-for="s in STATS" :key="s.key" class="stat">
            <span class="bar" :style="{ width: `${Math.min(100, (p.baseStats[s.key] / 180) * 100)}%`, background: `hsl(${statHue(p.baseStats[s.key])} 75% 52%)` }" />
            <span class="val">{{ p.baseStats[s.key] }}</span>
          </span>
          <span class="num total">{{ p.total }}</span>
          <span class="abilities">
            {{ p.abilities.join(" / ") }}
            <em v-if="p.hiddenAbility" title="Talent caché">· {{ p.hiddenAbility }}</em>
          </span>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.editor {
  max-width: 1280px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

.lead {
  color: var(--text-dim);
  font-size: 16px;
}

.picker {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 14px;
  margin-top: 24px;
}

.pick {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 18px;
  text-align: left;
  transition: transform 0.15s;
}

.pick:hover {
  transform: translateY(-2px);
}

.pick strong {
  font-size: 17px;
}

.pick small {
  color: var(--text-dim);
}

.empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 24px;
  padding: 20px 24px;
}

.error {
  margin-top: 16px;
  padding: 12px 16px;
  border: 1px solid var(--danger);
  border-radius: var(--radius-sm);
  color: var(--danger);
}

.loading {
  display: grid;
  place-items: center;
  gap: 16px;
  margin-top: 120px;
  color: var(--text-dim);
}

.spinner {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: conic-gradient(var(--accent), var(--accent-2), var(--accent-3), transparent);
  mask: radial-gradient(circle, transparent 55%, #000 56%);
  animation: spin 0.9s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
}

.chips {
  display: flex;
  gap: 6px;
  margin-bottom: 8px;
}

.chip.warn {
  color: var(--warn);
  border-color: var(--warn);
}

.toolbar {
  position: sticky;
  top: -36px;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 20px 0 14px;
  padding: 10px 0;
  background: var(--bg);
}

.search,
.select {
  padding: 10px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--panel);
  color: var(--text);
  font: inherit;
  outline: none;
}

.search {
  flex: 1;
  max-width: 460px;
}

.search:focus,
.select:focus {
  border-color: var(--accent);
}

.count {
  margin-left: auto;
  color: var(--text-dim);
}

.table {
  overflow: hidden;
  padding: 6px;
}

.row {
  display: grid;
  grid-template-columns: 56px minmax(120px, 1.1fr) 170px repeat(6, minmax(64px, 0.6fr)) 56px minmax(180px, 1.4fr);
  align-items: center;
  gap: 10px;
  padding: 7px 10px;
  border-radius: 8px;
}

.row:not(.header):hover {
  background: var(--panel-hover);
}

.row.header {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.row.header button {
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  text-transform: inherit;
}

.row.header button:hover {
  color: var(--text);
}

.id {
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.types {
  display: flex;
  gap: 4px;
}

.stat {
  position: relative;
  height: 20px;
  border-radius: 5px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  overflow: hidden;
}

.bar {
  position: absolute;
  inset: 0 auto 0 0;
  opacity: 0.55;
}

.val {
  position: relative;
  padding-left: 6px;
  font-size: 12px;
  font-weight: 600;
  line-height: 20px;
  font-variant-numeric: tabular-nums;
}

.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.total {
  font-weight: 700;
}

.abilities {
  color: var(--text-dim);
  font-size: 13px;
}

.abilities em {
  font-style: normal;
  color: var(--accent-2);
}
</style>
