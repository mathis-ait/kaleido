<script setup lang="ts">
import { computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import { formatCount, formatSize, modProgress, type ModEntry } from "./mods";

/** Carte d'un mod : aperçu, description, avertissements et actions. */
const props = defineProps<{ mod: ModEntry; busy: string | null; waiting?: boolean; have?: string | null }>();
const emit = defineEmits<{ install: []; import: []; download: []; cancelWait: []; uninstall: []; revert: []; toggle: [enabled: boolean] }>();

const m = computed(() => props.mod);
const manual = computed(() => m.value.source === "manual" || (m.value.kind === "patch" && !m.value.gb));
const working = computed(() => props.busy === m.value.id);
const locked = computed(() => !!props.busy);

const progress = computed(() => {
  const p = modProgress[m.value.id];
  if (!p) return { text: "Préparation…", percent: 6 };
  const percent = p.total ? Math.min(100, Math.round((p.done / p.total) * 100)) : p.step === "install" ? 100 : 6;
  const text =
    p.step === "verify" ? "Vérification…" : p.step === "extract" ? "Décompression…" : p.step === "install" ? "Installation…" : p.total ? `Téléchargement… ${formatSize(p.done)} / ${formatSize(p.total)}` : "Téléchargement…";
  return { text, percent };
});

/** Le mod a été fait pour une autre version du jeu que celle installée. */
const otherVersion = computed(() => !!props.have && !!m.value.gameVersion && !m.value.gameVersion.split(/[/, ]+/).some((v) => v.replace(/\+$/, "") === props.have));

const sourceLabel = computed(() => {
  const page = m.value.page;
  if (page.includes("nexusmods")) return "Nexus Mods";
  if (page.includes("gamebanana")) return "GameBanana";
  if (page.includes("github")) return "GitHub";
  return "Page du mod";
});
</script>

<template>
  <article class="mod" :class="{ installed: m.installed && m.enabled, parked: m.installed && !m.enabled }">
    <div class="thumb" :class="{ empty: !m.thumbnail }">
      <img v-if="m.thumbnail" :src="m.thumbnail" alt="" loading="lazy" referrerpolicy="no-referrer" />
      <Icon v-else :name="m.category === 'cheats' ? 'wand' : m.category === 'romhack' ? 'ball' : 'sparkle'" :size="22" />
    </div>

    <div class="body">
      <div class="title">
        <strong>{{ m.name }}</strong>
        <span v-if="m.recommended && !m.installed" class="sv-chip rec">Recommandé</span>
        <span v-if="m.installed && m.enabled" class="sv-chip ok"><Icon name="check" :size="11" /> Actif</span>
        <span v-if="m.installed && !m.enabled" class="sv-chip">Désactivé</span>
        <span v-if="m.updateAvailable" class="sv-chip upd">Mise à jour</span>
        <span v-if="m.exefs === 'ok'" class="sv-chip ok" title="Le correctif vise exactement l'exécutable qu'Eden lance">Compatible avec ta version</span>
        <span v-else-if="m.exefs" class="sv-chip upd">Version du jeu différente</span>
        <span v-if="manual && !m.installed" class="sv-chip">Téléchargement manuel</span>
      </div>
      <p class="desc">{{ m.description }}</p>
      <p v-if="m.requires.length" class="warn"><Icon name="alert" :size="13" /> Nécessite : {{ m.requires.join(", ") }}</p>
      <p v-if="m.warning" class="warn"><Icon name="alert" :size="13" /> {{ m.warning }}</p>
      <p v-if="m.overlaps.length" class="warn">
        <Icon name="alert" :size="13" /> Remplace les mêmes fichiers que {{ m.overlaps.join(", ") }} : pour ces fichiers, c'est l'ordre de « Priorité entre mods » (onglet Installés) qui décide.
      </p>
      <p v-if="m.conflicts.length && !m.installed" class="dim">Désactivera : {{ m.conflicts.join(", ") }}</p>
      <p class="meta">
        <span v-if="m.author">{{ m.author }}</span>
        <span v-if="m.size">{{ formatSize(m.size) }}</span>
        <span v-if="m.gameVersion">version du jeu {{ m.gameVersion }}<template v-if="otherVersion"> (tu as la {{ have }})</template></span>
        <span v-if="m.affectsSave && !m.installed">ta partie sera copiée avant</span>
        <span v-if="m.popularity && m.source !== 'auto'">{{ formatCount(m.popularity) }} {{ m.page.includes("nexusmods") ? "téléchargements" : "vues" }}</span>
        <button v-if="m.page" class="link" @click="openUrl(m.page)">{{ sourceLabel }}</button>
      </p>
      <p v-if="waiting && !working" class="wait">
        <span class="dot" /> Télécharge le fichier sur la page qui vient de s'ouvrir : Kaleido l'installera dès qu'il arrivera dans Téléchargements.
        <button class="link" @click="emit('cancelWait')">Annuler</button>
      </p>
      <template v-if="working">
        <p class="dim">{{ progress.text }}</p>
        <div class="bar"><div :style="{ width: progress.percent + '%' }" /></div>
      </template>
    </div>

    <div class="actions">
      <template v-if="!m.installed">
        <template v-if="manual">
          <button class="sv-btn" :disabled="locked" @click="emit('download')"><Icon name="download" :size="14" /> Télécharger</button>
          <button class="sv-btn solid" :disabled="locked" @click="emit('import')">
            {{ working ? "Installation…" : m.kind === "patch" ? "Appliquer le patch…" : "Installer le fichier…" }}
          </button>
        </template>
        <button v-else class="sv-btn solid" :disabled="locked" @click="emit('install')">{{ working ? "Installation…" : "Installer" }}</button>
      </template>
      <template v-else>
        <button v-if="m.updateAvailable" class="sv-btn solid" :disabled="locked" @click="emit('install')">Mettre à jour</button>
        <label v-if="m.kind !== 'cheats' && m.kind !== 'patch' && m.source !== 'auto'" class="sv-switch" :title="m.enabled ? 'Mettre de côté sans le supprimer' : 'Réactiver'">
          <input type="checkbox" :checked="m.enabled" :disabled="locked" @change="emit('toggle', !m.enabled)" />
          <span class="track" />
          <span>{{ m.enabled ? "Activé" : "Désactivé" }}</span>
        </label>
        <button v-if="m.previousFile && !m.updateAvailable" class="sv-btn" :disabled="locked" title="Remet le fichier d'avant la dernière mise à jour" @click="emit('revert')">Version précédente</button>
        <button class="sv-btn" :disabled="locked" @click="emit('uninstall')">{{ working ? "…" : m.kind === "patch" ? "Supprimer la ROM" : "Retirer" }}</button>
      </template>
    </div>
  </article>
</template>

<style scoped>
.mod {
  display: grid;
  grid-template-columns: 132px 1fr auto;
  gap: 14px;
  align-items: start;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
}

.mod.installed {
  border-color: color-mix(in srgb, var(--ok) 40%, var(--border));
}

.mod.parked {
  opacity: 0.75;
}

.thumb {
  display: grid;
  place-items: center;
  width: 132px;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  border-radius: calc(var(--radius) - 6px);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  color: var(--text-dim);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.body {
  min-width: 0;
}

.title {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.title strong {
  font-size: 14px;
}

.sv-chip {
  font-size: 11px;
}

.sv-chip.rec {
  border-color: var(--text);
  background: transparent;
}

.sv-chip.ok {
  background: color-mix(in srgb, var(--ok) 18%, transparent);
  color: var(--ok);
}

.sv-chip.upd {
  background: color-mix(in srgb, var(--warn) 18%, transparent);
  color: var(--warn);
}

.body p {
  margin: 5px 0 0;
  font-size: 13px;
  line-height: 1.45;
}

.desc {
  color: var(--text);
}

.warn {
  display: flex;
  gap: 6px;
  color: var(--warn);
}

.warn :deep(svg) {
  flex: none;
  margin-top: 2px;
}

.dim {
  color: var(--text-dim);
}

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  color: var(--text-dim);
  font-size: 12px !important;
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--text-dim);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}

.link:hover {
  color: var(--text);
}

.actions {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  min-width: 150px;
}

.actions .sv-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}

.actions .sv-switch {
  justify-content: center;
  font-size: 13px;
}

.wait {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  color: var(--text);
}

.dot {
  width: 8px;
  height: 8px;
  border: 2px solid var(--text-dim);
  border-top-color: var(--text);
  border-radius: 50%;
  animation: spin 0.9s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.bar {
  height: 5px;
  margin-top: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-hover);
}

.bar div {
  height: 100%;
  border-radius: inherit;
  background: var(--text);
  transition: width 0.25s ease;
}

@media (max-width: 720px) {
  .mod {
    grid-template-columns: 96px 1fr;
  }

  .thumb {
    width: 96px;
  }

  .actions {
    grid-column: 1 / -1;
    flex-direction: row;
    flex-wrap: wrap;
  }
}
</style>
