<script setup lang="ts">
import Icon from "./Icon.vue";
import Tip from "./Tip.vue";

/**
 * État vide, de chargement ou d'erreur d'une page ou d'un panneau : icône, titre, explication,
 * puis les actions pour en sortir (slot `actions`). Toujours centré, partout pareil.
 */
defineProps<{
  icon?: string;
  title?: string;
  /** Terme du glossaire expliqué à côté du titre. */
  term?: string;
  /** Variante plus petite pour l'intérieur d'un panneau ou d'une liste. */
  compact?: boolean;
  /** Affiche une icône qui tourne (chargement). */
  loading?: boolean;
}>();
</script>

<template>
  <div class="sv-empty" :class="{ compact }" :role="loading ? 'status' : undefined">
    <Icon v-if="loading" name="refresh" :size="compact ? 20 : 30" class="sv-spin" />
    <Icon v-else-if="icon" :name="icon" :size="compact ? 22 : 36" />
    <h3 v-if="title">{{ title }}<Tip v-if="term" :term="term" /></h3>
    <p v-if="$slots.default"><slot /></p>
    <slot name="details" />
    <div v-if="$slots.actions" class="sv-row"><slot name="actions" /></div>
  </div>
</template>
