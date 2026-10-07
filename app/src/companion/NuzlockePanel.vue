<script setup lang="ts">
import Icon from "../components/Icon.vue";
import Tip from "../components/Tip.vue";
import { markMissed, openInMain, trackNuzlocke, type NuzlockeSummary } from "./store";

/**
 * Suivi Nuzlocke en bas de l'équipe : trois compteurs sur une ligne, puis la route actuelle
 * avec une phrase claire sur ce qu'il reste à faire, et les alertes.
 */
defineProps<{ summary: NuzlockeSummary | null; canTrack: boolean; place: string | null }>();

/** Ce que dit la route, en une phrase. */
const HERE = {
  pending: { text: "pas encore de capture ici", cls: "" },
  caught: { text: "capture faite", cls: "ok" },
  missed: { text: "rencontre ratée", cls: "danger" },
  dupeOnly: { text: "seulement des doublons pour l'instant", cls: "warn" },
} as const;
</script>

<template>
  <section v-if="summary" class="nuz sv-card">
    <header>
      <span class="sv-label">Nuzlocke</span>
      <button v-if="summary.unassigned" type="button" class="link" @click="openInMain(null, 'nuzlocke')">
        {{ summary.unassigned }} lieu{{ summary.unassigned > 1 ? "x" : "" }} à rattacher
      </button>
      <Tip v-if="summary.unassigned" term="nuzlocke.unassigned" />
    </header>
    <dl class="stats">
      <div>
        <dt>Morts <Tip term="nuzlocke.graveyard" /></dt>
        <dd :class="{ danger: summary.dead > 0 }">{{ summary.dead }}</dd>
      </div>
      <div>
        <dt>Captures</dt>
        <dd>{{ summary.captures }}</dd>
      </div>
      <div v-if="summary.levelCap">
        <dt>Niveau max <Tip term="nuzlocke.levelCaps" /></dt>
        <dd>{{ summary.levelCap }}</dd>
      </div>
    </dl>

    <p v-if="summary.here" class="here">
      <Icon name="map" :size="14" />
      <span>
        <strong>{{ summary.here.name }}</strong> :
        <span :class="HERE[summary.here.status].cls">{{ summary.here.capture ? `capture faite (${summary.here.capture})` : HERE[summary.here.status].text }}</span>
      </span>
      <button
        v-if="summary.here.status === 'pending' || summary.here.markedMissed"
        type="button"
        class="link"
        :title="summary.here.markedMissed ? '' : 'Le Pokémon de la route est K.O. ou s’est enfui : plus de capture possible ici'"
        @click="markMissed(summary.here.key, !summary.here.markedMissed)"
      >
        {{ summary.here.markedMissed ? "annuler" : "marquer ratée" }}
      </button>
      <Tip term="nuzlocke.missed" />
    </p>
    <p v-else-if="place" class="here dim"><Icon name="map" :size="14" /> {{ place }} : pas de rencontre sauvage ici</p>

    <ul v-if="summary.warnings.length" class="warnings">
      <li v-for="w in summary.warnings" :key="w.title" :class="w.error ? 'danger' : 'warn'" :title="w.detail">
        <Icon name="alert" :size="13" /> {{ w.title }}
      </li>
    </ul>
  </section>

  <section v-else-if="canTrack" class="nuz sv-card offer">
    <p>
      <strong>Suivre cette partie en Nuzlocke</strong><Tip term="companion.track" /><br />
      <small class="dim">Captures par route, morts et niveau maximum relevés tout seuls.</small>
    </p>
    <button type="button" class="sv-btn solid small" @click="trackNuzlocke">Activer</button>
  </section>
</template>

<style scoped>
.nuz {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  padding: var(--sp-3);
}

header {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

header .sv-label {
  flex: 1;
}

.stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--sp-2);
  margin: 0;
}

.stats div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stats dt {
  display: inline-flex;
  align-items: center;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.stats dd {
  margin: 0;
  font-size: var(--fs-lg);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.here {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1) var(--sp-2);
  margin: 0;
  font-size: var(--fs-md);
}

.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent-2);
  font: inherit;
  font-size: var(--fs-md);
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}

.ok {
  color: var(--ok);
}

.warn {
  color: var(--warn);
}

.danger {
  color: var(--danger);
}

.dim,
small {
  color: var(--text-dim);
}

.warnings {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-md);
}

.offer {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.offer p {
  margin: 0;
}
</style>
