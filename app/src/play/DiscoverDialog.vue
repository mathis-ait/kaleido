<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Icon from "../components/Icon.vue";
import { addFolders, games } from "../games";
import { applyState, type EmulatorId, type EmulatorsState } from "./play";

/**
 * « Rechercher sur ce PC » : dossiers de jeux et émulateurs trouvés sur les disques,
 * à ajouter en un clic (rien n'est ajouté sans validation).
 */

interface FoundFolder {
  path: string;
  gba?: number;
  nds: number;
  ctr: number;
  switch: number;
  examples: string[];
}
interface FoundEmulator {
  id: EmulatorId;
  name: string;
  exe: string;
  current: boolean;
}
interface Discovery {
  folders: FoundFolder[];
  emulators: FoundEmulator[];
  partial: boolean;
  scanned: number;
}

const emit = defineEmits<{ close: [] }>();

const result = ref<Discovery | null>(null);
const error = ref<string | null>(null);
const progress = ref<{ scanned: number; current: string } | null>(null);
const chosenFolders = ref<Set<string>>(new Set());
const chosenEmus = ref<Set<EmulatorId>>(new Set());
const busy = ref(false);

const followed = (path: string) => games.config.folders.some((f) => f.toLowerCase() === path.toLowerCase());

let unlisten: (() => void) | null = null;
onMounted(async () => {
  unlisten = await listen<{ scanned: number; current: string }>("discover-progress", (e) => (progress.value = e.payload)).catch(() => null);
  try {
    const r = await invoke<Discovery>("discover_pc");
    result.value = r;
    chosenFolders.value = new Set(r.folders.filter((f) => !followed(f.path)).map((f) => f.path));
    chosenEmus.value = new Set(r.emulators.filter((e) => !e.current).map((e) => e.id));
  } catch (e) {
    error.value = String(e);
  }
});
onBeforeUnmount(() => unlisten?.());

const newFolders = computed(() => result.value?.folders.filter((f) => !followed(f.path)) ?? []);
const known = computed(() => result.value?.folders.filter((f) => followed(f.path)) ?? []);

function toggle<T>(set: Set<T>, v: T) {
  if (set.has(v)) set.delete(v);
  else set.add(v);
}

function counts(f: FoundFolder) {
  const parts: string[] = [];
  if (f.gba) parts.push(`${f.gba} GBA`);
  if (f.nds) parts.push(`${f.nds} DS`);
  if (f.ctr) parts.push(`${f.ctr} 3DS`);
  if (f.switch) parts.push(`${f.switch} Switch`);
  return parts.join(" · ");
}

const nothingChosen = computed(() => !chosenFolders.value.size && !chosenEmus.value.size);

async function apply() {
  if (!result.value) return;
  busy.value = true;
  try {
    for (const e of result.value.emulators.filter((x) => chosenEmus.value.has(x.id))) {
      applyState(await invoke<EmulatorsState>("emulator_locate", { id: e.id, exe: e.exe }));
    }
    if (chosenFolders.value.size) await addFolders([...chosenFolders.value]);
    emit("close");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && !busy.value) emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="dd-overlay" @mousedown.self="!busy && emit('close')">
    <section class="dd-dialog panel" role="dialog" aria-modal="true" aria-label="Rechercher sur ce PC">
      <header>
        <h2><Icon name="search" :size="18" /> Rechercher sur ce PC</h2>
        <button class="icon-btn" aria-label="Fermer" :disabled="busy" @click="emit('close')"><Icon name="x" :size="18" /></button>
      </header>

      <div class="body">
        <template v-if="!result && !error">
          <p class="dim">Kaleido parcourt tes disques à la recherche de jeux DS, 3DS et Switch et d'émulateurs…</p>
          <p v-if="progress" class="dim small path">{{ progress.scanned.toLocaleString("fr-FR") }} fichiers vus · {{ progress.current }}</p>
          <div class="bar"><div /></div>
        </template>
        <p v-if="error" class="error">{{ error }}</p>

        <template v-if="result">
          <p v-if="!result.folders.length && !result.emulators.length" class="dim">Aucun jeu ni émulateur trouvé sur ce PC.</p>

          <section v-if="result.emulators.length">
            <h3>Émulateurs</h3>
            <label v-for="e in result.emulators" :key="e.id" class="row">
              <input type="checkbox" :checked="chosenEmus.has(e.id)" :disabled="e.current" @change="toggle(chosenEmus, e.id)" />
              <span class="main"><strong>{{ e.name }}</strong><small class="dim path">{{ e.exe }}</small></span>
              <span v-if="e.current" class="ok"><Icon name="check" :size="13" /> Déjà utilisé</span>
            </label>
          </section>

          <section v-if="newFolders.length">
            <h3>Dossiers de jeux à ajouter</h3>
            <label v-for="f in newFolders" :key="f.path" class="row">
              <input type="checkbox" :checked="chosenFolders.has(f.path)" @change="toggle(chosenFolders, f.path)" />
              <span class="main">
                <strong class="path">{{ f.path }}</strong>
                <small class="dim">{{ counts(f) }} — {{ f.examples.join(", ") }}</small>
              </span>
            </label>
            <p class="dim small">Seuls les jeux Pokémon DS et 3DS sont affichés ; côté Switch, tous les jeux le sont.</p>
          </section>

          <p v-if="known.length" class="dim small">Déjà suivis : {{ known.map((f) => f.path).join(" · ") }}</p>
          <p v-if="result.partial" class="dim small">Recherche arrêtée avant la fin (disque très chargé) : ajoute les autres dossiers à la main.</p>
        </template>
      </div>

      <footer v-if="result">
        <button class="btn" :disabled="busy" @click="emit('close')">Annuler</button>
        <button class="btn btn-primary" :disabled="busy || nothingChosen" @click="apply"><Icon name="check" :size="15" /> {{ busy ? "Ajout…" : "Ajouter la sélection" }}</button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.dd-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.dd-dialog {
  display: flex;
  flex-direction: column;
  width: min(760px, 100%);
  max-height: calc(100vh - 56px);
  overflow: hidden;
  background: var(--surface);
}

header,
footer {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 18px;
}

header {
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

footer {
  justify-content: flex-end;
  border-top: 1px solid var(--border);
}

footer .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

h2 {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 18px;
}

h3 {
  margin: 4px 0 8px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.body {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 18px;
  overflow-y: auto;
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
  cursor: pointer;
}

.main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.path {
  overflow-wrap: anywhere;
}

.ok {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--ok);
  font-size: 12px;
  white-space: nowrap;
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: 0;
  font-size: 12px;
}

.error {
  color: var(--danger);
}

.bar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-hover);
}

.bar div {
  width: 35%;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
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
