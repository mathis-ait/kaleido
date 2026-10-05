<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import "../save/form.css";
import Icon from "../components/Icon.vue";
import { nav } from "../nav";
import PlayGuide from "./PlayGuide.vue";
import PlayTip from "./PlayTip.vue";
import { available, defaultEmulator, emus, fileName, loadEmulators, openGameSave, play, showGuide, type EmulatorId, type PlayPlatform } from "./play";

/**
 * Bloc « Jouer » de la carte de résultat du randomizer : lance la ROM (DS) ou
 * installe le mod LayeredFS puis lance le jeu de base (3DS).
 */
const props = defineProps<{
  platform: PlayPlatform;
  /** ROM DS générée, ou jeu 3DS d'origine. */
  rom: string | null;
  /** Dossier `romfs` du mod 3DS. */
  modRomfs?: string | null;
}>();

const emulator = ref<EmulatorId | null>(null);
const save = ref<string | null>(null);
const busy = ref(false);
const info = ref<string | null>(null);

const choices = computed(() => available(props.platform));

onMounted(async () => {
  if (!emus.loaded) await loadEmulators();
  emulator.value = defaultEmulator(props.platform)?.id ?? null;
});
watch(
  () => [props.rom, props.modRomfs],
  () => {
    info.value = null;
    save.value = null;
  },
);

const options = () => ({ platform: props.platform, emulator: emulator.value, rom: props.rom, modRomfs: props.modRomfs ?? null });

async function launch() {
  busy.value = true;
  info.value = null;
  const result = await play({ ...options(), save: save.value });
  busy.value = false;
  if (!result) return;
  const where = result.savePath ? ` Sa sauvegarde : ${fileName(result.savePath)}.` : "";
  info.value = `Lancé dans ${result.emulator}.${where} Quand tu sauvegardes en jeu, Kaleido le voit.`;
}

async function pickSave() {
  const picked = await open({
    title: "Commencer avec une sauvegarde existante",
    filters: [{ name: "Sauvegardes", extensions: props.platform === "nds" ? ["sav", "dsv"] : ["main", "sav", "bin"] }],
  });
  if (typeof picked === "string") save.value = picked;
}

async function openSaveOfGame() {
  info.value = await openGameSave(options());
}
</script>

<template>
  <div class="play-panel">
    <template v-if="emus.loaded && !choices.length">
      <p class="none">
        <Icon name="alert" :size="16" /> Aucun {{ platform === "nds" ? "émulateur DS" : "émulateur 3DS" }} trouvé pour jouer directement.
        <PlayTip term="emulator" />
      </p>
      <div class="row">
        <button class="btn" @click="nav.view = 'settings'">Configurer un émulateur</button>
        <button class="btn" @click="showGuide(null)">Comment ça marche ?</button>
      </div>
    </template>

    <template v-else>
      <div class="row">
        <button class="btn btn-primary play-btn" :disabled="busy || !emus.loaded" @click="launch">
          <Icon name="play" :size="20" /> {{ busy ? "Lancement…" : "Jouer" }}
        </button>
        <label v-if="choices.length > 1" class="emu">
          avec
          <select v-model="emulator" class="sv-select">
            <option v-for="e in choices" :key="e.id" :value="e.id">{{ e.name }}{{ e.version ? ` ${e.version}` : "" }}</option>
          </select>
        </label>
        <span v-else-if="choices.length" class="dim">avec {{ choices[0].name }} <PlayTip term="emulator" /></span>
      </div>

      <p v-if="platform === '3ds'" class="dim small">
        Le mod est copié dans le dossier des mods de l'émulateur <PlayTip term="layeredfs" />, puis Kaleido lance ton jeu d'origine.
      </p>

      <div class="row">
        <button class="link" @click="pickSave">
          {{ save ? `Sauvegarde : ${fileName(save)}` : "Commencer avec une sauvegarde existante…" }}
        </button>
        <button v-if="save" class="link" aria-label="Retirer la sauvegarde" @click="save = null"><Icon name="x" :size="14" /></button>
        <PlayTip term="backup" />
      </div>
      <button class="btn" @click="openSaveOfGame"><Icon name="save" :size="16" /> Ouvrir la sauvegarde de cette partie</button>
      <p v-if="info" class="dim small">{{ info }}</p>
    </template>
    <PlayGuide />
  </div>
</template>

<style scoped>
.play-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 14px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.play-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 12px 26px;
  font-size: 17px;
}

.emu {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
}

.emu .sv-select {
  width: auto;
}

.none {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--accent-2);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: 0;
  font-size: 13px;
  line-height: 1.45;
}
</style>
