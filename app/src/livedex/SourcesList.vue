<script setup lang="ts">
import { computed } from "vue";
import Icon from "../components/Icon.vue";
import Toggle from "../components/Toggle.vue";
import { livedex, setBank, toggleSource } from "./store";

/** Sauvegardes lues par Kaleido, chacune activable ; la banque ; les fichiers illisibles. */
const saves = computed(() => livedex.sources.filter((s) => s.path !== "bank"));
const bankSource = computed(() => livedex.sources.find((s) => s.path === "bank"));

const enabled = (path: string) => !livedex.saved.disabledSources.includes(path);
const shortPath = (p: string) => p.replace(/^.*?([^\\/]+[\\/][^\\/]+)$/, "…/$1");
const plural = (n: number, one: string, many: string) => `${n.toLocaleString("fr-FR")} ${n > 1 ? many : one}`;
</script>

<template>
  <div class="sources">
    <p v-if="!saves.length && !livedex.scanning" class="none">
      Aucune sauvegarde trouvée pour l'instant. Ajoute tes sauvegardes dans <strong>Fichiers</strong> ou joue depuis la <strong>Bibliothèque</strong> : Kaleido
      retrouve aussi celles d'Azahar, Citra, melonDS et DeSmuME.
    </p>
    <ul v-else class="list">
      <li v-for="s in saves" :key="s.path" :class="{ off: !enabled(s.path) }">
        <label class="row">
          <input type="checkbox" :checked="enabled(s.path)" @change="toggleSource(s.path, ($event.target as HTMLInputElement).checked)" />
          <span class="main">
            <strong>{{ s.game }}</strong>
            <span class="dim">{{ s.trainer || "Dresseur sans nom" }} · {{ plural(s.specimens.length, "Pokémon", "Pokémon") }}</span>
          </span>
          <span class="path" :title="s.path">{{ shortPath(s.path) }}</span>
        </label>
      </li>
    </ul>

    <div class="bank">
      <Toggle :model-value="livedex.saved.bank" label="Compter la banque Kaleido" hint="Les Pokémon rangés dans la banque de Kaleido (PC partagé par toutes les sauvegardes) remplissent aussi la Living Dex." @update:model-value="setBank" />
      <span v-if="bankSource && livedex.saved.bank" class="dim">{{ plural(bankSource.specimens.length, "Pokémon", "Pokémon") }}</span>
    </div>

    <details v-if="livedex.unreadable.length" class="unreadable">
      <summary><Icon name="shield-alert" :size="14" /> {{ plural(livedex.unreadable.length, "fichier illisible", "fichiers illisibles") }}</summary>
      <ul>
        <li v-for="u in livedex.unreadable" :key="u.path" :title="u.path">{{ shortPath(u.path) }} — {{ u.error }}</li>
      </ul>
    </details>
  </div>
</template>

<style scoped>
.sources {
  display: grid;
  gap: var(--sp-3);
}

.none,
.dim {
  color: var(--text-dim);
}

.list {
  display: grid;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.row {
  display: flex;
  align-items: center;
  gap: var(--sp-3);
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background-color 0.12s;
}

.row:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.off .row {
  opacity: 0.55;
}

.main {
  display: grid;
  flex: 1;
  min-width: 0;
}

.main .dim {
  font-size: var(--fs-sm);
}

.path {
  max-width: 40%;
  overflow: hidden;
  color: var(--text-dim);
  font-size: var(--fs-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bank {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
}

.unreadable {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.unreadable summary {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.unreadable ul {
  margin: var(--sp-2) 0 0;
  padding-left: var(--sp-4);
}
</style>
