<script setup lang="ts">
import type { PokeTypeKey, TypeTag } from "../types";

defineProps<{ type: TypeTag }>();

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
};
</script>

<template>
  <span class="type" :style="{ background: COLORS[type.key] }">{{ type.name }}</span>
</template>

<style scoped>
.type {
  display: inline-block;
  min-width: 64px;
  padding: 2px 8px;
  border-radius: 6px;
  color: #fff;
  font-size: 11px;
  font-weight: 700;
  text-align: center;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.35);
}
</style>
