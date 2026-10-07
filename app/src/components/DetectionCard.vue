<script setup lang="ts">
import { computed } from "vue";
import { ref } from "vue";
import { openRom } from "../editor";
import { openSave } from "../saveStore";
import { library, removeItem } from "../library";
import { nav } from "../nav";
import { RANDOMIZABLE, isKaleidoRom, isRom as isRomFile, type Detection, type FileKind } from "../types";
import Icon from "./Icon.vue";

const props = defineProps<{ item: Detection }>();

const KIND_LABELS: Record<FileKind, string> = {
  gb_rom: "ROM",
  gba_rom: "ROM",
  nds_rom: "ROM",
  ctr_rom: "ROM",
  ctr_dump: "Dossier extrait",
  save: "Sauvegarde",
  unknown: "Inconnu",
  switch_game: "Jeu Switch",
};

const platformLabel = computed(() => (props.item.platform === "nds" ? "DS" : props.item.platform === "3ds" ? "3DS" : props.item.platform === "gba" ? "GBA" : props.item.platform === "gb" ? "GB" : null));
const isRom = computed(() => isRomFile(props.item));
const isSave = computed(() => props.item.kind === "save");
const canEdit = computed(() => isRomFile(props.item));
const randomized = computed(() => isKaleidoRom(props.item));
const canRandomize = computed(() => canEdit.value && !randomized.value && RANDOMIZABLE.includes(props.item.game?.id ?? ""));
/** Autre fichier de la bibliothèque au contenu identique. */
const duplicateOf = computed(() =>
  props.item.fingerprint ? library.items.find((d) => d.path !== props.item.path && d.fingerprint === props.item.fingerprint) : undefined,
);
const copiedCode = ref(false);
async function copyShareCode() {
  if (!props.item.kaleido) return;
  await navigator.clipboard.writeText(props.item.kaleido.shareCode);
  copiedCode.value = true;
  setTimeout(() => (copiedCode.value = false), 1500);
}

function randomize() {
  nav.randomizerRom = props.item.path;
  nav.view = "randomizer";
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  const units = ["o", "Ko", "Mo", "Go"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} ${units[i]}`;
}
</script>

<template>
  <article class="card panel" :class="[`kind-${item.kind}`, { unknown: item.kind === 'unknown' }]">
    <button type="button" class="remove" @click="removeItem(item.path)" aria-label="Retirer de la bibliothèque" title="Retirer">
      <Icon name="x" :size="15" />
    </button>

    <div class="chips">
      <span v-if="platformLabel" class="sv-chip accent">{{ platformLabel }}</span>
      <span class="sv-chip dim">{{ KIND_LABELS[item.kind] }}</span>
      <span v-if="item.generation" class="sv-chip dim">Gen {{ item.generation }}</span>
      <span v-if="item.language" class="sv-chip" :class="item.isFrench ? 'ok' : 'dim'">{{ item.language }}</span>
      <span v-if="randomized" class="sv-chip" title="ROM générée par Kaleido">Kaleido</span>
      <span v-if="duplicateOf" class="sv-chip warn" :title="`Contenu identique à : ${duplicateOf.fileName}`">Doublon</span>
    </div>

    <h3>{{ item.title }}</h3>
    <p class="file" :title="item.path">{{ item.fileName }} · {{ formatSize(item.size) }}</p>

    <dl v-if="item.details.length">
      <template v-for="d in item.details" :key="d.label">
        <dt>{{ d.label }}</dt>
        <dd :title="d.value">{{ d.value }}</dd>
      </template>
    </dl>

    <ul v-if="item.warnings.length || duplicateOf" class="warnings">
      <li v-for="w in item.warnings" :key="w">{{ w }}</li>
      <li v-if="duplicateOf">Même contenu que « {{ duplicateOf.fileName }} » : tu peux retirer l'un des deux.</li>
    </ul>

    <div class="actions">
      <template v-if="isRom">
        <button v-if="canRandomize" type="button" class="sv-btn solid" @click="randomize">Randomiser</button>
        <button v-else-if="item.kaleido" type="button" class="sv-btn solid" title="Copie le code pour régénérer la même ROM à partir de l'originale" @click="copyShareCode">
          {{ copiedCode ? "Code copié !" : "Copier le code" }}
        </button>
        <button v-else type="button" class="sv-btn solid" disabled :title="randomized ? 'Déjà randomisée : pars de la ROM d\'origine' : 'Pas encore pris en charge par le randomizer'">
          Randomiser
        </button>
        <button v-if="canEdit" type="button" class="sv-btn" @click="openRom(item.path)">Éditer</button>
        <button v-else type="button" class="sv-btn" disabled title="Jeu non identifié">Éditer</button>
      </template>
      <button v-else-if="isSave" type="button" class="sv-btn solid" @click="openSave(item.path)">Ouvrir la sauvegarde</button>
    </div>
  </article>
</template>

<style scoped>
.card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 20px;
  overflow: hidden;
}

/* Fichier non reconnu : carte en retrait plutôt qu'un liseré coloré. */
.card.unknown {
  border-style: dashed;
}

.remove {
  position: absolute;
  top: 12px;
  right: 12px;
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  opacity: 0;
  transition: opacity 0.15s, background-color 0.15s, color 0.15s;
}

/* Visible au survol de la carte, mais aussi au clavier et à la manette. */
.card:hover .remove,
.card:focus-within .remove {
  opacity: 1;
}

.remove:hover {
  background: color-mix(in srgb, var(--text) 12%, transparent);
  color: var(--text);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding-right: 28px;
}

h3 {
  font-size: 20px;
  font-weight: 700;
}

.file {
  margin: -4px 0 0;
  color: var(--text-dim);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 14px;
  margin: 4px 0 0;
  font-size: 13px;
}

dt {
  color: var(--text-dim);
}

dd {
  margin: 0;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: text;
}

.warnings {
  margin: 4px 0 0;
  padding: 10px 12px 10px 28px;
  border-radius: var(--radius-sm);
  background: var(--warn-bg);
  color: var(--warn);
  font-size: 13px;
}

.actions {
  display: flex;
  gap: 8px;
  margin-top: auto;
  padding-top: 8px;
}

.actions:empty {
  display: none;
}
</style>
