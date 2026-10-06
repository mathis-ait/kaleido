<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";

/**
 * Fond du lanceur : la jaquette du jeu sélectionné en très grand, floutée, avec un
 * fondu enchaîné d'un jeu à l'autre, des halos qui dérivent lentement et des
 * particules lumineuses teintées par la couleur du jeu.
 */
const props = defineProps<{ image: string | null; color: string }>();

const canvas = ref<HTMLCanvasElement>();
const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

interface Particle {
  x: number;
  y: number;
  r: number;
  vy: number;
  vx: number;
  a: number;
  phase: number;
}

let particles: Particle[] = [];
let raf = 0;
let tint = props.color;

watch(
  () => props.color,
  (c) => (tint = c),
);

function resize() {
  const c = canvas.value;
  if (!c) return;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = c.clientWidth * dpr;
  c.height = c.clientHeight * dpr;
  const count = Math.round((c.clientWidth * c.clientHeight) / 14000);
  particles = Array.from({ length: Math.min(count, 140) }, () => spawn(c, true));
}

function spawn(c: HTMLCanvasElement, anywhere: boolean): Particle {
  const dpr = c.width / Math.max(1, c.clientWidth);
  return {
    x: Math.random() * c.width,
    y: anywhere ? Math.random() * c.height : c.height + 10,
    r: (0.6 + Math.random() * 2.2) * dpr,
    vy: -(0.15 + Math.random() * 0.5) * dpr,
    vx: (Math.random() - 0.5) * 0.2 * dpr,
    a: 0.15 + Math.random() * 0.55,
    phase: Math.random() * Math.PI * 2,
  };
}

function draw(t: number) {
  const c = canvas.value;
  if (!c) return;
  const g = c.getContext("2d")!;
  g.clearRect(0, 0, c.width, c.height);
  g.globalCompositeOperation = "lighter";
  g.fillStyle = tint;
  for (const p of particles) {
    p.y += p.vy;
    p.x += p.vx + Math.sin(t / 1800 + p.phase) * 0.15;
    if (p.y < -10) Object.assign(p, spawn(c, false));
    const twinkle = 0.6 + 0.4 * Math.sin(t / 700 + p.phase);
    g.globalAlpha = p.a * twinkle;
    g.beginPath();
    g.arc(p.x, p.y, p.r, 0, Math.PI * 2);
    g.fill();
  }
  g.globalAlpha = 1;
  raf = requestAnimationFrame(draw);
}

onMounted(() => {
  resize();
  window.addEventListener("resize", resize);
  if (!reduced) raf = requestAnimationFrame(draw);
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener("resize", resize);
});
</script>

<template>
  <div class="backdrop" :style="{ '--tint': color }">
    <TransitionGroup name="art">
      <img v-if="image" :key="image" class="art" :src="image" alt="" aria-hidden="true" />
    </TransitionGroup>
    <TransitionGroup name="feature">
      <img v-if="image" :key="image" class="feature" :src="image" alt="" aria-hidden="true" />
    </TransitionGroup>
    <div class="glow g1" />
    <div class="glow g2" />
    <canvas ref="canvas" class="particles" />
    <div class="vignette" />
  </div>
</template>

<style scoped>
.backdrop {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: #05060c;
}

.art {
  position: absolute;
  inset: -8%;
  width: 116%;
  height: 116%;
  object-fit: cover;
  filter: blur(48px) saturate(1.5) brightness(0.55);
  transform: scale(1.05);
  animation: drift 40s ease-in-out infinite alternate;
}

@keyframes drift {
  from {
    transform: scale(1.05) translate(0, 0);
  }
  to {
    transform: scale(1.18) translate(-2%, 1.5%);
  }
}

.art-enter-active,
.art-leave-active {
  transition: opacity 0.9s ease;
}

.art-enter-from,
.art-leave-to {
  opacity: 0;
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
  animation: kenburns 30s ease-in-out infinite alternate;
}

@keyframes kenburns {
  from {
    transform: scale(1.02) translateX(0);
  }
  to {
    transform: scale(1.1) translateX(-2%);
  }
}

.feature-enter-active,
.feature-leave-active {
  transition: opacity 0.7s ease, transform 0.7s ease;
}

.feature-enter-from {
  opacity: 0;
  transform: scale(1.06) translateX(3%);
}

.feature-leave-to {
  opacity: 0;
}

.glow {
  position: absolute;
  width: 60vmax;
  height: 60vmax;
  border-radius: 50%;
  background: radial-gradient(circle, var(--tint) 0%, transparent 60%);
  opacity: 0.22;
  mix-blend-mode: screen;
  transition: background 0.8s ease;
}

.g1 {
  top: -25vmax;
  left: -15vmax;
  animation: float1 26s ease-in-out infinite alternate;
}

.g2 {
  right: -20vmax;
  bottom: -30vmax;
  opacity: 0.16;
  animation: float2 32s ease-in-out infinite alternate;
}

@keyframes float1 {
  to {
    transform: translate(18vmax, 10vmax) scale(1.2);
  }
}

@keyframes float2 {
  to {
    transform: translate(-14vmax, -8vmax) scale(0.9);
  }
}

.particles {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.vignette {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at 50% 40%, transparent 30%, rgba(0, 0, 0, 0.55) 100%),
    linear-gradient(to bottom, rgba(0, 0, 0, 0.25), transparent 30%, transparent 55%, rgba(0, 0, 0, 0.6));
}

@media (prefers-reduced-motion: reduce) {
  .art,
  .feature,
  .glow {
    animation: none;
  }
}
</style>
