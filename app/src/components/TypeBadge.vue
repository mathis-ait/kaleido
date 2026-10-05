<script setup lang="ts">
import { computed } from "vue";
import type { PokeTypeKey, TypeTag } from "../types";

const props = defineProps<{ type: TypeTag }>();

// Couleurs officielles approximatives des types.
const COLORS: Record<PokeTypeKey, string> = {
  normal: "#9fa19f",
  fighting: "#ff8000",
  flying: "#81b9ef",
  poison: "#9141cb",
  ground: "#915121",
  rock: "#afa981",
  bug: "#91a119",
  ghost: "#704170",
  steel: "#60a1b8",
  mystery: "#68a090",
  fire: "#e62829",
  water: "#2980ef",
  grass: "#3fa129",
  electric: "#fac000",
  psychic: "#ef4179",
  ice: "#3dcef3",
  dragon: "#5060e1",
  dark: "#624d4e",
  fairy: "#ef70ef",
};

/** Texte foncé sur les couleurs claires (Électrik, Glace…), blanc sinon. */
const textColor = computed(() => {
  const hex = COLORS[props.type.key] ?? "#888888";
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return luminance > 0.5 ? "#16181f" : "#ffffff";
});
</script>

<template>
  <span class="type" :style="{ background: COLORS[type.key], color: textColor }">{{ type.name }}</span>
</template>

<style scoped>
.type {
  display: inline-block;
  min-width: 58px;
  padding: 2px 8px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 700;
  text-align: center;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12);
}
</style>
