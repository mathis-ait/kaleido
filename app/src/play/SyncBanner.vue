<script setup lang="ts">
import Icon from "../components/Icon.vue";
import { saveState } from "../saveStore";
import PlayTip from "./PlayTip.vue";
import { acceptGameSave, fileName, keepCopyThenReload, keepMine, live } from "./play";

/** Bandeau « Le jeu a sauvegardé — recharger ? » quand l'éditeur ne peut pas recharger seul. */
</script>

<template>
  <div v-if="live.pending && saveState.view" class="banner sync" role="alert">
    <Icon name="refresh" :size="16" />
    <span class="msg">
      <strong>Le jeu a sauvegardé — recharger ?</strong>
      <template v-if="saveState.dirty"> Tu as aussi des modifications non enregistrées dans Kaleido : choisis lesquelles garder.</template>
      <template v-else-if="saveState.path !== live.pending"> La sauvegarde du jeu est {{ fileName(live.pending) }}.</template>
      <PlayTip term="liveSync" />
    </span>
    <span class="actions">
      <button class="sv-btn solid" @click="acceptGameSave">{{ saveState.dirty ? "Prendre la version du jeu" : "Recharger" }}</button>
      <button v-if="saveState.dirty" class="sv-btn" @click="keepCopyThenReload">Copier mes modifications puis recharger</button>
      <button class="sv-btn" @click="keepMine">{{ saveState.dirty ? "Garder mes modifications" : "Ignorer" }}</button>
    </span>
  </div>
</template>

<style scoped>
.sync {
  flex-wrap: wrap;
  border: 1px solid var(--accent-2);
  background: color-mix(in srgb, var(--accent-2) 12%, transparent);
  color: var(--text);
}

.msg {
  flex: 1;
  min-width: 240px;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
</style>
