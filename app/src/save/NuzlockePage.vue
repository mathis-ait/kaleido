<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import { addPaths, library } from "../library";
import { notify, saveState } from "../saveStore";
import { useShell } from "./shell";
import {
  CATCH_LABEL,
  linkRom,
  monName,
  nuzlockeView,
  ORIGIN_LABEL,
  ROM_GAMES,
  RULES,
  setNuzlockeState,
  STATUS_LABEL,
  type NuzlockeView,
  type NuzMon,
  type NuzRoute,
  type NuzState,
} from "./nuzlocke/types";

type Tab = "routes" | "team" | "caps" | "violations" | "rules";
type RouteFilter = "all" | "todo" | "done";

const data = ref<NuzlockeView | null>(null);
const loading = ref(false);
const linking = ref(false);
const error = ref<string | null>(null);
const tab = ref<Tab>("routes");
const filter = ref<RouteFilter>("all");

const report = computed(() => data.value?.report ?? null);
const state = computed(() => data.value?.state ?? null);
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

async function run(action: () => Promise<NuzlockeView>) {
  error.value = null;
  try {
    data.value = await action();
  } catch (e) {
    error.value = String(e);
  }
}

async function load() {
  loading.value = true;
  await run(nuzlockeView);
  loading.value = false;
}

onMounted(load);
// La sauvegarde a changé (modification, annulation…) : bilan à recalculer.
watch(
  () => saveState.view,
  () => data.value?.report && load(),
);

/** Modifie l'état (règles, marques) et l'enregistre à côté de la sauvegarde. */
async function update(change: (s: NuzState) => void) {
  if (!state.value) return;
  const next: NuzState = JSON.parse(JSON.stringify(state.value));
  change(next);
  await run(() => setNuzlockeState(next));
}

// --- ROM liée.

const candidates = computed(() => {
  const games = ROM_GAMES[saveState.view?.version ?? ""] ?? [];
  return library.items.filter((d) => d.kind === "nds_rom" && d.game && games.includes(d.game.id));
});

async function link(path: string | null) {
  linking.value = true;
  await run(() => linkRom(path));
  linking.value = false;
  if (path && !error.value) {
    notify("ROM liée : les rencontres affichées sont celles de ta partie");
    if (!library.items.some((d) => d.path === path)) addPaths([path]);
  }
}

async function pickRom() {
  const path = await open({ title: "Choisir la ROM de la partie", filters: [{ name: "ROM Nintendo DS", extensions: ["nds"] }] });
  if (typeof path === "string") link(path);
}

// --- Routes.

const routes = computed(() => {
  const all = report.value?.routes ?? [];
  if (filter.value === "todo") return all.filter((r) => r.status === "pending" || r.status === "dupeOnly");
  if (filter.value === "done") return all.filter((r) => r.status === "caught" || r.status === "missed");
  return all;
});

function toggleMissed(r: NuzRoute) {
  update((s) => {
    s.missed = r.markedMissed ? s.missed.filter((k) => k !== r.key) : [...s.missed, r.key];
  });
}

const encounterTitle = (e: { name: string; minLevel: number; maxLevel: number; methods: string[]; owned: boolean }) =>
  `${e.name} · N. ${e.minLevel === e.maxLevel ? e.minLevel : `${e.minLevel}–${e.maxLevel}`} · ${e.methods.join(", ")}${e.owned ? " · famille déjà capturée (doublon)" : ""}`;

// --- Morts.

function setDead(m: NuzMon, dead: boolean) {
  update((s) => {
    s.dead = s.dead.filter((k) => k !== m.key);
    s.alive = s.alive.filter((k) => k !== m.key);
    // Une mort détectée automatiquement s'annule en déclarant le Pokémon vivant, et inversement.
    if (dead && !m.dead) s.dead.push(m.key);
    if (!dead && m.dead && m.deathCause !== "Marqué mort à la main") s.alive.push(m.key);
  });
}

const manualAlive = (m: NuzMon) => state.value?.alive.includes(m.key) ?? false;

function resetMon(m: NuzMon) {
  update((s) => {
    s.dead = s.dead.filter((k) => k !== m.key);
    s.alive = s.alive.filter((k) => k !== m.key);
  });
}

// --- Champions et niveau maximum.

const currentCap = computed(() => report.value?.caps.filter((c) => c.current) ?? []);
const capLeader = computed(() => {
  const caps = currentCap.value;
  if (!caps.length) return null;
  return caps.reduce((a, b) => (b.aceLevel > a.aceLevel ? b : a));
});

function setBadges(v: string) {
  update((s) => (s.badges = v === "" ? null : Number(v)));
}

function setGraveyard(v: string) {
  update((s) => (s.graveyardBox = v === "" ? null : Number(v)));
}

const counts = computed(() => {
  const v = report.value?.violations ?? [];
  return { errors: v.filter((x) => x.severity === "error").length, total: v.length };
});

const TABS = computed<{ id: Tab; label: string; badge?: number }[]>(() => [
  { id: "routes", label: "Routes" },
  { id: "team", label: "Équipe et cimetière" },
  { id: "caps", label: "Champions" },
  { id: "violations", label: "Infractions", badge: counts.value.total || undefined },
  { id: "rules", label: "Règles" },
]);

useShell(() => ({
  hint: report.value ? "Suivi du Nuzlocke : routes, morts, niveau maximum" : "Lie la ROM de ta partie pour suivre ton Nuzlocke",
  actions: report.value ? [{ key: "r", cap: "R", label: "Actualiser", run: load }] : [],
}));
</script>

<template>
  <div class="nuz">
    <div v-if="error" class="err" role="alert"><Icon name="alert" :size="16" /> {{ error }}</div>

    <div v-if="loading && !data" class="sv-panel empty"><p>Lecture de la ROM…</p></div>

    <!-- Jeu non pris en charge -->
    <div v-else-if="data && !data.supported" class="sv-panel empty">
      <Icon name="swords" :size="48" />
      <h2>Mode Nuzlocke</h2>
      <p>
        Le suivi Nuzlocke fonctionne pour l'instant avec <strong>Pokémon Platine</strong>, <strong>Noire</strong> et <strong>Blanche</strong>.
        Cette sauvegarde est une partie de {{ data.saveGame }}.
      </p>
    </div>

    <!-- Aucune ROM liée (ou ROM introuvable) -->
    <div v-else-if="data && !report" class="sv-panel empty link">
      <Icon name="swords" :size="48" />
      <h2>Lie la ROM de ta partie</h2>
      <p>
        Pour savoir quels Pokémon t'attendent sur chaque route, Kaleido lit la <strong>ROM avec laquelle tu joues</strong>, par exemple celle
        que tu as randomisée ici. Les rencontres, les familles d'évolution et le niveau des champions viennent de cette ROM.
        <Tip
          title="Nuzlocke"
          text="Un défi qui rend le jeu plus difficile : on ne capture que le premier Pokémon de chaque route, et un Pokémon K.O. est considéré comme mort (il ne peut plus être utilisé). Kaleido suit ta partie à partir de la sauvegarde."
        />
      </p>
      <ol class="steps">
        <li>Ajoute la ROM à la bibliothèque (glisser-déposer sur l'accueil de Kaleido), ou choisis le fichier ci-dessous.</li>
        <li>Kaleido vérifie qu'elle correspond à la sauvegarde ({{ data.saveGame }}).</li>
        <li>Le lien est gardé dans un petit fichier à côté de la sauvegarde.</li>
      </ol>
      <div v-if="data.state.romPath && data.error" class="warn-box">
        <Icon name="alert" :size="16" /> ROM liée inutilisable : {{ data.error }}
        <button class="sv-btn" @click="link(null)">Délier</button>
      </div>
      <div v-if="candidates.length" class="cands">
        <button v-for="d in candidates" :key="d.path" class="cand sv-panel" :disabled="linking" @click="link(d.path)">
          <Icon name="file" :size="22" />
          <span class="c-txt">
            <strong>{{ d.fileName }}</strong>
            <small>{{ d.title }}<template v-if="d.kaleido"> · randomisée par Kaleido, seed {{ d.kaleido.seed }}</template></small>
          </span>
          <span v-if="d.kaleido" class="chip ok">Kaleido</span>
        </button>
      </div>
      <p v-else class="sv-help">Aucune ROM compatible dans la bibliothèque pour l'instant.</p>
      <button class="sv-btn solid" :disabled="linking" @click="pickRom">
        <Icon name="folder-open" :size="16" /> {{ linking ? "Lecture de la ROM…" : "Choisir le fichier de la ROM…" }}
      </button>
    </div>

    <!-- Bilan -->
    <template v-else-if="data && report && state">
      <header class="head">
        <div class="stats">
          <div class="stat sv-panel">
            <small>Captures</small>
            <strong>{{ report.stats.captures }}</strong>
            <span>{{ report.stats.routesCaught }} / {{ report.stats.routes }} routes · {{ report.stats.routesMissed }} ratée(s)</span>
          </div>
          <div class="stat sv-panel" :class="{ bad: report.stats.dead > 0 }">
            <small>Morts</small>
            <strong>{{ report.stats.dead }}</strong>
            <span>{{ report.stats.alive }} en vie</span>
          </div>
          <div class="stat sv-panel">
            <small>Badges <Tip title="Badges" text="Lus dans la sauvegarde. Tu peux forcer une valeur si besoin : le niveau maximum suit le nombre de badges." /></small>
            <strong>{{ report.stats.badges }} / 8</strong>
            <select class="sv-select mini" :value="state.badges ?? ''" @change="setBadges(($event.target as HTMLSelectElement).value)">
              <option value="">Auto (sauvegarde)</option>
              <option v-for="n in 9" :key="n" :value="n - 1">{{ n - 1 }} badge{{ n - 1 > 1 ? "s" : "" }}</option>
            </select>
          </div>
          <div class="stat sv-panel cap" :class="{ off: !state.rules.levelCaps }">
            <small>Niveau maximum <Tip title="Niveau maximum" text="Niveau du Pokémon le plus fort du prochain adversaire important, lu dans ta ROM (après randomisation). Aucun Pokémon de l'équipe ne doit le dépasser." /></small>
            <div v-if="capLeader" class="cap-row">
              <Sprite :id="capLeader.aceSpecies" :size="56" />
              <div>
                <strong>N. {{ report.stats.levelCap }}</strong>
                <span>{{ currentCap.length > 1 ? "Conseil 4" : `${capLeader.name} (${capLeader.label})` }}</span>
              </div>
            </div>
            <span v-else>—</span>
          </div>
        </div>
        <div class="rom-line">
          <Icon name="file" :size="15" />
          <span :title="state.romPath ?? ''">{{ fileName(state.romPath ?? "") }} · {{ report.gameName }}</span>
          <span v-if="report.seed !== null" class="chip">Seed {{ report.seed }}</span>
          <span v-for="r in RULES.filter((r) => state!.rules[r.id])" :key="r.id" class="chip rule">{{ r.label }}</span>
          <button class="sv-btn" @click="pickRom">Changer de ROM</button>
          <button class="sv-btn" title="Relire la sauvegarde" @click="load"><Icon name="refresh" :size="14" /></button>
        </div>
      </header>

      <nav class="sv-seg tabs">
        <button v-for="t in TABS" :key="t.id" :class="{ on: tab === t.id }" @click="tab = t.id">
          {{ t.label }}<span v-if="t.badge" class="count" :class="{ red: counts.errors > 0 }">{{ t.badge }}</span>
        </button>
      </nav>

      <!-- Routes -->
      <section v-if="tab === 'routes'" class="body">
        <div class="sv-row filters">
          <div class="sv-seg">
            <button :class="{ on: filter === 'all' }" @click="filter = 'all'">Toutes</button>
            <button :class="{ on: filter === 'todo' }" @click="filter = 'todo'">À faire</button>
            <button :class="{ on: filter === 'done' }" @click="filter = 'done'">Terminées</button>
          </div>
          <p class="sv-help">
            Ordre approximatif de l'histoire. Les Pokémon affichés viennent de ta ROM ; ceux en transparence sont des doublons.
            <Tip
              title="Première rencontre"
              text="Kaleido range chaque Pokémon de la sauvegarde sur la route où il a été rencontré (lieu de rencontre). Le starter, les œufs, les échanges et les cadeaux ne comptent pas. Marque une route « ratée » si le premier Pokémon s'est enfui ou est tombé K.O."
            />
          </p>
        </div>
        <ul class="routes">
          <li v-for="r in routes" :key="r.key" class="route sv-panel" :class="`st-${r.status}`">
            <div class="r-head">
              <strong>{{ r.name }}</strong>
              <span class="chip status" :class="r.status">{{ STATUS_LABEL[r.status] }}</span>
              <button v-if="!r.capture" class="sv-btn small" @click="toggleMissed(r)">
                {{ r.markedMissed ? "Annuler « raté »" : "Marquer ratée" }}
              </button>
            </div>
            <div class="r-body">
              <div class="encs">
                <span v-for="e in r.encounters" :key="e.species" class="enc" :class="{ owned: e.owned }" :title="encounterTitle(e)">
                  <Sprite :id="e.species" :size="40" />
                </span>
              </div>
              <div class="caught">
                <div v-if="r.capture" class="mon" :class="{ dead: r.capture.dead }">
                  <Sprite :id="r.capture.species" :shiny="r.capture.shiny" :size="44" />
                  <span class="m-txt">
                    <strong>{{ monName(r.capture) }}</strong>
                    <small>N. {{ r.capture.level }} · {{ r.capture.place }}<template v-if="r.capture.dead"> · mort</template></small>
                  </span>
                </div>
                <span v-else class="none">{{ r.status === "missed" ? "Raté" : r.status === "dupeOnly" ? "Doublon seulement : route encore ouverte" : "Pas encore" }}</span>
                <span v-for="o in r.others" :key="o.key" class="chip other" :class="o.catch ?? ''" :title="`${monName(o)} · N. ${o.level} · ${o.place}`">
                  {{ monName(o) }} · {{ CATCH_LABEL[o.catch ?? "extra"] }}
                </span>
              </div>
            </div>
          </li>
        </ul>
      </section>

      <!-- Équipe et cimetière -->
      <section v-else-if="tab === 'team'" class="body two">
        <div class="sv-panel block">
          <h3 class="sv-section-title">Équipe</h3>
          <div v-for="m in report.party" :key="m.key" class="mon-row" :class="{ dead: m.dead }">
            <Sprite :id="m.species" :shiny="m.shiny" :size="48" />
            <span class="m-txt">
              <strong>{{ monName(m) }}</strong>
              <small>
                N. {{ m.level }}
                <template v-if="report.stats.levelCap && m.level > report.stats.levelCap && state.rules.levelCaps"> · au-dessus du maximum</template>
                · {{ m.route ?? ORIGIN_LABEL[m.origin] }}
                <template v-if="m.deathCause"> · {{ m.deathCause }}</template>
              </small>
            </span>
            <button v-if="!m.dead" class="sv-btn small danger" @click="setDead(m, true)">Marquer mort</button>
            <button v-else class="sv-btn small" @click="setDead(m, false)">Il est vivant</button>
          </div>
        </div>
        <div class="sv-panel block">
          <h3 class="sv-section-title">
            Cimetière
            <Tip
              title="Cimetière"
              text="Les Pokémon morts : K.O. dans l'équipe (PV à 0), rangés dans la boîte « Cimetière », ou marqués à la main. Une boîte est reconnue toute seule si son nom contient RIP, Cimetière ou Morts ; tu peux aussi la choisir dans l'onglet Règles."
            />
          </h3>
          <p class="sv-help">
            <template v-if="report.graveyardBox">Boîte « {{ report.graveyardBox.name }} »{{ report.graveyardBox.auto ? " (reconnue par son nom)" : "" }}.</template>
            <template v-else>Aucune boîte cimetière : renomme une boîte « RIP » ou choisis-la dans l'onglet Règles.</template>
          </p>
          <div v-if="!report.graveyard.length" class="none">Aucun mort pour l'instant. Courage !</div>
          <div v-for="m in report.graveyard" :key="m.key" class="mon-row dead">
            <Sprite :id="m.species" :shiny="m.shiny" :size="48" />
            <span class="m-txt">
              <strong>{{ monName(m) }}</strong>
              <small>N. {{ m.level }} · {{ m.route ?? ORIGIN_LABEL[m.origin] }} · {{ m.deathCause }}</small>
            </span>
            <button class="sv-btn small" @click="setDead(m, false)">Il est vivant</button>
          </div>
          <template v-if="report.others.length">
            <h3 class="sv-section-title sub">Hors routes</h3>
            <div v-for="m in report.others" :key="m.key" class="mon-row" :class="{ dead: m.dead }">
              <Sprite :id="m.species" :shiny="m.shiny" :size="40" />
              <span class="m-txt">
                <strong>{{ monName(m) }}</strong>
                <small>{{ ORIGIN_LABEL[m.origin] }} · {{ m.metLocationName ?? "lieu inconnu" }} · {{ m.place }}</small>
              </span>
              <button v-if="manualAlive(m) || state.dead.includes(m.key)" class="sv-btn small" @click="resetMon(m)">Réinitialiser</button>
            </div>
          </template>
        </div>
      </section>

      <!-- Champions -->
      <section v-else-if="tab === 'caps'" class="body">
        <p class="sv-help">
          Équipes lues dans ta ROM : le niveau indiqué est celui du Pokémon le plus fort de chaque adversaire.
          <Tip title="Ace" text="Le Pokémon le plus fort (« ace ») d'un champion donne le niveau maximum à ne pas dépasser avant de l'affronter." />
        </p>
        <div class="caps">
          <div v-for="c in report.caps" :key="c.label + c.name" class="capc sv-panel" :class="{ beaten: c.beaten, current: c.current }">
            <Sprite :id="c.aceSpecies" :size="56" />
            <span class="m-txt">
              <small>{{ c.label }} · {{ c.town }}</small>
              <strong>{{ c.name }}</strong>
              <span>Niveau max {{ c.aceLevel }}</span>
            </span>
            <Icon v-if="c.beaten" name="check" :size="18" />
            <span v-else-if="c.current" class="chip status caught">Prochain</span>
            <span v-if="!c.verified" class="chip status missed" title="Classe de dresseur inattendue : la ROM a peut-être été modifiée autrement">?</span>
          </div>
        </div>
      </section>

      <!-- Infractions -->
      <section v-else-if="tab === 'violations'" class="body">
        <div v-if="!report.violations.length" class="sv-panel empty small">
          <Icon name="shield" :size="40" />
          <p>Aucune règle enfreinte. Bravo !</p>
        </div>
        <div v-for="(v, i) in report.violations" :key="i" class="viol sv-panel" :class="v.severity">
          <Icon :name="v.severity === 'info' ? 'info' : 'alert'" :size="18" />
          <span class="m-txt">
            <strong>{{ v.title }}</strong>
            <small>{{ v.detail }}</small>
          </span>
        </div>
        <p class="sv-help">
          Rouge : règle enfreinte · orange : à vérifier · bleu : pour information.
          <template v-if="state.rules.noItemsInBattle || state.rules.setMode">
            Les règles « pas de soins en combat » et « mode Set » ne peuvent pas être vérifiées dans la sauvegarde.
          </template>
        </p>
      </section>

      <!-- Règles -->
      <section v-else class="body">
        <div class="sv-panel block">
          <h3 class="sv-section-title">Règles de la partie</h3>
          <div class="rules">
            <label v-for="r in RULES" :key="r.id" class="sv-switch">
              <input type="checkbox" :checked="state.rules[r.id]" @change="update((s) => (s.rules[r.id] = ($event.target as HTMLInputElement).checked))" />
              <span class="track" />
              {{ r.label }}
              <Tip :title="r.label" :text="r.tip" />
              <small v-if="!r.check" class="dim">rappel</small>
            </label>
          </div>
        </div>
        <div class="sv-panel block">
          <h3 class="sv-section-title">Boîte « Cimetière »</h3>
          <select class="sv-select" :value="state.graveyardBox ?? ''" @change="setGraveyard(($event.target as HTMLSelectElement).value)">
            <option value="">Automatique (nom contenant RIP, Cimetière, Morts…)</option>
            <option v-for="(n, i) in report.boxNames" :key="i" :value="i">{{ i + 1 }}. {{ n }}</option>
          </select>
          <p class="sv-help">Réglages enregistrés dans {{ fileName(data.stateFile) }}, à côté de la sauvegarde. La sauvegarde elle-même n'est pas modifiée.</p>
          <div class="sv-row">
            <button class="sv-btn" @click="pickRom">Changer de ROM</button>
            <button class="sv-btn danger" @click="link(null)">Délier la ROM</button>
          </div>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.nuz {
  display: flex;
  flex-direction: column;
  gap: 14px;
  height: 100%;
  min-height: 0;
  padding: 18px 26px;
  overflow-y: auto;
}

.err,
.warn-box {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 12px;
  background: color-mix(in srgb, var(--danger) 18%, transparent);
  font-size: 13px;
}

.warn-box {
  background: var(--warn-bg);
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-width: 720px;
  margin: 20px auto;
  padding: 30px 36px;
  text-align: center;
}

.empty.small {
  margin: 0 auto;
  padding: 20px;
}

.empty h2 {
  margin: 0;
  font-size: 22px;
}

.empty p {
  margin: 0;
  line-height: 1.5;
}

.steps {
  margin: 0;
  padding-left: 20px;
  color: var(--text-dim);
  font-size: 13px;
  line-height: 1.6;
  text-align: left;
}

.cands {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}

.cand {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  color: var(--text);
  text-align: left;
}

.cand:hover:not(:disabled) {
  border-color: var(--accent-2);
}

.c-txt,
.m-txt {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.c-txt small,
.m-txt small {
  color: var(--text-dim);
  font-size: 12px;
}

.chip {
  display: inline-flex;
  align-items: center;
  padding: 2px 9px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}

.chip.ok,
.chip.status.caught {
  background: color-mix(in srgb, #22c55e 30%, transparent);
}

.chip.status.missed {
  background: color-mix(in srgb, var(--danger) 30%, transparent);
}

.chip.status.dupeOnly {
  background: color-mix(in srgb, var(--warn) 30%, transparent);
}

.chip.rule {
  background: color-mix(in srgb, var(--accent-2) 22%, transparent);
}

.chip.other.extra {
  background: color-mix(in srgb, var(--danger) 25%, transparent);
}

.chip.other.dupe {
  background: color-mix(in srgb, var(--warn) 25%, transparent);
}

.chip.other.shinyBonus {
  background: color-mix(in srgb, #eab308 30%, transparent);
}

/* En-tête */
.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: 12px;
}

.stat {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 16px;
}

.stat small {
  display: flex;
  align-items: center;
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.stat strong {
  font-size: 26px;
  font-weight: 600;
}

.stat span {
  color: var(--text-dim);
  font-size: 12px;
}

.stat.bad strong {
  color: var(--danger);
}

.stat.off {
  opacity: 0.5;
}

.cap-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cap-row div {
  display: flex;
  flex-direction: column;
}

.sv-select.mini {
  padding: 4px 8px;
  font-size: 12px;
}

.rom-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
  color: var(--text-dim);
  font-size: 13px;
}

.tabs {
  align-self: flex-start;
}

.count {
  margin-left: 6px;
  padding: 0 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--warn) 60%, transparent);
  color: #fff;
  font-size: 11px;
}

.count.red {
  background: var(--danger);
}

.body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.body.two {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
  align-items: start;
}

.filters {
  justify-content: space-between;
}

/* Routes */
.routes {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.route {
  padding: 10px 14px;
}

.route.st-caught {
  border-left: 4px solid #22c55e;
}

.route.st-missed {
  border-left: 4px solid var(--danger);
  opacity: 0.75;
}

.route.st-dupeOnly {
  border-left: 4px solid var(--warn);
}

.r-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.r-head strong {
  font-size: 15px;
}

.r-head .sv-btn {
  margin-left: auto;
}

.sv-btn.small {
  padding: 4px 10px;
  font-size: 12px;
}

.r-body {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 6px;
}

.encs {
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
}

.enc.owned {
  opacity: 0.3;
}

.caught {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.mon,
.mon-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.mon.dead,
.mon-row.dead {
  filter: grayscale(1);
}

.mon-row {
  padding: 6px 0;
  border-bottom: 1px solid var(--border);
}

.none {
  color: var(--text-dim);
  font-size: 13px;
  font-style: italic;
}

.block {
  padding: 16px 18px;
}

.sub {
  margin-top: 18px;
}

/* Champions */
.caps {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 10px;
}

.capc {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
}

.capc.beaten {
  opacity: 0.55;
}

.capc.current {
  box-shadow: 0 0 0 2px var(--accent-2);
}

.capc .m-txt span {
  font-size: 13px;
}

/* Infractions */
.viol {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-left: 4px solid var(--accent-2);
}

.viol.error {
  border-left-color: var(--danger);
}

.viol.warning {
  border-left-color: var(--warn);
}

/* Règles */
.rules {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px 20px;
}

.dim {
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 400;
}
</style>
