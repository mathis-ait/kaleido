<script setup lang="ts">
import { computed } from "vue";
import { CONSOLES, type ConsoleId } from "../consoles";

/**
 * Logo officiel d'une console, dans la couleur du texte (`currentColor`) : le SVG sert de
 * masque, ce qui le rend lisible sur fond sombre comme clair, et sur une pastille inversée.
 * `size` : hauteur de référence en pixels, corrigée par l'échelle optique du logo.
 */
const props = withDefaults(defineProps<{ id: ConsoleId; size?: number }>(), { size: 16 });

const info = computed(() => CONSOLES[props.id]);
const style = computed(() => {
  const h = props.size * info.value.scale;
  return {
    height: `${h}px`,
    width: `${h * info.value.ratio}px`,
    "--logo": `url("${info.value.logo}")`,
  };
});
</script>

<template>
  <span class="console-logo" role="img" :aria-label="info.label" :style="style" />
</template>

<style scoped>
.console-logo {
  display: inline-block;
  flex: none;
  background: currentColor;
  -webkit-mask: var(--logo) center / contain no-repeat;
  mask: var(--logo) center / contain no-repeat;
}
</style>
