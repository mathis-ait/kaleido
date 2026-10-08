<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import Sidebar from "./components/Sidebar.vue";
import DropOverlay from "./components/DropOverlay.vue";
import PlayGuide from "./play/PlayGuide.vue";
import HomeView from "./views/HomeView.vue";
import LibraryView from "./views/LibraryView.vue";
import EditorView from "./views/EditorView.vue";
import RandomizerView from "./views/RandomizerView.vue";
import SavesView from "./views/SavesView.vue";
import SettingsView from "./views/SettingsView.vue";
import { addPaths } from "./library";
import { nav } from "./nav";
import { allGames, libraryUi } from "./games";

const dragging = ref(false);
const launcher = computed(() => nav.view === "library" && libraryUi.mode === "launcher" && allGames.value.length > 0);
/** Pages plein cadre (sans marges, positionnées dans la zone de contenu). */
const flush = computed(() => nav.view === "saves" || launcher.value);
/** Le lanceur et la grille sont deux pages distinctes : passer de l'une à l'autre fait un fondu. */
const pageKey = computed(() => (launcher.value ? "library-launcher" : nav.view));

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
      nav.view = "home";
      addPaths(p.paths);
    }
  });
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="shell" :class="{ compact: nav.view === 'saves', immersive: launcher && libraryUi.immersive }">
    <Sidebar v-if="!(launcher && libraryUi.immersive)" v-model="nav.view" :compact="nav.view === 'saves'" />
    <main class="content">
      <!-- Chaque page garde sa propre mise en page pendant qu'elle disparaît : sans ça, une
           page plein cadre qui s'efface sautait sur toute la fenêtre (et inversement). -->
      <Transition name="view" mode="out-in">
        <div :key="pageKey" class="page" :class="{ flush }">
          <HomeView v-if="nav.view === 'home'" />
          <LibraryView v-else-if="nav.view === 'library'" />
          <EditorView v-else-if="nav.view === 'editor'" />
          <RandomizerView v-else-if="nav.view === 'randomizer'" />
          <SavesView v-else-if="nav.view === 'saves'" />
          <SettingsView v-else-if="nav.view === 'settings'" />
        </div>
      </Transition>
    </main>
    <DropOverlay :visible="dragging" />
    <!-- Guide « Jouer » : une seule instance, ouvrable depuis n'importe quelle page (bibliothèque, lanceur…). -->
    <PlayGuide />
  </div>
</template>

<style scoped>
.shell {
  display: grid;
  grid-template-columns: 248px 1fr;
  height: 100%;
  transition: grid-template-columns 0.2s ease;
}

.shell.immersive {
  grid-template-columns: 1fr;
}

.shell.compact {
  grid-template-columns: 72px 1fr;
}

.content {
  position: relative;
  min-width: 0;
  overflow: hidden;
}

.page {
  position: absolute;
  inset: 0;
  overflow-y: auto;
  padding: 36px 44px 48px;
}

.page.flush {
  overflow: hidden;
  padding: 0;
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
