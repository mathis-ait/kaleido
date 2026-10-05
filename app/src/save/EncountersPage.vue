<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
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
const searchBox = ref<HTMLInputElement | null>(null);

// Résultats gardés pour la session (la base ne change pas tant que la sauvegarde reste ouverte).
const memo = new Map<string, SpeciesEncounters[]>();

async function loadIndex() {
  const k = `${saveState.path}:${!onlyThisGame.value}`;
  const hit = memo.get(k);
  if (hit) {
    index.value = hit;
    return;
  }
  index.value = null;
  const list = await invoke<SpeciesEncounters[]>("encounter_species", { all: !onlyThisGame.value }).catch(() => []);
  memo.set(k, list);
  index.value = list;
}

async function loadDetails() {
  const s = selected.value;
  created.value = null;
  if (s === null) {
    details.value = null;
    return;
  }
  details.value = null;
  details.value = await invoke<EncounterEntry[]>("encounter_details", { species: s, all: !onlyThisGame.value }).catch(() => []);
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
const levelText = (e: EncounterEntry) => (e.levelMin === e.levelMax ? `Niv. ${e.levelMin}` : `Niv. ${e.levelMin} à ${e.levelMax}`);
const genderText = (g?: number) => (g === 0 ? "Mâle" : g === 1 ? "Femelle" : g === 2 ? "Asexué" : "");
const STAT = ["PV", "Att", "Déf", "Atq Spé", "Déf Spé", "Vit"];
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
      <div class="searchbar">
        <Icon name="search" :size="16" />
        <input ref="searchBox" v-model="search" class="sv-input" type="search" placeholder="Nom ou numéro…" />
      </div>
      <label class="sv-switch">
        <input v-model="onlyThisGame" type="checkbox" />
        <span class="track" />
        Seulement les Pokémon de ce jeu
        <Tip term="onlyThisGame" />
      </label>
      <div class="chips">
        <button v-for="f in FAMILIES" :key="f.id" class="chip" :class="{ on: families.includes(f.id) }" :style="{ '--c': f.color }" @click="toggleFamily(f.id)">
          {{ f.label }}
        </button>
      </div>
      <p class="sv-help count">
        <template v-if="index">{{ shown.length }} Pokémon</template>
        <template v-else>Chargement de la base…</template>
        <Tip term="encounterDb" />
      </p>
      <div class="species">
        <button v-for="s in shown" :key="s.species" class="sp" :class="{ on: s.species === selected }" @click="selected = s.species">
          <Sprite :id="s.species" :size="40" />
          <div class="sp-name">
            <strong>{{ s.name }}</strong>
            <small>n° {{ s.species }} · {{ s.count }} rencontre{{ s.count > 1 ? "s" : "" }}</small>
          </div>
          <span class="dots">
            <i v-for="f in s.families" :key="f" :style="{ background: familyOf(f)?.color }" :title="familyOf(f)?.label" />
          </span>
        </button>
        <p v-if="index && !shown.length" class="sv-help">Aucun Pokémon ne correspond.</p>
      </div>
    </aside>

    <section class="sv-panel right">
      <div v-if="!current" class="empty">
        <Icon name="map" :size="40" />
        <h2>Où trouver chaque Pokémon</h2>
        <p>
          Choisis un Pokémon à gauche : hautes herbes, surf, pêche, dons, échanges, œufs offerts, Pokémon fixes… avec les niveaux, le lieu et les règles
          de chaque rencontre <Tip term="encounter" />.
        </p>
        <p>« Créer ce Pokémon » en fabrique un, légal, dans le premier emplacement libre de la boîte affichée.</p>
      </div>
      <template v-else>
        <header class="head">
          <div class="halo"><Sprite :id="current.species" :size="96" /></div>
          <div>
            <h2>{{ current.name }}</h2>
            <p class="sub">n° {{ current.species }} · {{ current.count }} rencontre{{ current.count > 1 ? "s" : "" }} · {{ current.games.join(", ") }}</p>
          </div>
        </header>
        <div v-if="created" class="created">
          <Icon name="check" :size="18" />
          <span>{{ created.speciesName }} créé (N. {{ created.level }}).</span>
          <button class="sv-btn" @click="openCreated"><Icon name="ball" :size="15" /> Ouvrir la fiche</button>
        </div>
        <p v-if="!details" class="sv-help">Chargement…</p>
        <p v-else-if="!shownDetails.length" class="sv-help">Aucune rencontre de ce type.</p>
        <div v-for="g in byGame" :key="g.game" class="game">
          <h3 class="sv-section-title">{{ g.name }}</h3>
          <div class="cards">
            <article v-for="e in g.list" :key="entryKey(e)" class="card" :style="{ '--c': familyOf(e.family)?.color }">
              <div class="card-top">
                <span class="kind">{{ e.kindLabel }}</span>
                <Tip :term="kindTerm(e)" />
                <span class="lvl">{{ levelText(e) }}</span>
              </div>
              <div v-if="e.title" class="versions">« {{ e.title }} »</div>
              <div class="place"><Icon name="map" :size="14" /> {{ e.locationName }}</div>
              <div class="versions">{{ e.versionNames.join(" · ") }}<template v-if="e.formName"> · {{ e.formName }}</template></div>
              <div class="badges">
                <span v-if="e.hiddenAbility" class="badge">{{ e.ability }} <Tip term="hiddenAbility" /></span>
                <span v-else-if="e.ability !== 'Talent 1 ou 2'" class="badge">{{ e.ability }}</span>
                <span v-if="e.shinyLock" class="badge lock">Verrou chromatique <Tip term="shinyLock" /></span>
                <span v-if="e.shinyAlways" class="badge shiny"><Icon name="star" :size="12" /> Toujours chromatique</span>
                <span v-if="e.flawlessIvs" class="badge">{{ e.flawlessIvs }} IV à 31 <Tip term="flawlessIvs" /></span>
                <span v-if="e.ball" class="badge">{{ e.ball }} <Tip term="ball" /></span>
                <span v-if="e.nature" class="badge">Nature {{ e.nature }}</span>
                <span v-if="e.gender !== undefined" class="badge">{{ genderText(e.gender) }}</span>
                <span v-if="e.fixedIvs" class="badge">IV : {{ ivText(e.fixedIvs) }}</span>
                <span v-if="e.heldItem" class="badge">Objet : {{ e.heldItem }}</span>
                <span v-if="e.trainer" class="badge">Dresseur : {{ e.trainer }}</span>
                <span v-if="e.fateful" class="badge">Rencontre fatidique <Tip term="fateful" /></span>
              </div>
              <p v-if="e.moves?.length" class="moves">Attaques : {{ e.moves.join(", ") }}</p>
              <div class="actions">
                <label v-if="!e.egg" class="lvl-in">
                  Niv.
                  <input
                    class="sv-input"
                    type="number"
                    :min="e.levelMin"
                    max="100"
                    :value="levelOf(e)"
                    @input="levels[entryKey(e)] = Number(($event.target as HTMLInputElement).value)"
                  />
                </label>
                <button class="sv-btn solid" :disabled="!!creating" @click="create(e)">
                  <Icon name="plus" :size="15" /> {{ creating === entryKey(e) ? "Création…" : "Créer ce Pokémon" }}
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
  gap: 20px;
  height: 100%;
  min-height: 0;
}

.left {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 16px;
}

.searchbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.searchbar .sv-input {
  flex: 1;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  padding: 3px 10px;
  border: 1px solid color-mix(in srgb, var(--c) 60%, transparent);
  border-radius: 999px;
  background: none;
  color: var(--text);
  font-size: 12px;
  font-weight: 600;
}

.chip.on {
  background: color-mix(in srgb, var(--c) 35%, transparent);
}

.count {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
}

.species {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-height: 0;
  overflow-y: auto;
}

.sp {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 10px;
  border: none;
  border-radius: 10px;
  background: none;
  text-align: left;
}

.sp:hover {
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.sp.on {
  background: color-mix(in srgb, var(--accent-2) 28%, transparent);
}

.sp-name {
  display: flex;
  flex: 1;
  flex-direction: column;
}

.sp-name small {
  color: var(--text-dim);
  font-size: 11px;
}

.dots {
  display: flex;
  gap: 3px;
}

.dots i {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.right {
  min-height: 0;
  padding: 20px 24px;
  overflow-y: auto;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  max-width: 560px;
  margin: 60px auto;
  text-align: center;
}

.empty p {
  margin: 0;
  color: var(--text-dim);
}

.head {
  display: flex;
  align-items: center;
  gap: 18px;
  margin-bottom: 14px;
}

.halo {
  display: grid;
  place-items: center;
  width: 110px;
  height: 110px;
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--text) 16%, transparent), transparent 70%);
}

.head h2 {
  margin: 0;
  font-size: 26px;
}

.sub {
  margin: 4px 0 0;
  color: var(--text-dim);
  font-size: 13px;
}

.created {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
  padding: 10px 14px;
  border-radius: 12px;
  background: color-mix(in srgb, #22c55e 18%, transparent);
  color: #c9ffe0;
}

.created span {
  flex: 1;
}

.game {
  margin-bottom: 18px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 10px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px 14px;
  border: 1px solid color-mix(in srgb, var(--c) 45%, transparent);
  border-left: 4px solid var(--c);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 5%, transparent);
}

.card-top {
  display: flex;
  align-items: center;
  gap: 6px;
}

.kind {
  font-weight: 700;
}

.lvl {
  margin-left: auto;
  color: var(--text-dim);
  font-size: 13px;
}

.place {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
}

.versions {
  color: var(--text-dim);
  font-size: 12px;
}

.badges {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  font-size: 11px;
  font-weight: 600;
}

.badge.lock {
  background: color-mix(in srgb, var(--danger) 20%, transparent);
}

.badge.shiny {
  background: color-mix(in srgb, #ffd84d 25%, transparent);
  color: #ffd84d;
}

.moves {
  margin: 0;
  color: var(--text-dim);
  font-size: 12px;
}

.actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  margin-top: auto;
}

.lvl-in {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
}

.lvl-in .sv-input {
  width: 64px;
}
</style>
