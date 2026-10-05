<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { spritePrefs, styleInfo, type SpriteStyle } from "../spriteStyle";

/**
 * Image d'un Pokémon, téléchargée puis mise en cache par le moteur.
 * - `variant="icon"` (défaut) : icône pokesprite, sauf si l'utilisateur a choisi « Partout ».
 * - `variant="model"` : style choisi dans les réglages (3D animés, pixel animés, artwork HOME).
 * Un modèle absent ou hors ligne retombe sur l'icône, puis sur une silhouette.
 */
const props = withDefaults(
  defineProps<{
    id: number;
    shiny?: boolean;
    size?: number;
    /** Forme dans l'ordre du jeu (0 = forme de base). */
    form?: number;
    gender?: "male" | "female" | "genderless";
    variant?: "icon" | "model";
    /** Impose un style (aperçus des réglages). */
    look?: SpriteStyle;
    /** Impose une image fixe. */
    still?: boolean;
  }>(),
  { shiny: false, size: 68, form: 0, gender: undefined, variant: "icon", look: undefined, still: false },
);

const style = computed(() => styleInfo(props.look ?? (props.variant === "model" || spritePrefs.everywhere ? spritePrefs.style : "icons")));
const modelSrc = computed(() => {
  const dir = props.still || spritePrefs.reduceMotion ? style.value.still : style.value.dir;
  if (!dir) return null;
  const ext = dir.endsWith("ani") ? "gif" : "png";
  const name = `${props.id}${props.form > 0 ? `-${props.form}` : ""}${props.gender === "female" ? "-f" : ""}${props.shiny ? "-shiny" : ""}`;
  return convertFileSrc(`${dir}/${name}.${ext}`, "sprite");
});
const iconSrc = computed(() => convertFileSrc(`${props.id}${props.shiny ? "-shiny" : ""}.png`, "sprite"));

/** `model` → `icon` → `failed` au fil des échecs. */
const stage = ref<"model" | "icon" | "failed">("icon");
const attempt = ref(0);
const loaded = ref(false);
const reset = () => {
  stage.value = modelSrc.value ? "model" : "icon";
  attempt.value = 0;
  loaded.value = false;
};
watch([modelSrc, iconSrc], reset, { immediate: true });

// Le paramètre `r` force un nouvel essai de l'icône après un échec réseau passager.
const src = computed(() => {
  if (stage.value === "model") return modelSrc.value ?? "";
  return attempt.value ? `${iconSrc.value}?r=${attempt.value}` : iconSrc.value;
});
const pixel = computed(() => stage.value === "icon" || style.value.pixel);
const square = computed(() => props.variant === "model" || !!props.look);

function onError() {
  loaded.value = false;
  if (stage.value === "model") {
    stage.value = "icon";
  } else if (attempt.value < 2) {
    setTimeout(() => attempt.value++, 1500 * (attempt.value + 1));
  } else {
    stage.value = "failed";
  }
}
</script>

<template>
  <span class="sprite" :style="{ width: `${size}px`, height: `${square ? size : Math.round((size * 56) / 68)}px` }">
    <img
      v-if="stage !== 'failed'"
      :key="src"
      :src="src"
      alt=""
      loading="lazy"
      draggable="false"
      :class="{ pixel, model: stage === 'model', loaded }"
      @load="loaded = true"
      @error="onError"
    />
    <svg v-else viewBox="0 0 24 24" class="fallback" aria-hidden="true">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="1.5" />
      <path d="M3 12h6a3 3 0 0 0 6 0h6" fill="none" stroke="currentColor" stroke-width="1.5" />
    </svg>
  </span>
</template>

<style scoped>
.sprite {
  display: inline-grid;
  place-items: center;
  flex-shrink: 0;
}

img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  image-rendering: auto;
}

/* Pixel art (icônes, style Noir et Blanc) : agrandi sans flou. */
img.pixel {
  image-rendering: pixelated;
}

/* Les modèles apparaissent en fondu une fois chargés. */
img.model {
  opacity: 0;
  transition: opacity 0.2s;
}

img.model.loaded {
  opacity: 1;
}

.fallback {
  width: 45%;
  height: 45%;
  color: var(--text-dim);
  opacity: 0.4;
}
</style>
