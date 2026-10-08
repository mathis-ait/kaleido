<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import { addFiles, allGames, games } from "../games";
import { formatMo } from "./play";

/**
 * Romhacks : installation en un clic. Kaleido télécharge le patch officiel de l'auteur,
 * l'applique à la ROM du joueur (vérifiée) et peut traduire le jeu en français
 * à l'aide de la version française officielle du même jeu.
 */

interface HackView {
  id: string;
  name: string;
  version: string;
  author: string;
  summary: string;
  threadUrl: string;
  notes: string[];
  baseLabel: string;
  base: string | null;
  baseMismatch: string[];
  french: boolean;
  referenceLabel: string | null;
  reference: string | null;
  installedEn: string | null;
  installedFr: string | null;
}
interface Progress {
  id: string;
  step: "download" | "extract" | "patch" | "translate" | "write";
  done: number;
  total: number;
}

const emit = defineEmits<{ close: [] }>();

const hacks = ref<HackView[] | null>(null);
const error = ref<string | null>(null);
/** ROMs choisies à la main, en plus de celles de la bibliothèque. */
const extra = reactive<string[]>([]);
const lang = reactive<Record<string, "fr" | "en">>({});
const progress = ref<Progress | null>(null);
const busy = ref<string | null>(null);
const done = reactive<Record<string, string>>({});

const roms = computed(() => [...new Set([...games.found.map((g) => g.path), ...allGames.value.map((g) => g.path), ...extra])].filter((p) => /\.nds$/i.test(p)));

async function refresh() {
  try {
    hacks.value = await invoke<HackView[]>("romhacks_list", { roms: roms.value });
    for (const h of hacks.value) lang[h.id] ??= h.french ? "fr" : "en";
  } catch (e) {
    error.value = String(e);
  }
}

let unlisten: (() => void) | null = null;
onMounted(async () => {
  unlisten = await listen<Progress>("romhack-install", (e) => (progress.value = e.payload)).catch(() => null);
  await refresh();
});
onBeforeUnmount(() => unlisten?.());

async function pickRom(label: string) {
  const picked = await open({ title: `Choisir la ROM : ${label}`, filters: [{ name: "ROM Nintendo DS", extensions: ["nds"] }] });
  if (typeof picked === "string") {
    extra.push(picked);
    await refresh();
  }
}

const wantsFrench = (h: HackView) => h.french && lang[h.id] === "fr";
const ready = (h: HackView) => !!h.base && (!wantsFrench(h) || !!h.reference);
const installed = (h: HackView) => done[h.id] ?? (wantsFrench(h) ? h.installedFr : h.installedEn);

async function install(h: HackView) {
  if (!h.base) return;
  busy.value = h.id;
  error.value = null;
  progress.value = null;
  try {
    const out = await invoke<string>("romhack_install", { id: h.id, base: h.base, reference: wantsFrench(h) ? h.reference : null });
    done[h.id] = out;
    await addFiles([out]);
    await refresh();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = null;
    progress.value = null;
  }
}

function stepText(p: Progress | null) {
  if (!p || p.step === "download") return p?.total ? `Téléchargement du patch… ${formatMo(p.done)} / ${formatMo(p.total)}` : "Téléchargement du patch…";
  return { extract: "Ouverture de l'archive…", patch: "Application du patch…", translate: "Traduction en français…", write: "Écriture de la ROM…" }[p.step];
}
const percent = (p: Progress | null) => (p?.step === "download" && p.total ? Math.round((p.done / p.total) * 100) : null);
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && !busy.value) emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="rh-overlay" @mousedown.self="!busy && emit('close')">
    <section class="rh-dialog panel" role="dialog" aria-modal="true" aria-label="Romhacks">
      <header>
        <h2><Icon name="wand" :size="18" /> Romhacks</h2>
        <button class="icon-btn" aria-label="Fermer" :disabled="!!busy" @click="emit('close')"><Icon name="x" :size="18" /></button>
      </header>

      <div class="body">
        <p class="dim small">
          Kaleido télécharge le patch publié par l'auteur et l'applique à ta propre ROM : aucune ROM n'est téléchargée. La version française est construite à partir de ta ROM
          française officielle du même jeu.
        </p>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-if="!hacks && !error" class="dim">Recherche de tes ROMs…</p>

        <article v-for="h in hacks ?? []" :key="h.id" class="hack">
          <div class="title">
            <h3>{{ h.name }} <span class="dim">v{{ h.version }}</span></h3>
            <span class="dim small">par {{ h.author }} · <button class="link" @click="openUrl(h.threadUrl)">fil officiel</button></span>
          </div>
          <p class="summary">{{ h.summary }}</p>

          <div v-if="h.french" class="lang" role="radiogroup" aria-label="Langue">
            <label :class="{ on: lang[h.id] === 'fr' }"><input v-model="lang[h.id]" type="radio" value="fr" :disabled="!!busy" /> Français (traduction Kaleido)</label>
            <label :class="{ on: lang[h.id] === 'en' }"><input v-model="lang[h.id]" type="radio" value="en" :disabled="!!busy" /> Anglais (original)</label>
          </div>

          <ul class="needs">
            <li>
              <Icon :name="h.base ? 'check' : 'alert'" :size="14" :class="h.base ? 'ok' : 'warn'" />
              <span class="need-main">
                <strong>{{ h.baseLabel }}</strong>
                <small v-if="h.base" class="dim path">{{ fileName(h.base) }}</small>
                <small v-else-if="h.baseMismatch.length" class="warn">Trouvée mais modifiée ou d'une autre révision : il faut une copie intacte de la cartouche.</small>
                <small v-else class="dim">Introuvable dans ta bibliothèque.</small>
              </span>
              <button v-if="!h.base" class="btn" :disabled="!!busy" @click="pickRom(h.baseLabel)"><Icon name="folder" :size="14" /> Choisir…</button>
            </li>
            <li v-if="wantsFrench(h)">
              <Icon :name="h.reference ? 'check' : 'alert'" :size="14" :class="h.reference ? 'ok' : 'warn'" />
              <span class="need-main">
                <strong>{{ h.referenceLabel }}</strong>
                <small v-if="h.reference" class="dim path">{{ fileName(h.reference) }} — source des textes officiels français</small>
                <small v-else class="dim">Nécessaire pour la version française (textes officiels du jeu).</small>
              </span>
              <button v-if="!h.reference" class="btn" :disabled="!!busy" @click="pickRom(h.referenceLabel ?? '')"><Icon name="folder" :size="14" /> Choisir…</button>
            </li>
          </ul>

          <ul v-if="h.notes.length" class="notes">
            <li v-for="n in h.notes" :key="n"><Icon name="info" :size="14" /> {{ n }}</li>
          </ul>

          <div v-if="busy === h.id" class="progress">
            <span class="dim small">{{ stepText(progress) }}</span>
            <div class="bar" :class="{ indeterminate: percent(progress) === null }"><div :style="percent(progress) !== null ? { width: percent(progress) + '%' } : undefined" /></div>
          </div>

          <div class="foot">
            <template v-if="installed(h)">
              <span class="ok small"><Icon name="check" :size="14" /> Installé : {{ fileName(installed(h)!) }}</span>
              <button class="btn" @click="revealItemInDir(installed(h)!)"><Icon name="folder-open" :size="14" /> Afficher</button>
            </template>
            <button class="btn btn-primary" :disabled="!ready(h) || !!busy" @click="install(h)">
              <Icon name="download" :size="15" /> {{ busy === h.id ? "Installation…" : installed(h) ? "Réinstaller" : "Installer" }}
            </button>
          </div>
        </article>
      </div>
    </section>
  </div>
</template>

<style scoped>
.rh-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.rh-dialog {
  display: flex;
  flex-direction: column;
  width: min(720px, 100%);
  max-height: calc(100vh - 56px);
  overflow: hidden;
  background: var(--surface);
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border);
}

h2 {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 18px;
}

.body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px;
  overflow-y: auto;
}

.hack {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
}

.title {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 6px 12px;
}

h3 {
  margin: 0;
  font-size: 16px;
}

.summary {
  margin: 0;
  line-height: 1.45;
}

.lang {
  display: inline-flex;
  align-self: flex-start;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.lang label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  font-size: 13px;
  cursor: pointer;
}

.lang label + label {
  border-left: 1px solid var(--border);
}

.lang label.on {
  background: var(--panel-hover);
  font-weight: 600;
}

.lang input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.needs,
.notes {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.needs li {
  display: flex;
  align-items: center;
  gap: 10px;
}

.need-main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.notes li {
  display: flex;
  gap: 8px;
  color: var(--text-dim);
  font-size: 12.5px;
  line-height: 1.45;
}

.notes :deep(svg) {
  flex: none;
  margin-top: 2px;
}

.foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}

.foot .ok {
  margin-right: auto;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font: inherit;
  cursor: pointer;
}

.path {
  overflow-wrap: anywhere;
}

.ok {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--ok);
}

.warn {
  color: var(--warning, var(--danger));
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: 0;
  font-size: 12px;
}

.error {
  margin: 0;
  color: var(--danger);
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-hover);
}

.bar div {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.2s;
}

.bar.indeterminate div {
  width: 35%;
  animation: slide 1.1s ease-in-out infinite alternate;
}

@keyframes slide {
  from {
    transform: translateX(-20%);
  }
  to {
    transform: translateX(210%);
  }
}
</style>
