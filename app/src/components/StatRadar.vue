<script setup lang="ts">
import { computed } from "vue";
import type { BaseStats } from "../types";

/** Hexagone des statistiques de base (échelle 0-200). */
const props = defineProps<{ stats: BaseStats; size?: number }>();

const AXES: { key: keyof BaseStats; label: string }[] = [
  { key: "hp", label: "PV" },
  { key: "attack", label: "Att" },
  { key: "defense", label: "Déf" },
  { key: "speed", label: "Vit" },
  { key: "spDefense", label: "Déf Spé" },
  { key: "spAttack", label: "Atq Spé" },
];
const MAX = 200;
const R = 70;

const point = (i: number, r: number) => {
  const a = -Math.PI / 2 + (i * Math.PI) / 3;
  return [Math.cos(a) * r, Math.sin(a) * r];
};

const polygon = (r: (i: number) => number) => AXES.map((_, i) => point(i, r(i)).join(",")).join(" ");
const shape = computed(() => polygon((i) => (Math.min(props.stats[AXES[i].key], MAX) / MAX) * R));
const rings = [0.25, 0.5, 0.75, 1].map((f) => polygon(() => f * R));
</script>

<template>
  <svg :width="size ?? 220" :height="size ?? 220" viewBox="-110 -105 220 210" role="img" aria-label="Statistiques de base">
    <polygon v-for="(r, i) in rings" :key="i" :points="r" class="ring" />
    <line v-for="(_, i) in AXES" :key="`a${i}`" x1="0" y1="0" :x2="point(i, R)[0]" :y2="point(i, R)[1]" class="ring" />
    <polygon :points="shape" class="shape" />
    <text
      v-for="(axis, i) in AXES"
      :key="axis.key"
      :x="point(i, R + 22)[0]"
      :y="point(i, R + 22)[1]"
      text-anchor="middle"
      dominant-baseline="middle"
    >
      {{ axis.label }} {{ stats[axis.key] }}
    </text>
  </svg>
</template>

<style scoped>
.ring {
  fill: none;
  stroke: color-mix(in srgb, var(--text) 18%, transparent);
  stroke-width: 1;
}

.shape {
  fill: color-mix(in srgb, var(--accent-2) 35%, transparent);
  stroke: var(--accent-2);
  stroke-width: 2;
  stroke-linejoin: round;
}

text {
  fill: var(--text);
  font-size: 10px;
  font-weight: 600;
}
</style>
