import { createApp } from "vue";
import App from "./App.vue";
import { initGamepadNav } from "./gamepadNav";
import { initTheme } from "./theme";
import { initUpdates, updates } from "./updates";
import "./styles/main.css";
import "./save/form.css";

import { editor, openRom } from "./editor";
import { addPaths, library } from "./library";
import { nav } from "./nav";
import * as saveStore from "./saveStore";
import { openSave, saveState } from "./saveStore";
import { currentTheme } from "./theme";

initTheme();

// Fenêtre du compagnon de partie (ouverte par le moteur avec « ?companion ») : vue seule, sans le reste de l'app.
const isCompanion = new URLSearchParams(location.search).has("companion");
if (isCompanion) {
  void import("./companion/Companion.vue").then((m) => createApp(m.default).mount("#app"));
} else {
  createApp(App).mount("#app");
}

// Jeux et émulateurs du PC retrouvés tout seuls, une fois l'interface affichée.
if (!isCompanion) {
  setTimeout(() => void import("./games").then((m) => m.autoDiscover()), 800);
  initUpdates();
  initGamepadNav();
  void import("./companion/bridge").then((m) => m.initCompanionBridge());
}

// Accès à l'état depuis les outils de test automatisés (mode développement uniquement).
if (import.meta.env.DEV) {
  Object.assign(window, { __kaleido: { nav, library, editor, addPaths, openRom, currentTheme, saveState, openSave, saveStore, updates } });
}
