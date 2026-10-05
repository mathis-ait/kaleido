<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Sprite from "./Sprite.vue";
import { patchPokemon } from "../saveStore";
import type { SlotView } from "../types";

/** Fenêtre de modification d'un Pokémon de la sauvegarde. */
const props = defineProps<{ pokemon: SlotView }>();
const emit = defineEmits<{ close: [] }>();

const STAT_LABELS = ["PV", "Att", "Déf", "Atq Spé", "Déf Spé", "Vit"];
const lists = ref<{ moves: string[]; items: string[] }>({ moves: [], items: [] });
const saving = ref(false);

const form = reactive({
  nickname: props.pokemon.nickname,
  level: props.pokemon.level,
  item: props.pokemon.itemName ?? "",
  moves: [0, 1, 2, 3].map((i) => props.pokemon.moveNames[i] ?? ""),
  ivs: [...props.pokemon.ivs],
  evs: [...props.pokemon.evs],
  friendship: props.pokemon.friendship,
});

onMounted(async () => {
  lists.value = await invoke("name_lists");
});

/** Nom → identifiant (premier index correspondant), 0 si vide ou inconnu. */
const idOf = (list: string[], name: string) => (name.trim() ? Math.max(0, list.findIndex((n, i) => i > 0 && n === name.trim())) : 0);
const unknownMoves = computed(() => form.moves.filter((m) => m.trim() && idOf(lists.value.moves, m) === 0));
const unknownItem = computed(() => form.item.trim() !== "" && idOf(lists.value.items, form.item) === 0);
const evTotal = computed(() => form.evs.reduce((a, b) => a + Number(b || 0), 0));
const valid = computed(() => !unknownMoves.value.length && !unknownItem.value && evTotal.value <= 510);

async function apply() {
  saving.value = true;
  const ok = await patchPokemon(props.pokemon.slot, {
    nickname: form.nickname,
    level: Number(form.level),
    heldItem: idOf(lists.value.items, form.item),
    moves: form.moves.map((m) => idOf(lists.value.moves, m)),
    ivs: form.ivs.map(Number),
    evs: form.evs.map(Number),
    friendship: Number(form.friendship),
  });
  saving.value = false;
  if (ok) emit("close");
}
</script>

<template>
  <div class="modal" @click.self="emit('close')" @keydown.esc="emit('close')">
    <form class="panel editor" @submit.prevent="apply">
      <header>
        <Sprite :id="pokemon.species" :shiny="pokemon.shiny" :size="80" />
        <div>
          <h2>Modifier {{ pokemon.nickname || pokemon.speciesName }}</h2>
          <small>{{ pokemon.speciesName }} · {{ pokemon.natureName }} · {{ pokemon.abilityName }}</small>
        </div>
        <button type="button" class="close" aria-label="Fermer" @click="emit('close')">×</button>
      </header>

      <div class="fields">
        <label>Surnom <input v-model="form.nickname" class="input" maxlength="12" /></label>
        <label>Niveau <input v-model.number="form.level" class="input" type="number" min="1" max="100" /></label>
        <label>Bonheur <input v-model.number="form.friendship" class="input" type="number" min="0" max="255" /></label>
        <label class="wide">
          Objet tenu
          <input v-model="form.item" class="input" list="kaleido-items" placeholder="Aucun" :class="{ invalid: unknownItem }" />
        </label>
      </div>

      <h3>Attaques</h3>
      <div class="moves">
        <input
          v-for="(_, i) in form.moves"
          :key="i"
          v-model="form.moves[i]"
          class="input"
          list="kaleido-moves"
          :placeholder="`Attaque ${i + 1}`"
          :class="{ invalid: unknownMoves.includes(form.moves[i]) }"
        />
      </div>

      <h3>IV et EV <small :class="{ over: evTotal > 510 }">EV : {{ evTotal }} / 510</small></h3>
      <div class="stats">
        <span></span>
        <span class="col">IV (0-31)</span>
        <span class="col">EV (0-252)</span>
        <template v-for="(label, i) in STAT_LABELS" :key="label">
          <span>{{ label }}</span>
          <input v-model.number="form.ivs[i]" class="input" type="number" min="0" max="31" :aria-label="`IV ${label}`" />
          <input v-model.number="form.evs[i]" class="input" type="number" min="0" max="252" :aria-label="`EV ${label}`" />
        </template>
      </div>
      <div class="quick">
        <button type="button" class="btn" @click="form.ivs = [31, 31, 31, 31, 31, 31]">IV au max</button>
        <button type="button" class="btn" @click="form.evs = [0, 0, 0, 0, 0, 0]">Remettre les EV à zéro</button>
      </div>

      <datalist id="kaleido-moves">
        <option v-for="(m, i) in lists.moves.slice(1)" :key="i" :value="m" />
      </datalist>
      <datalist id="kaleido-items">
        <option v-for="(it, i) in lists.items.slice(1).filter((n) => n && n !== '???')" :key="i" :value="it" />
      </datalist>

      <footer>
        <span v-if="!valid" class="error-text">Corrige les champs en rouge.</span>
        <button type="button" class="btn" @click="emit('close')">Annuler</button>
        <button type="submit" class="btn btn-primary" :disabled="!valid || saving">Appliquer</button>
      </footer>
    </form>
  </div>
</template>

<style scoped>
.modal {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: grid;
  place-items: center;
  padding: 32px;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
}

.editor {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: min(640px, 100%);
  max-height: 100%;
  overflow-y: auto;
  padding: 22px;
  background: var(--surface);
}

header {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
}

header h2 {
  font-size: 20px;
}

header small,
h3 small {
  color: var(--text-dim);
  font-weight: 500;
}

.close {
  position: absolute;
  top: 0;
  right: 0;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 50%;
  background: var(--panel-hover);
  font-size: 20px;
}

h3 {
  display: flex;
  justify-content: space-between;
  font-size: 14px;
}

.over {
  color: var(--danger) !important;
}

.fields {
  display: grid;
  grid-template-columns: 2fr 1fr 1fr;
  gap: 10px;
}

.fields .wide {
  grid-column: 1 / -1;
}

label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}

.input {
  min-width: 0;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: var(--text);
  font: inherit;
  font-size: 14px;
  outline: none;
}

.input:focus {
  border-color: var(--accent-2);
}

.input.invalid {
  border-color: var(--danger);
}

.moves {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.stats {
  display: grid;
  grid-template-columns: auto 1fr 1fr;
  align-items: center;
  gap: 6px 10px;
  font-size: 13px;
}

.col {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}

.quick {
  display: flex;
  gap: 8px;
}

footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.error-text {
  margin-right: auto;
  color: var(--danger);
  font-size: 13px;
}
</style>
