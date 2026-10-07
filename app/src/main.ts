import { createApp } from "vue";
import App from "./App.vue";
import { initTheme } from "./theme";
import { initUpdates } from "./updates";
import "./styles/main.css";

import { editor, openRom } from "./editor";
import { addPaths, library } from "./library";
import { nav } from "./nav";
import * as saveStore from "./saveStore";
import { openSave, saveState } from "./saveStore";
import { currentTheme } from "./theme";

initTheme();
createApp(App).mount("#app");

// Jeux et émulateurs du PC retrouvés tout seuls, une fois l'interface affichée.
setTimeout(() => void import("./games").then((m) => m.autoDiscover()), 800);
initUpdates();

// Accès à l'état depuis les outils de test automatisés (mode développement uniquement).
if (import.meta.env.DEV) {
  Object.assign(window, { __kaleido: { nav, library, editor, addPaths, openRom, currentTheme, saveState, openSave, saveStore } });
}
