<script setup lang="ts">
import { glyphs, layoutOf, type HintAction } from "./scene/hints";

/**
 * Aides manette en bas d'écran, partagées par les modes Jaquettes et Cartouches :
 * glyphe du bouton (rond avec la lettre, pilule pour les gâchettes, touche au clavier)
 * et libellé. À la souris, chaque aide reste cliquable.
 */
const props = defineProps<{
  items: { action: HintAction; label: string; run?: () => void }[];
  pad: boolean;
  padId?: string | null;
}>();

const layout = () => layoutOf(props.padId);
</script>

<template>
  <div class="pad-hints">
    <component
      :is="item.run ? 'button' : 'span'"
      v-for="item in items"
      :key="item.action + item.label"
      class="hint"
      :type="item.run ? 'button' : undefined"
      @click="item.run?.()"
    >
      <span class="glyphs">
        <span v-for="g in glyphs(item.action, pad, layout())" :key="g.text" class="glyph" :class="g.shape">{{ g.text }}</span>
      </span>
      <span>{{ item.label }}</span>
    </component>
    <slot />
  </div>
</template>

<style scoped>
.pad-hints {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 8px 22px;
}

.hint {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
}

button.hint {
  cursor: pointer;
}

button.hint:hover {
  color: #fff;
}

.glyphs {
  display: inline-flex;
  gap: 4px;
}

.glyph {
  display: inline-grid;
  place-items: center;
  height: 22px;
  font-size: 11px;
  font-weight: 800;
  line-height: 1;
}

.round {
  width: 22px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.92);
  color: #0b0d18;
}

.pill {
  min-width: 26px;
  padding: 0 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.92);
  color: #0b0d18;
}

.key {
  min-width: 24px;
  padding: 0 7px;
  border: 1px solid rgba(255, 255, 255, 0.35);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.85);
}
</style>
