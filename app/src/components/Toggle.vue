<script setup lang="ts">
import Tip from "./Tip.vue";

/**
 * Interrupteur. `term` (entrée du glossaire) ou `hint` (texte libre) ajoute une bulle « i ».
 */
defineProps<{ label: string; hint?: string; term?: string; disabled?: boolean }>();
const model = defineModel<boolean>({ required: true });
</script>

<template>
  <span class="toggle-wrap">
    <label class="sv-switch" :class="{ off: disabled }">
      <input v-model="model" type="checkbox" :disabled="disabled" />
      <span class="track" />
      <span>{{ label }}</span>
    </label>
    <Tip v-if="term || hint" :term="term" :title="hint ? label : undefined" :text="hint" />
  </span>
</template>

<style scoped>
.toggle-wrap {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.off {
  opacity: 0.45;
  cursor: default;
}
</style>
