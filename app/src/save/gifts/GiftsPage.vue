<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Banner from "../../components/Banner.vue";
import Combo, { type ComboOption } from "../../components/Combo.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import SearchField from "../../components/SearchField.vue";
import Segmented from "../../components/Segmented.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import Toggle from "../../components/Toggle.vue";
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
import FormatBadge from "./FormatBadge.vue";
import GiftDetails from "./GiftDetails.vue";
import { giftTone } from "./terms";

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
/** Dernière erreur, avec l'action à relancer quand ça a du sens. */
const error = ref<{ message: string; retry?: () => void } | null>(null);
const selected = ref<Details | null>(null);
const busy = ref(false);
const lastAdded = ref<SlotView | null>(null);
const speciesOptions = ref<ComboOption[]>([]);
const dbTotal = ref(0);
const grid = ref<HTMLElement | null>(null);

const hasSave = computed(() => !!saveState.view);
const saveGen = computed(() => saveState.view?.generation ?? null);

function fail(e: unknown, retry?: () => void) {
  error.value = { message: String(e), retry };
}

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
    if (id === request) fail(e, () => load(reset));
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
    fail(e, () => select(g));
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
    fail(e, addToSave);
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
    fail(e);
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
    fail(e);
  }
}

function openInEditor() {
  if (lastAdded.value) editPokemon(lastAdded.value);
}

async function loadOverview() {
  try {
    const o = await giftsApi.overview();
    dbTotal.value = o.total;
    speciesOptions.value = o.species.map((s) => ({ value: s.value, label: s.label, hint: `${s.count}`, sprite: s.value }));
  } catch (e) {
    fail(e, () => {
      loadOverview();
      load(true);
    });
  }
}

onMounted(async () => {
  await loadOverview();
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
      <h2>Cadeaux mystère <Tip term="gifts.mysteryGift" /></h2>
      <div class="sv-field">
        <span class="sv-label">Pokémon</span>
        <Combo v-model="filters.species" :options="speciesOptions" sprites none-label="Tous les Pokémon" placeholder="Espèce…" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Recherche</span>
        <SearchField v-model="filters.text" placeholder="Titre, Pokémon, dresseur, n° de carte…" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Chromatique <Tip term="shinyLock" /></span>
        <Segmented v-model="filters.shiny" :options="TRI" label="Chromatique" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Œuf</span>
        <Segmented v-model="filters.egg" :options="TRI" label="Œuf" />
      </div>
      <div class="sv-field">
        <span class="sv-label">Contenu</span>
        <div class="sv-row chips">
          <button
            v-for="k in KINDS"
            :key="k.value"
            type="button"
            class="sv-chip"
            :class="{ on: filters.kinds.includes(k.value) }"
            :aria-pressed="filters.kinds.includes(k.value)"
            @click="toggle(filters.kinds, k.value)"
          >
            {{ k.label }}
          </button>
        </div>
      </div>
      <Toggle v-model="filters.onlyThisGame" label="Seulement ce jeu" term="onlyThisGame" :disabled="!hasSave" />
      <div class="sv-field">
        <span class="sv-label">Génération <Tip term="gifts.card" /></span>
        <div class="sv-row chips">
          <button
            v-for="g in [4, 5, 6, 7]"
            :key="g"
            type="button"
            class="sv-chip"
            :class="{ on: filters.generations.includes(g) }"
            :aria-pressed="filters.generations.includes(g)"
            @click="toggle(filters.generations, g)"
          >
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
      <div class="foot sv-row">
        <button type="button" class="sv-btn" @click="resetFilters"><Icon name="refresh" :size="14" /> Réinitialiser</button>
        <button type="button" class="sv-btn" @click="importGift"><Icon name="folder-open" :size="14" /> Ouvrir un fichier…</button>
      </div>
    </aside>

    <!-- Cartes -->
    <section class="center">
      <header class="count">
        <strong>{{ total.toLocaleString("fr-FR") }}</strong> carte{{ total > 1 ? "s" : "" }}
        <span v-if="dbTotal">sur {{ dbTotal.toLocaleString("fr-FR") }} dans la base</span>
        <Icon v-if="loading && items.length" name="refresh" :size="13" class="sv-spin" />
      </header>
      <Banner v-if="error" :retry="error.retry" :dismiss="() => (error = null)">{{ error.message }}</Banner>
      <EmptyState v-if="loading && !items.length" loading title="Chargement des cartes…" />
      <EmptyState v-else-if="!items.length && !error" icon="search" title="Aucune carte" compact>
        Aucune carte ne correspond à ces filtres.
        <template #actions>
          <button type="button" class="sv-btn" @click="resetFilters"><Icon name="refresh" :size="14" /> Réinitialiser les filtres</button>
        </template>
      </EmptyState>
      <div v-else ref="grid" class="grid" @scroll.passive="onScroll">
        <button
          v-for="g in items"
          :key="g.id"
          type="button"
          class="card sv-panel"
          :class="{ on: selected?.id === g.id, imported: g.id >= IMPORTED_BASE }"
          :style="giftTone(g.formatLabel)"
          :aria-pressed="selected?.id === g.id"
          @click="select(g)"
          @dblclick="select(g).then(addToSave)"
        >
          <div class="card-top">
            <FormatBadge :label="g.formatLabel" />
            <span class="no">{{ g.id >= IMPORTED_BASE ? "Fichier" : g.cardId ? `n°${g.cardId}` : "" }}</span>
          </div>
          <div class="pic">
            <Sprite v-if="g.species" :id="g.species" :shiny="isShiny(g.shiny)" :size="76" />
            <Icon v-else :name="g.kind === 'item' ? 'bag' : 'gift'" :size="40" />
            <span v-if="isShiny(g.shiny)" class="star" title="Chromatique"><Icon name="star" :size="14" /></span>
            <span v-if="g.egg" class="egg" title="Œuf"><Icon name="egg" :size="14" /></span>
          </div>
          <strong class="title">{{ g.title }}</strong>
          <small class="sub">
            <template v-if="g.species">{{ g.speciesName }}{{ g.level ? ` · N. ${g.level}` : "" }}</template>
            <template v-else>{{ g.itemNames[0] ?? g.kindLabel }}</template>
            <template v-if="formatDate(g.date)"> · {{ formatDate(g.date) }}</template>
          </small>
          <div class="games">
            <span v-for="c in g.games" :key="c.id" class="sv-chip game">{{ c.name }}</span>
          </div>
        </button>
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
      <EmptyState icon="gift" title="Aucune carte choisie">Choisis une carte pour voir le Pokémon ou les objets qu'elle contient.</EmptyState>
    </aside>
  </div>
</template>

<style scoped>
.gifts {
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr) 340px;
  gap: var(--sp-4);
  height: 100%;
  min-height: 0;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
  min-height: 0;
  padding: var(--sp-4);
  overflow-y: auto;
}

.filters h2 {
  display: flex;
  align-items: center;
  margin: 0;
  font-size: var(--fs-lg);
}

.chips {
  gap: 6px;
}

.chips small {
  font-weight: 600;
  opacity: 0.8;
}

.foot {
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
  align-items: center;
  gap: 6px;
  font-size: var(--fs-base);
}

.count span,
.count .icon {
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
  align-content: start;
  gap: var(--sp-3);
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
  border-radius: var(--radius-card);
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

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.no {
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 700;
}

.pic {
  position: relative;
  display: grid;
  place-items: center;
  height: 70px;
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--gift-tone) 13%, transparent);
  color: var(--text-dim);
}

.pic .star,
.pic .egg {
  position: absolute;
  right: 6px;
  display: inline-flex;
}

.pic .star {
  top: 6px;
  color: var(--shiny);
}

.pic .egg {
  bottom: 6px;
}

.title {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  font-size: var(--fs-md);
  line-height: 1.3;
  min-height: 2.6em;
}

.sub {
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.games {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
}

.game {
  padding: 0 7px;
  font-size: var(--fs-xs);
}

.placeholder {
  display: grid;
  place-items: center;
}

@media (max-width: 1180px) {
  .gifts {
    grid-template-columns: 220px minmax(0, 1fr) 300px;
  }
}
</style>
