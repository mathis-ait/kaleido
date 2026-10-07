<script setup lang="ts">
import Icon from "./Icon.vue";

/**
 * Bandeau d'erreur ou d'avertissement en tête de page ou de panneau.
 * `retry` ajoute « Réessayer », `dismiss` la croix de fermeture.
 */
const props = withDefaults(defineProps<{ tone?: "danger" | "warn" | "info"; retry?: () => void; dismiss?: () => void }>(), {
  tone: "danger",
});
const ICONS = { danger: "alert", warn: "shield-alert", info: "info" } as const;
</script>

<template>
  <div class="sv-banner" :class="props.tone" :role="props.tone === 'danger' ? 'alert' : 'status'">
    <Icon :name="ICONS[props.tone]" :size="16" />
    <span class="grow"><slot /></span>
    <button v-if="retry" type="button" @click="retry">Réessayer</button>
    <button v-if="dismiss" type="button" aria-label="Fermer" @click="dismiss"><Icon name="x" :size="14" /></button>
  </div>
</template>
