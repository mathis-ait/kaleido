<script setup lang="ts">
import { computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import Dialog from "./Dialog.vue";
import { installUpdate, updates } from "../updates";

/** Fenêtre proposée au lancement quand une nouvelle version est publiée. */

/** Notes de la release (Markdown simple : titres, listes, paragraphes), sans HTML injecté. */
const blocks = computed(() => {
  const out: { kind: "h" | "li" | "p"; text: string }[] = [];
  const clean = (t: string) => t.replace(/\*\*(.+?)\*\*/g, "$1").replace(/`(.+?)`/g, "$1");
  for (const raw of (updates.info?.notes ?? "").split(/\r?\n/)) {
    const line = raw.trim();
    // Le titre de la release est déjà affiché en sous-titre.
    if (!line || /^#\s/.test(line)) continue;
    if (/^#{2,}\s/.test(line)) out.push({ kind: "h", text: clean(line.replace(/^#+\s*/, "")) });
    else if (/^[-*]\s/.test(line)) out.push({ kind: "li", text: clean(line.slice(2)) });
    else out.push({ kind: "p", text: clean(line) });
  }
  return out;
});

const percent = computed(() => {
  const p = updates.progress;
  return p && p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : null;
});

const mb = (n: number) => (n / 1048576).toFixed(1).replace(".", ",");

const status = computed(() => {
  if (!updates.installing) return null;
  const p = updates.progress;
  if (p && p.total > 0 && p.done >= p.total) return "Lancement de l'installation… Kaleido va se fermer puis se rouvrir.";
  if (p && p.total > 0) return `Téléchargement… ${mb(p.done)} / ${mb(p.total)} Mo`;
  return "Préparation du téléchargement…";
});

// Pendant le téléchargement, la fenêtre reste ouverte (Échap et clic à côté ignorés).
const open = computed({
  get: () => updates.prompt,
  set: (v: boolean) => {
    if (v || !updates.installing) updates.prompt = v;
  },
});
</script>

<template>
  <Dialog
    v-if="updates.info?.newer"
    v-model="open"
    :title="`Kaleido ${updates.info.latest} est disponible`"
    :subtitle="updates.info.name ?? `Tu as la version ${updates.info.current}`"
    :width="560"
  >
    <div class="notes">
      <template v-for="(b, i) in blocks" :key="i">
        <h3 v-if="b.kind === 'h'">{{ b.text }}</h3>
        <p v-else-if="b.kind === 'p'">{{ b.text }}</p>
        <p v-else class="li">{{ b.text }}</p>
      </template>
      <p v-if="!blocks.length">Pas de notes pour cette version.</p>
    </div>

    <div v-if="updates.installing" class="progress" role="progressbar" :aria-valuenow="percent ?? undefined" aria-valuemin="0" aria-valuemax="100">
      <div class="bar" :class="{ indeterminate: percent === null }" :style="percent !== null ? { width: `${percent}%` } : undefined" />
    </div>
    <p v-if="status" class="status">{{ status }}</p>
    <p v-if="updates.installError" class="error">{{ updates.installError }}</p>

    <template #foot>
      <button v-if="!updates.info.installable" type="button" class="sv-btn" @click="openUrl(updates.info.url)">Ouvrir la page de téléchargement</button>
      <span class="grow" />
      <button type="button" class="sv-btn" :disabled="updates.installing" @click="open = false">Plus tard</button>
      <button v-if="updates.info.installable" type="button" class="sv-btn solid" :disabled="updates.installing" autofocus @click="installUpdate">
        {{ updates.installing ? "Mise à jour…" : "Mettre à jour" }}
      </button>
    </template>
  </Dialog>
</template>

<style scoped>
.notes {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 340px;
  overflow-y: auto;
  font-size: 14px;
  line-height: 1.5;
}

.notes h3 {
  margin: 10px 0 2px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.notes h3:first-child {
  margin-top: 0;
}

.notes p {
  margin: 0;
  color: var(--text-dim);
}

.notes .li {
  position: relative;
  padding-left: 16px;
}

.notes .li::before {
  content: "";
  position: absolute;
  left: 4px;
  top: 0.62em;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: currentColor;
}

.progress {
  margin-top: 16px;
  height: 6px;
  border-radius: 3px;
  background: var(--border);
  overflow: hidden;
}

.bar {
  height: 100%;
  background: var(--accent);
  transition: width 0.2s;
}

.bar.indeterminate {
  width: 30%;
  animation: slide 1.1s ease-in-out infinite;
}

@keyframes slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(340%);
  }
}

.status {
  margin: 8px 0 0;
  font-size: 13px;
  color: var(--text-dim);
}

.error {
  margin: 8px 0 0;
  font-size: 13px;
  color: var(--danger);
}

.grow {
  flex: 1;
}
</style>
