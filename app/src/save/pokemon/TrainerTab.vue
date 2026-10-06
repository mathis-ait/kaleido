<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Tip from "../../components/Tip.vue";
import Combo from "../../components/Combo.vue";
import { lists, saveState } from "../../saveStore";
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

// --- Gen 6/7 : soigneur et pays.
const h = computed(() => props.p.extras.handler);
const htName = ref(h.value?.name ?? "");
const htFriendship = ref(h.value?.friendship ?? 0);
watch(h, (v) => {
  htName.value = v?.name ?? "";
  htFriendship.value = v?.friendship ?? 0;
});
function commitHtName() {
  const n = htName.value.trim();
  if (h.value && n !== h.value.name) apply({ extras: { htName: n } });
}
function commitHtFriendship() {
  const v = num(htFriendship.value, 0, 255);
  if (v !== null && h.value && v !== h.value.friendship) apply({ extras: { htFriendship: v } });
}
const countryOptions = computed(() => [{ value: 0, label: "(Aucun)" }, ...lists.countries]);
const regionsOf = (country: number) => [{ value: 0, label: "(Aucune)" }, ...lists.regions.filter((r) => r[0] === country).map((r) => ({ value: r[1], label: r[2] }))];
const country = computed({
  get: () => h.value?.country ?? 0,
  set: (v: number) => v !== h.value?.country && apply({ extras: { country: v, region: regionsOf(v).length > 1 ? regionsOf(v)[1].value : 0 } }),
});
const countryName = (c: number) => lists.countries.find((o) => o.value === c)?.label ?? (c ? `Pays n°${c}` : "—");
const regionName = (c: number, r: number) => lists.regions.find((x) => x[0] === c && x[1] === r)?.[2] ?? (r ? `n°${r}` : "");
/** Lieux des derniers échanges ; choisir un pays met sa première région. */
function setGeo(i: number, part: 0 | 1, v: number) {
  const geo = (h.value?.geo ?? []).map((g) => [...g] as [number, number]);
  geo[i][part] = v;
  if (part === 1) geo[i][0] = regionsOf(v).length > 1 ? regionsOf(v)[1].value : 0;
  apply({ extras: { geo } });
}
function clearHandler() {
  apply({
    extras: {
      htName: "",
      htGender: "male",
      currentHandler: 0,
      htFriendship: 0,
      htAffection: 0,
      htMemory: { id: 0, intensity: 0, feeling: 0, variable: 0 },
      geo: [[0, 0], [0, 0], [0, 0], [0, 0], [0, 0]],
    },
  });
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

    <section v-if="h">
      <h3 class="sv-section-title">Soigneur <Tip term="handler" /></h3>
      <div class="sv-grid">
        <div class="sv-field">
          <span class="sv-label">Nom du soigneur</span>
          <input v-model="htName" class="sv-input" maxlength="12" placeholder="Jamais échangé" @change="commitHtName" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Sexe du soigneur</span>
          <div class="sv-seg">
            <button :class="{ on: h.gender !== 'female' }" @click="apply({ extras: { htGender: 'male' } })">♂ Garçon</button>
            <button :class="{ on: h.gender === 'female' }" @click="apply({ extras: { htGender: 'female' } })">♀ Fille</button>
          </div>
        </div>
        <div class="sv-field">
          <span class="sv-label">S'occupe du Pokémon</span>
          <div class="sv-seg">
            <button :class="{ on: h.current === 0 }" @click="apply({ extras: { currentHandler: 0 } })">Dresseur d'origine</button>
            <button :class="{ on: h.current === 1 }" :disabled="!h.name" @click="apply({ extras: { currentHandler: 1 } })">Soigneur</button>
          </div>
        </div>
        <div class="sv-field">
          <span class="sv-label">Bonheur avec le soigneur</span>
          <input v-model="htFriendship" class="sv-input" type="number" min="0" max="255" :disabled="!h.name" @change="commitHtFriendship" />
        </div>
      </div>
      <div class="sv-row mine">
        <button class="sv-btn" :disabled="!h.name && h.current === 0" @click="clearHandler">Effacer le soigneur (jamais échangé)</button>
      </div>
    </section>

    <section v-if="h">
      <h3 class="sv-section-title">Pays et région <Tip term="country" /></h3>
      <div class="sv-grid">
        <div class="sv-field">
          <span class="sv-label">Pays</span>
          <Combo v-model="country" :options="countryOptions" placeholder="Chercher un pays…" />
        </div>
        <div class="sv-field">
          <span class="sv-label">Région</span>
          <select class="sv-select" :value="h.region" @change="apply({ extras: { region: Number(($event.target as HTMLSelectElement).value) } })">
            <option v-if="!regionsOf(h.country).some((r) => r.value === h!.region)" :value="h.region">Région n°{{ h.region }}</option>
            <option v-for="r in regionsOf(h.country)" :key="r.value" :value="r.value">{{ r.label }}</option>
          </select>
        </div>
        <div class="sv-field">
          <span class="sv-label">Zone de la console</span>
          <select class="sv-select" :value="h.consoleRegion" @change="apply({ extras: { consoleRegion: Number(($event.target as HTMLSelectElement).value) } })">
            <template v-for="(name, i) in lists.consoleRegions" :key="i">
              <option v-if="name" :value="i">{{ name }}</option>
            </template>
          </select>
        </div>
      </div>
      <details class="geo">
        <summary>Pays des derniers échanges ({{ h.geo.filter((g) => g[1]).length }}/5)</summary>
        <div v-for="(g, i) in h.geo" :key="i" class="geo-row">
          <span class="dim">{{ i + 1 }}.</span>
          <select class="sv-select" :value="g[1]" :aria-label="`Pays ${i + 1}`" @change="setGeo(i, 1, Number(($event.target as HTMLSelectElement).value))">
            <option :value="0">(Aucun)</option>
            <option v-if="g[1] && !lists.countries.some((c) => c.value === g[1])" :value="g[1]">{{ countryName(g[1]) }}</option>
            <option v-for="c in lists.countries" :key="c.value" :value="c.value">{{ c.label }}</option>
          </select>
          <select class="sv-select" :value="g[0]" :disabled="!g[1]" :aria-label="`Région ${i + 1}`" @change="setGeo(i, 0, Number(($event.target as HTMLSelectElement).value))">
            <option v-if="g[0] && !regionName(g[1], g[0])" :value="g[0]">Région n°{{ g[0] }}</option>
            <option v-for="r in regionsOf(g[1])" :key="r.value" :value="r.value">{{ r.label }}</option>
          </select>
        </div>
      </details>
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
  color: var(--ok);
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

.geo {
  margin-top: 14px;
}

.geo summary {
  color: var(--text-dim);
  font-weight: 600;
  cursor: pointer;
}

.geo-row {
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}
</style>
