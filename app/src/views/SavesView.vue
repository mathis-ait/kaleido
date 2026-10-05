<script setup lang="ts">
import { computed, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import PokemonEditor from "../components/PokemonEditor.vue";
import Sprite from "../components/Sprite.vue";
import { library } from "../library";
import { deletePokemon, exportPokemon, importPokemon, loadBox, movePokemon, openSave, saveState, writeSave } from "../saveStore";
import type { Slot, SlotView } from "../types";

const STAT_LABELS = ["PV", "Att", "Déf", "Atq Spé", "Déf Spé", "Vit"];
const savesInLibrary = computed(() => library.items.filter((d) => d.kind === "save"));
const editing = ref<SlotView | null>(null);
const confirmDelete = ref(false);

const view = computed(() => saveState.view);
const boxCount = computed(() => view.value?.boxNames.length ?? 0);
const partySlots = computed(() => Array.from({ length: 6 }, (_, i) => view.value?.party[i] ?? null));

async function pickSave() {
  const path = await open({
    title: "Ouvrir une sauvegarde",
    filters: [
      { name: "Sauvegardes", extensions: ["sav", "dsv", "bin", "main"] },
      { name: "Tous les fichiers", extensions: ["*"] },
    ],
  });
  if (typeof path === "string") openSave(path);
}

function changeBox(step: number) {
  const n = boxCount.value;
  if (n) loadBox((saveState.box + step + n) % n);
}

// --- Glisser-déposer entre boîtes et équipe, avec la souris (le glisser-déposer
// HTML est capté par la fenêtre Tauri pour les fichiers, sous Windows).
const key = (s: Slot) => JSON.stringify(s);
const dropTarget = ref<string | null>(null);
const ghost = ref<{ species: number; shiny: boolean; x: number; y: number } | null>(null);
let pending: { slot: Slot; p: SlotView; x: number; y: number } | null = null;
let justDragged = false;

const slotUnder = (x: number, y: number) => document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-slot]")?.dataset.slot ?? null;

function onPointerDown(e: PointerEvent, slot: Slot, p: SlotView | null) {
  if (!p || e.button !== 0) return;
  pending = { slot, p, x: e.clientX, y: e.clientY };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp, { once: true });
}

function onPointerMove(e: PointerEvent) {
  if (!pending) return;
  if (!ghost.value && Math.hypot(e.clientX - pending.x, e.clientY - pending.y) < 6) return;
  ghost.value = { species: pending.p.species, shiny: pending.p.shiny, x: e.clientX, y: e.clientY };
  dropTarget.value = slotUnder(e.clientX, e.clientY);
}

function onPointerUp(e: PointerEvent) {
  window.removeEventListener("pointermove", onPointerMove);
  if (pending && ghost.value) {
    const target = slotUnder(e.clientX, e.clientY);
    if (target) movePokemon(pending.slot, JSON.parse(target) as Slot);
    justDragged = true;
    setTimeout(() => (justDragged = false), 0);
  }
  pending = null;
  ghost.value = null;
  dropTarget.value = null;
}

function onSlotClick(slot: Slot, p: SlotView | null) {
  if (justDragged) return;
  if (p) saveState.selected = p;
  else if (slot.kind === "box") importInto(slot);
}

const boxSlot = (index: number): Slot => ({ kind: "box", box: saveState.box, index });
const partySlot = (index: number): Slot => ({ kind: "party", index });

async function importInto(slot: Slot) {
  const file = await open({ title: "Importer un Pokémon", filters: [{ name: "Pokémon", extensions: ["pk4", "pk5", "pk6", "pk7"] }] });
  if (typeof file === "string") importPokemon(slot, file);
}

async function exportSelected() {
  const p = saveState.selected;
  if (!p || !view.value) return;
  const output = await save({
    title: "Exporter le Pokémon",
    defaultPath: `${p.speciesName}.pk${view.value.generation}`,
    filters: [{ name: "Pokémon", extensions: [`pk${view.value.generation}`] }],
  });
  if (output) exportPokemon(p.slot, output);
}

async function saveAs() {
  const output = await save({ title: "Enregistrer la sauvegarde sous", defaultPath: saveState.path ?? undefined });
  if (output) writeSave(output);
}

async function doDelete() {
  if (saveState.selected) await deletePokemon(saveState.selected.slot);
  confirmDelete.value = false;
}

const genderSymbol = (g: string) => (g === "male" ? "♂" : g === "female" ? "♀" : "");
const evTotal = (p: SlotView) => p.evs.reduce((a, b) => a + b, 0);
</script>

<template>
  <section class="saves">
    <!-- Aucune sauvegarde ouverte -->
    <template v-if="!view">
      <h1>Sauvegardes</h1>
      <p class="lead">Ouvre ta sauvegarde Gen 4 à 7 : boîtes, équipe et fiches de tes Pokémon.</p>
      <div v-if="saveState.error" class="banner danger">{{ saveState.error }}</div>
      <div class="picker">
        <button v-for="s in savesInLibrary" :key="s.path" class="pick panel" @click="openSave(s.path)">
          <span class="chip chip-accent">Gen {{ s.generation }}</span>
          <strong>{{ s.title.replace("Sauvegarde · ", "") }}</strong>
          <small>{{ s.fileName }}</small>
        </button>
        <button class="pick panel add" @click="pickSave">
          <strong>＋ Ouvrir une sauvegarde…</strong>
          <small>.sav, .dsv, main (3DS)…</small>
        </button>
      </div>
    </template>

    <template v-else>
      <header class="head">
        <div>
          <div class="chips">
            <span class="chip chip-accent">Gen {{ view.generation }}</span>
            <span class="chip">{{ view.game }}</span>
            <span class="chip">ID {{ view.trainer.displayId }}</span>
            <span class="chip">{{ view.trainer.playTime.hours }} h {{ String(view.trainer.playTime.minutes).padStart(2, "0") }}</span>
          </div>
          <h1>{{ view.trainer.name || "Dresseur" }}</h1>
        </div>
        <div class="head-actions">
          <button class="btn" @click="saveState.view = null">Fermer</button>
          <button class="btn" @click="saveAs">Enregistrer sous…</button>
          <button class="btn btn-primary" :disabled="!saveState.dirty" @click="writeSave()">
            {{ saveState.dirty ? "Enregistrer" : "Enregistré" }}
          </button>
        </div>
      </header>

      <div class="banner warn">
        Éditeur expérimental : les formats de sauvegarde n'ont pas encore été vérifiés sur de vraies parties. Kaleido garde
        une copie de sécurité de l'original (<code>.kaleido.bak</code>) avant d'écrire.
      </div>
      <div v-if="view.needsResign" class="banner danger">
        Soleil / Lune : après modification, la sauvegarde doit être re-signée (par ex. avec PKHeX) pour être acceptée par le jeu.
      </div>
      <div v-for="w in view.warnings" :key="w" class="banner warn">{{ w }}</div>
      <div v-if="saveState.error" class="banner danger">{{ saveState.error }}</div>
      <div v-if="saveState.notice" class="banner ok">{{ saveState.notice }}</div>

      <div class="layout">
        <!-- Boîte -->
        <div class="box panel">
          <div class="box-head">
            <button class="round" aria-label="Boîte précédente" @click="changeBox(-1)">‹</button>
            <h2>{{ view.boxNames[saveState.box] }}</h2>
            <button class="round" aria-label="Boîte suivante" @click="changeBox(1)">›</button>
            <div class="dots">
              <button
                v-for="(_, i) in view.boxNames"
                :key="i"
                :class="{ on: i === saveState.box }"
                :aria-label="view.boxNames[i]"
                @click="loadBox(i)"
              />
            </div>
          </div>
          <div class="grid">
            <button
              v-for="(p, i) in saveState.slots"
              :key="i"
              class="slot"
              :class="{ selected: p && saveState.selected && key(saveState.selected.slot) === key(boxSlot(i)), over: dropTarget === key(boxSlot(i)) }"
              :data-slot="key(boxSlot(i))"
              :title="p ? `${p.nickname || p.speciesName} · N. ${p.level}` : 'Case vide : clic pour importer un .pk'"
              @pointerdown="onPointerDown($event, boxSlot(i), p)"
              @click="onSlotClick(boxSlot(i), p)"
              @dblclick="p && (editing = p)"
            >
              <template v-if="p">
                <span v-if="p.shiny" class="shiny" title="Chromatique">★</span>
                <Sprite :id="p.species" :shiny="p.shiny" :size="68" />
                <span class="lvl">N. {{ p.level }}</span>
              </template>
            </button>
          </div>
        </div>

        <!-- Équipe et fiche -->
        <aside class="side">
          <div class="panel party">
            <h3>Équipe</h3>
            <div class="party-grid">
              <button
                v-for="(p, i) in partySlots"
                :key="i"
                class="slot"
                :class="{ selected: p && saveState.selected && key(saveState.selected.slot) === key(partySlot(i)), over: dropTarget === key(partySlot(i)) }"
                :data-slot="key(partySlot(i))"
                @pointerdown="onPointerDown($event, partySlot(i), p)"
                @click="onSlotClick(partySlot(i), p)"
                @dblclick="p && (editing = p)"
              >
                <template v-if="p">
                  <span v-if="p.shiny" class="shiny">★</span>
                  <Sprite :id="p.species" :shiny="p.shiny" :size="64" />
                  <span class="lvl">N. {{ p.level }}</span>
                </template>
              </button>
            </div>
          </div>

          <div v-if="saveState.selected" class="panel detail">
            <div class="detail-head">
              <Sprite :id="saveState.selected.species" :shiny="saveState.selected.shiny" :size="96" />
              <div>
                <h3>
                  {{ saveState.selected.nickname || saveState.selected.speciesName }}
                  <span class="gender">{{ genderSymbol(saveState.selected.gender) }}</span>
                </h3>
                <small class="dim">
                  {{ saveState.selected.speciesName }} · N. {{ saveState.selected.level
                  }}<span v-if="saveState.selected.levelEstimated" title="Niveau estimé">~</span>
                  <span v-if="saveState.selected.shiny"> · ★ Chromatique</span>
                </small>
              </div>
            </div>
            <dl>
              <dt>Nature</dt>
              <dd>{{ saveState.selected.natureName }}</dd>
              <dt>Talent</dt>
              <dd>{{ saveState.selected.abilityName }}</dd>
              <dt>Objet</dt>
              <dd>{{ saveState.selected.itemName ?? "—" }}</dd>
              <dt>Dresseur</dt>
              <dd>{{ saveState.selected.otName }} ({{ saveState.selected.tid }})</dd>
            </dl>
            <ul class="moves">
              <li v-for="m in saveState.selected.moveNames" :key="m">{{ m }}</li>
            </ul>
            <table v-if="saveState.selected.stats" class="stats">
              <thead>
                <tr><th></th><th>Stat</th><th>IV</th><th>EV</th></tr>
              </thead>
              <tbody>
                <tr v-for="(label, i) in STAT_LABELS" :key="label">
                  <td>{{ label }}</td>
                  <td class="num">{{ saveState.selected.stats[i] }}</td>
                  <td class="num" :class="{ max: saveState.selected.ivs[i] === 31 }">{{ saveState.selected.ivs[i] }}</td>
                  <td class="num">{{ saveState.selected.evs[i] }}</td>
                </tr>
              </tbody>
            </table>
            <small class="dim">EV : {{ evTotal(saveState.selected) }} / 510</small>
            <div class="detail-actions">
              <button class="btn btn-primary" @click="editing = saveState.selected">Modifier</button>
              <button class="btn" @click="exportSelected">Exporter</button>
              <button v-if="!confirmDelete" class="btn" @click="confirmDelete = true">Supprimer</button>
              <button v-else class="btn danger-btn" @click="doDelete">Confirmer la suppression</button>
            </div>
          </div>
          <p class="hint dim">Glisse un Pokémon pour le déplacer. Double-clic pour le modifier, clic sur une case vide pour importer un fichier .pk.</p>
        </aside>
      </div>
    </template>

    <PokemonEditor v-if="editing" :pokemon="editing" @close="editing = null" />

    <!-- Sprite qui suit la souris pendant un déplacement -->
    <div v-if="ghost" class="ghost" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
      <Sprite :id="ghost.species" :shiny="ghost.shiny" :size="80" />
    </div>
  </section>
</template>

<style scoped>
.saves {
  max-width: 1280px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
  font-size: 16px;
}

.picker {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 14px;
  margin-top: 24px;
}

.pick {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 18px;
  text-align: left;
  transition: transform 0.15s;
}

.pick:hover {
  transform: translateY(-2px);
}

.pick small {
  color: var(--text-dim);
}

.pick.add {
  border-style: dashed;
  justify-content: center;
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
}

.head-actions {
  display: flex;
  gap: 8px;
}

.banner {
  margin-bottom: 10px;
  padding: 10px 14px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  line-height: 1.45;
}

.banner.warn {
  background: var(--warn-bg);
  color: var(--warn);
}

.banner.danger {
  border: 1px solid var(--danger);
  color: var(--danger);
}

.banner.ok {
  background: color-mix(in srgb, var(--accent-2) 15%, transparent);
  color: var(--text);
}

.layout {
  display: grid;
  grid-template-columns: 1fr 340px;
  gap: 18px;
  align-items: start;
  margin-top: 8px;
}

.box {
  padding: 16px;
}

.box-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.box-head h2 {
  min-width: 160px;
  font-size: 20px;
  text-align: center;
}

.round {
  width: 36px;
  height: 36px;
  border: 1px solid var(--border);
  border-radius: 50%;
  background: var(--panel);
  font-size: 20px;
  line-height: 1;
}

.round:hover {
  background: var(--panel-hover);
}

.dots {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-left: auto;
}

.dots button {
  width: 8px;
  height: 8px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 30%, transparent);
}

.dots button.on {
  width: 18px;
  border-radius: 4px;
  background: var(--accent-2);
}

.grid {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 8px;
}

.slot {
  position: relative;
  display: grid;
  place-items: center;
  aspect-ratio: 1.15;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 5%, transparent);
  transition: transform 0.12s, background 0.12s, box-shadow 0.12s;
}

.slot:hover {
  background: var(--panel-hover);
  transform: translateY(-1px);
}

.slot.selected {
  box-shadow: 0 0 0 3px var(--accent-2);
}

.slot.over {
  box-shadow: 0 0 0 3px var(--accent);
  background: var(--panel-hover);
}

.slot .lvl {
  position: absolute;
  bottom: 4px;
  left: 6px;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-dim);
}

.slot .shiny {
  position: absolute;
  top: 4px;
  left: 6px;
  color: #ffd84d;
  font-size: 13px;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
}

.side {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.party {
  padding: 14px;
}

.party h3 {
  margin-bottom: 10px;
  font-size: 13px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-dim);
}

.party-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.party-grid .slot {
  aspect-ratio: 1.6;
}

.detail {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.detail-head h3 {
  font-size: 20px;
}

.gender {
  color: var(--accent-2);
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
  font-size: 13px;
}

dt {
  color: var(--text-dim);
}

dd {
  margin: 0;
  font-weight: 600;
}

.moves {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.moves li {
  padding: 6px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  font-size: 13px;
  font-weight: 600;
}

.stats {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.stats th {
  color: var(--text-dim);
  font-weight: 600;
  text-align: right;
}

.stats td {
  padding: 2px 0;
}

.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.max {
  color: var(--accent-2);
  font-weight: 700;
}

.detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.danger-btn {
  border-color: var(--danger);
  color: var(--danger);
}

.hint {
  font-size: 12px;
  line-height: 1.4;
}

.slot {
  touch-action: none;
  user-select: none;
}

.ghost {
  position: fixed;
  z-index: 60;
  pointer-events: none;
  transform: translate(-50%, -60%) scale(1.1);
  filter: drop-shadow(0 8px 12px rgba(0, 0, 0, 0.35));
}
</style>
