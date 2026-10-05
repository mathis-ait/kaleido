<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Combo, { type ComboOption } from "../../components/Combo.vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { editPokemon, loadBox, notify, saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { useShell } from "../shell";
import {
  GIFT_EXTENSIONS,
  IMPORTED_BASE,
  formatDate,
  giftsApi,
  isShiny,
  type GiftDetails as Details,
  type GiftKind,
  type GiftQuery,
  type GiftSort,
  type GiftSummary,
} from "./api";
import GiftDetails from "./GiftDetails.vue";
import { GIFT_TERMS } from "./terms";

/**
 * Page « Cadeaux mystère » : filtres à gauche, cartes au centre (chargées par pages au fil
 * du défilement), fiche de la carte choisie à droite.
 */

const PAGE = 120;

type Tri = "any" | "yes" | "no";
const filters = reactive({
  species: 0,
  text: "",
  shiny: "any" as Tri,
  egg: "any" as Tri,
  generations: [] as number[],
  kinds: [] as GiftKind[],
  onlyThisGame: false,
  sort: "default" as GiftSort,
});

const items = ref<GiftSummary[]>([]);
const total = ref(0);
const loading = ref(false);
const error = ref<string | null>(null);
const selected = ref<Details | null>(null);
const busy = ref(false);
const lastAdded = ref<SlotView | null>(null);
const speciesOptions = ref<ComboOption[]>([]);
const dbTotal = ref(0);
const grid = ref<HTMLElement | null>(null);

const hasSave = computed(() => !!saveState.view);
const saveGen = computed(() => saveState.view?.generation ?? null);

const triValue = (t: Tri) => (t === "any" ? null : t === "yes");
const query = computed<GiftQuery>(() => ({
  species: filters.species || null,
  text: filters.text,
  shiny: triValue(filters.shiny),
  egg: triValue(filters.egg),
  generations: filters.generations,
  kinds: filters.kinds.flatMap((k) => (k === "pokemon" ? (["pokemon", "egg"] as GiftKind[]) : [k])),
  versions: [],
  sort: filters.sort,
}));

let request = 0;
async function load(reset: boolean) {
  if (loading.value && !reset) return;
  const id = ++request;
  loading.value = true;
  error.value = null;
  try {
    const page = await giftsApi.search(query.value, filters.onlyThisGame && hasSave.value, reset ? 0 : items.value.length, PAGE);
    if (id !== request) return;
    total.value = page.total;
    items.value = reset ? page.items : [...items.value, ...page.items];
    if (reset) {
      grid.value?.scrollTo({ top: 0 });
      if (!selected.value && items.value[0]) select(items.value[0]);
    }
  } catch (e) {
    if (id === request) error.value = String(e);
  } finally {
    if (id === request) loading.value = false;
  }
}

let debounce: number | undefined;
watch(
  query,
  () => {
    clearTimeout(debounce);
    debounce = window.setTimeout(() => load(true), 160);
  },
  { deep: true },
);
watch(() => filters.onlyThisGame, () => load(true));

function onScroll() {
  const el = grid.value;
  if (!el || loading.value || items.value.length >= total.value) return;
  if (el.scrollTop + el.clientHeight > el.scrollHeight - 700) load(false);
}

async function select(g: GiftSummary) {
  try {
    selected.value = await giftsApi.details(g.id);
    lastAdded.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

function toggle<T>(list: T[], v: T) {
  const i = list.indexOf(v);
  if (i >= 0) list.splice(i, 1);
  else list.push(v);
}

function resetFilters() {
  Object.assign(filters, { species: 0, text: "", shiny: "any", egg: "any", generations: [], kinds: [], sort: "default" });
}

async function addToSave() {
  const g = selected.value;
  if (!g || busy.value || !hasSave.value || g.compatible === false) return;
  busy.value = true;
  try {
    const r = await giftsApi.add(g.id, null);
    saveState.view = r.view;
    saveState.dirty = true;
    if (r.slot?.kind === "box") await loadBox(r.slot.box);
    else await loadBox(saveState.box);
    lastAdded.value = r.pokemon;
    notify(r.warning ? `${r.message} — ${r.warning}` : r.message);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

function safeName(s: string) {
  return s.replace(/[\\/:*?"<>|]+/g, " ").replace(/\s+/g, " ").trim().slice(0, 60) || "cadeau";
}

async function exportGift() {
  const g = selected.value;
  if (!g) return;
  const name = `${g.cardId ? `${String(g.cardId).padStart(4, "0")} - ` : ""}${safeName(g.title)}.${g.extension}`;
  const output = await save({
    title: "Enregistrer la carte cadeau",
    defaultPath: name,
    filters: [{ name: `Carte ${g.formatLabel}`, extensions: [g.extension] }],
  });
  if (!output) return;
  try {
    await giftsApi.exportFile(g.id, output);
    notify(`Carte enregistrée : ${output.split(/[\\/]/).pop()}`);
  } catch (e) {
    error.value = String(e);
  }
}

async function importGift() {
  const file = await open({
    title: "Ouvrir une carte cadeau",
    filters: [
      { name: "Cartes cadeau (PCD, PGT, PGF, WC6, WC7)", extensions: GIFT_EXTENSIONS },
      { name: "Tous les fichiers", extensions: ["*"] },
    ],
  });
  if (typeof file !== "string") return;
  try {
    selected.value = await giftsApi.importFile(file);
    lastAdded.value = null;
    await load(true);
    notify(`Carte ouverte : ${selected.value.title}`);
  } catch (e) {
    error.value = String(e);
  }
}

function openInEditor() {
  if (lastAdded.value) editPokemon(lastAdded.value);
}

onMounted(async () => {
  try {
    const o = await giftsApi.overview();
    dbTotal.value = o.total;
    speciesOptions.value = o.species.map((s) => ({ value: s.value, label: s.label, hint: `${s.count}`, sprite: s.value }));
  } catch (e) {
    error.value = String(e);
  }
  // Par défaut : les cartes de la génération de la sauvegarde ouverte.
  if (saveGen.value) filters.generations = [saveGen.value];
  else load(true);
});

useShell(() => ({
  hint: selected.value ? selected.value.title : "Choisis une carte pour voir son contenu",
  actions: [
    { key: "Ctrl+i", cap: "Ctrl+I", label: "Ouvrir un fichier", run: importGift },
    { key: "Ctrl+e", cap: "Ctrl+E", label: "Enregistrer la carte", run: exportGift, disabled: !selected.value },
    {
      key: "Enter",
      cap: "Entrée",
      label: "Ajouter à la sauvegarde",
      run: addToSave,
      disabled: !selected.value || !hasSave.value || selected.value.compatible === false || busy.value,
    },
  ],
}));

const SORTS: { value: GiftSort; label: string }[] = [
  { value: "default", label: "Par défaut" },
  { value: "newest", label: "Plus récents" },
  { value: "species", label: "Espèce" },
  { value: "card", label: "N° de carte" },
  { value: "title", label: "Titre" },
];
const KINDS: { value: GiftKind; label: string }[] = [
  { value: "pokemon", label: "Pokémon" },
  { value: "item", label: "Objets" },
  { value: "other", label: "Autres" },
];
const TRI: { value: Tri; label: string }[] = [
  { value: "any", label: "Tous" },
  { value: "yes", label: "Oui" },
  { value: "no", label: "Non" },
];
</script>

<template>
  <div class="gifts">
    <!-- Filtres -->
    <aside class="filters sv-panel">
      <h2>
        Cadeaux mystère
        <Tip :title="GIFT_TERMS.mysteryGift.title" :text="GIFT_TERMS.mysteryGift.text" />
      </h2>
      <div class="sv-field">
        <span class="sv-label">Pokémon</span>
        <Combo v-model="filters.species" :options="speciesOptions" sprites none-label="Tous les Pokémon" placeholder="Espèce…" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Recherche</span>
        <div class="search">
          <Icon name="search" :size="15" />
          <input v-model="filters.text" class="sv-input" placeholder="Titre, Pokémon, dresseur, n° de carte…" />
        </div>
      </div>
      <div class="sv-field">
        <span class="sv-label">Chromatique <Tip :title="GIFT_TERMS.shinyLock.title" :text="GIFT_TERMS.shinyLock.text" /></span>
        <div class="sv-seg">
          <button v-for="t in TRI" :key="t.value" :class="{ on: filters.shiny === t.value }" @click="filters.shiny = t.value">{{ t.label }}</button>
        </div>
      </div>
      <div class="sv-field">
        <span class="sv-label">Œuf</span>
        <div class="sv-seg">
          <button v-for="t in TRI" :key="t.value" :class="{ on: filters.egg === t.value }" @click="filters.egg = t.value">{{ t.label }}</button>
        </div>
      </div>
      <div class="sv-field">
        <span class="sv-label">Contenu</span>
        <div class="chips">
          <button v-for="k in KINDS" :key="k.value" class="chip" :class="{ on: filters.kinds.includes(k.value) }" @click="toggle(filters.kinds, k.value)">
            {{ k.label }}
          </button>
        </div>
      </div>
      <label class="sv-switch" :class="{ off: !hasSave }">
        <input v-model="filters.onlyThisGame" type="checkbox" :disabled="!hasSave" />
        <span class="track" />
        Seulement ce jeu
        <Tip :title="GIFT_TERMS.onlyThisGame.title" :text="GIFT_TERMS.onlyThisGame.text" />
      </label>
      <div class="sv-field">
        <span class="sv-label">Génération <Tip :title="GIFT_TERMS.card.title" :text="GIFT_TERMS.card.text" /></span>
        <div class="chips">
          <button v-for="g in [4, 5, 6, 7]" :key="g" class="chip" :class="{ on: filters.generations.includes(g) }" @click="toggle(filters.generations, g)">
            Gen {{ g }}<small v-if="g === saveGen"> · ce jeu</small>
          </button>
        </div>
      </div>
      <div class="sv-field">
        <span class="sv-label">Tri</span>
        <select v-model="filters.sort" class="sv-select">
          <option v-for="s in SORTS" :key="s.value" :value="s.value">{{ s.label }}</option>
        </select>
      </div>
      <div class="foot">
        <button class="sv-btn" @click="resetFilters"><Icon name="refresh" :size="14" /> Réinitialiser</button>
        <button class="sv-btn" @click="importGift"><Icon name="folder-open" :size="14" /> Ouvrir un fichier…</button>
      </div>
    </aside>

    <!-- Cartes -->
    <section class="center">
      <header class="count">
        <strong>{{ total.toLocaleString("fr-FR") }}</strong> carte{{ total > 1 ? "s" : "" }}
        <span v-if="dbTotal">sur {{ dbTotal.toLocaleString("fr-FR") }} dans la base</span>
        <span v-if="loading" class="spin"><Icon name="refresh" :size="13" /></span>
      </header>
      <p v-if="error" class="error"><Icon name="alert" :size="14" /> {{ error }}</p>
      <div ref="grid" class="grid" @scroll.passive="onScroll">
        <button
          v-for="g in items"
          :key="g.id"
          class="card sv-panel"
          :class="[`f-${g.formatLabel.toLowerCase()}`, { on: selected?.id === g.id, imported: g.id >= IMPORTED_BASE }]"
          @click="select(g)"
          @dblclick="select(g).then(addToSave)"
        >
          <div class="card-top">
            <span class="fmt">{{ g.formatLabel }}</span>
            <span class="no">{{ g.id >= IMPORTED_BASE ? "Fichier" : g.cardId ? `n°${g.cardId}` : "" }}</span>
          </div>
          <div class="pic">
            <Sprite v-if="g.species" :id="g.species" :shiny="isShiny(g.shiny)" :size="76" />
            <Icon v-else :name="g.kind === 'item' ? 'bag' : 'gift'" :size="40" />
            <Icon v-if="isShiny(g.shiny)" class="star" name="star" :size="14" />
            <Icon v-if="g.egg" class="egg" name="egg" :size="14" />
          </div>
          <strong class="title">{{ g.title }}</strong>
          <small class="sub">
            <template v-if="g.species">{{ g.speciesName }}{{ g.level ? ` · N. ${g.level}` : "" }}</template>
            <template v-else>{{ g.itemNames[0] ?? g.kindLabel }}</template>
            <template v-if="formatDate(g.date)"> · {{ formatDate(g.date) }}</template>
          </small>
          <div class="games">
            <span v-for="c in g.games" :key="c.id" class="game">{{ c.name }}</span>
          </div>
        </button>
        <p v-if="!loading && !items.length" class="empty">Aucune carte ne correspond à ces filtres.</p>
      </div>
    </section>

    <!-- Fiche -->
    <GiftDetails
      v-if="selected"
      :gift="selected"
      :busy="busy"
      :has-save="hasSave"
      :last-added="!!lastAdded"
      @add="addToSave"
      @export="exportGift"
      @edit="openInEditor"
    />
    <aside v-else class="placeholder sv-panel">
      <Icon name="gift" :size="48" />
      <p>Choisis une carte pour voir le Pokémon ou les objets qu'elle contient.</p>
    </aside>
  </div>
</template>

<style scoped>
.gifts {
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr) 340px;
  gap: 18px;
  height: 100%;
  min-height: 0;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 15px;
  min-height: 0;
  padding: 16px;
  overflow-y: auto;
}

.filters h2 {
  display: flex;
  align-items: center;
  margin: 0;
  font-size: 18px;
}

.search {
  position: relative;
  display: flex;
  align-items: center;
}

.search :deep(svg) {
  position: absolute;
  left: 10px;
  color: var(--text-dim);
  pointer-events: none;
}

.search .sv-input {
  padding-left: 32px;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  padding: 5px 11px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-weight: 700;
  font-size: 12.5px;
}

.chip.on {
  border-color: var(--text);
  background: var(--text);
  color: var(--bg);
}

.chip small {
  font-weight: 600;
  opacity: 0.8;
}

.sv-switch.off {
  opacity: 0.45;
}

.foot {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: auto;
}

.center {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
}

.count {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 14px;
}

.count span {
  color: var(--text-dim);
  font-size: 13px;
}

.spin {
  display: inline-flex;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error {
  margin: 0;
  padding: 8px 12px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--danger) 18%, transparent);
  font-size: 13px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
  align-content: start;
  gap: 12px;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 6px 16px 2px;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 5px;
  padding: 10px 12px 12px;
  text-align: left;
  color: var(--text);
  border-radius: 16px;
  /* Les cartes hors écran ne sont pas dessinées (grandes listes). */
  content-visibility: auto;
  contain-intrinsic-size: auto 210px;
  transition: transform 0.12s, border-color 0.12s;
}

.card:hover {
  transform: translateY(-2px);
}

.card.on {
  border-color: var(--gift-tone);
  box-shadow: 0 0 0 2px var(--gift-tone), var(--shadow);
}

.card.imported {
  border-style: dashed;
}

.f-pcd,
.f-pgt {
  --gift-tone: #3b82f6;
}

.f-pgf {
  --gift-tone: #64748b;
}

.f-wc6 {
  --gift-tone: #db2777;
}

.f-wc7 {
  --gift-tone: #ea7a1a;
}

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.fmt {
  padding: 2px 7px;
  border-radius: 6px;
  background: var(--gift-tone);
  color: #fff;
  font-size: 10.5px;
  font-weight: 800;
  letter-spacing: 0.05em;
}

.no {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 700;
}

.pic {
  position: relative;
  display: grid;
  place-items: center;
  height: 70px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--gift-tone) 13%, transparent);
  color: var(--text-dim);
}

.pic .star {
  position: absolute;
  top: 6px;
  right: 6px;
  color: #ffd45c;
}

.pic .egg {
  position: absolute;
  bottom: 6px;
  right: 6px;
}

.title {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  font-size: 13px;
  line-height: 1.3;
  min-height: 2.6em;
}

.sub {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.games {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.game {
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  font-size: 10.5px;
  font-weight: 700;
}

.empty {
  grid-column: 1 / -1;
  color: var(--text-dim);
  text-align: center;
}

.placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  color: var(--text-dim);
  text-align: center;
}

@media (max-width: 1180px) {
  .gifts {
    grid-template-columns: 220px minmax(0, 1fr) 300px;
  }
}
</style>
