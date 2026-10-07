<script setup lang="ts">
import { STAT_SHORT } from "./refdata";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Banner from "../components/Banner.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import { editPokemon, saveState } from "../saveStore";
import type { SlotView } from "../types";
import { FAMILIES, familyOf, generateLegal, type EncounterEntry, type SpeciesEncounters } from "./legality";
import { useShell } from "./shell";

/** Base « Rencontres » : où trouver chaque Pokémon dans le jeu de la sauvegarde. */

const onlyThisGame = ref(true);
const search = ref("");
const families = ref<string[]>([]);
const index = ref<SpeciesEncounters[] | null>(null);
const selected = ref<number | null>(null);
const details = ref<EncounterEntry[] | null>(null);
const levels = ref<Record<string, number>>({});
const creating = ref<string | null>(null);
const created = ref<SlotView | null>(null);
const searchBox = ref<InstanceType<typeof SearchField> | null>(null);
const listEl = ref<HTMLElement | null>(null);
const indexError = ref<string | null>(null);
const detailsError = ref<string | null>(null);

// Résultats gardés pour la session (la base ne change pas tant que la sauvegarde reste ouverte).
// Seules les réussites sont gardées : un échec est retenté au prochain chargement.
const memo = new Map<string, SpeciesEncounters[]>();

async function loadIndex() {
  const k = `${saveState.path}:${!onlyThisGame.value}`;
  indexError.value = null;
  const hit = memo.get(k);
  if (hit) {
    index.value = hit;
    return;
  }
  index.value = null;
  try {
    const list = await invoke<SpeciesEncounters[]>("encounter_species", { all: !onlyThisGame.value });
    memo.set(k, list);
    index.value = list;
  } catch (e) {
    indexError.value = String(e);
  }
}

let detailsRequest = 0;

async function loadDetails() {
  const s = selected.value;
  const req = ++detailsRequest;
  created.value = null;
  detailsError.value = null;
  details.value = null;
  if (s === null) return;
  try {
    const list = await invoke<EncounterEntry[]>("encounter_details", { species: s, all: !onlyThisGame.value });
    if (req === detailsRequest) details.value = list;
  } catch (e) {
    if (req === detailsRequest) detailsError.value = String(e);
  }
}

onMounted(loadIndex);
watch(onlyThisGame, async () => {
  await loadIndex();
  await loadDetails();
});
watch(selected, loadDetails);

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const shown = computed(() => {
  const q = norm(search.value.trim());
  const num = /^\d+$/.test(q) ? Number(q) : null;
  return (index.value ?? []).filter((s) => {
    if (families.value.length && !s.families.some((f) => families.value.includes(f))) return false;
    if (!q) return true;
    return num !== null ? s.species === num : norm(s.name).includes(q);
  });
});

const current = computed(() => (index.value ?? []).find((s) => s.species === selected.value) ?? null);

const shownDetails = computed(() => (details.value ?? []).filter((e) => !families.value.length || families.value.includes(e.family)));

/** Rencontres regroupées par jeu (plusieurs jeux quand « Seulement ce jeu » est décoché). */
const byGame = computed(() => {
  const groups: { game: string; name: string; list: EncounterEntry[] }[] = [];
  for (const e of shownDetails.value) {
    let g = groups.find((x) => x.game === e.game);
    if (!g) groups.push((g = { game: e.game, name: e.gameName, list: [] }));
    g.list.push(e);
  }
  return groups;
});

function toggleFamily(id: string) {
  families.value = families.value.includes(id) ? families.value.filter((f) => f !== id) : [...families.value, id];
}

const entryKey = (e: EncounterEntry) => `${e.game}:${e.index}`;
const levelOf = (e: EncounterEntry) => levels.value[entryKey(e)] ?? e.levelMin;
const levelText = (e: EncounterEntry) => (e.levelMin === e.levelMax ? `N. ${e.levelMin}` : `N. ${e.levelMin} à ${e.levelMax}`);
const genderText = (g?: number) => (g === 0 ? "Mâle" : g === 1 ? "Femelle" : g === 2 ? "Asexué" : "");
const STAT = STAT_SHORT;
const ivText = (ivs?: number[]) => (ivs ?? []).map((v, i) => (v >= 0 ? `${STAT[i]} ${v}` : "")).filter(Boolean).join(" · ");

/** Bulle d'aide adaptée au type de rencontre. */
function kindTerm(e: EncounterEntry) {
  if (e.kind === "dreamWorld") return "dreamWorld";
  if (e.kind.startsWith("safari") || e.kind === "friendSafari") return "safari";
  if (["horde", "sos"].includes(e.kind)) return "hordeSos";
  if (e.egg) return "eggOrigin";
  if (["herbes", "surf", "peche", "special"].includes(e.family)) return "encounterSlot";
  return "encounter";
}

async function create(e: EncounterEntry) {
  if (creating.value) return;
  creating.value = entryKey(e);
  const level = Math.min(100, Math.max(e.levelMin, levelOf(e)));
  const r = await generateLegal({ species: e.species, form: e.form >= 30 ? 0 : e.form, level, encounterIndex: e.index, encounterGame: e.game });
  creating.value = null;
  if (r) created.value = r.view;
}

function openCreated() {
  if (created.value) editPokemon(created.value);
}

function move(d: number) {
  const list = shown.value;
  if (!list.length) return;
  const i = list.findIndex((s) => s.species === selected.value);
  selected.value = list[Math.min(list.length - 1, Math.max(0, i + d))].species;
  // Garde la ligne choisie visible ; le focus la suit s'il était déjà dans la liste.
  const hadFocus = !!listEl.value?.contains(document.activeElement);
  nextTick(() => {
    const el = listEl.value?.querySelector<HTMLElement>(".sp.on");
    el?.scrollIntoView({ block: "nearest" });
    if (hadFocus) el?.focus();
  });
}

useShell(() => ({
  hint: "Choisis un Pokémon pour voir où le trouver · « Créer ce Pokémon » en ajoute un légal dans tes boîtes",
  actions: [
    { key: "/", cap: "/", label: "Rechercher", run: () => searchBox.value?.focus() },
    { key: "ArrowUp", cap: "↑", label: "Précédent", run: () => move(-1) },
    { key: "ArrowDown", cap: "↓", label: "Suivant", run: () => move(1) },
  ],
}));
</script>

<template>
  <div class="enc">
    <aside class="sv-panel left">
      <SearchField ref="searchBox" v-model="search" placeholder="Nom ou numéro…" />
      <Toggle v-model="onlyThisGame" label="Seulement les Pokémon de ce jeu" term="onlyThisGame" />
      <div class="chips" role="group" aria-label="Types de rencontre">
        <button
          v-for="f in FAMILIES"
          :key="f.id"
          type="button"
          class="sv-chip"
          :class="{ on: families.includes(f.id) }"
          :aria-pressed="families.includes(f.id)"
          @click="toggleFamily(f.id)"
        >
          <i class="dot" :style="{ background: f.color }" />{{ f.label }}
        </button>
      </div>
      <Banner v-if="indexError" :retry="loadIndex">Impossible de lire la base des rencontres : {{ indexError }}</Banner>
      <template v-else>
        <p v-if="index" class="sv-help count">
          {{ shown.length }} Pokémon
          <Tip term="encounterDb" />
        </p>
        <EmptyState v-if="!index" compact loading title="Chargement de la base…" />
        <div v-else ref="listEl" class="species" role="list">
          <button
            v-for="s in shown"
            :key="s.species"
            type="button"
            role="listitem"
            class="sp"
            :class="{ on: s.species === selected }"
            :aria-current="s.species === selected ? 'true' : undefined"
            @click="selected = s.species"
          >
            <Sprite :id="s.species" :size="40" />
            <span class="sp-name">
              <strong>{{ s.name }}</strong>
              <small>n° {{ s.species }} · {{ s.count }} rencontre{{ s.count > 1 ? "s" : "" }}</small>
            </span>
            <span class="dots">
              <i v-for="f in s.families" :key="f" class="dot" :style="{ background: familyOf(f)?.color }" :title="familyOf(f)?.label" />
            </span>
          </button>
          <EmptyState v-if="!shown.length" compact icon="search" title="Aucun Pokémon ne correspond">
            Change la recherche ou les types de rencontre choisis.
          </EmptyState>
        </div>
      </template>
    </aside>

    <section class="sv-panel right">
      <EmptyState v-if="!current" icon="map" title="Où trouver chaque Pokémon" term="encounter">
        Choisis un Pokémon à gauche : hautes herbes, surf, pêche, dons, échanges, œufs offerts, Pokémon fixes… avec les niveaux, le lieu et les règles
        de chaque rencontre. « Créer ce Pokémon » en fabrique un, légal, dans le premier emplacement libre de la boîte affichée.
      </EmptyState>
      <template v-else>
        <header class="head">
          <Sprite :id="current.species" :size="96" />
          <div>
            <h2>{{ current.name }}</h2>
            <p class="sub">n° {{ current.species }} · {{ current.count }} rencontre{{ current.count > 1 ? "s" : "" }} · {{ current.games.join(", ") }}</p>
          </div>
        </header>
        <Banner v-if="created" tone="ok">
          <span class="created">
            {{ created.speciesName }} créé (N. {{ created.level }}).
            <button type="button" class="sv-btn small" @click="openCreated"><Icon name="ball" :size="15" /> Ouvrir la fiche</button>
          </span>
        </Banner>
        <Banner v-if="detailsError" :retry="loadDetails">Impossible de lire les rencontres de {{ current.name }} : {{ detailsError }}</Banner>
        <EmptyState v-else-if="!details" compact loading title="Chargement des rencontres…" />
        <EmptyState v-else-if="!shownDetails.length" compact icon="map" :title="families.length ? 'Aucune rencontre de ce type' : 'Aucune rencontre'">
          <template v-if="families.length">Aucune rencontre de {{ current.name }} pour les types choisis.</template>
          <template v-else>Aucune rencontre connue pour {{ current.name }} dans ce jeu.</template>
          <template v-if="families.length" #actions>
            <button type="button" class="sv-btn" @click="families = []">Voir tous les types</button>
          </template>
        </EmptyState>
        <div v-for="g in byGame" :key="g.game" class="game">
          <h3 class="sv-section-title">{{ g.name }}</h3>
          <div class="cards">
            <article v-for="e in g.list" :key="entryKey(e)" class="card">
              <div class="card-top">
                <i class="dot" :style="{ background: familyOf(e.family)?.color }" :title="familyOf(e.family)?.label" />
                <span class="kind">{{ e.kindLabel }}</span>
                <Tip :term="kindTerm(e)" />
                <span class="lvl">{{ levelText(e) }}</span>
              </div>
              <div v-if="e.title" class="versions">« {{ e.title }} »</div>
              <div class="place"><Icon name="map" :size="14" /> {{ e.locationName }}</div>
              <div class="versions">{{ e.versionNames.join(" · ") }}<template v-if="e.formName"> · {{ e.formName }}</template></div>
              <div class="badges">
                <span v-if="e.hiddenAbility" class="sv-chip">{{ e.ability }} <Tip term="hiddenAbility" /></span>
                <span v-else-if="e.ability !== 'Talent 1 ou 2'" class="sv-chip">{{ e.ability }} <Tip term="ability" /></span>
                <span v-if="e.shinyLock" class="sv-chip danger">Verrou chromatique <Tip term="shinyLock" /></span>
                <span v-if="e.shinyAlways" class="sv-chip shiny">Toujours chromatique <Tip term="shiny" /></span>
                <span v-if="e.flawlessIvs" class="sv-chip">{{ e.flawlessIvs }} IV à 31 <Tip term="flawlessIvs" /></span>
                <span v-if="e.ball" class="sv-chip">{{ e.ball }} <Tip term="ball" /></span>
                <span v-if="e.nature" class="sv-chip">Nature {{ e.nature }} <Tip term="nature" /></span>
                <span v-if="e.gender !== undefined" class="sv-chip">{{ genderText(e.gender) }} <Tip term="gender" /></span>
                <span v-if="e.fixedIvs" class="sv-chip">IV : {{ ivText(e.fixedIvs) }} <Tip term="iv" /></span>
                <span v-if="e.heldItem" class="sv-chip">Objet tenu : {{ e.heldItem }} <Tip term="heldItem" /></span>
                <span v-if="e.trainer" class="sv-chip">Dresseur d'origine : {{ e.trainer }} <Tip term="ot" /></span>
                <span v-if="e.fateful" class="sv-chip">Rencontre fatidique <Tip term="fateful" /></span>
              </div>
              <p v-if="e.moves?.length" class="moves">Attaques : {{ e.moves.join(", ") }}</p>
              <div class="actions">
                <label v-if="!e.egg" class="lvl-in">
                  N.
                  <input
                    class="sv-input"
                    type="number"
                    :min="e.levelMin"
                    max="100"
                    aria-label="Niveau du Pokémon créé"
                    :value="levelOf(e)"
                    @input="levels[entryKey(e)] = Number(($event.target as HTMLInputElement).value)"
                  />
                </label>
                <button type="button" class="sv-btn solid" :disabled="!!creating" @click="create(e)">
                  <Icon :name="creating === entryKey(e) ? 'refresh' : 'plus'" :size="15" :class="{ 'sv-spin': creating === entryKey(e) }" />
                  {{ creating === entryKey(e) ? "Création…" : "Créer ce Pokémon" }}
                </button>
              </div>
            </article>
          </div>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.enc {
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  gap: var(--sp-5);
  height: 100%;
  min-height: 0;
}

.left {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 0;
  padding: var(--sp-4);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
}

/* Pastille de couleur d'un type de rencontre (couleurs : FAMILIES, table de données). */
.dot {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.chips .dot {
  margin-right: 2px;
}

.count {
  display: flex;
  align-items: center;
  gap: var(--sp-1);
}

.species {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-height: 0;
  padding: 2px;
  overflow-y: auto;
}

.sp {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  flex-shrink: 0;
  padding: var(--sp-1) var(--sp-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--text);
  text-align: left;
}

.sp:hover {
  background: var(--panel-hover);
}

/* Sélection : inversion texte / fond, comme les onglets. */
.sp.on {
  background: var(--text);
  color: var(--bg);
}

.sp-name {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.sp-name small {
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.sp.on .sp-name small {
  color: inherit;
  opacity: 0.75;
}

.dots {
  display: flex;
  gap: 3px;
}

.right {
  min-height: 0;
  padding: var(--sp-5) var(--sp-6);
  overflow-y: auto;
}

.right > .sv-empty:only-child {
  height: 100%;
}

.head {
  display: flex;
  align-items: center;
  gap: var(--sp-4);
  margin-bottom: var(--sp-3);
}

.head h2 {
  margin: 0;
  font-size: 26px;
}

.sub {
  margin: var(--sp-1) 0 0;
  color: var(--text-dim);
  font-size: var(--fs-md);
}



.created {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
}

.right > .sv-banner {
  margin-bottom: var(--sp-3);
}

.game {
  margin-bottom: var(--sp-4);
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--sp-2);
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3) var(--sp-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 5%, transparent);
}

.card-top {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.kind {
  font-weight: 700;
}

.lvl {
  margin-left: auto;
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.place {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-base);
}

.versions {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.badges {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
}

.moves {
  margin: 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-2);
  margin-top: auto;
}

.lvl-in {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.lvl-in .sv-input {
  width: 64px;
}
</style>
