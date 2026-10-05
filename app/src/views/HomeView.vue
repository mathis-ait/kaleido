<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import DetectionCard from "../components/DetectionCard.vue";
import KaleidoLogo from "../components/KaleidoLogo.vue";
import { addPaths, library } from "../library";

async function pickFiles() {
  const selected = await open({
    multiple: true,
    title: "Ouvrir une ROM ou une sauvegarde",
    filters: [
      { name: "ROMs et sauvegardes", extensions: ["nds", "3ds", "cci", "cia", "cxi", "sav", "dsv", "bin"] },
      { name: "Tous les fichiers", extensions: ["*"] },
    ],
  });
  if (selected) addPaths(Array.isArray(selected) ? selected : [selected]);
}

async function pickFolder() {
  const selected = await open({ directory: true, title: "Ouvrir un dossier de jeu 3DS extrait" });
  if (typeof selected === "string") addPaths([selected]);
}
</script>

<template>
  <section class="home">
    <header>
      <h1>Bienvenue dans <span class="prism-text">Kaleido</span></h1>
      <p class="lead">Randomise et modifie tes jeux Pokémon DS et 3DS, en français.</p>
    </header>

    <div class="dropzone panel" :class="{ compact: library.items.length > 0 }">
      <KaleidoLogo :size="library.items.length ? 40 : 64" />
      <div class="dz-text">
        <h2>Glisse une ROM ou une sauvegarde ici</h2>
        <p>.nds · .3ds · .cia · dossier extrait (romfs + exheader) · sauvegarde Gen 4 à 7</p>
      </div>
      <div class="dz-actions">
        <button class="btn btn-primary" @click="pickFiles">Ouvrir un fichier</button>
        <button class="btn" @click="pickFolder">Ouvrir un dossier</button>
      </div>
    </div>

    <div v-if="library.pending" class="pending">Analyse en cours…</div>

    <div v-for="(err, i) in library.errors" :key="i" class="error">
      <span>{{ err }}</span>
      <button @click="library.errors.splice(i, 1)" aria-label="Fermer">×</button>
    </div>

    <TransitionGroup name="card" tag="div" class="grid">
      <DetectionCard v-for="item in library.items" :key="item.path" :item="item" />
    </TransitionGroup>
  </section>
</template>

<style scoped>
.home {
  max-width: 1100px;
  margin: 0 auto;
}

header {
  margin-bottom: 28px;
}

h1 {
  font-size: 34px;
  font-weight: 700;
}

.lead {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: 16px;
}

.dropzone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 56px 32px;
  text-align: center;
  border-style: dashed;
  border-width: 2px;
  transition: padding 0.3s ease;
}

.dropzone.compact {
  flex-direction: row;
  padding: 20px 24px;
  text-align: left;
}

.dz-text {
  flex: 1;
}

.dz-text h2 {
  font-size: 20px;
}

.compact .dz-text h2 {
  font-size: 16px;
}

.dz-text p {
  margin: 6px 0 0;
  color: var(--text-dim);
  font-size: 13px;
}

.dz-actions {
  display: flex;
  gap: 10px;
}

.pending {
  margin-top: 16px;
  color: var(--text-dim);
}

.error {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-top: 12px;
  padding: 12px 16px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--danger);
  color: var(--danger);
}

.error button {
  border: none;
  background: none;
  font-size: 18px;
  line-height: 1;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 18px;
  margin-top: 24px;
}

.card-enter-active {
  transition: opacity 0.35s ease, transform 0.35s cubic-bezier(0.2, 0.9, 0.3, 1.2);
}

.card-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.card-enter-from {
  opacity: 0;
  transform: translateY(16px) scale(0.97);
}

.card-leave-to {
  opacity: 0;
  transform: scale(0.96);
}

.card-move {
  transition: transform 0.3s ease;
}
</style>
