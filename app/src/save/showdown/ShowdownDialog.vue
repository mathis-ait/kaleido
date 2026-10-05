<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { goTo, notify, saveState } from "../../saveStore";
import type { Slot } from "../../types";
import { keyOf, typing } from "../shell";
import {
  exportShowdown,
  importShowdown,
  openShowdown,
  previewShowdown,
  showdownUi,
  slotText,
  statLine,
  type ImportReport,
  type ImportTarget,
  type Lang,
  type ShowdownPreview,
} from "./api";

/**
 * Fenêtre « Showdown » : importer une équipe écrite au format texte de Pokémon
 * Showdown (anglais ou français), ou exporter des Pokémon de la sauvegarde.
 * `target` : emplacement choisi dans la page (case vide ou Pokémon sélectionné).
 */
const props = defineProps<{ target?: Slot | null }>();

const view = computed(() => saveState.view);
const boxNames = computed(() => view.value?.boxNames ?? []);

// ---- Importer
const text = ref("");
const preview = ref<ShowdownPreview | null>(null);
const previewError = ref<string | null>(null);
const report = ref<ImportReport | null>(null);
const busy = ref(false);
const area = ref<HTMLTextAreaElement | null>(null);

type TargetKind = "box" | "party" | "slot";
const targetKind = ref<TargetKind>("box");
const targetSlot = computed(() => props.target ?? saveState.selected?.slot ?? null);
const targetOccupied = computed(() => {
  const s = targetSlot.value;
  if (!s) return false;
  if (s.kind === "party") return s.index < (view.value?.party.length ?? 0);
  return s.box === saveState.box && !!saveState.slots[s.index];
});

const SAMPLE = `Carchacrok (F) @ Mouchoir Choix
Talent : Peau Dure
EVs : 252 Att / 4 Déf Spé / 252 Vit
Nature : Jovial
- Séisme
- Colère
- Lame de Roc
- Crocs Feu`;

let timer: number | undefined;
watch(text, (t) => {
  report.value = null;
  clearTimeout(timer);
  if (!t.trim()) {
    preview.value = null;
    previewError.value = null;
    return;
  }
  timer = window.setTimeout(async () => {
    try {
      preview.value = await previewShowdown(t);
      previewError.value = null;
    } catch (e) {
      previewError.value = String(e);
    }
  }, 250);
});

const valid = computed(() => preview.value?.sets.filter((s) => !s.error) ?? []);
const warningCount = computed(() => preview.value?.sets.reduce((n, s) => n + s.warnings.length, 0) ?? 0);
const langLabel = (l: Lang) => (l === "en" ? "anglais (Showdown)" : "français");

const target = computed<ImportTarget>(() => {
  if (targetKind.value === "party") return { kind: "party" };
  if (targetKind.value === "slot" && targetSlot.value) return { kind: "slot", slot: targetSlot.value };
  return { kind: "box", box: saveState.box };
});

async function doImport() {
  if (!valid.value.length || busy.value) return;
  busy.value = true;
  try {
    report.value = await importShowdown(text.value, target.value);
  } catch (e) {
    saveState.error = String(e);
  } finally {
    busy.value = false;
  }
}

function useSample() {
  text.value = SAMPLE;
  nextTick(() => area.value?.focus());
}

async function pasteClipboard() {
  try {
    text.value = await navigator.clipboard.readText();
  } catch {
    notify("Presse-papiers inaccessible : colle le texte avec Ctrl+V");
  }
}

// ---- Exporter
type Source = "party" | "box" | "selected";
const source = ref<Source>("party");
const lang = ref<Lang>("en");
const exported = ref("");

const sourceSlots = computed<Slot[]>(() => {
  if (source.value === "selected") return saveState.selected ? [saveState.selected.slot] : [];
  if (source.value === "party") return (view.value?.party ?? []).map((p) => p.slot);
  return saveState.slots.filter((s) => !!s).map((s) => s!.slot);
});

async function refreshExport() {
  if (!showdownUi.open || showdownUi.tab !== "export") return;
  try {
    exported.value = sourceSlots.value.length ? await exportShowdown(sourceSlots.value, lang.value) : "";
  } catch (e) {
    exported.value = "";
    saveState.error = String(e);
  }
}
watch([source, lang, sourceSlots, () => showdownUi.tab, () => showdownUi.open], refreshExport);

async function copy() {
  try {
    await navigator.clipboard.writeText(exported.value);
    notify(lang.value === "en" ? "Copié : colle-le dans le constructeur d'équipe de Showdown (Import from text)" : "Texte copié");
  } catch {
    notify("Presse-papiers inaccessible : sélectionne le texte puis Ctrl+C");
  }
}

// ---- Ouverture / fermeture et clavier
function close() {
  showdownUi.open = false;
}

watch(
  () => showdownUi.open,
  (open) => {
    if (!open) return;
    report.value = null;
    targetKind.value = targetSlot.value && !targetOccupied.value && props.target ? "slot" : "box";
    source.value = saveState.selected ? "selected" : "party";
    if (showdownUi.tab === "import") nextTick(() => area.value?.focus());
  },
);

function onKey(e: KeyboardEvent) {
  const k = keyOf(e);
  if (!showdownUi.open) {
    // Ctrl+I : ouvre la fenêtre Showdown depuis les boîtes ou la fiche Pokémon.
    if (k === "Ctrl+i" && saveState.view) {
      e.preventDefault();
      e.stopPropagation();
      openShowdown("import");
    }
    return;
  }
  // Fenêtre ouverte : les raccourcis de la page sont suspendus.
  e.stopPropagation();
  if (k === "Escape") {
    e.preventDefault();
    close();
  } else if (k === "Ctrl+Enter" && showdownUi.tab === "import") {
    e.preventDefault();
    doImport();
  } else if (k === "Ctrl+Tab" || (k === "Ctrl+i" && !typing(e))) {
    e.preventDefault();
    showdownUi.tab = showdownUi.tab === "import" ? "export" : "import";
  }
}

onMounted(() => window.addEventListener("keydown", onKey, true));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  showdownUi.open = false;
});

function showSlot(s: Slot) {
  close();
  if (s.kind === "box") goTo("boxes");
}
</script>

<template>
  <Teleport to="body">
    <Transition name="sd-fade">
      <div v-if="showdownUi.open" class="sd-overlay" @pointerdown.self="close">
        <section class="sd-dialog sv-panel" role="dialog" aria-modal="true" aria-label="Showdown">
          <header class="sd-head">
            <h2>Showdown <Tip term="showdown" /></h2>
            <div class="sv-seg">
              <button :class="{ on: showdownUi.tab === 'import' }" @click="showdownUi.tab = 'import'"><Icon name="download" :size="14" /> Importer</button>
              <button :class="{ on: showdownUi.tab === 'export' }" @click="showdownUi.tab = 'export'"><Icon name="upload" :size="14" /> Exporter</button>
            </div>
            <span class="grow" />
            <kbd class="hint">Échap</kbd>
            <button class="round" aria-label="Fermer" @click="close"><Icon name="x" :size="16" /></button>
          </header>

          <!-- Importer -->
          <div v-if="showdownUi.tab === 'import'" class="sd-body import">
            <div class="col">
              <label class="sv-label" for="sd-text">Texte de l'équipe <Tip term="showdownLang" /></label>
              <textarea
                id="sd-text"
                ref="area"
                v-model="text"
                class="sd-text"
                spellcheck="false"
                :placeholder="'Colle ici une équipe exportée de Showdown, par exemple :\n\n' + SAMPLE"
              />
              <div class="sv-row">
                <button class="sv-btn" @click="pasteClipboard"><Icon name="copy" :size="14" /> Coller</button>
                <button class="sv-btn" @click="useSample"><Icon name="wand" :size="14" /> Exemple</button>
                <button v-if="text" class="sv-btn" @click="text = ''"><Icon name="trash" :size="14" /> Vider</button>
              </div>
              <p class="sv-help">
                Un Pokémon par bloc, séparés par une ligne vide. Noms anglais ou français, accents facultatifs. Sans « Level », le niveau est 100 comme sur
                Showdown.
              </p>
            </div>

            <div class="col preview">
              <div class="preview-head">
                <span class="sv-label">Aperçu</span>
                <span v-if="preview?.sets.length" class="chip">Noms en {{ langLabel(preview.lang) }}</span>
                <span v-if="warningCount" class="chip warn">{{ warningCount }} remarque{{ warningCount > 1 ? "s" : "" }}</span>
              </div>
              <p v-if="previewError" class="err">{{ previewError }}</p>
              <div v-if="!preview?.sets.length" class="empty">
                <Icon name="file" :size="30" />
                <p>Les Pokémon reconnus apparaîtront ici, avec leurs noms en français.</p>
              </div>
              <ul v-else class="cards">
                <li v-for="(s, i) in preview.sets" :key="i" class="card" :class="{ bad: !!s.error }">
                  <div class="card-head">
                    <Sprite v-if="s.species" :id="s.species" :shiny="s.shiny" :size="56" />
                    <div class="who">
                      <strong>
                        {{ s.nickname || s.speciesName }}
                        <span v-if="s.shiny" class="gold" title="Chromatique">★</span>
                        <span v-if="s.gender === 'male'" class="g m">♂</span>
                        <span v-else-if="s.gender === 'female'" class="g f">♀</span>
                      </strong>
                      <small>
                        <template v-if="s.nickname">{{ s.speciesName }} · </template>
                        <template v-if="s.formName">{{ s.formName }} · </template>
                        N. {{ s.level }}
                      </small>
                    </div>
                  </div>
                  <p v-if="s.error" class="err">{{ s.error }}</p>
                  <template v-else>
                    <dl>
                      <dt>Objet</dt>
                      <dd>{{ s.itemName ?? "—" }}</dd>
                      <dt>Talent</dt>
                      <dd>{{ s.abilityName ?? "—" }}<span v-if="s.abilityNumber === 4" class="dim"> (caché)</span></dd>
                      <dt>Nature</dt>
                      <dd>{{ s.natureName ?? "au hasard" }}</dd>
                      <dt>EV</dt>
                      <dd>{{ statLine(s.evs, 0) || "aucun" }}</dd>
                      <template v-if="statLine(s.ivs, 31)">
                        <dt>IV</dt>
                        <dd>{{ statLine(s.ivs, 31) }} <span class="dim">(31 ailleurs)</span></dd>
                      </template>
                    </dl>
                    <div class="moves">
                      <span v-for="m in s.moveNames" :key="m">{{ m }}</span>
                    </div>
                  </template>
                  <ul v-if="s.warnings.length" class="warns">
                    <li v-for="w in s.warnings" :key="w"><Icon name="alert" :size="13" /> {{ w }}</li>
                  </ul>
                </li>
              </ul>
            </div>
          </div>

          <!-- Exporter -->
          <div v-else class="sd-body export">
            <div class="options">
              <div class="sv-field">
                <span class="sv-label">Pokémon à exporter</span>
                <div class="sv-seg">
                  <button :class="{ on: source === 'party' }" @click="source = 'party'">Équipe</button>
                  <button :class="{ on: source === 'box' }" @click="source = 'box'">{{ boxNames[saveState.box] ?? "Boîte" }}</button>
                  <button :class="{ on: source === 'selected' }" :disabled="!saveState.selected" @click="source = 'selected'">
                    {{ saveState.selected ? saveState.selected.nickname || saveState.selected.speciesName : "Pokémon sélectionné" }}
                  </button>
                </div>
              </div>
              <div class="sv-field">
                <span class="sv-label">Langue des noms <Tip term="showdownLang" /></span>
                <div class="sv-seg">
                  <button :class="{ on: lang === 'en' }" @click="lang = 'en'">Anglais (Showdown)</button>
                  <button :class="{ on: lang === 'fr' }" @click="lang = 'fr'">Français</button>
                </div>
              </div>
            </div>
            <textarea class="sd-text out" readonly spellcheck="false" :value="exported" :placeholder="'Aucun Pokémon à exporter ici.'" />
            <p class="sv-help">
              Les œufs sont ignorés. Sur Showdown : constructeur d'équipe → « Import from text », puis colle le texte.
            </p>
          </div>

          <footer class="sd-foot">
            <template v-if="showdownUi.tab === 'import'">
              <div v-if="report" class="result">
                <strong>{{ report.imported ? `${report.imported} Pokémon ajouté${report.imported > 1 ? "s" : ""}` : "Rien n'a été ajouté" }}</strong>
                <span v-for="(r, i) in report.sets" :key="i" class="placed" :class="{ bad: !!r.error }">
                  <template v-if="r.slot">
                    <button class="link" @click="showSlot(r.slot)">{{ r.speciesName }} → {{ slotText(r.slot, boxNames) }}</button>
                  </template>
                  <template v-else>{{ r.speciesName }} : {{ r.error }}</template>
                </span>
              </div>
              <template v-else>
                <span class="sv-label">Ranger dans</span>
                <div class="sv-seg">
                  <button :class="{ on: targetKind === 'box' }" @click="targetKind = 'box'">Cases libres de {{ boxNames[saveState.box] ?? "la boîte" }}</button>
                  <button :class="{ on: targetKind === 'party' }" :disabled="(view?.party.length ?? 6) >= 6" @click="targetKind = 'party'">Équipe</button>
                  <button
                    :class="{ on: targetKind === 'slot' }"
                    :disabled="!targetSlot"
                    :title="targetOccupied ? 'Le Pokémon de cet emplacement sera remplacé' : ''"
                    @click="targetKind = 'slot'"
                  >
                    {{ targetSlot ? slotText(targetSlot, boxNames) : "Emplacement choisi" }}{{ targetOccupied ? " (remplace)" : "" }}
                  </button>
                </div>
              </template>
              <span class="grow" />
              <button v-if="report" class="sv-btn" @click="(report = null), (text = '')">Importer autre chose</button>
              <button v-else class="sv-btn solid" :disabled="!valid.length || busy" @click="doImport">
                <Icon name="plus" :size="15" />
                Ajouter à la sauvegarde{{ valid.length > 1 ? ` (${valid.length})` : "" }}
                <kbd>Ctrl+Entrée</kbd>
              </button>
            </template>
            <template v-else>
              <span class="dim">{{ sourceSlots.length }} Pokémon</span>
              <span class="grow" />
              <button class="sv-btn solid" :disabled="!exported" @click="copy"><Icon name="copy" :size="15" /> Copier le texte</button>
            </template>
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.sd-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.sd-dialog {
  display: flex;
  flex-direction: column;
  width: min(1040px, 100%);
  max-height: calc(100vh - 56px);
  min-height: min(620px, calc(100vh - 56px));
  overflow: hidden;
  background: var(--surface);
}

.sd-head,
.sd-foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  padding: 14px 18px;
}

.sd-head {
  border-bottom: 1px solid var(--border);
}

.sd-head h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 700;
}

.sd-foot {
  border-top: 1px solid var(--border);
}

.grow {
  flex: 1;
}

.hint,
kbd {
  padding: 1px 6px;
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text-dim);
  font: 600 11px var(--font);
}

.sv-btn.solid kbd {
  border-color: color-mix(in srgb, var(--bg) 40%, transparent);
  color: inherit;
  opacity: 0.7;
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

.sd-body {
  flex: 1;
  min-height: 0;
  padding: 16px 18px;
  overflow: auto;
}

.sd-body.import {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  gap: 18px;
}

.col {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
}

.sd-text {
  flex: 1;
  min-height: 300px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: var(--text);
  font: 13px/1.5 "Cascadia Mono", Consolas, monospace;
  resize: none;
  outline: none;
}

.sd-text:focus {
  border-color: var(--accent-2);
}

.sd-text.out {
  width: 100%;
  min-height: 340px;
}

.preview {
  overflow: auto;
}

.preview-head {
  display: flex;
  align-items: center;
  gap: 8px;
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

.empty {
  display: grid;
  flex: 1;
  place-items: center;
  align-content: center;
  gap: 8px;
  padding: 30px;
  border: 1px dashed var(--border);
  border-radius: var(--radius);
  color: var(--text-dim);
  text-align: center;
}

.empty p {
  margin: 0;
  max-width: 280px;
}

.cards {
  display: grid;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.card {
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 4%, transparent);
}

.card.bad {
  border-color: var(--danger);
}

.card-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.who {
  display: flex;
  flex-direction: column;
}

.who small,
.dim {
  color: var(--text-dim);
}

.gold {
  color: #f5b301;
}

.g.m {
  color: #4a90e2;
}

.g.f {
  color: #e2507a;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 2px 12px;
  margin: 8px 0 6px;
  font-size: 13px;
}

dt {
  color: var(--text-dim);
}

dd {
  margin: 0;
}

.moves {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.moves span {
  padding: 2px 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  font-size: 12px;
  font-weight: 600;
}

.warns {
  display: grid;
  gap: 3px;
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  color: var(--warn);
  font-size: 12px;
}

.err {
  margin: 6px 0 0;
  color: var(--danger);
  font-size: 13px;
}

.export {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.options {
  display: flex;
  flex-wrap: wrap;
  gap: 18px;
}

.result {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 12px;
  font-size: 13px;
}

.placed.bad {
  color: var(--danger);
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--accent-2);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}

.sd-fade-enter-active,
.sd-fade-leave-active {
  transition: opacity 0.15s;
}

.sd-fade-enter-from,
.sd-fade-leave-to {
  opacity: 0;
}

@media (max-width: 760px) {
  .sd-body.import {
    grid-template-columns: 1fr;
  }
}
</style>
