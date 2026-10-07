<script setup lang="ts">
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import { openInMain, type NextBattle } from "./store";

/** Prochain champion et, pour chacun de ses Pokémon, ton meilleur contre. */
defineProps<{ battle: NextBattle }>();

const VERDICT = {
  win: { label: "gagné", cls: "ok" },
  uncertain: { label: "incertain", cls: "warn" },
  lose: { label: "perdu", cls: "danger" },
  none: { label: "sans dégâts", cls: "dim" },
} as const;
</script>

<template>
  <section class="next sv-card">
    <header>
      <span>
        <span class="sv-label">Prochain combat <Tip term="companion.nextBattle" /></span>
        <span><strong>{{ battle.name }}</strong> · {{ battle.label }}<template v-if="battle.town"> · {{ battle.town }}</template></span>
      </span>
      <button type="button" class="sv-btn small" @click="openInMain(battle.trainerId)"><Icon name="swords" :size="14" /> Préparer</button>
    </header>
    <ul>
      <li v-for="(o, i) in battle.team" :key="i">
        <Sprite :id="o.species" :form="o.form" :size="36" />
        <span class="them">
          <strong>{{ o.name }}</strong>
          <small>N. {{ o.level }}</small>
        </span>
        <span v-if="o.counter" class="counter">
          <Sprite :id="o.counter.species" :size="28" />
          <span>
            {{ o.counter.name }}<template v-if="o.counter.moveName"> · {{ o.counter.moveName }}</template>
          </span>
          <span class="sv-chip" :class="VERDICT[o.counter.verdict].cls">{{ VERDICT[o.counter.verdict].label }}</span>
        </span>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.next {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3);
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
  font-size: var(--fs-md);
}

header > span {
  display: flex;
  flex-direction: column;
}

ul {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

li {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-md);
}

.them {
  display: flex;
  flex-direction: column;
  min-width: 84px;
}

small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.counter {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: flex-end;
  gap: var(--sp-1);
  min-width: 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.counter > span:not(.sv-chip) {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
