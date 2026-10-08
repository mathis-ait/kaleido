<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * ScreenScraper (scans des vraies étiquettes du mode Cartouche). Rien à faire quand
 * Kaleido est compilé avec ses identifiants développeur : le compte personnel est
 * seulement un plus (meilleur quota). Voir screenscraper.rs.
 */

interface Status {
  dev: boolean;
  builtIn: boolean;
  user: string | null;
}

const status = ref<Status | null>(null);
const devId = ref("");
const devPassword = ref("");
const user = ref("");
const password = ref("");
const busy = ref(false);
const message = ref<string | null>(null);

onMounted(async () => {
  status.value = await invoke<Status>("screenscraper_status").catch(() => null);
  user.value = status.value?.user ?? "";
});

async function save() {
  busy.value = true;
  message.value = null;
  try {
    status.value = await invoke<Status>("screenscraper_save", {
      devId: status.value?.builtIn ? null : devId.value,
      devPassword: status.value?.builtIn ? null : devPassword.value,
      user: user.value,
      password: password.value || (user.value ? null : ""),
    });
    password.value = "";
    devPassword.value = "";
    message.value = "Enregistré. Les étiquettes manquantes seront redemandées à la prochaine ouverture du lanceur.";
  } catch (e) {
    message.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="ss">
    <p class="lead">
      En mode Cartouche, le lanceur affiche l'étiquette réelle de chaque jeu, en français quand elle existe, grâce à la base de
      <button class="link" @click="openUrl('https://www.screenscraper.fr/')">ScreenScraper</button>. Sans scan, la cartouche garde
      une étiquette neutre avec le titre.
    </p>
    <div class="panel block">
      <p v-if="status?.builtIn" class="state ok">Activé : Kaleido est fourni avec son accès à ScreenScraper, rien à configurer.</p>
      <template v-else>
        <p class="state" :class="{ ok: status?.dev }">
          {{ status?.dev ? "Activé avec les identifiants développeur saisis ci-dessous." : "Cette version de Kaleido n'a pas d'accès développeur ScreenScraper." }}
        </p>
        <div class="row">
          <label>
            <span class="sv-label">Identifiant développeur</span>
            <input v-model="devId" class="sv-input" type="text" autocomplete="off" :placeholder="status?.dev ? 'enregistré' : ''" />
          </label>
          <label>
            <span class="sv-label">Mot de passe développeur</span>
            <input v-model="devPassword" class="sv-input" type="password" autocomplete="off" :placeholder="status?.dev ? 'enregistré' : ''" />
          </label>
        </div>
      </template>
      <p class="dim small">Facultatif : ton compte ScreenScraper donne un meilleur quota de requêtes.</p>
      <div class="row">
        <label>
          <span class="sv-label">Compte ScreenScraper</span>
          <input v-model="user" class="sv-input" type="text" autocomplete="username" />
        </label>
        <label>
          <span class="sv-label">Mot de passe</span>
          <input v-model="password" class="sv-input" type="password" autocomplete="current-password" :placeholder="status?.user ? 'enregistré' : ''" />
        </label>
        <button class="btn" :disabled="busy" @click="save">Enregistrer</button>
      </div>
      <p v-if="message" class="dim small">{{ message }}</p>
    </div>
  </div>
</template>

<style scoped>
.ss .block {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 18px 20px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 12px;
}

.row label {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 220px;
}

.state {
  margin: 0;
  font-weight: 600;
}

.state.ok {
  color: var(--ok);
}

.link {
  padding: 0;
  border: none;
  background: none;
  color: var(--accent, inherit);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}
</style>
