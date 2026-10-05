<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import Sidebar from "./components/Sidebar.vue";
import DropOverlay from "./components/DropOverlay.vue";
import HomeView from "./views/HomeView.vue";
import PlaceholderView from "./views/PlaceholderView.vue";
import SettingsView from "./views/SettingsView.vue";
import { addPaths } from "./library";
import type { ViewId } from "./types";

const view = ref<ViewId>("home");
const dragging = ref(false);

const upcoming = {
  randomizer: {
    title: "Randomizer",
    phase: "Phases 2 et 3",
    description: "Mélange starters, Pokémon sauvages, dresseurs et statistiques, avec une seed partageable.",
    features: ["Préréglages en un clic : Équilibré, Chaos, Nuzlocke, Monotype", "Aperçu des starters avant de générer", "Export .nds ou dossier LayeredFS pour la 3DS"],
  },
  editor: {
    title: "Éditeur de ROM",
    phase: "Phase 2",
    description: "Modifie à la main les données du jeu : Pokémon, attaques, dresseurs, rencontres.",
    features: ["Fiches Pokémon : statistiques, types, talents", "Équipes des dresseurs et des champions", "Tables de rencontres par zone"],
  },
  saves: {
    title: "Sauvegardes",
    phase: "Phase 4",
    description: "Ouvre ta sauvegarde Gen 4 à 7 : boîtes, équipe, dresseur, objets.",
    features: ["Grille de boîtes avec glisser-déposer", "Édition complète d'un Pokémon", "Import / export de Pokémon individuels"],
  },
} as const;

const placeholder = computed(() => (view.value in upcoming ? upcoming[view.value as keyof typeof upcoming] : null));

// Déposer un fichier fonctionne partout dans la fenêtre, quelle que soit la vue.
let unlisten: (() => void) | undefined;
onMounted(async () => {
  unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "enter" || p.type === "over") {
      dragging.value = true;
    } else if (p.type === "leave") {
      dragging.value = false;
    } else if (p.type === "drop") {
      dragging.value = false;
      view.value = "home";
      addPaths(p.paths);
    }
  });
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="shell">
    <Sidebar v-model="view" />
    <main class="content">
      <Transition name="view" mode="out-in">
        <HomeView v-if="view === 'home'" key="home" />
        <SettingsView v-else-if="view === 'settings'" key="settings" />
        <PlaceholderView v-else-if="placeholder" :key="view" v-bind="placeholder" />
      </Transition>
    </main>
    <DropOverlay :visible="dragging" />
  </div>
</template>

<style scoped>
.shell {
  display: grid;
  grid-template-columns: 248px 1fr;
  height: 100%;
}

.content {
  overflow-y: auto;
  padding: 36px 44px 48px;
}

.view-enter-active,
.view-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.view-enter-from {
  opacity: 0;
  transform: translateY(8px);
}

.view-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
