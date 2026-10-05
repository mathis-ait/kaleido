import { reactive } from "vue";
import type { ViewId } from "./types";

/** Vue affichée, accessible depuis n'importe quel composant. */
export const nav = reactive({ view: "home" as ViewId });
