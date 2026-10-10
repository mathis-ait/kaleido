<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import SearchField from "../components/SearchField.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import { dex, fold, genLabel, TAG_NAMES, TYPE_NAMES } from "./data";
import { ownedGames } from "./owned";
import { collection } from "./store";
import type { SpeciesSummary, SpeciesTag, TypeId } from "./types";
import { livedexUi, openSpecies } from "./ui";

type Status = "all" | "missing" | "caught" | "shiny";

const query = ref("");
const gen = ref<number | "">("");
const type = ref<TypeId | "">("");
const tag = ref<SpeciesTag | "">("");
/** Jeu où l'espèce s'obtient : identifiant, « mine » (mes jeux) ou rien. */
const game = ref<string>("");
const status = ref<Status>("all");
const search = ref<InstanceType<typeof SearchField> | null>(null);

watch(
  () => livedexUi.searchTick,
  async () => {
    await nextTick();
    search.value?.focus();
  },
);

const statusOptions: { value: Status; label: string }[] = [
  { value: "all", label: "Tous" },
  { value: "missing", label: "Manquants" },
  { value: "caught", label: "Obtenus" },
  { value: "shiny", label: "Chromatiques" },
];

const games = computed(() => dex.value?.games ?? []);
const gameIndex = computed(() => new Map(games.value.map((g, i) => [g.id, i])));
const mine = computed(() => [...ownedGames.value.keys()].map((id) => gameIndex.value.get(id)).filter((i): i is number => i !== undefined));

/** Une espèce est complète quand toutes ses cases sont remplies. */
function speciesState(s: SpeciesSummary): "none" | "partial" | "full" {
  const c = collection.value;
  if (!c?.speciesCaught.has(s.id)) return "none";
  const slots = c.slotsBySpecies.get(s.id) ?? [];
  return slots.every((x) => c.caught.has(x.key)) ? "full" : "partial";
}

function obtainableIn(s: SpeciesSummary, wanted: number[]): boolean {
  return s.forms.some((f) => f.obtain.some((g) => wanted.includes(g)) || f.event.some((g) => wanted.includes(g)));
}

const list = computed(() => {
  const d = dex.value;
  const c = collection.value;
  if (!d || !c) return [];
  const q = fold(query.value.trim());
  const n = /^\d+$/.test(q) ? Number(q) : null;
  const wanted = game.value === "mine" ? mine.value : game.value ? [gameIndex.value.get(game.value) ?? -1] : null;
  return d.speciesList.filter((s) => {
    if (q && n !== s.id && !fold(s.name).includes(q) && !fold(s.en).includes(q)) return false;
    if (gen.value !== "" && s.gen !== gen.value) return false;
    if (type.value && !s.forms[0].types.includes(type.value)) return false;
    if (tag.value && !s.tags.includes(tag.value)) return false;
    if (status.value === "missing" && speciesState(s) === "full") return false;
    if (status.value === "caught" && !c.speciesCaught.has(s.id)) return false;
    if (status.value === "shiny" && !c.speciesShiny.has(s.id)) return false;
    if (wanted && !obtainableIn(s, wanted)) return false;
    return true;
  });
});

const num = (n: number) => `n° ${String(n).padStart(4, "0")}`;
const filtersOn = computed(() => !!(query.value || gen.value !== "" || type.value || tag.value || game.value || status.value !== "all"));
function reset() {
  query.value = "";
  gen.value = "";
  type.value = "";
  tag.value = "";
  game.value = "";
  status.value = "all";
}
</script>

<template>
  <div class="dex-tab">
    <div class="filters">
      <SearchField ref="search" v-model="query" placeholder="Nom ou numéro (Ctrl+K)" />
      <Segmented v-model="status" :options="statusOptions" label="Statut" />
    </div>
    <div class="filters">
      <select v-model="gen" class="sv-select" aria-label="Génération">
        <option value="">Toutes les générations</option>
        <option v-for="g in 9" :key="g" :value="g">{{ genLabel(g) }}</option>
      </select>
      <select v-model="type" class="sv-select" aria-label="Type">
        <option value="">Tous les types</option>
        <option v-for="(name, id) in TYPE_NAMES" :key="id" :value="id" :hidden="id === 'stellar'">{{ name }}</option>
      </select>
      <select v-model="tag" class="sv-select" aria-label="Catégorie">
        <option value="">Toutes les catégories</option>
        <option v-for="(name, id) in TAG_NAMES" :key="id" :value="id">{{ name }}</option>
      </select>
      <select v-model="game" class="sv-select" aria-label="Obtenable dans">
        <option value="">Obtenable n'importe où</option>
        <option value="mine" :disabled="!mine.length">Obtenable dans mes jeux</option>
        <option v-for="g in games" :key="g.id" :value="g.id">Obtenable dans {{ g.name }}</option>
      </select>
      <button v-if="filtersOn" type="button" class="sv-btn small" @click="reset">Effacer les filtres</button>
      <span class="dim count">{{ list.length.toLocaleString("fr-FR") }} espèce{{ list.length > 1 ? "s" : "" }}</span>
    </div>

    <p v-if="!list.length" class="dim empty">Aucune espèce ne correspond à ces filtres.</p>

    <div class="cards">
      <button v-for="s in list" :key="s.id" type="button" class="card sv-card" :class="speciesState(s)" @click="openSpecies(s.id)">
        <Sprite :id="s.id" :size="56" :shiny="!!collection?.speciesShiny.has(s.id)" />
        <span class="text">
          <small class="dim">{{ num(s.id) }}</small>
          <strong>{{ s.name }}</strong>
          <small class="state">
            <template v-if="speciesState(s) === 'full'">Complet</template>
            <template v-else-if="speciesState(s) === 'partial'">En partie</template>
            <template v-else>Manquant</template>
            <span v-if="collection?.speciesShiny.has(s.id)" class="shiny"> · chromatique</span>
          </small>
        </span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.filters {
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

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(196px, 1fr));
  gap: var(--sp-2);
}

.card {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 6px 10px 6px 6px;
  content-visibility: auto;
  contain-intrinsic-size: auto 70px;
}

.card.none :deep(img) {
  filter: brightness(0);
  opacity: 0.18;
}

.text {
  display: grid;
  min-width: 0;
}

.text strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.state {
  color: var(--text-dim);
}

.card.full .state {
  color: var(--ok);
}

.shiny {
  color: var(--shiny);
}
</style>
