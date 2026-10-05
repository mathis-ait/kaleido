<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { goTo, loadBox, saveState, setBoxName } from "../../saveStore";

const view = computed(() => saveState.view!);
const names = ref([...view.value.boxNames]);
watch(
  () => view.value.boxNames,
  (n) => (names.value = [...n]),
);

async function commit(i: number) {
  const n = names.value[i].trim();
  if (n && n !== view.value.boxNames[i]) await setBoxName(i, n);
  else names.value[i] = view.value.boxNames[i];
}

async function numberAll() {
  for (let i = 0; i < names.value.length; i++) {
    const n = `Boîte ${i + 1}`.slice(0, view.value.boxNameMax);
    if (view.value.boxNames[i] !== n) await setBoxName(i, n);
  }
}

function open(i: number) {
  loadBox(i);
  goTo("boxes");
}
</script>

<template>
  <div class="sv-panel wrap">
    <div class="sv-row head">
      <h3 class="sv-section-title">Noms des boîtes <Tip term="boxName" /></h3>
      <button class="sv-btn" @click="numberAll">Renommer « Boîte 1, 2, 3… »</button>
    </div>
    <div class="list">
      <div v-for="(_, i) in names" :key="i" class="item">
        <span class="n">{{ i + 1 }}</span>
        <input v-model="names[i]" class="sv-input" :maxlength="view.boxNameMax" @change="commit(i)" @keydown.enter="commit(i)" />
        <span class="fill">{{ view.boxFill[i] }}/30</span>
        <button class="sv-btn" @click="open(i)">Ouvrir</button>
      </div>
    </div>
    <p class="sv-help">{{ view.boxNameMax }} caractères au maximum. Fonds d'écran des boîtes : bientôt.</p>
  </div>
</template>

<style scoped>
.wrap {
  display: flex;
  flex-direction: column;
  padding: 20px;
  overflow: hidden;
}

.head {
  justify-content: space-between;
}

.list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 8px 16px;
  overflow-y: auto;
}

.item {
  display: flex;
  align-items: center;
  gap: 10px;
}

.n {
  min-width: 24px;
  color: var(--text-dim);
  font-weight: 700;
  text-align: right;
}

.fill {
  color: var(--text-dim);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
</style>
