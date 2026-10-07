<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Banner from "../../components/Banner.vue";
import Dialog from "../../components/Dialog.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { lists, saveState } from "../../saveStore";
import type { Slot, SlotView } from "../../types";
import { diffRows, legalizeAll, previewLegalizeAll, type LegalizeAllResult } from "../legality";
import { genderLabel, LANGUAGES, VERSIONS } from "../refdata";

/**
 * « Légaliser l'équipe / la boîte » avec aperçu groupé : chaque Pokémon illégal avant et
 * après, à cocher ou décocher, puis une seule application (une étape d'annulation).
 */
const props = defineProps<{ slots: Slot[]; title: string }>();
const open = defineModel<boolean>({ required: true });
const emit = defineEmits<{ applied: [] }>();

const data = ref<LegalizeAllResult | null>(null);
const error = ref<string | null>(null);
const applying = ref(false);
const picked = ref<Set<string>>(new Set());
const expanded = ref<string | null>(null);

const key = (s: Slot) => JSON.stringify(s);

async function load() {
  data.value = null;
  error.value = null;
  try {
    const r = await previewLegalizeAll(props.slots);
    data.value = r;
    picked.value = new Set(r.items.filter((i) => i.success).map((i) => key(i.slot)));
  } catch (e) {
    error.value = String(e);
  }
}

watch(open, (v) => v && load(), { immediate: true });

function toggle(s: Slot) {
  const k = key(s);
  const next = new Set(picked.value);
  if (next.has(k)) next.delete(k);
  else next.add(k);
  picked.value = next;
}

const names = {
  ball: (id: number) => lists.balls.find((b) => b.value === id)?.label ?? `Ball n°${id}`,
  version: (id: number) => VERSIONS.find((v) => v.id === id)?.name ?? `n°${id}`,
  language: (id: number) => LANGUAGES.find((l) => l.id === id)?.name ?? `n°${id}`,
  gender: (g: SlotView["gender"]) => genderLabel(g),
};
const changedRows = (a: SlotView, b: SlotView) => diffRows(a, b, names).filter((r) => r.changed);
const chosen = computed(() => data.value?.items.filter((i) => i.success && picked.value.has(key(i.slot))) ?? []);
const where = (s: Slot) => (s.kind === "party" ? `Équipe, place ${s.index + 1}` : `${saveState.view?.boxNames[s.box] ?? "Boîte"}, case ${s.index + 1}`);

async function applyIt() {
  if (!chosen.value.length || applying.value) return;
  applying.value = true;
  const r = await legalizeAll(chosen.value.map((i) => i.slot));
  applying.value = false;
  if (r) {
    open.value = false;
    emit("applied");
  }
}
</script>

<template>
  <Dialog v-model="open" :title="title" term="legalizeTeam" icon="wand" :width="980">
    <Banner v-if="error" :retry="load">Impossible de préparer l'aperçu : {{ error }}</Banner>
    <EmptyState v-else-if="!data" loading compact title="Calcul des corrections…" />
    <EmptyState v-else-if="!data.items.length" icon="shield" compact title="Rien à corriger">
      Aucun Pokémon illégal ici ({{ data.untouched }} déjà légal{{ data.untouched > 1 ? "s" : "" }} ou douteux).
    </EmptyState>
    <ul v-else class="gl">
      <li v-for="it in data.items" :key="key(it.slot)" class="gl-item" :class="{ off: !picked.has(key(it.slot)) }">
        <div class="gl-row">
          <label class="gl-check">
            <input
              type="checkbox"
              :checked="picked.has(key(it.slot))"
              :disabled="!it.success"
              :aria-label="`Corriger ${it.before.nickname || it.before.speciesName}`"
              @change="toggle(it.slot)"
            />
          </label>
          <Sprite :id="it.after.species" :shiny="it.after.shiny" :form="it.after.form" :size="48" />
          <div class="who">
            <strong>{{ it.before.nickname || it.before.speciesName }}</strong>
            <small>{{ where(it.slot) }} · N. {{ it.before.level }}</small>
          </div>
          <div class="what">
            <span v-if="it.encounter" class="dim">{{ it.encounter.kindLabel }} · {{ it.encounter.locationName }} · {{ it.encounter.versionName }}</span>
            <span v-else class="dim">Rencontre actuelle gardée</span>
            <small>{{ changedRows(it.before, it.after).length }} champ{{ changedRows(it.before, it.after).length > 1 ? "s" : "" }} modifié{{ changedRows(it.before, it.after).length > 1 ? "s" : "" }}</small>
          </div>
          <span class="sv-chip" :class="it.success ? 'ok' : 'danger'">{{ it.success ? "Légal après" : "Impossible" }}</span>
          <button
            type="button"
            class="sv-btn small"
            :aria-expanded="expanded === key(it.slot)"
            @click="expanded = expanded === key(it.slot) ? null : key(it.slot)"
          >
            {{ expanded === key(it.slot) ? "Masquer" : "Détail" }}
          </button>
        </div>
        <div v-if="expanded === key(it.slot)" class="gl-detail">
          <div v-for="r in changedRows(it.before, it.after)" :key="r.label" class="gl-diff">
            <span class="lab">{{ r.label }} <Tip :term="r.term" /></span>
            <span>{{ r.before }}</span>
            <Icon name="chevron-right" :size="14" />
            <strong>{{ r.after }}</strong>
          </div>
          <ul class="gl-changes">
            <li v-for="c in it.changes" :key="c.text">{{ c.text }} <Tip :term="c.term" /></li>
          </ul>
        </div>
      </li>
    </ul>

    <template #foot>
      <span v-if="data" class="dim">
        {{ data.fixed }} corrigeable{{ data.fixed > 1 ? "s" : "" }}<template v-if="data.failed"> · {{ data.failed }} impossible{{ data.failed > 1 ? "s" : "" }}</template>
        · Ctrl+Z annule tout d'un coup
      </span>
      <span class="grow" />
      <button type="button" class="sv-btn" @click="open = false">Annuler</button>
      <button type="button" class="sv-btn solid" :disabled="!chosen.length || applying" @click="applyIt">
        <Icon name="wand" :size="15" /> {{ applying ? "Application…" : `Appliquer (${chosen.length})` }}
      </button>
    </template>
  </Dialog>
</template>

<style scoped>
.gl {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  max-height: min(560px, calc(100vh - 280px));
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}

.gl-item {
  padding: var(--sp-2) var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
}

.gl-item.off {
  opacity: 0.6;
}

.gl-row {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.gl-check input {
  width: 18px;
  height: 18px;
}

.who,
.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.who {
  width: 200px;
}

.what {
  flex: 1;
  font-size: var(--fs-md);
}

.who small,
.what small,
.dim {
  color: var(--text-dim);
}

.gl-detail {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: var(--sp-2);
  padding-top: var(--sp-2);
  border-top: 1px solid var(--border);
  font-size: var(--fs-md);
}

.gl-diff {
  display: grid;
  grid-template-columns: 190px minmax(0, 1fr) 16px minmax(0, 1fr);
  gap: var(--sp-2);
  align-items: center;
}

.lab {
  display: inline-flex;
  align-items: center;
  color: var(--text-dim);
}

.gl-changes {
  display: grid;
  gap: 3px;
  margin: var(--sp-2) 0 0;
  padding-left: 18px;
}

.grow {
  flex: 1;
}
</style>
