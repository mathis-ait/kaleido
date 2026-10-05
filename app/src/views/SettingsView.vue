<script setup lang="ts">
import { THEMES, currentTheme } from "../theme";
</script>

<template>
  <section class="settings">
    <h1>Apparence</h1>
    <p class="lead">Choisis l'ambiance de Kaleido. D'autres thèmes arriveront plus tard.</p>

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

.lead {
  color: var(--text-dim);
  font-size: 16px;
}

.themes {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
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

small {
  color: var(--text-dim);
}
</style>
