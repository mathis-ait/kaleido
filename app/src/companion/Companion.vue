<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import Banner from "../components/Banner.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import BoxGrid from "./BoxGrid.vue";
import JournalList from "./JournalList.vue";
import LiveBattle from "./LiveBattle.vue";
import NextBattleCard from "./NextBattle.vue";
import NuzlockePanel from "./NuzlockePanel.vue";
import TeamCard from "./TeamCard.vue";
import { companion, initCompanion, monName, openInMain, refresh, setAutoOpen, setCompact, setMemory, setOnTop } from "./store";

/**
 * Compagnon de partie : fenêtre étroite (420 à 560 px) à côté de l'émulateur. Tout se met
 * à jour seul à chaque sauvegarde en jeu ; rien n'est écrit dans la sauvegarde.
 */

const tab = ref<"team" | "boxes" | "journal">("team");
const openMon = ref<string | null>(null);

const state = computed(() => companion.state);
const snap = computed(() => companion.snapshot);
const playing = computed(() => companion.running.length > 0);
/** Pastille : mémoire lue (« En direct »), émulateur lancé (« En jeu »), aucun émulateur. */
const status = computed(() => state.value?.live?.status ?? (playing.value ? "ingame" : "offline"));
const STATUS_LABEL = { live: "En direct", ingame: "En jeu", offline: "Hors ligne" } as const;
const STATUS_TONE = { live: "ok", ingame: "accent", offline: "dim" } as const;
const memoryOn = computed(() => state.value?.live?.enabled ?? true);
const STATUS_HINT = {
  live: "Kaleido lit la mémoire de l’émulateur : PV et combats à la seconde",
  ingame: "Émulateur lancé : le compagnon suit les sauvegardes en jeu",
  offline: "Aucun émulateur lancé",
} as const;
const statusHint = computed(() => STATUS_HINT[status.value]);
/** Menu des réglages (mémoire, premier plan, ouverture auto, éditeur). */
const menu = ref(false);

// Horloge pour « il y a 2 min » (rafraîchie toutes les 15 s).
const now = ref(Date.now());
let timer: number | undefined;
onMounted(() => {
  initCompanion();
  timer = window.setInterval(() => (now.value = Date.now()), 15_000);
});
onBeforeUnmount(() => clearInterval(timer));

function ago(ms: number | null | undefined) {
  if (!ms) return "jamais";
  const s = Math.max(0, Math.round((now.value - ms) / 1000));
  if (s < 60) return "à l'instant";
  const m = Math.round(s / 60);
  if (m < 60) return `il y a ${m} min`;
  const time = new Date(ms).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  const sameDay = new Date(ms).toDateString() === new Date(now.value).toDateString();
  return sameDay ? `à ${time}` : `le ${new Date(ms).toLocaleDateString("fr-FR")} à ${time}`;
}

const playTime = computed(() => {
  const t = snap.value?.playTime;
  return t ? `${t.hours} h ${String(t.minutes).padStart(2, "0")}` : "";
});
const badgeCount = computed(() => {
  const b = snap.value?.badges;
  return b === null || b === undefined ? null : [...b.toString(2)].filter((c) => c === "1").length;
});
const badgeLabel = computed(() => {
  const one = (badgeCount.value ?? 0) <= 1;
  return snap.value?.generation === 7 ? (one ? "épreuve" : "épreuves") : one ? "badge" : "badges";
});
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

const tabs = [
  { value: "team" as const, label: "Équipe" },
  { value: "boxes" as const, label: "Boîtes" },
  { value: "journal" as const, label: "Journal" },
];
</script>

<template>
  <div class="companion" :class="{ compact: companion.compact }">
    <!-- Mode barre : une seule ligne avec l'équipe et ses PV -->
    <template v-if="companion.compact">
      <div class="bar">
        <span class="dot" :class="{ on: status !== 'offline' }" :title="STATUS_LABEL[status]" />
        <ul v-if="snap" class="minis">
          <li v-for="m in snap.party" :key="m.uid" :title="`${monName(m)} · N. ${m.level} · ${m.hp}/${m.maxHp} PV`">
            <Sprite :id="m.isEgg ? 0 : m.species" :form="m.form" :shiny="m.shiny" :size="36" />
            <span v-if="!m.isEgg && m.maxHp" class="mini-hp" :class="{ low: m.hp / m.maxHp <= 0.2, ko: m.hp === 0 }">
              <i :style="{ width: `${Math.round((m.hp / m.maxHp) * 100)}%` }" />
            </span>
          </li>
        </ul>
        <span v-else class="dim">En attente de la sauvegarde…</span>
        <small class="dim when">{{ ago(state?.modified) }}</small>
        <button type="button" class="sv-round sq" title="Vue complète" aria-label="Vue complète" @click="setCompact(false)">
          <Icon name="plus" :size="16" />
        </button>
      </div>
    </template>

    <template v-else>
      <header class="top">
        <div class="title">
          <h1><span class="name">{{ state?.title || snap?.game || "Compagnon" }}</span><Tip term="companion.companion" /></h1>
          <p class="meta">
            <span class="sv-chip" :class="STATUS_TONE[status]" :title="statusHint">{{ STATUS_LABEL[status] }}</span>
            <template v-if="snap">
              <span v-if="state?.place">{{ state.place }}</span>
              <span>{{ playTime }}</span>
              <span v-if="badgeCount !== null">{{ badgeCount }} {{ badgeLabel }}</span>
            </template>
          </p>
        </div>
        <div class="tools">
          <button
            type="button"
            class="sv-round sq"
            :class="{ active: menu }"
            :aria-expanded="menu"
            aria-label="Réglages du compagnon"
            title="Réglages du compagnon"
            @click="menu = !menu"
          >
            <Icon name="settings" :size="15" />
          </button>
          <button type="button" class="sv-round sq" title="Réduire en barre" aria-label="Réduire en barre" @click="setCompact(true)">
            <Icon name="minus" :size="16" />
          </button>
        </div>
      </header>

      <div v-if="menu && state" class="menu" role="dialog" aria-label="Réglages du compagnon">
        <p class="dim small">
          Sauvegarde lue {{ ago(state.modified) }}. <Tip term="companion.saveTiming" />
          <template v-if="status === 'live' && state.live && !state.live.verified"><br />Lecture en direct non vérifiée pour ce jeu. <Tip term="companion.unverified" /></template>
          <template v-else-if="status === 'ingame' && state.live?.enabled && state.live.detail"><br />{{ state.live.detail }}</template>
        </p>
        <Toggle :model-value="memoryOn" label="Lire la mémoire de l’émulateur" term="companion.memory" @update:model-value="setMemory" />
        <Toggle :model-value="!!state.onTop" label="Toujours au premier plan" term="companion.onTop" @update:model-value="setOnTop" />
        <Toggle v-if="state.key" :model-value="state.autoOpen" label="Ouvrir à chaque partie" term="companion.autoOpen" @update:model-value="setAutoOpen" />
        <span class="edit">
          <button
            type="button"
            class="sv-btn small"
            :disabled="playing"
            :title="playing ? 'Ferme d’abord l’émulateur : écrire pendant qu’il tourne ferait perdre une des deux versions' : 'Ouvre cette sauvegarde dans l’éditeur (copie de secours automatique)'"
            @click="openInMain(null)"
          >
            <Icon name="pencil" :size="13" /> Modifier dans l'éditeur
          </button>
          <Tip term="companion.readOnly" />
        </span>
      </div>

      <main class="body">
        <EmptyState v-if="!companion.loaded" loading compact title="Lecture de la sauvegarde…" />
        <EmptyState v-else-if="!state" icon="play" title="Aucune partie suivie">
          Lance un jeu depuis la bibliothèque de Kaleido : le compagnon suivra sa sauvegarde.
        </EmptyState>
        <template v-else>
          <Banner v-if="state.error && snap" tone="warn" :retry="refresh">
            Dernière lecture impossible ({{ state.error }}). Affichage de la sauvegarde précédente.
          </Banner>
          <EmptyState v-if="!snap && !state.exists" icon="clock" title="En attente de la première sauvegarde">
            Sauvegarde une fois en jeu : le compagnon affichera ton équipe dans la seconde.
            <template #details><small class="dim path" :title="state.path">{{ fileName(state.path) }}</small></template>
          </EmptyState>
          <EmptyState v-else-if="!snap" icon="alert" title="Sauvegarde illisible">
            {{ state.error }}
            <template #actions><button type="button" class="sv-btn" @click="refresh">Réessayer</button></template>
          </EmptyState>

          <template v-if="snap">
            <Segmented v-model="tab" :options="tabs" label="Affichage" />

            <template v-if="tab === 'team'">
              <LiveBattle v-if="state.battle?.foes.length || state.encounter" :battle="state.battle" :encounter="state.encounter" :place="state.place" />
              <NextBattleCard v-else-if="state.nextBattle" :battle="state.nextBattle" />
              <ul class="team">
                <TeamCard v-for="m in snap.party" :key="m.uid" :mon="m" :open="openMon === m.uid" @toggle="openMon = openMon === m.uid ? null : m.uid" />
                <li v-if="!snap.party.length" class="none">
                  <EmptyState compact icon="ball" title="Équipe vide">Ton équipe apparaîtra ici dès ton premier Pokémon.</EmptyState>
                </li>
              </ul>
              <NuzlockePanel :summary="state.nuzlocke" :can-track="state.canTrack" :place="state.place" />
            </template>
            <BoxGrid v-else-if="tab === 'boxes'" :snapshot="snap" />
            <JournalList v-else :entries="state.journal" />
          </template>
        </template>
      </main>

    </template>

    <TransitionGroup name="notice" tag="ul" class="notices" aria-live="polite">
      <li v-for="n in companion.notices" :key="n.id">{{ n.text }}</li>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.companion {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.dim {
  color: var(--text-dim);
}

/* ---- Vue complète ---- */
.top {
  display: flex;
  align-items: flex-start;
  gap: var(--sp-3);
  padding: var(--sp-4) var(--sp-4) var(--sp-3);
  border-bottom: 1px solid var(--border);
}

.title {
  flex: 1;
  min-width: 0;
}

/* Points de suspension sur le nom seul : sur le h1 en flex, ils ne s'affichaient pas et le « i » était rogné. */
h1 {
  display: flex;
  align-items: center;
  min-width: 0;
  font-size: var(--fs-lg);
}

h1 .name {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1) var(--sp-3);
  margin: var(--sp-1) 0 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.menu {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--sp-2);
  padding: var(--sp-3) var(--sp-4);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}

.small {
  margin: 0;
  font-size: var(--fs-sm);
}

.title p {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  margin: var(--sp-1) 0 0;
  font-size: var(--fs-sm);
}


.tools {
  display: flex;
  gap: var(--sp-1);
}

/* Le survol de .sv-round ne doit pas effacer l'état actif (même spécificité). */
.sv-round.active,
.sv-round.active:hover:not(:disabled) {
  border-color: var(--text);
  background: var(--text);
  color: var(--bg);
}

.body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--sp-3);
  min-height: 0;
  padding: var(--sp-3) var(--sp-4);
  overflow-y: auto;
}




.team {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  margin: 0;
  padding: 0;
}

.none {
  list-style: none;
}

.path {
  display: block;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}


.edit {
  display: inline-flex;
  align-items: center;
}

/* ---- Mode barre ---- */
.bar {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  height: 100%;
  padding: 0 var(--sp-3);
}

.dot {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-dim);
}

.dot.on {
  background: var(--ok);
}

.minis {
  display: flex;
  flex: 1;
  gap: var(--sp-1);
  min-width: 0;
  margin: 0;
  padding: 0;
  list-style: none;
}

.minis li {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  width: 40px;
}

.mini-hp {
  width: 32px;
  height: 4px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 16%, transparent);
  overflow: hidden;
}

.mini-hp i {
  display: block;
  height: 100%;
  background: var(--ok);
}

.mini-hp.low i {
  background: var(--danger);
}

.mini-hp.ko {
  outline: 1px solid var(--danger);
}

.when {
  flex-shrink: 0;
  font-size: var(--fs-sm);
}

/* ---- Notifications discrètes ---- */
/* Au-dessus du pied de fenêtre, pour ne pas cacher ses réglages. */
.notices {
  position: absolute;
  right: var(--sp-3);
  bottom: 56px;
  left: var(--sp-3);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-1);
  margin: 0;
  padding: 0;
  list-style: none;
  pointer-events: none;
}

.compact .notices {
  display: none;
}

.notices li {
  padding: 6px 14px;
  border-radius: var(--radius-pill);
  background: var(--text);
  color: var(--bg);
  font-size: var(--fs-md);
  font-weight: 600;
  box-shadow: var(--shadow);
}

.notice-enter-active,
.notice-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.notice-enter-from,
.notice-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
