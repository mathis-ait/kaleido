<script setup lang="ts">
import { computed, ref } from "vue";
import Tip from "../../components/Tip.vue";
import type { SlotView } from "../../types";
import { apply } from "./edit";

const props = defineProps<{ p: SlotView }>();

const query = ref("");
const onlyOn = ref(false);
const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

const ribbons = computed(() => props.p.extras.ribbons);
const count = computed(() => ribbons.value.filter((r) => r.on).length);
const shown = computed(() => {
  const q = fold(query.value.trim());
  return ribbons.value.filter((r) => (!onlyOn.value || r.on) && (!q || fold(r.name).includes(q)));
});

function toggle(key: string, on: boolean) {
  apply({ extras: { ribbons: { [key]: on } } });
}

function clearAll() {
  apply({ extras: { ribbons: Object.fromEntries(ribbons.value.filter((r) => r.on).map((r) => [r.key, false])) } });
}
</script>

<template>
  <div class="ribbons">
    <div class="bar">
      <h3 class="sv-section-title">Rubans · {{ count }}/{{ ribbons.length }} <Tip term="ribbons" /></h3>
      <input v-model="query" class="sv-input search" type="search" placeholder="Chercher un ruban…" />
      <label class="sv-switch">
        <input v-model="onlyOn" type="checkbox" />
        <span class="track" />
        Seulement ceux posés
      </label>
      <button class="sv-btn danger" :disabled="!count" @click="clearAll">Tout retirer</button>
    </div>
    <p class="sv-help">Un ruban impossible à obtenir pour ce Pokémon le rend illégal : pose seulement ceux qu'il a pu gagner.</p>
    <div class="grid">
      <label v-for="r in shown" :key="r.key" class="ribbon" :class="{ on: r.on }">
        <input type="checkbox" :checked="r.on" @change="toggle(r.key, !r.on)" />
        <span class="medal" aria-hidden="true">🎗</span>
        <span>{{ r.name }}</span>
      </label>
      <p v-if="!shown.length" class="sv-help">Aucun ruban ne correspond.</p>
    </div>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px 18px;
}

.bar .sv-section-title {
  margin: 0;
}

.search {
  width: 240px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 4px 12px;
  margin-top: 14px;
}

.ribbon {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 10px;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
}

.ribbon:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.ribbon.on {
  background: color-mix(in srgb, #ffc94d 14%, transparent);
  color: var(--text);
  font-weight: 600;
}

.medal {
  filter: grayscale(1);
  opacity: 0.45;
}

.ribbon.on .medal {
  filter: none;
  opacity: 1;
}
</style>
