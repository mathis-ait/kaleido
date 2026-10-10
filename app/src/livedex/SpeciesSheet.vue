<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Dialog from "../components/Dialog.vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import TypeBadge from "../components/TypeBadge.vue";
import { launchGame } from "../launcher/actions";
import type { PokeTypeKey } from "../types";
import { BALLS, CATEGORY_NAMES, dex, genLabel, KIND_NAMES, loadSpecies, TAG_NAMES, TYPE_NAMES } from "./data";
import { ownedGames } from "./owned";
import { collection, manualOf } from "./store";
import type { CatchEntry, EncounterRow, SpeciesDetail, TypeId } from "./types";
import { livedexUi, openEntry, openSpecies } from "./ui";

const open = computed({
  get: () => livedexUi.species !== null,
  set: (v: boolean) => {
    if (!v) livedexUi.species = null;
  },
});

const species = computed(() => (livedexUi.species !== null ? dex.value?.species(livedexUi.species) : undefined));
const detail = ref<SpeciesDetail | null>(null);
const failed = ref(false);
watch(
  () => livedexUi.species,
  async (id) => {
    detail.value = null;
    failed.value = false;
    if (id === null) return;
    try {
      const d = await loadSpecies(id);
      if (livedexUi.species === id) detail.value = d;
    } catch {
      failed.value = true;
    }
  },
  { immediate: true },
);

/** Forme dont on montre « où le trouver ». */
const form = computed(() => species.value?.forms.find((f) => f.f === livedexUi.form) ?? species.value?.forms[0]);
const visibleForms = computed(() => species.value?.forms.filter((f) => f.cat !== "hidden" && (f.present.length > 0 || f.f === 0)) ?? []);

const typeTag = (t: TypeId) => ({ key: (t === "stellar" ? "normal" : t) as PokeTypeKey, name: TYPE_NAMES[t] });
const num = (n: number) => `n° ${String(n).padStart(4, "0")}`;

const slots = computed(() => (species.value ? (collection.value?.slotsBySpecies.get(species.value.id) ?? []) : []));
const mine = computed<CatchEntry[]>(() => {
  const c = collection.value;
  if (!c || !species.value) return [];
  return slots.value.flatMap((s) => c.bySlot.get(s.key) ?? []);
});

const gameName = (id: string | null) => (id ? (dex.value?.game(id)?.name ?? id) : "Jeu inconnu");
const gameAt = (i: number) => dex.value?.games[i];

// --- Où le trouver : lignes regroupées par jeu, jeux possédés d'abord.
interface GameBlock {
  index: number;
  id: string;
  name: string;
  owned: boolean;
  rows: EncounterRow[];
  evolve: { from: [number, number]; how: string }[];
  breed: boolean;
}

const blocks = computed<GameBlock[]>(() => {
  const d = detail.value;
  const f = form.value;
  if (!d || !f) return [];
  const fd = d.forms[String(f.f)];
  if (!fd) return [];
  const map = new Map<number, GameBlock>();
  const block = (i: number) => {
    let b = map.get(i);
    if (!b) {
      const g = gameAt(i);
      b = { index: i, id: g?.id ?? String(i), name: g?.name ?? "?", owned: !!g && ownedGames.value.has(g.id), rows: [], evolve: [], breed: false };
      map.set(i, b);
    }
    return b;
  };
  for (const r of fd.rows) for (const g of r.g) block(g).rows.push(r);
  for (const e of fd.evolve) for (const g of e.g) block(g).evolve.push({ from: e.from, how: e.how });
  for (const g of fd.breed) block(g).breed = true;
  return [...map.values()].sort((a, b) => Number(b.owned) - Number(a.owned) || a.index - b.index);
});
const ownedBlocks = computed(() => blocks.value.filter((b) => b.owned));
const otherBlocks = computed(() => blocks.value.filter((b) => !b.owned));

const place = (r: EncounterRow) => (r.l !== undefined ? detail.value?.strings[r.l] : undefined);
const levels = (r: EncounterRow) => (r.lv[0] ? (r.lv[0] === r.lv[1] ? `N. ${r.lv[0]}` : `N. ${r.lv[0]}–${r.lv[1]}`) : "");
const formLabel = (s: number, f: number) => dex.value?.form(s, f)?.full ?? dex.value?.species(s)?.name ?? `#${s}`;

const launching = ref<string | null>(null);
async function playGame(id: string) {
  const d = ownedGames.value.get(id);
  if (!d) return;
  launching.value = id;
  await launchGame(d);
  launching.value = null;
}

function note() {
  if (!species.value) return;
  openEntry({ species: species.value.id, form: form.value?.f ?? 0 });
}

function edit(e: CatchEntry) {
  const m = e.auto ? undefined : manualOf(e.id);
  if (m) openEntry({ ...m });
}
</script>

<template>
  <Dialog v-model="open" :title="species ? species.name : ''" :subtitle="species ? num(species.id) : ''" :width="900">
    <template #head>
      <button type="button" class="sv-btn small" @click="note"><Icon name="plus" :size="14" /> Noter ce Pokémon</button>
    </template>

    <template v-if="species">
      <div class="top">
        <Sprite :id="species.id" :form="form?.f ?? 0" variant="model" :size="112" />
        <div class="ident">
          <div class="types">
            <TypeBadge v-for="t in form?.types ?? []" :key="t" :type="typeTag(t)" />
            <span v-for="t in species.tags" :key="t" class="sv-chip dim">{{ TAG_NAMES[t] }}</span>
          </div>
          <p class="dim">{{ genLabel(species.gen) }} · {{ species.en }}</p>
          <div class="slots">
            <span v-for="s in slots" :key="s.key" class="sv-chip" :class="collection?.caughtShiny.has(s.key) ? 'shiny' : collection?.caught.has(s.key) ? 'ok' : 'dim'" :title="s.label">
              {{ s.label }}
            </span>
          </div>
        </div>
      </div>

      <section v-if="visibleForms.length > 1">
        <h3 class="sv-section-title">Formes <Tip term="livedex.form" /></h3>
        <div class="forms">
          <button v-for="f in visibleForms" :key="f.f" type="button" class="sv-chip" :class="{ on: form?.f === f.f }" :title="CATEGORY_NAMES[f.cat]" @click="livedexUi.form = f.f">
            {{ f.name || "Normale" }}
          </button>
        </div>
      </section>

      <section>
        <h3 class="sv-section-title">Mes exemplaires</h3>
        <p v-if="!mine.length" class="dim">Aucun pour l'instant.</p>
        <ul v-else class="mine">
          <li v-for="e in mine" :key="e.id">
            <Sprite :id="e.species" :form="e.form" :shiny="e.shiny" :size="40" />
            <div class="what">
              <strong>{{ e.nickname || formLabel(e.species, e.form) }}<span v-if="e.shiny" class="shiny-mark" title="Chromatique"> ★</span></strong>
              <small class="dim">
                {{ gameName(e.game) }}<template v-if="e.location"> · {{ e.location }}</template><template v-if="e.level"> · N. {{ e.level }}</template
                ><template v-if="e.ball"> · {{ BALLS[e.ball] }}</template><template v-if="e.ot"> · DO {{ e.ot }}</template>
              </small>
            </div>
            <span v-if="e.auto" class="src dim" :title="e.where?.map((w) => `${w.label} · ${w.place}`).join('\n')">
              {{ e.where?.[0]?.label }}<template v-if="(e.where?.length ?? 0) > 1"> +{{ (e.where?.length ?? 1) - 1 }}</template>
            </span>
            <button v-else type="button" class="sv-btn small" @click="edit(e)">Modifier</button>
            <span v-if="e.legality === 'illegal'" class="sv-chip danger" title="Jugé illégal par le moteur de légalité">Illégal</span>
          </li>
        </ul>
      </section>

      <section v-if="detail && detail.family.length > 1">
        <h3 class="sv-section-title">Évolution</h3>
        <div class="family">
          <button v-for="n in detail.family" :key="`${n.s}-${n.f}`" type="button" class="evo sv-card" :class="{ on: n.s === species.id }" @click="openSpecies(n.s, n.f)">
            <Sprite :id="n.s" :form="n.f" :size="44" />
            <span class="text">
              <strong>{{ formLabel(n.s, n.f) }}</strong>
              <small v-if="n.how" class="dim">{{ n.how }}</small>
            </span>
          </button>
        </div>
      </section>

      <section>
        <h3 class="sv-section-title">Où le trouver<Tip term="livedex.obtain" /></h3>
        <p v-if="failed" class="dim">Impossible de charger cette fiche.</p>
        <p v-else-if="!detail" class="dim">Chargement…</p>
        <p v-else-if="!blocks.length" class="dim">Aucune façon connue de l'obtenir{{ form?.go ? ", sauf dans Pokémon GO" : "" }}.</p>
        <template v-else>
          <div v-for="b in ownedBlocks" :key="b.index" class="game owned">
            <header>
              <strong>{{ b.name }}</strong>
              <span class="sv-chip ok">Dans ta bibliothèque</span>
              <button type="button" class="sv-btn small solid" :disabled="launching === b.id" @click="playGame(b.id)"><Icon name="power" :size="13" /> Jouer</button>
            </header>
            <ul>
              <li v-for="(r, i) in b.rows" :key="i">
                <span class="kind">{{ r.m ?? KIND_NAMES[r.k] }}</span>
                <span>{{ place(r) ?? "" }}</span>
                <span class="dim">{{ levels(r) }}<template v-if="r.c?.length"> · {{ r.c.join(", ") }}</template><template v-if="r.s === 'locked'"> · chromatique impossible</template><template v-if="r.n"> · {{ r.n }}</template></span>
              </li>
              <li v-for="(e, i) in b.evolve" :key="`e${i}`"><span class="kind">Évolution</span><span>{{ formLabel(e.from[0], e.from[1]) }}</span><span class="dim">{{ e.how }}</span></li>
              <li v-if="b.breed"><span class="kind">Œuf</span><span>Reproduction</span><span /></li>
            </ul>
          </div>
          <details :open="!ownedBlocks.length" class="others">
            <summary>{{ ownedBlocks.length ? `Autres jeux (${otherBlocks.length})` : `${otherBlocks.length} jeux` }}</summary>
            <div v-for="b in otherBlocks" :key="b.index" class="game">
              <header><strong>{{ b.name }}</strong></header>
              <ul>
                <li v-for="(r, i) in b.rows" :key="i">
                  <span class="kind">{{ r.m ?? KIND_NAMES[r.k] }}</span>
                  <span>{{ place(r) ?? "" }}</span>
                  <span class="dim">{{ levels(r) }}<template v-if="r.c?.length"> · {{ r.c.join(", ") }}</template><template v-if="r.s === 'locked'"> · chromatique impossible</template><template v-if="r.n"> · {{ r.n }}</template></span>
                </li>
                <li v-for="(e, i) in b.evolve" :key="`e${i}`"><span class="kind">Évolution</span><span>{{ formLabel(e.from[0], e.from[1]) }}</span><span class="dim">{{ e.how }}</span></li>
                <li v-if="b.breed"><span class="kind">Œuf</span><span>Reproduction</span><span /></li>
              </ul>
            </div>
          </details>
        </template>
      </section>
    </template>
  </Dialog>
</template>

<style scoped>
.top {
  display: flex;
  gap: var(--sp-4);
  align-items: center;
}

.ident {
  display: grid;
  gap: var(--sp-2);
  min-width: 0;
}

.ident p {
  margin: 0;
}

.types,
.slots,
.forms {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

section {
  margin-top: var(--sp-5);
}

.dim {
  color: var(--text-dim);
}

.mine {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.mine li {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 4px 10px 4px 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.what {
  display: grid;
  flex: 1;
  min-width: 0;
}

.what small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shiny-mark {
  color: var(--shiny);
}

.src {
  font-size: var(--fs-sm);
  white-space: nowrap;
}

.family {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
}

.evo {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: 4px 12px 4px 4px;
}

.evo .text {
  display: grid;
}

.game {
  margin-top: var(--sp-3);
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.game header {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.game header .sv-btn {
  margin-left: auto;
}

.game ul {
  display: grid;
  gap: 2px;
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-sm);
}

.game li {
  display: grid;
  grid-template-columns: 200px minmax(0, 1fr) minmax(0, 1.2fr);
  gap: var(--sp-3);
}

.kind {
  font-weight: 600;
}

.others {
  margin-top: var(--sp-3);
}

.others summary {
  color: var(--text-dim);
  cursor: pointer;
}
</style>
