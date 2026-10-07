<script setup lang="ts">
import KaleidoLogo from "./KaleidoLogo.vue";

defineProps<{ visible: boolean }>();
</script>

<template>
  <Transition name="fade">
    <div v-if="visible" class="overlay">
      <div class="zone">
        <KaleidoLogo :size="72" />
        <h2>Lâche ici</h2>
        <p>ROM DS, ROM 3DS, dossier extrait ou sauvegarde</p>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--bg) 70%, transparent);
  backdrop-filter: blur(10px);
  pointer-events: none;
}

/* Zone de dépôt : cadre en pointillés fixe (ni dégradé ni rotation). */
.zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 48px 72px;
  border: 2px dashed color-mix(in srgb, var(--text) 45%, transparent);
  border-radius: var(--radius-panel);
  background: var(--surface);
  box-shadow: var(--shadow-dialog);
  text-align: center;
}

h2 {
  font-size: 26px;
}

p {
  margin: 0;
  color: var(--text-dim);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
