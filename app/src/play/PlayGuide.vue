<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import Icon from "../components/Icon.vue";
import { nav } from "../nav";
import { closeGuide, defaultEmulator, emus, guide } from "./play";

/** Guide de première utilisation du bouton « Jouer ». */
const hasNds = () => !!defaultEmulator("nds");
const hasCtr = () => !!defaultEmulator("3ds");

function configure() {
  closeGuide(false);
  nav.view = "settings";
}

function onKey(e: KeyboardEvent) {
  if (guide.open && e.key === "Escape") closeGuide(false);
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <Teleport to="body">
    <div v-if="guide.open" class="sv-overlay" @click.self="closeGuide(false)">
      <div class="sv-dialog play-guide" role="dialog" aria-modal="true" aria-labelledby="play-guide-title">
        <header>
          <h2 id="play-guide-title"><Icon name="play" :size="20" /> Jouer depuis Kaleido</h2>
        </header>

        <div class="body">
          <ol>
            <li>
              <strong>Un émulateur fait tourner le jeu.</strong>
              C'est un programme gratuit qui imite une console sur ton ordinateur : <em>melonDS</em> ou <em>DeSmuME</em> pour la DS,
              <em>Azahar</em> (successeur de Citra) pour la 3DS. Kaleido ne les fournit pas ; installe-les depuis leur site officiel.
            </li>
            <li>
              <strong>Kaleido les retrouve tout seul.</strong>
              Il regarde dans Program Files et dans ton dossier utilisateur. S'il ne trouve rien, indique-lui l'exécutable dans
              <em>Paramètres → Émulateurs</em>.
            </li>
            <li>
              <strong>Où Kaleido met les fichiers :</strong>
              <ul>
                <li>DS : la sauvegarde est placée là où l'émulateur la cherche (melonDS : <code>&lt;nom de la ROM&gt;.sav</code> à côté de la ROM ; DeSmuME : <code>Battery\&lt;nom de la ROM&gt;.dsv</code>).</li>
                <li>3DS : le mod est copié dans <code>load\mods\&lt;title ID&gt;\romfs</code> de l'émulateur, ton jeu d'origine n'est pas modifié.</li>
                <li>Avant de remplacer quoi que ce soit, Kaleido fait une copie de sécurité (<code>.kaleido-&lt;date&gt;.bak</code>, ou <code>load\kaleido-backups</code> pour un ancien mod).</li>
              </ul>
            </li>
            <li>
              <strong>La sauvegarde reste synchronisée.</strong>
              Quand tu sauvegardes en jeu, l'éditeur de Kaleido se met à jour. Pour renvoyer tes modifications au jeu, utilise
              « Envoyer au jeu » après avoir fermé le jeu.
            </li>
          </ol>

          <p v-if="emus.loaded && !hasNds() && !hasCtr()" class="warn"><Icon name="alert" :size="16" /> Aucun émulateur trouvé pour l'instant.</p>
        </div>

        <footer>
          <button class="btn" @click="configure">Configurer les émulateurs</button>
          <button v-if="guide.next" class="btn btn-primary" @click="closeGuide(true)">J'ai compris, on joue !</button>
          <button v-else class="btn btn-primary" @click="closeGuide(false)">J'ai compris</button>
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
/* Voile et fenêtre partagés : `.sv-overlay` et `.sv-dialog` (save/form.css). */
.play-guide {
  --dialog-width: 640px;
}

h2 {
  gap: 10px;
  margin: 0;
}

ol {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin: 0;
  padding-left: 20px;
  line-height: 1.5;
}

ol strong {
  display: block;
}

ul {
  margin: 6px 0 0;
  padding-left: 18px;
  color: var(--text-dim);
}

code {
  font-size: 12px;
}

.warn {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 14px 0 0;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  background: var(--warn-bg);
  color: var(--warn);
}

footer {
  justify-content: flex-end;
}
</style>
