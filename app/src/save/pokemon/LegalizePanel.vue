<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Banner from "../../components/Banner.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import Tip from "../../components/Tip.vue";
import { lists } from "../../saveStore";
import type { SlotView } from "../../types";
import { applyLegalize, diffRows, previewLegalize, type EncounterOption, type LegalityChange, type LegalizePreview } from "../legality";
import { genderLabel, LANGUAGES, VERSIONS } from "../refdata";

/**
 * Aperçu de « Rendre légal » : Pokémon avant / après (champs modifiés en surbrillance),
 * choix de la rencontre quand plusieurs conviennent, puis « Appliquer ». Rien n'est écrit
 * avant le clic : l'aperçu et l'application donnent le même Pokémon.
 */
const props = defineProps<{ p: SlotView }>();
const emit = defineEmits<{ close: []; applied: [changes: LegalityChange[]] }>();

const preview = ref<LegalizePreview | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const applying = ref(false);
/** Rencontre choisie (`null` : celle du moteur). */
const choice = ref<number | null>(null);
const showAll = ref(false);
/** La rencontre actuelle convient (corrections sur place seulement). */
const currentOk = ref(false);

async function load() {
  loading.value = true;
  error.value = null;
  try {
    preview.value = await previewLegalize(props.p.slot, choice.value);
    if (choice.value === null) currentOk.value = !preview.value.encounter && preview.value.success;
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}
onMounted(load);

function pick(o: EncounterOption | null) {
  const id = o?.id ?? null;
  if (id === choice.value && preview.value) return;
  choice.value = id;
  load();
}

async function applyIt() {
  if (!preview.value || applying.value) return;
  applying.value = true;
  const r = await applyLegalize(props.p.slot, choice.value);
  applying.value = false;
  if (r) emit("applied", r.changes);
}

const names = {
  ball: (id: number) => lists.balls.find((b) => b.value === id)?.label ?? `Ball n°${id}`,
  version: (id: number) => VERSIONS.find((v) => v.id === id)?.name ?? `n°${id}`,
  language: (id: number) => LANGUAGES.find((l) => l.id === id)?.name ?? `n°${id}`,
  gender: (g: SlotView["gender"]) => genderLabel(g),
};
const rows = computed(() => (preview.value ? diffRows(preview.value.before, preview.value.after, names) : []));
const changedCount = computed(() => rows.value.filter((r) => r.changed).length);
const shown = computed(() => (showAll.value ? rows.value : rows.value.filter((r) => r.changed)));
const remaining = computed(() => preview.value?.report.checks.filter((c) => c.severity === "invalid") ?? []);
/** Rencontre sélectionnée dans la liste : `null` = rencontre actuelle gardée. */
const selectedId = computed(() => preview.value?.encounter?.id ?? null);
const levels = (o: EncounterOption) => (o.levelMin === o.levelMax ? `N. ${o.levelMin}` : `N. ${o.levelMin}–${o.levelMax}`);
</script>

<template>
  <div class="lz">
    <div class="lz-head">
      <h3 class="sv-section-title">Aperçu de la correction <Tip term="legalizePreview" /></h3>
      <span class="grow" />
      <button type="button" class="sv-btn" @click="emit('close')">Fermer</button>
    </div>

    <Banner v-if="error" :retry="load">Impossible de calculer la correction : {{ error }}</Banner>
    <EmptyState v-else-if="!preview" loading compact title="Calcul de la correction…" />
    <template v-else>
      <!-- Rencontre -->
      <section v-if="preview.options.length > 1 || (currentOk && preview.options.length)" class="lz-enc">
        <span class="sv-label">Rencontre <Tip term="legalizeEncounter" /></span>
        <div class="lz-options" role="listbox" aria-label="Rencontre">
          <button
            v-if="currentOk"
            type="button"
            role="option"
            class="sv-list-item"
            :class="{ on: selectedId === null }"
            :aria-selected="selectedId === null"
            :disabled="loading"
            @click="pick(null)"
          >
            <span class="grow">Rencontre actuelle, gardée</span>
            <small>corrections sur place</small>
          </button>
          <button
            v-for="o in preview.options"
            :key="o.id"
            type="button"
            role="option"
            class="sv-list-item"
            :class="{ on: selectedId === o.id }"
            :aria-selected="selectedId === o.id"
            :disabled="loading"
            @click="pick(o)"
          >
            <span class="grow">{{ o.kindLabel }} · {{ o.locationName }}</span>
            <small>{{ o.versionName }} · {{ levels(o) }}</small>
          </button>
        </div>
      </section>
      <p v-else-if="preview.encounter" class="lz-one">
        Rencontre : <strong>{{ preview.encounter.kindLabel }} · {{ preview.encounter.locationName }}</strong> ({{ preview.encounter.versionName }})
        <Tip term="encounter" />
      </p>

      <!-- Avant / après -->
      <section class="lz-diff" :class="{ busy: loading }">
        <div class="lz-row lz-cols">
          <span class="sv-label">Champ</span>
          <span class="sv-label">Avant</span>
          <span class="sv-label">Après</span>
        </div>
        <div v-for="r in shown" :key="r.label" class="lz-row" :class="{ changed: r.changed }">
          <span class="lab">{{ r.label }} <Tip :term="r.term" /></span>
          <span class="val">{{ r.before }}</span>
          <span class="val after">{{ r.after }}</span>
        </div>
        <p v-if="!changedCount" class="sv-help">Aucun champ affiché ne change.</p>
        <button type="button" class="sv-link" @click="showAll = !showAll">
          {{ showAll ? "Montrer seulement les champs modifiés" : `Montrer tous les champs (${rows.length})` }}
        </button>
      </section>

      <!-- Détail des modifications -->
      <section v-if="preview.changes.length" class="lz-changes">
        <span class="sv-label">Ce que Kaleido va faire</span>
        <ul>
          <li v-for="c in preview.changes" :key="c.text">{{ c.text }} <Tip :term="c.term" /></li>
        </ul>
      </section>

      <Banner v-if="!preview.success" tone="warn">
        Kaleido ne trouve aucune rencontre qui rende ce Pokémon légal. Il restera {{ remaining.length }} problème{{ remaining.length > 1 ? "s" : "" }} :
        {{ remaining.map((c) => c.title).join(", ") }}.
      </Banner>

      <div class="lz-foot">
        <span class="sv-chip" :class="preview.success ? 'ok' : 'danger'">{{ preview.success ? "Légal après correction" : "Encore illégal" }}</span>
        <small class="dim">Une seule étape d'annulation : Ctrl+Z</small>
        <span class="grow" />
        <button type="button" class="sv-btn" @click="emit('close')">Annuler</button>
        <button type="button" class="sv-btn solid" :disabled="applying || loading || !preview.changes.length" @click="applyIt">
          <Icon name="wand" :size="15" /> {{ applying ? "Application…" : "Appliquer" }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.lz {
  display: flex;
  flex-direction: column;
  gap: var(--sp-4);
}

.lz-head,
.lz-foot {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.lz-head h3 {
  margin: 0;
}

.grow {
  flex: 1;
}

.lz-enc {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
}

.lz-options {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 220px;
  padding: var(--sp-1);
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
}

.lz-options small {
  color: inherit;
  font-size: var(--fs-sm);
  opacity: 0.75;
}

.lz-one {
  margin: 0;
  font-size: var(--fs-md);
}

.lz-diff {
  display: flex;
  flex-direction: column;
  gap: 2px;
  transition: opacity 0.12s;
}

.lz-diff.busy {
  opacity: 0.5;
}

.lz-row {
  display: grid;
  grid-template-columns: minmax(150px, 0.8fr) minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--sp-3);
  align-items: center;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--fs-md);
}

.lz-cols {
  padding-bottom: 2px;
}

.lab {
  display: inline-flex;
  align-items: center;
  color: var(--text-dim);
}

.val {
  overflow-wrap: anywhere;
}

/* Champ modifié : fond teinté sur toute la ligne, valeur d'après en gras. */
.lz-row.changed {
  background: var(--warn-bg);
}

.lz-row.changed .after {
  font-weight: 700;
}

.lz-row.changed .lab {
  color: var(--text);
}

.lz-changes ul {
  display: grid;
  gap: 4px;
  margin: var(--sp-2) 0 0;
  padding-left: 18px;
  font-size: var(--fs-md);
}

.lz-changes li {
  padding: 0;
}

.dim {
  color: var(--text-dim);
}
</style>
