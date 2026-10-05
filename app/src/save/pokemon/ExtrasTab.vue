<script setup lang="ts">
import { computed } from "vue";
import Tip from "../../components/Tip.vue";
import { saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { MARKS } from "../refdata";
import { apply } from "./edit";

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

function setPokerus(kind: "none" | "infected" | "cured") {
  if (kind === "none") apply({ pokerus: [0, 0] });
  else if (kind === "infected") apply({ pokerus: [props.p.pokerusStrain || 1, 4] });
  else apply({ pokerus: [props.p.pokerusStrain || 1, 0] });
}
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
        <dt>Rubans, souvenirs, concours</dt>
        <dd class="dim">Pas encore modifiables dans Kaleido</dd>
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
  color: #8ff0b5;
}

.bad {
  color: var(--danger);
}

.dim {
  color: var(--text-dim);
  font-weight: 400;
}
</style>
