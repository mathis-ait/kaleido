<script setup lang="ts">
import { computed } from "vue";
import Tip from "../components/Tip.vue";
import { ACHIEVEMENTS, RANKS, rankOf, type Achievement, type AchievementCategory } from "./achievements";
import { dex, frDate } from "./data";
import { collection, entries, livedex } from "./store";

const ctx = computed(() => (dex.value && collection.value ? { dex: dex.value, collection: collection.value, entries: entries.value } : null));
const percent = computed(() => {
  const t = collection.value?.totals;
  return t && t.slots ? (t.caught / t.slots) * 100 : 0;
});
const rank = computed(() => rankOf(percent.value));
const unlockedCount = computed(() => ACHIEVEMENTS.filter((a) => livedex.saved.achievements[a.id]).length);

const groups = computed(() => {
  const map = new Map<AchievementCategory, Achievement[]>();
  for (const a of ACHIEVEMENTS) {
    const list = map.get(a.category);
    if (list) list.push(a);
    else map.set(a.category, [a]);
  }
  return [...map.entries()];
});

function progress(a: Achievement): [number, number] {
  if (!ctx.value) return [0, 1];
  const [cur, goal] = a.progress(ctx.value);
  return [Math.min(cur, goal), goal];
}
</script>

<template>
  <div class="ach">
    <div class="rank sv-panel">
      <div>
        <small class="dim">Rang de collectionneur <Tip term="livedex.rank" /></small>
        <strong>{{ rank.name }}</strong>
        <small v-if="rank.next" class="dim">Prochain rang, {{ rank.next.name }}, à {{ rank.next.min }} %</small>
      </div>
      <ol class="ladder" aria-label="Rangs">
        <li v-for="(r, i) in RANKS" :key="r.name" :class="{ on: i <= rank.index }" :title="`${r.name} · ${r.min} %`" />
      </ol>
      <div class="score">
        <strong>{{ unlockedCount }}/{{ ACHIEVEMENTS.length }}</strong>
        <small class="dim">succès</small>
      </div>
    </div>

    <section v-for="[cat, list] in groups" :key="cat">
      <h3 class="sv-section-title">{{ cat }}</h3>
      <ul class="list">
        <li v-for="a in list" :key="a.id" :class="{ done: !!livedex.saved.achievements[a.id] }">
          <div class="text">
            <strong>{{ a.title }}</strong>
            <small class="dim">{{ a.description }}</small>
          </div>
          <div class="state">
            <template v-if="livedex.saved.achievements[a.id]">
              <span class="sv-chip ok">Débloqué</span>
              <small class="dim">{{ frDate(livedex.saved.achievements[a.id]) }}</small>
            </template>
            <template v-else>
              <span class="bar"><span :style="{ width: `${(progress(a)[0] / progress(a)[1]) * 100}%` }" /></span>
              <small class="dim num">{{ progress(a)[0].toLocaleString("fr-FR") }} / {{ progress(a)[1].toLocaleString("fr-FR") }}</small>
            </template>
          </div>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.rank {
  display: flex;
  align-items: center;
  gap: var(--sp-5);
  padding: var(--sp-4) var(--sp-5);
}

.rank > div:first-child {
  display: grid;
  gap: 2px;
}

.rank strong {
  font-family: var(--font-display);
  font-size: var(--fs-xl);
}

.ladder {
  display: flex;
  flex: 1;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.ladder li {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.ladder li.on {
  background: var(--text);
}

.score {
  display: grid;
  justify-items: end;
}

section {
  margin-top: var(--sp-5);
}

.dim {
  color: var(--text-dim);
}

.list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.list li {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.list li:not(.done) .text strong {
  color: var(--text-dim);
}

.text {
  display: grid;
  flex: 1;
  min-width: 0;
}

.state {
  display: grid;
  justify-items: end;
  gap: 4px;
  min-width: 110px;
}

.bar {
  display: block;
  width: 110px;
  height: 4px;
  overflow: hidden;
  border-radius: 2px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.bar span {
  display: block;
  height: 100%;
  background: var(--text);
}

.num {
  font-variant-numeric: tabular-nums;
}
</style>
