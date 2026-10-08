<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { coverUrl } from "../launcher/actions";
import { ask, message, open } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import Segmented from "../components/Segmented.vue";
import Tip from "../components/Tip.vue";
import SwitchMusic from "./SwitchMusic.vue";
import ModCard from "./ModCard.vue";
import ModBrowser from "./ModBrowser.vue";
import { rescan } from "../games";
import { installEmulator, installs, loadEmulators, locateEmulator, emus, PLATFORM_LABEL, RECOMMENDED } from "./play";
import {
  CATEGORY_LABEL,
  CATEGORY_ORDER,
  CATEGORY_SHORT,
  downloadsDir,
  formatSize,
  installMod,
  listCheats,
  listMods,
  modsDialog,
  setCheats,
  stopWatch,
  targetOf,
  watchDownloads,
  toggleMod,
  toggleOther,
  tuneApply,
  tunePlan,
  tuneRestore,
  uninstallMod,
  type Cheat,
  type GbProfile,
  type InstallOptions,
  type ModCategory,
  type ModEntry,
  type ModsView,
  type TunePlan,
} from "./mods";

/**
 * « Mods et réglages » d'un jeu de la bibliothèque : sélection des meilleurs mods du jeu,
 * explorateur GameBanana, mods installés, codes de triche et réglages de l'émulateur.
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

type Tab = "selection" | "explore" | "installed" | "cheats" | "settings";
const tab = ref<Tab>("selection");
const filter = ref<ModCategory | "all" | "recommended">("all");

const cheats = ref<Cheat[] | null>(null);
const cheatQuery = ref("");
const onlyEnabled = ref(false);

const coverSrc = computed(() => coverUrl(game.value));

async function load() {
  loading.value = true;
  error.value = null;
  try {
    if (!emus.loaded) await loadEmulators();
    // Pas encore de réglages optimaux pour mGBA (Game Boy, Game Boy Advance).
    const tune = target.value.platform === "gba" ? Promise.resolve(null) : tunePlan(target.value).catch(() => null);
    const [v, p] = await Promise.all([listMods(target.value), tune]);
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
  if (busy.value || variantAsk.value) return;
  modsDialog.game = null;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    if (variantAsk.value) variantAsk.value.resolve(null);
    else close();
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

// --- Téléchargement manuel surveillé

/** Mod dont Kaleido attend le fichier dans Téléchargements. */
const waiting = ref<ModEntry | null>(null);
let unlisten: UnlistenFn | undefined;
onMounted(async () => {
  unlisten = await listen<{ id: string; path: string | null }>("mods-download", async (e) => {
    const m = waiting.value;
    if (!m || e.payload.id !== m.id) return;
    waiting.value = null;
    if (!e.payload.path) {
      await message("Aucun fichier n'est arrivé dans Téléchargements. Tu peux aussi choisir le fichier avec « Installer le fichier… ».", { title: "Téléchargement non trouvé" });
      return;
    }
    await run(m.id, m.name, { local: e.payload.path });
  });
});
onBeforeUnmount(() => {
  unlisten?.();
  if (waiting.value) void stopWatch();
});

function extensionsFor(m: ModEntry) {
  if (platform.value === "nds") return [...PATCHES, ...ARCHIVES];
  return m.kind === "plugin3gx" ? ["3gx", ...ARCHIVES] : ARCHIVES;
}

async function download(m: ModEntry) {
  if (!(await confirmConflicts(m))) return;
  try {
    await watchDownloads(m.id, extensionsFor(m));
    waiting.value = m;
    await openUrl(m.page);
  } catch (e) {
    await message(String(e), { title: "Surveillance impossible", kind: "error" });
  }
}

function cancelWait() {
  waiting.value = null;
  void stopWatch();
}

// --- Onglets

const installedCount = computed(() => (view.value?.mods.filter((m) => m.installed && m.kind !== "cheats").length ?? 0) + (view.value?.others.length ?? 0));

const tabs = computed(() => [
  { value: "selection" as Tab, label: "Sélection" },
  ...(view.value?.gamebanana ? [{ value: "explore" as Tab, label: "Explorer GameBanana" }] : []),
  { value: "installed" as Tab, label: installedCount.value ? `Installés (${installedCount.value})` : "Installés" },
  ...(cheats.value ? [{ value: "cheats" as Tab, label: "Codes" }] : []),
  ...(platform.value !== "gba" ? [{ value: "settings" as Tab, label: "Réglages" }] : []),
]);

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

const emulatorBusy = computed(() => !!installs[emulatorId.value]);

// --- Sélection

const selection = computed(() => (view.value?.mods ?? []).filter((m) => m.kind !== "cheats" || !m.installed).filter((m) => m.source !== "explorer" && m.source !== "local"));

const available = computed(() => CATEGORY_ORDER.filter((c) => selection.value.some((m) => m.category === c)));

const groups = computed(() => {
  const list = selection.value.filter((m) => filter.value === "all" || (filter.value === "recommended" ? m.recommended : m.category === filter.value));
  return CATEGORY_ORDER.map((c) => ({
    category: c,
    label: CATEGORY_LABEL[c],
    mods: list.filter((m) => m.category === c).sort((a, b) => Number(b.recommended) - Number(a.recommended) || (b.popularity ?? 0) - (a.popularity ?? 0)),
  })).filter((g) => g.mods.length);
});

const recommendedToInstall = computed(() => (view.value?.mods ?? []).filter((m) => m.recommended && m.source !== "manual" && (!m.installed || m.updateAvailable)));

// --- Installation

const variantAsk = ref<{ name: string; list: string[]; choice: string; resolve: (v: string | null) => void } | null>(null);

function chooseVariant(name: string, list: string[]) {
  return new Promise<string | null>((resolve) => {
    variantAsk.value = { name, list, choice: list[0], resolve: (v) => ((variantAsk.value = null), resolve(v)) };
  });
}

async function confirmConflicts(m: { name: string; conflicts: string[] }) {
  if (!m.conflicts.length) return true;
  return ask(
    `« ${m.name} » ne peut pas fonctionner en même temps que :\n\n• ${m.conflicts.join("\n• ")}\n\nKaleido les met de côté : tu pourras les réactiver dans l'onglet « Installés ».`,
    { title: "Mods incompatibles", kind: "warning", okLabel: "Mettre de côté et installer", cancelLabel: "Annuler" },
  );
}

/** Installe un mod (choix de variante si l'archive en contient plusieurs). */
async function run(id: string, name: string, options: InstallOptions) {
  busy.value = id;
  try {
    let opts = { disableConflicts: true, ...options };
    for (;;) {
      const r = await installMod(target.value, id, opts);
      if (r.variants.length) {
        const v = await chooseVariant(name, r.variants);
        if (!v) return;
        opts = { ...opts, variant: v };
        continue;
      }
      if (r.view) view.value = r.view;
      if (r.output) {
        await rescan().catch(() => undefined);
        await message(`La ROM « ${r.output.split(/[\\/]/).pop()} » a été créée à côté de ton jeu. Elle apparaît dans la bibliothèque.`, { title: "Romhack prêt" });
      }
      break;
    }
    await loadCheats();
  } catch (e) {
    await message(String(e), { title: `« ${name} » non installé`, kind: "error" });
  } finally {
    busy.value = null;
  }
}

async function install(m: ModEntry) {
  if (!(await confirmConflicts(m))) return;
  if (m.size && m.size > 200 * 1024 * 1024) {
    const ok = await ask(`« ${m.name} » pèse ${formatSize(m.size)}. Le téléchargement peut prendre plusieurs minutes.`, { title: "Gros téléchargement", okLabel: "Télécharger", cancelLabel: "Annuler" });
    if (!ok) return;
  }
  await run(m.id, m.name, {});
}

const ARCHIVES = ["zip", "7z", "rar"];
const PATCHES = ["xdelta", "xdelta3", "vcdiff", "bps", "ips"];

/** Fichier téléchargé à la main (Nexus Mods, Discord…) ou mod d'une autre source. */
async function importFile(m: ModEntry | null) {
  if (m && !(await confirmConflicts(m))) return;
  if (waiting.value) cancelWait();
  const patch = platform.value === "nds";
  const extensions = m ? extensionsFor(m) : patch ? [...PATCHES, ...ARCHIVES] : ARCHIVES;
  const path = await open({
    title: m ? `Fichier téléchargé pour « ${m.name} »` : "Mod à installer",
    defaultPath: (await downloadsDir().catch(() => null)) ?? undefined,
    filters: [{ name: patch ? "Patch ou archive" : "Archive", extensions }],
  });
  if (typeof path !== "string") return;
  await run(m?.id ?? "", m?.name ?? path.split(/[\\/]/).pop() ?? "Mod", { local: path });
}

async function installFromBrowser(mod: GbProfile, file: number) {
  await run(`gb:${mod.id}`, mod.name, { file });
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
  if (m.kind === "patch") {
    const ok = await ask(`Supprimer la ROM « ${m.name} » créée par Kaleido ? Ta ROM d'origine n'est pas touchée.`, { title: "Supprimer la ROM", kind: "warning", okLabel: "Supprimer", cancelLabel: "Annuler" });
    if (!ok) return;
  }
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

async function toggle(m: ModEntry, enabled: boolean) {
  busy.value = m.id;
  try {
    view.value = await toggleMod(target.value, m.id, enabled);
  } catch (e) {
    await message(String(e), { title: "Action impossible", kind: "error" });
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

const installedMods = computed(() => (view.value?.mods ?? []).filter((m) => m.installed && m.kind !== "cheats"));

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
</script>

<template>
  <div class="md-overlay" @mousedown.self="close">
    <section class="md-dialog panel" role="dialog" aria-modal="true" :aria-label="`Mods et réglages : ${game.title}`">
      <header class="md-head">
        <img v-if="coverSrc" class="md-cover" :src="coverSrc" alt="" />
        <div class="md-title">
          <small>{{ PLATFORM_LABEL[platform] }}<template v-if="target.gameVersion"> · version {{ target.gameVersion }}</template></small>
          <h2>{{ game.title }}</h2>
        </div>
        <button class="icon-btn" aria-label="Fermer" title="Fermer" :disabled="!!busy" @click="close"><Icon name="x" :size="18" /></button>
      </header>

      <nav v-if="!loading && !error" class="md-tabs">
        <Segmented v-model="tab" :options="tabs" label="Sections" />
        <button v-if="view?.canImport && tab !== 'settings'" class="sv-btn small-btn" :disabled="!!busy" @click="importFile(null)">
          <Icon name="upload" :size="14" /> Installer depuis un fichier…
        </button>
      </nav>

      <div class="md-body">
        <p v-if="loading" class="dim center">Recherche des mods disponibles…</p>
        <p v-else-if="error" class="error">{{ error }}</p>

        <template v-else-if="view">
          <p v-if="!view.emulatorFound" class="warn small">
            <Icon name="alert" :size="14" /> {{ view.emulator }} n'est pas encore installé : les mods seront prêts dès qu'il le sera.
          </p>

          <!-- Sélection -->
          <template v-if="tab === 'selection'">
            <div class="sel-head">
              <div class="chips">
                <button class="sv-chip" :class="{ on: filter === 'all' }" @click="filter = 'all'">Tout</button>
                <button v-if="selection.some((m) => m.recommended)" class="sv-chip" :class="{ on: filter === 'recommended' }" @click="filter = 'recommended'">Recommandés</button>
                <button v-for="c in available" :key="c" class="sv-chip" :class="{ on: filter === c }" @click="filter = c">{{ CATEGORY_SHORT[c] }}</button>
              </div>
              <button v-if="recommendedToInstall.length > 1" class="sv-btn solid small-btn" :disabled="!!busy" @click="installRecommended">
                <Icon name="download" :size="14" /> Installer les {{ recommendedToInstall.length }} recommandés
              </button>
            </div>
            <p v-for="n in view.notes" :key="n" class="note small"><Icon name="info" :size="14" /> {{ n }}</p>
            <p v-for="e in view.errors" :key="e" class="error small">{{ e }}</p>

            <section v-for="g in groups" :key="g.category" class="group">
              <h4>{{ g.label }}</h4>
              <ModCard v-for="m in g.mods" :key="m.id" :mod="m" :busy="busy" :waiting="waiting?.id === m.id" @install="install(m)" @import="importFile(m)" @download="download(m)" @cancel-wait="cancelWait" @uninstall="uninstall(m)" @toggle="(on) => toggle(m, on)" />
            </section>
            <p v-if="!groups.length" class="dim center">Aucun mod dans cette catégorie pour ce jeu.</p>
            <p v-if="view.gamebanana" class="dim small center">
              Il en manque un ? <button class="link" @click="tab = 'explore'">Explore tous les mods GameBanana du jeu</button>, ou installe une archive téléchargée ailleurs.
            </p>
          </template>

          <!-- Explorateur -->
          <ModBrowser v-else-if="tab === 'explore' && view.gamebanana" :game="view.gamebanana" :installed="view.installedGb" :busy="busy" @install="installFromBrowser" />

          <!-- Installés -->
          <template v-else-if="tab === 'installed'">
            <p v-if="!installedMods.length && !view.others.length" class="dim center">Aucun mod installé pour ce jeu.</p>
            <ModCard v-for="m in installedMods" :key="m.id" :mod="m" :busy="busy" :waiting="waiting?.id === m.id" @install="install(m)" @import="importFile(m)" @download="download(m)" @cancel-wait="cancelWait" @uninstall="uninstall(m)" @toggle="(on) => toggle(m, on)" />
            <template v-if="view.others.length">
              <div class="block-head">
                <h4>Installés hors de Kaleido</h4>
                <Tip term="play.otherMods" />
              </div>
              <ul class="others">
                <li v-for="o in view.others" :key="o.name" :class="{ off: !o.enabled }">
                  <div>
                    <span>{{ o.name }}</span>
                    <small class="dim">{{ CATEGORY_LABEL[o.category] ?? "Autres" }}</small>
                    <small v-if="o.overlaps.length" class="warn">Mêmes fichiers que {{ o.overlaps.join(", ") }}</small>
                  </div>
                  <button v-if="o.canToggle" class="sv-btn small-btn" :disabled="!!busy" @click="setOther(o.name, !o.enabled)">{{ o.enabled ? "Désactiver" : "Réactiver" }}</button>
                </li>
              </ul>
            </template>
            <p v-if="view.location" class="dim small location">
              Dossier des mods : <button class="link" @click="revealItemInDir(view.location)">{{ view.location }}</button>
            </p>
          </template>

          <!-- Codes de triche -->
          <section v-else-if="tab === 'cheats' && cheats" class="block">
            <div class="block-head">
              <h3>Codes activés ({{ enabledCount }} / {{ cheats.length }})</h3>
              <SearchField v-model="cheatQuery" class="cheat-search" placeholder="Chercher un code (money, shiny, walk…)" />
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

          <!-- Réglages -->
          <template v-else-if="tab === 'settings'">
            <section v-if="plan || emulatorMissing" class="block tune">
              <div class="block-head">
                <h3><Icon name="sliders" :size="17" /> Réglages optimaux {{ plan ? (/^[AEIOUaeiou]/.test(plan.emulator) ? "d'" : "de ") + plan.emulator : "de l'émulateur" }}</h3>
                <Tip term="play.optimalSettings" />
              </div>
              <template v-if="emulatorMissing">
                <p class="dim">{{ plan?.error }}</p>
                <div class="row">
                  <button class="sv-btn solid" :disabled="emulatorBusy" @click="installEmu">
                    <Icon name="download" :size="15" /> {{ emulatorBusy ? "Installation…" : `Installer ${plan?.emulator}` }}
                  </button>
                  <button class="sv-btn" :disabled="emulatorBusy" @click="locate"><Icon name="folder" :size="15" /> Localiser…</button>
                </div>
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
                  <button v-if="!plan.applied" class="sv-btn solid" :disabled="tuneBusy || !plan.file" @click="applyTune"><Icon name="wand" :size="15" /> Appliquer ces réglages</button>
                  <span v-else class="ok"><Icon name="check" :size="15" /> Réglages appliqués</span>
                  <button v-if="plan.canRestore" class="sv-btn" :disabled="tuneBusy" @click="restoreTune">Restaurer mes anciens réglages</button>
                  <span class="dim small">Ferme l'émulateur avant d'appliquer.</span>
                </div>
              </template>
            </section>
            <SwitchMusic v-if="platform === 'switch'" :game="game" />
          </template>
        </template>
      </div>

      <!-- Choix d'une variante -->
      <div v-if="variantAsk" class="variant-overlay" @mousedown.self="variantAsk.resolve(null)">
        <div class="variant panel" role="dialog" aria-modal="true" aria-label="Choisir une variante">
          <h3>Plusieurs variantes</h3>
          <p class="dim small">L'archive de « {{ variantAsk.name }} » contient plusieurs versions du mod. La page du mod explique souvent laquelle prendre.</p>
          <label v-for="v in variantAsk.list" :key="v" class="variant-row">
            <input v-model="variantAsk.choice" type="radio" :value="v" />
            <span>{{ v || "(racine de l'archive)" }}</span>
          </label>
          <div class="row end">
            <button class="sv-btn" @click="variantAsk.resolve(null)">Annuler</button>
            <button class="sv-btn solid" @click="variantAsk.resolve(variantAsk.choice)">Installer cette variante</button>
          </div>
        </div>
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
  position: relative;
  display: flex;
  flex-direction: column;
  width: min(1040px, 100%);
  height: min(860px, calc(100vh - 56px));
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

.md-tabs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 10px 18px;
  border-bottom: 1px solid var(--border);
}

.md-tabs .small-btn {
  margin-left: auto;
}

.md-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px 20px;
  overflow-y: auto;
}

.sel-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.chips {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 6px;
}

.group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.group h4,
.block-head h4 {
  margin: 8px 0 0;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
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
  background: var(--text);
  color: var(--bg);
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

.row.end {
  justify-content: flex-end;
}

.row .sv-btn,
.small-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.small-btn {
  padding: 4px 12px;
  font-size: 13px;
}

.cheat-search {
  width: 260px;
  margin-left: auto;
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
  margin: 0;
  padding: 0;
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

.others li div {
  display: flex;
  flex: 1;
  flex-direction: column;
}

.others li.off span {
  color: var(--text-dim);
  text-decoration: line-through;
}

.variant-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: grid;
  place-items: center;
  background: color-mix(in srgb, var(--bg) 50%, transparent);
}

.variant {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: min(520px, calc(100% - 32px));
  max-height: 80%;
  overflow-y: auto;
  padding: 18px;
  background: var(--surface);
  box-shadow: var(--shadow-dialog);
}

.variant h3 {
  margin: 0;
}

.variant-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-size: 13px;
  overflow-wrap: anywhere;
  cursor: pointer;
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
