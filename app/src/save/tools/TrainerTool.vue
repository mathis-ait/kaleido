<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { saveState, setTrainer } from "../../saveStore";
import type { Gender, TrainerPatch } from "../../types";
import { useShell } from "../shell";

const view = computed(() => saveState.view!);
const t = computed(() => view.value.trainer);

const draft = reactive({ name: "", tid: 0, sid: 0, gender: "male" as Gender, money: 0, hours: 0, minutes: 0, seconds: 0 });
function reset() {
  Object.assign(draft, {
    name: t.value.name,
    tid: t.value.tid,
    sid: t.value.sid,
    gender: t.value.gender,
    money: t.value.money,
    ...t.value.playTime,
  });
}
reset();
watch(t, reset);

const changes = computed<TrainerPatch>(() => {
  const p: TrainerPatch = {};
  const cur = t.value;
  if (draft.name.trim() !== cur.name) p.name = draft.name.trim();
  if (Number(draft.tid) !== cur.tid) p.tid = Number(draft.tid);
  if (Number(draft.sid) !== cur.sid) p.sid = Number(draft.sid);
  if (draft.gender !== cur.gender) p.gender = draft.gender;
  if (Number(draft.money) !== cur.money) p.money = Number(draft.money);
  if (Number(draft.hours) !== cur.playTime.hours) p.hours = Number(draft.hours);
  if (Number(draft.minutes) !== cur.playTime.minutes) p.minutes = Number(draft.minutes);
  if (Number(draft.seconds) !== cur.playTime.seconds) p.seconds = Number(draft.seconds);
  return p;
});
const dirty = computed(() => Object.keys(changes.value).length > 0);
const valid = computed(
  () =>
    draft.name.trim().length > 0 &&
    [draft.tid, draft.sid].every((v) => Number(v) >= 0 && Number(v) <= 65535) &&
    Number(draft.money) >= 0 &&
    Number(draft.minutes) < 60 &&
    Number(draft.seconds) < 60,
);

async function applyChanges() {
  if (dirty.value && valid.value) await setTrainer(changes.value);
}

const displayId = computed(() =>
  view.value.generation >= 7 ? String((Number(draft.sid) * 65536 + Number(draft.tid)) % 1_000_000).padStart(6, "0") : String(draft.tid).padStart(5, "0"),
);

useShell(() => ({
  hint: dirty.value ? "Modifications non appliquées" : "Change les champs puis « Appliquer »",
  actions: [
    { key: "Ctrl+Enter", cap: "Ctrl+Entrée", label: "Appliquer", run: applyChanges, disabled: !dirty.value || !valid.value },
  ],
}));
</script>

<template>
  <div class="trainer-tool">
    <div class="card sv-panel">
      <div class="tcard">
        <div class="avatar" :class="draft.gender">{{ draft.name.slice(0, 1).toUpperCase() || "?" }}</div>
        <div>
          <h3>{{ draft.name || "Dresseur" }}</h3>
          <p>ID {{ displayId }}</p>
          <p>{{ Number(draft.money).toLocaleString("fr-FR") }} ₽ · {{ draft.hours }} h {{ String(draft.minutes).padStart(2, "0") }}</p>
        </div>
      </div>
    </div>

    <div class="form sv-panel">
      <div class="sv-grid">
        <div class="sv-field">
          <span class="sv-label">Nom</span>
          <input v-model="draft.name" class="sv-input" :maxlength="view.trainerNameMax" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Sexe</span>
          <div class="sv-seg">
            <button :class="{ on: draft.gender === 'male' }" @click="draft.gender = 'male'">♂ Garçon</button>
            <button :class="{ on: draft.gender === 'female' }" @click="draft.gender = 'female'">♀ Fille</button>
          </div>
        </div>
        <div class="sv-field">
          <span class="sv-label">ID <Tip term="tid" /></span>
          <input v-model.number="draft.tid" class="sv-input" type="number" min="0" max="65535" />
        </div>
        <div class="sv-field">
          <span class="sv-label">ID secret <Tip term="sid" /></span>
          <input v-model.number="draft.sid" class="sv-input" type="number" min="0" max="65535" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Argent <Tip term="money" /></span>
          <div class="sv-row nowrap">
            <input v-model.number="draft.money" class="sv-input" type="number" min="0" max="9999999" />
            <button class="sv-btn" @click="draft.money = 999999">Max</button>
          </div>
        </div>
        <div class="sv-field">
          <span class="sv-label">Temps de jeu <Tip term="playTime" /></span>
          <div class="time">
            <input v-model.number="draft.hours" class="sv-input" type="number" min="0" max="999" aria-label="Heures" /> h
            <input v-model.number="draft.minutes" class="sv-input" type="number" min="0" max="59" aria-label="Minutes" /> min
            <input v-model.number="draft.seconds" class="sv-input" type="number" min="0" max="59" aria-label="Secondes" /> s
          </div>
        </div>
      </div>
      <p class="sv-help warn">
        Changer l'ID ou l'ID secret ne modifie pas tes Pokémon : ceux que tu as déjà attrapés seront vus comme venant d'un autre dresseur
        (ils obéissent moins bien sans badges).
      </p>
      <footer class="sv-row">
        <span v-if="dirty" class="pending">Modifications non appliquées</span>
        <button class="sv-btn" :disabled="!dirty" @click="reset">Annuler</button>
        <button class="sv-btn solid" :disabled="!dirty || !valid" @click="applyChanges">Appliquer</button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.trainer-tool {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 20px;
  align-items: start;
}

.card {
  padding: 20px;
}

.tcard {
  display: flex;
  align-items: center;
  gap: 16px;
}

.tcard h3 {
  font-size: 22px;
}

.tcard p {
  margin: 2px 0;
  color: var(--text-dim);
}

.avatar {
  display: grid;
  place-items: center;
  width: 72px;
  height: 72px;
  border: 3px solid #fff;
  border-radius: 50%;
  background: linear-gradient(160deg, #6fb4ff, #2f6fd6);
  color: #fff;
  font: 700 30px var(--font-display);
}

.avatar.female {
  background: linear-gradient(160deg, #ff9ccc, #e0558f);
}

.form {
  padding: 22px;
}

.nowrap {
  flex-wrap: nowrap;
}

.time {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
}

.time .sv-input {
  width: 74px;
}

.warn {
  margin-top: 16px;
  color: var(--warn);
}

footer {
  justify-content: flex-end;
  margin-top: 16px;
}

.pending {
  margin-right: auto;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--warn-bg);
  color: var(--warn);
  font-size: 12px;
  font-weight: 700;
}
</style>
