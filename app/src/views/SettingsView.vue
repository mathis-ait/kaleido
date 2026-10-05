<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Sprite from "../components/Sprite.vue";
import { THEMES, currentTheme } from "../theme";

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

    <h2>Sprites</h2>
    <div class="sprites panel">
      <div class="demo">
        <Sprite :id="25" :size="68" />
        <Sprite :id="6" shiny :size="68" />
        <Sprite :id="445" :size="68" />
        <Sprite :id="700" :size="68" />
      </div>
      <div class="sprite-text">
        <p>
          Les icônes sont téléchargées la première fois qu'elles s'affichent, puis gardées sur ton disque :
          elles restent disponibles hors ligne.
        </p>
        <p v-if="cache" class="dim">{{ cache.files }} sprites en cache · {{ (cache.bytes / 1024).toFixed(0) }} Ko</p>
        <p class="dim credit">Icônes : pokesprite (licence MIT). Pokémon © Nintendo, Game Freak, The Pokémon Company.</p>
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
