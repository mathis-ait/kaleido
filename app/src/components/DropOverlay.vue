<script setup lang="ts">
import KaleidoLogo from "./KaleidoLogo.vue";

defineProps<{ visible: boolean }>();
</script>

<template>
  <Transition name="fade">
    <div v-if="visible" class="overlay">
      <div class="ring">
        <div class="ring-inner">
          <KaleidoLogo :size="72" class="spin" />
          <h2>Lâche ici</h2>
          <p>ROM DS, ROM 3DS, dossier extrait ou sauvegarde</p>
        </div>
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

.ring {
  position: relative;
  padding: 3px;
  border-radius: 28px;
  overflow: hidden;
}

/* Bordure prismatique qui tourne */
.ring::before {
  content: "";
  position: absolute;
  inset: -50%;
  background: conic-gradient(var(--accent), var(--accent-2), var(--accent-3), var(--accent));
  animation: rotate 3s linear infinite;
}

.ring-inner {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 48px 72px;
  border-radius: 25px;
  background: var(--bg);
  text-align: center;
}

h2 {
  font-size: 26px;
}

p {
  margin: 0;
  color: var(--text-dim);
}

.spin {
  animation: rotate 6s linear infinite;
}

@keyframes rotate {
  to {
    transform: rotate(360deg);
  }
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
