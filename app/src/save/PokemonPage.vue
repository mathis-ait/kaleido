<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import TypeBadge from "../components/TypeBadge.vue";
import { editPokemon, exportPokemon, goTo, lists, saveState } from "../saveStore";
import type { ShinyMode } from "../types";
import { checkPokemon, checkSummary } from "./checks";
import { apply } from "./pokemon/edit";
import OverviewTab from "./pokemon/OverviewTab.vue";
import MetTab from "./pokemon/MetTab.vue";
import StatsTab from "./pokemon/StatsTab.vue";
import MovesTab from "./pokemon/MovesTab.vue";
import TrainerTab from "./pokemon/TrainerTab.vue";
import ExtrasTab from "./pokemon/ExtrasTab.vue";
import { BALLS, genderLabel, genderSymbol, TYPE_KEYS } from "./refdata";
import { useShell } from "./shell";

const p = computed(() => saveState.selected);
const view = computed(() => saveState.view!);

const TABS = [
  { id: "overview", label: "Aperçu", comp: OverviewTab },
  { id: "met", label: "Rencontre", comp: MetTab },
  { id: "stats", label: "Statistiques", comp: StatsTab },
  { id: "moves", label: "Attaques", comp: MovesTab },
  { id: "trainer", label: "Dresseur", comp: TrainerTab },
  { id: "extras", label: "Extras", comp: ExtrasTab },
];
const tab = ref("overview");
const current = computed(() => TABS.find((t) => t.id === tab.value)!);

const checks = computed(() => (p.value ? checkPokemon(p.value, view.value.generation, lists.itemName) : []));
const summary = computed(() => checkSummary(checks.value));
const showReport = ref(false);
watch(p, () => (showReport.value = false));

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
  const i = TABS.findIndex((t) => t.id === tab.value);
  tab.value = TABS[(i + d + TABS.length) % TABS.length].id;
}

useShell(() => ({
  hint: p.value ? "Chaque modification est appliquée aussitôt · Ctrl+Z pour annuler" : "",
  actions: p.value
    ? [
        { key: ",", cap: "<", label: "Précédent", run: () => stepPokemon(-1) },
        { key: ".", cap: ">", label: "Suivant", run: () => stepPokemon(1) },
        { key: "y", cap: "Y", label: "Vérifications", run: () => (showReport.value = !showReport.value) },
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
        <Sprite :id="p.species" :shiny="p.shiny" :size="168" />
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
          class="round-tog"
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
          <strong>{{ summary.level === "ok" ? "Cohérent" : summary.level === "warn" ? "À vérifier" : "Problèmes" }}</strong>
          <small>{{ summary.text }} · voir le rapport</small>
        </span>
      </button>

      <div class="spacer" />
      <p class="where"><Icon name="box" :size="14" /> {{ locationText }}</p>
      <button class="sv-btn solid" @click="goTo('boxes')"><Icon name="grid" :size="15" /> Voir dans les boîtes</button>
      <div class="two">
        <button class="sv-btn" @click="exportIt"><Icon name="file" :size="15" /> Exporter</button>
        <button class="sv-btn" @click="showReport = !showReport"><Icon name="shield" :size="15" /> Rapport</button>
      </div>
    </aside>

    <!-- Onglets -->
    <section class="main">
      <nav class="tabs">
        <button v-for="t in TABS" :key="t.id" :class="{ on: tab === t.id }" @click="tab = t.id">{{ t.label }}</button>
      </nav>
      <div class="body sv-panel">
        <Transition name="fade" mode="out-in">
          <div v-if="showReport" key="report" class="report">
            <h3 class="sv-section-title">Vérifications <Tip term="legality" /></h3>
            <ul>
              <li v-for="c in checks" :key="c.title" :class="c.level">
                <Icon :name="c.level === 'ok' ? 'check' : 'alert'" :size="18" />
                <div>
                  <strong>{{ c.title }}</strong>
                  <p>{{ c.detail }}</p>
                </div>
                <button v-if="c.tab" class="sv-btn" @click="(tab = c.tab), (showReport = false)">Corriger</button>
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
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
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
  border: 10px solid color-mix(in srgb, var(--text) 18%, transparent);
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--text) 16%, transparent), transparent 70%);
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
  background: color-mix(in srgb, #1a2340 55%, transparent);
  color: color-mix(in srgb, var(--text) 75%, transparent);
  transition: border-color 0.15s, transform 0.15s;
}

.round-tog:hover:not(:disabled) {
  transform: translateY(-2px);
}

.round-tog.on {
  border-color: #ffd84d;
  color: #ffd84d;
}

.round-tog:disabled {
  cursor: default;
}

.g {
  font-size: 20px;
  font-weight: 700;
}

.g-male .g {
  color: #6fb4ff;
}

.g-female .g {
  color: #ff8cc4;
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
  border-radius: 14px;
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
  border-color: #6ee7a8;
  background: color-mix(in srgb, #22c55e 20%, transparent);
  color: #c9ffe0;
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

.tabs {
  display: flex;
  flex-wrap: wrap;
  align-self: flex-start;
  gap: 4px;
  padding: 5px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.tabs button {
  padding: 8px 18px;
  border: none;
  border-radius: 999px;
  background: none;
  font-weight: 600;
}

.tabs button.on {
  background: var(--text);
  color: var(--bg);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 55%, transparent);
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
  border-radius: 12px;
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

.report li.warn {
  color: var(--warn);
}

.report li.ok {
  color: #8ff0b5;
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
