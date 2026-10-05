<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Combo, { type ComboOption } from "../components/Combo.vue";
import Sprite from "../components/Sprite.vue";
import StatRadar from "../components/StatRadar.vue";
import Tip from "../components/Tip.vue";
import TypeBadge from "../components/TypeBadge.vue";
import { closeRom, editCount, editor, openRom, speciesEdit } from "../editor";
import { addPaths, library } from "../library";
import { nav } from "../nav";
import { isRom, type BaseStats, type PokeTypeKey, type SpeciesEdit, type TypeTag } from "../types";

const STATS: { key: keyof BaseStats; label: string }[] = [
  { key: "hp", label: "PV" },
  { key: "attack", label: "Attaque" },
  { key: "defense", label: "Défense" },
  { key: "spAttack", label: "Atq. Spé." },
  { key: "spDefense", label: "Déf. Spé." },
  { key: "speed", label: "Vitesse" },
];

const openableRoms = computed(() => library.items.filter(isRom));
const readOnly = computed(() => !editor.data);

/** Recherche insensible à la casse et aux accents. */
const normalize = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

const typeTag = (index: number) => editor.data?.types.find((t) => t.index === index)?.tag;
const abilityName = (id: number) => editor.data?.abilities.find((a) => a.id === id)?.name;

const abilityOptions = computed<ComboOption[]>(() => (editor.data?.abilities ?? []).map((a) => ({ value: a.id, label: a.name, hint: `${a.id}` })));
const moveOptions = computed<ComboOption[]>(() => (editor.data?.moves ?? []).map((m) => ({ value: m.id, label: m.name, hint: `${m.id}` })));

// ---------- Liste ----------

const query = ref("");
const typeFilter = ref<PokeTypeKey | "">("");
const onlyModified = ref(false);
type SortKey = "id" | "name" | "total";
const sortKey = ref<SortKey>("id");

interface Row {
  id: number;
  name: string;
  types: TypeTag[];
  total: number;
  abilities: string[];
  modified: boolean;
}

const allRows = computed<Row[]>(() =>
  (editor.overview?.species ?? []).map((s) => {
    const d = editor.edits[s.id];
    if (!d) return { id: s.id, name: s.name, types: s.types, total: s.total, abilities: [...s.abilities, s.hiddenAbility ?? ""], modified: false };
    return {
      id: s.id,
      name: s.name,
      types: [...new Set(d.types)].map(typeTag).filter((t): t is TypeTag => !!t),
      total: d.stats.reduce((a, b) => a + b, 0),
      abilities: d.abilities.map((a) => abilityName(a) ?? ""),
      modified: true,
    };
  }),
);

const types = computed(() => {
  const seen = new Map<PokeTypeKey, string>();
  for (const r of allRows.value) for (const t of r.types) seen.set(t.key, t.name);
  return [...seen].sort((a, b) => a[1].localeCompare(b[1], "fr"));
});

const rows = computed(() => {
  const q = normalize(query.value.trim());
  const list = allRows.value.filter(
    (r) =>
      (!q || normalize(r.name).includes(q) || String(r.id) === q || r.abilities.some((a) => normalize(a).includes(q))) &&
      (!typeFilter.value || r.types.some((t) => t.key === typeFilter.value)) &&
      (!onlyModified.value || r.modified),
  );
  const by = sortKey.value;
  return [...list].sort((a, b) => (by === "name" ? a.name.localeCompare(b.name, "fr") : by === "total" ? b.total - a.total : 0) || a.id - b.id);
});

const selectedId = ref(1);
const selected = computed(() => editor.overview?.species.find((s) => s.id === selectedId.value) ?? editor.overview?.species[0]);
const listEl = ref<HTMLElement | null>(null);

function select(id: number) {
  selectedId.value = id;
  nextTick(() => listEl.value?.querySelector<HTMLElement>(".item.on")?.scrollIntoView({ block: "nearest" }));
}

/** Pokémon voisin dans la liste filtrée et triée. */
const neighbour = (step: number) => {
  const i = rows.value.findIndex((r) => r.id === selectedId.value);
  return i < 0 ? null : (rows.value[i + step] ?? null);
};

watch(
  () => editor.overview?.path,
  () => (selectedId.value = 1),
);

// ---------- Fiche ----------

const original = computed(() => editor.data?.species[selectedId.value - 1]);
const cur = computed(() => speciesEdit(selectedId.value));
const modified = computed(() => !!editor.edits[selectedId.value]);

/** Valeurs affichées (statistiques et types) même en lecture seule. */
const shownStats = computed<number[]>(() => {
  if (cur.value) return cur.value.stats;
  const b = selected.value?.baseStats;
  return b ? STATS.map((s) => b[s.key]) : [0, 0, 0, 0, 0, 0];
});
const radarStats = computed<BaseStats>(() => Object.fromEntries(STATS.map((s, i) => [s.key, shownStats.value[i]])) as unknown as BaseStats);
const total = computed(() => shownStats.value.reduce((a, b) => a + b, 0));
const originalTotal = computed(() => original.value?.stats.reduce((a, b) => a + b, 0) ?? total.value);
const shownTypes = computed(() => allRows.value.find((r) => r.id === selectedId.value)?.types ?? []);

const same = (a: SpeciesEdit, b: SpeciesEdit) => JSON.stringify(a) === JSON.stringify(b);

/** Modifie l'espèce affichée ; revenir aux valeurs d'origine efface la modification. */
function mutate(fn: (d: SpeciesEdit) => void) {
  const base = cur.value;
  const orig = original.value;
  if (!base || !orig || readOnly.value) return;
  const d: SpeciesEdit = JSON.parse(JSON.stringify(base));
  fn(d);
  if (same(d, orig)) delete editor.edits[d.id];
  else editor.edits[d.id] = d;
}

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, Math.round(v) || min));
const numberOf = (e: Event) => Number((e.target as HTMLInputElement).value);

function setStat(i: number, e: Event) {
  mutate((d) => (d.stats[i] = clamp(numberOf(e), 1, 255)));
}

/** Types proposés : « ??? » (Gen 4, inutilisé par les espèces) seulement s'il est déjà en place. */
const typeOptions = (current: number) => (editor.data?.types ?? []).filter((t) => t.tag.key !== "mystery" || t.index === current);

const secondType = computed(() => (cur.value && cur.value.types[1] !== cur.value.types[0] ? cur.value.types[1] : -1));

function setType(slot: 0 | 1, e: Event) {
  const v = Number((e.target as HTMLSelectElement).value);
  mutate((d) => {
    if (slot === 0) {
      const mono = d.types[0] === d.types[1];
      d.types = [v, mono ? v : d.types[1]];
    } else {
      d.types[1] = v < 0 ? d.types[0] : v;
    }
  });
}

// Un second talent identique au premier équivaut à « aucun » : il suit le premier.
function setAbility(slot: number, v: number) {
  mutate((d) => {
    if (slot === 0 && d.abilities[1] === d.abilities[0]) d.abilities[1] = v;
    d.abilities[slot] = v;
  });
}

function setCatchRate(e: Event) {
  mutate((d) => (d.catchRate = clamp(numberOf(e), 1, 255)));
}

const byLevel = (d: SpeciesEdit) => d.learnset.sort((a, b) => a.level - b.level);

function setLevel(i: number, e: Event) {
  mutate((d) => {
    d.learnset[i].level = clamp(numberOf(e), 1, 100);
    byLevel(d);
  });
}

function setMove(i: number, v: number) {
  mutate((d) => (d.learnset[i].move = v));
}

function removeMove(i: number) {
  mutate((d) => d.learnset.splice(i, 1));
}

function addMove() {
  mutate((d) => {
    const last = d.learnset[d.learnset.length - 1];
    d.learnset.push({ level: last ? Math.min(100, last.level + 1) : 1, move: editor.data?.moves[0]?.id ?? 1 });
  });
}

function revert() {
  delete editor.edits[selectedId.value];
}

function revertAll() {
  if (confirm(`Annuler les modifications des ${editCount.value} Pokémon ?`)) editor.edits = {};
}

// ---------- Enregistrement ----------

const saving = ref(false);
const saveError = ref<string | null>(null);
const saved = ref<{ path: string; count: number } | null>(null);

async function saveRom() {
  const src = editor.overview?.path;
  if (!src || !editCount.value) return;
  const output = await save({
    title: "Enregistrer la ROM modifiée",
    defaultPath: `${src.replace(/\.nds$/i, "")} - modifiée.nds`,
    filters: [{ name: "ROM Nintendo DS", extensions: ["nds"] }],
  });
  if (!output) return;
  saving.value = true;
  saveError.value = null;
  try {
    const count = await invoke<number>("rom_editor_save", { path: src, edits: Object.values(editor.edits), output });
    const keep = selectedId.value;
    await addPaths([output]);
    // La suite de l'édition se fait sur la copie qui vient d'être écrite (sinon les
    // modifications restent en cours, pour un nouvel essai).
    if (await openRom(output, true)) {
      selectedId.value = keep;
      saved.value = { path: output, count };
    } else {
      saveError.value = `ROM enregistrée (${output}), mais impossible de la rouvrir : ${editor.error ?? "erreur inconnue"}`;
    }
  } catch (e) {
    saveError.value = String(e);
  } finally {
    saving.value = false;
  }
}

</script>

<template>
  <section class="editor">
    <!-- Aucune ROM ouverte -->
    <template v-if="!editor.overview && !editor.loadingPath">
      <h1>Éditeur de ROM</h1>
      <p class="lead">Modifie les Pokémon d'un jeu : statistiques, types, talents, taux de capture et attaques apprises.</p>

      <div v-if="editor.error" class="error">{{ editor.error }}</div>

      <div v-if="openableRoms.length" class="picker">
        <button v-for="rom in openableRoms" :key="rom.path" class="pick panel" @click="openRom(rom.path)">
          <span class="chip chip-accent">Gen {{ rom.generation }}</span>
          <strong>{{ rom.title }}</strong>
          <small>{{ rom.fileName }}</small>
        </button>
      </div>
      <div v-else class="empty panel">
        <p>Aucune ROM DS ou 3DS dans la bibliothèque.</p>
        <button class="btn btn-primary" @click="nav.view = 'home'">Ajouter une ROM</button>
      </div>
    </template>

    <!-- Chargement -->
    <div v-else-if="editor.loadingPath" class="loading">
      <div class="spinner" />
      <p>Lecture de la ROM…</p>
    </div>

    <template v-else-if="editor.overview">
      <header class="head">
        <div>
          <div class="chips">
            <span class="chip chip-accent">Gen {{ editor.overview.game.generation }}</span>
            <span class="chip">{{ editor.overview.gameCode }}</span>
            <span v-if="!editor.overview.verified" class="chip warn" title="Emplacements des données pas encore vérifiés sur ce jeu">Non vérifié</span>
            <span v-if="readOnly" class="chip">Lecture seule</span>
          </div>
          <h1>{{ editor.overview.game.name }}</h1>
        </div>
        <div class="actions">
          <button class="btn" @click="closeRom">Changer de ROM</button>
          <button v-if="editCount" class="btn" @click="revertAll">Tout annuler</button>
          <button class="btn btn-primary" :disabled="!editCount || saving" @click="saveRom">
            {{ saving ? "Enregistrement…" : "Enregistrer la ROM" }}
            <span v-if="editCount" class="count-badge">{{ editCount }}</span>
          </button>
        </div>
      </header>

      <p v-if="readOnly" class="notice">
        L'édition de ce jeu n'est pas encore disponible : seuls Platine, Noire et Blanche sont modifiables pour l'instant. Tu peux toujours consulter son Pokédex.
      </p>
      <div v-if="saveError" class="error">{{ saveError }}</div>
      <div v-else-if="editor.error" class="error">{{ editor.error }}</div>
      <div v-if="saved" class="success">
        <span>
          ROM enregistrée ({{ saved.count }} Pokémon modifié{{ saved.count > 1 ? "s" : "" }}). La suite de l'édition se fait sur cette copie ; l'originale n'a pas été touchée.
        </span>
        <button class="btn" @click="revealItemInDir(saved.path)">Afficher dans le dossier</button>
        <button class="close-x" aria-label="Fermer" @click="saved = null">×</button>
      </div>

      <div class="workspace">
        <!-- Liste des espèces -->
        <aside class="list panel">
          <div class="filters">
            <input v-model="query" class="field" type="search" placeholder="Nom, numéro, talent…" />
            <div class="filter-row">
              <select v-model="typeFilter" class="field">
                <option value="">Tous les types</option>
                <option v-for="[key, name] in types" :key="key" :value="key">{{ name }}</option>
              </select>
              <select v-model="sortKey" class="field" aria-label="Trier">
                <option value="id">N°</option>
                <option value="name">Nom</option>
                <option value="total">Total</option>
              </select>
            </div>
            <label v-if="editCount" class="only">
              <input v-model="onlyModified" type="checkbox" />
              Modifiés seulement ({{ editCount }})
            </label>
          </div>
          <div ref="listEl" class="items">
            <button v-for="r in rows" :key="r.id" class="item" :class="{ on: r.id === selectedId }" @click="select(r.id)">
              <Sprite :id="r.id" :size="48" class="icon" />
              <span class="item-main">
                <strong>{{ r.name }}</strong>
                <small>#{{ String(r.id).padStart(3, "0") }} · {{ r.types.map((t) => t.name).join(" / ") }}</small>
              </span>
              <span v-if="r.modified" class="dot" title="Modifié" />
              <span class="item-total">{{ r.total }}</span>
            </button>
            <p v-if="!rows.length" class="none">Aucun Pokémon</p>
          </div>
        </aside>

        <!-- Fiche -->
        <div v-if="selected" class="sheet">
          <div class="hero panel">
            <div class="halo">
              <Sprite :id="selected.id" variant="model" :size="140" />
            </div>
            <div class="hero-main">
              <span class="id">#{{ String(selected.id).padStart(3, "0") }}</span>
              <h2>
                {{ selected.name }}
                <span v-if="modified" class="chip chip-mod">Modifié</span>
              </h2>
              <div v-if="cur && editor.data" class="type-pickers">
                <select class="field" :value="cur.types[0]" aria-label="Type 1" @change="setType(0, $event)">
                  <option v-for="t in typeOptions(cur.types[0])" :key="t.index" :value="t.index">{{ t.tag.name }}</option>
                </select>
                <select class="field" :value="secondType" aria-label="Type 2" @change="setType(1, $event)">
                  <option :value="-1">Aucun second type</option>
                  <option v-for="t in typeOptions(cur.types[1]).filter((t) => t.index !== cur!.types[0])" :key="t.index" :value="t.index">{{ t.tag.name }}</option>
                </select>
              </div>
              <div v-else class="types"><TypeBadge v-for="t in shownTypes" :key="t.key" :type="t" /></div>
            </div>
            <div class="hero-nav">
              <button v-if="modified" class="btn" title="Revenir aux valeurs du jeu d'origine" @click="revert">Rétablir l'original</button>
              <button class="btn" :disabled="!neighbour(-1)" title="Pokémon précédent" @click="select(neighbour(-1)!.id)">‹</button>
              <button class="btn" :disabled="!neighbour(1)" title="Pokémon suivant" @click="select(neighbour(1)!.id)">›</button>
            </div>
          </div>

          <div class="cards">
            <section class="card panel stats-card">
              <h3>Statistiques de base</h3>
              <div class="stats-body">
                <div class="stat-rows">
                  <div v-for="(s, i) in STATS" :key="s.key" class="stat-row" :class="{ changed: original && original.stats[i] !== shownStats[i] }">
                    <span class="stat-label">{{ s.label }}</span>
                    <input
                      type="range"
                      min="1"
                      max="255"
                      class="slider"
                      :value="shownStats[i]"
                      :disabled="readOnly"
                      :style="{ '--pct': `${(shownStats[i] / 255) * 100}%`, '--hue': Math.min(190, (shownStats[i] / 150) * 190) }"
                      :aria-label="s.label"
                      @input="setStat(i, $event)"
                    />
                    <input type="number" min="1" max="255" class="field num" :value="shownStats[i]" :disabled="readOnly" :aria-label="s.label" @change="setStat(i, $event)" />
                  </div>
                  <div class="stat-row total-row">
                    <span class="stat-label">Total</span>
                    <span />
                    <strong class="num-total">
                      {{ total }}
                      <small v-if="total !== originalTotal" :class="total > originalTotal ? 'up' : 'down'">
                        {{ total > originalTotal ? "+" : "" }}{{ total - originalTotal }}
                      </small>
                    </strong>
                  </div>
                </div>
                <StatRadar :stats="radarStats" :size="200" class="radar" />
              </div>
            </section>

            <section class="card panel">
              <h3>Talents et capture</h3>
              <div v-if="cur" class="form">
                <label>
                  <span>Talent 1</span>
                  <Combo :model-value="cur.abilities[0]" :options="abilityOptions" @update:model-value="setAbility(0, $event)" />
                </label>
                <label>
                  <span>Talent 2</span>
                  <Combo :model-value="cur.abilities[1] === cur.abilities[0] ? 0 : cur.abilities[1]" :options="abilityOptions" none-label="Aucun" @update:model-value="setAbility(1, $event)" />
                </label>
                <label v-if="editor.data?.hiddenAbility">
                  <span>Talent caché <Tip title="Talent caché" text="Troisième talent, obtenu surtout via le Monde des Rêves (Pokémon Global Link) et certaines rencontres spéciales." /></span>
                  <Combo :model-value="cur.abilities[2]" :options="abilityOptions" none-label="Aucun" @update:model-value="setAbility(2, $event)" />
                </label>
                <label>
                  <span>
                    Taux de capture
                    <Tip
                      title="Taux de capture"
                      text="De 1 à 255 : plus il est haut, plus le Pokémon est facile à attraper. Repères : 3 pour les légendaires, 45 pour les starters, 255 pour Rattata ou Chenipan."
                    />
                  </span>
                  <input type="number" min="1" max="255" class="field" :value="cur.catchRate" @change="setCatchRate" />
                </label>
              </div>
              <dl v-else class="facts">
                <dt>Talents</dt>
                <dd>{{ selected.abilities.join(" / ") || "—" }}</dd>
                <template v-if="selected.hiddenAbility">
                  <dt>Talent caché</dt>
                  <dd>{{ selected.hiddenAbility }}</dd>
                </template>
                <dt>Taux de capture</dt>
                <dd>{{ selected.catchRate }}</dd>
              </dl>
            </section>

            <section v-if="cur && editor.data" class="card panel moves-card">
              <div class="card-head">
                <h3>Attaques apprises par niveau</h3>
                <span class="dim">{{ cur.learnset.length }} / {{ editor.data.maxLearnset }}</span>
              </div>
              <div class="moves">
                <div v-for="(m, i) in cur.learnset" :key="`${i}-${m.level}-${m.move}`" class="move-row">
                  <label class="lvl">
                    <span>Niv.</span>
                    <input type="number" min="1" max="100" class="field" :value="m.level" aria-label="Niveau" @change="setLevel(i, $event)" />
                  </label>
                  <Combo :model-value="m.move" :options="moveOptions" @update:model-value="setMove(i, $event)" />
                  <button class="remove" title="Retirer cette attaque" @click="removeMove(i)">×</button>
                </div>
              </div>
              <button class="btn add" :disabled="cur.learnset.length >= editor.data.maxLearnset" @click="addMove">+ Ajouter une attaque</button>
            </section>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.editor {
  max-width: 1680px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

h3 {
  margin: 0 0 14px;
  font-size: 15px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
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

.error,
.notice,
.success {
  margin-top: 16px;
  padding: 12px 16px;
  border-radius: var(--radius-sm);
}

.error {
  border: 1px solid var(--danger);
  color: var(--danger);
}

.notice {
  border: 1px solid var(--warn);
  background: var(--warn-bg);
  color: var(--text);
}

.success {
  display: flex;
  align-items: center;
  gap: 12px;
  border: 1px solid color-mix(in srgb, var(--accent-2) 60%, transparent);
  background: color-mix(in srgb, var(--accent-2) 12%, transparent);
}

.success span {
  flex: 1;
}

.close-x {
  border: none;
  background: none;
  color: var(--text-dim);
  font-size: 20px;
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
  flex-wrap: wrap;
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

.actions {
  display: flex;
  gap: 8px;
}

.count-badge {
  min-width: 22px;
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg) 35%, transparent);
  font-size: 12px;
  font-weight: 700;
}

.field {
  padding: 9px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font: inherit;
  outline: none;
  min-width: 0;
}

.field:focus {
  border-color: var(--accent);
}

.field:disabled {
  opacity: 0.7;
}

/* ---------- Espace de travail : liste + fiche ---------- */

.workspace {
  display: grid;
  grid-template-columns: clamp(280px, 22vw, 340px) minmax(0, 1fr);
  gap: 20px;
  align-items: start;
  margin-top: 20px;
}

.list {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  max-height: calc(100vh - 84px);
  overflow: hidden;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-bottom: 1px solid var(--border);
}

.filter-row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px;
}

.only {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-dim);
  font-size: 13px;
}

.items {
  overflow-y: auto;
  padding: 6px;
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 46px;
  padding: 4px 10px 4px 4px;
  border: none;
  border-radius: 8px;
  background: none;
  color: var(--text);
  text-align: left;
}

.item:hover {
  background: var(--panel-hover);
}

.item.on {
  background: var(--panel-hover);
  box-shadow: inset 3px 0 0 var(--accent-2);
}

/* Les icônes pokesprite ont beaucoup de marge transparente. */
.item .icon {
  flex: none;
  margin: -8px -6px -6px -4px;
}

.item-main {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.item-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-main small {
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent-2);
  box-shadow: 0 0 8px var(--accent-2);
}

.item-total {
  color: var(--text-dim);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.none {
  padding: 16px;
  color: var(--text-dim);
  text-align: center;
}

/* ---------- Fiche ---------- */

.sheet {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.hero {
  display: flex;
  align-items: center;
  gap: 20px;
  padding: 16px 20px;
}

.halo {
  display: grid;
  flex: none;
  place-items: center;
  width: 150px;
  height: 150px;
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--accent-2) 30%, transparent), transparent 70%);
  border: 2px solid color-mix(in srgb, var(--text) 15%, transparent);
}

.hero-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.hero-main h2 {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 28px;
}

.chip-mod {
  color: var(--accent-2);
  border-color: var(--accent-2);
}

.id {
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.type-pickers {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.type-pickers .field {
  min-width: 170px;
}

.types {
  display: flex;
  gap: 4px;
}

.hero-nav {
  display: flex;
  align-self: flex-start;
  gap: 8px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(440px, 1fr));
  gap: 16px;
  align-items: start;
}

.card {
  padding: 18px 20px;
}

.card-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.stats-body {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.stat-rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-width: 260px;
}

.stat-row {
  display: grid;
  grid-template-columns: 76px 1fr 70px;
  align-items: center;
  gap: 12px;
}

.stat-label {
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
}

.stat-row.changed .stat-label {
  color: var(--accent-2);
}

.num {
  padding: 6px 8px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  appearance: none;
  background: linear-gradient(to right, hsl(var(--hue) 80% 55%) var(--pct), color-mix(in srgb, var(--text) 14%, transparent) var(--pct));
  cursor: pointer;
}

.slider:disabled {
  cursor: default;
}

.slider::-webkit-slider-thumb {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  appearance: none;
  background: var(--text);
  box-shadow: 0 0 0 3px color-mix(in srgb, hsl(var(--hue) 80% 55%) 45%, transparent);
}

.slider:disabled::-webkit-slider-thumb {
  visibility: hidden;
}

.total-row {
  margin-top: 4px;
  padding-top: 8px;
  border-top: 1px solid var(--border);
}

.num-total {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.num-total small {
  display: block;
  font-size: 12px;
}

.up {
  color: #4ade80;
}

.down {
  color: var(--danger);
}

.radar {
  flex: none;
  margin: 0 auto;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form label {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form label > span {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
}

.facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 14px;
  margin: 0;
}

.facts dt {
  color: var(--text-dim);
}

.facts dd {
  margin: 0;
  font-weight: 600;
}

.moves-card {
  grid-column: 1 / -1;
}

.moves {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 8px 16px;
}

.move-row {
  display: grid;
  grid-template-columns: 96px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 8px;
}

.lvl {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
}

.lvl .field {
  width: 58px;
  padding: 7px 8px;
  text-align: right;
}

.remove {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: none;
  color: var(--text-dim);
  font-size: 18px;
}

.remove:hover {
  background: color-mix(in srgb, var(--danger) 18%, transparent);
  color: var(--danger);
}

.add {
  margin-top: 12px;
}

@media (max-width: 900px) {
  .workspace {
    grid-template-columns: 1fr;
  }

  .list {
    position: static;
    max-height: 320px;
  }
}
</style>
