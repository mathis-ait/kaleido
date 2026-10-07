<script setup lang="ts">
import { computed } from "vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import type { JournalEntry } from "./store";

/** Fil chronologique de la partie, du plus récent au plus ancien, groupé par sauvegarde. */
const props = defineProps<{ entries: JournalEntry[] }>();

const ICON: Record<JournalEntry["kind"], string> = {
  caught: "ball",
  hatched: "egg",
  evolved: "sparkle",
  levelUp: "plus",
  fainted: "alert",
  revived: "heart",
  died: "x",
  gone: "minus",
  badge: "star",
};

const playTime = (s: number) => `${Math.floor(s / 3600)} h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}`;

/** Une sauvegarde = un groupe (même heure de lecture). */
const groups = computed(() => {
  const out: { at: number; play: number; place: string | null; items: JournalEntry[] }[] = [];
  for (const e of props.entries) {
    const g = out[out.length - 1];
    if (g && g.at === e.at) g.items.push(e);
    else out.push({ at: e.at, play: e.playSeconds, place: e.place, items: [e] });
  }
  return out;
});
</script>

<template>
  <EmptyState v-if="!entries.length" compact icon="book" title="Journal vide">
    Chaque sauvegarde en jeu ajoutera ici tes captures, montées de niveau, évolutions et badges.
  </EmptyState>
  <ol v-else class="journal">
    <li v-for="g in groups" :key="g.at" class="group">
      <header>
        <strong>{{ playTime(g.play) }}</strong>
        <span v-if="g.place"> · {{ g.place }}</span>
        <small>{{ new Date(g.at).toLocaleString("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) }}</small>
      </header>
      <ul>
        <li v-for="(e, i) in g.items" :key="i" :class="e.kind">
          <Sprite v-if="e.species" :id="e.species" :size="28" />
          <Icon v-else :name="ICON[e.kind]" :size="16" />
          <span>{{ e.text }}</span>
        </li>
      </ul>
    </li>
  </ol>
</template>

<style scoped>
.journal {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

header {
  display: flex;
  align-items: baseline;
  gap: var(--sp-1);
  margin-bottom: var(--sp-1);
  font-size: var(--fs-md);
}

header small {
  margin-left: auto;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

ul {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

ul li {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-height: 30px;
  padding: 2px var(--sp-2);
  border-radius: var(--radius-xs);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  font-size: var(--fs-md);
}

.died {
  color: var(--danger);
}

.fainted {
  color: var(--warn);
}

.badge,
.caught {
  font-weight: 600;
}
</style>
