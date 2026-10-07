<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Combo, { type ComboOption } from "../../components/Combo.vue";
import Tip from "../../components/Tip.vue";
import { lists, saveState } from "../../saveStore";
import type { Memory, SlotView } from "../../types";
import { apply, num } from "./edit";

const props = defineProps<{ p: SlotView }>();
const gen = computed(() => saveState.view!.generation);
const h = computed(() => props.p.extras.handler!);

/**
 * Type de la variable de chaque souvenir (PKHeX `Memories.ArgTypes`) : 0 aucune, 1 lieu
 * général, 2 lieu précis (lieu général en Gen 7), 3 espèce, 4 attaque, 5 objet.
 */
const ARG_TYPES = [
  0, 1, 1, 1, 1, 5, 2, 3, 0, 3, 0, 0, 4, 3, 3, 5, 4, 3, 3, 1, 0, 3, 0, 0, 1, 3, 5, 0, 0, 3, 0, 1, 1, 1, 5, 1, 1, 1, 1, 1, 5, 0, 1, 0, 3, 3, 0, 0, 4, 4,
  3, 5, 1, 0, 0, 0, 0, 0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 3, 3, 0, 0, 3, 0, 0, 0, 0, 4, 4, 3, 3, 5, 0, 1, 3, 5, 4,
];
const ARG_LABELS = ["", "Lieu", "Lieu précis", "Espèce", "Attaque", "Objet"];

function argType(id: number) {
  const t = ARG_TYPES[id] ?? 0;
  return gen.value > 6 && t === 2 ? 1 : t;
}

const named = (list: string[]) => list.map((label, value) => ({ value, label })).filter((o) => o.label);
function argOptions(type: number): ComboOption[] {
  switch (type) {
    case 1:
      return named(lists.generalLocations);
    case 2:
      return lists.locations;
    case 3:
      return lists.species;
    case 4:
      return lists.moves;
    case 5:
      return lists.items;
    default:
      return [];
  }
}

/** Libellé court pour la liste : la phrase sans intensité ni ressenti. */
const shortLabel = (text: string) => text.split(" {4}")[0].replace("{0}", "Il").replace("{1}", "…").replace("{2}", "…").replace(/\.$/, "");
const memoryOptions = computed<ComboOption[]>(() =>
  lists.memories.map((text, value) => ({ value, label: value === 0 ? "(Aucun souvenir)" : shortLabel(text) })),
);

/** Phrase du souvenir, comme dans le résumé du jeu. */
function preview(text: string, m: Memory, trainer: string) {
  const t = argType(m.id);
  const variable = t ? (argOptions(t).find((o) => o.value === m.variable)?.label ?? `n°${m.variable}`) : "";
  return text
    .replace("{0}", props.p.nickname)
    .replace("{1}", trainer)
    .replace("{2}", variable)
    .replace("{3}", lists.feelings[m.feeling] ?? "")
    .replace("{4}", `Il ${lists.intensities[m.intensity] ?? ""}`);
}

interface Side {
  key: "otMemory" | "htMemory";
  title: string;
  trainer: string;
  memory: Memory;
  enabled: boolean;
}
const sides = computed<Side[]>(() => [
  { key: "otMemory", title: "Avec le dresseur d'origine", trainer: props.p.otName, memory: h.value.otMemory, enabled: true },
  { key: "htMemory", title: "Avec le soigneur", trainer: h.value.name, memory: h.value.htMemory, enabled: !!h.value.name },
]);

function setMemory(side: Side, change: Partial<Memory>) {
  const m = { ...side.memory, ...change };
  if (change.id !== undefined && argType(change.id) !== argType(side.memory.id)) m.variable = 0;
  if (change.id === 0) Object.assign(m, { intensity: 0, feeling: 0, variable: 0 });
  apply({ extras: { [side.key]: m } });
}

const memoryModel = (side: Side) => computed({ get: () => side.memory.id, set: (v: number) => v !== side.memory.id && setMemory(side, { id: v }) });
const variableModel = (side: Side) => computed({ get: () => side.memory.variable, set: (v: number) => v !== side.memory.variable && setMemory(side, { variable: v }) });

// --- Affection et Poké Récré.
type ByteKey = "otAffection" | "htAffection" | "fullness" | "enjoyment";
const BYTES: { key: ByteKey; label: string }[] = [
  { key: "otAffection", label: "Affection (dresseur d'origine)" },
  { key: "htAffection", label: "Affection (soigneur)" },
  { key: "fullness", label: "Satiété" },
  { key: "enjoyment", label: "Entrain" },
];
const bytes = ref<Record<ByteKey, number>>({ otAffection: 0, htAffection: 0, fullness: 0, enjoyment: 0 });
watch(
  h,
  (v) => (bytes.value = { otAffection: v.otAffection, htAffection: v.htAffection, fullness: v.fullness, enjoyment: v.enjoyment }),
  { immediate: true },
);
function commitByte(key: ByteKey) {
  const v = num(bytes.value[key], 0, 255);
  if (v !== null && v !== h.value[key]) apply({ extras: { [key]: v } });
}
</script>

<template>
  <div class="memories">
    <section v-for="side in sides" :key="side.key" class="card" :class="{ off: !side.enabled }">
      <h3 class="sv-section-title">{{ side.title }} <Tip term="memories" /></h3>
      <p v-if="!side.enabled" class="sv-help">Ce Pokémon n'a jamais été échangé : il n'a pas de soigneur (voir l'onglet Dresseur).</p>
      <template v-else>
        <div class="sv-field">
          <span class="sv-label">Souvenir</span>
          <Combo v-model="memoryModel(side).value" :options="memoryOptions" placeholder="Chercher un souvenir…" />
        </div>
        <template v-if="side.memory.id">
          <div class="sv-grid">
            <div class="sv-field">
              <span class="sv-label">Intensité</span>
              <select class="sv-select" :value="side.memory.intensity" @change="setMemory(side, { intensity: Number(($event.target as HTMLSelectElement).value) })">
                <option v-for="i in 8" :key="i" :value="i - 1">{{ i - 1 }} · {{ lists.intensities[i - 1] || "—" }}</option>
              </select>
            </div>
            <div class="sv-field">
              <span class="sv-label">Ressenti</span>
              <select class="sv-select" :value="side.memory.feeling" @change="setMemory(side, { feeling: Number(($event.target as HTMLSelectElement).value) })">
                <option v-for="(f, i) in lists.feelings" :key="i" :value="i">{{ f }}</option>
              </select>
            </div>
            <div v-if="argType(side.memory.id)" class="sv-field">
              <span class="sv-label">{{ ARG_LABELS[argType(side.memory.id)] }}</span>
              <Combo v-model="variableModel(side).value" :options="argOptions(argType(side.memory.id))" placeholder="Chercher…" />
            </div>
          </div>
          <blockquote>{{ preview(lists.memories[side.memory.id] ?? "", side.memory, side.trainer) }}</blockquote>
        </template>
      </template>
    </section>

    <section class="card">
      <h3 class="sv-section-title">Affection et Poké Récré <Tip term="affection" /></h3>
      <div class="sv-grid">
        <label v-for="b in BYTES" :key="b.key" class="sv-field">
          <span class="sv-label">{{ b.label }}</span>
          <input
            v-model="bytes[b.key]"
            class="sv-input"
            type="number"
            min="0"
            max="255"
            :disabled="b.key === 'htAffection' && !h.name"
            @change="commitByte(b.key)"
          />
        </label>
      </div>
      <div class="sv-row">
        <button class="sv-btn" @click="apply({ extras: h.name ? { otAffection: 255, htAffection: 255 } : { otAffection: 255 } })">Affection au maximum</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.memories {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  gap: 24px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.card.off .sv-section-title {
  opacity: 0.6;
}

blockquote {
  margin: 0;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  font-size: 14px;
  line-height: 1.5;
}
</style>
