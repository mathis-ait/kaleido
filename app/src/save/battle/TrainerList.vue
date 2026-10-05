<script setup lang="ts">
import { computed, ref } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { battle, ROLE_COLORS, selectTrainer, type TrainerSummary } from "../../battle";
import { BATTLE_TIPS } from "./glossary";

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
</script>

<template>
  <aside class="trainers sv-panel">
    <div class="head">
      <div class="sv-seg">
        <button :class="{ on: !all }" @click="all = false">Importants ({{ importantCount }})</button>
        <button :class="{ on: all }" @click="all = true">Tous les dresseurs</button>
      </div>
      <Tip v-bind="BATTLE_TIPS.importantTrainers" />
    </div>
    <label class="search">
      <Icon name="search" :size="15" />
      <input v-model="search" class="sv-input" placeholder="Nom, classe, Pokémon ou numéro…" />
    </label>
    <div class="list">
      <button
        v-for="t in shown"
        :key="t.id"
        class="card"
        :class="{ on: battle.trainerId === t.id }"
        :style="{ '--role': t.role ? ROLE_COLORS[t.role] : 'var(--text-dim)' }"
        @click="selectTrainer(t.id)"
      >
        <div class="line">
          <span v-if="t.roleLabel" class="chip">{{ t.roleLabel }}</span>
          <small class="cls">{{ t.className }}</small>
          <small v-if="t.double" class="dbl" title="Combat en duo">Duo</small>
          <small class="lvl">N. {{ t.maxLevel }}</small>
        </div>
        <strong>{{ t.name }}</strong>
        <div class="team">
          <span v-for="(p, i) in t.team" :key="i" :title="`${p.name} · N. ${p.level}`"><Sprite :id="p.species" :size="34" /></span>
        </div>
      </button>
      <p v-if="!shown.length" class="sv-help empty">Aucun dresseur ne correspond.</p>
    </div>
  </aside>
</template>

<style scoped>
.trainers {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 14px;
}

.head {
  display: flex;
  align-items: center;
  gap: 6px;
}

.search {
  position: relative;
  display: block;
}

.search .icon {
  position: absolute;
  top: 50%;
  left: 11px;
  transform: translateY(-50%);
  color: var(--text-dim);
}

.search input {
  padding-left: 32px;
}

.list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  overflow-y: auto;
  padding-right: 2px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-left: 4px solid var(--role);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 3%, transparent);
  text-align: left;
}

.card:hover {
  background: var(--panel-hover);
}

.card.on {
  border-color: var(--role);
  background: color-mix(in srgb, var(--role) 14%, transparent);
}

.line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
}

.chip {
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--role);
  color: #111;
  font-weight: 700;
}

.cls {
  color: var(--text-dim);
}

.dbl {
  padding: 0 5px;
  border: 1px solid var(--border);
  border-radius: 5px;
}

.lvl {
  margin-left: auto;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.team {
  display: flex;
  flex-wrap: wrap;
}

.empty {
  padding: 12px 4px;
}
</style>
