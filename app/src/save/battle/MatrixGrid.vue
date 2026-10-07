<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import TypeBadge from "../../components/TypeBadge.vue";
import { battle, openDuel, pct, typeTag, type BestMove, type Cell, type KoInfo } from "../../battle";

/** « K.O. en 2 » si assuré, « 63 % en 2 » sinon. */
function koShort(ko: KoInfo) {
  if (!ko.hits) return "pas de K.O.";
  if (ko.guaranteed === ko.hits) return `K.O. en ${ko.hits}`;
  return `${Math.round(ko.chance * 100)} % en ${ko.hits}`;
}

/** Fourchette en % des PV max ; au-delà de 100 %, le détail n'apporte rien. */
const range = (b: BestMove) => (b.minPercent >= 100 ? "≥ 100 %" : `${pct(b.minPercent)}–${pct(Math.min(b.maxPercent, 999))} %`);

const VERDICT_LABEL: Record<Cell["verdict"], string> = {
  win: "Tu gagnes ce duel",
  uncertain: "Ça dépend des jets",
  lose: "Tu perds ce duel",
  none: "Personne ne peut blesser l'autre",
};

const FIRST_SHORT: Record<Cell["first"], string> = { me: "toi", them: "lui", tie: "hasard" };

function firstLabel(c: Cell) {
  if (c.first === "me") return `Tu agis en premier (${c.mySpeed} contre ${c.theirSpeed})`;
  if (c.first === "them") return `Il agit en premier (${c.theirSpeed} contre ${c.mySpeed})`;
  return `Égalité de Vitesse (${c.mySpeed}) : au hasard`;
}

const isSelected = (i: number, j: number) => battle.selected?.mine === i && battle.selected?.theirs === j;

// Focus itinérant : une seule case atteignable au Tab, les flèches se déplacent dans la grille.
const cursor = ref({ i: 0, j: 0 });
const grid = ref<HTMLElement | null>(null);

watch(
  () => battle.selected,
  (s) => {
    if (s) cursor.value = { i: s.mine, j: s.theirs };
  },
);
watch(
  () => battle.matrix,
  (m) => {
    const rows = m?.cells.length ?? 0;
    const cols = m?.cells[0]?.length ?? 0;
    if (cursor.value.i >= rows || cursor.value.j >= cols) cursor.value = { i: 0, j: 0 };
  },
);

async function onKey(e: KeyboardEvent) {
  const cells = battle.matrix?.cells;
  if (!cells?.length) return;
  const rows = cells.length;
  const cols = cells[0].length;
  let { i, j } = cursor.value;
  switch (e.key) {
    case "ArrowRight":
      j = Math.min(cols - 1, j + 1);
      break;
    case "ArrowLeft":
      j = Math.max(0, j - 1);
      break;
    case "ArrowDown":
      i = Math.min(rows - 1, i + 1);
      break;
    case "ArrowUp":
      i = Math.max(0, i - 1);
      break;
    case "Home":
      j = 0;
      if (e.ctrlKey) i = 0;
      break;
    case "End":
      j = cols - 1;
      if (e.ctrlKey) i = rows - 1;
      break;
    default:
      return;
  }
  e.preventDefault();
  e.stopPropagation();
  cursor.value = { i, j };
  await nextTick();
  grid.value?.querySelector<HTMLButtonElement>(`[data-cell="${i}-${j}"]`)?.focus();
}
</script>

<template>
  <div v-if="battle.matrix" class="matrix sv-panel" :class="{ busy: battle.computing }" :aria-busy="battle.computing">
    <div class="title">
      <h3>Matrice des duels <Tip term="battle.matrix" /></h3>
      <span class="legend">
        <i class="win" /> gagné <i class="uncertain" /> incertain <i class="lose" /> perdu <i class="none" /> sans dégâts
        <Tip term="battle.verdict" />
        <span class="sep" />
        <Icon name="clock" :size="12" /> premier à agir <Tip term="battle.speedOrder" />
      </span>
    </div>
    <div class="scroll">
      <table ref="grid">
        <thead>
          <tr>
            <th class="corner">Moi ↓ / Lui →</th>
            <th v-for="(t, j) in battle.matrix.theirs" :key="j">
              <div class="mon">
                <Sprite :id="t.species" :size="44" />
                <div>
                  <strong>{{ t.name }}</strong>
                  <small>N. {{ t.level }}</small>
                  <div class="types"><TypeBadge v-for="ty in t.types" :key="ty" compact :type="typeTag(ty)" /></div>
                </div>
              </div>
            </th>
          </tr>
        </thead>
        <tbody @keydown="onKey">
          <tr v-for="(m, i) in battle.matrix.mine" :key="i">
            <th>
              <div class="mon">
                <Sprite :id="m.species" :size="44" />
                <div>
                  <strong>{{ m.name }}</strong>
                  <small>N. {{ m.level }}</small>
                  <div class="types"><TypeBadge v-for="ty in m.types" :key="ty" compact :type="typeTag(ty)" /></div>
                </div>
              </div>
            </th>
            <td v-for="(c, j) in battle.matrix.cells[i]" :key="j">
              <button
                type="button"
                class="cell"
                :class="[c.verdict, { on: isSelected(i, j) }]"
                :data-cell="`${i}-${j}`"
                :tabindex="cursor.i === i && cursor.j === j ? 0 : -1"
                :aria-pressed="isSelected(i, j)"
                :title="`${VERDICT_LABEL[c.verdict]} — ${firstLabel(c)}. Clic : détail des attaques.`"
                @focus="cursor = { i, j }"
                @click="openDuel(i, j)"
              >
                <span class="row mine">
                  <b>→</b>
                  <template v-if="c.mine">
                    <span class="mv">{{ c.mine.name }}</span>
                    <span class="num">{{ range(c.mine) }}</span>
                    <span class="ko">{{ koShort(c.mine.ko) }}</span>
                  </template>
                  <span v-else class="mv dim">aucun dégât</span>
                </span>
                <span class="row theirs">
                  <b>←</b>
                  <template v-if="c.theirs">
                    <span class="mv">{{ c.theirs.name }}</span>
                    <span class="num">{{ range(c.theirs) }}</span>
                    <span class="ko">{{ koShort(c.theirs.ko) }}</span>
                  </template>
                  <span v-else class="mv dim">aucun dégât</span>
                </span>
                <span class="speed" :class="c.first"><Icon name="clock" :size="11" /> {{ FIRST_SHORT[c.first] }}</span>
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.matrix {
  padding: var(--sp-3) var(--sp-4);
  transition: opacity 0.2s;
}

.matrix.busy {
  opacity: 0.6;
}

.title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
  margin-bottom: var(--sp-3);
}

.title h3 {
  display: flex;
  align-items: center;
  margin: 0;
  font-size: var(--fs-lg);
}

.legend {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.legend i {
  display: inline-block;
  width: 12px;
  height: 12px;
  margin-left: var(--sp-1);
  border: 1.5px solid transparent;
  border-radius: var(--radius-xs);
}

.legend .sep {
  width: 1px;
  height: 14px;
  margin: 0 var(--sp-1);
  background: var(--border);
}

.scroll {
  overflow-x: auto;
}

table {
  border-collapse: separate;
  border-spacing: var(--sp-2);
}

th {
  font-weight: 600;
  text-align: left;
  vertical-align: middle;
}

.corner {
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.mon {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 150px;
}

.mon strong {
  display: block;
  font-size: var(--fs-md);
}

.mon small {
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.types {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
  margin-top: 2px;
}

.cell {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  width: 190px;
  min-height: 64px;
  padding: var(--sp-2) var(--sp-3) 18px;
  border: 1.5px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text);
  text-align: left;
  font-size: var(--fs-sm);
}

/* Fond teinté et bordure pleine : les trois verdicts restent distincts même sur les thèmes pastel (Lagon). */
.win {
  border-color: var(--ok);
  background: color-mix(in srgb, var(--ok) 30%, transparent);
}

.uncertain {
  border-color: var(--warn);
  background: color-mix(in srgb, var(--warn) 30%, transparent);
}

.lose {
  border-color: var(--danger);
  background: color-mix(in srgb, var(--danger) 30%, transparent);
}

.none {
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.cell:hover {
  filter: brightness(1.12);
}

.cell.on {
  border-color: var(--text);
  box-shadow: inset 0 0 0 1px var(--text);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--sp-1);
}

.row b {
  color: var(--text-dim);
}

.mv {
  font-weight: 700;
}

.dim {
  color: var(--text-dim);
  font-weight: 500;
}

.num {
  font-variant-numeric: tabular-nums;
}

.ko {
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.speed {
  position: absolute;
  right: var(--sp-2);
  bottom: var(--sp-1);
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.speed.me {
  color: var(--ok);
}

.speed.them {
  color: var(--danger);
}
</style>
