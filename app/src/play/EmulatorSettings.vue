<script setup lang="ts">
import { computed, onMounted, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import PlayGuide from "./PlayGuide.vue";
import PlayTip from "./PlayTip.vue";
import { emus, loadEmulators, showGuide, updateConfig, type EmulatorId, type EmulatorInfo, type PlayPlatform } from "./play";

/** Paramètres → Émulateurs : détection, choix des exécutables et des dossiers. */

const tests = reactive<Partial<Record<EmulatorId, { ok: boolean; text: string }>>>({});

onMounted(loadEmulators);

const groups = computed(() => [
  { platform: "nds" as PlayPlatform, title: "Nintendo DS", list: emus.list.filter((e) => e.platform === "nds") },
  { platform: "3ds" as PlayPlatform, title: "Nintendo 3DS", list: emus.list.filter((e) => e.platform === "3ds") },
]);

const profile = (id: EmulatorId) => emus.config.profiles[id] ?? {};
const preferred = (p: PlayPlatform) => (p === "nds" ? emus.config.preferredNds : emus.config.preferredCtr);

async function pickExe(e: EmulatorInfo) {
  const exe = await open({ title: `Exécutable de ${e.name}`, filters: [{ name: "Programme", extensions: ["exe"] }, { name: "Tous les fichiers", extensions: ["*"] }] });
  if (typeof exe !== "string") return;
  await updateConfig((c) => (c.profiles[e.id] = { ...c.profiles[e.id], exe }));
  await test(e.id);
}

async function autoDetect(e: EmulatorInfo) {
  await updateConfig((c) => (c.profiles[e.id] = { ...c.profiles[e.id], exe: null }));
  delete tests[e.id];
}

async function pickDir(e: EmulatorInfo) {
  const dir = await open({ directory: true, title: e.platform === "nds" ? `Dossier des sauvegardes de ${e.name}` : `Dossier utilisateur de ${e.name} (contient sdmc et load)` });
  if (typeof dir !== "string") return;
  await updateConfig((c) => (c.profiles[e.id] = { ...c.profiles[e.id], ...(e.platform === "nds" ? { saveDir: dir } : { userDir: dir }) }));
}

async function resetDir(e: EmulatorInfo) {
  await updateConfig((c) => (c.profiles[e.id] = { ...c.profiles[e.id], saveDir: null, userDir: null }));
}

const hasDirOverride = (e: EmulatorInfo) => !!(e.platform === "nds" ? profile(e.id).saveDir : profile(e.id).userDir);

async function setPreferred(e: EmulatorInfo) {
  await updateConfig((c) => {
    if (e.platform === "nds") c.preferredNds = e.id;
    else c.preferredCtr = e.id;
  });
}

async function test(id: EmulatorId) {
  try {
    tests[id] = { ok: true, text: await invoke<string>("emulator_test", { id, exe: null }) };
  } catch (err) {
    tests[id] = { ok: false, text: String(err) };
  }
}

async function addSearchDir() {
  const dir = await open({ directory: true, title: "Dossier où se trouvent tes émulateurs" });
  if (typeof dir !== "string" || emus.config.searchDirs.includes(dir)) return;
  await updateConfig((c) => c.searchDirs.push(dir));
}

async function removeSearchDir(dir: string) {
  await updateConfig((c) => (c.searchDirs = c.searchDirs.filter((d) => d !== dir)));
}

function status(e: EmulatorInfo) {
  if (!e.exe) return e.missing ? "Exécutable choisi introuvable" : "Non trouvé";
  return e.detected ? "Trouvé automatiquement" : "Choisi à la main";
}
</script>

<template>
  <div class="emus">
    <p class="lead">
      Lance tes ROMs et tes mods directement depuis Kaleido. <PlayTip term="emulator" />
      <button class="link" @click="showGuide(null)">Comment ça marche ?</button>
    </p>

    <p v-if="emus.error" class="error"><Icon name="alert" :size="16" /> {{ emus.error }}</p>

    <section v-for="g in groups" :key="g.platform" class="group">
      <h3>{{ g.title }}</h3>
      <div v-for="e in g.list" :key="e.id" class="emu panel" :class="{ found: !!e.exe }">
        <header>
          <div class="name">
            <strong>{{ e.name }}</strong>
            <span v-if="e.version" class="chip">{{ e.version }}</span>
            <span class="state" :class="{ ok: !!e.exe, bad: e.missing }">{{ status(e) }}</span>
          </div>
          <label v-if="e.exe" class="pref">
            <input type="radio" :name="`pref-${g.platform}`" :checked="preferred(g.platform) === e.id || (!preferred(g.platform) && g.list.find((x) => x.exe)?.id === e.id)" @change="setPreferred(e)" />
            Par défaut
          </label>
        </header>

        <p v-if="e.exe" class="path mono">{{ e.exe }}</p>

        <div class="row">
          <button class="sv-btn" @click="pickExe(e)"><Icon name="folder-open" :size="15" /> {{ e.exe ? "Changer d'exécutable…" : "Choisir l'exécutable…" }}</button>
          <button v-if="profile(e.id).exe" class="sv-btn" @click="autoDetect(e)">Détection automatique</button>
          <button v-if="e.exe" class="sv-btn" @click="test(e.id)">Tester</button>
          <span v-if="tests[e.id]" class="test" :class="{ ok: tests[e.id]!.ok }">
            <Icon :name="tests[e.id]!.ok ? 'check' : 'alert'" :size="14" /> {{ tests[e.id]!.text }}
          </span>
        </div>

        <div v-if="e.exe || hasDirOverride(e)" class="dir">
          <span class="sv-label">
            {{ e.platform === "nds" ? "Sauvegardes" : "Dossier utilisateur" }}
            <PlayTip :term="e.platform === 'nds' ? 'saveDir' : 'userDir'" />
            <PlayTip v-if="e.platform === 'nds'" term="formats" />
          </span>
          <p class="path">
            <template v-if="e.platform === 'nds' && !e.dataDir">À côté de la ROM (<code>{{ e.id === "desmume" ? "nom.dsv" : "nom.sav" }}</code>)</template>
            <template v-else>
              <span class="mono">{{ e.dataDir }}</span>
              <span v-if="!e.dataDirExists" class="dim"> — pas encore créé</span>
            </template>
          </p>
          <div class="row">
            <button class="sv-btn" @click="pickDir(e)">Choisir un autre dossier…</button>
            <button v-if="hasDirOverride(e)" class="sv-btn" @click="resetDir(e)">Suivre le réglage de l'émulateur</button>
          </div>
          <p v-if="e.platform === '3ds'" class="dim small">
            Mods : <code>load\mods\&lt;title ID&gt;\romfs</code> <PlayTip term="layeredfs" /> <PlayTip term="titleId" /> · Sauvegardes :
            <code>sdmc\Nintendo 3DS\…\title\…\data\00000001\main</code>
          </p>
        </div>
      </div>
    </section>

    <section class="group">
      <h3>Dossiers portables <PlayTip term="portable" /></h3>
      <div class="panel emu">
        <p v-if="!emus.config.searchDirs.length" class="dim small">Aucun dossier ajouté : Kaleido cherche dans Program Files et dans ton dossier utilisateur.</p>
        <ul v-else class="dirs">
          <li v-for="d in emus.config.searchDirs" :key="d">
            <span class="mono">{{ d }}</span>
            <button class="link" aria-label="Retirer ce dossier" @click="removeSearchDir(d)"><Icon name="x" :size="14" /></button>
          </li>
        </ul>
        <div class="row">
          <button class="sv-btn" @click="addSearchDir"><Icon name="plus" :size="15" /> Ajouter un dossier…</button>
          <button class="sv-btn" :disabled="emus.loading" @click="loadEmulators"><Icon name="refresh" :size="15" /> {{ emus.loading ? "Recherche…" : "Relancer la détection" }}</button>
        </div>
      </div>
    </section>
    <PlayGuide />
  </div>
</template>

<style scoped>
.lead {
  color: var(--text-dim);
  font-size: 15px;
}

.group h3 {
  display: flex;
  align-items: center;
  margin: 22px 0 10px;
  font-size: 16px;
}

.emu {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 10px;
  padding: 14px 18px;
}

.emu:not(.found) {
  opacity: 0.85;
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.name {
  display: flex;
  align-items: center;
  gap: 8px;
}

.state {
  color: var(--text-dim);
  font-size: 12px;
}

.state.ok {
  color: #34d399;
}

.state.bad {
  color: var(--danger);
}

.pref {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.path {
  margin: 0;
  font-size: 13px;
  word-break: break-all;
}

.mono {
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
}

.dir {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px dashed var(--border);
}

.test {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--danger);
  font-size: 13px;
}

.test.ok {
  color: #34d399;
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
}

.dirs {
  margin: 0;
  padding: 0;
  list-style: none;
}

.dirs li {
  display: flex;
  align-items: center;
  gap: 8px;
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--accent-2);
  font: inherit;
  cursor: pointer;
}

.error {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--danger);
}
</style>
