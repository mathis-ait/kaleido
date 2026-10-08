import { computed, reactive, ref, watch } from "vue";

/**
 * Présentation du lanceur : jaquettes (par défaut) ou cartouches en 3D.
 * Le mode Cartouche ne s'active que si la machine a un vrai WebGL 2 (pas le rendu
 * logiciel SwiftShader) et si « Réduire les animations » n'est pas activé dans Windows.
 */

export type Presentation = "covers" | "cartridges";

const KEY = "kaleido.launcher.presentation";

function read(): Presentation {
  try {
    return localStorage.getItem(KEY) === "cartridges" ? "cartridges" : "covers";
  } catch {
    return "covers";
  }
}

export const presentation = reactive({ wanted: read() });

watch(
  () => presentation.wanted,
  (v) => {
    try {
      localStorage.setItem(KEY, v);
    } catch {
      /* stockage indisponible */
    }
  },
);

let webgl: string | null | undefined;

/** Raison pour laquelle WebGL ne convient pas, ou null. Testé une seule fois. */
function webglProblem(): string | null {
  if (webgl !== undefined) return webgl;
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2", { failIfMajorPerformanceCaveat: true });
    if (!gl) {
      webgl = "WebGL 2 indisponible sur cette machine";
    } else {
      const info = gl.getExtension("WEBGL_debug_renderer_info");
      const renderer = info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : "";
      webgl = /swiftshader|llvmpipe|software|basic render/i.test(renderer) ? "rendu graphique logiciel seulement (pilote à mettre à jour)" : null;
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    }
  } catch {
    webgl = "WebGL 2 indisponible sur cette machine";
  }
  return webgl;
}

const reducedQuery = window.matchMedia?.("(prefers-reduced-motion: reduce)");
const reducedMotion = ref(!!reducedQuery?.matches);
reducedQuery?.addEventListener?.("change", (e) => (reducedMotion.value = e.matches));

/** Pourquoi le mode Cartouche reste en Jaquettes, ou null s'il est possible. */
export const cartridgeBlocker = computed(() => {
  if (reducedMotion.value) return "« Réduire les animations » est activé dans Windows";
  return webglProblem();
});

export const cartridgeMode = computed(() => presentation.wanted === "cartridges" && !cartridgeBlocker.value);

/**
 * Échelle du lanceur selon la taille de la fenêtre : `ui` pour la barre du haut et la fiche
 * du jeu, `stage` pour le carrousel (jaquettes et cartouches 3D, qui lisent la même valeur).
 */
export const launcherScale = reactive({ ui: 1, stage: 1 });

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

export function measureLauncher(width: number, height: number) {
  launcherScale.ui = clamp(Math.min(width / 1350, height / 900), 0.9, 1.5);
  launcherScale.stage = clamp(height / 1000, 0.75, 1.45);
}
