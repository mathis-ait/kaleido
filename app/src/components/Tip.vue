<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { GLOSSARY } from "../glossary";

/**
 * Bulle d'information (« toggletip ») : un petit « i » qui explique un terme technique.
 * Survol ou clic pour l'ouvrir, Échap ou clic ailleurs pour la fermer.
 */
const props = defineProps<{ term?: string; title?: string; text?: string }>();

const entry = computed(() => {
  const g = props.term ? GLOSSARY[props.term] : undefined;
  return { title: props.title ?? g?.title ?? "", text: props.text ?? g?.text ?? "" };
});

const open = ref(false);
const pinned = ref(false);
const button = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, top: 0, above: false });
let hoverTimer: number | undefined;

async function place() {
  await nextTick();
  const r = button.value?.getBoundingClientRect();
  if (!r) return;
  const width = 300;
  const left = Math.min(Math.max(12, r.left + r.width / 2 - width / 2), window.innerWidth - width - 12);
  const above = r.bottom + 180 > window.innerHeight;
  pos.value = { left, top: above ? r.top - 8 : r.bottom + 8, above };
}

function show() {
  open.value = true;
  place();
  document.addEventListener("pointerdown", outside, true);
  document.addEventListener("keydown", onKey, true);
}

function hide() {
  open.value = false;
  pinned.value = false;
  document.removeEventListener("pointerdown", outside, true);
  document.removeEventListener("keydown", onKey, true);
}

function toggle() {
  if (open.value && pinned.value) hide();
  else {
    pinned.value = true;
    show();
  }
}

function outside(e: PointerEvent) {
  if (!button.value?.contains(e.target as Node)) hide();
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    hide();
  }
}

function enter() {
  clearTimeout(hoverTimer);
  hoverTimer = window.setTimeout(() => !open.value && show(), 250);
}

function leave() {
  clearTimeout(hoverTimer);
  if (!pinned.value) hide();
}

onBeforeUnmount(hide);
</script>

<template>
  <button
    ref="button"
    type="button"
    class="tip"
    :class="{ on: open }"
    :aria-label="`À propos : ${entry.title}`"
    :aria-expanded="open"
    @click.stop.prevent="toggle"
    @pointerenter="enter"
    @pointerleave="leave"
  >
    i
  </button>
  <Teleport to="body">
    <div
      v-if="open"
      class="tip-pop"
      role="status"
      :class="{ above: pos.above }"
      :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
    >
      <strong>{{ entry.title }}</strong>
      <p>{{ entry.text }}</p>
    </div>
  </Teleport>
</template>

<style scoped>
.tip {
  display: inline-grid;
  place-items: center;
  width: 16px;
  height: 16px;
  margin-left: 5px;
  padding: 0;
  border: 1px solid color-mix(in srgb, currentColor 55%, transparent);
  border-radius: 50%;
  background: transparent;
  color: inherit;
  font: italic 700 10px/1 Georgia, serif;
  vertical-align: middle;
  opacity: 0.75;
  transition: opacity 0.15s, background 0.15s;
  flex-shrink: 0;
}

.tip:hover,
.tip.on {
  opacity: 1;
  background: color-mix(in srgb, currentColor 18%, transparent);
}
</style>

<style>
.tip-pop {
  position: fixed;
  z-index: 200;
  width: 300px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 14px 36px rgba(0, 0, 0, 0.35);
  font-size: 13px;
  line-height: 1.45;
  text-transform: none;
  letter-spacing: normal;
  font-weight: 400;
  pointer-events: none;
  animation: tip-in 0.12s ease-out;
}

.tip-pop.above {
  transform: translateY(-100%);
}

.tip-pop strong {
  display: block;
  margin-bottom: 4px;
  font-size: 13px;
}

.tip-pop p {
  margin: 0;
  color: var(--text-dim);
}

@keyframes tip-in {
  from {
    opacity: 0;
  }
}
</style>
