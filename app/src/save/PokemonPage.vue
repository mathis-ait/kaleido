<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import TypeBadge from "../components/TypeBadge.vue";
import { editPokemon, exportPokemon, goTo, lists, saveState } from "../saveStore";
import type { ShinyMode } from "../types";
import { checkPokemon, checkSummary, legalityOf } from "./checks";
import { legalizeSlot } from "./legality";
import { apply } from "./pokemon/edit";
import OverviewTab from "./pokemon/OverviewTab.vue";
import MetTab from "./pokemon/MetTab.vue";
import StatsTab from "./pokemon/StatsTab.vue";
import MovesTab from "./pokemon/MovesTab.vue";
import TrainerTab from "./pokemon/TrainerTab.vue";
import ExtrasTab from "./pokemon/ExtrasTab.vue";
import RibbonsTab from "./pokemon/RibbonsTab.vue";
import MemoriesTab from "./pokemon/MemoriesTab.vue";
import { BALLS, genderLabel, genderSymbol, TYPE_KEYS } from "./refdata";
import { useShell } from "./shell";
import { openShowdown, showdownUi } from "./showdown/api";
import ShowdownDialog from "./showdown/ShowdownDialog.vue";
import SmogonSets from "./showdown/SmogonSets.vue";

const p = computed(() => saveState.selected);
const view = computed(() => saveState.view!);

const ALL_TABS = [
  { id: "overview", label: "Aperçu", comp: OverviewTab, since: 4 },
  { id: "met", label: "Rencontre", comp: MetTab, since: 4 },
  { id: "stats", label: "Statistiques", comp: StatsTab, since: 4 },
  { id: "moves", label: "Attaques", comp: MovesTab, since: 4 },
  { id: "trainer", label: "Dresseur", comp: TrainerTab, since: 4 },
  { id: "extras", label: "Extras", comp: ExtrasTab, since: 4 },
  { id: "ribbons", label: "Rubans", comp: RibbonsTab, since: 4 },
  { id: "memories", label: "Souvenirs", comp: MemoriesTab, since: 6 },
];
/** Onglets du jeu ouvert : les souvenirs n'existent qu'à partir de la Gen 6. */
const TABS = computed(() => ALL_TABS.filter((t) => t.since <= view.value.generation));
const tab = ref("overview");
const current = computed(() => TABS.value.find((t) => t.id === tab.value) ?? TABS.value[0]);

const checks = computed(() => (p.value ? checkPokemon(p.value, view.value.generation, lists.itemName) : []));
const summary = computed(() => checkSummary(checks.value));
const showReport = ref(false);
const report = computed(() => (p.value ? legalityOf(p.value) : null));
/** Modifications faites par le dernier « Rendre légal ». */
const lastChanges = ref<string[] | null>(null);
const legalizing = ref(false);
watch(
  () => JSON.stringify(p.value?.slot),
  () => {
    showReport.value = false;
    lastChanges.value = null;
  },
);

const PID_LABELS: Record<string, string> = {
  method1: "méthode 1",
  method2: "méthode 2",
  method4: "méthode 4",
  chainShiny: "Poké Radar (chaîne chromatique)",
  cuteCharm: "Joli Sourire",
  pokewalker: "Pokéwalker",
  none: "aucune",
};
const pidLabel = (t: string) => PID_LABELS[t] ?? t;

async function makeLegal() {
  if (!p.value || legalizing.value) return;
  legalizing.value = true;
  const r = await legalizeSlot(p.value.slot);
  legalizing.value = false;
  if (r) {
    lastChanges.value = r.changes;
    showReport.value = true;
  }
}

const types = computed(() =>
  (p.value?.speciesData?.types ?? []).map((t) => ({ key: TYPE_KEYS[t] ?? "normal", name: lists.types[t] ?? "" })),
);

/** Clic : chromatique ; Maj : carré ; Alt : garde le PID (raccourcis de PKHeX). */
function onShiny(e: MouseEvent) {
  if (!p.value) return;
  let mode: ShinyMode;
  if (e.altKey) mode = "keepPid";
  else if (e.shiftKey) mode = "square";
  else mode = p.value.shiny ? "none" : "star";
  apply({ shiny: mode });
}

function cycleGender() {
  const p0 = p.value;
  if (!p0) return;
  const ratio = p0.speciesData?.genderRatio;
  if (ratio === 255 || ratio === 0 || ratio === 254) return; // sexe imposé par l'espèce
  apply({ gender: p0.gender === "male" ? "female" : "male" });
}

const ball = computed(() => BALLS.find((b) => b.id === p.value?.ball));
const ballLabel = computed(() => lists.balls.find((b) => b.value === p.value?.ball)?.label ?? ball.value?.name ?? "Ball");

async function exportIt() {
  const s = p.value;
  if (!s) return;
  const output = await save({
    title: "Exporter le Pokémon",
    defaultPath: `${s.speciesName}.pk${view.value.generation}`,
    filters: [{ name: "Pokémon", extensions: [`pk${view.value.generation}`] }],
  });
  if (output) exportPokemon(s.slot, output);
}

// Pokémon précédent / suivant dans l'équipe ou la boîte affichée.
const siblings = computed(() => {
  const s = p.value?.slot;
  if (!s) return [];
  return s.kind === "party" ? view.value.party : saveState.slots.filter((x) => !!x).map((x) => x!);
});
function stepPokemon(d: number) {
  const list = siblings.value;
  const i = list.findIndex((x) => JSON.stringify(x.slot) === JSON.stringify(p.value?.slot));
  if (i >= 0 && list.length) editPokemon(list[(i + d + list.length) % list.length]);
}

function goTab(d: number) {
  const tabs = TABS.value;
  const i = tabs.findIndex((t) => t.id === tab.value);
  tab.value = tabs[(i + d + tabs.length) % tabs.length].id;
}

useShell(() => ({
  hint: p.value ? "Chaque modification est appliquée aussitôt · Ctrl+Z pour annuler" : "",
  actions: p.value
    ? [
        { key: ",", cap: "<", label: "Précédent", run: () => stepPokemon(-1) },
        { key: ".", cap: ">", label: "Suivant", run: () => stepPokemon(1) },
        { key: "y", cap: "Y", label: "Vérifications", run: () => (showReport.value = !showReport.value) },
        { key: "l", cap: "L", label: "Rendre légal", run: makeLegal, disabled: summary.value.level !== "error" || legalizing.value },
        { key: "m", cap: "M", label: "Sets compétitifs", run: () => (showdownUi.smogon = true), disabled: p.value.isEgg },
        { key: "Ctrl+Tab", cap: "Ctrl+Tab", label: "Onglet suivant", run: () => goTab(1) },
      ]
    : [],
}));

const locationText = computed(() => {
  const s = p.value?.slot;
  if (!s) return "";
  return s.kind === "party" ? `Équipe · place ${s.index + 1}` : `${view.value.boxNames[s.box]} · case ${s.index + 1}`;
});
</script>

<template>
  <div v-if="!p" class="none sv-panel">
    <Icon name="ball" :size="40" />
    <h2>Aucun Pokémon sélectionné</h2>
    <p>Choisis un Pokémon dans ton équipe ou tes boîtes.</p>
    <div class="party-pick">
      <button v-for="m in view.party" :key="JSON.stringify(m.slot)" class="pick" @click="editPokemon(m)">
        <Sprite :id="m.species" :shiny="m.shiny" :size="64" />
        <small>{{ m.nickname || m.speciesName }}</small>
      </button>
    </div>
    <button class="sv-btn" @click="goTo('boxes')"><Icon name="grid" :size="15" /> Ouvrir les boîtes</button>
  </div>

  <div v-else class="pkm">
    <!-- Carte du Pokémon -->
    <aside class="card sv-panel">
      <div class="halo">
        <Sprite :id="p.species" :shiny="p.shiny" :form="p.form" :gender="p.gender" variant="model" :size="168" />
      </div>
      <h2>{{ p.nickname || p.speciesName }}</h2>
      <div class="sub">
        N. {{ p.level }}
        <span v-if="p.isNicknamed"> · {{ p.speciesName }}</span>
        <span v-if="p.speciesData?.formName"> · {{ p.speciesData.formName }}</span>
      </div>
      <div class="types">
        <TypeBadge v-for="t in types" :key="t.key" :type="t" />
      </div>
      <div class="toggles">
        <button
          class="round-tog shiny"
          :class="{ on: p.shiny }"
          :title="`${p.shiny ? 'Chromatique' : 'Non chromatique'} — clic : basculer · Maj : carré · Alt : garder le PID`"
          @click="onShiny"
        >
          <Icon name="star" :size="20" />
        </button>
        <button
          class="round-tog"
          :class="['g-' + p.gender]"
          :title="`${genderLabel(p.gender)} — clic pour changer`"
          :disabled="[0, 254, 255].includes(p.speciesData?.genderRatio ?? -1)"
          @click="cycleGender"
        >
          <span class="g">{{ genderSymbol(p.gender) || "∅" }}</span>
        </button>
        <button class="round-tog" :title="ballLabel" @click="tab = 'met'">
          <span class="ball" :style="{ '--ball': ball?.color ?? '#e74c3c' }" />
        </button>
        <button class="round-tog" :class="{ on: p.isEgg }" :title="p.isEgg ? 'Œuf — clic pour faire éclore' : 'Pas un œuf'" @click="apply({ isEgg: !p.isEgg })">
          <Icon name="egg" :size="20" />
        </button>
      </div>
      <p class="tip-line">Chromatique <Tip term="shiny" /></p>

      <button class="legal" :class="summary.level" @click="showReport = !showReport">
        <Icon :name="summary.level === 'ok' ? 'shield' : 'shield-alert'" :size="22" />
        <span>
          <strong>{{ summary.label }}</strong>
          <small>{{ summary.text }} · voir le rapport</small>
        </span>
      </button>
      <button v-if="summary.level === 'error'" class="sv-btn solid fix" :disabled="legalizing" @click="makeLegal">
        <Icon name="wand" :size="15" /> {{ legalizing ? "Correction…" : "Rendre légal" }}
      </button>

      <div class="spacer" />
      <p class="where"><Icon name="box" :size="14" /> {{ locationText }}</p>
      <button class="sv-btn solid" @click="goTo('boxes')"><Icon name="grid" :size="15" /> Voir dans les boîtes</button>
      <div class="two">
        <button class="sv-btn" @click="exportIt"><Icon name="file" :size="15" /> Exporter</button>
        <button class="sv-btn" @click="showReport = !showReport"><Icon name="shield" :size="15" /> Rapport</button>
      </div>
      <div class="two">
        <button class="sv-btn" title="Sets conseillés par Smogon (M)" :disabled="p.isEgg" @click="showdownUi.smogon = true">
          <Icon name="swords" :size="15" /> Sets
        </button>
        <button class="sv-btn" title="Exporter / importer au format Showdown (Ctrl+I)" @click="openShowdown('export')">
          <Icon name="upload" :size="15" /> Showdown
        </button>
      </div>
      <ShowdownDialog :target="p.slot" />
      <SmogonSets :p="p" />
    </aside>

    <!-- Onglets -->
    <section class="main">
      <nav class="sv-seg tabs">
        <button v-for="t in TABS" :key="t.id" :class="{ on: tab === t.id }" @click="tab = t.id">{{ t.label }}</button>
      </nav>
      <div class="body sv-panel">
        <Transition name="fade" mode="out-in">
          <div v-if="showReport" key="report" class="report">
            <h3 class="sv-section-title">Vérifications <Tip term="legality" /></h3>
            <div class="verdict" :class="summary.level">
              <Icon :name="summary.level === 'ok' ? 'shield' : 'shield-alert'" :size="22" />
              <div>
                <strong>{{ summary.label }}</strong>
                <p v-if="report">
                  Origine : {{ report.origin || "inconnue" }} <Tip term="origin" />
                  <template v-if="report.encounter">
                    <br />Rencontre : {{ report.encounter.kindLabel }}
                    <template v-if="report.encounter.locationName"> · {{ report.encounter.locationName }}</template>
                    · niv. {{ report.encounter.levelMin }}<template v-if="report.encounter.levelMax !== report.encounter.levelMin">–{{ report.encounter.levelMax }}</template>
                    <Tip term="encounter" />
                  </template>
                  <template v-if="report.pidType">
                    <br />Corrélation PID / IV : {{ pidLabel(report.pidType) }} <Tip term="pidiv" />
                  </template>
                </p>
                <p v-else>Analyse en cours…</p>
              </div>
              <button v-if="summary.level === 'error'" class="sv-btn solid" :disabled="legalizing" @click="makeLegal">
                <Icon name="wand" :size="15" /> {{ legalizing ? "Correction…" : "Rendre légal" }}
              </button>
              <Tip term="legalize" />
            </div>
            <div v-if="lastChanges" class="changes">
              <strong>{{ lastChanges.length ? "Modifications faites" : "Aucune modification nécessaire" }}</strong>
              <ul v-if="lastChanges.length">
                <li v-for="c in lastChanges" :key="c">{{ c }}</li>
              </ul>
              <small v-if="lastChanges.length">Ctrl+Z annule toutes ces modifications d'un coup.</small>
            </div>
            <ul>
              <li v-for="c in checks" :key="c.title + c.detail" :class="c.level">
                <Icon :name="c.level === 'ok' ? 'check' : 'alert'" :size="18" />
                <div>
                  <strong>{{ c.title }} <Tip v-if="c.term" :term="c.term" /></strong>
                  <p>{{ c.detail }}</p>
                </div>
                <button v-if="c.tab && c.level !== 'ok'" class="sv-btn" @click="(tab = c.tab), (showReport = false)">Corriger</button>
              </li>
            </ul>
            <button class="sv-btn" @click="showReport = false">Fermer le rapport</button>
          </div>
          <component :is="current.comp" v-else :key="tab + JSON.stringify(p.slot)" :p="p" />
        </Transition>
      </div>
    </section>
  </div>
</template>

<style scoped>
.none {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-width: 640px;
  margin: 40px auto;
  padding: 40px;
  text-align: center;
}

.none p {
  margin: 0;
  color: var(--text-dim);
}

.party-pick {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 10px;
}

.pick {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.pick:hover {
  background: color-mix(in srgb, var(--text) 14%, transparent);
}

.pkm {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 20px;
  height: 100%;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  min-height: 0;
  padding: 20px;
  overflow-y: auto;
}

.halo {
  display: grid;
  place-items: center;
  width: 190px;
  height: 190px;
  border: 6px solid color-mix(in srgb, var(--text) 10%, transparent);
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.card h2 {
  font-size: 28px;
  text-align: center;
}

.sub {
  color: var(--text-dim);
  font-size: 14px;
}

.types {
  display: flex;
  gap: 6px;
  min-height: 24px;
}

.toggles {
  display: flex;
  gap: 10px;
  margin-top: 4px;
}

.round-tog {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  border: 2px solid transparent;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  color: var(--text-dim);
  transition: border-color 0.15s, background-color 0.15s;
}

.round-tog:hover:not(:disabled, .on) {
  background: color-mix(in srgb, var(--text) 20%, transparent);
}

/* Actif (chromatique, œuf) : contour net couleur du texte, l'étoile prend la couleur chromatique. */
.round-tog.on {
  border-color: var(--text);
  color: var(--text);
}

.round-tog.on.shiny {
  color: var(--shiny);
}

.round-tog:disabled {
  cursor: default;
}

.g {
  font-size: 20px;
  font-weight: 700;
}

.g-male .g {
  color: var(--male);
}

.g-female .g {
  color: var(--female);
}

.ball {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: linear-gradient(var(--ball) 0 46%, #222 46% 54%, #fff 54%);
  box-shadow: inset 0 0 0 2px #222;
}

.tip-line {
  margin: 0;
  color: var(--text-dim);
  font-size: 12px;
}

.legal {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  margin-top: 8px;
  padding: 12px 14px;
  border: 1px solid;
  border-radius: var(--radius-card);
  text-align: left;
}

.legal strong {
  display: block;
  font-size: 15px;
}

.legal small {
  color: var(--text-dim);
}

.legal.ok {
  border-color: color-mix(in srgb, var(--ok) 60%, transparent);
  background: color-mix(in srgb, var(--ok) 14%, transparent);
  color: var(--ok);
}

.legal.warn {
  border-color: var(--warn);
  background: var(--warn-bg);
  color: var(--warn);
}

.legal.error {
  border-color: var(--danger);
  background: color-mix(in srgb, var(--danger) 16%, transparent);
  color: var(--danger);
}

.spacer {
  flex: 1;
}

.where {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  color: var(--text-dim);
  font-size: 12px;
}

.card > .sv-btn {
  width: 100%;
}

.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  width: 100%;
}

.main {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
}

/* Onglets : pastilles partagées (.sv-seg), un peu plus grandes ici. */
.tabs {
  align-self: flex-start;
  padding: 4px;
}

.tabs button {
  padding: 8px 18px;
  font-size: var(--fs-base);
}

/* Onglet choisi : l'inversion suffit, pas d'anneau de focus en plus. */
.tabs button.on:focus-visible {
  outline: none;
}

.body {
  flex: 1;
  min-height: 0;
  padding: 22px 24px;
  overflow-y: auto;
}

.report ul {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
}

.report li {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.report li > div {
  flex: 1;
}

.report li p {
  margin: 2px 0 0;
  color: var(--text-dim);
  font-size: 13px;
}

.report li.error {
  color: var(--danger);
}

.card > .fix {
  width: 100%;
}

.verdict {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
  padding: 12px 14px;
  border: 1px solid;
  border-radius: var(--radius-card);
}

.verdict > div {
  flex: 1;
}

.verdict strong {
  font-size: 16px;
}

.verdict p {
  margin: 2px 0 0;
  color: var(--text-dim);
  font-size: 13px;
  line-height: 1.5;
}

.verdict.ok {
  border-color: color-mix(in srgb, var(--ok) 60%, transparent);
  background: color-mix(in srgb, var(--ok) 12%, transparent);
  color: var(--ok);
}

.verdict.warn {
  border-color: var(--warn);
  background: var(--warn-bg);
  color: var(--warn);
}

.verdict.error {
  border-color: var(--danger);
  background: color-mix(in srgb, var(--danger) 12%, transparent);
  color: var(--danger);
}

.changes {
  margin-bottom: 12px;
  padding: 10px 14px;
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--accent-2) 14%, transparent);
}

.changes ul {
  margin: 6px 0;
  padding-left: 18px;
  gap: 2px;
  list-style: disc;
}

.changes li {
  display: list-item;
  padding: 0;
  background: none;
  font-size: 13px;
}

.changes small {
  color: var(--text-dim);
}

.report li.warn {
  color: var(--warn);
}

.report li.ok {
  color: var(--ok);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.12s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
