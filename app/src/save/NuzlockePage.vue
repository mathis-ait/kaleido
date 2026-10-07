<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Banner from "../components/Banner.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import { addPaths, library } from "../library";
import { notify, saveState } from "../saveStore";
import { isRom } from "../types";
import { useShell } from "./shell";
import {
  ALOLA_VERSIONS,
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
  type NuzRules,
  type NuzState,
  type NuzViolation,
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
  return library.items.filter((d) => isRom(d) && d.game && games.includes(d.game.id));
});

/** Sauvegarde d'un jeu 3DS : ROM `.3ds` ou dossier extrait. */
const is3ds = computed(() => (saveState.view?.generation ?? 0) >= 6);
/** Soleil / Lune et Ultra : capitaines et Grands Duels au lieu des badges. */
const alola = computed(() => ALOLA_VERSIONS.includes(saveState.view?.version ?? ""));
const gymCount = computed(() => Math.min(report.value?.caps.filter((c) => c.kind === "gym").length ?? 8, 8) || 8);

async function link(path: string | null) {
  linking.value = true;
  await run(() => linkRom(path));
  linking.value = false;
  if (path && !error.value) {
    notify("ROM liée : les rencontres affichées sont celles de ta partie");
    if (!library.items.some((d) => d.path === path)) addPaths([path]);
  }
}

async function pickRom(folder = false) {
  const path = await open({
    title: folder ? "Dossier du jeu 3DS extrait" : "Choisir la ROM de la partie",
    directory: folder,
    filters: folder ? undefined : [{ name: "ROM DS / 3DS", extensions: ["nds", "3ds", "cci", "cxi", "app"] }],
  });
  if (typeof path === "string") link(path);
}

// --- Routes.

const routes = computed(() => {
  const all = report.value?.routes ?? [];
  if (filter.value === "todo") return all.filter((r) => r.status === "pending" || r.status === "dupeOnly");
  if (filter.value === "done") return all.filter((r) => r.status === "caught" || r.status === "missed");
  return all;
});

const FILTERS: { value: RouteFilter; label: string }[] = [
  { value: "all", label: "Toutes" },
  { value: "todo", label: "À faire" },
  { value: "done", label: "Terminées" },
];

/** Liste de routes vide : message selon le filtre. */
const routesEmpty = computed(() =>
  filter.value === "todo"
    ? { title: "Aucune route à faire", text: "Toutes les routes lues dans ta ROM ont leur capture ou sont marquées ratées." }
    : filter.value === "done"
      ? { title: "Aucune route terminée", text: "Capture le premier Pokémon d'une route, ou marque-la ratée : elle apparaîtra ici." }
      : { title: "Aucune route", text: "Kaleido n'a trouvé aucune route avec des rencontres dans cette ROM." },
);

/** Pastille de statut d'une route (couleur de sens et bulle d'aide). */
const STATUS_CHIP: Record<NuzRoute["status"], { cls: string; term?: string }> = {
  pending: { cls: "dim" },
  caught: { cls: "ok" },
  missed: { cls: "danger", term: "nuzlocke.missed" },
  dupeOnly: { cls: "warn", term: "nuzlocke.dupe" },
};

/** Pastille d'un Pokémon capturé en plus sur une route. */
const CATCH_CHIP: Record<NonNullable<NuzMon["catch"]>, { cls: string; term?: string }> = {
  counted: { cls: "" },
  dupe: { cls: "warn", term: "nuzlocke.dupe" },
  shinyBonus: { cls: "shiny", term: "nuzlocke.shinyBonus" },
  extra: { cls: "danger", term: "nuzlocke.extra" },
};

function setRule(id: keyof NuzRules, on: boolean) {
  update((s) => (s.rules[id] = on));
}

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

/** Rattache un lieu de capture à une route (ou le marque « pas une route » avec une clé vide). */
function assignLocation(location: number, route: string) {
  update((s) => {
    s.locationRoutes = { ...(s.locationRoutes ?? {}), [location]: route };
  });
}

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

const TABS: { value: Tab; label: string }[] = [
  { value: "routes", label: "Routes" },
  { value: "team", label: "Équipe et cimetière" },
  { value: "caps", label: "Champions" },
  { value: "violations", label: "Infractions" },
  { value: "rules", label: "Règles" },
];

/** Gravité d'une infraction : pastille et icône. */
const SEVERITY: Record<NuzViolation["severity"], { cls: string; label: string; icon: string }> = {
  error: { cls: "danger", label: "Enfreinte", icon: "alert" },
  warning: { cls: "warn", label: "À vérifier", icon: "alert" },
  info: { cls: "dim", label: "Info", icon: "info" },
};

useShell(() => ({
  hint: report.value ? "Suivi du Nuzlocke : routes, morts, niveau maximum" : "Lie la ROM de ta partie pour suivre ton Nuzlocke",
  actions: report.value ? [{ key: "r", cap: "R", label: "Actualiser", run: load }] : [],
}));
</script>

<template>
  <div class="nuz" :aria-busy="loading || linking">
    <Banner v-if="error" :retry="load" :dismiss="() => (error = null)">{{ error }}</Banner>

    <EmptyState v-if="loading && !data" loading title="Lecture de la ROM…" />

    <!-- Jeu non pris en charge -->
    <div v-else-if="data && !data.supported" class="sv-panel gate">
      <EmptyState icon="swords" title="Mode Nuzlocke" term="nuzlocke.nuzlocke">
        Le suivi Nuzlocke fonctionne pour l'instant avec <strong>Pokémon Diamant, Perle, Platine</strong>,
        <strong>HeartGold, SoulSilver</strong>, <strong>Noire, Blanche</strong> et leurs suites, et sur 3DS avec
        <strong>X, Y, Rubis Oméga, Saphir Alpha, Soleil, Lune, Ultra-Soleil</strong> et <strong>Ultra-Lune</strong>.
        Cette sauvegarde est une partie de {{ data.saveGame }}.
      </EmptyState>
    </div>

    <!-- Aucune ROM liée (ou ROM introuvable) -->
    <div v-else-if="data && !report" class="sv-panel gate">
      <EmptyState icon="swords" title="Lie la ROM de ta partie" term="nuzlocke.nuzlocke">
        Pour savoir quels Pokémon t'attendent sur chaque route, Kaleido lit la <strong>ROM avec laquelle tu joues</strong>, par exemple celle
        que tu as randomisée ici. Les rencontres, les familles d'évolution et le niveau des champions viennent de cette ROM.
        <template #details>
          <ol class="steps">
            <li>Ajoute la ROM à la bibliothèque (glisser-déposer sur l'accueil de Kaleido), ou choisis le fichier ci-dessous.</li>
            <li>Kaleido vérifie qu'elle correspond à la sauvegarde ({{ data.saveGame }}).</li>
            <li>Le lien est gardé dans un petit fichier à côté de la sauvegarde.</li>
          </ol>
          <Banner v-if="data.state.romPath && data.error" tone="warn" class="wide">
            ROM liée inutilisable : {{ data.error }}
            <button type="button" class="sv-btn small" @click="link(null)">Délier</button>
          </Banner>
          <div v-if="candidates.length" class="cands">
            <button v-for="d in candidates" :key="d.path" type="button" class="cand" :disabled="linking" @click="link(d.path)">
              <Icon name="file" :size="22" />
              <span class="m-txt">
                <strong>{{ d.fileName }}</strong>
                <small>{{ d.title }}<template v-if="d.kaleido"> · randomisée par Kaleido, seed {{ d.kaleido.seed }}</template></small>
              </span>
              <span v-if="d.kaleido" class="sv-chip ok">Kaleido</span>
            </button>
          </div>
          <p v-else class="sv-help">Aucune ROM compatible dans la bibliothèque pour l'instant.</p>
        </template>
        <template #actions>
          <button type="button" class="sv-btn solid" :disabled="linking" @click="pickRom()">
            <Icon :name="linking ? 'refresh' : 'folder-open'" :size="16" :class="{ 'sv-spin': linking }" /> {{ linking ? "Lecture de la ROM…" : "Choisir le fichier de la ROM…" }}
          </button>
          <button v-if="is3ds" type="button" class="sv-btn" :disabled="linking" @click="pickRom(true)">
            <Icon name="folder" :size="16" /> Dossier 3DS extrait…
          </button>
        </template>
      </EmptyState>
    </div>

    <!-- Bilan -->
    <template v-else-if="data && report && state">
      <header class="head">
        <div class="stats">
          <div class="stat sv-panel">
            <span class="sv-label">Captures</span>
            <strong>{{ report.stats.captures }}</strong>
            <small>{{ report.stats.routesCaught }} / {{ report.stats.routes }} routes · {{ report.stats.routesMissed }} ratée(s)</small>
          </div>
          <div class="stat sv-panel" :class="{ bad: report.stats.dead > 0 }">
            <span class="sv-label">Morts <Tip term="nuzlocke.graveyard" /></span>
            <strong>{{ report.stats.dead }}</strong>
            <small>{{ report.stats.alive }} en vie</small>
          </div>
          <div class="stat sv-panel">
            <span v-if="alola" class="sv-label">Épreuves <Tip term="nuzlocke.trials" /></span>
            <span v-else class="sv-label">Badges <Tip term="nuzlocke.badges" /></span>
            <strong>{{ report.stats.badges }} / {{ gymCount }}</strong>
            <select
              class="sv-select compact"
              :aria-label="alola ? 'Nombre d\'épreuves' : 'Nombre de badges'"
              :value="state.badges ?? ''"
              @change="setBadges(($event.target as HTMLSelectElement).value)"
            >
              <option value="">Auto (sauvegarde)</option>
              <option v-for="n in gymCount + 1" :key="n" :value="n - 1">
                {{ n - 1 }} {{ alola ? "épreuve" : "badge" }}{{ n - 1 > 1 ? "s" : "" }}
              </option>
            </select>
          </div>
          <div class="stat sv-panel" :class="{ off: !state.rules.levelCaps }">
            <span class="sv-label">Niveau maximum <Tip term="nuzlocke.levelCaps" /></span>
            <div v-if="capLeader" class="cap-row">
              <Sprite :id="capLeader.aceSpecies" :size="56" />
              <div class="m-txt">
                <strong>N. {{ report.stats.levelCap }}</strong>
                <small>{{ currentCap.length > 1 ? "Conseil 4" : `${capLeader.name} (${capLeader.label})` }}</small>
              </div>
            </div>
            <small v-else>Aucun champion trouvé dans la ROM</small>
          </div>
        </div>
        <div class="rom-line">
          <Icon name="file" :size="15" />
          <span :title="state.romPath ?? ''">{{ fileName(state.romPath ?? "") }} · {{ report.gameName }}</span>
          <span v-if="report.seed !== null" class="sv-chip dim">Seed {{ report.seed }} <Tip term="nuzlocke.seed" /></span>
          <span v-for="r in RULES.filter((r) => state!.rules[r.id])" :key="r.id" class="sv-chip">{{ r.label }} <Tip :term="`nuzlocke.${r.id}`" /></span>
          <span class="grow" />
          <button type="button" class="sv-btn" @click="pickRom()">Changer de ROM</button>
          <button
            type="button"
            class="sv-round sq"
            :disabled="loading"
            :aria-busy="loading"
            :aria-label="loading ? 'Relecture de la sauvegarde…' : 'Relire la sauvegarde'"
            title="Relire la sauvegarde (R)"
            @click="load"
          >
            <Icon name="refresh" :size="14" :class="{ 'sv-spin': loading }" />
          </button>
        </div>
      </header>

      <Segmented v-model="tab" class="tabs" label="Sections du suivi" :options="TABS">
        <template #extra="{ option }">
          <span v-if="option.value === 'violations' && counts.total" class="sv-chip count" :class="counts.errors ? 'danger' : 'warn'">{{ counts.total }}</span>
        </template>
      </Segmented>

      <!-- Routes -->
      <section v-if="tab === 'routes'" class="body">
        <div class="sv-row filters">
          <Segmented v-model="filter" label="Filtrer les routes" :options="FILTERS" />
          <p class="sv-help">
            Ordre approximatif de l'histoire. Les Pokémon affichés viennent de ta ROM ; ceux en transparence sont des doublons.
            <Tip term="nuzlocke.firstEncounter" />
          </p>
        </div>
        <div v-if="!routes.length" class="sv-panel">
          <EmptyState compact icon="map" :title="routesEmpty.title">
            {{ routesEmpty.text }}
            <template v-if="filter !== 'all'" #actions>
              <button type="button" class="sv-btn" @click="filter = 'all'">Voir toutes les routes</button>
            </template>
          </EmptyState>
        </div>
        <ul v-else class="routes">
          <li v-for="r in routes" :key="r.key" class="route sv-panel" :class="{ missed: r.status === 'missed' }">
            <div class="r-head">
              <strong>{{ r.name }}</strong>
              <span class="sv-chip" :class="STATUS_CHIP[r.status].cls">
                {{ STATUS_LABEL[r.status] }}<Tip v-if="STATUS_CHIP[r.status].term" :term="STATUS_CHIP[r.status].term" />
              </span>
              <button v-if="!r.capture" type="button" class="sv-btn small" @click="toggleMissed(r)">
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
                <span v-else-if="r.status === 'dupeOnly'" class="sv-help">Doublon seulement : route encore ouverte</span>
                <span
                  v-for="o in r.others"
                  :key="o.key"
                  class="sv-chip"
                  :class="CATCH_CHIP[o.catch ?? 'extra'].cls"
                  :title="`${monName(o)} · N. ${o.level} · ${o.place}`"
                >
                  {{ monName(o) }} · {{ CATCH_LABEL[o.catch ?? "extra"] }}<Tip v-if="CATCH_CHIP[o.catch ?? 'extra'].term" :term="CATCH_CHIP[o.catch ?? 'extra'].term" />
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
          <EmptyState v-if="!report.party.length" compact icon="ball" title="Équipe vide">
            Aucun Pokémon dans l'équipe de la sauvegarde.
          </EmptyState>
          <div v-for="m in report.party" :key="m.key" class="mon-row" :class="{ dead: m.dead }">
            <Sprite :id="m.species" :shiny="m.shiny" :size="48" />
            <span class="m-txt">
              <strong>{{ monName(m) }}</strong>
              <small>
                N. {{ m.level }}
                <template v-if="report.stats.levelCap && m.level > report.stats.levelCap && state.rules.levelCaps"> · au-dessus du maximum</template>
                · {{ m.route ?? ORIGIN_LABEL[m.origin] }}
                <template v-if="m.deathCause"> · {{ m.deathCause }}</template>
                <template v-else-if="m.fainted"> · K.O. : à déposer en boîte</template>
              </small>
            </span>
            <button v-if="!m.dead" type="button" class="sv-btn small danger" @click="setDead(m, true)">Marquer mort</button>
            <button v-else type="button" class="sv-btn small" @click="setDead(m, false)">Il est vivant</button>
          </div>
        </div>
        <div class="sv-panel block">
          <h3 class="sv-section-title">Cimetière <Tip term="nuzlocke.graveyard" /></h3>
          <p class="sv-help">
            <template v-if="report.graveyardBox">Boîte « {{ report.graveyardBox.name }} »{{ report.graveyardBox.auto ? " (reconnue par son nom)" : "" }}.</template>
            <template v-else>Aucune boîte cimetière : renomme une boîte « RIP » ou choisis-la dans l'onglet Règles.</template>
          </p>
          <EmptyState v-if="!report.graveyard.length" compact icon="heart" title="Aucun mort pour l'instant">Courage !</EmptyState>
          <div v-for="m in report.graveyard" :key="m.key" class="mon-row dead">
            <Sprite :id="m.species" :shiny="m.shiny" :size="48" />
            <span class="m-txt">
              <strong>{{ monName(m) }}</strong>
              <small>N. {{ m.level }} · {{ m.route ?? ORIGIN_LABEL[m.origin] }} · {{ m.deathCause }}</small>
            </span>
            <button type="button" class="sv-btn small" @click="setDead(m, false)">Il est vivant</button>
          </div>
          <template v-if="report.unassigned.length">
            <h3 class="sv-section-title sub">Lieux à rattacher <Tip term="nuzlocke.unassigned" /></h3>
            <div v-for="u in report.unassigned" :key="u.location" class="mon-row">
              <Icon name="map" :size="18" />
              <span class="m-txt">
                <strong>{{ u.name }}</strong>
                <small>{{ u.count }} Pokémon capturé{{ u.count > 1 ? "s" : "" }} ici</small>
              </span>
              <select class="sv-select compact" :aria-label="`Route pour ${u.name}`" @change="assignLocation(u.location, ($event.target as HTMLSelectElement).value)">
                <option value="" disabled selected>Rattacher à…</option>
                <option v-for="r in report.routes" :key="r.key" :value="r.key">{{ r.name }}</option>
              </select>
              <button type="button" class="sv-btn small" title="Cadeau ou rencontre fixe : ne compte pour aucune route" @click="assignLocation(u.location, '')">Pas une route</button>
            </div>
          </template>
          <template v-if="report.others.length">
            <h3 class="sv-section-title sub">Hors routes <Tip term="nuzlocke.others" /></h3>
            <div v-for="m in report.others" :key="m.key" class="mon-row" :class="{ dead: m.dead }">
              <Sprite :id="m.species" :shiny="m.shiny" :size="40" />
              <span class="m-txt">
                <strong>{{ monName(m) }}</strong>
                <small>{{ ORIGIN_LABEL[m.origin] }} · {{ m.metLocationName ?? "lieu inconnu" }} · {{ m.place }}</small>
              </span>
              <button v-if="manualAlive(m) || state.dead.includes(m.key)" type="button" class="sv-btn small" @click="resetMon(m)">Réinitialiser</button>
            </div>
          </template>
        </div>
      </section>

      <!-- Champions -->
      <section v-else-if="tab === 'caps'" class="body">
        <p class="sv-help">
          Équipes lues dans ta ROM : le niveau indiqué est celui du Pokémon le plus fort de chaque adversaire.
          <Tip term="nuzlocke.ace" />
        </p>
        <div v-if="!report.caps.length" class="sv-panel">
          <EmptyState compact icon="swords" title="Aucun champion trouvé">
            Kaleido n'a pas reconnu les champions de cette ROM : le niveau maximum ne peut pas être calculé.
          </EmptyState>
        </div>
        <div v-else class="caps">
          <div v-for="c in report.caps" :key="c.label + c.name" class="capc sv-panel" :class="{ beaten: c.beaten, current: c.current }">
            <Sprite :id="c.aceSpecies" :size="56" />
            <span class="m-txt">
              <small>{{ c.label }} · {{ c.town }}</small>
              <strong>{{ c.name }}</strong>
              <small>Niveau max N. {{ c.aceLevel }}</small>
            </span>
            <span v-if="c.beaten" class="sv-chip ok"><Icon name="check" :size="12" /> Battu</span>
            <span v-else-if="c.current" class="sv-chip on">Prochain</span>
            <span v-if="!c.verified" class="sv-chip warn">?<Tip term="nuzlocke.unverified" /></span>
          </div>
        </div>
      </section>

      <!-- Infractions -->
      <section v-else-if="tab === 'violations'" class="body">
        <h3 class="sv-section-title">Infractions <Tip term="nuzlocke.violations" /></h3>
        <div v-if="!report.violations.length" class="sv-panel">
          <EmptyState compact icon="shield" title="Aucune règle enfreinte">Bravo !</EmptyState>
        </div>
        <div v-for="(v, i) in report.violations" :key="i" class="viol sv-panel" :class="SEVERITY[v.severity].cls">
          <Icon :name="SEVERITY[v.severity].icon" :size="18" class="viol-icon" />
          <span class="m-txt">
            <strong>{{ v.title }}</strong>
            <small>{{ v.detail }}</small>
          </span>
          <span class="sv-chip" :class="SEVERITY[v.severity].cls">{{ SEVERITY[v.severity].label }}</span>
        </div>
        <p v-if="state.rules.noItemsInBattle || state.rules.setMode" class="sv-help">
          Les règles « pas de soins en combat » et « mode Set » ne peuvent pas être vérifiées dans la sauvegarde.
        </p>
      </section>

      <!-- Règles -->
      <section v-else class="body">
        <div class="sv-panel block">
          <h3 class="sv-section-title">Règles de la partie</h3>
          <div class="rules">
            <span v-for="r in RULES" :key="r.id" class="rule">
              <Toggle :model-value="state.rules[r.id]" :label="r.label" :term="`nuzlocke.${r.id}`" @update:model-value="setRule(r.id, $event)" />
              <span v-if="!r.check" class="sv-chip dim">rappel<Tip term="nuzlocke.reminder" /></span>
            </span>
          </div>
        </div>
        <div class="sv-panel block">
          <h3 class="sv-section-title">Boîte « Cimetière » <Tip term="nuzlocke.graveyard" /></h3>
          <select
            class="sv-select"
            aria-label="Boîte cimetière"
            :value="state.graveyardBox ?? ''"
            @change="setGraveyard(($event.target as HTMLSelectElement).value)"
          >
            <option value="">Automatique (nom contenant RIP, Cimetière, Morts…)</option>
            <option v-for="(n, i) in report.boxNames" :key="i" :value="i">{{ i + 1 }}. {{ n }}</option>
          </select>
          <p class="sv-help">Réglages enregistrés dans {{ fileName(data.stateFile) }}, à côté de la sauvegarde. La sauvegarde elle-même n'est pas modifiée.</p>
          <div class="sv-row">
            <button type="button" class="sv-btn" @click="pickRom()">Changer de ROM</button>
            <button type="button" class="sv-btn danger" @click="link(null)">Délier la ROM</button>
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
  gap: var(--sp-3);
  height: 100%;
  min-height: 0;
  padding: var(--sp-4) var(--sp-6);
  overflow-y: auto;
}

/* États « jeu non pris en charge » et « ROM à lier » : panneau centré. */
.gate {
  width: min(100%, 720px);
  margin: var(--sp-5) auto;
}

.gate :deep(.sv-empty) {
  max-width: none;
}

.steps {
  margin: 0;
  padding-left: var(--sp-5);
  color: var(--text-dim);
  font-size: var(--fs-md);
  line-height: 1.6;
  text-align: left;
}

.wide {
  align-self: stretch;
  text-align: left;
}

.cands {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  align-self: stretch;
}

.cand {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 5%, transparent);
  color: var(--text);
  text-align: left;
}

.cand:hover:not(:disabled) {
  background: var(--panel-hover);
}

.cand:disabled {
  opacity: 0.5;
}

.m-txt {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.m-txt small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.grow {
  flex: 1;
}



/* En-tête */
.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: var(--sp-3);
}

.stat {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-3) var(--sp-4);
}

.stat > strong {
  font-size: 26px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.stat > small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.stat.bad > strong {
  color: var(--danger);
}

.stat.off {
  opacity: 0.5;
}

.cap-row {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.cap-row strong {
  font-size: var(--fs-xl);
}

.rom-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  margin-top: var(--sp-3);
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.tabs {
  align-self: flex-start;
}

.count {
  margin-left: var(--sp-2);
  padding: 0 var(--sp-2);
  font-size: var(--fs-xs);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.body.two {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
  align-items: start;
}

.body > .sv-section-title {
  margin: 0;
}

.filters {
  justify-content: space-between;
}

/* Routes */
.routes {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.route {
  padding: var(--sp-3) var(--sp-4);
  border-radius: var(--radius-card);
}

.route.missed {
  opacity: 0.75;
}

.r-head {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.r-head strong {
  font-size: var(--fs-lg);
}

.r-head .sv-btn {
  margin-left: auto;
}

.r-body {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
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
  gap: var(--sp-2);
}

.mon,
.mon-row {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.mon.dead,
.mon-row.dead {
  filter: grayscale(1);
}

.mon-row {
  padding: var(--sp-2) 0;
  border-bottom: 1px solid var(--border);
}

.block {
  padding: var(--sp-4);
}

.block > .sv-help {
  margin-bottom: var(--sp-2);
}

.sub {
  margin-top: var(--sp-4);
}

/* Champions */
.caps {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: var(--sp-2);
}

.capc {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-3);
  border-radius: var(--radius-card);
}

.capc.beaten {
  opacity: 0.55;
}

.capc.current {
  border-color: var(--text);
}

/* Infractions : pastille de gravité et bordure discrète de la même couleur. */
.viol {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-4);
  border-radius: var(--radius-card);
}

.viol.danger {
  border-color: color-mix(in srgb, var(--danger) 55%, var(--border));
}

.viol.danger .viol-icon {
  color: var(--danger);
}

.viol.warn {
  border-color: color-mix(in srgb, var(--warn) 55%, var(--border));
}

.viol.warn .viol-icon {
  color: var(--warn);
}

.viol.dim .viol-icon {
  color: var(--text-dim);
}

/* Règles */
.rules {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--sp-3) var(--sp-5);
}

.rule {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}
</style>
