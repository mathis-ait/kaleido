<script setup lang="ts">
import { computed } from "vue";
import Dialog from "../components/Dialog.vue";
import Segmented from "../components/Segmented.vue";
import Toggle from "../components/Toggle.vue";
import SourcesList from "./SourcesList.vue";
import { matchRulePreset, RULE_INFO, RULE_KEYS, RULE_PRESETS, type RulePresetId } from "./slots";
import { collection, livedex, setLegalOnly, setRules } from "./store";
import { livedexUi } from "./ui";
import type { DexRules } from "./types";

const preset = computed<RulePresetId | "custom">({
  get: () => matchRulePreset(livedex.saved.rules) ?? "custom",
  set: (id) => {
    if (id !== "custom") setRules(RULE_PRESETS[id].rules);
  },
});
const presetOptions = [
  ...Object.values(RULE_PRESETS).map((p) => ({ value: p.id as RulePresetId | "custom", label: p.label, hint: p.description })),
  { value: "custom" as const, label: "Personnalisé", hint: "Règles choisies une par une ci-dessous.", disabled: true },
];

function setRule(key: keyof DexRules, on: boolean) {
  setRules({ ...livedex.saved.rules, [key]: on });
}
</script>

<template>
  <Dialog v-model="livedexUi.settings" title="Réglages de la Living Dex" term="livedex.livingDex" icon="sliders" :width="760">
    <section>
      <h3 class="sv-section-title">Sauvegardes lues</h3>
      <SourcesList />
    </section>

    <section>
      <h3 class="sv-section-title">Cases de la Living Dex</h3>
      <div class="presets">
        <Segmented v-model="preset" :options="presetOptions" label="Règles" />
        <span class="dim">{{ collection?.totals.slots.toLocaleString("fr-FR") }} cases</span>
      </div>
      <div class="rules">
        <Toggle
          v-for="k in RULE_KEYS"
          :key="k"
          :model-value="livedex.saved.rules[k]"
          :label="RULE_INFO[k].label"
          :hint="RULE_INFO[k].description"
          @update:model-value="setRule(k, $event)"
        />
      </div>
    </section>

    <section>
      <h3 class="sv-section-title">Pokémon comptés</h3>
      <Toggle
        :model-value="livedex.saved.legalOnly"
        label="Ignorer les Pokémon illégaux"
        term="legality"
        @update:model-value="setLegalOnly"
      />
      <p class="dim small">Les Pokémon jugés illégaux par le moteur de légalité de Kaleido ne remplissent plus de case. Les douteux restent comptés.</p>
    </section>
  </Dialog>
</template>

<style scoped>
section + section {
  margin-top: var(--sp-5);
}

.presets {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  margin-bottom: var(--sp-3);
}

.rules {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--sp-2) var(--sp-4);
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: var(--sp-2) 0 0;
  font-size: var(--fs-sm);
}
</style>
