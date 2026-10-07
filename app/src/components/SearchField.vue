<script setup lang="ts">
import { ref } from "vue";
import Icon from "./Icon.vue";

/** Champ de recherche avec loupe ; Échap vide le champ puis rend la main à la page. */
defineProps<{ placeholder?: string }>();
const model = defineModel<string>({ required: true });
const input = ref<HTMLInputElement | null>(null);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && model.value) {
    e.stopPropagation();
    e.preventDefault();
    model.value = "";
  }
}

defineExpose({ focus: () => input.value?.focus() });
</script>

<template>
  <label class="sv-search">
    <Icon name="search" :size="15" />
    <input ref="input" v-model="model" class="sv-input" type="search" :placeholder="placeholder" :aria-label="placeholder" @keydown="onKey" />
  </label>
</template>
