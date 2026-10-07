<script setup lang="ts">
import { computed } from "vue";
import Icon from "../components/Icon.vue";
import { saveState } from "../saveStore";
import { fileName, live, sendTarget, sendToGame } from "./play";

/** Indicateur de synchro en direct (barre du haut de l'éditeur) + « Envoyer au jeu ». */
withDefaults(defineProps<{ compact?: boolean; send?: boolean }>(), { compact: false, send: true });

const time = computed(() => (live.at ? new Date(live.at).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }) : ""));

const label = computed(() => {
  switch (live.status) {
    case "watching":
      return "Synchro active";
    case "changed":
      return live.pending ? "Le jeu a sauvegardé" : `Le jeu a sauvegardé (${time.value})`;
    case "reloaded":
      return `Rechargé à ${time.value}`;
    default:
      return "";
  }
});

const title = computed(() => {
  const file = live.path ? fileName(live.path) : "";
  const game = live.session ? ` (${live.session.emulatorName})` : "";
  return `Kaleido surveille ${file}${game} : quand le jeu sauvegarde, l'éditeur se met à jour.`;
});
</script>

<template>
  <span class="live-wrap">
    <span v-if="live.status !== 'idle'" class="live" :class="[live.status, { compact }]" :title="title" role="status">
      <span class="dot" />
      <span v-if="!compact" class="text">{{ label }}</span>
    </span>
    <button
      v-if="send && saveState.view && sendTarget"
      class="send"
      :title="`Envoyer au jeu : écrire la sauvegarde modifiée dans ${fileName(sendTarget)} (ferme le jeu avant)`"
      aria-label="Envoyer au jeu"
      @click="sendToGame"
    >
      <Icon name="send" :size="16" />
    </button>
  </span>
</template>

<style scoped>
.live-wrap {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.send {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--panel);
  color: inherit;
  cursor: pointer;
}

.send:hover {
  border-color: var(--accent-2);
}

.live {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}

.live.compact {
  padding: 4px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ok);
}

.changed {
  border-color: var(--warn);
  color: var(--warn);
}

.changed .dot {
  background: var(--warn);
}

.reloaded .dot {
  background: var(--accent-2);
}
</style>
