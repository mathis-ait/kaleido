<script setup lang="ts">
import { computed, ref } from "vue";
import SearchField from "../../components/SearchField.vue";
import Segmented from "../../components/Segmented.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { battle, selectTrainer, type TrainerSummary } from "../../battle";

const search = ref("");
const all = ref(false);

/** Recherche sans accents ni majuscules, sur le nom, la classe et les Pokémon de l'équipe. */
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const shown = computed<TrainerSummary[]>(() => {
  const list = battle.link?.trainers ?? [];
  const q = fold(search.value.trim());
  const base = all.value || q ? list : list.filter((t) => t.role);
  if (!q) return base;
  return base.filter((t) => fold(`${t.name} ${t.className} ${t.id} ${t.team.map((p) => p.name).join(" ")}`).includes(q));
});
const importantCount = computed(() => battle.link?.trainers.filter((t) => t.role).length ?? 0);

const scopes = computed(() => [
  { value: false, label: `Importants (${importantCount.value})` },
  { value: true, label: "Tous les dresseurs" },
]);
</script>

<template>
  <aside class="trainers sv-panel">
    <div class="head">
      <Segmented v-model="all" :options="scopes" label="Dresseurs affichés" />
      <Tip term="battle.importantTrainers" />
    </div>
    <SearchField v-model="search" placeholder="Nom, classe, Pokémon ou numéro…" />
    <div class="list">
      <button
        v-for="t in shown"
        :key="t.id"
        type="button"
        class="card"
        :class="{ on: battle.trainerId === t.id }"
        :aria-pressed="battle.trainerId === t.id"
        @click="selectTrainer(t.id)"
      >
        <span class="line">
          <span v-if="t.roleLabel" class="sv-chip accent">{{ t.roleLabel }}</span>
          <span class="cls">{{ t.className }}</span>
          <span v-if="t.double" class="sv-chip dim">Duo</span>
          <span class="lvl">N. {{ t.maxLevel }}</span>
        </span>
        <strong>{{ t.name }}</strong>
        <span class="team">
          <span v-for="(p, i) in t.team" :key="i" :title="`${p.name} · N. ${p.level}`"><Sprite :id="p.species" :size="34" /></span>
        </span>
      </button>
      <p v-if="!shown.length" class="sv-help empty">Aucun dresseur ne correspond.</p>
    </div>
    <p v-if="shown.some((t) => t.double)" class="sv-help legend">« Duo » : combat double <Tip term="battle.double" /></p>
  </aside>
</template>

<style scoped>
.trainers {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 0;
  padding: var(--sp-4);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--sp-1);
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  min-height: 0;
  overflow-y: auto;
  padding: 2px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-2) var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 3%, transparent);
  color: var(--text);
  text-align: left;
}

.card:hover {
  background: var(--panel-hover);
}

/* Sélection : contour plein et fond relevé, comme la case choisie des boîtes. */
.card.on {
  border-color: var(--text);
  background: color-mix(in srgb, var(--text) 16%, transparent);
  box-shadow: inset 0 0 0 1px var(--text);
}

.line {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-xs);
}

.line .sv-chip {
  font-size: var(--fs-xs);
}

.cls {
  min-width: 0;
  overflow: hidden;
  color: var(--text-dim);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lvl {
  margin-left: auto;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.team {
  display: flex;
  flex-wrap: wrap;
}

.empty {
  padding: var(--sp-3) var(--sp-1);
}

.legend {
  display: flex;
  align-items: center;
}
</style>
