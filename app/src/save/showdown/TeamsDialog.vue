<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import Banner from "../../components/Banner.vue";
import Dialog from "../../components/Dialog.vue";
import EmptyState from "../../components/EmptyState.vue";
import Icon from "../../components/Icon.vue";
import Segmented from "../../components/Segmented.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { saveState } from "../../saveStore";
import { keyOf } from "../shell";
import { importShowdown, previewShowdown, showdownUi, statLine, type ImportTarget, type ShowdownPreview } from "./api";

/**
 * Fenêtre « Équipes stratégiques » : équipes d'exemple de Smogon (via crob.at),
 * par génération et par format, à importer en un clic dans la sauvegarde.
 */

interface TeamSummary {
  slug: string;
  name: string;
  author: string | null;
  views: number | null;
  sourceUrl: string | null;
}

interface Team {
  slug: string;
  name: string;
  author: string | null;
  format: string;
  paste: string;
  url: string;
  sourceUrl: string | null;
}

/** Formats proposés par génération (ceux qui ont des équipes d'exemple chez Smogon). */
const FORMATS: Record<number, { id: string; label: string }[]> = {
  3: [
    { id: "ou", label: "OU" },
    { id: "uu", label: "UU" },
  ],
  4: [
    { id: "ou", label: "OU" },
    { id: "uu", label: "UU" },
    { id: "lc", label: "LC" },
    { id: "doublesou", label: "Doubles" },
  ],
  5: [
    { id: "ou", label: "OU" },
    { id: "uu", label: "UU" },
    { id: "ru", label: "RU" },
    { id: "doublesou", label: "Doubles" },
  ],
  6: [
    { id: "ou", label: "OU" },
    { id: "uu", label: "UU" },
    { id: "ru", label: "RU" },
    { id: "doublesou", label: "Doubles" },
  ],
  7: [
    { id: "ou", label: "OU" },
    { id: "uu", label: "UU" },
    { id: "ru", label: "RU" },
  ],
};

const saveGen = computed(() => saveState.view?.generation ?? 5);
/** Générations jouables dans la sauvegarde : la sienne et les précédentes. */
const gens = computed(() => Object.keys(FORMATS).map(Number).filter((g) => g <= saveGen.value).reverse());
const gen = ref(5);
const format = ref("ou");
const formatId = computed(() => `gen${gen.value}${format.value}`);

const list = ref<TeamSummary[]>([]);
const listError = ref<string | null>(null);
const loadingList = ref(false);

const team = ref<Team | null>(null);
const preview = ref<ShowdownPreview | null>(null);
const loadingTeam = ref<string | null>(null);
const teamError = ref<string | null>(null);
const importing = ref(false);
const done = ref<string | null>(null);
const importError = ref<string | null>(null);

const genOptions = computed(() => gens.value.map((g) => ({ value: g, label: `Gen ${g}` })));
const formatOptions = computed(() => (FORMATS[gen.value] ?? []).map((f) => ({ value: f.id, label: f.label })));

async function loadList(refresh = false) {
  loadingList.value = true;
  listError.value = null;
  const forFormat = formatId.value;
  try {
    const result = await invoke<TeamSummary[]>("teams_list", { format: forFormat, refresh });
    // Réponse d'un format qu'on a quitté entre-temps : ignorée.
    if (forFormat !== formatId.value) return;
    list.value = result;
    if (list.value.length && !list.value.some((t) => t.slug === team.value?.slug)) openTeam(list.value[0]);
    fillSprites(list.value);
  } catch (e) {
    if (forFormat !== formatId.value) return;
    listError.value = String(e);
    list.value = [];
  } finally {
    if (forFormat === formatId.value) loadingList.value = false;
  }
}

/** Équipes déjà lues (texte + aperçu), par identifiant. */
const loaded = new Map<string, { full: Team; pv: ShowdownPreview }>();
/** Espèces de chaque équipe, pour la distinguer dans la liste (beaucoup s'appellent « Sample Team »). */
const species = reactive<Record<string, number[]>>({});

async function fetchTeam(slug: string) {
  let t = loaded.get(slug);
  if (!t) {
    const full = await invoke<Team>("teams_get", { slug });
    t = { full, pv: await previewShowdown(full.paste) };
    loaded.set(slug, t);
    species[slug] = t.pv.sets.map((s) => s.species);
  }
  return t;
}

/** Lit les équipes de la liste une par une, en arrière-plan. */
async function fillSprites(teams: TeamSummary[]) {
  const forFormat = formatId.value;
  for (const t of teams) {
    if (!showdownUi.teams || formatId.value !== forFormat) return;
    await fetchTeam(t.slug).catch(() => undefined);
  }
}

let openedSlug = "";
async function openTeam(t: TeamSummary) {
  openedSlug = t.slug;
  loadingTeam.value = t.slug;
  teamError.value = null;
  importError.value = null;
  done.value = null;
  try {
    const { full, pv } = await fetchTeam(t.slug);
    if (openedSlug !== t.slug) return;
    team.value = full;
    preview.value = pv;
  } catch (e) {
    if (openedSlug === t.slug) teamError.value = String(e);
  } finally {
    if (openedSlug === t.slug) loadingTeam.value = null;
  }
}

const valid = computed(() => preview.value?.sets.filter((s) => !s.error).length ?? 0);
const boxName = computed(() => saveState.view?.boxNames[saveState.box] ?? `Boîte ${saveState.box + 1}`);
const partyFree = computed(() => 6 - (saveState.view?.party.length ?? 0));

async function importTo(target: ImportTarget) {
  if (!team.value) return;
  importing.value = true;
  importError.value = null;
  try {
    const report = await importShowdown(team.value.paste, target);
    const failed = report.sets.filter((s) => s.error).length;
    done.value = `${report.imported} Pokémon importé${report.imported > 1 ? "s" : ""}${failed ? `, ${failed} impossible${failed > 1 ? "s" : ""} dans ce jeu` : ""}.`;
  } catch (e) {
    importError.value = String(e);
  } finally {
    importing.value = false;
  }
}

watch(
  () => showdownUi.teams,
  (open) => {
    if (!open) return;
    if (!gens.value.includes(gen.value)) gen.value = gens.value[0] ?? 5;
    loadList();
  },
  { immediate: true },
);

watch(gen, () => {
  if (!FORMATS[gen.value]?.some((f) => f.id === format.value)) format.value = "ou";
});
watch(formatId, () => {
  openedSlug = "";
  loadingTeam.value = null;
  team.value = null;
  preview.value = null;
  if (showdownUi.teams) loadList();
});

function close() {
  showdownUi.teams = false;
}

/** Fenêtre ouverte : les raccourcis de la page sont suspendus (Échap et Tab restent à la fenêtre). */
function onKey(e: KeyboardEvent) {
  if (!showdownUi.teams) return;
  e.stopPropagation();
  if (keyOf(e) === "Escape") {
    e.preventDefault();
    close();
  }
}
onMounted(() => document.addEventListener("keydown", onKey));
onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKey);
  showdownUi.teams = false;
});
</script>

<template>
  <Dialog v-model="showdownUi.teams" title="Équipes stratégiques" term="smogonTeams" subtitle="Équipes d'exemple de Smogon, prêtes à jouer" icon="swords" :width="1180">
    <template #head>
      <button type="button" class="sv-round sq" title="Actualiser la liste" aria-label="Actualiser la liste" :disabled="loadingList" @click="loadList(true)">
        <Icon name="refresh" :size="15" />
      </button>
    </template>

    <div class="tm-wrap">
      <nav class="filters">
        <span class="sv-label">Génération</span>
        <Segmented v-model="gen" :options="genOptions" label="Génération" />
        <span class="sv-label">Format <Tip term="smogonFormat" /></span>
        <Segmented v-model="format" :options="formatOptions" label="Format" />
      </nav>

      <div class="tm-body">
        <!-- Liste des équipes -->
        <div class="teams">
          <EmptyState v-if="loadingList" loading compact title="Chargement…" />
          <EmptyState v-else-if="listError" icon="alert" compact title="Équipes indisponibles">
            Kaleido a besoin d'Internet la première fois ; les équipes sont ensuite gardées hors ligne.
            <template #details>
              <p class="sv-help">{{ listError }}</p>
            </template>
            <template #actions>
              <button type="button" class="sv-btn" @click="loadList(true)"><Icon name="refresh" :size="14" /> Réessayer</button>
            </template>
          </EmptyState>
          <EmptyState v-else-if="!list.length" icon="search" compact>Aucune équipe d'exemple pour ce format.</EmptyState>
          <ul v-else class="team-list">
            <li v-for="t in list" :key="t.slug">
              <button type="button" class="team-item" :class="{ on: team?.slug === t.slug }" :aria-pressed="team?.slug === t.slug" @click="openTeam(t)">
                <strong>{{ t.name }}</strong>
                <small class="dim">{{ t.author ?? "Anonyme" }}<template v-if="t.views"> · {{ t.views }} vues</template></small>
                <span class="mini">
                  <Sprite v-for="(id, i) in species[t.slug] ?? []" :key="i" :id="id" :size="34" />
                </span>
                <Icon v-if="loadingTeam === t.slug" name="refresh" :size="14" class="sv-spin side" />
              </button>
            </li>
          </ul>
        </div>

        <!-- Aperçu de l'équipe -->
        <div class="detail">
          <EmptyState v-if="teamError" icon="alert" title="Impossible d'ouvrir cette équipe">
            {{ teamError }}
          </EmptyState>
          <template v-else-if="team && preview">
            <div class="detail-head">
              <div>
                <h3>{{ team.name }}</h3>
                <small class="dim">par {{ team.author ?? "anonyme" }} · {{ team.format }}</small>
              </div>
              <span class="grow" />
              <button v-if="team.sourceUrl" type="button" class="sv-btn" title="Fil Smogon d'où vient l'équipe" @click="openUrl(team.sourceUrl)">
                <Icon name="book" :size="14" /> Source Smogon
              </button>
              <button type="button" class="sv-btn" title="Voir l'équipe sur crob.at" @click="openUrl(team.url)"><Icon name="send" :size="14" /> crob.at</button>
            </div>
            <ul class="mons">
              <li v-for="(s, i) in preview.sets" :key="i" class="mon" :class="{ bad: !!s.error }">
                <Sprite :id="s.species" :size="64" class="mon-sprite" />
                <div class="mon-main">
                  <strong>{{ s.speciesName }}</strong>
                  <dl class="sv-dl">
                    <dt>Objet tenu <Tip term="heldItem" /></dt>
                    <dd>{{ s.itemName ?? "Aucun" }}</dd>
                    <dt>Talent <Tip term="ability" /></dt>
                    <dd>{{ s.abilityName ?? "—" }}</dd>
                    <dt>Nature <Tip term="nature" /></dt>
                    <dd>{{ s.natureName ?? "—" }}</dd>
                    <template v-if="statLine(s.evs, 0)">
                      <dt>EV <Tip term="ev" /></dt>
                      <dd>{{ statLine(s.evs, 0) }}</dd>
                    </template>
                  </dl>
                  <ul class="sv-moves">
                    <li v-for="m in s.moveNames" :key="m">{{ m }}</li>
                  </ul>
                  <p v-if="s.error" class="err"><Icon name="alert" :size="13" /> {{ s.error }}</p>
                  <template v-else>
                    <p v-for="w in s.warnings" :key="w" class="warn"><Icon name="alert" :size="13" /> {{ w }}</p>
                  </template>
                </div>
              </li>
            </ul>
            <Banner v-if="importError" :dismiss="() => (importError = null)">{{ importError }}</Banner>
            <div class="actions">
              <p v-if="done" class="ok"><Icon name="check" :size="14" /> {{ done }}</p>
              <span class="grow" />
              <button
                type="button"
                class="sv-btn"
                :disabled="importing || !valid || partyFree <= 0"
                :title="partyFree <= 0 ? 'Équipe pleine' : ''"
                @click="importTo({ kind: 'party' })"
              >
                <Icon name="plus" :size="14" /> Dans l'équipe
              </button>
              <button type="button" class="sv-btn solid" :disabled="importing || !valid" @click="importTo({ kind: 'box', box: saveState.box })">
                <Icon name="download" :size="14" /> Importer dans {{ boxName }}
              </button>
            </div>
          </template>
          <EmptyState v-else-if="loadingTeam" loading compact />
          <EmptyState v-else icon="swords" compact>Choisis une équipe à gauche.</EmptyState>
        </div>
      </div>
    </div>

    <template #foot>
      <p class="sv-help">
        Équipes d'exemple publiées par Smogon University, récupérées via crob.at. Les Pokémon sont créés à ton nom ; ce qui n'existe pas dans ton jeu
        (attaque, objet, talent) est signalé. Ctrl+Z pour annuler.
      </p>
    </template>
  </Dialog>
</template>

<style scoped>
/* Corps sans marge intérieure : la liste et l'aperçu défilent chacun de leur côté. */
.tm-wrap {
  display: flex;
  flex-direction: column;
  height: min(640px, calc(100vh - 240px));
  margin: calc(-1 * var(--sp-4)) calc(-1 * var(--sp-5));
}

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: var(--sp-3) var(--sp-5);
  border-bottom: 1px solid var(--border);
}

.dim {
  color: var(--text-dim);
}

.grow {
  flex: 1;
}

.tm-body {
  display: grid;
  flex: 1;
  grid-template-columns: 280px minmax(0, 1fr);
  min-height: 0;
}

.teams {
  min-height: 0;
  padding: var(--sp-2);
  overflow: auto;
  border-right: 1px solid var(--border);
}

.team-list {
  display: grid;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.team-item {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: 9px 30px 9px var(--sp-3);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--text);
  text-align: left;
}

.team-item:hover {
  background: color-mix(in srgb, var(--text) 7%, transparent);
}

/* Équipe ouverte : inversion texte / fond, comme les onglets. */
.team-item.on {
  border-color: var(--text);
  background: var(--text);
  color: var(--bg);
}

.team-item.on .dim {
  color: inherit;
  opacity: 0.75;
}

.mini {
  display: flex;
  min-height: 28px;
  margin: 2px 0 -4px -4px;
}

.mini > * {
  margin: -4px -3px;
}

.team-item .side {
  position: absolute;
  top: var(--sp-3);
  right: 10px;
}

.detail {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 0;
  padding: var(--sp-4) var(--sp-5);
  overflow: auto;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.detail-head h3 {
  margin: 0;
  font-size: var(--fs-lg);
}

.mons {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

/* Apparition discrète des fiches (fondu + léger glissement), sans halo. */
.mon {
  display: flex;
  gap: var(--sp-2);
  padding: 10px var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 4%, transparent);
  animation: pop 0.25s ease both;
}

.mon:nth-child(2) {
  animation-delay: 0.03s;
}
.mon:nth-child(3) {
  animation-delay: 0.06s;
}
.mon:nth-child(4) {
  animation-delay: 0.09s;
}
.mon:nth-child(5) {
  animation-delay: 0.12s;
}
.mon:nth-child(6) {
  animation-delay: 0.15s;
}

@keyframes pop {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .mon {
    animation: none;
  }
}

.mon.bad {
  border-color: var(--danger);
}

.mon-sprite {
  flex: none;
  margin: -6px -4px;
}

.mon-main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.warn,
.err,
.ok {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  font-size: var(--fs-sm);
}

.warn {
  color: var(--warn);
}

.err {
  color: var(--danger);
}

.ok {
  color: var(--ok);
  font-size: var(--fs-md);
}

/* Toujours visibles en bas de l'aperçu, même quand les six fiches défilent. */
.actions {
  position: sticky;
  bottom: calc(-1 * var(--sp-4));
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  margin: auto calc(-1 * var(--sp-5)) calc(-1 * var(--sp-4));
  padding: var(--sp-3) var(--sp-5);
  border-top: 1px solid var(--border);
  background: var(--surface);
}

@media (max-width: 760px) {
  .tm-body {
    grid-template-columns: 1fr;
  }

  .teams {
    max-height: 200px;
    border-right: none;
    border-bottom: 1px solid var(--border);
  }
}
</style>
