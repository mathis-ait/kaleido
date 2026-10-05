<script setup lang="ts">
defineProps<{ size: number }>();

// Six triangles en rosace, comme l'intérieur d'un kaléidoscope.
const petals = [0, 60, 120, 180, 240, 300];
const colors = ["var(--accent)", "var(--accent-2)", "var(--accent-3)"];
</script>

<template>
  <svg :width="size" :height="size" viewBox="-50 -50 100 100" aria-hidden="true">
    <g v-for="(angle, i) in petals" :key="angle" :transform="`rotate(${angle})`">
      <path d="M0 0 L-24 -42 L24 -42 Z" :fill="colors[i % 3]" :opacity="i % 2 ? 0.75 : 1" />
    </g>
    <circle r="9" fill="var(--bg)" />
  </svg>
</template>
