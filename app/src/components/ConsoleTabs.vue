<script setup lang="ts">
import { computed } from "vue";
import ConsoleLogo from "./ConsoleLogo.vue";
import { CONSOLES, SHELVES, consoleOf, type ConsoleId, type ShelfFilter } from "../consoles";
import type { Detection } from "../types";

/**
 * Onglets de consoles : « Tous » puis le logo de chaque console présente dans la liste.
 * Sélection inversée (pastille claire, logo sombre). Les couleurs se règlent par variables
 * (--tabs-fg, --tabs-fg-hover, --tabs-on-bg, --tabs-on-fg, --tabs-bg) : le lanceur, toujours
 * sombre, les fixe ; la grille garde celles du thème.
 */
const props = withDefaults(defineProps<{ games: Detection[]; size?: number; counts?: boolean }>(), { size: 14, counts: false });
const model = defineModel<ShelfFilter>({ required: true });

const shelves = computed(() =>
  SHELVES.map((platform) => {
    const list = props.games.filter((g) => g.platform === platform);
    // Onglet Game Boy : logo Color si tous ses jeux sont des jeux Game Boy Color.
    const logo: ConsoleId = platform === "gb" && list.length && list.every((g) => consoleOf(g) === "gbc") ? "gbc" : platform;
    return { platform, logo, count: list.length };
  }).filter((s) => s.count > 0 || s.platform === model.value),
);
</script>

<template>
  <div class="console-tabs" role="tablist" aria-label="Consoles">
    <slot name="before" />
    <button type="button" role="tab" :aria-selected="model === 'all'" :class="{ on: model === 'all' }" @click="model = 'all'">
      <span class="all">Tous</span>
      <span v-if="counts" class="count">{{ games.length }}</span>
    </button>
    <button
      v-for="s in shelves"
      :key="s.platform"
      type="button"
      role="tab"
      :aria-selected="model === s.platform"
      :class="{ on: model === s.platform }"
      :title="`${CONSOLES[s.logo].label} · ${s.count} jeu${s.count > 1 ? 'x' : ''}`"
      @click="model = s.platform"
    >
      <ConsoleLogo :id="s.logo" :size="size" />
      <span v-if="counts" class="count">{{ s.count }}</span>
    </button>
    <slot name="after" />
  </div>
</template>

<style scoped>
.console-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: var(--radius-pill);
  background: var(--tabs-bg, var(--panel));
  box-shadow: inset 0 0 0 1px var(--tabs-border, var(--border));
}

button {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-2);
  height: 36px;
  padding: 0 var(--tabs-pad, var(--sp-4));
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--tabs-fg, var(--text-dim));
  font-size: var(--fs-base);
  font-weight: 600;
  white-space: nowrap;
  transition:
    background-color 0.2s ease,
    color 0.2s ease;
}

button:hover:not(.on) {
  color: var(--tabs-fg-hover, var(--text));
}

button.on {
  background: var(--tabs-on-bg, var(--text));
  color: var(--tabs-on-fg, var(--bg));
}

.all {
  line-height: 1;
}

.count {
  font-size: var(--fs-sm);
  font-variant-numeric: tabular-nums;
  opacity: 0.6;
}
</style>
