<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Icône d'un Pokémon (pokesprite), téléchargée puis mise en cache par le moteur.
 * En cas d'échec (hors ligne, forme inconnue), une silhouette prend le relais.
 */
const props = withDefaults(defineProps<{ id: number; shiny?: boolean; size?: number }>(), { shiny: false, size: 68 });

const failed = ref(false);
const src = computed(() => convertFileSrc(`${props.id}${props.shiny ? "-shiny" : ""}.png`, "sprite"));
watch(src, () => (failed.value = false));
</script>

<template>
  <span class="sprite" :style="{ width: `${size}px`, height: `${Math.round((size * 56) / 68)}px` }">
    <img v-if="!failed" :src="src" alt="" loading="lazy" draggable="false" @error="failed = true" />
    <svg v-else viewBox="0 0 24 24" class="fallback" aria-hidden="true">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="1.5" />
      <path d="M3 12h6a3 3 0 0 0 6 0h6" fill="none" stroke="currentColor" stroke-width="1.5" />
    </svg>
  </span>
</template>

<style scoped>
.sprite {
  display: inline-grid;
  place-items: center;
  flex-shrink: 0;
}

img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  image-rendering: pixelated;
}

.fallback {
  width: 45%;
  height: 45%;
  color: var(--text-dim);
  opacity: 0.4;
}
</style>
