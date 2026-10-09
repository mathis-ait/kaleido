<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import Dialog from "../components/Dialog.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import { nav } from "../nav";
import { BUILTIN_PRESETS, deepMerge, engineDefaults, settingsOf, userPresets } from "../presets";
import { describeSettings, type SettingRow, type TabId } from "../views/randomizerSummary";
import type { RandomizerSettings } from "../types";
import { randomizedDialog, type RandomizedInfo } from "./randomized";

/**
 * Paramètres appliqués à une ROM randomisée, relus dans sa signature (seed + code de partage).
 * « Modifiés » compare au jeu d'origine : seuls les réglages qui ont changé quelque chose.
 */
const game = computed(() => randomizedDialog.game!);
const open = computed({
  get: () => !!randomizedDialog.game,
  set: (v) => {
    if (!v) randomizedDialog.game = null;
  },
});

const GROUPS: { id: TabId; label: string }[] = [
  { id: "general", label: "Général" },
  { id: "pokemon", label: "Pokémon" },
  { id: "starters", label: "Starters" },
  { id: "wild", label: "Sauvages" },
  { id: "trainers", label: "Dresseurs" },
  { id: "moves", label: "Attaques & CT" },
  { id: "items", label: "Objets" },
  { id: "statics", label: "Fixes & échanges" },
  { id: "shiny", label: "Chromatiques" },
];

const info = ref<RandomizedInfo | null>(null);
const rows = ref<SettingRow[]>([]);
const preset = ref<string | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const show = ref<"changed" | "all">("changed");
const copied = ref<"seed" | "code" | null>(null);

/** Réglages comparables : complétés par ceux du moteur, sans le format de sortie 3DS. */
function canonical(s: RandomizerSettings, base: RandomizerSettings) {
  const full = deepMerge(base, s as unknown as Record<string, unknown>) as unknown as Record<string, unknown>;
  delete full.ctrOutput;
  const sort = (v: unknown): unknown =>
    v && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)).map(([k, x]) => [k, sort(x)]))
      : v;
  return JSON.stringify(sort(full));
}

onMounted(async () => {
  try {
    const res = await invoke<RandomizedInfo | null>("randomized_info", { path: game.value.path });
    if (!res) {
      error.value = "Cette ROM ne contient pas de paramètres lisibles : elle a sans doute été générée par une ancienne version de Kaleido.";
      return;
    }
    info.value = res;
    const base = await engineDefaults();
    const isCtr = game.value.platform === "3ds";
    const species = (await invoke<{ species: string[] }>("name_lists").catch(() => ({ species: [] as string[] }))).species;
    const names = (ids?: number[]) => (ids ?? []).map((id) => (id ? (species[id] ?? `n° ${id}`) : ""));
    rows.value = describeSettings(res.settings, base, {
      isCtr,
      isXy: ["x", "y"].includes(game.value.game?.id ?? ""),
      starterNames: names(res.settings.customStarters),
      kantoNames: names(res.settings.customKantoStarters),
    });
    // Préréglage d'où viennent ces réglages, s'ils n'ont pas été retouchés.
    const target = canonical(res.settings, base);
    for (const p of [...(await userPresets()), ...BUILTIN_PRESETS]) {
      if (canonical(await settingsOf(p, isCtr ? "3ds" : "nds"), base) === target) {
        preset.value = p.name;
        break;
      }
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
});

const changedCount = computed(() => rows.value.filter((r) => r.changed).length);
const groups = computed(() =>
  GROUPS.map((g) => ({ ...g, rows: rows.value.filter((r) => r.tab === g.id && (show.value === "all" || r.changed)) })).filter((g) => g.rows.length),
);

async function copy(what: "seed" | "code") {
  if (!info.value) return;
  await navigator.clipboard.writeText(what === "seed" ? String(info.value.seed) : info.value.shareCode);
  copied.value = what;
  setTimeout(() => (copied.value = null), 1500);
}

/** Ouvre le Randomizer avec ces réglages et cette seed (sur une ROM d'origine). */
function reuse() {
  if (!info.value) return;
  nav.randomizerSettings = { settings: info.value.settings, seed: info.value.seed };
  nav.view = "randomizer";
  open.value = false;
}
</script>

<template>
  <Dialog v-model="open" title="Paramètres de randomisation" :subtitle="game.title" icon="dice" :width="620">
    <p v-if="loading" class="dim">Lecture de la ROM…</p>
    <p v-else-if="error" class="error-text">{{ error }}</p>
    <template v-else-if="info">
      <dl class="facts">
        <div>
          <dt>Seed</dt>
          <dd>
            <code>{{ info.seed }}</code>
            <button class="sv-btn small" @click="copy('seed')">{{ copied === "seed" ? "Copiée" : "Copier" }}</button>
          </dd>
        </div>
        <div>
          <dt>Préréglage</dt>
          <dd>{{ preset ?? "Réglages personnalisés" }}</dd>
        </div>
        <div>
          <dt>Générée par</dt>
          <dd>Kaleido {{ info.version }}</dd>
        </div>
      </dl>

      <div class="bar">
        <Segmented
          v-model="show"
          label="Réglages affichés"
          :options="[
            { value: 'changed', label: `Modifiés (${changedCount})`, hint: 'Ce qui change par rapport au jeu d\'origine' },
            { value: 'all', label: 'Tous' },
          ]"
        />
      </div>

      <p v-if="!groups.length" class="dim">Aucun réglage ne modifie le jeu d'origine.</p>
      <section v-for="g in groups" :key="g.id" class="group">
        <h3>{{ g.label }}</h3>
        <ul>
          <li v-for="r in g.rows" :key="r.label" :class="{ same: !r.changed }">
            <span>{{ r.label }}</span>
            <strong>{{ r.value }}</strong>
          </li>
        </ul>
      </section>
    </template>

    <template #foot>
      <button v-if="info?.journal" class="sv-btn" title="Liste détaillée de tout ce qui a été changé (spoilers !)" @click="openPath(info.journal)">
        <Icon name="folder" :size="15" /> Ouvrir le journal
      </button>
      <button v-if="info" class="sv-btn" title="Code KLD1-… à importer dans le Randomizer" @click="copy('code')">
        {{ copied === "code" ? "Code copié" : "Copier le code de partage" }}
      </button>
      <span class="spacer" />
      <button v-if="info" class="sv-btn solid" title="Ouvre le Randomizer avec ces réglages et cette seed" @click="reuse">Réutiliser ces réglages</button>
    </template>
  </Dialog>
</template>

<style scoped>
.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: var(--sp-3);
  margin: 0 0 var(--sp-4);
}

.facts div {
  padding: var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

dt {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

dd {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin: 4px 0 0;
  font-weight: 600;
}

dd code {
  font-size: var(--fs-md);
}

.bar {
  margin-bottom: var(--sp-3);
}

.group + .group {
  margin-top: var(--sp-4);
}

h3 {
  margin: 0 0 var(--sp-2);
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 600;
}

ul {
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: flex;
  justify-content: space-between;
  gap: var(--sp-4);
  padding: 7px 0;
  border-bottom: 1px solid var(--border);
}

li:last-child {
  border-bottom: none;
}

li strong {
  text-align: right;
}

li.same {
  color: var(--text-dim);
}

li.same strong {
  font-weight: 400;
}

.spacer {
  flex: 1;
}
</style>
