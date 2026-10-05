import { createApp } from "vue";
import App from "./App.vue";
import { initTheme } from "./theme";
import "./styles/main.css";

import { editor, openRom } from "./editor";
import { addPaths, library } from "./library";
import { nav } from "./nav";
import { openSave, saveState } from "./saveStore";
import { currentTheme } from "./theme";

initTheme();
createApp(App).mount("#app");

// Accès à l'état depuis les outils de test automatisés (mode développement uniquement).
if (import.meta.env.DEV) {
  Object.assign(window, { __kaleido: { nav, library, editor, addPaths, openRom, currentTheme, saveState, openSave } });
}
