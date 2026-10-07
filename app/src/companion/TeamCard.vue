<script setup lang="ts">
import { computed } from "vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import HpBar from "./HpBar.vue";
import { monName, type LiveMon, type StatusCondition } from "./store";

/** Un Pokémon de l'équipe, tel que le jeu l'affiche : PV, statut, objet, attaques. */
const props = defineProps<{ mon: LiveMon; open?: boolean }>();
defineEmits<{ toggle: [] }>();

const STATUS: Record<Exclude<StatusCondition, "none">, string> = {
  sleep: "Sommeil",
  poison: "Poison",
  badPoison: "Poison grave",
  burn: "Brûlure",
  freeze: "Gel",
  paralysis: "Paralysie",
};

const ko = computed(() => !props.mon.isEgg && props.mon.maxHp > 0 && props.mon.hp === 0);
</script>

<template>
  <li class="mon sv-card" :class="{ ko, open }">
    <button type="button" class="head" :aria-expanded="open" @click="$emit('toggle')">
      <Sprite :id="mon.isEgg ? 0 : mon.species" :form="mon.form" :shiny="mon.shiny" :gender="mon.gender" variant="model" :size="56" />
      <span class="who">
        <span class="name">
          <strong>{{ monName(mon) }}</strong>
          <span v-if="!mon.isEgg && mon.gender !== 'genderless'" class="sex" :class="mon.gender">{{ mon.gender === "male" ? "♂" : "♀" }}</span>
          <span v-if="mon.shiny" class="sv-chip shiny"><Icon name="sparkle" :size="11" /> Chromatique</span>
        </span>
        <small v-if="!mon.isEgg">
          N. {{ mon.level }}<template v-if="mon.isNicknamed && mon.nickname !== mon.speciesName"> · {{ mon.speciesName }}</template>
        </small>
        <HpBar v-if="!mon.isEgg && mon.maxHp" :hp="mon.hp" :max="mon.maxHp" />
      </span>
      <span class="flags">
        <span v-if="ko" class="sv-chip danger">K.O.</span>
        <span v-else-if="mon.status !== 'none'" class="sv-chip warn">{{ STATUS[mon.status] }}</span>
      </span>
    </button>
    <div v-if="open && !mon.isEgg" class="more">
      <dl class="sv-dl">
        <dt>PV <Tip term="companion.hp" /></dt>
        <dd>{{ mon.hp }} / {{ mon.maxHp }}</dd>
        <template v-if="mon.status !== 'none'">
          <dt>Statut <Tip term="companion.status" /></dt>
          <dd>{{ STATUS[mon.status] }}</dd>
        </template>
        <dt>Objet tenu <Tip term="heldItem" /></dt>
        <dd>{{ mon.itemName ?? "Aucun" }}</dd>
        <dt>Talent <Tip term="ability" /></dt>
        <dd>{{ mon.abilityName }}</dd>
        <dt>Nature <Tip term="nature" /></dt>
        <dd>{{ mon.natureName }}</dd>
        <template v-if="mon.metLocationName">
          <dt>Lieu de rencontre <Tip term="metLocation" /></dt>
          <dd>{{ mon.metLocationName }}</dd>
        </template>
      </dl>
      <ul class="sv-moves">
        <li v-for="m in mon.moveNames" :key="m">{{ m }}</li>
      </ul>
    </div>
  </li>
</template>

<style scoped>
.mon {
  list-style: none;
  overflow: hidden;
}

.mon.ko {
  border-color: var(--danger);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  width: 100%;
  padding: var(--sp-2) var(--sp-3);
  border: none;
  background: none;
  border-radius: inherit;
  color: inherit;
  text-align: left;
  transition: background-color 0.15s;
}

.head:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

/* La carte rogne ce qui dépasse : l'anneau de focus est tracé à l'intérieur. */
.head:focus-visible {
  outline-offset: -2px;
}

.open .head {
  border-bottom-right-radius: 0;
  border-bottom-left-radius: 0;
}

.ko :deep(.sprite) {
  filter: grayscale(1);
  opacity: 0.6;
}

.who {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.name {
  display: flex;
  align-items: center;
  gap: var(--sp-1);
  min-width: 0;
}

.name strong {
  overflow: hidden;
  font-size: var(--fs-base);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.sex {
  font-weight: 700;
}

.sex.male {
  color: var(--male);
}

.sex.female {
  color: var(--female);
}

small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.flags {
  display: flex;
  flex-shrink: 0;
}

.more {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: 0 var(--sp-3) var(--sp-3);
}
</style>
