<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref } from "vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import { nav } from "../nav";
import { closeSave, editPokemon, goTo, history, saveState, writeSave } from "../saveStore";
import { useShell } from "./shell";
import LiveSyncBadge from "../play/LiveSyncBadge.vue";
import { playOpenSave, sendToGame } from "../play/play";
import { bankState, loadBankInfo } from "../bankStore";

const emit = defineEmits<{ open: []; "save-as": [] }>();

const view = computed(() => saveState.view!);
const lead = computed(() => saveState.selected ?? view.value.party[0] ?? null);
const stored = computed(() => view.value.boxFill.reduce((a, b) => a + b, 0));
const boxPreview = computed(() => saveState.slots.filter((s) => !!s).slice(0, 6));
const t = computed(() => view.value.trainer);

interface Tile {
  id: string;
  title: string;
  sub: string;
  band: string;
  icon?: string;
  gradient: string;
  go?: () => void;
  soon?: boolean;
}

const tiles = computed<Tile[]>(() => [
  {
    id: "pokemon",
    title: "Pokémon",
    sub: lead.value ? `${lead.value.nickname || lead.value.speciesName} · N. ${lead.value.level}` : "Aucun Pokémon sélectionné",
    band: "Modifier le Pokémon sélectionné : identité, statistiques, attaques, rencontre",
    gradient: "linear-gradient(150deg, #ff9a5c, #ff5c8a 55%, #d63d9a)",
    go: () => (lead.value ? editPokemon(lead.value) : goTo("boxes")),
  },
  {
    id: "boxes",
    title: "Boîtes",
    sub: `${stored.value} Pokémon stockés · ${view.value.boxNames.length} boîtes`,
    band: "Ranger, copier, importer et exporter tes Pokémon",
    gradient: "linear-gradient(150deg, #5ad1ff, #2f7fe6 60%, #2457c9)",
    go: () => goTo("boxes"),
  },
  {
    id: "trainer",
    title: "Dresseur",
    sub: `${t.value.name} · ${t.value.money.toLocaleString("fr-FR")} ₽`,
    band: "Nom, ID, argent et temps de jeu",
    icon: "user",
    gradient: "linear-gradient(150deg, #b18cff, #7b5cf0 55%, #5b3fd1)",
    go: () => goTo("tools", "trainer"),
  },
  {
    id: "items",
    title: "Sac",
    sub: "Objets, CT, Baies, Poké Balls…",
    band: "Donner ou retirer des objets, poche par poche",
    icon: "bag",
    gradient: "linear-gradient(150deg, #ffd45c, #ffa23d 55%, #f2733a)",
    go: () => goTo("tools", "items"),
  },
  {
    id: "dex",
    title: "Pokédex",
    sub: "Vus et capturés",
    band: "Compléter ou corriger le Pokédex",
    icon: "book",
    gradient: "linear-gradient(150deg, #7cf0a4, #2fc27a 55%, #179a63)",
    go: () => goTo("tools", "dex"),
  },
  {
    id: "nuzlocke",
    title: "Nuzlocke",
    sub: "Routes, morts, niveau maximum",
    band: "Suivre un défi Nuzlocke avec la ROM (randomisée) de ta partie",
    icon: "swords",
    gradient: "linear-gradient(150deg, #ff7b7b, #c2365a 55%, #7a1f4a)",
    go: () => goTo("nuzlocke"),
  },
  {
    id: "manager",
    title: "Sauvegardes",
    sub: "Toutes tes parties",
    band: "Passer d'une sauvegarde à l'autre",
    icon: "folder",
    gradient: "linear-gradient(150deg, #e9f3ff, #b9d4f5 60%, #92b5e6)",
    go: () => goTo("manager"),
  },
  {
    id: "open",
    title: "Ouvrir",
    sub: "Une autre sauvegarde (Ctrl+O)",
    band: "Ouvrir un fichier de sauvegarde",
    icon: "folder-open",
    gradient: "linear-gradient(150deg, #8fa3c0, #5f7396 60%, #46587a)",
    go: () => emit("open"),
  },
  {
    id: "encounters",
    title: "Rencontres",
    sub: "Bientôt",
    band: "Où trouver chaque Pokémon (en préparation)",
    icon: "sparkle",
    gradient: "linear-gradient(150deg, #5fe0c2, #22a59a)",
    soon: true,
  },
  {
    id: "gifts",
    title: "Cadeaux mystère",
    sub: "Distributions Gen 4 à 7",
    band: "Recevoir les Pokémon et objets des événements officiels, ouvrir ou enregistrer des cartes cadeau",
    icon: "gift",
    gradient: "linear-gradient(150deg, #ffb36b, #f0703a)",
    go: () => goTo("gifts"),
  },
  {
    id: "bank",
    title: "Banque",
    sub: bankState.info ? `${bankState.info.count} Pokémon à l'abri` : "PC commun à tes sauvegardes",
    band: "Ranger tes Pokémon hors des sauvegardes et les transférer vers un jeu plus récent",
    icon: "bank",
    gradient: "linear-gradient(150deg, #9db4ff, #5a6fe0)",
    go: () => goTo("bank"),
  },
]);

const focus = ref(0);
const strip = ref<HTMLElement | null>(null);
const focused = computed(() => tiles.value[focus.value]);

function scrollToFocus() {
  // Défilement horizontal de la bande seulement (scrollIntoView ferait défiler toute la page).
  nextTick(() => {
    const s = strip.value;
    const el = s?.querySelector<HTMLElement>(`[data-i="${focus.value}"]`);
    if (!s || !el) return;
    const left = el.offsetLeft - 48;
    const right = el.offsetLeft + el.offsetWidth + 48 - s.clientWidth;
    if (s.scrollLeft > left) s.scrollTo({ left, behavior: "smooth" });
    else if (s.scrollLeft < right) s.scrollTo({ left: right, behavior: "smooth" });
  });
}

function move(d: number) {
  focus.value = Math.min(tiles.value.length - 1, Math.max(0, focus.value + d));
  scrollToFocus();
}

function activate(i = focus.value) {
  focus.value = i;
  const tile = tiles.value[i];
  if (!tile.soon) tile.go?.();
}

function onKey(e: KeyboardEvent) {
  if ((e.target as HTMLElement)?.tagName === "INPUT") return;
  if (e.key === "ArrowRight") move(1);
  else if (e.key === "ArrowLeft") move(-1);
  else return;
  e.preventDefault();
}

onMounted(() => {
  window.addEventListener("keydown", onKey);
  loadBankInfo();
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

useShell(() => ({
  hint: focused.value.band,
  actions: [
    { key: "Ctrl+s", cap: "Ctrl+S", label: "Enregistrer", run: () => writeSave(), disabled: !saveState.dirty },
    { key: "Enter", cap: "Entrée", label: focused.value.soon ? "Bientôt" : "Ouvrir", run: () => activate(), disabled: focused.value.soon },
  ],
}));

const time = ref(new Date().toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));

const dock: { icon: string; label: string; run: () => void; tone?: string }[] = [
  { icon: "folder-open", label: "Ouvrir une sauvegarde (Ctrl+O)", run: () => emit("open"), tone: "#ff7a6b" },
  { icon: "save", label: "Enregistrer (Ctrl+S)", run: () => writeSave() },
  { icon: "upload", label: "Enregistrer sous…", run: () => emit("save-as"), tone: "#ffc65c" },
  { icon: "play", label: "Jouer : lancer le jeu avec cette sauvegarde", run: () => playOpenSave(), tone: "#5ce0a0" },
  { icon: "send", label: "Envoyer au jeu (ferme le jeu avant)", run: () => sendToGame(), tone: "#5ad1ff" },
  { icon: "grid", label: "Boîtes (Ctrl+1)", run: () => goTo("boxes"), tone: "#7fb2ff" },
  { icon: "undo", label: "Annuler (Ctrl+Z)", run: () => history(false), tone: "#7ef0b0" },
  { icon: "settings", label: "Apparence de Kaleido", run: () => (nav.view = "settings") },
  { icon: "power", label: "Fermer la sauvegarde", run: () => closeSave() },
];

</script>

<template>
  <div class="home">
    <!-- Équipe en haut, comme les icônes de profil d'une console -->
    <div class="top">
      <div class="bubbles">
        <button class="bubble trainer" :title="`${t.name} — carte de dresseur`" @click="goTo('tools', 'trainer')">
          {{ t.name.slice(0, 1).toUpperCase() || "?" }}
        </button>
        <button
          v-for="p in view.party"
          :key="JSON.stringify(p.slot)"
          class="bubble"
          :class="{ on: lead && JSON.stringify(lead.slot) === JSON.stringify(p.slot) }"
          :title="`${p.nickname || p.speciesName} · N. ${p.level}`"
          @click="editPokemon(p)"
        >
          <Sprite :id="p.species" :shiny="p.shiny" :size="54" />
        </button>
      </div>
      <div class="corner">
        <LiveSyncBadge :send="false" />
        <span class="game">{{ view.game }}</span>
        <Icon name="save" :size="20" :class="{ dirty: saveState.dirty }" />
        <span class="clock">{{ time }}</span>
      </div>
    </div>

    <div class="band">
      <h2>{{ focused.title }}</h2>
      <span>{{ focused.sub }}</span>
    </div>

    <!-- Tuiles -->
    <div class="strip-wrap">
      <button v-if="focus > 0" class="arrow left" aria-label="Précédent" @click="move(-1)"><Icon name="chevron-left" /></button>
      <div ref="strip" class="strip">
        <button
          v-for="(tile, i) in tiles"
          :key="tile.id"
          :data-i="i"
          class="tile"
          :class="{ focus: i === focus, soon: tile.soon, [`t-${tile.id}`]: true }"
          :style="{ background: tile.gradient }"
          @mouseenter="focus = i"
          @focus="focus = i"
          @click="activate(i)"
        >
          <div class="art">
            <template v-if="tile.id === 'pokemon' && lead">
              <Sprite :id="lead.species" :shiny="lead.shiny" :form="lead.form" :gender="lead.gender" variant="model" :size="150" />
            </template>
            <div v-else-if="tile.id === 'boxes'" class="mini">
              <span v-for="p in boxPreview" :key="JSON.stringify(p!.slot)"><Sprite :id="p!.species" :shiny="p!.shiny" :size="52" /></span>
            </div>
            <Icon v-else :name="tile.icon ?? 'info'" :size="64" />
          </div>
          <div class="label">
            <strong>{{ tile.title }}</strong>
            <small>{{ tile.sub }}</small>
          </div>
          <span v-if="tile.soon" class="soon-chip">Bientôt</span>
        </button>
      </div>
      <button v-if="focus < tiles.length - 1" class="arrow right" aria-label="Suivant" @click="move(1)"><Icon name="chevron-right" /></button>
    </div>

    <!-- Dock d'actions rapides -->
    <div class="dock">
      <button v-for="d in dock" :key="d.label" class="dock-btn" :title="d.label" :aria-label="d.label" :style="{ color: d.tone }" @click="d.run">
        <Icon :name="d.icon" :size="24" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.home {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 26px 40px 18px;
}

.bubbles {
  display: flex;
  gap: 10px;
}

.bubble {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  padding: 0;
  overflow: hidden;
  border: 3px solid rgba(255, 255, 255, 0.85);
  border-radius: 50%;
  background: linear-gradient(160deg, #ffffff, #dfe8f5);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.2);
  transition: transform 0.15s, box-shadow 0.15s;
}

.bubble:hover,
.bubble.on {
  transform: translateY(-2px);
  box-shadow: 0 0 0 3px var(--accent-2), 0 8px 18px rgba(0, 0, 0, 0.25);
}

.bubble.trainer {
  color: #1d4fb8;
  font: 700 26px var(--font-display);
}

.corner {
  display: flex;
  align-items: center;
  gap: 14px;
}

.game {
  color: var(--text-dim);
  font-size: 13px;
}

.corner .dirty {
  color: var(--warn);
}

.clock {
  font-size: 26px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
}

.band {
  display: flex;
  align-items: baseline;
  gap: 26px;
  margin-right: 40px;
  padding: 12px 40px 12px 64px;
  background: color-mix(in srgb, var(--bg) 55%, rgba(0, 20, 60, 0.4));
  clip-path: polygon(0 0, calc(100% - 40px) 0, 100% 100%, 0 100%);
}

.band h2 {
  font-size: 26px;
  font-weight: 300;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--accent-2);
}

.band span {
  font-size: 14px;
}

.strip-wrap {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  min-height: 0;
}

.strip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 22px;
  width: 100%;
  padding: 30px 48px;
  overflow-x: auto;
  scroll-behavior: smooth;
  scrollbar-width: none;
}

.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  flex: 0 0 272px;
  height: 272px;
  padding: 18px 20px;
  border: none;
  border-radius: 6px;
  color: #fff;
  text-align: left;
  box-shadow: 0 12px 28px rgba(0, 20, 70, 0.3);
  transition: transform 0.18s, box-shadow 0.18s, filter 0.18s;
}

.tile.focus {
  transform: scale(1.04);
  box-shadow: 0 0 0 4px #fff, 0 0 0 9px color-mix(in srgb, var(--accent-2) 80%, transparent), 0 18px 34px rgba(0, 20, 70, 0.35);
}

.tile.soon {
  filter: saturate(0.55);
  cursor: default;
}

.t-manager {
  color: #1c3a66;
}

.art {
  position: absolute;
  inset: 16px 16px 72px;
  display: grid;
  place-items: center;
}

.art :deep(.icon) {
  stroke-width: 1.5;
  filter: drop-shadow(0 4px 8px rgba(0, 0, 0, 0.2));
}

.mini {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px 10px;
}

.mini span {
  display: grid;
  place-items: center;
  border-bottom: 3px solid rgba(255, 255, 255, 0.5);
}

.label strong {
  display: block;
  font-size: 21px;
  text-shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
}

.label small {
  display: block;
  margin-top: 2px;
  font-size: 13px;
  opacity: 0.92;
}

.soon-chip {
  position: absolute;
  top: 12px;
  right: 12px;
  padding: 3px 10px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.25);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
}

.arrow {
  position: absolute;
  z-index: 2;
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border: 2px solid #fff;
  border-radius: 50%;
  background: rgba(20, 60, 140, 0.7);
  color: #fff;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.arrow.left {
  left: 10px;
}

.arrow.right {
  right: 10px;
}

.dock {
  display: flex;
  justify-content: center;
  gap: 22px;
  align-self: center;
  margin-bottom: 18px;
  padding: 18px 40px;
  border: 2px solid rgba(255, 255, 255, 0.55);
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 16%, transparent);
  backdrop-filter: blur(10px);
}

.dock-btn {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  border: none;
  border-radius: 50%;
  background: #3b4250;
  color: #fff;
  box-shadow: 0 6px 14px rgba(0, 0, 0, 0.3);
  transition: transform 0.15s;
}

.dock-btn:hover {
  transform: translateY(-3px) scale(1.05);
}
</style>
