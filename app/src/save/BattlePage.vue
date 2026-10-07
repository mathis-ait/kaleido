<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Banner from "../components/Banner.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import { battle, computeMatrix, linkRom, restoreLink, unlinkRom } from "../battle";
import { library } from "../library";
import { lists, loadLists, saveState } from "../saveStore";
import { isKaleidoRom, isRom } from "../types";
import BattleSettings from "./battle/BattleSettings.vue";
import DuelPanel from "./battle/DuelPanel.vue";
import MatrixGrid from "./battle/MatrixGrid.vue";
import TrainerList from "./battle/TrainerList.vue";
import { useShell } from "./shell";

const view = computed(() => saveState.view);

/** ROM de la bibliothèque de la même génération que la sauvegarde (le moteur vérifie le jeu exact). */
const candidates = computed(() => library.items.filter((d) => isRom(d) && d.generation === view.value?.generation));

const trainer = computed(() => battle.matrix?.trainer ?? battle.link?.trainers.find((t) => t.id === battle.trainerId) ?? null);
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Relecture de la ROM déjà liée à cette sauvegarde, à l'ouverture de la page. */
const restoring = ref(false);

async function pickRom(folder = false) {
  const path = await open({
    title: folder ? "Dossier d'un jeu 3DS extrait" : "ROM du jeu de cette sauvegarde",
    directory: folder,
    filters: folder ? undefined : [{ name: "ROM DS / 3DS", extensions: ["nds", "3ds", "cci", "cxi", "app"] }],
  });
  if (typeof path === "string") await linkRom(path, saveState.path);
}

// Les réglages changent : on recalcule (un peu après la dernière modification).
let timer: number | undefined;
watch(
  () => battle.options,
  () => {
    clearTimeout(timer);
    timer = window.setTimeout(() => computeMatrix(), 200);
  },
  { deep: true },
);

onMounted(async () => {
  if (!lists.loaded) loadLists();
  restoring.value = true;
  try {
    await restoreLink(saveState.path);
  } finally {
    restoring.value = false;
  }
});

useShell(() => ({
  hint: battle.link
    ? "Choisis un dresseur, puis clique sur une case de la matrice pour le détail des attaques"
    : "Lie la ROM de ta partie pour préparer tes combats",
  actions: [{ key: "Ctrl+l", cap: "Ctrl+L", label: battle.link ? "Changer de ROM" : "Lier une ROM", run: () => pickRom() }],
}));
</script>

<template>
  <div class="battle">
    <Banner v-if="battle.error" :dismiss="() => (battle.error = null)">{{ battle.error }}</Banner>

    <!-- Relecture de la ROM déjà liée -->
    <section v-if="restoring && !battle.link" class="sv-panel">
      <EmptyState loading title="Lecture de la ROM liée…">Kaleido relit les dresseurs de ta partie.</EmptyState>
    </section>

    <!-- Pas encore de ROM liée -->
    <section v-else-if="!battle.link" class="sv-panel">
      <EmptyState icon="swords" title="Préparer un combat" term="battle.linkRom">
        Lie la ROM de ta partie — l'originale ou celle randomisée par Kaleido. Kaleido y lit tous les dresseurs (Champions, rivaux, Conseil 4…)
        avec leurs vraies équipes, puis calcule les dégâts entre ton équipe et la leur : fourchette de dégâts, nombre de coups pour mettre
        K.O., qui attaque en premier.
        <template #details>
          <p class="sv-help">
            Jeux pris en charge : Diamant / Perle, Platine, HeartGold / SoulSilver, Noir / Blanc, Noir 2 / Blanc 2, Rubis Oméga / Saphir Alpha,
            X / Y. La ROM doit être du même jeu que la sauvegarde ({{ view?.game }}).
          </p>
          <div v-if="candidates.length" class="cands">
            <span class="sv-label">Dans ta bibliothèque</span>
            <button
              v-for="c in candidates"
              :key="c.path"
              type="button"
              class="cand"
              :disabled="battle.linking"
              @click="linkRom(c.path, saveState.path)"
            >
              <Icon name="file" :size="16" />
              <span class="cand-name">
                <strong>{{ c.game?.name ?? c.title }}</strong>
                <small>{{ c.fileName }}</small>
              </span>
              <span v-if="isKaleidoRom(c)" class="sv-chip">Randomisée</span>
            </button>
          </div>
        </template>
        <template #actions>
          <button type="button" class="sv-btn solid" :disabled="battle.linking" @click="pickRom()">
            <Icon v-if="battle.linking" name="refresh" :size="16" class="sv-spin" />
            <Icon v-else name="folder-open" :size="16" />
            {{ battle.linking ? "Lecture de la ROM…" : "Choisir une ROM…" }}
          </button>
          <button type="button" class="sv-btn" :disabled="battle.linking" @click="pickRom(true)">
            <Icon name="folder" :size="16" /> Dossier 3DS extrait…
          </button>
        </template>
      </EmptyState>
    </section>

    <template v-else>
      <header class="linked">
        <Icon name="swords" :size="18" />
        <span>
          <strong>{{ battle.link.game }}</strong>
          <small :title="battle.link.path">{{ fileName(battle.link.path) }}</small>
        </span>
        <span v-if="!battle.link.verified" class="unverified">
          <span class="sv-chip warn">À l'essai</span><Tip term="battle.unverified" />
        </span>
        <span class="grow" />
        <button type="button" class="sv-btn" :disabled="battle.linking" @click="pickRom()">
          <Icon name="refresh" :size="14" /> Changer de ROM
        </button>
        <button type="button" class="sv-btn" @click="unlinkRom(saveState.path)"><Icon name="x" :size="14" /> Délier</button>
      </header>

      <div class="layout">
        <TrainerList />
        <div class="main">
          <section v-if="!trainer" class="sv-panel">
            <EmptyState compact icon="swords" title="Aucun dresseur choisi">
              Choisis un dresseur à gauche pour voir comment ton équipe s'en sort contre la sienne.
            </EmptyState>
          </section>
          <template v-else>
            <div class="trainer-head">
              <span v-if="trainer.roleLabel" class="role">
                <span class="sv-chip accent">{{ trainer.roleLabel }}</span><Tip term="battle.roles" />
              </span>
              <h2>{{ trainer.className }} {{ trainer.name }}</h2>
              <small>
                {{ trainer.team.length }} Pokémon · jusqu'au N. {{ trainer.maxLevel }}
                <template v-if="trainer.double">· combat en duo <Tip term="battle.double" /></template>
              </small>
              <small class="tips">
                IV <Tip term="battle.trainerIvs" /> · attaques <Tip term="battle.trainerMoves" /> · nature <Tip term="battle.trainerNature" />
              </small>
            </div>
            <BattleSettings />
            <section v-if="battle.computing && !battle.matrix" class="sv-panel">
              <EmptyState compact loading title="Calcul des dégâts…" />
            </section>
            <MatrixGrid />
            <DuelPanel />
          </template>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.battle {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  height: 100%;
  min-height: 0;
}

.cands {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  width: 100%;
  margin-top: var(--sp-2);
  text-align: left;
}

.cand {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: transparent;
  color: var(--text);
  text-align: left;
}

.cand:hover:not(:disabled) {
  background: var(--panel-hover);
}

.cand:disabled {
  opacity: 0.4;
  cursor: default;
}

.cand-name {
  flex: 1;
  min-width: 0;
}

.cand small {
  display: block;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.linked {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
}

.linked small {
  display: block;
  color: var(--text-dim);
  font-size: var(--fs-xs);
}

.unverified,
.role {
  display: inline-flex;
  align-items: center;
}

.grow {
  flex: 1;
}

.layout {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: var(--sp-4);
  flex: 1;
  min-height: 0;
}

.main {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  padding-right: var(--sp-1);
}

.trainer-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1) var(--sp-3);
}

.trainer-head h2 {
  margin: 0;
  font-size: var(--fs-xl);
}

.trainer-head small {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.tips {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}
</style>
