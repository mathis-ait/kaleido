<script setup lang="ts">
import { library } from "../library";
import { updates } from "../updates";
import type { ViewId } from "../types";
import KaleidoLogo from "./KaleidoLogo.vue";

defineProps<{ compact?: boolean }>();
const view = defineModel<ViewId>({ required: true });

// Icônes en traits fins, 24×24.
const items: { id: ViewId; label: string; icon: string }[] = [
  { id: "home", label: "Fichiers", icon: "M6 3h8l5 5v13H6zM14 3v5h5" },
  { id: "library", label: "Bibliothèque", icon: "M4 5h6v6H4zM14 5h6v6h-6zM4 15h6v4H4zM14 15h6v4h-6z" },
  { id: "randomizer", label: "Randomizer", icon: "M4 7h3l10 10h3M4 17h3l3-3M14 10l3-3h3M18 4l3 3-3 3M18 14l3 3-3 3" },
  { id: "editor", label: "Éditeur de ROM", icon: "M5 19h4L19 9l-4-4L5 15zM13 7l4 4" },
  { id: "saves", label: "Sauvegardes", icon: "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6" },
];
</script>

<template>
  <aside class="sidebar" :class="{ compact }">
    <div class="brand">
      <KaleidoLogo :size="34" />
      <div>
        <div class="brand-name">Kaleido</div>
        <div class="brand-sub">Randomizer & éditeur</div>
      </div>
    </div>

    <nav>
      <button v-for="item in items" :key="item.id" class="nav-item" :class="{ active: view === item.id }" :title="item.label" @click="view = item.id">
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
          <path :d="item.icon" />
        </svg>
        <span>{{ item.label }}</span>
        <span v-if="item.id === 'home' && library.items.length" class="count">{{ library.items.length }}</span>
      </button>
    </nav>

    <button class="nav-item settings" :class="{ active: view === 'settings' }" @click="view = 'settings'">
      <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" />
      </svg>
      <span>Apparence</span>
      <span v-if="updates.info?.newer" class="update-dot" :title="`Kaleido ${updates.info.latest} est disponible`" />
    </button>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 24px 16px;
  background: var(--sidebar);
  border-right: 1px solid var(--border);
  backdrop-filter: blur(24px);
  color: var(--text);
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 8px;
}

.brand-name {
  font-family: var(--font-display);
  font-size: 20px;
  font-weight: 700;
}

.brand-sub {
  font-size: 12px;
  color: var(--text-dim);
}

nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
}

.update-dot {
  position: absolute;
  top: 8px;
  left: 28px;
  width: 9px;
  height: 9px;
  border: 2px solid var(--sidebar);
  border-radius: 50%;
  background: #3ccf7a;
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 11px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-dim);
  font-weight: 500;
  text-align: left;
  transition: background 0.15s, color 0.15s;
}

.nav-item:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.nav-item.active {
  background: var(--panel-hover);
  color: var(--text);
}

.nav-item.active::before {
  content: "";
  position: absolute;
  left: -16px;
  top: 8px;
  bottom: 8px;
  width: 4px;
  border-radius: 0 4px 4px 0;
  background: var(--prism);
}

.count {
  margin-left: auto;
  min-width: 22px;
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--prism);
  color: var(--on-accent);
  font-size: 11px;
  font-weight: 700;
  text-align: center;
}
</style>

<style scoped>
/* Barre réduite aux icônes (éditeur de sauvegardes en plein écran). */
.sidebar.compact {
  padding: 24px 10px;
  align-items: center;
}

.compact .brand {
  padding: 0;
}

.compact .brand > div,
.compact .nav-item span {
  display: none;
}

.compact .nav-item {
  justify-content: center;
  padding: 12px;
}

.compact .nav-item.active::before {
  left: -10px;
}
</style>
