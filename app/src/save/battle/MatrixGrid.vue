<script setup lang="ts">
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import TypeBadge from "../../components/TypeBadge.vue";
import { battle, openDuel, pct, typeTag, type BestMove, type Cell, type KoInfo } from "../../battle";
import { BATTLE_TIPS } from "./glossary";

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

function firstLabel(c: Cell) {
  if (c.first === "me") return `Tu agis en premier (${c.mySpeed} contre ${c.theirSpeed})`;
  if (c.first === "them") return `Il agit en premier (${c.theirSpeed} contre ${c.mySpeed})`;
  return `Égalité de Vitesse (${c.mySpeed}) : au hasard`;
}

const isSelected = (i: number, j: number) => battle.selected?.mine === i && battle.selected?.theirs === j;
</script>

<template>
  <div v-if="battle.matrix" class="matrix sv-panel" :class="{ busy: battle.computing }">
    <div class="title">
      <h3>Matrice des duels <Tip v-bind="BATTLE_TIPS.matrix" /></h3>
      <span class="legend">
        <i class="win" /> gagné <i class="uncertain" /> incertain <i class="lose" /> perdu
        <Tip v-bind="BATTLE_TIPS.verdict" />
      </span>
    </div>
    <div class="scroll">
      <table>
        <thead>
          <tr>
            <th class="corner">Moi ↓ / Lui →</th>
            <th v-for="(t, j) in battle.matrix.theirs" :key="j">
              <div class="mon">
                <Sprite :id="t.species" :size="44" />
                <div>
                  <strong>{{ t.name }}</strong>
                  <small>N. {{ t.level }}</small>
                  <div class="types"><TypeBadge v-for="ty in t.types" :key="ty" :type="typeTag(ty)" /></div>
                </div>
              </div>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(m, i) in battle.matrix.mine" :key="i">
            <th>
              <div class="mon">
                <Sprite :id="m.species" :size="44" />
                <div>
                  <strong>{{ m.name }}</strong>
                  <small>N. {{ m.level }}</small>
                  <div class="types"><TypeBadge v-for="ty in m.types" :key="ty" :type="typeTag(ty)" /></div>
                </div>
              </div>
            </th>
            <td v-for="(c, j) in battle.matrix.cells[i]" :key="j">
              <button
                class="cell"
                :class="[c.verdict, { on: isSelected(i, j) }]"
                :title="`${VERDICT_LABEL[c.verdict]} — ${firstLabel(c)}. Clic : détail des attaques.`"
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
                <span class="speed" :class="c.first">{{ c.first === "me" ? "⚡ toi" : c.first === "them" ? "⚡ lui" : "⚡ =" }}</span>
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
  padding: 14px 16px;
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
  gap: 8px;
  margin-bottom: 10px;
}

.title h3 {
  display: flex;
  align-items: center;
  margin: 0;
  font-size: 16px;
}

.legend {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 12px;
}

.legend i {
  display: inline-block;
  width: 12px;
  height: 12px;
  margin-left: 6px;
  border-radius: 3px;
}

.scroll {
  overflow-x: auto;
}

table {
  border-collapse: separate;
  border-spacing: 6px;
}

th {
  font-weight: 600;
  text-align: left;
  vertical-align: middle;
}

.corner {
  color: var(--text-dim);
  font-size: 11px;
}

.mon {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 150px;
}

.mon strong {
  display: block;
  font-size: 13px;
}

.mon small {
  color: var(--text-dim);
  font-size: 11px;
}

.types {
  display: flex;
  gap: 3px;
  margin-top: 2px;
}

.types :deep(.type) {
  min-width: 0;
  padding: 0 5px;
  font-size: 10px;
}

.cell {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 190px;
  min-height: 64px;
  padding: 8px 10px 18px;
  border: 1.5px solid transparent;
  border-radius: 10px;
  text-align: left;
  font-size: 12px;
}

.win,
.legend .win {
  background: color-mix(in srgb, #22c55e 26%, transparent);
}

.uncertain,
.legend .uncertain {
  background: color-mix(in srgb, #f59e0b 26%, transparent);
}

.lose,
.legend .lose {
  background: color-mix(in srgb, #ef4444 26%, transparent);
}

.none {
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.cell:hover {
  filter: brightness(1.12);
}

.cell.on {
  border-color: var(--text);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px;
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
  font-size: 11px;
}

.speed {
  position: absolute;
  right: 8px;
  bottom: 4px;
  color: var(--text-dim);
  font-size: 10px;
}

.speed.me {
  color: #16a34a;
}

.speed.them {
  color: var(--danger);
}
</style>
