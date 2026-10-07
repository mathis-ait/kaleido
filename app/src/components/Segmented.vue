<script setup lang="ts" generic="T extends string | number | boolean | null">
/** Choix exclusif sous forme de pastilles (« segmented control »), flèches gauche / droite au clavier. */
defineProps<{
  options: { value: T; label: string; hint?: string; disabled?: boolean }[];
  /** Nom du groupe pour les lecteurs d'écran. */
  label?: string;
}>();
const model = defineModel<T>({ required: true });

function arrow(e: KeyboardEvent) {
  const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
  if (!dir) return;
  const group = (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
  const list = [...group];
  const i = list.indexOf(document.activeElement as HTMLButtonElement);
  const next = list[(i + dir + list.length) % list.length];
  if (!next) return;
  e.preventDefault();
  next.focus();
  next.click();
}
</script>

<template>
  <div class="sv-seg" role="radiogroup" :aria-label="label" @keydown="arrow">
    <button
      v-for="o in options"
      :key="String(o.value)"
      type="button"
      role="radio"
      :aria-checked="model === o.value"
      :tabindex="model === o.value ? 0 : -1"
      :class="{ on: model === o.value }"
      :title="o.hint"
      :disabled="o.disabled"
      @click="model = o.value"
    >
      {{ o.label }}<slot name="extra" :option="o" />
    </button>
  </div>
</template>
