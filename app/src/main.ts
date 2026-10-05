import { createApp } from "vue";
import App from "./App.vue";
import { initTheme } from "./theme";
import "./styles/main.css";

initTheme();
createApp(App).mount("#app");
