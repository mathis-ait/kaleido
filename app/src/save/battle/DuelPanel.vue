<script setup lang="ts">
import { computed } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import TypeBadge from "../../components/TypeBadge.vue";
import { battle, pct, typeTag, type Combatant, type Duel, type MoveLine } from "../../battle";
import { lists } from "../../saveStore";
import { natureLabel, STAT_SHORT } from "../refdata";
import { BATTLE_TIPS } from "./glossary";

const d = computed(() => battle.duel);

const abilityName = computed(() => Object.fromEntries(lists.abilities.map((a) => [a.value, a.label])) as Record<number, string>);
const ability = (c: Combatant) => abilityName.value[c.ability] ?? `Talent n°${c.ability}`;
const item = (c: Combatant) => (c.item ? (lists.itemName[c.item] ?? `Objet n°${c.item}`) : "Aucun objet");

const CATEGORY: Record<MoveLine["category"], string> = { physical: "Physique", special: "Spéciale", status: "Statut" };

function effLabel(e: number) {
  if (e === 0) return "×0";
  return `×${e.toLocaleString("fr-FR")}`;
}

/** Position de la fourchette de dégâts sur la barre de PV (en % des PV max). */
function bar(m: MoveLine, duel: Duel) {
  const max = duel.defenderMaxHp;
  const left = Math.min(100, (m.min / max) * 100);
  const width = Math.max(1.5, Math.min(100, (m.max / max) * 100) - left);
  return { left: `${left}%`, width: `${width}%` };
}

const sides = computed(() =>
  d.value
    ? [
        { key: "attack", title: `Mes attaques sur ${d.value.them.name}`, duel: d.value.attack, attacker: d.value.me, defender: d.value.them },
        { key: "defense", title: `Ses attaques sur ${d.value.me.name}`, duel: d.value.defense, attacker: d.value.them, defender: d.value.me },
      ]
    : [],
);

const speedText = computed(() => {
  if (!d.value) return "";
  const [me, them] = [d.value.attack.attackerSpeed, d.value.attack.defenderSpeed];
  if (me > them) return `${d.value.me.name} est plus rapide (${me} contre ${them}).`;
  if (me < them) return `${d.value.them.name} est plus rapide (${them} contre ${me}).`;
  return `Même Vitesse (${me}) : l'ordre est tiré au hasard.`;
});
</script>

<template>
  <section v-if="d" class="duel sv-panel">
    <header class="vs">
      <div v-for="(c, k) in [d.me, d.them]" :key="k" class="fighter" :class="{ foe: k === 1 }">
        <Sprite :id="c.species" :size="72" />
        <div class="info">
          <strong>{{ c.name }} <small>N. {{ c.level }}</small></strong>
          <div class="types"><TypeBadge v-for="t in c.types" :key="t" :type="typeTag(t)" /></div>
          <small>{{ ability(c) }} · {{ item(c) }} · {{ natureLabel(c.nature) }}</small>
          <div class="stats">
            <span v-for="(s, i) in c.stats" :key="i"><em>{{ STAT_SHORT[i] }}</em> {{ s }}</span>
          </div>
          <small v-if="k === 1" class="ivs">
            IV {{ c.ivs[0] }} partout, 0 EV <Tip v-bind="BATTLE_TIPS.trainerIvs" />
          </small>
          <ul v-if="c.notes.length" class="notes">
            <li v-for="n in c.notes" :key="n"><Icon name="info" :size="12" /> {{ n }}</li>
          </ul>
        </div>
      </div>
    </header>
    <p class="speed"><Icon name="clock" :size="14" /> {{ speedText }} <Tip v-bind="BATTLE_TIPS.speed" /> <Tip v-bind="BATTLE_TIPS.priority" /></p>

    <div class="sides">
      <div v-for="s in sides" :key="s.key" class="side">
        <h4>
          {{ s.title }}
          <small>PV pris en compte : {{ s.duel.defenderHp }} / {{ s.duel.defenderMaxHp }}</small>
        </h4>
        <table>
          <thead>
            <tr>
              <th>Attaque</th>
              <th>Dégâts <Tip v-bind="BATTLE_TIPS.random" /></th>
              <th>K.O. <Tip v-bind="BATTLE_TIPS.ko" /></th>
              <th>Critique <Tip v-bind="BATTLE_TIPS.crit" /></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="m in s.duel.moves" :key="m.moveId" :class="{ none: m.damage.kind === 'none' }">
              <td class="move">
                <div class="mvhead">
                  <TypeBadge :type="typeTag(m.typeId)" />
                  <strong>{{ m.name }}</strong>
                </div>
                <small>
                  {{ CATEGORY[m.category] }} <Tip v-bind="BATTLE_TIPS.category" />
                  <template v-if="m.power"> · Puissance {{ m.power }}</template>
                  <template v-if="m.hits > 1"> · {{ m.hits }} coups <Tip v-bind="BATTLE_TIPS.multihit" /></template>
                  <template v-if="m.priority"> · Priorité {{ m.priority > 0 ? "+" : "" }}{{ m.priority }}</template>
                  <template v-if="m.damage.kind === 'rolls'"> · {{ effLabel(m.effectiveness) }} <Tip v-bind="BATTLE_TIPS.effectiveness" /></template>
                </small>
                <div v-if="m.modifiers.length" class="mods">
                  <span v-for="x in m.modifiers" :key="x" class="mod">{{ x }}</span>
                  <Tip v-bind="BATTLE_TIPS.modifiers" />
                </div>
              </td>
              <td v-if="m.damage.kind === 'none'" colspan="3" class="reason">{{ m.damage.reason }}</td>
              <template v-else>
                <td class="dmg">
                  <strong>{{ pct(m.minPercent) }} – {{ pct(m.maxPercent) }} %</strong>
                  <small>
                    {{ m.min }} – {{ m.max }} PV
                    <template v-if="m.damage.kind === 'fixed'"> (fixes <Tip v-bind="BATTLE_TIPS.fixed" />)</template>
                  </small>
                  <div class="hpbar" :title="`Jets possibles : ${m.rolls.join(', ')}`">
                    <i :style="bar(m, s.duel)" />
                  </div>
                </td>
                <td class="ko" :class="{ good: m.ko.hits && m.ko.guaranteed === m.ko.hits }">{{ m.ko.label }}</td>
                <td class="crit">{{ pct(m.critMinPercent) }} – {{ pct(m.critMaxPercent) }} %<br /><small>{{ m.critMin }} – {{ m.critMax }} PV</small></td>
              </template>
            </tr>
          </tbody>
        </table>
        <p v-if="!s.duel.moves.length" class="sv-help">Aucune attaque connue.</p>
        <ul v-if="s.duel.notes.length" class="notes">
          <li v-for="n in s.duel.notes" :key="n"><Icon name="info" :size="12" /> {{ n }}</li>
        </ul>
      </div>
    </div>
  </section>
</template>

<style scoped>
.duel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.vs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.fighter {
  display: flex;
  gap: 12px;
  padding: 10px;
  border-radius: 12px;
  background: color-mix(in srgb, #22c55e 8%, transparent);
}

.fighter.foe {
  background: color-mix(in srgb, #ef4444 8%, transparent);
}

.info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.info strong small {
  color: var(--text-dim);
  font-weight: 500;
}

.info > small {
  color: var(--text-dim);
}

.types {
  display: flex;
  gap: 4px;
}

.stats {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.stats em {
  color: var(--text-dim);
  font-style: normal;
}

.ivs {
  display: flex;
  align-items: center;
}

.notes {
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--text-dim);
  font-size: 11.5px;
}

.notes li {
  display: flex;
  gap: 5px;
  align-items: flex-start;
}

.speed {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: 13px;
}

.sides {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
  gap: 18px;
}

h4 {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 6px;
  margin: 0 0 8px;
  font-size: 14px;
}

h4 small {
  color: var(--text-dim);
  font-weight: 500;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
}

th {
  padding: 4px 6px;
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
  text-align: left;
  text-transform: uppercase;
}

td {
  padding: 7px 6px;
  border-top: 1px solid var(--border);
  vertical-align: top;
}

tr.none td {
  color: var(--text-dim);
}

.move small,
.dmg small,
.crit small {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 2px;
  color: var(--text-dim);
}

.mvhead {
  display: flex;
  align-items: center;
  gap: 6px;
}

.dmg {
  min-width: 140px;
  font-variant-numeric: tabular-nums;
}

.hpbar {
  position: relative;
  height: 6px;
  margin-top: 5px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  overflow: hidden;
}

.hpbar i {
  position: absolute;
  top: 0;
  bottom: 0;
  border-radius: 3px;
  background: linear-gradient(90deg, #f59e0b, #ef4444);
}

.ko.good {
  color: #16a34a;
  font-weight: 700;
}

.crit {
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.reason {
  font-style: italic;
}

.mods {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  margin-top: 4px;
}

.mod {
  display: inline-block;
  margin: 2px 4px 2px 0;
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  font-size: 11px;
}
</style>
