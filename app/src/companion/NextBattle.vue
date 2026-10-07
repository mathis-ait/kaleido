<script setup lang="ts">
import { computed } from "vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import { openInMain, type NextBattle } from "./store";

/**
 * Prochain champion : son équipe en petit et une phrase de conseil, au lieu d'un verdict par
 * Pokémon. Le détail des duels est dans la page Combat (« Préparer »).
 */
const props = defineProps<{ battle: NextBattle }>();

const duels = computed(() => props.battle.team.filter((o) => o.counter));
const wins = computed(() => duels.value.filter((o) => o.counter!.verdict === "win"));

/** Pokémon de l'équipe qui gagne le plus de duels. */
const ace = computed(() => {
  const count = new Map<string, { name: string; species: number; move: string | null; n: number }>();
  for (const o of wins.value) {
    const c = o.counter!;
    const e = count.get(c.name) ?? { name: c.name, species: c.species, move: c.moveName, n: 0 };
    e.n++;
    count.set(c.name, e);
  }
  return [...count.values()].sort((a, b) => b.n - a.n)[0] ?? null;
});

const advice = computed(() => {
  const n = props.battle.team.length;
  if (!duels.value.length) return { tone: "dim", text: "Ajoute des Pokémon à ton équipe pour voir tes chances." };
  if (!wins.value.length) return { tone: "warn", text: "Aucun de tes Pokémon ne gagne ses duels pour l'instant : entraîne-toi ou capture d'autres Pokémon." };
  if (wins.value.length === n) return { tone: "ok", text: "Tu gagnes chaque duel avec ton équipe actuelle." };
  return { tone: "", text: `Tu gagnes ${wins.value.length} duel${wins.value.length > 1 ? "s" : ""} sur ${n}.` };
});
</script>

<template>
  <section class="next sv-card">
    <header>
      <span class="sv-label">Prochain combat <Tip term="companion.nextBattle" /></span>
      <button type="button" class="sv-btn small" @click="openInMain(battle.trainerId)"><Icon name="swords" :size="14" /> Préparer</button>
    </header>
    <p class="who">
      <strong>{{ battle.name }}</strong>
      <span class="dim">{{ battle.label }}<template v-if="battle.town"> · {{ battle.town }}</template></span>
    </p>
    <ul class="team">
      <li v-for="(o, i) in battle.team" :key="i" :title="`${o.name} · N. ${o.level}`">
        <Sprite :id="o.species" :form="o.form" :size="40" />
        <small>N. {{ o.level }}</small>
      </li>
    </ul>
    <p class="advice" :class="advice.tone">
      {{ advice.text }}
      <template v-if="ace">
        <br />
        <span class="dim">Ton meilleur atout :</span> <strong>{{ ace.name }}</strong><span v-if="ace.move" class="dim"> avec {{ ace.move }}</span>
      </template>
    </p>
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
}

.who {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-1) var(--sp-2);
  margin: 0;
  font-size: var(--fs-base);
}

.dim {
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.team {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.team li {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.team small {
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.advice {
  margin: 0;
  font-size: var(--fs-md);
  line-height: 1.5;
}

.advice.ok {
  color: var(--ok);
}

.advice.warn {
  color: var(--warn);
}
</style>
