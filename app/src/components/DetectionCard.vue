<script setup lang="ts">
import { computed } from "vue";
import { openRom } from "../editor";
import { removeItem } from "../library";
import type { Detection, FileKind } from "../types";

const props = defineProps<{ item: Detection }>();

const KIND_LABELS: Record<FileKind, string> = {
  nds_rom: "ROM",
  ctr_rom: "ROM",
  ctr_dump: "Dossier extrait",
  save: "Sauvegarde",
  unknown: "Inconnu",
};

const platformLabel = computed(() => (props.item.platform === "nds" ? "DS" : props.item.platform === "3ds" ? "3DS" : null));
const isRom = computed(() => ["nds_rom", "ctr_rom", "ctr_dump"].includes(props.item.kind) && props.item.game !== null);
const isSave = computed(() => props.item.kind === "save");
const canEdit = computed(() => props.item.kind === "nds_rom" && props.item.game !== null);

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  const units = ["o", "Ko", "Mo", "Go"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toLocaleString("fr-FR", { maximumFractionDigits: 1 })} ${units[i]}`;
}
</script>

<template>
  <article class="card panel" :class="[`kind-${item.kind}`, { unknown: item.kind === 'unknown' }]">
    <button class="remove" @click="removeItem(item.path)" aria-label="Retirer de la bibliothèque" title="Retirer">×</button>

    <div class="chips">
      <span v-if="platformLabel" class="chip chip-accent">{{ platformLabel }}</span>
      <span class="chip">{{ KIND_LABELS[item.kind] }}</span>
      <span v-if="item.generation" class="chip">Gen {{ item.generation }}</span>
      <span v-if="item.language" class="chip" :class="{ fr: item.isFrench }">{{ item.language }}</span>
    </div>

    <h3>{{ item.title }}</h3>
    <p class="file" :title="item.path">{{ item.fileName }} · {{ formatSize(item.size) }}</p>

    <dl v-if="item.details.length">
      <template v-for="d in item.details" :key="d.label">
        <dt>{{ d.label }}</dt>
        <dd :title="d.value">{{ d.value }}</dd>
      </template>
    </dl>

    <ul v-if="item.warnings.length" class="warnings">
      <li v-for="w in item.warnings" :key="w">{{ w }}</li>
    </ul>

    <div class="actions">
      <template v-if="isRom">
        <button class="btn btn-primary" disabled title="Arrive en phase 2">Randomiser</button>
        <button v-if="canEdit" class="btn" @click="openRom(item.path)">Explorer</button>
        <button v-else class="btn" disabled title="Jeux 3DS : arrive en phase 3">Explorer</button>
      </template>
      <button v-else-if="isSave" class="btn btn-primary" disabled title="Arrive en phase 4">Ouvrir la sauvegarde</button>
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

/* Liseré prismatique en haut de la carte */
.card::before {
  content: "";
  position: absolute;
  inset: 0 0 auto;
  height: 3px;
  background: var(--prism);
}

.card.unknown::before {
  background: var(--border);
}

.remove {
  position: absolute;
  top: 12px;
  right: 12px;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  font-size: 18px;
  line-height: 1;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}

.card:hover .remove {
  opacity: 1;
}

.remove:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding-right: 28px;
}

.chip.fr {
  color: var(--accent-2);
  border-color: color-mix(in srgb, var(--accent-2) 45%, transparent);
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
