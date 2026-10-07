<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Combo from "../../components/Combo.vue";
import Tip from "../../components/Tip.vue";
import { lists, saveState } from "../../saveStore";
import type { Gender, SlotView } from "../../types";
import { LANGUAGES, NATURES, natureEffect, STAT_SHORT } from "../refdata";
import { apply, num } from "./edit";

const props = defineProps<{ p: SlotView }>();
const view = computed(() => saveState.view!);
const gen = computed(() => view.value.generation);

const nickname = ref(props.p.nickname);
const level = ref(props.p.level);
const exp = ref(props.p.exp);
const friendship = ref(props.p.friendship);
watch(
  () => props.p,
  (p) => {
    nickname.value = p.nickname;
    level.value = p.level;
    exp.value = p.exp;
    friendship.value = p.friendship;
  },
);

const species = computed({ get: () => props.p.species, set: (v) => v !== props.p.species && apply({ species: v }) });
const item = computed({ get: () => props.p.heldItem, set: (v) => v !== props.p.heldItem && apply({ heldItem: v }) });
const ability = computed({ get: () => props.p.ability, set: (v) => v !== props.p.ability && apply({ ability: v }) });
const speciesOptions = computed(() => lists.species.map((o) => ({ ...o, hint: `n°${o.value}` })));
const abilityOptions = computed(() => lists.abilities);
const itemOptions = computed(() => lists.items);

/** Emplacements de talent proposés par l'espèce : 1, 2 et caché (Gen 5+). */
const slots = computed(() => {
  const d = props.p.speciesData;
  const out: { n: number; label: string; id: number }[] = [];
  if (!d) return out;
  out.push({ n: 1, label: d.abilityNames[0] || "Talent 1", id: d.abilities[0] });
  if (d.abilities[1] && (d.abilities[1] !== d.abilities[0] || props.p.abilityNumber === 2)) out.push({ n: 2, label: d.abilityNames[1], id: d.abilities[1] });
  if (gen.value >= 5 && d.abilities[2]) out.push({ n: 4, label: `${d.abilityNames[2]} (caché)`, id: d.abilities[2] });
  return out;
});

function setSlot(s: { n: number; id: number }) {
  apply({ abilityNumber: s.n, ability: s.id });
}

const genderLocked = computed(() => {
  const r = props.p.speciesData?.genderRatio;
  return r === 0 ? "male" : r === 254 ? "female" : r === 255 ? "genderless" : null;
});

function setGender(g: Gender) {
  if (g !== props.p.gender) apply({ gender: g });
}

function commitNickname() {
  const n = nickname.value.trim();
  if (n && n !== props.p.nickname) apply({ nickname: n });
  else nickname.value = props.p.nickname;
}

function commitNumber(field: "level" | "exp" | "friendship", v: unknown, min: number, max: number) {
  const n = num(v, min, max);
  if (n === null) return;
  if (field === "level" && n !== props.p.level) apply({ level: n });
  if (field === "exp" && n !== props.p.exp) apply({ exp: n });
  if (field === "friendship" && n !== props.p.friendship) apply({ friendship: n });
}

const natureHint = (n: number) => {
  const e = natureEffect(n);
  return e ? `+${STAT_SHORT[e.up]} −${STAT_SHORT[e.down]}` : "neutre";
};
const languages = computed(() => LANGUAGES.filter((l) => !l.since || gen.value >= l.since));
</script>

<template>
  <div class="sv-grid">
    <div class="sv-field">
      <span class="sv-label">Espèce <Tip term="species" /></span>
      <Combo v-model="species" :options="speciesOptions" sprites />
    </div>
    <div v-if="(p.speciesData?.formNames.length ?? 0) > 1" class="sv-field">
      <span class="sv-label">Forme <Tip term="form" /></span>
      <select class="sv-select" :value="p.form" @change="apply({ form: Number(($event.target as HTMLSelectElement).value) })">
        <option v-for="(f, i) in p.speciesData!.formNames" :key="i" :value="i">{{ f || `Forme ${i}` }}</option>
      </select>
    </div>
    <div class="sv-field">
      <span class="sv-label">Surnom <Tip term="nickname" /></span>
      <input v-model="nickname" class="sv-input" :maxlength="view.nicknameMax" @change="commitNickname" @keydown.enter="commitNickname" />
    </div>
    <div class="sv-field">
      <span class="sv-label">Surnommé</span>
      <label class="sv-switch">
        <input type="checkbox" :checked="p.isNicknamed" @change="apply(p.isNicknamed ? { isNicknamed: false } : { nickname: nickname })" />
        <span class="track" />
        {{ p.isNicknamed ? "Surnom personnalisé" : "Nom de l'espèce" }}
      </label>
    </div>

    <div class="sv-field">
      <span class="sv-label">Niveau <Tip term="level" /></span>
      <input
        v-model="level"
        class="sv-input"
        type="number"
        min="1"
        max="100"
        @change="commitNumber('level', level, 1, 100)"
        @keydown.enter="commitNumber('level', level, 1, 100)"
      />
    </div>
    <div class="sv-field">
      <span class="sv-label">Expérience</span>
      <input v-model="exp" class="sv-input" type="number" min="0" max="2000000" @change="commitNumber('exp', exp, 0, 2_000_000)" />
    </div>
    <div class="sv-field">
      <span class="sv-label">Nature <Tip term="nature" /></span>
      <select class="sv-select" :value="p.nature" @change="apply({ nature: Number(($event.target as HTMLSelectElement).value) })">
        <option v-for="(n, i) in NATURES" :key="i" :value="i">{{ n }} ({{ natureHint(i) }})</option>
      </select>
      <p v-if="gen <= 4" class="sv-help">En Gen {{ gen }} la nature vient du PID : il sera recalculé.</p>
    </div>
    <div class="sv-field gender">
      <span class="sv-label">Sexe <Tip term="gender" /></span>
      <div class="sv-seg">
        <button :class="{ on: p.gender === 'male' }" :disabled="!!genderLocked && genderLocked !== 'male'" @click="setGender('male')">♂ Mâle</button>
        <button :class="{ on: p.gender === 'female' }" :disabled="!!genderLocked && genderLocked !== 'female'" @click="setGender('female')">♀ Femelle</button>
        <button :class="{ on: p.gender === 'genderless' }" :disabled="genderLocked !== 'genderless'" @click="setGender('genderless')">Asexué</button>
      </div>
    </div>

    <div class="sv-field wide">
      <span class="sv-label">Talent <Tip term="ability" /> <Tip v-if="gen >= 5" term="hiddenAbility" /></span>
      <div class="sv-row">
        <div v-if="slots.length" class="sv-seg">
          <button v-for="s in slots" :key="s.n" :class="{ on: p.abilityNumber === s.n }" @click="setSlot(s)">{{ s.label }}</button>
        </div>
        <div class="other">
          <Combo v-model="ability" :options="abilityOptions" placeholder="Autre talent…" />
        </div>
      </div>
      <p v-if="slots.length && !slots.some((s) => s.id === p.ability)" class="sv-help warn">
        « {{ p.abilityName }} » n'est pas un talent de {{ p.speciesName }} dans ce jeu.
      </p>
    </div>

    <div class="sv-field">
      <span class="sv-label">Objet tenu <Tip term="heldItem" /></span>
      <Combo v-model="item" :options="itemOptions" none-label="(Aucun)" />
    </div>
    <div class="sv-field">
      <span class="sv-label">Langue <Tip term="language" /></span>
      <select class="sv-select" :value="p.language" @change="apply({ language: Number(($event.target as HTMLSelectElement).value) })">
        <option v-for="l in languages" :key="l.id" :value="l.id">{{ l.code }} ({{ l.name }})</option>
      </select>
    </div>
    <div class="sv-field">
      <span class="sv-label">{{ p.isEgg ? "Cycles d'éclosion" : "Bonheur" }} <Tip :term="p.isEgg ? 'egg' : 'friendship'" /></span>
      <div class="sv-row nowrap">
        <input v-model="friendship" class="sv-input" type="number" min="0" max="255" @change="commitNumber('friendship', friendship, 0, 255)" />
        <button v-if="!p.isEgg" class="sv-btn" title="Bonheur au maximum" @click="apply({ friendship: 255 })">Max</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Les trois choix de sexe ne tiennent pas dans une colonne de 220 px : deux colonnes. */
.gender {
  grid-column: span 2;
}

.other {
  flex: 1;
  min-width: 200px;
}

.nowrap {
  flex-wrap: nowrap;
}

.warn {
  color: var(--warn);
}
</style>
