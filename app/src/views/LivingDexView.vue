<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import Tip from "../components/Tip.vue";
import AchievementsTab from "../livedex/AchievementsTab.vue";
import { ACHIEVEMENTS, rankOf } from "../livedex/achievements";
import BoxesTab from "../livedex/BoxesTab.vue";
import EntryDialog from "../livedex/EntryDialog.vue";
import JournalTab from "../livedex/JournalTab.vue";
import PokedexTab from "../livedex/PokedexTab.vue";
import SettingsDialog from "../livedex/SettingsDialog.vue";
import SourcesList from "../livedex/SourcesList.vue";
import SpeciesSheet from "../livedex/SpeciesSheet.vue";
import { RULE_PRESETS, matchRulePreset } from "../livedex/slots";
import { collection, completeSetup, entries, initLivedex, livedex, refreshIfStale, scan } from "../livedex/store";
import { livedexUi, openEntry, rememberUi, type LivedexTab } from "../livedex/ui";

onMounted(async () => {
  await initLivedex();
  refreshIfStale();
});

// Revenir dans Kaleido (après une partie, par exemple) relit les sauvegardes.
const onFocus = () => refreshIfStale(10_000);
function onKey(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    livedexUi.tab = "pokedex";
    livedexUi.searchTick++;
  }
}
onMounted(() => {
  window.addEventListener("focus", onFocus);
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => {
  window.removeEventListener("focus", onFocus);
  window.removeEventListener("keydown", onKey);
});

const tab = computed({
  get: () => livedexUi.tab,
  set: (t: LivedexTab) => {
    livedexUi.tab = t;
    rememberUi();
  },
});
const tabs: { value: LivedexTab; label: string }[] = [
  { value: "boxes", label: "Boîtes" },
  { value: "pokedex", label: "Pokédex" },
  { value: "journal", label: "Journal" },
  { value: "achievements", label: "Succès" },
];

const totals = computed(() => collection.value?.totals);
const percent = computed(() => (totals.value?.slots ? (totals.value.caught / totals.value.slots) * 100 : 0));
const percentLabel = computed(() => `${percent.value.toLocaleString("fr-FR", { maximumFractionDigits: 1 })} %`);
const rank = computed(() => rankOf(percent.value));
const presetName = computed(() => {
  const id = matchRulePreset(livedex.saved.rules);
  return id ? RULE_PRESETS[id].label : "Personnalisé";
});
const activeSources = computed(() => livedex.sources.filter((s) => s.path !== "bank" && !livedex.saved.disabledSources.includes(s.path)).length);
const autoCount = computed(() => entries.value.filter((e) => e.auto).length);
const fmt = (n: number | undefined) => (n ?? 0).toLocaleString("fr-FR");

// Succès débloqués : annoncés un par un dans un bandeau discret.
const toast = ref<string | null>(null);
let toastTimer: number | undefined;
watch(
  () => livedex.unlocked.length,
  () => {
    const id = livedex.unlocked.shift();
    if (!id) return;
    const a = ACHIEVEMENTS.find((x) => x.id === id);
    toast.value = a ? `Succès débloqué : ${a.title}` : null;
    clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toast.value = null), 4000);
  },
);
</script>

<template>
  <section class="livedex">
    <header class="page-head">
      <div>
        <h1>Living Dex<Tip term="livedex.livingDex" /></h1>
        <p class="lead">
          <template v-if="livedex.ready">
            {{ fmt(autoCount) }} Pokémon lus dans {{ activeSources }} sauvegarde{{ activeSources > 1 ? "s" : "" }}<template v-if="livedex.saved.bank"> et la banque</template
            ><template v-if="livedex.saved.manual.length">, {{ fmt(livedex.saved.manual.length) }} noté{{ livedex.saved.manual.length > 1 ? "s" : "" }} à la main</template>.
          </template>
          <template v-else>Ta collection, remplie toute seule à partir de tes sauvegardes.</template>
        </p>
      </div>
      <div class="sv-row">
        <button type="button" class="sv-btn" :disabled="livedex.scanning || !livedex.ready" title="Relire toutes les sauvegardes" @click="scan()">
          <Icon name="refresh" :size="15" :class="{ 'sv-spin': livedex.scanning }" /> Actualiser
        </button>
        <button type="button" class="sv-btn" :disabled="!livedex.ready" @click="livedexUi.settings = true"><Icon name="sliders" :size="15" /> Réglages</button>
        <button type="button" class="sv-btn solid" :disabled="!livedex.ready" @click="openEntry()"><Icon name="plus" :size="15" /> Noter un Pokémon</button>
      </div>
    </header>

    <EmptyState v-if="livedex.error && !livedex.ready" icon="shield-alert" title="Living Dex indisponible">
      {{ livedex.error }}
      <template #actions><button type="button" class="sv-btn" @click="initLivedex()">Réessayer</button></template>
    </EmptyState>

    <EmptyState v-else-if="!livedex.ready" loading title="Chargement de la Living Dex…" />

    <!-- Premier lancement : choisir les sauvegardes qui comptent. -->
    <div v-else-if="!livedex.saved.setupDone" class="setup sv-panel">
      <h2>Construire ta Living Dex</h2>
      <p class="dim">
        Kaleido a cherché tes sauvegardes (bibliothèque, dossiers des émulateurs, banque). Décoche celles qui ne doivent pas compter, par exemple une
        partie de test ou une sauvegarde modifiée.
      </p>
      <EmptyState v-if="livedex.scanning && !livedex.sources.length" compact loading title="Recherche des sauvegardes…" />
      <SourcesList v-else />
      <div class="sv-row end">
        <span class="dim">Règles : {{ presetName }} ({{ fmt(totals?.slots) }} cases), modifiables dans les réglages.</span>
        <button type="button" class="sv-btn solid" :disabled="livedex.scanning" @click="completeSetup">Construire ma Living Dex</button>
      </div>
    </div>

    <template v-else>
      <div class="summary sv-panel">
        <div class="main">
          <div class="figure">
            <strong>{{ fmt(totals?.caught) }}</strong><span class="dim"> / {{ fmt(totals?.slots) }} cases</span>
          </div>
          <div class="progress" role="progressbar" :aria-valuenow="Math.round(percent)" aria-valuemin="0" aria-valuemax="100">
            <span :style="{ width: `${percent}%` }" />
          </div>
          <small class="dim">{{ percentLabel }} · règles {{ presetName }}</small>
        </div>
        <dl class="stats">
          <div>
            <dt>Espèces</dt>
            <dd>{{ fmt(totals?.speciesCaught) }}<span class="dim"> / {{ fmt(totals?.species) }}</span></dd>
          </div>
          <div>
            <dt>Chromatiques</dt>
            <dd>{{ fmt(totals?.shiny) }}</dd>
          </div>
          <div>
            <dt>Rang <Tip term="livedex.rank" /></dt>
            <dd>{{ rank.name }}</dd>
          </div>
        </dl>
      </div>

      <div class="tabs">
        <Segmented v-model="tab" :options="tabs" label="Vue" />
      </div>

      <BoxesTab v-if="tab === 'boxes'" />
      <PokedexTab v-else-if="tab === 'pokedex'" />
      <JournalTab v-else-if="tab === 'journal'" />
      <AchievementsTab v-else />
    </template>

    <SpeciesSheet />
    <EntryDialog />
    <SettingsDialog />

    <Transition name="toast">
      <div v-if="toast" class="toast" role="status">{{ toast }}</div>
    </Transition>
  </section>
</template>

<style scoped>
.livedex {
  max-width: 1680px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
  font-size: var(--fs-lg);
}

.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-bottom: var(--sp-5);
}

.setup {
  display: grid;
  gap: var(--sp-4);
  max-width: 860px;
  padding: var(--sp-5) var(--sp-6);
}

.setup h2 {
  margin: 0;
}

.setup p {
  margin: 0;
}

.end {
  justify-content: space-between;
}

.summary {
  display: flex;
  align-items: center;
  gap: var(--sp-6);
  padding: var(--sp-4) var(--sp-5);
}

.summary .main {
  display: grid;
  flex: 1;
  gap: 6px;
}

.figure strong {
  font-family: var(--font-display);
  font-size: 28px;
  font-variant-numeric: tabular-nums;
}

.progress {
  height: 6px;
  overflow: hidden;
  border-radius: 3px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.progress span {
  display: block;
  height: 100%;
  background: var(--text);
  transition: width 0.4s ease;
}

.stats {
  display: flex;
  gap: var(--sp-6);
  margin: 0;
}

.stats dt {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.stats dd {
  margin: 2px 0 0;
  font-size: var(--fs-lg);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.tabs {
  margin: var(--sp-5) 0 var(--sp-4);
}

.toast {
  position: fixed;
  right: 24px;
  bottom: 24px;
  z-index: 50;
  padding: 10px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  box-shadow: var(--shadow-pop);
  font-weight: 600;
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
