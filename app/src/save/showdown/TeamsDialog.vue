<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../../components/Icon.vue";
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
  try {
    const report = await importShowdown(team.value.paste, target);
    const failed = report.sets.filter((s) => s.error).length;
    done.value = `${report.imported} Pokémon importé${report.imported > 1 ? "s" : ""}${failed ? `, ${failed} impossible${failed > 1 ? "s" : ""} dans ce jeu` : ""}.`;
  } catch (e) {
    saveState.error = String(e);
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

function onKey(e: KeyboardEvent) {
  if (!showdownUi.teams) return;
  e.stopPropagation();
  if (keyOf(e) === "Escape") {
    e.preventDefault();
    close();
  }
}
onMounted(() => window.addEventListener("keydown", onKey, true));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  showdownUi.teams = false;
});
</script>

<template>
  <Teleport to="body">
    <Transition name="tm-fade">
      <div v-if="showdownUi.teams" class="tm-overlay" @pointerdown.self="close">
        <section class="tm-dialog sv-panel" role="dialog" aria-modal="true" aria-label="Équipes stratégiques">
          <header class="tm-head">
            <Icon name="swords" :size="22" />
            <div class="title">
              <h2>Équipes stratégiques <Tip term="smogonTeams" /></h2>
              <small>Équipes d'exemple de Smogon, prêtes à jouer</small>
            </div>
            <span class="grow" />
            <button class="round" title="Actualiser la liste" :disabled="loadingList" @click="loadList(true)"><Icon name="refresh" :size="15" /></button>
            <button class="round" aria-label="Fermer" @click="close"><Icon name="x" :size="16" /></button>
          </header>

          <nav class="filters">
            <span class="sv-label">Génération</span>
            <div class="sv-seg">
              <button v-for="g in gens" :key="g" :class="{ on: gen === g }" @click="gen = g">Gen {{ g }}</button>
            </div>
            <span class="sv-label">Format <Tip term="smogonFormat" /></span>
            <div class="sv-seg">
              <button v-for="f in FORMATS[gen]" :key="f.id" :class="{ on: format === f.id }" @click="format = f.id">{{ f.label }}</button>
            </div>
          </nav>

          <div class="tm-body">
            <!-- Liste des équipes -->
            <ul class="teams">
              <li v-if="loadingList" class="state"><Icon name="refresh" :size="24" class="spin" /> Chargement…</li>
              <li v-else-if="listError" class="state">
                <p><strong>Équipes indisponibles.</strong> Kaleido a besoin d'Internet la première fois ; les équipes sont ensuite gardées hors ligne.</p>
                <p class="dim small">{{ listError }}</p>
                <button class="sv-btn" @click="loadList(true)"><Icon name="refresh" :size="14" /> Réessayer</button>
              </li>
              <li v-else-if="!list.length" class="state dim">Aucune équipe d'exemple pour ce format.</li>
              <template v-else>
                <li v-for="t in list" :key="t.slug">
                <button class="team-item" :class="{ on: team?.slug === t.slug }" @click="openTeam(t)">
                  <strong>{{ t.name }}</strong>
                  <small class="dim">{{ t.author ?? "Anonyme" }}<template v-if="t.views"> · {{ t.views }} vues</template></small>
                  <span class="mini">
                    <Sprite v-for="(id, i) in species[t.slug] ?? []" :key="i" :id="id" :size="34" />
                  </span>
                  <Icon v-if="loadingTeam === t.slug" name="refresh" :size="14" class="spin side" />
                </button>
                </li>
              </template>
            </ul>

            <!-- Aperçu de l'équipe -->
            <div class="detail">
              <div v-if="teamError" class="state">
                <p><strong>Impossible d'ouvrir cette équipe.</strong></p>
                <p class="dim small">{{ teamError }}</p>
              </div>
              <template v-else-if="team && preview">
                <div class="detail-head">
                  <div>
                    <h3>{{ team.name }}</h3>
                    <small class="dim">par {{ team.author ?? "anonyme" }} · {{ team.format }}</small>
                  </div>
                  <span class="grow" />
                  <button v-if="team.sourceUrl" class="sv-btn" title="Fil Smogon d'où vient l'équipe" @click="openUrl(team.sourceUrl)">
                    <Icon name="book" :size="14" /> Source Smogon
                  </button>
                  <button class="sv-btn" title="Voir l'équipe sur crob.at" @click="openUrl(team.url)"><Icon name="send" :size="14" /> crob.at</button>
                </div>
                <ul class="mons">
                  <li v-for="(s, i) in preview.sets" :key="i" class="mon" :class="{ bad: !!s.error }">
                    <Sprite :id="s.species" :size="64" class="mon-sprite" />
                    <div class="mon-main">
                      <strong>{{ s.speciesName }}</strong>
                      <small class="dim">{{ s.itemName ?? "Sans objet" }} · {{ s.abilityName ?? "—" }} · {{ s.natureName ?? "—" }}</small>
                      <small v-if="statLine(s.evs, 0)" class="dim">EV : {{ statLine(s.evs, 0) }}</small>
                      <div class="moves">
                        <span v-for="m in s.moveNames" :key="m" class="mv">{{ m }}</span>
                      </div>
                      <p v-if="s.error" class="err"><Icon name="alert" :size="13" /> {{ s.error }}</p>
                      <template v-else>
                        <p v-for="w in s.warnings" :key="w" class="warn"><Icon name="alert" :size="13" /> {{ w }}</p>
                      </template>
                    </div>
                  </li>
                </ul>
                <div class="actions">
                  <p v-if="done" class="ok"><Icon name="check" :size="14" /> {{ done }}</p>
                  <span class="grow" />
                  <button class="sv-btn" :disabled="importing || !valid || partyFree <= 0" :title="partyFree <= 0 ? 'Équipe pleine' : ''" @click="importTo({ kind: 'party' })">
                    <Icon name="plus" :size="14" /> Dans l'équipe
                  </button>
                  <button class="sv-btn solid" :disabled="importing || !valid" @click="importTo({ kind: 'box', box: saveState.box })">
                    <Icon name="download" :size="14" /> Importer dans {{ boxName }}
                  </button>
                </div>
              </template>
              <div v-else-if="loadingTeam" class="state"><Icon name="refresh" :size="24" class="spin" /></div>
              <div v-else class="state dim">Choisis une équipe à gauche.</div>
            </div>
          </div>

          <footer class="tm-foot">
            <p class="dim small">
              Équipes d'exemple publiées par Smogon University, récupérées via crob.at. Les Pokémon sont créés à ton nom ; ce qui n'existe pas dans ton jeu
              (attaque, objet, talent) est signalé. Ctrl+Z pour annuler.
            </p>
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.tm-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.tm-dialog {
  display: flex;
  flex-direction: column;
  width: min(1180px, 100%);
  height: min(820px, calc(100vh - 56px));
  overflow: hidden;
  background: var(--surface);
}

.tm-head,
.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 12px 18px;
  border-bottom: 1px solid var(--border);
}

.title h2 {
  margin: 0;
  font-size: 19px;
}

.title small,
.dim {
  color: var(--text-dim);
}

.small {
  font-size: 12px;
}

.grow {
  flex: 1;
}

.round {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: transparent;
  color: var(--text);
}

.tm-body {
  display: grid;
  flex: 1;
  grid-template-columns: 280px minmax(0, 1fr);
  min-height: 0;
}

.teams {
  margin: 0;
  padding: 8px;
  overflow: auto;
  list-style: none;
  border-right: 1px solid var(--border);
}

.team-item {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: 9px 30px 9px 12px;
  border: none;
  border-radius: 10px;
  background: none;
  color: var(--text);
  text-align: left;
}

.team-item:hover {
  background: color-mix(in srgb, var(--text) 7%, transparent);
}

.team-item.on {
  background: color-mix(in srgb, var(--accent) 18%, transparent);
  box-shadow: inset 3px 0 0 var(--accent);
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
  top: 12px;
  right: 10px;
}

.detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  overflow: auto;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.detail-head h3 {
  margin: 0;
  font-size: 18px;
}

.mons {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.mon {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 14px;
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

.mon.bad {
  border-color: var(--danger);
}

.mon-sprite {
  flex: none;
  margin: -6px -4px;
}

.mon-main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.mon-main small {
  font-size: 12px;
}

.moves {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 2px;
}

.mv {
  padding: 2px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  font-size: 12px;
  font-weight: 600;
}

.warn,
.err,
.ok {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  font-size: 12px;
}

.warn {
  color: var(--warn);
}

.err {
  color: var(--danger);
}

.ok {
  color: var(--accent-2);
  font-size: 13px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: auto;
}

.state {
  display: grid;
  place-items: center;
  gap: 6px;
  padding: 30px 16px;
  text-align: center;
}

.state p {
  margin: 0;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.tm-foot {
  padding: 10px 18px;
  border-top: 1px solid var(--border);
}

.tm-foot p {
  margin: 0;
}

.tm-fade-enter-active,
.tm-fade-leave-active {
  transition: opacity 0.15s;
}

.tm-fade-enter-from,
.tm-fade-leave-to {
  opacity: 0;
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
