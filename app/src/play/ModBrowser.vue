<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import { browseMods, formatCount, formatSize, modCategories, modDetails, type GbCategory, type GbItem, type GbProfile } from "./mods";

/**
 * Explorateur de tous les mods GameBanana d'un jeu : recherche, tri, catégories, fiche
 * détaillée et installation d'un fichier au choix.
 */
const props = defineProps<{ game: number; installed: number[]; busy: string | null }>();
const emit = defineEmits<{ install: [mod: GbProfile, file: number] }>();

const query = ref("");
const sort = ref("popular");
const category = ref<number | null>(null);
const categories = ref<GbCategory[]>([]);
const items = ref<GbItem[]>([]);
const page = ref(1);
const total = ref(0);
const complete = ref(false);
const hidden = ref(0);
const loading = ref(false);
const error = ref<string | null>(null);

const detail = ref<GbProfile | null>(null);
const detailLoading = ref<number | null>(null);
const shot = ref(0);

const SORTS = [
  { value: "popular", label: "Les plus téléchargés" },
  { value: "liked", label: "Les mieux notés" },
  { value: "new", label: "Les plus récents" },
  { value: "updated", label: "Mis à jour récemment" },
];

let request = 0;
async function load(reset: boolean) {
  const id = ++request;
  if (reset) {
    page.value = 1;
    items.value = [];
  }
  loading.value = true;
  error.value = null;
  try {
    const p = await browseMods(props.game, query.value, sort.value, query.value.trim() ? null : category.value, page.value);
    if (id !== request) return;
    items.value = reset ? p.items : [...items.value, ...p.items.filter((i) => !items.value.some((x) => x.id === i.id))];
    total.value = p.total;
    complete.value = p.complete || p.items.length === 0;
    hidden.value = reset ? p.hidden : hidden.value + p.hidden;
  } catch (e) {
    if (id === request) error.value = String(e);
  } finally {
    if (id === request) loading.value = false;
  }
}

function more() {
  page.value++;
  load(false);
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch(query, () => {
  clearTimeout(timer);
  timer = setTimeout(() => load(true), 350);
});
watch([sort, category], () => load(true));

onMounted(async () => {
  load(true);
  categories.value = await modCategories(props.game).catch(() => []);
});

async function open(item: GbItem) {
  detailLoading.value = item.id;
  try {
    detail.value = await modDetails(item.id);
    shot.value = 0;
  } catch (e) {
    error.value = String(e);
  } finally {
    detailLoading.value = null;
  }
}

const files = computed(() => [...(detail.value?.files ?? [])].sort((a, b) => b.date - a.date));
const date = (ts: number) => new Date(ts * 1000).toLocaleDateString("fr-FR", { day: "numeric", month: "short", year: "numeric" });
const isInstalled = (id: number) => props.installed.includes(id);
</script>

<template>
  <div class="browser">
    <template v-if="!detail">
      <div class="filters">
        <SearchField v-model="query" class="search" placeholder="Chercher un mod (scale, texture, shiny…)" />
        <select v-model="sort" class="sv-input sort" :disabled="!!query.trim()" aria-label="Tri">
          <option v-for="s in SORTS" :key="s.value" :value="s.value">{{ s.label }}</option>
        </select>
      </div>
      <div v-if="categories.length && !query.trim()" class="cats">
        <button class="sv-chip" :class="{ on: category === null }" @click="category = null">Tout</button>
        <button v-for="c in categories" :key="c.id" class="sv-chip" :class="{ on: category === c.id }" @click="category = c.id">
          {{ c.name }} <span class="count">{{ c.count }}</span>
        </button>
      </div>
      <p v-if="error" class="error">{{ error }}</p>

      <div class="grid">
        <button v-for="i in items" :key="i.id" class="card" :class="{ on: isInstalled(i.id) }" @click="open(i)">
          <div class="shot">
            <img v-if="i.thumbnail" :src="i.thumbnail" alt="" loading="lazy" referrerpolicy="no-referrer" />
            <span v-if="isInstalled(i.id)" class="sv-chip ok"><Icon name="check" :size="11" /> Installé</span>
          </div>
          <div class="info">
            <strong :title="i.name">{{ i.name }}</strong>
            <small>{{ i.author }}</small>
            <small class="stats">
              <span><Icon name="heart" :size="12" /> {{ i.likes }}</span>
              <span>{{ formatCount(i.views) }} vues</span>
              <span v-if="i.obsolete" class="warn">obsolète</span>
              <span v-if="!i.hasFiles" class="warn">sans fichier</span>
            </small>
          </div>
          <span v-if="detailLoading === i.id" class="loading">Ouverture…</span>
        </button>
      </div>

      <p v-if="loading" class="dim center">Chargement…</p>
      <p v-else-if="!items.length && !error" class="dim center">Aucun mod trouvé.</p>
      <div v-if="!loading && !complete && items.length" class="center">
        <button class="sv-btn" @click="more">Afficher plus</button>
      </div>
      <p v-if="hidden" class="dim small center">{{ hidden }} mod{{ hidden > 1 ? "s" : "" }} marqué{{ hidden > 1 ? "s" : "" }} « contenu sensible » par GameBanana masqué{{ hidden > 1 ? "s" : "" }}.</p>
    </template>

    <!-- Fiche détaillée -->
    <section v-else class="detail">
      <button class="sv-btn back" @click="detail = null"><Icon name="undo" :size="14" /> Retour à la liste</button>
      <div class="head">
        <div class="gallery">
          <img v-if="detail.images.length" :src="detail.images[shot]" alt="" referrerpolicy="no-referrer" />
          <div v-if="detail.images.length > 1" class="dots">
            <button v-for="(_, n) in detail.images" :key="n" :class="{ on: n === shot }" :aria-label="`Image ${n + 1}`" @click="shot = n" />
          </div>
        </div>
        <div class="about">
          <h3>{{ detail.name }}</h3>
          <p class="dim">
            {{ detail.author }} · {{ detail.category }} · <Icon name="heart" :size="12" /> {{ detail.likes }} · {{ formatCount(detail.downloads) }} téléchargements · mis à jour le
            {{ date(detail.updated) }}
          </p>
          <p v-if="detail.obsolete" class="warn"><Icon name="alert" :size="13" /> L'auteur l'a marqué obsolète.</p>
          <button class="link" @click="openUrl(`https://gamebanana.com/mods/${detail.id}`)">Ouvrir la page GameBanana</button>
        </div>
      </div>
      <p class="text">{{ detail.text || "Pas de description." }}</p>
      <p class="dim small">Description de l'auteur, en anglais le plus souvent. Lis-la : certains mods demandent une version précise du jeu.</p>

      <h4>Fichiers</h4>
      <p v-if="!files.length" class="dim">Ce mod n'a pas de fichier téléchargeable.</p>
      <ul class="files">
        <li v-for="f in files" :key="f.id">
          <div>
            <strong>{{ f.name }}</strong>
            <small class="dim">{{ formatSize(f.size) }} · {{ date(f.date) }} · {{ formatCount(f.downloads) }} téléchargements</small>
            <small v-if="f.description" class="dim">{{ f.description }}</small>
          </div>
          <button class="sv-btn solid" :disabled="!!busy" @click="emit('install', detail, f.id)">
            {{ busy === `gb:${detail.id}` ? "Installation…" : isInstalled(detail.id) ? "Réinstaller" : "Installer" }}
          </button>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.browser {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.filters {
  display: flex;
  gap: 10px;
}

.search {
  flex: 1;
  min-width: 0;
}

.sort {
  flex: none;
  width: 230px;
}

.cats {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.count {
  opacity: 0.6;
  font-weight: 500;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(196px, 1fr));
  gap: 12px;
}

.card {
  position: relative;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  color: var(--text);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s;
}

.card:hover {
  border-color: color-mix(in srgb, var(--text) 40%, var(--border));
  transform: translateY(-1px);
}

.card.on {
  border-color: color-mix(in srgb, var(--ok) 45%, var(--border));
}

.shot {
  position: relative;
  aspect-ratio: 16 / 9;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.shot img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.shot .sv-chip {
  position: absolute;
  top: 6px;
  left: 6px;
  font-size: 11px;
}

.sv-chip.ok {
  background: var(--surface);
  color: var(--ok);
}

.info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px 10px;
}

.info strong {
  overflow: hidden;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.info small {
  color: var(--text-dim);
  font-size: 12px;
}

.stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.stats span {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--surface) 70%, transparent);
  font-size: 13px;
}

.detail {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.back {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.head {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
  gap: 16px;
}

.gallery img {
  width: 100%;
  aspect-ratio: 16 / 9;
  object-fit: cover;
  border-radius: calc(var(--radius) - 4px);
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.dots {
  display: flex;
  justify-content: center;
  gap: 6px;
  margin-top: 6px;
}

.dots button {
  width: 8px;
  height: 8px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 25%, transparent);
  cursor: pointer;
}

.dots button.on {
  background: var(--text);
}

.about h3 {
  margin: 0 0 6px;
  font-size: 18px;
}

.about p {
  margin: 0 0 6px;
  font-size: 13px;
}

.text {
  max-height: 240px;
  margin: 0;
  overflow-y: auto;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-size: 13px;
  line-height: 1.5;
  white-space: pre-line;
}

h4 {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: 12px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.files {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.files li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.files li div {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  font-size: 13px;
}

.files strong {
  overflow-wrap: anywhere;
}

.warn {
  color: var(--warn);
}

.error {
  color: var(--danger);
}

.dim {
  color: var(--text-dim);
}

.small {
  font-size: 12px;
}

.center {
  text-align: center;
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--text-dim);
  font: inherit;
  font-size: 13px;
  text-decoration: underline;
  cursor: pointer;
}

.link:hover {
  color: var(--text);
}

@media (max-width: 720px) {
  .head {
    grid-template-columns: 1fr;
  }
}
</style>
