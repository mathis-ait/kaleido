<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { notify, saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { keyOf } from "../shell";
import { addSet, applySet, showdownUi, smogonSets, statLine, type SmogonSet, type SmogonSets } from "./api";

/**
 * Fenêtre « Sets compétitifs » : sets conseillés par Smogon pour l'espèce du Pokémon,
 * à appliquer sur lui ou à ajouter comme nouveau Pokémon.
 */
const props = defineProps<{ p: SlotView }>();

const data = ref<SmogonSets | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const format = ref<string>("all");
const lastWarnings = ref<{ key: string; list: string[] } | null>(null);
const busy = ref<string | null>(null);

async function load(refresh = false) {
  loading.value = true;
  error.value = null;
  try {
    data.value = await smogonSets(props.p.species, props.p.form, refresh);
    if (!formats.value.some((f) => f.id === format.value)) format.value = "all";
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// À l'ouverture, et quand l'espèce ou la forme change (le moteur garde les sets en mémoire).
watch(
  () => showdownUi.smogon && `${props.p.species}-${props.p.form}`,
  (key) => {
    if (!key) return;
    lastWarnings.value = null;
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

const shown = computed(() => (data.value?.sets ?? []).filter((s) => format.value === "all" || s.format === format.value));
const keyOfSet = (s: SmogonSet) => `${s.speciesKey}|${s.format}|${s.name}`;
const name = computed(() => props.p.nickname || props.p.speciesName);
const fetched = computed(() => (data.value?.fetchedAt ? new Date(data.value.fetchedAt * 1000).toLocaleDateString("fr-FR") : null));

async function apply(s: SmogonSet) {
  busy.value = keyOfSet(s);
  try {
    const warnings = await applySet(props.p.slot, s.set);
    lastWarnings.value = { key: keyOfSet(s), list: warnings };
    notify(`Set « ${s.name} » (${s.formatLabel}) appliqué à ${name.value}`);
  } catch (e) {
    saveState.error = String(e);
  } finally {
    busy.value = null;
  }
}

async function addNew(s: SmogonSet) {
  busy.value = keyOfSet(s);
  try {
    const box = saveState.box;
    const report = await addSet(s.set, { kind: "box", box });
    const r = report.sets[0];
    if (r?.error) saveState.error = r.error;
    else lastWarnings.value = { key: keyOfSet(s), list: r?.warnings ?? [] };
  } catch (e) {
    saveState.error = String(e);
  } finally {
    busy.value = null;
  }
}

function close() {
  showdownUi.smogon = false;
}

function onKey(e: KeyboardEvent) {
  if (!showdownUi.smogon) return;
  e.stopPropagation();
  if (keyOf(e) === "Escape") {
    e.preventDefault();
    close();
  }
}
onMounted(() => window.addEventListener("keydown", onKey, true));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  showdownUi.smogon = false;
});
</script>

<template>
  <Teleport to="body">
    <Transition name="sm-fade">
      <div v-if="showdownUi.smogon" class="sm-overlay" @pointerdown.self="close">
        <section class="sm-dialog sv-panel" role="dialog" aria-modal="true" aria-label="Sets compétitifs">
          <header class="sm-head">
            <Sprite :id="p.species" :shiny="p.shiny" :size="48" />
            <div class="title">
              <h2>Sets compétitifs <Tip term="smogon" /></h2>
              <small>{{ p.speciesName }}<template v-if="p.speciesData?.formName"> · {{ p.speciesData.formName }}</template> · Gen {{ saveState.view?.generation }}</small>
            </div>
            <span class="grow" />
            <span v-if="fetched" class="chip" :class="{ warn: data?.stale }" :title="data?.stale ? 'Pas de connexion : copie gardée en mémoire' : ''">
              {{ data?.stale ? "Hors ligne · " : "" }}données du {{ fetched }}
            </span>
            <button class="round" title="Retélécharger les sets" :disabled="loading" @click="load(true)"><Icon name="refresh" :size="15" /></button>
            <button class="round" aria-label="Fermer" @click="close"><Icon name="x" :size="16" /></button>
          </header>

          <nav v-if="formats.length > 1" class="formats">
            <span class="sv-label">Format <Tip term="smogonFormat" /></span>
            <div class="sv-seg">
              <button :class="{ on: format === 'all' }" @click="format = 'all'">Tous ({{ data?.sets.length }})</button>
              <button v-for="f in formats" :key="f.id" :class="{ on: format === f.id }" @click="format = f.id">{{ f.label }}</button>
            </div>
          </nav>

          <div class="sm-body">
            <div v-if="loading" class="state">
              <Icon name="refresh" :size="30" class="spin" />
              <p>Récupération des sets Smogon…</p>
            </div>
            <div v-else-if="error" class="state">
              <Icon name="alert" :size="30" />
              <p><strong>Sets indisponibles pour le moment.</strong></p>
              <p class="dim">
                Kaleido a besoin d'Internet la première fois pour télécharger les sets de Smogon ; ils sont ensuite gardés et marchent hors ligne.
              </p>
              <p class="dim small">{{ error }}</p>
              <button class="sv-btn" @click="load(true)"><Icon name="refresh" :size="14" /> Réessayer</button>
            </div>
            <div v-else-if="!shown.length" class="state">
              <Icon name="search" :size="30" />
              <p>Smogon ne propose pas de set pour {{ p.speciesName }} en Gen {{ saveState.view?.generation }}.</p>
              <p class="dim">Les Pokémon peu utilisés en compétition n'ont souvent pas d'analyse.</p>
            </div>
            <ul v-else class="sets">
              <li v-for="s in shown" :key="keyOfSet(s)" class="set">
                <div class="set-head">
                  <span class="fmt">{{ s.formatLabel }}</span>
                  <strong>{{ s.name }}</strong>
                  <span v-if="!s.sameForm" class="chip">{{ s.speciesKey }}</span>
                  <span class="grow" />
                  <span class="dim small">N. {{ s.resolved.level }}</span>
                </div>
                <dl>
                  <dt>Objet</dt>
                  <dd>
                    {{ s.resolved.itemName ?? s.set.item ?? "—" }}
                    <span v-if="s.options.items.length" class="dim"> ou {{ s.options.items.join(", ") }}</span>
                  </dd>
                  <dt>Talent</dt>
                  <dd>
                    {{ s.resolved.abilityName ?? "—" }}
                    <span v-if="s.options.abilities.length" class="dim"> ou {{ s.options.abilities.join(", ") }}</span>
                  </dd>
                  <dt>Nature</dt>
                  <dd>
                    {{ s.resolved.natureName ?? "—" }}
                    <span v-if="s.options.natures.length" class="dim"> ou {{ s.options.natures.join(", ") }}</span>
                  </dd>
                  <dt>EV <Tip term="ev" /></dt>
                  <dd>{{ statLine(s.resolved.evs, 0) || "aucun" }}</dd>
                  <template v-if="statLine(s.resolved.ivs, 31)">
                    <dt>IV <Tip term="iv" /></dt>
                    <dd>{{ statLine(s.resolved.ivs, 31) }}</dd>
                  </template>
                </dl>
                <ol class="moves">
                  <li v-for="(m, i) in s.resolved.moveNames" :key="i">
                    <span class="mv">{{ m }}</span>
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
                  <button class="sv-btn" :disabled="!!busy" @click="addNew(s)"><Icon name="plus" :size="14" /> Nouveau Pokémon</button>
                  <button class="sv-btn solid" :disabled="!!busy || p.isEgg" @click="apply(s)"><Icon name="wand" :size="14" /> Appliquer à {{ name }}</button>
                </div>
              </li>
            </ul>
          </div>
          <footer class="sm-foot">
            <p class="dim small">
              Sets <Tip term="showdownSet" /> issus des analyses de Smogon University (données pkmn/smogon). « Appliquer » remplace l'objet, le talent, la
              nature, les EV/IV, les attaques et le niveau ; le dresseur et la rencontre sont gardés. Ctrl+Z pour annuler.
            </p>
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.sm-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.sm-dialog {
  display: flex;
  flex-direction: column;
  width: min(980px, 100%);
  max-height: calc(100vh - 56px);
  overflow: hidden;
  background: var(--surface);
}

.sm-head,
.formats {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 12px 18px;
  border-bottom: 1px solid var(--border);
}

.title h2 {
  margin: 0;
  font-size: 19px;
}

.title small,
.dim {
  color: var(--text-dim);
}

.small {
  font-size: 12px;
}

.grow {
  flex: 1;
}

.round {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: transparent;
  color: var(--text);
}

.chip {
  padding: 2px 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  font-size: 12px;
  font-weight: 600;
}

.chip.warn {
  background: var(--warn-bg);
  color: var(--warn);
}

.sm-body {
  flex: 1;
  min-height: 240px;
  padding: 16px 18px;
  overflow: auto;
}

.state {
  display: grid;
  place-items: center;
  gap: 6px;
  padding: 40px 20px;
  text-align: center;
}

.state p {
  max-width: 460px;
  margin: 0;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.sets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
  gap: 12px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.set {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 4%, transparent);
}

.set-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.fmt {
  padding: 2px 8px;
  border-radius: 6px;
  background: var(--accent);
  color: var(--on-accent);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.04em;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 3px 12px;
  margin: 0;
  font-size: 13px;
}

dt {
  display: flex;
  align-items: center;
  color: var(--text-dim);
}

dd {
  margin: 0;
}

.moves {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 13px;
}

.mv {
  padding: 2px 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  font-weight: 600;
}

.alt {
  margin-left: 6px;
  font-size: 12px;
}

.warns,
.done {
  display: grid;
  gap: 3px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--warn);
  font-size: 12px;
}

.done {
  color: var(--accent-2);
}

.done .warn {
  color: var(--warn);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-top: auto;
}

.sm-foot {
  padding: 10px 18px;
  border-top: 1px solid var(--border);
}

.sm-foot p {
  margin: 0;
}

.sm-fade-enter-active,
.sm-fade-leave-active {
  transition: opacity 0.15s;
}

.sm-fade-enter-from,
.sm-fade-leave-to {
  opacity: 0;
}
</style>
