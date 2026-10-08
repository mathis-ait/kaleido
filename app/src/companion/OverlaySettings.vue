<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import { THEMES, currentTheme } from "../theme";

/**
 * Réglages → Overlay de stream : serveur local à coller dans OBS, une URL par mise en page.
 * Les options d'affichage sont dans l'URL : rien à régler dans OBS après le premier collage.
 */

interface OverlayStatus {
  enabled: boolean;
  port: number;
  activePort: number | null;
  token: string;
  error: string | null;
}

type Layout = "bar" | "card" | "deaths" | "ticker" | "next";

/** Taille conseillée de la source OBS à l'échelle 1. */
const LAYOUTS: { id: Layout; name: string; text: string; w: number; h: number }[] = [
  { id: "bar", name: "Équipe en ligne", text: "Les six Pokémon côte à côte, PV en barre.", w: 760, h: 200 },
  { id: "card", name: "Équipe en colonne", text: "Nom, niveau et PV de chaque Pokémon, l'un sous l'autre.", w: 320, h: 560 },
  { id: "deaths", name: "Compteur", text: "Morts, captures et badges.", w: 360, h: 100 },
  { id: "ticker", name: "Derniers évènements", text: "Captures, K.O., évolutions : les cinq derniers défilent.", w: 660, h: 64 },
  { id: "next", name: "Prochain combat", text: "Équipe du prochain champion et ton meilleur contre.", w: 560, h: 160 },
];

const SHOW_OPTIONS = [
  { id: "nickname", label: "Surnoms" },
  { id: "status", label: "Statuts" },
  { id: "item", label: "Objets tenus" },
  { id: "route", label: "Lieu actuel" },
];

const STORE = "kaleido.overlay";
const prefs = reactive({ theme: currentTheme.value, scale: 1, hp: "bar" as "bar" | "number" | "none", show: ["nickname", "status"] as string[], preview: "bar" as Layout });
try {
  Object.assign(prefs, JSON.parse(localStorage.getItem(STORE) ?? "{}"));
} catch {
  /* préférences illisibles : valeurs par défaut */
}
watch(prefs, () => {
  try {
    localStorage.setItem(STORE, JSON.stringify(prefs));
  } catch {
    /* stockage indisponible : les choix ne seront pas mémorisés */
  }
});

const status = ref<OverlayStatus | null>(null);
const busy = ref(false);
const error = ref<string | null>(null);
const port = ref(48080);
const copied = ref<Layout | null>(null);

async function load() {
  status.value = await invoke<OverlayStatus>("overlay_status").catch(() => null);
  if (status.value) port.value = status.value.port;
}

async function configure(enabled: boolean, newToken = false) {
  busy.value = true;
  error.value = null;
  try {
    status.value = await invoke<OverlayStatus>("overlay_configure", { enabled, port: port.value, newToken });
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

const enabled = computed({
  get: () => !!status.value?.enabled,
  set: (on: boolean) => void configure(on),
});

const running = computed(() => !!status.value?.enabled && status.value.activePort !== null);

function url(layout: Layout) {
  const s = status.value;
  if (!s?.activePort) return "";
  const p = new URLSearchParams({ k: s.token, layout, theme: prefs.theme, scale: String(prefs.scale), hp: prefs.hp, show: prefs.show.join(",") });
  return `http://127.0.0.1:${s.activePort}/overlay?${p}`;
}

async function copy(layout: Layout) {
  await navigator.clipboard.writeText(url(layout)).catch(() => undefined);
  copied.value = layout;
  window.setTimeout(() => copied.value === layout && (copied.value = null), 1600);
}

function toggleShow(id: string, on: boolean) {
  prefs.show = on ? [...new Set([...prefs.show, id])] : prefs.show.filter((s) => s !== id);
}

const size = (l: (typeof LAYOUTS)[number]) => `${Math.ceil(l.w * prefs.scale)} × ${Math.ceil(l.h * prefs.scale)}`;
const previewLayout = computed(() => LAYOUTS.find((l) => l.id === prefs.preview) ?? LAYOUTS[0]);

onMounted(load);
</script>

<template>
  <div class="overlay-settings">
    <p class="lead">
      Affiche ton équipe et ta run sur ton stream : Kaleido sert une page sur ton ordinateur, à ajouter dans OBS comme source navigateur.
      Elle suit la partie ouverte dans le compagnon et se met à jour à chaque sauvegarde en jeu.
      <Tip term="companion.overlay" />
    </p>

    <div class="panel block">
      <div class="row">
        <Toggle v-model="enabled" label="Activer l'overlay" :disabled="busy" />
        <label class="port">
          <span class="sv-label">Port <Tip term="companion.overlayPort" /></span>
          <input v-model.number="port" class="sv-input" type="number" min="1024" max="65535" :disabled="busy" @change="status?.enabled && configure(true)" />
        </label>
        <span class="state" :class="{ ok: running, bad: !!(status?.error || error) }">
          <template v-if="error || status?.error">{{ error || status?.error }}</template>
          <template v-else-if="running">Ouvert sur 127.0.0.1:{{ status!.activePort }}</template>
          <template v-else>Arrêté</template>
        </span>
      </div>
      <p v-if="running && status!.activePort !== status!.port" class="dim small">
        Le port {{ status!.port }} était déjà pris : Kaleido utilise {{ status!.activePort }}. Les URL ci-dessous en tiennent compte.
      </p>
    </div>

    <template v-if="running">
      <div class="panel block options">
        <div class="opt">
          <span class="opt-label">Thème</span>
          <Segmented v-model="prefs.theme" :options="THEMES.map((t) => ({ value: t.id, label: t.name }))" label="Thème de l'overlay" />
        </div>
        <div class="opt">
          <span class="opt-label">PV</span>
          <Segmented
            v-model="prefs.hp"
            :options="[
              { value: 'bar', label: 'Barre' },
              { value: 'number', label: 'Nombres' },
              { value: 'none', label: 'Masqués' },
            ]"
            label="Affichage des PV"
          />
        </div>
        <label class="opt">
          <span class="opt-label">Taille <Tip term="companion.overlayPixel" /></span>
          <input v-model.number="prefs.scale" type="range" min="0.5" max="3" step="0.5" />
          <span class="num">× {{ prefs.scale.toLocaleString("fr-FR") }}</span>
        </label>
        <div class="opt shows">
          <span class="opt-label">Afficher</span>
          <Toggle v-for="o in SHOW_OPTIONS" :key="o.id" :model-value="prefs.show.includes(o.id)" :label="o.label" @update:model-value="toggleShow(o.id, $event)" />
        </div>
      </div>

      <ul class="layouts">
        <li v-for="l in LAYOUTS" :key="l.id" class="panel layout">
          <div class="info">
            <strong>{{ l.name }}</strong>
            <small>{{ l.text }} Source conseillée : {{ size(l) }}.</small>
          </div>
          <button type="button" class="sv-btn" :class="{ solid: prefs.preview === l.id }" :aria-pressed="prefs.preview === l.id" @click="prefs.preview = l.id">Aperçu</button>
          <button type="button" class="sv-btn" @click="copy(l.id)">
            <Icon :name="copied === l.id ? 'check' : 'copy'" :size="14" /> {{ copied === l.id ? "Copiée" : "Copier l'URL" }}
          </button>
        </li>
      </ul>

      <div class="panel block">
        <div class="preview-head">
          <strong>Aperçu : {{ previewLayout.name }}</strong>
          <small class="dim">Dans OBS : Sources, Navigateur, colle l'URL, largeur et hauteur {{ size(previewLayout) }}. <Tip term="companion.obsSource" /></small>
        </div>
        <div class="stage">
          <iframe :key="url(prefs.preview)" :src="url(prefs.preview)" title="Aperçu de l'overlay" />
        </div>
        <div class="row">
          <small class="dim">Sans partie ouverte dans le compagnon, l'overlay affiche « En attente de partie ».</small>
          <span class="spacer" />
          <button type="button" class="sv-btn small" :disabled="busy" @click="configure(true, true)">Nouveau jeton</button>
          <Tip term="companion.overlayToken" />
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.overlay-settings {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
}

.lead {
  margin: 0;
  color: var(--text-dim);
  font-size: var(--fs-lg);
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-4) var(--sp-5);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3) var(--sp-5);
}

.port {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.port .sv-input {
  width: 96px;
}

.state {
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.state.ok {
  color: var(--ok);
}

.state.bad {
  color: var(--danger);
}

.small {
  margin: 0;
  font-size: var(--fs-md);
}

.dim {
  color: var(--text-dim);
}

.options {
  flex-direction: row;
  flex-wrap: wrap;
  gap: var(--sp-4) var(--sp-6);
}

.opt {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3);
}

.opt-label {
  display: inline-flex;
  align-items: center;
  color: var(--text-dim);
  font-weight: 600;
}

.num {
  min-width: 36px;
  font-variant-numeric: tabular-nums;
}

.layouts {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.layout {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-3) var(--sp-4);
}

.info {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.info small {
  color: var(--text-dim);
}

.preview-head {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

/* Fond neutre et uni pour juger la lisibilité de l'overlay, comme sur une image de jeu. */
.stage {
  height: 240px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 10%, var(--bg));
}

.stage iframe {
  display: block;
  width: 100%;
  height: 100%;
  border: 0;
  background: transparent;
  color-scheme: normal;
}

.spacer {
  flex: 1;
}
</style>
