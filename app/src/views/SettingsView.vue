<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Toggle from "../components/Toggle.vue";
import { THEMES, currentTheme } from "../theme";
import { SPRITE_STYLES, spritePrefs } from "../spriteStyle";

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

onMounted(refresh);
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
      <Toggle v-model="shinyPreview" label="Aperçu chromatique ✨" hint="Montre les aperçus ci-dessus en version chromatique." />
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

.themes {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 16px;
  margin-top: 24px;
}

.theme {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 14px;
  text-align: left;
  transition: transform 0.15s, outline-color 0.15s;
  outline: 2px solid transparent;
  outline-offset: 2px;
}

.theme:hover {
  transform: translateY(-2px);
}

.theme.active {
  outline-color: var(--accent);
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

.style {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px;
  text-align: left;
  outline: 2px solid transparent;
  outline-offset: 2px;
  transition: transform 0.15s, outline-color 0.15s;
}

.style:hover {
  transform: translateY(-2px);
}

.style.active {
  outline-color: var(--accent);
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
