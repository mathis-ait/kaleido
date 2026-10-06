<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { lists, saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { MARKS } from "../refdata";
import { apply, num } from "./edit";

const props = defineProps<{ p: SlotView }>();
const gen = computed(() => saveState.view!.generation);

/** Gen 7 : aucun → bleu → rouge ; avant : présent ou non. */
function toggleMark(i: number) {
  const marks = [...props.p.markings];
  marks[i] = gen.value >= 7 ? (marks[i] + 1) % 3 : marks[i] ? 0 : 1;
  apply({ markings: marks });
}

const pokerusState = computed(() => {
  const { pokerusStrain: s, pokerusDays: d } = props.p;
  if (!s) return "Jamais infecté";
  return d ? `Infecté (${d} jour${d > 1 ? "s" : ""} restant${d > 1 ? "s" : ""})` : "Guéri (effet permanent)";
});

/** Durée maximale de l'infection pour une souche : souche % 4 + 1 jours. */
const maxDays = (strain: number) => (strain % 4) + 1;
function setPokerus(kind: "none" | "infected" | "cured") {
  const strain = props.p.pokerusStrain || 1;
  if (kind === "none") apply({ pokerus: [0, 0] });
  else if (kind === "infected") apply({ pokerus: [strain, maxDays(strain)] });
  else apply({ pokerus: [strain, 0] });
}
function setStrain(strain: number) {
  apply({ pokerus: [strain, strain ? Math.min(props.p.pokerusDays, maxDays(strain)) : 0] });
}

// --- Concours.
const CONTEST = ["Sang-froid", "Beauté", "Grâce", "Intelligence", "Robustesse", "Lustre"];
const x = computed(() => props.p.extras);
const contest = ref([...x.value.contest]);
watch(x, (v) => (contest.value = [...v.contest]));
function commitContest(i: number) {
  const v = num(contest.value[i], 0, 255);
  if (v === null) return;
  const next = [...x.value.contest];
  next[i] = v;
  if (v !== x.value.contest[i]) apply({ extras: { contest: next } });
}

// --- Gen 4 : feuilles brillantes (5 feuilles + couronne) et humeur ; Gen 5 : renommée Pokéstar.
function toggleLeaf(bit: number) {
  apply({ extras: { shinyLeaf: (x.value.shinyLeaf ?? 0) ^ (1 << bit) } });
}
const leafOn = (bit: number) => ((x.value.shinyLeaf ?? 0) >> bit) & 1;
const mood = ref(x.value.walkingMood ?? 0);
const fame = ref(x.value.pokestarFame ?? 0);
const formArg = ref(x.value.formArgument ?? 0);
watch(x, (v) => {
  mood.value = v.walkingMood ?? 0;
  fame.value = v.pokestarFame ?? 0;
  formArg.value = v.formArgument ?? 0;
});
function commitByte(kind: "walkingMood" | "pokestarFame") {
  const v = kind === "walkingMood" ? num(mood.value, -127, 127) : num(fame.value, 0, 255);
  if (v !== null && v !== x.value[kind]) apply({ extras: { [kind]: v } });
}
function commitFormArg() {
  const v = num(formArg.value, 0, 0xffffffff);
  if (v !== null && v !== x.value.formArgument) apply({ extras: { formArgument: v } });
}

// --- Super Training (Gen 6/7) : 30 médailles normales puis 8 distribuées.
const st = computed(() => x.value.superTraining);
function toggleMedal(i: number) {
  const medals = [...(st.value?.medals ?? [])];
  medals[i] = !medals[i];
  apply({ extras: { medals } });
}
const medalCount = computed(() => st.value?.medals.filter(Boolean).length ?? 0);
</script>

<template>
  <div class="extras">
    <section>
      <h3 class="sv-section-title">Marquages <Tip term="markings" /></h3>
      <div class="marks">
        <button
          v-for="(m, i) in MARKS"
          :key="i"
          class="mark"
          :class="{ on: p.markings[i] === 1, red: p.markings[i] === 2 }"
          :aria-pressed="p.markings[i] > 0"
          @click="toggleMark(i)"
        >
          {{ m }}
        </button>
      </div>
      <p v-if="gen >= 7" class="sv-help">Clic : bleu, puis rouge, puis aucun.</p>
    </section>

    <section>
      <h3 class="sv-section-title">Pokérus <Tip term="pokerus" /></h3>
      <div class="sv-seg">
        <button :class="{ on: !p.pokerusStrain }" @click="setPokerus('none')">Jamais</button>
        <button :class="{ on: p.pokerusStrain && p.pokerusDays }" @click="setPokerus('infected')">Infecté</button>
        <button :class="{ on: p.pokerusStrain && !p.pokerusDays }" @click="setPokerus('cured')">Guéri</button>
      </div>
      <p class="sv-help">{{ pokerusState }}</p>
      <div class="pair">
        <label class="sv-field">
          <span class="sv-label">Souche <Tip term="pokerusStrain" /></span>
          <select class="sv-select" :value="p.pokerusStrain" @change="setStrain(Number(($event.target as HTMLSelectElement).value))">
            <option v-for="n in 16" :key="n" :value="n - 1">{{ n === 1 ? "Aucune" : `Souche ${n - 1}` }}</option>
          </select>
        </label>
        <label class="sv-field">
          <span class="sv-label">Jours restants</span>
          <select
            class="sv-select"
            :value="p.pokerusDays"
            :disabled="!p.pokerusStrain"
            @change="apply({ pokerus: [p.pokerusStrain, Number(($event.target as HTMLSelectElement).value)] })"
          >
            <option v-for="d in maxDays(p.pokerusStrain) + 1" :key="d" :value="d - 1">{{ d === 1 ? "0 (guéri)" : d - 1 }}</option>
            <option v-if="p.pokerusDays > maxDays(p.pokerusStrain)" :value="p.pokerusDays">{{ p.pokerusDays }} (impossible)</option>
          </select>
        </label>
      </div>
    </section>

    <section>
      <h3 class="sv-section-title">Concours <Tip term="contest" /></h3>
      <div class="contest">
        <label v-for="(label, i) in CONTEST" :key="label" class="sv-field">
          <span class="sv-label">{{ label }}</span>
          <input v-model="contest[i]" class="sv-input" type="number" min="0" max="255" @change="commitContest(i)" />
        </label>
      </div>
      <div class="sv-row">
        <button class="sv-btn" @click="apply({ extras: { contest: [255, 255, 255, 255, 255, 255] } })">Tout au maximum</button>
        <button class="sv-btn" @click="apply({ extras: { contest: [0, 0, 0, 0, 0, 0] } })">Remettre à 0</button>
      </div>
    </section>

    <section v-if="x.shinyLeaf !== null">
      <h3 class="sv-section-title">Feuilles brillantes <Tip term="shinyLeaf" /></h3>
      <div class="marks">
        <button
          v-for="b in 5"
          :key="b"
          class="mark leaf"
          :class="{ on: leafOn(b - 1) }"
          :aria-pressed="!!leafOn(b - 1)"
          :title="`Feuille ${b}`"
          @click="toggleLeaf(b - 1)"
        >
          🍃
        </button>
        <button class="mark leaf" :class="{ on: leafOn(5) }" :aria-pressed="!!leafOn(5)" title="Couronne" @click="toggleLeaf(5)">👑</button>
      </div>
      <label class="sv-field mood">
        <span class="sv-label">Humeur en promenade <Tip term="walkingMood" /></span>
        <input v-model="mood" class="sv-input" type="number" min="-127" max="127" @change="commitByte('walkingMood')" />
      </label>
    </section>

    <section v-if="x.nSparkle !== null">
      <h3 class="sv-section-title">Noir 2 et Blanc 2</h3>
      <label class="sv-switch">
        <input type="checkbox" :checked="!!x.nSparkle" @change="apply({ extras: { nSparkle: !x.nSparkle } })" />
        <span class="track" />
        Éclat de N <Tip term="nSparkle" />
      </label>
      <label class="sv-field mood">
        <span class="sv-label">Renommée Pokéstar <Tip term="pokestarFame" /></span>
        <input v-model="fame" class="sv-input" type="number" min="0" max="255" @change="commitByte('pokestarFame')" />
      </label>
    </section>

    <section v-if="x.formArgument !== null && (p.species === 676 || p.species === 720)">
      <h3 class="sv-section-title">Argument de forme <Tip term="formArgument" /></h3>
      <label class="sv-field">
        <span class="sv-label">{{ p.species === 676 ? "Jours avant la fin de la coupe" : "Jours avant de redevenir Enchaîné" }}</span>
        <input v-model="formArg" class="sv-input" type="number" min="0" @change="commitFormArg" />
      </label>
    </section>

    <section v-if="st" class="wide">
      <h3 class="sv-section-title">Super Training · {{ medalCount }}/{{ st.medals.length }} médailles <Tip term="superTraining" /></h3>
      <div class="sv-row">
        <button class="sv-btn" @click="apply({ extras: { medals: st.medals.map((_, i) => i < 30), secretUnlocked: true } })">Toutes les médailles normales</button>
        <button class="sv-btn" @click="apply({ extras: { medals: st.medals.map(() => false) } })">Aucune</button>
      </div>
      <div class="medals">
        <label v-for="(on, i) in st.medals" :key="i" class="medal" :class="{ on, dist: i >= 30 }">
          <input type="checkbox" :checked="on" @change="toggleMedal(i)" />
          <span>{{ lists.superTraining[i] || `Médaille ${i + 1}` }}</span>
        </label>
      </div>
      <div class="sv-row flags">
        <label class="sv-switch">
          <input type="checkbox" :checked="st.secretUnlocked" @change="apply({ extras: { secretUnlocked: !st.secretUnlocked } })" />
          <span class="track" />
          Entraînements secrets débloqués
        </label>
        <label class="sv-switch">
          <input type="checkbox" :checked="st.supremelyTrained" @change="apply({ extras: { supremelyTrained: !st.supremelyTrained } })" />
          <span class="track" />
          Entraînement suprême terminé
        </label>
      </div>
    </section>

    <section>
      <h3 class="sv-section-title">Œuf <Tip term="egg" /></h3>
      <label class="sv-switch">
        <input type="checkbox" :checked="p.isEgg" @change="apply({ isEgg: !p.isEgg })" />
        <span class="track" />
        {{ p.isEgg ? "Encore dans son œuf" : "Éclos" }}
      </label>
    </section>

    <section>
      <h3 class="sv-section-title">Données techniques <Tip term="checksum" /></h3>
      <dl>
        <dt>Somme de contrôle</dt>
        <dd :class="p.checksumValid ? 'ok' : 'bad'">{{ p.checksumValid ? "Valide" : "Invalide" }}</dd>
        <dt>Format</dt>
        <dd>PK{{ gen }}</dd>
      </dl>
    </section>
  </div>
</template>

<style scoped>
.extras {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 28px;
}

.marks {
  display: flex;
  gap: 8px;
}

.mark {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border: 1.5px solid var(--border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: color-mix(in srgb, var(--text) 40%, transparent);
  font-size: 20px;
}

.mark.on {
  border-color: #6fb4ff;
  color: #6fb4ff;
  background: color-mix(in srgb, #6fb4ff 18%, transparent);
}

.mark.red {
  border-color: #ff7a8a;
  color: #ff7a8a;
  background: color-mix(in srgb, #ff7a8a 18%, transparent);
}

section .sv-help {
  margin-top: 8px;
}

.wide {
  grid-column: 1 / -1;
}

.pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-top: 10px;
}

.contest {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px 10px;
  margin-bottom: 10px;
}

.mark.leaf {
  filter: grayscale(1);
  opacity: 0.5;
}

.mark.leaf.on {
  border-color: var(--ok);
  background: color-mix(in srgb, var(--ok) 18%, transparent);
  filter: none;
  opacity: 1;
}

.mood {
  max-width: 220px;
  margin-top: 12px;
}

.medals {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 4px 12px;
  margin: 12px 0;
}

.medal {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 8px;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
}

.medal.on {
  background: color-mix(in srgb, #ffc94d 14%, transparent);
  color: var(--text);
}

.medal.dist span::after {
  content: " · distribuée";
  color: var(--text-dim);
  font-size: 11px;
}

.flags {
  gap: 18px;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 14px;
  margin: 0;
  font-size: 14px;
}

dt {
  color: var(--text-dim);
}

dd {
  margin: 0;
  font-weight: 600;
}

.ok {
  color: var(--ok);
}

.bad {
  color: var(--danger);
}

.dim {
  color: var(--text-dim);
  font-weight: 400;
}
</style>
