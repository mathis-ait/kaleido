<script setup lang="ts" generic="T extends string">
/** Choix exclusif sous forme de pastilles (« segmented control »). */
defineProps<{ options: { value: T; label: string; hint?: string }[] }>();
const model = defineModel<T>({ required: true });
</script>

<template>
  <div class="segmented" role="radiogroup">
    <button
      v-for="o in options"
      :key="o.value"
      role="radio"
      :aria-checked="model === o.value"
      :class="{ active: model === o.value }"
      :title="o.hint"
      @click="model = o.value"
    >
      {{ o.label }}
    </button>
  </div>
</template>

<style scoped>
.segmented {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 4px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--text) 5%, transparent);
}

button {
  padding: 7px 14px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-weight: 600;
  font-size: 13px;
  transition: background 0.15s, color 0.15s;
}

button:hover {
  color: var(--text);
}

button.active {
  background: var(--prism);
  color: var(--on-accent);
}

:root[data-theme="lagon"] button.active {
  background: #fff;
}
</style>
