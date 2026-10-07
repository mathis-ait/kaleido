<script setup lang="ts">
/**
 * Fond du lanceur : la jaquette du jeu sélectionné en très grand, floutée, sur un fond
 * légèrement teinté par la couleur du jeu. Fond fixe : seul un fondu enchaîné
 * accompagne le changement de jeu.
 */
defineProps<{ image: string | null; color: string }>();
</script>

<template>
  <div class="backdrop" :style="{ '--tint': color }">
    <TransitionGroup name="fade">
      <img v-if="image" :key="image" class="art" :src="image" alt="" aria-hidden="true" />
    </TransitionGroup>
    <TransitionGroup name="fade">
      <img v-if="image" :key="image" class="feature" :src="image" alt="" aria-hidden="true" />
    </TransitionGroup>
    <div class="vignette" />
  </div>
</template>

<style scoped>
.backdrop {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background-color: color-mix(in srgb, var(--tint) 10%, #05060c);
  transition: background-color 0.8s ease;
}

.art {
  position: absolute;
  inset: -8%;
  width: 116%;
  height: 116%;
  object-fit: cover;
  filter: blur(48px) saturate(1.3) brightness(0.5);
  transform: scale(1.05);
}

/* Affiche nette du jeu, fondue dans le fond, en haut à droite. */
.feature {
  position: absolute;
  top: -4%;
  right: -2%;
  width: 66%;
  height: 78%;
  object-fit: cover;
  object-position: center 40%;
  opacity: 0.8;
  filter: saturate(1.1) brightness(0.9);
  -webkit-mask-image: radial-gradient(ellipse 50% 60% at 54% 42%, #000 32%, transparent 82%);
  mask-image: radial-gradient(ellipse 50% 60% at 54% 42%, #000 32%, transparent 82%);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.6s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.vignette {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at 50% 40%, transparent 30%, rgba(0, 0, 0, 0.55) 100%),
    linear-gradient(to bottom, rgba(0, 0, 0, 0.25), transparent 30%, transparent 55%, rgba(0, 0, 0, 0.6));
}

@media (prefers-reduced-motion: reduce) {
  .backdrop,
  .fade-enter-active,
  .fade-leave-active {
    transition: none;
  }
}
</style>
