<script setup lang="ts">
import { computed } from "vue";
import type { MoveCat } from "../types";

/** Catégorie d'attaque : Physique (étoile d'impact), Spéciale (ondes), Statut (cercle). */
const props = defineProps<{ cat: MoveCat }>();

const META: Record<MoveCat, { label: string; color: string }> = {
  physical: { label: "Physique", color: "#ef6a3a" },
  special: { label: "Spéciale", color: "#4f8cff" },
  status: { label: "Statut", color: "#9aa0ad" },
};
const meta = computed(() => META[props.cat]);
</script>

<template>
  <span class="cat" :style="{ background: meta.color }" :title="meta.label" role="img" :aria-label="meta.label">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path v-if="cat === 'physical'" d="M12 2l2.2 6.1L20.5 6l-3.4 5.6L22 15l-6.3.6L16 22l-4-5-4 5 .3-6.4L2 15l4.9-3.4L3.5 6l6.3 2.1z" />
      <g v-else-if="cat === 'special'" fill="none" stroke="currentColor" stroke-width="2.4">
        <circle cx="12" cy="12" r="3.2" />
        <circle cx="12" cy="12" r="7.6" />
      </g>
      <g v-else fill="none" stroke="currentColor" stroke-width="2.6">
        <circle cx="12" cy="12" r="7" />
        <path d="M12 5a3.5 3.5 0 0 1 0 7 3.5 3.5 0 0 0 0 7" />
      </g>
    </svg>
  </span>
</template>

<style scoped>
.cat {
  display: inline-grid;
  flex: none;
  place-items: center;
  width: 26px;
  height: 18px;
  border-radius: 6px;
  color: #fff;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15);
}

svg {
  width: 13px;
  height: 13px;
  fill: currentColor;
}
</style>
