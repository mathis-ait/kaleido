<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Banner from "../../components/Banner.vue";
import Dialog from "../../components/Dialog.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import Segmented from "../../components/Segmented.vue";
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

const TABS: { value: "import" | "export"; label: string }[] = [
  { value: "import", label: "Importer" },
  { value: "export", label: "Exporter" },
];

// ---- Importer
const text = ref("");
const preview = ref<ShowdownPreview | null>(null);
const previewError = ref<string | null>(null);
const report = ref<ImportReport | null>(null);
const importError = ref<string | null>(null);
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
const targetOptions = computed(() => [
  { value: "box" as TargetKind, label: `Cases libres de ${boxNames.value[saveState.box] ?? "la boîte"}` },
  { value: "party" as TargetKind, label: "Équipe", disabled: (view.value?.party.length ?? 6) >= 6 },
  {
    value: "slot" as TargetKind,
    label: `${targetSlot.value ? slotText(targetSlot.value, boxNames.value) : "Emplacement choisi"}${targetOccupied.value ? " (remplace)" : ""}`,
    hint: targetOccupied.value ? "Le Pokémon de cet emplacement sera remplacé" : undefined,
    disabled: !targetSlot.value,
  },
]);

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
  importError.value = null;
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
  importError.value = null;
  try {
    report.value = await importShowdown(text.value, target.value);
  } catch (e) {
    importError.value = String(e);
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
const exportError = ref<string | null>(null);
const exporting = ref(false);

const sourceSlots = computed<Slot[]>(() => {
  if (source.value === "selected") return saveState.selected ? [saveState.selected.slot] : [];
  if (source.value === "party") return (view.value?.party ?? []).map((p) => p.slot);
  return saveState.slots.filter((s) => !!s).map((s) => s!.slot);
});
const sourceOptions = computed(() => [
  { value: "party" as Source, label: "Équipe" },
  { value: "box" as Source, label: boxNames.value[saveState.box] ?? "Boîte" },
  {
    value: "selected" as Source,
    label: saveState.selected ? saveState.selected.nickname || saveState.selected.speciesName : "Pokémon sélectionné",
    disabled: !saveState.selected,
  },
]);
const LANGS: { value: Lang; label: string }[] = [
  { value: "en", label: "Anglais (Showdown)" },
  { value: "fr", label: "Français" },
];

// ---- Ouverture / fermeture et clavier

// À chaque ouverture (y compris quand la fenêtre est déjà ouverte au montage, par exemple
// `goTo("boxes")` suivi d'`openShowdown("export")` dans le même tour) : cible et source par défaut.
// Déclaré avant le calcul de l'export pour que celui-ci parte de la bonne source.
watch(
  () => showdownUi.open,
  (open) => {
    if (!open) return;
    report.value = null;
    importError.value = null;
    targetKind.value = targetSlot.value && !targetOccupied.value && props.target ? "slot" : "box";
    source.value = saveState.selected ? "selected" : "party";
  },
  { immediate: true },
);

let exportRequest = 0;
async function refreshExport() {
  if (!showdownUi.open || showdownUi.tab !== "export") return;
  const id = ++exportRequest;
  const slots = sourceSlots.value;
  exportError.value = null;
  if (!slots.length) {
    exported.value = "";
    return;
  }
  exporting.value = true;
  try {
    const out = await exportShowdown(slots, lang.value);
    // Réponse d'une demande dépassée (source ou langue changée entre-temps) : ignorée.
    if (id === exportRequest) exported.value = out;
  } catch (e) {
    if (id === exportRequest) {
      exported.value = "";
      exportError.value = String(e);
    }
  } finally {
    if (id === exportRequest) exporting.value = false;
  }
}
// `immediate` : la fenêtre peut être montée déjà ouverte sur l'onglet Exporter, auquel cas
// aucun changement ne déclencherait le calcul (c'était la cause du texte vide).
watch([source, lang, sourceSlots, () => showdownUi.tab, () => showdownUi.open], refreshExport, { immediate: true });

const exportPlaceholder = computed(() => {
  if (exporting.value) return "Préparation du texte…";
  if (!sourceSlots.value.length) return "Aucun Pokémon à exporter ici.";
  return "Rien à exporter : les œufs sont ignorés.";
});

async function copy() {
  try {
    await navigator.clipboard.writeText(exported.value);
    notify(lang.value === "en" ? "Copié : colle-le dans le constructeur d'équipe de Showdown (Import from text)" : "Texte copié");
  } catch {
    notify("Presse-papiers inaccessible : sélectionne le texte puis Ctrl+C");
  }
}

function close() {
  showdownUi.open = false;
}

/** Fenêtre fermée : Ctrl+I l'ouvre depuis les boîtes ou la fiche Pokémon. */
function onWindowKey(e: KeyboardEvent) {
  if (showdownUi.open || keyOf(e) !== "Ctrl+i" || !saveState.view) return;
  e.preventDefault();
  e.stopPropagation();
  openShowdown("import");
}

/**
 * Fenêtre ouverte : ses raccourcis, puis on arrête la touche avant les raccourcis de la page.
 * Écouté sur `document` (phase de remontée) pour laisser la fenêtre gérer Échap et Tab avant.
 */
function onDialogKey(e: KeyboardEvent) {
  if (!showdownUi.open) return;
  e.stopPropagation();
  const k = keyOf(e);
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

watch(
  () => showdownUi.tab,
  (tab) => {
    if (showdownUi.open && tab === "import") nextTick(() => area.value?.focus());
  },
);

onMounted(() => {
  window.addEventListener("keydown", onWindowKey, true);
  document.addEventListener("keydown", onDialogKey);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onWindowKey, true);
  document.removeEventListener("keydown", onDialogKey);
  showdownUi.open = false;
});

function showSlot(s: Slot) {
  close();
  if (s.kind === "box") goTo("boxes");
}
</script>

<template>
  <Dialog v-model="showdownUi.open" title="Showdown" term="showdown" icon="swords" :width="1040">
    <template #head>
      <Segmented v-model="showdownUi.tab" :options="TABS" label="Importer ou exporter" />
    </template>

    <!-- Importer -->
    <div v-if="showdownUi.tab === 'import'" class="sd-import">
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
          <button type="button" class="sv-btn" @click="pasteClipboard"><Icon name="copy" :size="14" /> Coller</button>
          <button type="button" class="sv-btn" @click="useSample"><Icon name="wand" :size="14" /> Exemple</button>
          <button v-if="text" type="button" class="sv-btn" @click="text = ''"><Icon name="trash" :size="14" /> Vider</button>
        </div>
        <p class="sv-help">
          Un Pokémon par bloc, séparés par une ligne vide. Noms anglais ou français, accents facultatifs. Sans « Level », le niveau est 100 comme sur
          Showdown.
        </p>
      </div>

      <div class="col preview">
        <div class="sv-row">
          <span class="sv-label">Aperçu</span>
          <span v-if="preview?.sets.length" class="sv-chip dim">Noms en {{ langLabel(preview.lang) }}</span>
          <span v-if="warningCount" class="sv-chip warn">{{ warningCount }} remarque{{ warningCount > 1 ? "s" : "" }}</span>
        </div>
        <Banner v-if="previewError">{{ previewError }}</Banner>
        <Banner v-if="importError" :dismiss="() => (importError = null)">{{ importError }}</Banner>
        <EmptyState v-if="!preview?.sets.length" icon="file" compact class="sd-empty">
          Les Pokémon reconnus apparaîtront ici, avec leurs noms en français.
        </EmptyState>
        <ul v-else class="cards">
          <li v-for="(s, i) in preview.sets" :key="i" class="card" :class="{ bad: !!s.error }">
            <div class="card-head">
              <Sprite v-if="s.species" :id="s.species" :shiny="s.shiny" :size="56" />
              <div class="who">
                <strong>
                  {{ s.nickname || s.speciesName }}
                  <span v-if="s.gender === 'male'" class="male" title="Mâle">♂</span>
                  <span v-else-if="s.gender === 'female'" class="female" title="Femelle">♀</span>
                </strong>
                <small>
                  <template v-if="s.nickname">{{ s.speciesName }} · </template>
                  <template v-if="s.formName">{{ s.formName }} · </template>
                  N. {{ s.level }}
                </small>
              </div>
              <span v-if="s.shiny" class="sv-chip shiny">Chromatique</span>
            </div>
            <p v-if="s.error" class="err">{{ s.error }}</p>
            <template v-else>
              <dl class="sv-dl">
                <dt>Objet tenu <Tip term="heldItem" /></dt>
                <dd>{{ s.itemName ?? "—" }}</dd>
                <dt>Talent <Tip term="ability" /></dt>
                <dd>
                  {{ s.abilityName ?? "—" }}<span v-if="s.abilityNumber === 4" class="dim hidden"> (caché <Tip term="hiddenAbility" />)</span>
                </dd>
                <dt>Nature <Tip term="nature" /></dt>
                <dd>{{ s.natureName ?? "au hasard" }}</dd>
                <dt>EV <Tip term="ev" /></dt>
                <dd>{{ statLine(s.evs, 0) || "aucun" }}</dd>
                <template v-if="statLine(s.ivs, 31)">
                  <dt>IV <Tip term="iv" /></dt>
                  <dd>{{ statLine(s.ivs, 31) }} <span class="dim">(31 ailleurs)</span></dd>
                </template>
              </dl>
              <ul class="sv-moves">
                <li v-for="m in s.moveNames" :key="m">{{ m }}</li>
              </ul>
            </template>
            <ul v-if="s.warnings.length" class="warns">
              <li v-for="w in s.warnings" :key="w"><Icon name="alert" :size="13" /> {{ w }}</li>
            </ul>
          </li>
        </ul>
      </div>
    </div>

    <!-- Exporter -->
    <div v-else class="sd-export">
      <div class="options">
        <div class="sv-field">
          <span class="sv-label">Pokémon à exporter</span>
          <Segmented v-model="source" :options="sourceOptions" label="Pokémon à exporter" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Langue des noms <Tip term="showdownLang" /></span>
          <Segmented v-model="lang" :options="LANGS" label="Langue des noms" />
        </div>
      </div>
      <Banner v-if="exportError" :retry="refreshExport">{{ exportError }}</Banner>
      <textarea class="sd-text out" readonly spellcheck="false" :value="exported" :placeholder="exportPlaceholder" />
      <p class="sv-help">Les œufs sont ignorés. Sur Showdown : constructeur d'équipe → « Import from text », puis colle le texte.</p>
    </div>

    <template #foot>
      <div class="foot">
        <template v-if="showdownUi.tab === 'import'">
          <div v-if="report" class="result">
            <strong>{{ report.imported ? `${report.imported} Pokémon ajouté${report.imported > 1 ? "s" : ""}` : "Rien n'a été ajouté" }}</strong>
            <span v-for="(r, i) in report.sets" :key="i" class="placed" :class="{ bad: !!r.error }">
              <template v-if="r.slot">
                <button type="button" class="link" @click="showSlot(r.slot)">{{ r.speciesName }} → {{ slotText(r.slot, boxNames) }}</button>
              </template>
              <template v-else>{{ r.speciesName }} : {{ r.error }}</template>
            </span>
          </div>
          <template v-else>
            <span class="sv-label">Ranger dans</span>
            <Segmented v-model="targetKind" :options="targetOptions" label="Ranger dans" />
          </template>
          <span class="grow" />
          <button v-if="report" type="button" class="sv-btn" @click="(report = null), (text = '')">Importer autre chose</button>
          <button v-else type="button" class="sv-btn solid" :disabled="!valid.length || busy" @click="doImport">
            <Icon name="plus" :size="15" />
            Ajouter à la sauvegarde{{ valid.length > 1 ? ` (${valid.length})` : "" }}
            <kbd>Ctrl+Entrée</kbd>
          </button>
        </template>
        <template v-else>
          <span class="dim">{{ sourceSlots.length }} Pokémon</span>
          <span class="grow" />
          <button type="button" class="sv-btn solid" :disabled="!exported" @click="copy"><Icon name="copy" :size="15" /> Copier le texte</button>
        </template>
      </div>
    </template>
  </Dialog>
</template>

<style scoped>
.sd-import {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  gap: var(--sp-4);
  min-height: min(480px, calc(100vh - 260px));
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
  padding: var(--sp-3) 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: var(--text);
  font: var(--fs-md) / 1.5 "Cascadia Mono", Consolas, monospace;
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

.sd-empty {
  flex: 1;
  width: 100%;
  max-width: none;
  border: 1px dashed var(--border);
  border-radius: var(--radius-card);
  color: var(--text-dim);
}

.cards {
  display: grid;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: 10px var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
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
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.who small,
.dim {
  color: var(--text-dim);
}

.male {
  color: var(--male);
}

.female {
  color: var(--female);
}

.hidden {
  display: inline-flex;
  align-items: center;
  font-weight: 400;
}

.warns {
  display: grid;
  gap: 3px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--warn);
  font-size: var(--fs-sm);
}

.err {
  margin: 0;
  color: var(--danger);
  font-size: var(--fs-md);
}

.sd-export {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.options {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-4);
}

.foot {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
  min-width: 0;
}

.grow {
  flex: 1;
}

kbd {
  padding: 1px 6px;
  border: 1px solid color-mix(in srgb, var(--bg) 40%, transparent);
  border-radius: var(--radius-xs);
  color: inherit;
  font: 600 var(--fs-xs) var(--font);
  opacity: 0.7;
}

.result {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px var(--sp-3);
  font-size: var(--fs-md);
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

@media (max-width: 760px) {
  .sd-import {
    grid-template-columns: 1fr;
  }
}
</style>
