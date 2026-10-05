<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import { saveState } from "../../saveStore";
import type { SlotView } from "../../types";
import { apply, hex, num } from "./edit";

const props = defineProps<{ p: SlotView }>();
const view = computed(() => saveState.view!);
const gen = computed(() => view.value.generation);
const otMax = computed(() => (gen.value >= 6 ? 12 : 7));

const ot = ref(props.p.otName);
const tid = ref(props.p.tid);
const sid = ref(props.p.sid);
const pid = ref(hex(props.p.pid));
const ec = ref(hex(props.p.encryptionConstant));
watch(
  () => props.p,
  (p) => {
    ot.value = p.otName;
    tid.value = p.tid;
    sid.value = p.sid;
    pid.value = hex(p.pid);
    ec.value = hex(p.encryptionConstant);
  },
);

const isMine = computed(() => {
  const t = view.value.trainer;
  return props.p.otName === t.name && props.p.tid === t.tid && props.p.sid === t.sid;
});

function useMine() {
  const t = view.value.trainer;
  apply({ otName: t.name, tid: t.tid, sid: t.sid, otGender: t.gender });
}

function commitOt() {
  const n = ot.value.trim();
  if (n && n !== props.p.otName) apply({ otName: n });
}

function commitId(which: "tid" | "sid") {
  const v = num(which === "tid" ? tid.value : sid.value, 0, 65535);
  if (v === null) return;
  if (which === "tid" && v !== props.p.tid) apply({ tid: v });
  if (which === "sid" && v !== props.p.sid) apply({ sid: v });
}

const parseHex = (s: string) => (/^[0-9a-f]{1,8}$/i.test(s.trim()) ? parseInt(s.trim(), 16) >>> 0 : null);
const pidValid = computed(() => parseHex(pid.value) !== null);
const ecValid = computed(() => parseHex(ec.value) !== null);

function commitPid() {
  const v = parseHex(pid.value);
  if (v !== null && v !== props.p.pid) apply({ pid: v });
}

function commitEc() {
  const v = parseHex(ec.value);
  if (v !== null && v !== props.p.encryptionConstant) apply({ encryptionConstant: v });
}

const displayTid = computed(() => (gen.value >= 7 ? String(((props.p.sid * 65536 + props.p.tid) % 1_000_000)).padStart(6, "0") : String(props.p.tid).padStart(5, "0")));
</script>

<template>
  <div class="trainer">
    <section>
      <h3 class="sv-section-title">Dresseur d'origine <Tip term="ot" /></h3>
      <div class="sv-grid">
        <div class="sv-field">
          <span class="sv-label">Nom</span>
          <input v-model="ot" class="sv-input" :maxlength="otMax" @change="commitOt" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Sexe</span>
          <div class="sv-seg">
            <button :class="{ on: p.otGender === 'male' }" @click="apply({ otGender: 'male' })">♂ Garçon</button>
            <button :class="{ on: p.otGender === 'female' }" @click="apply({ otGender: 'female' })">♀ Fille</button>
          </div>
        </div>
        <div class="sv-field">
          <span class="sv-label">ID <Tip term="tid" /></span>
          <input v-model="tid" class="sv-input" type="number" min="0" max="65535" @change="commitId('tid')" />
          <p class="sv-help">Affiché en jeu : {{ displayTid }}</p>
        </div>
        <div class="sv-field">
          <span class="sv-label">ID secret <Tip term="sid" /></span>
          <input v-model="sid" class="sv-input" type="number" min="0" max="65535" @change="commitId('sid')" />
        </div>
      </div>
      <div class="sv-row mine">
        <span v-if="isMine" class="ok">✓ C'est ton Pokémon ({{ view.trainer.name }})</span>
        <template v-else>
          <span class="dim">Dresseur différent de la sauvegarde ({{ view.trainer.name }}).</span>
          <button class="sv-btn" @click="useMine">Mettre mes infos de dresseur</button>
        </template>
      </div>
    </section>

    <section>
      <h3 class="sv-section-title">Chromatique <Tip term="shiny" /></h3>
      <div class="sv-row">
        <button class="sv-btn" :class="{ solid: p.shiny }" @click="apply({ shiny: 'star' })">★ Chromatique</button>
        <button class="sv-btn" @click="apply({ shiny: 'square' })">■ Carré</button>
        <button class="sv-btn" @click="apply({ shiny: 'keepPid' })">Garder le PID</button>
        <button class="sv-btn" :class="{ solid: !p.shiny }" @click="apply({ shiny: 'none' })">Normal</button>
      </div>
      <p class="sv-help">
        Kaleido choisit un nouveau PID qui garde la nature, le sexe et le talent. « Garder le PID » change plutôt l'ID secret : utile pour les
        Pokémon sauvages des Gen 3 à 5, dont les IV sont liés au PID.
      </p>
      <p class="sv-help">TSV {{ p.tsv }} · PSV {{ p.psv }} <Tip term="tsv" /></p>
    </section>

    <section>
      <h3 class="sv-section-title">Identifiants cachés</h3>
      <div class="sv-grid">
        <div class="sv-field">
          <span class="sv-label">PID <Tip term="pid" /></span>
          <input v-model="pid" class="sv-input mono" :class="{ invalid: !pidValid }" maxlength="8" spellcheck="false" @change="commitPid" />
        </div>
        <div v-if="gen >= 6" class="sv-field">
          <span class="sv-label">Constante de chiffrement <Tip term="ec" /></span>
          <input v-model="ec" class="sv-input mono" :class="{ invalid: !ecValid }" maxlength="8" spellcheck="false" @change="commitEc" />
        </div>
      </div>
      <p class="sv-help warn">Modifier ces valeurs à la main peut rendre le Pokémon illégal. Valeurs en hexadécimal.</p>
    </section>
  </div>
</template>

<style scoped>
.trainer {
  display: flex;
  flex-direction: column;
  gap: 26px;
}

.mine {
  margin-top: 12px;
}

.ok {
  color: #8ff0b5;
  font-weight: 600;
}

.dim {
  color: var(--text-dim);
}

.warn {
  color: var(--warn);
}

section .sv-help {
  margin-top: 8px;
}
</style>
