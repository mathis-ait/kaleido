<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Toggle from "../components/Toggle.vue";
import EmulatorSettings from "../play/EmulatorSettings.vue";
import { THEMES, currentTheme } from "../theme";
import { SPRITE_STYLES, spritePrefs } from "../spriteStyle";
import { openUrl } from "@tauri-apps/plugin-opener";
import { checkUpdate, updates } from "../updates";

/** Pokémon des aperçus : un classique, une forme, un sprite femelle, un chromatique. */
const DEMO: { id: number; form?: number; gender?: "female"; label: string }[] = [
  { id: 6, label: "Dracaufeu" },
  { id: 479, form: 2, label: "Motisma Lavage" },
  { id: 450, gender: "female", label: "Hippodocus ♀" },
  { id: 658, label: "Amphinobi" },
];
const shinyPreview = ref(false);
const scope = computed<"big" | "all">({
  get: () => (spritePrefs.everywhere ? "all" : "big"),
  set: (v) => (spritePrefs.everywhere = v === "all"),
});

interface CacheInfo {
  files: number;
  bytes: number;
  path: string;
}

const cache = ref<CacheInfo | null>(null);
const refresh = async () => (cache.value = await invoke<CacheInfo>("sprite_cache_info").catch(() => null));

async function clearCache() {
  await invoke("clear_sprite_cache");
  await refresh();
}

// Compagnon en direct : lecture de la mémoire de l'émulateur (activée par défaut).
const memory = ref(true);
async function setMemory(on: boolean) {
  memory.value = on;
  await invoke("companion_set_memory", { on }).catch(() => undefined);
}

onMounted(() => {
  void refresh();
  invoke<boolean>("companion_memory")
    .then((on) => (memory.value = on))
    .catch(() => undefined);
});
</script>

<template>
  <section class="settings">
    <h1>Apparence</h1>
    <p class="lead">Choisis l'ambiance de Kaleido.</p>

    <div class="themes">
      <button
        v-for="t in THEMES"
        :key="t.id"
        class="theme panel"
        :class="{ active: currentTheme === t.id }"
        @click="currentTheme = t.id"
      >
        <div class="preview" :style="{ background: t.swatch[0] }">
          <span v-for="c in t.swatch.slice(1)" :key="c" :style="{ background: c }" />
        </div>
        <strong>{{ t.name }}</strong>
        <small>{{ t.description }}</small>
      </button>
    </div>

    <h2>Style des Pokémon</h2>
    <p class="lead">Utilisé pour les grands affichages : fiche Pokémon, accueil de la sauvegarde, starters, Pokédex de l'éditeur.</p>

    <div class="styles">
      <button
        v-for="s in SPRITE_STYLES"
        :key="s.id"
        class="style panel"
        :class="{ active: spritePrefs.style === s.id }"
        :aria-pressed="spritePrefs.style === s.id"
        @click="spritePrefs.style = s.id"
      >
        <div class="stage" :class="{ pixel: s.pixel }">
          <Sprite
            v-for="d in DEMO"
            :key="`${s.id}-${d.id}`"
            :id="d.id"
            :form="d.form"
            :gender="d.gender"
            :shiny="shinyPreview"
            :look="s.id"
            :size="s.id === 'icons' ? 56 : 76"
            :title="d.label"
          />
        </div>
        <strong>{{ s.name }}</strong>
        <small>{{ s.description }}</small>
      </button>
    </div>

    <div class="style-options panel">
      <div class="opt">
        <span class="opt-label">Où l'utiliser</span>
        <Segmented
          v-model="scope"
          :options="[
            { value: 'big', label: 'Grands affichages', hint: 'Les grilles (boîtes, équipe, Pokédex) gardent les icônes, plus lisibles en petit' },
            { value: 'all', label: 'Partout', hint: 'Même style dans les boîtes, l\'équipe et le Pokédex' },
          ]"
        />
      </div>
      <Toggle
        v-model="spritePrefs.reduceMotion"
        label="Réduire les animations"
        hint="Affiche une image fixe du même style au lieu des modèles animés (moins de mouvement, moins de calcul). Activé d'office si Windows demande de réduire les animations."
      />
      <Toggle v-model="shinyPreview" label="Aperçu chromatique" hint="Montre les aperçus ci-dessus en version chromatique." />
    </div>

    <h2>Sprites</h2>
    <div class="sprites panel">
      <div class="demo">
        <Sprite :id="25" :size="68" look="icons" />
        <Sprite :id="6" shiny :size="68" look="icons" />
        <Sprite :id="445" :size="68" look="icons" />
        <Sprite :id="700" :size="68" look="icons" />
      </div>
      <div class="sprite-text">
        <p>
          Les images sont téléchargées la première fois qu'elles s'affichent, puis gardées sur ton disque :
          elles restent disponibles hors ligne. Si un modèle manque, l'icône prend le relais.
        </p>
        <p v-if="cache" class="dim">{{ cache.files }} sprites en cache · {{ (cache.bytes / 1024 / 1024).toFixed(1) }} Mo</p>
        <p class="dim credit">
          Icônes : pokesprite (licence MIT). Modèles 3D, pixel animés et rendus HOME : Pokémon Showdown et ses contributeurs.
          Pokémon © Nintendo, Game Freak, The Pokémon Company.
        </p>
      </div>
      <button class="btn" @click="clearCache">Vider le cache</button>
    </div>

    <h2>Émulateurs</h2>
    <EmulatorSettings />

    <h2>Mises à jour</h2>
    <div class="update panel">
      <div class="update-row">
        <div>
          <strong>Kaleido {{ updates.info?.current ?? "" }}</strong>
          <p v-if="updates.checking" class="lead small">Recherche d'une nouvelle version…</p>
          <p v-else-if="updates.error" class="lead small bad">{{ updates.error }}</p>
          <p v-else-if="updates.info?.newer" class="lead small good">Kaleido {{ updates.info.latest }} est disponible{{ updates.info.name ? ` : ${updates.info.name}` : "" }}.</p>
          <p v-else-if="updates.info?.latest" class="lead small">Tu as la dernière version ({{ updates.info.latest }}).</p>
          <p v-else-if="updates.info" class="lead small">Aucune version publiée pour l'instant.</p>
          <p v-else class="lead small">Vérifie s'il existe une version plus récente sur GitHub.</p>
        </div>
        <div class="update-actions">
          <button v-if="updates.info?.newer" class="btn btn-primary" @click="openUrl(updates.info.url)">Télécharger</button>
          <button class="btn" :disabled="updates.checking" @click="checkUpdate">Vérifier maintenant</button>
        </div>
      </div>
      <pre v-if="updates.info?.newer && updates.info.notes" class="notes">{{ updates.info.notes }}</pre>
      <Toggle v-model="updates.auto" label="Vérifier au démarrage" hint="Kaleido demande à GitHub le numéro de la dernière version publiée, une fois par lancement. Rien d'autre n'est envoyé." />
    </div>

    <h2>À propos</h2>
    <div class="about panel">
      <p>
        Kaleido est un logiciel libre, sous licence GPL version 3 ou ultérieure.
      </p>
      <p>
        <strong>Compagnon en direct.</strong> Pour suivre ta partie à la seconde (PV, rencontres, K.O.), le compagnon lit la
        mémoire de l'émulateur (melonDS, DeSmuME, Azahar) pendant que tu joues. Cette lecture est strictement en lecture seule :
        Kaleido n'écrit jamais dans l'émulateur et n'y injecte rien. Certains antivirus signalent pourtant ce type d'accès à un
        autre programme ; tu peux le couper ici ou dans le compagnon, qui suivra alors seulement ta sauvegarde.
      </p>
      <Toggle :model-value="memory" label="Lire la mémoire de l’émulateur" term="companion.memory" @update:model-value="setMemory" />
    </div>
  </section>
</template>

<style scoped>
.settings {
  max-width: 900px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

h2 {
  margin-top: 40px;
  font-size: 22px;
}

.lead {
  color: var(--text-dim);
  font-size: 16px;
}

.about {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: 18px 20px;
}

.about p {
  margin: 0;
  color: var(--text-dim);
  font-size: var(--fs-base);
}

.about strong {
  color: var(--text);
}

.update {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px 20px;
}

.update-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.update-row .small {
  margin: 4px 0 0;
  font-size: 14px;
}

.update-actions {
  display: flex;
  gap: 8px;
}

.good {
  color: var(--ok);
}

.bad {
  color: var(--danger);
}

.notes {
  max-height: 200px;
  margin: 0;
  padding: 12px;
  overflow: auto;
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 6%, transparent);
  font: inherit;
  font-size: 13px;
  white-space: pre-wrap;
}

/* Trois thèmes : trois colonnes égales, la rangée est pleine. */
.themes {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
  margin-top: 24px;
}

.theme,
.style {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}

.theme:hover:not(.active),
.style:hover:not(.active) {
  background: var(--panel-hover);
}

/* Sélection : inversion fond / texte, comme les onglets. L'aperçu garde ses propres couleurs. */
.theme.active,
.style.active {
  border-color: var(--text);
  background: var(--text);
  color: var(--bg);
}

.theme.active small,
.style.active small {
  color: color-mix(in srgb, var(--bg) 72%, transparent);
}

.style.active .stage {
  background-color: var(--bg);
}

.preview {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  height: 90px;
  margin-bottom: 6px;
  padding: 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
}

.preview span {
  width: 22px;
  height: 22px;
  border-radius: 50%;
}

small,
.dim {
  color: var(--text-dim);
}

.styles {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
  gap: 16px;
  margin-top: 18px;
}

/* Petite scène où les Pokémon d'aperçu se tiennent côte à côte. */
.stage {
  display: flex;
  align-items: center;
  justify-content: space-around;
  height: 100px;
  margin-bottom: 8px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: radial-gradient(ellipse at 50% 85%, color-mix(in srgb, var(--text) 14%, transparent), transparent 70%);
}

.style-options {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 18px 28px;
  margin-top: 14px;
  padding: 16px 20px;
}

.opt {
  display: flex;
  align-items: center;
  gap: 12px;
}

.opt-label {
  color: var(--text-dim);
  font-weight: 600;
}

.sprites {
  display: flex;
  align-items: center;
  gap: 20px;
  margin-top: 14px;
  padding: 18px 20px;
}

.demo {
  display: flex;
}

.sprite-text {
  flex: 1;
}

.sprite-text p {
  margin: 0 0 6px;
}

.credit {
  font-size: 12px;
}
</style>
