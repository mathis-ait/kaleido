<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import Combo, { type ComboOption } from "../components/Combo.vue";
import Dialog from "../components/Dialog.vue";
import Segmented from "../components/Segmented.vue";
import Toggle from "../components/Toggle.vue";
import { BALLS, dex, SYSTEM_NAMES, today } from "./data";
import { removeManual, saveManual } from "./store";
import type { ManualEntry } from "./types";
import { livedexUi } from "./ui";

/** Jeux proposés d'abord : ceux que Kaleido ne lit pas (Switch, services). */
const FIRST_SYSTEMS = ["switch", "mobile"];

const open = computed({
  get: () => livedexUi.entry !== null,
  set: (v: boolean) => {
    if (!v) livedexUi.entry = null;
  },
});

const blank = () => ({
  id: undefined as string | undefined,
  species: 0,
  form: 0,
  gender: "n" as "m" | "f" | "n",
  shiny: false,
  game: "scarlet",
  method: "",
  location: "",
  ball: 4,
  level: null as number | null,
  date: today(),
  nickname: "",
  ot: "",
  notes: "",
});
const f = reactive(blank());

watch(
  () => livedexUi.entry,
  (e) => {
    if (!e) return;
    Object.assign(f, blank(), {
      ...e,
      method: e.method ?? "",
      location: e.location ?? "",
      nickname: e.nickname ?? "",
      ot: e.ot ?? "",
      notes: e.notes ?? "",
      level: e.level ?? null,
      date: e.date ?? today(),
      ball: e.ball ?? 4,
      gender: e.gender ?? "n",
      game: e.game ?? "scarlet",
    });
    const s = dex.value?.species(f.species);
    if (s && e.gender === undefined) f.gender = s.genderRate === -1 ? "n" : s.genderRate === 8 ? "f" : "m";
  },
);

const speciesOptions = computed<ComboOption[]>(() => dex.value?.speciesList.map((s) => ({ value: s.id, label: s.name, hint: `n° ${s.id}`, sprite: s.id })) ?? []);
const species = computed(() => dex.value?.species(f.species));
const forms = computed(() => species.value?.forms.filter((x) => x.cat !== "hidden") ?? []);
watch(
  () => f.species,
  () => {
    if (!forms.value.some((x) => x.f === f.form)) f.form = 0;
  },
);

const gameGroups = computed(() => {
  const games = dex.value?.games ?? [];
  const systems = [...new Set(games.map((g) => g.system))].sort((a, b) => Number(FIRST_SYSTEMS.includes(b)) - Number(FIRST_SYSTEMS.includes(a)));
  return systems.map((sys) => ({ name: SYSTEM_NAMES[sys], games: games.filter((g) => g.system === sys) }));
});

const genderOptions = computed(() => {
  const rate = species.value?.genderRate ?? 4;
  return [
    { value: "m" as const, label: "♂ Mâle", disabled: rate === -1 || rate === 8 },
    { value: "f" as const, label: "♀ Femelle", disabled: rate === -1 || rate === 0 },
    { value: "n" as const, label: "Asexué", disabled: rate !== -1 },
  ];
});

const valid = computed(() => f.species > 0 && !!f.game);

function save() {
  if (!valid.value) return;
  const entry: Omit<ManualEntry, "id" | "createdAt"> & { id?: string } = {
    id: f.id,
    species: f.species,
    form: f.form,
    gender: f.gender,
    shiny: f.shiny,
    game: f.game,
    ball: f.ball || undefined,
    level: f.level ?? undefined,
    date: f.date || undefined,
    method: f.method.trim() || undefined,
    location: f.location.trim() || undefined,
    nickname: f.nickname.trim() || undefined,
    ot: f.ot.trim() || undefined,
    notes: f.notes.trim() || undefined,
  };
  saveManual(entry);
  open.value = false;
}

function remove() {
  if (f.id) removeManual([f.id]);
  open.value = false;
}
</script>

<template>
  <Dialog v-model="open" :title="f.id ? 'Modifier le Pokémon noté' : 'Noter un Pokémon'" term="livedex.manual" icon="pencil" :width="640">
    <div class="sv-grid form">
      <label class="sv-field wide">
        <span class="sv-label">Pokémon</span>
        <Combo v-model="f.species" :options="speciesOptions" sprites placeholder="Choisir un Pokémon…" />
      </label>
      <label v-if="forms.length > 1" class="sv-field wide">
        <span class="sv-label">Forme</span>
        <select v-model="f.form" class="sv-select">
          <option v-for="x in forms" :key="x.f" :value="x.f">{{ x.name || "Normale" }}</option>
        </select>
      </label>
      <div class="sv-field wide">
        <span class="sv-label">Sexe</span>
        <Segmented v-model="f.gender" :options="genderOptions" label="Sexe" />
      </div>
      <div class="sv-field wide">
        <Toggle v-model="f.shiny" label="Chromatique" term="shiny" />
      </div>
      <label class="sv-field wide">
        <span class="sv-label">Jeu</span>
        <select v-model="f.game" class="sv-select">
          <optgroup v-for="g in gameGroups" :key="g.name" :label="g.name">
            <option v-for="x in g.games" :key="x.id" :value="x.id">{{ x.name }}</option>
          </optgroup>
        </select>
      </label>
      <label class="sv-field">
        <span class="sv-label">Méthode</span>
        <input v-model="f.method" class="sv-input" placeholder="Hautes herbes, raid, don…" />
      </label>
      <label class="sv-field">
        <span class="sv-label">Lieu</span>
        <input v-model="f.location" class="sv-input" />
      </label>
      <label class="sv-field">
        <span class="sv-label">Ball</span>
        <select v-model="f.ball" class="sv-select">
          <option v-for="(b, i) in BALLS" :key="i" :value="i" :hidden="!i">{{ b }}</option>
        </select>
      </label>
      <label class="sv-field">
        <span class="sv-label">Niveau</span>
        <input v-model.number="f.level" class="sv-input" type="number" min="1" max="100" />
      </label>
      <label class="sv-field">
        <span class="sv-label">Date</span>
        <input v-model="f.date" class="sv-input" type="date" />
      </label>
      <label class="sv-field">
        <span class="sv-label">Surnom</span>
        <input v-model="f.nickname" class="sv-input" maxlength="24" />
      </label>
      <label class="sv-field">
        <span class="sv-label">Dresseur d'origine</span>
        <input v-model="f.ot" class="sv-input" maxlength="24" />
      </label>
      <label class="sv-field wide">
        <span class="sv-label">Notes</span>
        <textarea v-model="f.notes" class="sv-input" rows="2" />
      </label>
    </div>

    <template #foot>
      <button v-if="f.id" type="button" class="sv-btn danger" @click="remove">Supprimer</button>
      <span class="spacer" />
      <button type="button" class="sv-btn" @click="open = false">Annuler</button>
      <button type="button" class="sv-btn solid" :disabled="!valid" @click="save">Enregistrer</button>
    </template>
  </Dialog>
</template>

<style scoped>
.form {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.spacer {
  flex: 1;
}

textarea {
  resize: vertical;
}
</style>
