<script setup lang="ts">
import { computed } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import TypeBadge from "../../components/TypeBadge.vue";
import { battle, pct, typeTag, type Combatant, type Duel, type MoveLine } from "../../battle";
import { lists } from "../../saveStore";
import { natureLabel, STAT_SHORT } from "../refdata";

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
          <small class="meta">
            <span>{{ ability(c) }}<Tip term="ability" /></span> ·
            <span>{{ item(c) }}<Tip term="heldItem" /></span> ·
            <span>{{ natureLabel(c.nature) }}<Tip term="nature" /></span>
          </small>
          <div class="stats">
            <span v-for="(s, i) in c.stats" :key="i"><em>{{ STAT_SHORT[i] }}</em> {{ s }}</span>
            <Tip term="statShort" />
          </div>
          <small v-if="k === 1" class="ivs">
            IV {{ c.ivs[0] }} partout, 0 EV <Tip term="battle.trainerIvs" />
          </small>
          <ul v-if="c.notes.length" class="notes">
            <li v-for="n in c.notes" :key="n"><Icon name="info" :size="12" /> {{ n }}</li>
          </ul>
        </div>
      </div>
    </header>
    <p class="speed"><Icon name="clock" :size="14" /> {{ speedText }} <Tip term="battle.speed" /> <Tip term="battle.priority" /></p>

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
              <th>Dégâts <Tip term="battle.random" /></th>
              <th>K.O. <Tip term="battle.ko" /></th>
              <th>Critique <Tip term="battle.crit" /></th>
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
                  {{ CATEGORY[m.category] }} <Tip term="battle.category" />
                  <template v-if="m.power"> · Puissance {{ m.power }}</template>
                  <template v-if="m.hits > 1"> · {{ m.hits }} coups <Tip term="battle.multihit" /></template>
                  <template v-if="m.priority"> · Priorité {{ m.priority > 0 ? "+" : "" }}{{ m.priority }}</template>
                  <template v-if="m.damage.kind === 'rolls'"> · {{ effLabel(m.effectiveness) }} <Tip term="battle.effectiveness" /></template>
                </small>
                <div v-if="m.modifiers.length" class="mods">
                  <span v-for="x in m.modifiers" :key="x" class="sv-chip">{{ x }}</span>
                  <Tip v-if="m.modifiers.some((x) => x.startsWith('STAB'))" term="battle.stab" />
                  <Tip term="battle.modifiers" />
                </div>
              </td>
              <td v-if="m.damage.kind === 'none'" colspan="3" class="reason">{{ m.damage.reason }}</td>
              <template v-else>
                <td class="dmg">
                  <strong>{{ pct(m.minPercent) }} – {{ pct(m.maxPercent) }} %</strong>
                  <small>
                    {{ m.min }} – {{ m.max }} PV
                    <template v-if="m.damage.kind === 'fixed'"> (fixes <Tip term="battle.fixed" />)</template>
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
  gap: var(--sp-3);
  padding: var(--sp-4);
}

.vs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--sp-4);
}

.fighter {
  display: flex;
  gap: var(--sp-3);
  padding: var(--sp-3);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--ok) 8%, transparent);
}

.fighter.foe {
  background: color-mix(in srgb, var(--danger) 8%, transparent);
}

.info {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  min-width: 0;
}

.info strong small {
  color: var(--text-dim);
  font-weight: 500;
}

.info > small {
  color: var(--text-dim);
}

.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1);
}

.meta span {
  display: inline-flex;
  align-items: center;
}

.types {
  display: flex;
  gap: var(--sp-1);
}

.stats {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1) var(--sp-3);
  font-size: var(--fs-sm);
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
  font-size: var(--fs-xs);
}

.notes li {
  display: flex;
  gap: 5px;
  align-items: flex-start;
}

.speed {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  margin: 0;
  font-size: var(--fs-md);
}

.sides {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
  gap: var(--sp-5);
}

h4 {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--sp-2);
  margin: 0 0 var(--sp-2);
  font-size: var(--fs-base);
}

h4 small {
  color: var(--text-dim);
  font-weight: 500;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-sm);
}

th {
  padding: 4px 6px;
  color: var(--text-dim);
  font-size: var(--fs-xs);
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
  gap: var(--sp-2);
}

.dmg {
  min-width: 140px;
  font-variant-numeric: tabular-nums;
}

.hpbar {
  position: relative;
  height: 6px;
  margin-top: 5px;
  border-radius: var(--radius-xs);
  background: color-mix(in srgb, var(--text) 12%, transparent);
  overflow: hidden;
}

.hpbar i {
  position: absolute;
  top: 0;
  bottom: 0;
  border-radius: var(--radius-xs);
  background: var(--warn);
}

.ko.good {
  color: var(--ok);
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
  gap: var(--sp-1);
  margin-top: var(--sp-1);
}
</style>
