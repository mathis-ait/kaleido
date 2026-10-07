<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";
import Tip from "./Tip.vue";

/**
 * Fenêtre par-dessus la page : fond flouté, Échap ou clic à côté pour fermer, focus gardé
 * dans la fenêtre (Tab boucle) puis rendu à l'élément d'origine à la fermeture.
 * Slots : `head` (à droite du titre), défaut (corps défilant), `foot` (barre du bas).
 */
const props = defineProps<{ title: string; term?: string; subtitle?: string; icon?: string; width?: number }>();
const open = defineModel<boolean>({ required: true });

const box = ref<HTMLElement | null>(null);
let before: HTMLElement | null = null;

const FOCUSABLE = "button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

watch(
  open,
  async (v) => {
    if (v) {
      before = document.activeElement as HTMLElement | null;
      await nextTick();
      // Premier champ s'il y en a un, sinon la fenêtre elle-même.
      const first = box.value?.querySelector<HTMLElement>("[autofocus], .body input, .body textarea") ?? box.value;
      first?.focus();
    } else {
      before?.focus?.();
      before = null;
    }
  },
  { immediate: true },
);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    e.preventDefault();
    open.value = false;
  } else if (e.key === "Tab" && box.value) {
    const items = [...box.value.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
}

onBeforeUnmount(() => before?.focus?.());
</script>

<template>
  <Teleport to="body">
    <Transition name="sv-fade">
      <div v-if="open" class="sv-overlay" @pointerdown.self="open = false">
        <section
          ref="box"
          class="sv-dialog"
          role="dialog"
          aria-modal="true"
          :aria-label="props.title"
          tabindex="-1"
          :style="props.width ? { '--dialog-width': `${props.width}px` } : undefined"
          @keydown="onKey"
        >
          <header>
            <Icon v-if="props.icon" :name="props.icon" :size="20" />
            <div class="titles">
              <h2>{{ props.title }}<Tip v-if="props.term" :term="props.term" /></h2>
              <p v-if="props.subtitle">{{ props.subtitle }}</p>
            </div>
            <slot name="head" />
            <button type="button" class="sv-round sq" title="Fermer (Échap)" aria-label="Fermer" @click="open = false">
              <Icon name="x" :size="16" />
            </button>
          </header>
          <div class="body"><slot /></div>
          <footer v-if="$slots.foot"><slot name="foot" /></footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.titles {
  flex: 1;
  min-width: 0;
}

.titles p {
  margin: 2px 0 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.sv-dialog:focus {
  outline: none;
}
</style>
