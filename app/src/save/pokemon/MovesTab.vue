<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import CategoryIcon from "../../components/CategoryIcon.vue";
import Combo, { type ComboOption } from "../../components/Combo.vue";
import Tip from "../../components/Tip.vue";
import TypeBadge from "../../components/TypeBadge.vue";
import { lists, notify } from "../../saveStore";
import type { SlotView } from "../../types";
import { TYPE_KEYS } from "../refdata";
import { apply } from "./edit";

const props = defineProps<{ p: SlotView }>();

/** Infos des 4 emplacements (les attaques vides n'ont pas d'entrée dans `knownMoves`). */
const rows = computed(() => props.p.moves.map((id, i) => ({ id, i, info: props.p.knownMoves.find((k) => k.id === id && id !== 0) ?? null })));
const target = ref(0);

function setMove(i: number, id: number) {
  if (id === props.p.moves[i]) return;
  const moves = [...props.p.moves];
  if (id && moves.includes(id)) {
    notify("Cette attaque est déjà connue");
    return;
  }
  moves[i] = id;
  apply({ moves }).then(() => refillPp());
}

function moveModel(i: number) {
  return computed({ get: () => props.p.moves[i], set: (v: number) => setMove(i, v) });
}
const models = [0, 1, 2, 3].map(moveModel);

const maxPps = computed(() => props.p.moves.map((id, i) => (id ? (props.p.knownMoves.find((k) => k.id === id)?.basePp ?? 0) : 0) * (1 + (Math.min(3, props.p.ppUps[i]) * 0.2))).map(Math.floor));

function refillPp() {
  apply({ pp: maxPps.value });
}

function setPp(i: number, v: string) {
  const pp = [...props.p.pp];
  pp[i] = Math.max(0, Math.min(maxPps.value[i] || 99, Number(v) || 0));
  apply({ pp });
}

function setUps(i: number, v: number) {
  const ups = [...props.p.ppUps];
  ups[i] = v;
  apply({ ppUps: ups }).then(refillPp);
}

async function suggest() {
  const s = await invoke<{ moves: number[]; pp: number[] }>("save_suggest_moves", { species: props.p.species, form: props.p.form, level: props.p.level });
  if (s.moves.every((m) => !m)) {
    notify("Aucune attaque apprise par niveau connue pour cette espèce");
    return;
  }
  apply({ moves: s.moves, pp: s.pp, ppUps: [0, 0, 0, 0] });
}

// Attaques apprenables : par niveau, par reproduction, et toutes celles du jeu (CT, donneurs…).
type Learnset = { levelup: [number, number][]; egg: number[]; learnable: number[] };
const learnset = ref<Learnset | null>(null);
watch(
  () => [props.p.species, props.p.form, props.p.level],
  async () => {
    learnset.value = await invoke<Learnset>("save_learnset", { species: props.p.species, form: props.p.form, level: props.p.level }).catch(() => null);
  },
  { immediate: true },
);

const CATS = ["physical", "special", "status"] as const;

/** Attaques de la liste : apprenables en tête (en vert), avec type et catégorie. */
const moveOptions = computed<ComboOption[]>(() => {
  const ok = new Set(learnset.value?.learnable ?? []);
  const decorate = (o: { value: number; label: string }, good: boolean): ComboOption => {
    const meta = lists.moveMeta[o.value];
    return { ...o, good, type: meta ? typeTag(meta[0]) : undefined, category: meta ? (CATS.includes(meta[1]) ? meta[1] : undefined) : undefined };
  };
  const good = lists.moves.filter((o) => ok.has(o.value)).map((o) => decorate(o, true));
  const rest = lists.moves.filter((o) => !ok.has(o.value)).map((o) => decorate(o, false));
  return [...good, ...rest];
});
const learnableCount = computed(() => learnset.value?.learnable.length ?? 0);

function pick(id: number) {
  const empty = props.p.moves.findIndex((m) => !m);
  setMove(empty >= 0 ? empty : target.value, id);
}

const typeTag = (t: number) => ({ key: TYPE_KEYS[t] ?? "normal", name: lists.types[t] ?? "" });
const name = (id: number) => lists.moveName[id] ?? `n°${id}`;
</script>

<template>
  <div class="moves">
    <div class="list">
      <h3 class="sv-section-title">Attaques <Tip term="moves" /></h3>
      <div v-for="r in rows" :key="r.i" class="move" :class="{ target: target === r.i }" @click="target = r.i">
        <span class="n">{{ r.i + 1 }}</span>
        <div class="pick-move">
          <Combo v-model="models[r.i].value" :options="moveOptions" none-label="(Aucune)" placeholder="Choisir une attaque…" />
        </div>
        <div class="meta">
          <template v-if="r.info">
            <TypeBadge :type="typeTag(r.info.typeId)" />
            <CategoryIcon v-if="lists.moveMeta[r.id]" :cat="lists.moveMeta[r.id]![1]" />
            <small>{{ r.info.category }}</small>
            <small v-if="learnset && r.id && !learnset.learnable.includes(r.id)" class="warn" title="Attaque que l'espèce ne peut pas apprendre dans ce jeu (hors événements et transferts)">Non apprenable</small>
            <small>Puiss. {{ r.info.power ?? "—" }}</small>
            <small>Préc. {{ r.info.accuracy ?? "—" }}</small>
          </template>
        </div>
        <div class="pp">
          <input class="sv-input" type="number" min="0" :max="maxPps[r.i]" :value="p.pp[r.i]" :disabled="!r.id" aria-label="PP" @change="setPp(r.i, ($event.target as HTMLInputElement).value)" />
          <span class="dim">/ {{ maxPps[r.i] }}</span>
          <select class="sv-select ups" :value="p.ppUps[r.i]" :disabled="!r.id" aria-label="PP Plus" @change="setUps(r.i, Number(($event.target as HTMLSelectElement).value))">
            <option v-for="u in 4" :key="u" :value="u - 1">{{ u - 1 }} PP Plus</option>
          </select>
        </div>
      </div>
      <div class="sv-row actions">
        <button class="sv-btn solid" @click="suggest">Attaques suggérées</button>
        <button class="sv-btn" @click="refillPp">PP au maximum <Tip term="pp" /></button>
        <button class="sv-btn" @click="apply({ ppUps: [3, 3, 3, 3] }).then(refillPp)">PP Plus partout <Tip term="ppUps" /></button>
      </div>
      <p class="sv-help">« Attaques suggérées » reprend les 4 dernières attaques apprises par niveau, comme PKHeX.</p>
      <p v-if="learnableCount" class="sv-help">
        Dans la liste, les <span class="good">{{ learnableCount }} attaques apprenables</span> dans ce jeu (niveau, CT/CS, donneurs, Œuf) sont en tête, en vert.
        <Tip term="learnable" />
      </p>
    </div>

    <aside class="learn">
      <h3 class="sv-section-title">Apprises par niveau</h3>
      <p class="sv-help">Clic pour ajouter (dans une place libre, sinon à la place {{ target + 1 }}).</p>
      <ul>
        <li v-for="([m, l], k) in learnset?.levelup ?? []" :key="`l${k}`">
          <button :class="{ known: p.moves.includes(m) }" @click="pick(m)">
            <span class="lv">{{ l === 0 ? "Évol." : `N. ${l}` }}</span>{{ name(m) }}
          </button>
        </li>
      </ul>
      <template v-if="learnset?.egg.length">
        <h3 class="sv-section-title">Par reproduction</h3>
        <ul>
          <li v-for="m in learnset.egg" :key="`e${m}`">
            <button :class="{ known: p.moves.includes(m) }" @click="pick(m)"><span class="lv">Œuf</span>{{ name(m) }}</button>
          </li>
        </ul>
      </template>
    </aside>
  </div>
</template>

<style scoped>
.moves {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 260px;
  gap: 24px;
}

.move {
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.move.target {
  border-color: color-mix(in srgb, var(--text) 40%, transparent);
}

.n {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 15%, transparent);
  font-weight: 700;
}

.meta {
  grid-column: 2 / -1;
  grid-row: 2;
  min-height: 20px;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 10px;
}

.meta small.warn {
  color: var(--danger);
  font-weight: 700;
}

.good {
  color: #8ff0b5;
  font-weight: 700;
}

.meta small,
.dim {
  color: var(--text-dim);
  font-size: 12px;
}

.pp {
  display: flex;
  align-items: center;
  gap: 6px;
}

.pp .sv-input {
  width: 64px;
  padding: 7px 8px;
}

.ups {
  width: auto;
  padding: 7px 8px;
}

.actions {
  margin-top: 8px;
}

.learn {
  max-height: 100%;
}

.learn ul {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
}

.learn button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  font-size: 13px;
  font-weight: 600;
  text-align: left;
}

.learn button:hover {
  background: color-mix(in srgb, var(--text) 14%, transparent);
}

.learn button.known {
  opacity: 0.5;
}

.lv {
  min-width: 44px;
  color: var(--text-dim);
  font-size: 11px;
}
</style>
