import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Configuration recommandée par Tauri : port fixe, pas d'effacement de la console.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
