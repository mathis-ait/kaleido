<script setup lang="ts">
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import { markMissed, trackNuzlocke, type NuzlockeSummary } from "./store";

/** Suivi Nuzlocke dans le compagnon : où l'on est, la rencontre du lieu, le prochain champion, les alertes. */
defineProps<{ summary: NuzlockeSummary | null; canTrack: boolean; place: string | null }>();

const STATUS = {
  pending: { label: "Rencontre à faire", cls: "dim" },
  caught: { label: "Capturé", cls: "ok" },
  missed: { label: "Ratée", cls: "danger" },
  dupeOnly: { label: "Doublons seulement", cls: "warn" },
} as const;
</script>

<template>
  <section v-if="summary" class="nuz sv-card">
    <div class="stats">
      <div>
        <span class="sv-label">Captures</span>
        <strong>{{ summary.captures }}</strong>
        <small>{{ summary.routesCaught }} / {{ summary.routes }} routes</small>
      </div>
      <div>
        <span class="sv-label">Morts <Tip term="nuzlocke.graveyard" /></span>
        <strong>{{ summary.dead }}</strong>
        <small>{{ summary.alive }} en vie</small>
      </div>
      <div v-if="summary.levelCap">
        <span class="sv-label">Niveau max <Tip term="nuzlocke.levelCaps" /></span>
        <strong>N. {{ summary.levelCap }}</strong>
      </div>
    </div>

    <div v-if="summary.here" class="here">
      <span class="where"><Icon name="map" :size="14" /> {{ summary.here.name }}</span>
      <span class="sv-chip" :class="STATUS[summary.here.status].cls">
        {{ summary.here.capture ?? STATUS[summary.here.status].label }}
      </span>
      <button
        v-if="summary.here.status === 'pending' || summary.here.markedMissed"
        type="button"
        class="sv-btn small"
        @click="markMissed(summary.here.key, !summary.here.markedMissed)"
      >
        {{ summary.here.markedMissed ? "Annuler « ratée »" : "Rencontre ratée ici" }}
      </button>
      <Tip term="nuzlocke.missed" />
    </div>
    <p v-else-if="place" class="here dim"><Icon name="map" :size="14" /> {{ place }} : pas de rencontre sauvage ici</p>

    <div v-if="summary.next" class="next">
      <Sprite :id="summary.next.aceSpecies" :size="40" />
      <span>
        <span class="sv-label">Prochain combat</span>
        <span><strong>{{ summary.next.name }}</strong> · {{ summary.next.label }}<template v-if="summary.next.town"> · {{ summary.next.town }}</template></span>
        <small>Pokémon le plus fort : N. {{ summary.next.aceLevel }}</small>
      </span>
    </div>

    <ul v-if="summary.warnings.length" class="warnings">
      <li v-for="w in summary.warnings" :key="w.title" :class="w.error ? 'danger' : 'warn'" :title="w.detail">
        <Icon name="alert" :size="13" /> {{ w.title }}
      </li>
    </ul>
  </section>

  <section v-else-if="canTrack" class="nuz sv-card offer">
    <p>
      <strong>Suivre cette partie en Nuzlocke</strong><Tip term="companion.track" /><br />
      <small class="dim">Captures par route, morts et niveau maximum relevés à chaque sauvegarde.</small>
    </p>
    <button type="button" class="sv-btn solid small" @click="trackNuzlocke">Activer</button>
  </section>
</template>

<style scoped>
.nuz {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  padding: var(--sp-3);
}

.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(90px, 1fr));
  gap: var(--sp-2);
}

.stats div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stats strong {
  font-size: var(--fs-lg);
}

small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.dim {
  color: var(--text-dim);
}

.here {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  margin: 0;
  font-size: var(--fs-md);
}

.where {
  display: inline-flex;
  align-items: center;
  gap: var(--sp-1);
  font-weight: 700;
}

.next {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  font-size: var(--fs-md);
}

.next > span {
  display: flex;
  flex-direction: column;
}

.warnings {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: var(--fs-sm);
}

.warnings li {
  display: flex;
  align-items: center;
  gap: var(--sp-1);
}

.warnings .danger {
  color: var(--danger);
}

.warnings .warn {
  color: var(--warn);
}

.offer {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.offer p {
  margin: 0;
  font-size: var(--fs-md);
}
</style>
