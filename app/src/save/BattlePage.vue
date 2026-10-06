<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import { battle, computeMatrix, linkRom, restoreLink, ROLE_COLORS, unlinkRom } from "../battle";
import { library } from "../library";
import { lists, loadLists, saveState } from "../saveStore";
import { isKaleidoRom, isRom } from "../types";
import BattleSettings from "./battle/BattleSettings.vue";
import DuelPanel from "./battle/DuelPanel.vue";
import MatrixGrid from "./battle/MatrixGrid.vue";
import TrainerList from "./battle/TrainerList.vue";
import { BATTLE_TIPS } from "./battle/glossary";
import { useShell } from "./shell";

const view = computed(() => saveState.view);

/** ROM de la bibliothèque de la même génération que la sauvegarde (le moteur vérifie le jeu exact). */
const candidates = computed(() => library.items.filter((d) => isRom(d) && d.generation === view.value?.generation));

const trainer = computed(() => battle.matrix?.trainer ?? battle.link?.trainers.find((t) => t.id === battle.trainerId) ?? null);
const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;

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

onMounted(() => {
  if (!lists.loaded) loadLists();
  restoreLink(saveState.path);
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
    <div v-if="battle.error" class="banner danger" role="alert">
      <Icon name="alert" :size="16" /> {{ battle.error }}
      <button class="x" aria-label="Fermer" @click="battle.error = null"><Icon name="x" :size="14" /></button>
    </div>

    <!-- Pas encore de ROM liée -->
    <section v-if="!battle.link" class="empty sv-panel">
      <span class="ico"><Icon name="swords" :size="34" /></span>
      <h2>Préparer un combat <Tip v-bind="BATTLE_TIPS.linkRom" /></h2>
      <p>
        Lie la ROM de ta partie — l'originale ou celle randomisée par Kaleido. Kaleido y lit tous les dresseurs (Champions, rivaux, Conseil 4…)
        avec leurs vraies équipes, puis calcule les dégâts entre ton équipe et la leur : fourchette de dégâts, nombre de coups pour mettre
        K.O., qui attaque en premier.
      </p>
      <p class="sv-help">
        Jeux pris en charge : Diamant / Perle, Platine, HeartGold / SoulSilver, Noir / Blanc, Noir 2 / Blanc 2, Rubis Oméga / Saphir Alpha, X / Y. La ROM doit être du même jeu que la
        sauvegarde ({{ view?.game }}).
      </p>
      <div v-if="candidates.length" class="cands">
        <span class="sv-label">Dans ta bibliothèque</span>
        <button v-for="c in candidates" :key="c.path" class="cand" :disabled="battle.linking" @click="linkRom(c.path, saveState.path)">
          <Icon name="file" :size="16" />
          <span>
            <strong>{{ c.game?.name ?? c.title }}</strong>
            <small>{{ c.fileName }}</small>
          </span>
          <span v-if="isKaleidoRom(c)" class="tag">Randomisée</span>
        </button>
      </div>
      <div class="sv-row">
        <button class="sv-btn solid" :disabled="battle.linking" @click="pickRom()">
          <Icon name="folder-open" :size="16" /> {{ battle.linking ? "Lecture de la ROM…" : "Choisir une ROM…" }}
        </button>
        <button class="sv-btn" :disabled="battle.linking" @click="pickRom(true)"><Icon name="folder" :size="16" /> Dossier 3DS extrait…</button>
      </div>
    </section>

    <template v-else>
      <header class="linked">
        <Icon name="swords" :size="18" />
        <span>
          <strong>{{ battle.link.game }}</strong>
          <small :title="battle.link.path">{{ fileName(battle.link.path) }}</small>
        </span>
        <span v-if="!battle.link.verified" class="tag warn" title="Emplacements des données non vérifiés sur une vraie ROM de ce jeu">À l'essai</span>
        <span class="grow" />
        <button class="sv-btn" :disabled="battle.linking" @click="pickRom()"><Icon name="refresh" :size="14" /> Changer de ROM</button>
        <button class="sv-btn" @click="unlinkRom(saveState.path)"><Icon name="x" :size="14" /> Délier</button>
      </header>

      <div class="layout">
        <TrainerList />
        <div class="main">
          <section v-if="!trainer" class="pick sv-panel">
            <Icon name="swords" :size="40" />
            <p>Choisis un dresseur à gauche pour voir comment ton équipe s'en sort contre la sienne.</p>
          </section>
          <template v-else>
            <div class="trainer-head" :style="{ '--role': trainer.role ? ROLE_COLORS[trainer.role] : 'var(--text-dim)' }">
              <span v-if="trainer.roleLabel" class="chip">{{ trainer.roleLabel }}</span>
              <h2>{{ trainer.className }} {{ trainer.name }}</h2>
              <small>{{ trainer.team.length }} Pokémon · niveau max {{ trainer.maxLevel }}<template v-if="trainer.double"> · combat en duo</template></small>
              <span class="tips">
                <Tip v-bind="BATTLE_TIPS.trainerIvs" />
                <Tip v-bind="BATTLE_TIPS.trainerMoves" />
                <Tip v-bind="BATTLE_TIPS.trainerNature" />
              </span>
            </div>
            <BattleSettings />
            <p v-if="battle.computing && !battle.matrix" class="sv-help">Calcul…</p>
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
  gap: 12px;
  height: 100%;
  min-height: 0;
}

.banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 14px;
  border-radius: 12px;
  font-size: 13px;
}

.banner.danger {
  border: 1px solid color-mix(in srgb, var(--danger) 50%, transparent);
  background: color-mix(in srgb, var(--danger) 12%, transparent);
}

.banner .x {
  margin-left: auto;
  color: inherit;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  max-width: 760px;
  padding: 26px 28px;
}

.empty h2 {
  display: flex;
  align-items: center;
  margin: 0;
  font-size: 22px;
}

.empty p {
  margin: 0;
  line-height: 1.55;
}

.ico {
  display: grid;
  place-items: center;
  width: 60px;
  height: 60px;
  border-radius: 50%;
  background: linear-gradient(150deg, #ff7a6b, #e0457b);
  color: #fff;
}

.cands {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
}

.cand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: 12px;
  text-align: left;
}

.cand:hover:not(:disabled) {
  background: var(--panel-hover);
}

.cand small {
  display: block;
  color: var(--text-dim);
}

.tag {
  margin-left: auto;
  padding: 1px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 20%, transparent);
  font-size: 11px;
  font-weight: 700;
}

.tag.warn {
  margin-left: 0;
  background: var(--warn-bg);
  color: var(--warn);
}

.linked {
  display: flex;
  align-items: center;
  gap: 10px;
}

.linked small {
  display: block;
  color: var(--text-dim);
  font-size: 11px;
}

.grow {
  flex: 1;
}

.layout {
  display: grid;
  grid-template-columns: 320px minmax(0, 1fr);
  gap: 14px;
  flex: 1;
  min-height: 0;
}

.main {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.pick {
  display: grid;
  place-items: center;
  gap: 8px;
  padding: 40px;
  color: var(--text-dim);
  text-align: center;
}

.trainer-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 12px;
  padding-left: 12px;
  border-left: 4px solid var(--role);
}

.trainer-head h2 {
  margin: 0;
  font-size: 20px;
}

.trainer-head small {
  color: var(--text-dim);
}

.trainer-head .chip {
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--role);
  color: #111;
  font-size: 11px;
  font-weight: 700;
}

.tips {
  display: inline-flex;
}
</style>
