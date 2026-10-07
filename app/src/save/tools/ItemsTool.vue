<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Combo from "../../components/Combo.vue";
import Icon from "../../components/Icon.vue";
import Tip from "../../components/Tip.vue";
import { getInventory, lists, saveState, setInventory } from "../../saveStore";
import type { InventoryItem, Pouch } from "../../types";
import { POUCH_COLORS, POUCH_LABELS } from "../refdata";
import { useShell } from "../shell";

const original = ref<Pouch[]>([]);
const pouches = ref<Pouch[]>([]);
const current = ref(0);
const search = ref("");
const quantity = ref(99);
const loading = ref(true);

const gen = computed(() => saveState.view!.generation);
const pouch = computed(() => pouches.value[current.value]);

async function load() {
  loading.value = true;
  const inv = (await getInventory()) ?? [];
  original.value = inv;
  pouches.value = JSON.parse(JSON.stringify(inv));
  loading.value = false;
}
onMounted(load);

const dirty = computed(() => JSON.stringify(pouches.value.map((p) => p.items)) !== JSON.stringify(original.value.map((p) => p.items)));
const name = (id: number) => lists.itemName[id] ?? `Objet n°${id}`;

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
const visible = computed(() => {
  const q = fold(search.value.trim());
  return (pouch.value?.items ?? []).map((it, i) => ({ it, i })).filter(({ it }) => !q || fold(name(it.id)).includes(q));
});

/** Options d'objets autorisés dans la poche (déjà présents exclus, sauf celui de la ligne). */
function optionsFor(it: InventoryItem | null) {
  const p = pouch.value;
  if (!p) return [];
  const used = new Set(p.items.map((x) => x.id));
  return p.allowed.filter((id) => id === it?.id || !used.has(id)).map((id) => ({ value: id, label: name(id) }));
}

function setId(i: number, id: number) {
  const p = pouch.value;
  if (!id) p.items.splice(i, 1);
  else p.items[i].id = id;
}

function setCount(i: number, v: string) {
  const p = pouch.value;
  const n = Math.max(0, Math.min(p.maxCount, Number(v) || 0));
  p.items[i].count = n;
}

const adding = ref(0);
function add(id: number) {
  const p = pouch.value;
  if (!id || p.items.some((x) => x.id === id) || p.items.length >= p.capacity) return;
  p.items.push({ id, count: Math.min(quantity.value, p.maxCount), isNew: false, isFavorite: false });
  adding.value = 0;
}

function giveAll() {
  const p = pouch.value;
  const have = new Set(p.items.map((x) => x.id));
  for (const id of p.allowed) {
    if (p.items.length >= p.capacity) break;
    if (!have.has(id) && lists.itemName[id]) p.items.push({ id, count: Math.min(quantity.value, p.maxCount), isNew: false, isFavorite: false });
  }
}

function setEvery() {
  for (const it of pouch.value.items) it.count = Math.min(quantity.value, pouch.value.maxCount);
}

function sortBy(mode: "name" | "id") {
  pouch.value.items.sort((a, b) => (mode === "name" ? name(a.id).localeCompare(name(b.id), "fr") : a.id - b.id));
}

function removeAll() {
  pouch.value.items = [];
}

function discard() {
  pouches.value = JSON.parse(JSON.stringify(original.value));
}

async function saveChanges() {
  if (await setInventory(pouches.value)) await load();
}

const isOneOff = computed(() => ["key_items", "tm_hm", "z_crystals"].includes(pouch.value?.kind ?? ""));
const pouchTip = (k: string) => (k === "key_items" ? "keyItems" : k === "tm_hm" ? "tms" : "pouch");

useShell(() => ({
  hint: dirty.value ? "Modifications non appliquées : « Appliquer » pour les garder" : "Les changements s'appliquent avec « Appliquer »",
  actions: [
    { key: "Ctrl+Enter", cap: "Ctrl+Entrée", label: "Appliquer", run: saveChanges, disabled: !dirty.value },
  ],
}));
</script>

<template>
  <div class="items">
    <nav class="pouches sv-panel">
      <button
        v-for="(p, i) in pouches"
        :key="p.kind"
        :class="{ on: i === current }"
        @click="(current = i), (search = '')"
      >
        <span class="dot" :style="{ background: POUCH_COLORS[p.kind] }"><Icon name="bag" :size="14" /></span>
        <span class="label">{{ POUCH_LABELS[p.kind] ?? p.kind }}</span>
        <span class="count">{{ p.items.length }}</span>
      </button>
    </nav>

    <section v-if="pouch" class="panel sv-panel">
      <div class="toolbar">
        <input v-model="search" class="sv-input search" :placeholder="`Chercher dans ${POUCH_LABELS[pouch.kind] ?? ''}…`" />
        <button class="sv-btn" @click="sortBy('name')" title="Trier par nom"><Icon name="refresh" :size="14" /> Trier A→Z</button>
        <button class="sv-btn" @click="sortBy('id')">Ordre du jeu</button>
        <button class="sv-btn danger" @click="removeAll"><Icon name="trash" :size="14" /> Tout retirer</button>
      </div>
      <div class="toolbar">
        <span class="sv-label">Quantité</span>
        <input v-model.number="quantity" class="sv-input qty" type="number" min="1" :max="pouch.maxCount" />
        <button class="sv-btn" @click="giveAll"><Icon name="gift" :size="14" /> Tout donner</button>
        <button class="sv-btn" :disabled="isOneOff" @click="setEvery"><Icon name="refresh" :size="14" /> Mettre cette quantité partout</button>
        <span class="usage">
          {{ pouch.items.length }} / {{ pouch.capacity }} places · {{ pouch.maxCount }} max par objet
          <Tip :term="pouchTip(pouch.kind)" />
        </span>
      </div>
      <p v-if="pouch.kind === 'key_items'" class="sv-help warn">
        Ajouter des objets rares avant le bon moment de l'histoire peut bloquer un scénario.
      </p>

      <div v-if="loading" class="sv-help">Chargement…</div>
      <div v-else class="grid">
        <div v-for="{ it, i } in visible" :key="it.id" class="row">
          <div class="pick"><Combo :model-value="it.id" :options="optionsFor(it)" @update:model-value="setId(i, $event)" /></div>
          <input
            class="sv-input count-in"
            type="number"
            min="1"
            :max="pouch.maxCount"
            :value="it.count"
            :aria-label="`Quantité de ${name(it.id)}`"
            @change="setCount(i, ($event.target as HTMLInputElement).value)"
          />
          <button v-if="gen >= 7" class="flag" :class="{ on: it.isNew }" title="Marque « nouveau »" @click="it.isNew = !it.isNew">Nouveau</button>
          <button class="del" :aria-label="`Retirer ${name(it.id)}`" @click="pouch.items.splice(i, 1)"><Icon name="x" :size="15" /></button>
        </div>
        <div v-if="pouch.items.length < pouch.capacity" class="row add">
          <div class="pick"><Combo v-model="adding" :options="optionsFor(null)" placeholder="＋ Ajouter un objet…" @update:model-value="add" /></div>
        </div>
      </div>
      <p v-if="gen >= 7" class="sv-help">« Nouveau » : marque affichée dans le sac en jeu. <Tip term="newFlag" /></p>

      <footer class="sv-row">
        <span v-if="dirty" class="pending">Modifications non appliquées</span>
        <button class="sv-btn" :disabled="!dirty" @click="discard">Annuler</button>
        <button class="sv-btn solid" :disabled="!dirty" @click="saveChanges"><Icon name="check" :size="15" /> Appliquer</button>
      </footer>
    </section>
    <p v-else-if="!loading" class="sv-help">Sac introuvable dans cette sauvegarde.</p>
  </div>
</template>

<style scoped>
.items {
  display: grid;
  grid-template-columns: 230px minmax(0, 1fr);
  gap: 18px;
  height: 100%;
  min-height: 0;
}

.pouches {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px;
  overflow-y: auto;
}

.pouches button {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border: none;
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  font-weight: 600;
  text-align: left;
}

.pouches button.on {
  background: var(--text);
  color: var(--bg);
}

.dot {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: #fff;
}

.label {
  flex: 1;
}

.count {
  min-width: 26px;
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, currentColor 18%, transparent);
  font-size: 12px;
  text-align: center;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 16px;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.search {
  flex: 1;
  min-width: 200px;
}

.qty {
  width: 90px;
}

.usage {
  display: flex;
  align-items: center;
  margin-left: auto;
  color: var(--text-dim);
  font-size: 13px;
}

.warn {
  color: var(--warn);
}

.grid {
  display: grid;
  flex: 1;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  align-content: start;
  gap: 8px;
  min-height: 0;
  overflow-y: auto;
  padding: 2px;
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.row.add {
  background: transparent;
  border: 1px dashed var(--border);
}

.pick {
  flex: 1;
  min-width: 0;
}

.count-in {
  width: 82px;
}

.flag {
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: none;
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
}

/* Actif : inversion texte / fond, comme les onglets. */
.flag.on {
  border-color: var(--text);
  background: var(--text);
  color: var(--bg);
}

.del {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: none;
  color: var(--text-dim);
}

.del:hover {
  background: color-mix(in srgb, var(--danger) 25%, transparent);
  color: var(--danger);
}

footer {
  justify-content: flex-end;
}

.pending {
  margin-right: auto;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--warn-bg);
  color: var(--warn);
  font-size: 12px;
  font-weight: 700;
}
</style>
