<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Banner from "../../components/Banner.vue";
import Dialog from "../../components/Dialog.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import Segmented from "../../components/Segmented.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { notify, saveState } from "../../saveStore";
import type { Slot, SlotView } from "../../types";
import { keyOf } from "../shell";
import { addSet, applySet, showdownUi, smogonSets, statLine, type SmogonSet, type SmogonSets } from "./api";

/**
 * Fenêtre « Sets compétitifs » : sets conseillés par Smogon pour l'espèce du Pokémon,
 * à appliquer sur lui ou à ajouter comme nouveau Pokémon. Depuis une case vide (`empty`),
 * les sets de l'espèce choisie créent directement le Pokémon dans cette case.
 */
const props = defineProps<{ p?: SlotView | null; empty?: { species: number; speciesName: string; slot: Slot } | null }>();

/** Pokémon dont on montre les sets (existant ou à créer). */
const subj = computed(() => {
  const p = props.p;
  if (p) return { species: p.species, form: p.form, shiny: p.shiny, speciesName: p.speciesName, formName: p.speciesData?.formName ?? null, name: p.nickname || p.speciesName, isEgg: p.isEgg };
  const e = props.empty;
  return { species: e?.species ?? 0, form: 0, shiny: false, speciesName: e?.speciesName ?? "", formName: null, name: e?.speciesName ?? "", isEgg: false };
});

const data = ref<SmogonSets | null>(null);
const error = ref<string | null>(null);
const actionError = ref<string | null>(null);
const loading = ref(false);
const format = ref<string>("all");
const lastWarnings = ref<{ key: string; list: string[] } | null>(null);
const busy = ref<string | null>(null);

async function load(refresh = false) {
  loading.value = true;
  error.value = null;
  try {
    data.value = await smogonSets(subj.value.species, subj.value.form, refresh);
    if (!formats.value.some((f) => f.id === format.value)) format.value = "all";
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// À l'ouverture, et quand l'espèce ou la forme change (le moteur garde les sets en mémoire).
watch(
  () => showdownUi.smogon && subj.value.species && `${subj.value.species}-${subj.value.form}`,
  (key) => {
    if (!key) return;
    lastWarnings.value = null;
    actionError.value = null;
    load();
  },
  { immediate: true },
);

const formats = computed(() => {
  const seen = new Map<string, { id: string; label: string; count: number }>();
  for (const s of data.value?.sets ?? []) {
    const f = seen.get(s.format) ?? { id: s.format, label: s.formatLabel, count: 0 };
    f.count++;
    seen.set(s.format, f);
  }
  return [...seen.values()];
});
const formatOptions = computed(() => [
  { value: "all", label: `Tous (${data.value?.sets.length ?? 0})` },
  ...formats.value.map((f) => ({ value: f.id, label: f.label })),
]);

const shown = computed(() => (data.value?.sets ?? []).filter((s) => format.value === "all" || s.format === format.value));
const keyOfSet = (s: SmogonSet) => `${s.speciesKey}|${s.format}|${s.name}`;
const name = computed(() => subj.value.name);
const fetched = computed(() => (data.value?.fetchedAt ? new Date(data.value.fetchedAt * 1000).toLocaleDateString("fr-FR") : null));
const subtitle = computed(
  () => `${subj.value.speciesName}${subj.value.formName ? ` · ${subj.value.formName}` : ""} · Gen ${saveState.view?.generation ?? "?"}`,
);

async function apply(s: SmogonSet) {
  busy.value = keyOfSet(s);
  actionError.value = null;
  try {
    if (!props.p) return;
    const warnings = await applySet(props.p.slot, s.set);
    lastWarnings.value = { key: keyOfSet(s), list: warnings };
    notify(`Set « ${s.name} » (${s.formatLabel}) appliqué à ${name.value}`);
  } catch (e) {
    actionError.value = String(e);
  } finally {
    busy.value = null;
  }
}

async function addNew(s: SmogonSet) {
  busy.value = keyOfSet(s);
  actionError.value = null;
  try {
    const target = props.empty ? { kind: "slot" as const, slot: props.empty.slot } : { kind: "box" as const, box: saveState.box };
    const report = await addSet(s.set, target);
    const r = report.sets[0];
    if (r?.error) actionError.value = r.error;
    else if (props.empty) {
      close();
    } else lastWarnings.value = { key: keyOfSet(s), list: r?.warnings ?? [] };
  } catch (e) {
    actionError.value = String(e);
  } finally {
    busy.value = null;
  }
}

function close() {
  showdownUi.smogon = false;
}

/** Fenêtre ouverte : les raccourcis de la page sont suspendus (Échap et Tab restent à la fenêtre). */
function onKey(e: KeyboardEvent) {
  if (!showdownUi.smogon) return;
  e.stopPropagation();
  if (keyOf(e) === "Escape") {
    e.preventDefault();
    close();
  }
}
onMounted(() => document.addEventListener("keydown", onKey));
onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKey);
  showdownUi.smogon = false;
});
</script>

<template>
  <Dialog v-model="showdownUi.smogon" title="Sets compétitifs" term="smogon" :subtitle="subtitle" :width="980">
    <template #head>
      <Sprite :id="subj.species" :shiny="subj.shiny" :size="48" class="head-sprite" />
      <span v-if="fetched" class="sv-chip" :class="data?.stale ? 'warn' : 'dim'" :title="data?.stale ? 'Pas de connexion : copie gardée en mémoire' : ''">
        {{ data?.stale ? "Hors ligne · " : "" }}données du {{ fetched }}
      </span>
      <button type="button" class="sv-round sq" title="Retélécharger les sets" aria-label="Retélécharger les sets" :disabled="loading" @click="load(true)">
        <Icon name="refresh" :size="15" />
      </button>
    </template>

    <nav v-if="formats.length > 1 && !loading && !error" class="formats sv-row">
      <span class="sv-label">Format <Tip term="smogonFormat" /></span>
      <Segmented v-model="format" :options="formatOptions" label="Format" />
    </nav>

    <Banner v-if="actionError" :dismiss="() => (actionError = null)">{{ actionError }}</Banner>

    <EmptyState v-if="loading" loading title="Récupération des sets Smogon…" />
    <EmptyState v-else-if="error" icon="alert" title="Sets indisponibles pour le moment">
      Kaleido a besoin d'Internet la première fois pour télécharger les sets de Smogon ; ils sont ensuite gardés et marchent hors ligne.
      <template #details>
        <p class="sv-help">{{ error }}</p>
      </template>
      <template #actions>
        <button type="button" class="sv-btn" @click="load(true)"><Icon name="refresh" :size="14" /> Réessayer</button>
      </template>
    </EmptyState>
    <EmptyState v-else-if="!shown.length" icon="search" title="Aucun set">
      Smogon ne propose pas de set pour {{ subj.speciesName }} en Gen {{ saveState.view?.generation }}. Les Pokémon peu utilisés en compétition n'ont
      souvent pas d'analyse.
    </EmptyState>
    <ul v-else class="sets">
      <li v-for="s in shown" :key="keyOfSet(s)" class="set">
        <div class="set-head">
          <span class="sv-chip">{{ s.formatLabel }}</span>
          <strong>{{ s.name }}</strong>
          <span v-if="!s.sameForm" class="sv-chip dim">{{ s.speciesKey }}</span>
          <span class="grow" />
          <span class="dim small">N. {{ s.resolved.level }}</span>
        </div>
        <dl class="sv-dl">
          <dt>Objet tenu <Tip term="heldItem" /></dt>
          <dd>
            {{ s.resolved.itemName ?? s.set.item ?? "—" }}
            <span v-if="s.options.items.length" class="dim alt"> ou {{ s.options.items.join(", ") }}</span>
          </dd>
          <dt>Talent <Tip term="ability" /></dt>
          <dd>
            {{ s.resolved.abilityName ?? "—" }}
            <span v-if="s.options.abilities.length" class="dim alt"> ou {{ s.options.abilities.join(", ") }}</span>
          </dd>
          <dt>Nature <Tip term="nature" /></dt>
          <dd>
            {{ s.resolved.natureName ?? "—" }}
            <span v-if="s.options.natures.length" class="dim alt"> ou {{ s.options.natures.join(", ") }}</span>
          </dd>
          <dt>EV <Tip term="ev" /></dt>
          <dd>{{ statLine(s.resolved.evs, 0) || "aucun" }}</dd>
          <template v-if="statLine(s.resolved.ivs, 31)">
            <dt>IV <Tip term="iv" /></dt>
            <dd>{{ statLine(s.resolved.ivs, 31) }}</dd>
          </template>
        </dl>
        <ol class="sv-moves alts">
          <li v-for="(m, i) in s.resolved.moveNames" :key="i">
            {{ m }}
            <span v-if="s.options.moves[i]?.length" class="dim alt">ou {{ s.options.moves[i].join(", ") }}</span>
          </li>
        </ol>
        <ul v-if="s.resolved.warnings.length" class="warns">
          <li v-for="w in s.resolved.warnings" :key="w"><Icon name="alert" :size="13" /> {{ w }}</li>
        </ul>
        <ul v-if="lastWarnings?.key === keyOfSet(s)" class="done">
          <li><Icon name="check" :size="13" /> C'est fait !</li>
          <li v-for="w in lastWarnings.list" :key="w" class="warn"><Icon name="alert" :size="13" /> {{ w }}</li>
        </ul>
        <div class="actions">
          <button type="button" class="sv-btn" :class="{ solid: !!empty }" :disabled="!!busy" @click="addNew(s)">
            <Icon name="plus" :size="14" /> {{ empty ? "Créer dans la case" : "Nouveau Pokémon" }}
          </button>
          <button v-if="p" type="button" class="sv-btn solid" :disabled="!!busy || p.isEgg" @click="apply(s)">
            <Icon name="wand" :size="14" /> Appliquer à {{ name }}
          </button>
        </div>
      </li>
    </ul>

    <template #foot>
      <p class="sv-help">
        Sets <Tip term="showdownSet" /> issus des analyses de Smogon University (données pkmn/smogon).
        <template v-if="empty">Le Pokémon est créé à ton nom, dans une Poké Ball, avec le set choisi. Ctrl+Z pour annuler.</template>
        <template v-else>
          « Appliquer » remplace l'objet, le talent, la nature, les EV/IV, les attaques et le niveau ; le dresseur et la rencontre sont gardés. Ctrl+Z
          pour annuler.
        </template>
      </p>
    </template>
  </Dialog>
</template>

<style scoped>
.head-sprite {
  margin: -8px 0;
}

.formats {
  margin-bottom: var(--sp-4);
}

.sv-banner {
  margin-bottom: var(--sp-3);
}

.dim {
  color: var(--text-dim);
}

.small {
  font-size: var(--fs-sm);
}

.grow {
  flex: 1;
}

.sets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
  gap: var(--sp-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.set {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3) 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 4%, transparent);
}

.set-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
}

.alt {
  font-size: var(--fs-sm);
  font-weight: 400;
}

/* Une attaque par ligne : les variantes (« ou … ») restent lisibles en entier. */
.sv-moves.alts {
  grid-template-columns: 1fr;
}

.sv-moves.alts li {
  display: block;
  white-space: normal;
}

.warns,
.done {
  display: grid;
  gap: 3px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--warn);
  font-size: var(--fs-sm);
}

.done {
  color: var(--ok);
}

.done .warn {
  color: var(--warn);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--sp-2);
  margin-top: auto;
}
</style>
