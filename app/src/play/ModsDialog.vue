<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import SwitchMusic from "./SwitchMusic.vue";
import { titleIdOf } from "../games";
import { installEmulator, installs, loadEmulators, locateEmulator, emus, PLATFORM_LABEL, RECOMMENDED } from "./play";
import {
  CATEGORY_LABEL,
  CATEGORY_ORDER,
  formatSize,
  installMod,
  listCheats,
  listMods,
  modProgress,
  modsDialog,
  setCheats,
  targetOf,
  toggleOther,
  tuneApply,
  tunePlan,
  tuneRestore,
  uninstallMod,
  type Cheat,
  type ModEntry,
  type ModsView,
  type TunePlan,
} from "./mods";

/**
 * « Mods et réglages » d'un jeu de la bibliothèque : réglages optimaux de l'émulateur
 * pour ce PC, mods à installer en un clic, mods déjà présents et codes de triche.
 */

const game = computed(() => modsDialog.game!);
const target = computed(() => targetOf(game.value));
const platform = computed(() => target.value.platform);

const view = ref<ModsView | null>(null);
const plan = ref<TunePlan | null>(null);
const loading = ref(true);
const error = ref<string | null>(null);
const busy = ref<string | null>(null);
const tuneBusy = ref(false);

const cheats = ref<Cheat[] | null>(null);
const cheatQuery = ref("");
const onlyEnabled = ref(false);

const coverSrc = computed(() => {
  const d = game.value;
  if (d.platform === "switch") return convertFileSrc(`nx-${titleIdOf(d)}.png`, "cover");
  if (!d.game) return null;
  return convertFileSrc(`${d.game.id}${d.platform === "nds" && !d.isFrench ? "-en" : ""}.png`, "cover");
});

async function load() {
  loading.value = true;
  error.value = null;
  try {
    if (!emus.loaded) await loadEmulators();
    const [v, p] = await Promise.all([listMods(target.value), tunePlan(target.value).catch(() => null)]);
    view.value = v;
    plan.value = p;
    await loadCheats();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function loadCheats() {
  const installed = view.value?.mods.some((m) => m.category === "cheats" && m.installed);
  cheats.value = installed ? await listCheats(target.value).catch(() => null) : null;
}

onMounted(load);

function close() {
  if (busy.value) return;
  modsDialog.game = null;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") close();
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

// --- Réglages optimaux

const emulatorId = computed(() => RECOMMENDED[platform.value]);
const emulatorMissing = computed(() => !!plan.value?.error && !plan.value.file);

async function applyTune() {
  tuneBusy.value = true;
  try {
    plan.value = await tuneApply(target.value);
  } catch (e) {
    await message(String(e), { title: "Réglages non appliqués", kind: "error" });
  } finally {
    tuneBusy.value = false;
  }
}

async function restoreTune() {
  const ok = await ask("Remettre les réglages de l'émulateur tels qu'ils étaient avant Kaleido ?", { title: "Restaurer", okLabel: "Restaurer", cancelLabel: "Annuler" });
  if (!ok) return;
  tuneBusy.value = true;
  try {
    plan.value = await tuneRestore(target.value);
  } catch (e) {
    await message(String(e), { title: "Restauration impossible", kind: "error" });
  } finally {
    tuneBusy.value = false;
  }
}

async function locate() {
  if (await locateEmulator(emulatorId.value, plan.value?.emulator ?? "l'émulateur")) await load();
}

async function installEmu() {
  if (await installEmulator(emulatorId.value)) await load();
}

// --- Mods

const groups = computed(() => {
  const mods = view.value?.mods ?? [];
  return CATEGORY_ORDER.map((c) => ({ category: c, label: CATEGORY_LABEL[c], mods: mods.filter((m) => m.category === c) })).filter((g) => g.mods.length);
});

const recommendedToInstall = computed(() => (view.value?.mods ?? []).filter((m) => m.recommended && (!m.installed || m.updateAvailable)));

async function confirmConflicts(m: ModEntry) {
  if (!m.conflicts.length) return true;
  return ask(
    `« ${m.name} » ne peut pas fonctionner en même temps que :\n\n• ${m.conflicts.join("\n• ")}\n\nKaleido les désactive (les mods installés à la main sont seulement mis de côté : tu peux les réactiver plus bas).`,
    { title: "Mods incompatibles", kind: "warning", okLabel: "Désactiver et installer", cancelLabel: "Annuler" },
  );
}

async function install(m: ModEntry) {
  if (!(await confirmConflicts(m))) return;
  if (m.size && m.size > 200 * 1024 * 1024) {
    const ok = await ask(`« ${m.name} » pèse ${formatSize(m.size)}. Le téléchargement peut prendre plusieurs minutes.`, {
      title: "Gros téléchargement",
      okLabel: "Télécharger",
      cancelLabel: "Annuler",
    });
    if (!ok) return;
  }
  busy.value = m.id;
  try {
    view.value = await installMod(target.value, m.id, true);
    await loadCheats();
  } catch (e) {
    await message(String(e), { title: `« ${m.name} » non installé`, kind: "error" });
  } finally {
    busy.value = null;
  }
}

async function installRecommended() {
  for (const m of recommendedToInstall.value) {
    // La liste change après chaque installation (conflits résolus).
    const fresh = view.value?.mods.find((x) => x.id === m.id);
    if (!fresh || (fresh.installed && !fresh.updateAvailable)) continue;
    await install(fresh);
  }
}

async function uninstall(m: ModEntry) {
  busy.value = m.id;
  try {
    view.value = await uninstallMod(target.value, m.id);
    await loadCheats();
  } catch (e) {
    await message(String(e), { title: "Impossible de retirer le mod", kind: "error" });
  } finally {
    busy.value = null;
  }
}

async function setOther(name: string, enabled: boolean) {
  busy.value = `other:${name}`;
  try {
    view.value = await toggleOther(target.value, name, enabled);
  } catch (e) {
    await message(String(e), { title: "Action impossible", kind: "error" });
  } finally {
    busy.value = null;
  }
}

function progressText(id: string) {
  const p = modProgress[id];
  if (!p) return "Préparation…";
  if (p.step === "verify") return "Vérification…";
  if (p.step === "extract") return "Décompression…";
  if (p.step === "install") return "Installation…";
  return p.total ? `Téléchargement… ${formatSize(p.done)} / ${formatSize(p.total)}` : "Téléchargement…";
}

const percent = (id: string) => {
  const p = modProgress[id];
  return p && p.total ? Math.min(100, Math.round((p.done / p.total) * 100)) : p?.step === "install" ? 100 : 8;
};

// --- Codes de triche

const shownCheats = computed(() => {
  const q = cheatQuery.value.trim().toLowerCase();
  return (cheats.value ?? []).filter((c) => (!onlyEnabled.value || c.enabled) && (!q || c.name.toLowerCase().includes(q) || c.group?.toLowerCase().includes(q)));
});
const enabledCount = computed(() => (cheats.value ?? []).filter((c) => c.enabled).length);

async function toggleCheat(c: Cheat) {
  const names = (cheats.value ?? []).filter((x) => (x.name === c.name ? !x.enabled : x.enabled)).map((x) => x.name);
  try {
    cheats.value = await setCheats(target.value, names);
  } catch (e) {
    await message(String(e), { title: "Codes non enregistrés", kind: "error" });
  }
}

const emulatorBusy = computed(() => !!installs[emulatorId.value]);
</script>

<template>
  <div class="md-overlay" @mousedown.self="close">
    <section class="md-dialog panel" role="dialog" aria-modal="true" :aria-label="`Mods et réglages : ${game.title}`">
      <header class="md-head">
        <img v-if="coverSrc" class="md-cover" :src="coverSrc" alt="" />
        <div class="md-title">
          <small>{{ PLATFORM_LABEL[platform] }}</small>
          <h2>{{ game.title }}</h2>
        </div>
        <button class="icon-btn" aria-label="Fermer" title="Fermer" :disabled="!!busy" @click="close"><Icon name="x" :size="18" /></button>
      </header>

      <div class="md-body">
        <p v-if="loading" class="dim center">Recherche des mods disponibles…</p>
        <p v-else-if="error" class="error">{{ error }}</p>

        <template v-else>
          <!-- Réglages optimaux -->
          <section class="block tune">
            <div class="block-head">
              <h3><Icon name="sliders" :size="17" /> Réglages optimaux {{ plan ? (/^[AEIOUaeiou]/.test(plan.emulator) ? "d'" : "de ") + plan.emulator : "de l'émulateur" }}</h3>
              <Tip
                title="Réglages optimaux"
                text="Kaleido choisit les réglages graphiques selon ta carte graphique (résolution, filtres, cache des shaders…) et les écrit dans la configuration de l'émulateur. Une copie de ton ancienne configuration est gardée : « Restaurer » la remet exactement."
              />
            </div>

            <template v-if="emulatorMissing">
              <p class="dim">{{ plan?.error }}</p>
              <button class="btn btn-primary" :disabled="emulatorBusy" @click="installEmu">
                <Icon name="download" :size="15" /> {{ emulatorBusy ? "Installation…" : `Installer ${plan?.emulator}` }}
              </button>
              <button class="btn" :disabled="emulatorBusy" @click="locate"><Icon name="folder" :size="15" /> Localiser…</button>
            </template>
            <template v-else-if="plan">
              <p class="gpu">
                <template v-if="plan.gpu">
                  Carte graphique : <strong>{{ plan.gpu.name }}</strong>&nbsp;
                  <span class="dim">({{ Math.round(plan.gpu.vramMb / 1024) }} Go)</span> → profil
                </template>
                <template v-else>Carte graphique non détectée → profil</template>
                <span class="tier" :class="plan.tier">{{ plan.tierLabel }}</span>
                <span class="dim">{{ plan.perGame ? "· pour ce jeu seulement" : "· pour tous les jeux de l'émulateur" }}</span>
              </p>
              <ul class="settings">
                <li v-for="s in plan.settings" :key="s.label">
                  <span class="dim">{{ s.label }}</span><span>{{ s.value }}</span>
                </li>
              </ul>
              <p v-if="plan.error" class="error small">{{ plan.error }}</p>
              <div class="row">
                <button v-if="!plan.applied" class="btn btn-primary" :disabled="tuneBusy || !plan.file" @click="applyTune">
                  <Icon name="wand" :size="15" /> Appliquer ces réglages
                </button>
                <span v-else class="ok"><Icon name="check" :size="15" /> Réglages appliqués</span>
                <button v-if="plan.canRestore" class="btn" :disabled="tuneBusy" @click="restoreTune">Restaurer mes anciens réglages</button>
                <span class="dim small">Ferme l'émulateur avant d'appliquer.</span>
              </div>
            </template>
          </section>

          <SwitchMusic v-if="platform === 'switch'" :game="game" />

          <!-- Mods -->
          <section class="block">
            <div class="block-head">
              <h3><Icon name="sparkle" :size="17" /> Mods</h3>
              <button v-if="recommendedToInstall.length > 1" class="btn btn-primary small-btn" :disabled="!!busy" @click="installRecommended">
                <Icon name="download" :size="14" /> Installer les {{ recommendedToInstall.length }} recommandés
              </button>
            </div>
            <p v-if="view && !view.emulatorFound" class="warn small">
              <Icon name="alert" :size="14" /> {{ view.emulator }} n'est pas encore installé : les mods seront prêts dès qu'il le sera.
            </p>
            <p v-for="n in view?.notes" :key="n" class="note small"><Icon name="info" :size="14" /> {{ n }}</p>
            <p v-for="e in view?.errors" :key="e" class="error small">{{ e }}</p>

            <div v-for="g in groups" :key="g.category" class="group">
              <h4>{{ g.label }}</h4>
              <article v-for="m in g.mods" :key="m.id" class="mod" :class="{ installed: m.installed }">
                <div class="mod-text">
                  <div class="mod-name">
                    <strong>{{ m.name }}</strong>
                    <span v-if="m.recommended" class="badge rec">Recommandé</span>
                    <span v-if="m.installed" class="badge on"><Icon name="check" :size="12" /> Installé</span>
                    <span v-if="m.updateAvailable" class="badge upd">Mise à jour</span>
                  </div>
                  <p>{{ m.description }}</p>
                  <p v-if="m.warning" class="warn small"><Icon name="alert" :size="13" /> {{ m.warning }}</p>
                  <p v-if="m.conflicts.length && !m.installed" class="dim small">Remplacera : {{ m.conflicts.join(", ") }}</p>
                  <p class="meta">
                    <span>{{ m.author }}</span>
                    <span v-if="m.size">· {{ formatSize(m.size) }}</span>
                    <span v-if="m.gameVersion">· pour la version {{ m.gameVersion }} du jeu</span>
                    <span>·</span>
                    <button class="link" @click="openUrl(m.page)">Page du mod</button>
                  </p>
                  <template v-if="busy === m.id && m.id in modProgress">
                    <p class="dim small">{{ progressText(m.id) }}</p>
                    <div class="bar"><div :style="{ width: percent(m.id) + '%' }" /></div>
                  </template>
                </div>
                <div class="mod-actions">
                  <button v-if="!m.installed" class="btn btn-primary" :disabled="!!busy" @click="install(m)">
                    {{ busy === m.id ? "Installation…" : "Installer" }}
                  </button>
                  <template v-else>
                    <button v-if="m.updateAvailable" class="btn btn-primary" :disabled="!!busy" @click="install(m)">Mettre à jour</button>
                    <button v-if="!m.warning?.startsWith('Déjà installé à la main')" class="btn" :disabled="!!busy" @click="uninstall(m)">
                      {{ busy === m.id ? "…" : "Retirer" }}
                    </button>
                  </template>
                </div>
              </article>
            </div>
            <p v-if="!groups.length" class="dim">Aucun mod disponible pour ce jeu pour l'instant.</p>
          </section>

          <!-- Codes de triche -->
          <section v-if="cheats" class="block">
            <div class="block-head">
              <h3><Icon name="wand" :size="17" /> Codes activés ({{ enabledCount }} / {{ cheats.length }})</h3>
              <label class="search">
                <Icon name="search" :size="14" />
                <input v-model="cheatQuery" type="search" placeholder="Chercher un code (money, shiny, walk…)" />
              </label>
              <label class="only"><input v-model="onlyEnabled" type="checkbox" /> Activés seulement</label>
            </div>
            <p class="dim small">Les noms viennent de la base anglaise. Le jeu doit être relancé pour prendre en compte un changement.</p>
            <ul class="cheats">
              <li v-for="c in shownCheats.slice(0, 300)" :key="c.name">
                <label>
                  <input type="checkbox" :checked="c.enabled" @change="toggleCheat(c)" />
                  <span>{{ c.name }}</span>
                  <small v-if="c.group" class="dim">{{ c.group }}</small>
                </label>
              </li>
            </ul>
            <p v-if="shownCheats.length > 300" class="dim small">{{ shownCheats.length - 300 }} autres codes : affine la recherche.</p>
          </section>

          <!-- Mods installés à la main -->
          <section v-if="view?.others.length" class="block">
            <div class="block-head">
              <h3><Icon name="folder" :size="17" /> Autres mods de ce jeu</h3>
              <Tip
                title="Autres mods"
                text="Mods déjà présents dans le dossier de l'émulateur, installés sans Kaleido. Les désactiver les déplace dans un dossier de Kaleido (rien n'est supprimé) ; les réactiver les remet en place."
              />
            </div>
            <ul class="others">
              <li v-for="o in view.others" :key="o.name" :class="{ off: !o.enabled }">
                <span>{{ o.name }}</span>
                <small class="dim">{{ CATEGORY_LABEL[o.category] }}</small>
                <button class="btn small-btn" :disabled="!!busy" @click="setOther(o.name, !o.enabled)">{{ o.enabled ? "Désactiver" : "Réactiver" }}</button>
              </li>
            </ul>
          </section>

          <p v-if="view?.location" class="dim small location">
            Dossier des mods : <button class="link" @click="revealItemInDir(view.location)">{{ view.location }}</button>
          </p>
        </template>
      </div>
    </section>
  </div>
</template>

<style scoped>
.md-overlay {
  position: fixed;
  inset: 0;
  z-index: 150;
  display: grid;
  place-items: center;
  padding: 28px;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
  backdrop-filter: blur(6px);
}

.md-dialog {
  display: flex;
  flex-direction: column;
  width: min(920px, 100%);
  max-height: calc(100vh - 56px);
  overflow: hidden;
  background: var(--surface);
}

.md-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 18px;
  border-bottom: 1px solid var(--border);
}

.md-cover {
  width: 52px;
  height: 52px;
  object-fit: cover;
  border-radius: 10px;
  border: 1px solid var(--border);
}

.md-title {
  flex: 1;
  min-width: 0;
}

.md-title small {
  color: var(--text-dim);
  font-size: 12px;
}

.md-title h2 {
  margin: 0;
  overflow: hidden;
  font-size: 19px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.md-body {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 18px;
  overflow-y: auto;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.block-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.block-head h3 {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 16px;
}

.block-head .small-btn {
  margin-left: auto;
}

.tune {
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
}

.gpu {
  margin: 0;
  font-size: 13px;
}

.tier {
  display: inline-block;
  margin: 0 4px;
  padding: 1px 9px;
  border-radius: 999px;
  background: var(--panel-hover);
  font-weight: 600;
}

.tier.ultra,
.tier.high {
  background: var(--prism);
  color: #fff;
}

.settings {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 4px 18px;
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 13px;
}

.settings li {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  padding: 3px 0;
  border-bottom: 1px dashed var(--border);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.row .btn,
.mod-actions .btn,
.small-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.small-btn {
  padding: 4px 12px;
  font-size: 13px;
}

.group h4 {
  margin: 6px 0 8px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.mod {
  display: flex;
  gap: 14px;
  align-items: flex-start;
  margin-bottom: 8px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--panel);
}

.mod.installed {
  border-color: color-mix(in srgb, var(--ok) 45%, var(--border));
}

.mod-text {
  flex: 1;
  min-width: 0;
}

.mod-text p {
  margin: 4px 0 0;
  font-size: 13px;
}

.mod-name {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
}

.badge.rec {
  background: var(--prism);
  color: #fff;
}

.badge.on {
  background: color-mix(in srgb, var(--ok) 18%, transparent);
  color: var(--ok);
}

.badge.upd {
  background: color-mix(in srgb, var(--warn) 18%, transparent);
  color: var(--warn);
}

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  color: var(--text-dim);
  font-size: 12px !important;
}

.mod-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bar {
  height: 6px;
  margin-top: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--panel-hover);
}

.bar div {
  height: 100%;
  border-radius: inherit;
  background: var(--prism);
  transition: width 0.25s ease;
}

.search {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
  padding: 5px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--panel);
  color: var(--text-dim);
}

.search input {
  width: 230px;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 13px;
}

.only {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 13px;
}

.cheats {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 2px 14px;
  max-height: 340px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
  font-size: 13px;
}

.cheats label {
  display: flex;
  align-items: baseline;
  gap: 7px;
  padding: 3px 0;
  cursor: pointer;
}

.cheats small {
  margin-left: auto;
  white-space: nowrap;
}

.others {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.others li {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-size: 13px;
}

.others li span {
  flex: 1;
}

.others li.off span {
  color: var(--text-dim);
  text-decoration: line-through;
}

.ok {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--ok);
  font-weight: 600;
}

.warn {
  color: var(--warn);
}

.note {
  display: flex;
  gap: 6px;
  margin: 0;
  color: var(--text-dim);
}

.error {
  color: var(--danger);
}

.dim {
  color: var(--text-dim);
}

.small {
  margin: 0;
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
  text-decoration: underline;
  cursor: pointer;
}

.link:hover {
  color: var(--text);
}

.location {
  word-break: break-all;
}
</style>
