<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Toggle from "../components/Toggle.vue";
import { library } from "../library";
import { nav } from "../nav";
import { open } from "@tauri-apps/plugin-dialog";
import { RANDOMIZABLE, isRom, type CtrOutcome, type Outcome, type PokemonRef, type Preset, type RandomizerSettings } from "../types";

const presets = ref<Preset[]>([]);
const activePreset = ref<string | null>("equilibre");
const settings = reactive<RandomizerSettings>({
  starters: "unchanged",
  wild: "unchanged",
  wildSimilarStrength: true,
  wildLevelPercent: 100,
  trainers: "unchanged",
  trainersSimilarStrength: true,
  trainerLevelPercent: 100,
  stats: "unchanged",
  randomTypes: false,
  randomAbilities: false,
  noLegendaries: true,
});

const newSeed = () => Math.floor(Math.random() * 4_294_967_295);
const seed = ref(newSeed());
const romPath = ref<string | null>(null);
const preview = ref<PokemonRef[]>([]);
const previewError = ref<string | null>(null);
const shareInput = ref("");
const shareError = ref(false);
const running = ref(false);
const outcome = ref<Outcome | null>(null);
const outputPath = ref<string | null>(null);
const runError = ref<string | null>(null);
const showLog = ref(false);

const roms = computed(() => library.items.filter(isRom));
const selected = computed(() => roms.value.find((r) => r.path === romPath.value) ?? null);
const isCtr = computed(() => selected.value?.platform === "3ds");
const target = ref<"luma" | "emulator">("luma");
const lastWasCtr = ref(false);
const supported = (id?: string) => !!id && RANDOMIZABLE.includes(id);

onMounted(async () => {
  presets.value = await invoke<Preset[]>("randomizer_presets");
  applyPreset(presets.value[0]);
  romPath.value = nav.randomizerRom ?? roms.value.find((r) => supported(r.game?.id))?.path ?? null;
});

function applyPreset(p: Preset | undefined) {
  if (!p) return;
  Object.assign(settings, p.settings);
  activePreset.value = p.id;
}

// Toute modification manuelle sort du préréglage.
watch(
  () => ({ ...settings }),
  (now) => {
    const p = presets.value.find((p) => p.id === activePreset.value);
    if (p && JSON.stringify(p.settings) !== JSON.stringify(now)) activePreset.value = null;
  },
);

// Aperçu des starters, recalculé quand la ROM, la seed ou les réglages changent.
let previewTimer: number | undefined;
watch(
  [romPath, seed, () => settings.starters, () => settings.noLegendaries, () => settings.randomTypes],
  () => {
    clearTimeout(previewTimer);
    previewTimer = window.setTimeout(refreshPreview, 200);
  },
  { immediate: true },
);

async function refreshPreview() {
  preview.value = [];
  previewError.value = null;
  if (!romPath.value || !supported(selected.value?.game?.id) || isCtr.value) return;
  try {
    preview.value = await invoke<PokemonRef[]>("preview_starters", { path: romPath.value, settings: { ...settings }, seed: seed.value });
  } catch (e) {
    previewError.value = String(e);
  }
}

async function shareCode() {
  // Même format que le moteur : KLD1- + JSON en base64 URL.
  const json = JSON.stringify({ seed: seed.value, settings });
  const b64 = btoa(String.fromCharCode(...new TextEncoder().encode(json))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  await navigator.clipboard.writeText(`KLD1-${b64}`);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
const copied = ref(false);

async function importCode() {
  const parsed = await invoke<[number, RandomizerSettings] | null>("parse_share_code", { code: shareInput.value });
  shareError.value = !parsed;
  if (parsed) {
    seed.value = parsed[0];
    Object.assign(settings, parsed[1]);
    shareInput.value = "";
  }
}

async function generate() {
  if (!selected.value) return;
  if (isCtr.value) return generateCtr();
  const base = selected.value.path.replace(/\.nds$/i, "");
  const output = await save({
    title: "Enregistrer la ROM randomisée",
    defaultPath: `${base} - Kaleido ${seed.value}.nds`,
    filters: [{ name: "ROM Nintendo DS", extensions: ["nds"] }],
  });
  if (!output) return;
  running.value = true;
  runError.value = null;
  outcome.value = null;
  try {
    outcome.value = await invoke<Outcome>("randomize_rom", { path: selected.value.path, settings: { ...settings }, seed: seed.value, output });
    outputPath.value = output;
    lastWasCtr.value = false;
  } catch (e) {
    runError.value = String(e);
  } finally {
    running.value = false;
  }
}

/** 3DS : les fichiers modifiés sont écrits dans un dossier LayeredFS. */
async function generateCtr() {
  const output = await open({ directory: true, title: "Choisis le dossier où créer le mod" });
  if (typeof output !== "string" || !selected.value) return;
  running.value = true;
  runError.value = null;
  outcome.value = null;
  try {
    const res = await invoke<CtrOutcome>("randomize_ctr", {
      path: selected.value.path,
      settings: { ...settings },
      seed: seed.value,
      output,
      target: target.value,
    });
    outcome.value = res;
    outputPath.value = res.romfs;
    lastWasCtr.value = true;
  } catch (e) {
    runError.value = String(e);
  } finally {
    running.value = false;
  }
}

const levelLabel = (p: number) => (p === 100 ? "inchangés" : `${p > 100 ? "+" : ""}${p - 100} %`);
</script>

<template>
  <section class="rando">
    <header>
      <h1>Randomizer</h1>
      <p class="lead">Une nouvelle aventure à chaque seed. Partage le code pour que tes amis jouent exactement la même.</p>
    </header>

    <!-- Choix de la ROM -->
    <div v-if="!roms.length" class="panel empty">
      <p>Ajoute d'abord une ROM DS (Platine, Noire ou Blanche) dans la bibliothèque.</p>
      <button class="btn btn-primary" @click="nav.view = 'home'">Ajouter une ROM</button>
    </div>

    <template v-else>
      <div class="roms">
        <button
          v-for="r in roms"
          :key="r.path"
          class="rom panel"
          :class="{ active: romPath === r.path }"
          :disabled="!supported(r.game?.id)"
          :title="supported(r.game?.id) ? r.path : 'Pas encore pris en charge par le randomizer'"
          @click="romPath = r.path"
        >
          <strong>{{ r.title }}</strong>
          <small>{{ supported(r.game?.id) ? r.language : "Bientôt" }}</small>
        </button>
      </div>

      <div class="layout">
        <div class="options">
          <!-- Préréglages -->
          <div class="presets">
            <button v-for="p in presets" :key="p.id" class="preset panel" :class="{ active: activePreset === p.id }" @click="applyPreset(p)">
              <strong>{{ p.name }}</strong>
              <small>{{ p.description }}</small>
            </button>
          </div>

          <div v-if="isCtr" class="section panel">
            <h3>Sortie 3DS</h3>
            <Segmented
              v-model="target"
              :options="[
                { value: 'luma', label: 'Console (Luma3DS)', hint: 'Crée luma/titles/…/romfs : copie le dossier « luma » à la racine de la carte SD' },
                { value: 'emulator', label: 'Émulateur', hint: 'Crée <title ID>/romfs : à placer dans le dossier « mods » de l\'émulateur (Azahar, Citra…)' },
              ]"
            />
            <p class="dim note">
              Seuls les fichiers modifiés sont écrits : ta ROM d'origine n'est jamais touchée. Les starters restent
              ceux du jeu sur 3DS pour l'instant.
            </p>
          </div>

          <div v-else class="section panel">
            <h3>Starters</h3>
            <Segmented
              v-model="settings.starters"
              :options="[
                { value: 'unchanged', label: 'Inchangés' },
                { value: 'random', label: 'Aléatoires' },
                { value: 'three_stage', label: 'Trio évolutif', hint: 'Pokémon de base avec deux évolutions' },
                { value: 'triangle', label: 'Plante · Feu · Eau', hint: 'Trio évolutif qui garde le triangle des types' },
              ]"
            />
          </div>

          <div class="section panel">
            <h3>Pokémon sauvages</h3>
            <Segmented
              v-model="settings.wild"
              :options="[
                { value: 'unchanged', label: 'Inchangés' },
                { value: 'random', label: 'Totalement aléatoires' },
                { value: 'area', label: 'Par zone', hint: 'Dans une zone, chaque espèce est remplacée par une même nouvelle espèce' },
                { value: 'global', label: 'Global', hint: 'Une espèce devient la même partout dans le jeu' },
              ]"
            />
            <div class="row">
              <Toggle v-model="settings.wildSimilarStrength" label="Puissance similaire" />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.wildLevelPercent) }}</strong>
                <input v-model.number="settings.wildLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
          </div>

          <div class="section panel">
            <h3>Dresseurs</h3>
            <Segmented
              v-model="settings.trainers"
              :options="[
                { value: 'unchanged', label: 'Inchangés' },
                { value: 'random', label: 'Aléatoires' },
                { value: 'type_themed', label: 'Thématiques', hint: 'Chaque dresseur garde un type dominant (champions compris)' },
              ]"
            />
            <div class="row">
              <Toggle v-model="settings.trainersSimilarStrength" label="Puissance similaire" />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.trainerLevelPercent) }}</strong>
                <input v-model.number="settings.trainerLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
          </div>

          <div class="section panel">
            <h3>Espèces</h3>
            <Segmented
              v-model="settings.stats"
              :options="[
                { value: 'unchanged', label: 'Statistiques inchangées' },
                { value: 'shuffle', label: 'Mélangées', hint: 'Les 6 statistiques sont permutées, le total ne change pas' },
                { value: 'random', label: 'Redistribuées', hint: 'Nouvelle répartition du même total' },
              ]"
            />
            <div class="row">
              <Toggle v-model="settings.randomTypes" label="Types aléatoires" hint="Une famille d'évolution garde les mêmes types" />
              <Toggle v-model="settings.randomAbilities" label="Talents aléatoires" />
              <Toggle v-model="settings.noLegendaries" label="Sans légendaires" />
            </div>
          </div>
        </div>

        <!-- Colonne de droite : aperçu et génération -->
        <aside class="side">
          <div class="panel card">
            <h3>Tes starters</h3>
            <div class="starters">
              <div v-for="(p, i) in preview" :key="`${p.id}-${i}`" class="starter">
                <Sprite :id="p.id" :size="88" />
                <span>{{ p.name }}</span>
              </div>
              <p v-if="isCtr" class="dim">Sur 3DS, tu gardes les starters du jeu (Arcko, Poussifeu, Gobou).</p>
              <p v-else-if="!preview.length && !previewError" class="dim">Choisis une ROM compatible.</p>
              <p v-if="previewError" class="error-text">{{ previewError }}</p>
            </div>
          </div>

          <div class="panel card">
            <h3>Seed</h3>
            <div class="seed">
              <input v-model.number="seed" type="number" min="0" class="input" />
              <button class="btn icon" title="Nouvelle seed" @click="seed = newSeed()">🎲</button>
            </div>
            <div class="share">
              <button class="btn" @click="shareCode">{{ copied ? "Copié !" : "Copier le code de partage" }}</button>
            </div>
            <div class="seed">
              <input v-model="shareInput" class="input" placeholder="Coller un code KLD1-…" :class="{ invalid: shareError }" @keydown.enter="importCode" />
              <button class="btn" :disabled="!shareInput" @click="importCode">Importer</button>
            </div>
          </div>

          <button class="btn btn-primary generate" :disabled="!selected || !supported(selected.game?.id) || running" @click="generate">
            {{ running ? "Génération…" : isCtr ? "Générer le mod 3DS" : "Générer la ROM" }}
          </button>

          <div v-if="runError" class="panel card error">{{ runError }}</div>

          <Transition name="pop">
            <div v-if="outcome" class="panel card done">
              <h3>✨ {{ lastWasCtr ? "Mod prêt !" : "ROM prête !" }}</h3>
              <p class="dim">{{ outcome.wildSlots }} Pokémon sauvages et {{ outcome.trainerPokemon }} Pokémon de dresseurs modifiés.</p>
              <p v-if="lastWasCtr" class="dim note">
                {{ target === "luma" ? "Copie le dossier « luma » à la racine de ta carte SD et active « Game patching » dans Luma." : "Place le dossier du title ID dans le dossier « mods » de ton émulateur." }}
              </p>
              <div class="done-actions">
                <button v-if="outputPath" class="btn" @click="revealItemInDir(outputPath)">Ouvrir le dossier</button>
                <button class="btn" @click="showLog = true">Voir le journal</button>
              </div>
            </div>
          </Transition>
        </aside>
      </div>
    </template>

    <div v-if="showLog && outcome" class="modal" @click.self="showLog = false">
      <div class="panel log">
        <header>
          <h3>Journal de randomisation</h3>
          <button class="btn" @click="showLog = false">Fermer</button>
        </header>
        <pre>{{ outcome.log }}</pre>
      </div>
    </div>
  </section>
</template>

<style scoped>
.rando {
  max-width: 1200px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

h3 {
  margin-bottom: 12px;
  font-size: 15px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
  font-size: 16px;
}

.empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
}

.roms {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin: 20px 0;
}

.rom {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 12px 16px;
  outline: 2px solid transparent;
  outline-offset: 2px;
  text-align: left;
}

.rom.active,
.preset.active {
  outline-color: var(--accent);
}

.rom:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.rom small,
.preset small {
  color: var(--text-dim);
}

.layout {
  display: grid;
  grid-template-columns: 1fr 320px;
  gap: 20px;
  align-items: start;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.presets {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
}

.preset {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px;
  text-align: left;
  outline: 2px solid transparent;
  outline-offset: 2px;
  transition: transform 0.15s;
}

.preset:hover {
  transform: translateY(-2px);
}

.preset small {
  font-size: 12px;
  line-height: 1.35;
}

.section {
  padding: 18px 20px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 24px;
  margin-top: 14px;
}

.slider {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-dim);
}

.slider strong {
  min-width: 80px;
  color: var(--text);
}

.slider input {
  accent-color: var(--accent-2);
}

.side {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card {
  padding: 18px;
}

.starters {
  display: flex;
  justify-content: space-around;
  min-height: 110px;
}

.starter {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  font-weight: 600;
  animation: rise 0.35s ease both;
}

.starter:nth-child(2) {
  animation-delay: 0.06s;
}

.starter:nth-child(3) {
  animation-delay: 0.12s;
}

@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.9);
  }
}

.seed {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.share {
  margin-top: 8px;
}

.share .btn {
  width: 100%;
  justify-content: center;
}

.input {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font: inherit;
  outline: none;
}

.input:focus {
  border-color: var(--accent);
}

.input.invalid {
  border-color: var(--danger);
}

.icon {
  padding: 8px 12px;
}

.generate {
  justify-content: center;
  padding: 16px;
  font-size: 16px;
  font-weight: 700;
}

.error,
.error-text {
  color: var(--danger);
}

.note {
  margin: 12px 0 0;
  font-size: 13px;
  line-height: 1.45;
}

.done h3 {
  font-size: 18px;
}

.done-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.pop-enter-active {
  transition: opacity 0.3s, transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.3);
}

.pop-enter-from {
  opacity: 0;
  transform: scale(0.95);
}

.modal {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: grid;
  place-items: center;
  padding: 40px;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
}

.log {
  display: flex;
  flex-direction: column;
  width: min(1000px, 100%);
  max-height: 100%;
  padding: 18px;
  background: var(--bg);
}

.log header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.log pre {
  overflow: auto;
  margin: 12px 0 0;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  user-select: text;
  white-space: pre-wrap;
}
</style>
