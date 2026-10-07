<script setup lang="ts">
import { giftTone } from "./terms";

/** Pastille du format d'une carte cadeau (PCD, PGF, WC7…), teintée selon le format. */
defineProps<{ label: string }>();
</script>

<template>
  <span class="sv-chip gift-fmt" :style="giftTone(label)">{{ label }}</span>
</template>

<style scoped>
.gift-fmt {
  border-color: var(--gift-tone);
  border-radius: var(--radius-xs);
  background: color-mix(in srgb, var(--gift-tone) 28%, transparent);
  font-size: var(--fs-xs);
  font-weight: 800;
  letter-spacing: 0.05em;
}
</style>
