<script setup lang="ts">
import { computed } from "vue";
import Icon from "../components/Icon.vue";
import { goTo, saveState, type SaveTool } from "../saveStore";
import TrainerTool from "./tools/TrainerTool.vue";
import ItemsTool from "./tools/ItemsTool.vue";
import DexTool from "./tools/DexTool.vue";
import BoxNamesTool from "./tools/BoxNamesTool.vue";
import ChecksTool from "./tools/ChecksTool.vue";
import { useShell } from "./shell";

const TOOLS: { id: SaveTool; title: string; sub: string; icon: string; color: string; comp: unknown }[] = [
  { id: "trainer", title: "Dresseur", sub: "Nom, ID, argent, temps de jeu", icon: "user", color: "#8b6cf6", comp: TrainerTool },
  { id: "items", title: "Sac", sub: "Objets, CT, Baies, Poké Balls…", icon: "bag", color: "#f59e0b", comp: ItemsTool },
  { id: "dex", title: "Pokédex", sub: "Vus et capturés", icon: "book", color: "#22c55e", comp: DexTool },
  { id: "boxes", title: "Boîtes", sub: "Noms des boîtes", icon: "box", color: "#3b82f6", comp: BoxNamesTool },
  { id: "checks", title: "Tout vérifier", sub: "Contrôle de chaque Pokémon", icon: "shield", color: "#14b8a6", comp: ChecksTool },
];

const tool = computed(() => TOOLS.find((t) => t.id === saveState.tool) ?? null);

useShell(() => (tool.value ? {} : { hint: "Choisis un outil", actions: [] }));
</script>

<template>
  <div class="tools">
    <template v-if="!tool">
      <div class="cards">
        <button v-for="t in TOOLS" :key="t.id" class="card sv-panel" @click="goTo('tools', t.id)">
          <span class="ico" :style="{ background: t.color }"><Icon :name="t.icon" :size="28" /></span>
          <strong>{{ t.title }}</strong>
          <small>{{ t.sub }}</small>
        </button>
      </div>
      <p class="later">
        Bientôt : rubans, souvenirs, records, Pokéwalker / Pokémon Global Link, et un mode « avancé » pour les données
        propres à chaque jeu.
      </p>
    </template>
    <template v-else>
      <header class="tool-head">
        <button class="back" aria-label="Retour aux outils" @click="saveState.tool = null"><Icon name="chevron-left" /></button>
        <span class="ico small" :style="{ background: tool.color }"><Icon :name="tool.icon" :size="20" /></span>
        <div>
          <h2>{{ tool.title }}</h2>
          <small>{{ saveState.view?.game }}</small>
        </div>
      </header>
      <component :is="tool.comp" class="tool-body" />
    </template>
  </div>
</template>

<style scoped>
.tools {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 18px;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 22px;
  text-align: left;
  transition: transform 0.15s;
}

.card:hover {
  transform: translateY(-3px);
}

.card strong {
  margin-top: 8px;
  font-size: 19px;
}

.card small {
  color: var(--text-dim);
}

.ico {
  display: grid;
  place-items: center;
  width: 54px;
  height: 54px;
  border-radius: 50%;
  color: #fff;
  box-shadow: 0 6px 14px rgba(0, 0, 0, 0.25);
}

.ico.small {
  width: 40px;
  height: 40px;
}

.later {
  margin-top: 26px;
  color: var(--text-dim);
  font-size: 13px;
}

.tool-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.tool-head h2 {
  font-size: 22px;
}

.tool-head small {
  color: var(--text-dim);
}

.back {
  display: grid;
  place-items: center;
  width: 40px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.tool-body {
  flex: 1;
  min-height: 0;
}
</style>
