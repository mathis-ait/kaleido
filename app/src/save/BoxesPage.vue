<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import Combo from "../components/Combo.vue";
import Icon from "../components/Icon.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import {
  copyPokemon,
  createPokemon,
  deletePokemon,
  editPokemon,
  exportPokemon,
  importPokemon,
  loadBox,
  movePokemon,
  lists,
  notify,
  sameSlot,
  saveState,
  setBoxName,
} from "../saveStore";
import type { Slot, SlotView } from "../types";
import { BALLS, genderSymbol, NATURES } from "./refdata";
import { useShell } from "./shell";
import { openShowdown, showdownUi } from "./showdown/api";
import ShowdownDialog from "./showdown/ShowdownDialog.vue";
import SmogonSets from "./showdown/SmogonSets.vue";
import TeamsDialog from "./showdown/TeamsDialog.vue";

const view = computed(() => saveState.view!);
const boxCount = computed(() => view.value.boxNames.length);
const partySlots = computed(() => Array.from({ length: 6 }, (_, i) => view.value.party[i] ?? null));
const key = (s: Slot) => JSON.stringify(s);
const boxSlot = (index: number): Slot => ({ kind: "box", box: saveState.box, index });
const partySlot = (index: number): Slot => ({ kind: "party", index });

/** Case vide sélectionnée (création / import). */
const emptySel = ref<Slot | null>(null);
const sel = computed(() => saveState.selected);
const cursor = computed<Slot | null>(() => sel.value?.slot ?? emptySel.value);
const isCursor = (s: Slot) => !!cursor.value && sameSlot(cursor.value, s);

function select(slot: Slot, p: SlotView | null) {
  if (p) {
    saveState.selected = p;
    emptySel.value = null;
  } else {
    saveState.selected = null;
    emptySel.value = slot;
  }
  confirmDelete.value = false;
}

function changeBox(step: number) {
  const n = boxCount.value;
  if (n) loadBox((saveState.box + step + n) % n);
}

// ---- Renommer la boîte (double-clic sur le nom)
const renaming = ref(false);
const newName = ref("");
const renameInput = ref<HTMLInputElement | null>(null);
async function startRename() {
  newName.value = view.value.boxNames[saveState.box];
  renaming.value = true;
  await nextTick();
  renameInput.value?.select();
}
async function commitRename() {
  if (!renaming.value) return;
  renaming.value = false;
  const name = newName.value.trim();
  if (name && name !== view.value.boxNames[saveState.box]) await setBoxName(saveState.box, name);
}

// ---- Glisser-déposer à la souris (le glisser HTML est capté par Tauri pour les fichiers).
// Maj = copier, Alt = écraser la cible.
const dropTarget = ref<string | null>(null);
const ghost = ref<{ species: number; shiny: boolean; x: number; y: number; mode: "move" | "copy" | "overwrite" } | null>(null);
let pending: { slot: Slot; p: SlotView; x: number; y: number } | null = null;
let justDragged = false;

const slotUnder = (x: number, y: number) => document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-slot]")?.dataset.slot ?? null;
const modeOf = (e: { shiftKey: boolean; altKey: boolean }) => (e.shiftKey ? "copy" : e.altKey ? "overwrite" : "move");

function onPointerDown(e: PointerEvent, slot: Slot, p: SlotView | null) {
  if (!p || e.button !== 0) return;
  pending = { slot, p, x: e.clientX, y: e.clientY };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp, { once: true });
}

function onPointerMove(e: PointerEvent) {
  if (!pending) return;
  if (!ghost.value && Math.hypot(e.clientX - pending.x, e.clientY - pending.y) < 6) return;
  ghost.value = { species: pending.p.species, shiny: pending.p.shiny, x: e.clientX, y: e.clientY, mode: modeOf(e) };
  dropTarget.value = slotUnder(e.clientX, e.clientY);
}

function onPointerUp(e: PointerEvent) {
  window.removeEventListener("pointermove", onPointerMove);
  if (pending && ghost.value) {
    const target = slotUnder(e.clientX, e.clientY);
    if (target) {
      const to = JSON.parse(target) as Slot;
      const mode = modeOf(e);
      if (mode === "move") movePokemon(pending.slot, to);
      else copyPokemon(pending.slot, to, mode === "overwrite");
    }
    justDragged = true;
    setTimeout(() => (justDragged = false), 0);
  }
  pending = null;
  ghost.value = null;
  dropTarget.value = null;
}

function onSlotClick(slot: Slot, p: SlotView | null) {
  if (!justDragged) select(slot, p);
}

// ---- Presse-papiers interne (Ctrl+C / Ctrl+V)
const clipboard = ref<Slot | null>(null);
function copySel() {
  if (!sel.value) return;
  clipboard.value = sel.value.slot;
  notify(`${sel.value.nickname || sel.value.speciesName} copié : choisis une case puis Ctrl+V`);
}
function paste() {
  if (clipboard.value && cursor.value) copyPokemon(clipboard.value, cursor.value, false);
}

// ---- Case vide : création ou import
const newSpecies = ref(25);
const newLevel = ref(5);
const speciesOptions = computed(() => lists.species.map((o) => ({ ...o, hint: `n°${o.value}` })));
/** Espèce choisie pour la case vide, pour la fenêtre des sets stratégiques. */
const emptyTarget = computed(() =>
  emptySel.value ? { species: newSpecies.value, speciesName: lists.species.find((o) => o.value === newSpecies.value)?.label ?? "", slot: emptySel.value } : null,
);

async function importInto(slot: Slot) {
  const file = await open({ title: "Importer un Pokémon", filters: [{ name: "Pokémon", extensions: ["pk4", "pk5", "pk6", "pk7"] }] });
  if (typeof file === "string") importPokemon(slot, file);
}

async function exportSelected() {
  const p = sel.value;
  if (!p) return;
  const output = await save({
    title: "Exporter le Pokémon",
    defaultPath: `${p.speciesName}.pk${view.value.generation}`,
    filters: [{ name: "Pokémon", extensions: [`pk${view.value.generation}`] }],
  });
  if (output) exportPokemon(p.slot, output);
}

const confirmDelete = ref(false);
async function doDelete() {
  if (!sel.value) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    return;
  }
  await deletePokemon(sel.value.slot);
  confirmDelete.value = false;
}

// ---- Navigation au clavier dans la boîte
function moveCursor(dx: number, dy: number) {
  const c = cursor.value;
  let i = c?.kind === "box" ? c.index : 0;
  if (c?.kind === "box") {
    const col = (i % 6) + dx;
    const row = Math.floor(i / 6) + dy;
    if (col < 0 || col > 5) {
      changeBox(col < 0 ? -1 : 1);
      i = Math.floor(i / 6) * 6 + (col < 0 ? 5 : 0);
    } else {
      i = Math.min(29, Math.max(0, row * 6 + col));
    }
  }
  const slot = boxSlot(i);
  select(slot, saveState.slots[i] ?? null);
}

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement;
  if (["INPUT", "TEXTAREA"].includes(t?.tagName) || e.ctrlKey) return;
  const dirs: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  if (dirs[e.key]) {
    e.preventDefault();
    moveCursor(...dirs[e.key]);
  } else if (e.key === "PageUp" || e.key === "PageDown") {
    e.preventDefault();
    changeBox(e.key === "PageUp" ? -1 : 1);
  }
}

// Après un changement de boîte, la sélection d'une case vide suit la boîte affichée.
watch(
  () => saveState.box,
  (b) => {
    if (emptySel.value?.kind === "box") emptySel.value = { ...emptySel.value, box: b };
  },
);

onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

useShell(() => ({
  hint: "Glisser : déplacer · Maj + glisser : copier · Alt + glisser : écraser · Pg↑/Pg↓ : boîte",
  actions: sel.value
    ? [
        { key: "Ctrl+c", cap: "Ctrl+C", label: "Copier", run: copySel },
        { key: "x", cap: "X", label: "Exporter", run: exportSelected },
        { key: "Delete", cap: "Suppr", label: confirmDelete.value ? "Confirmer" : "Supprimer", run: doDelete },
        { key: "Enter", cap: "Entrée", label: "Modifier", run: () => editPokemon(sel.value!) },
      ]
    : emptySel.value
      ? [
          { key: "Ctrl+v", cap: "Ctrl+V", label: "Coller", run: paste, disabled: !clipboard.value },
          { key: "i", cap: "I", label: "Importer un fichier", run: () => importInto(emptySel.value!) },
          { key: "Enter", cap: "Entrée", label: "Créer", run: () => createPokemon(emptySel.value!, newSpecies.value, newLevel.value) },
        ]
      : [],
}));

const ballName = (id: number) => BALLS.find((b) => b.id === id)?.name ?? `Ball n°${id}`;
const locationText = (s: Slot) => (s.kind === "party" ? `Équipe · place ${s.index + 1}` : `${view.value.boxNames[s.box]} · case ${s.index + 1}`);
</script>

<template>
  <div class="boxes">
    <!-- Boîte -->
    <section class="box panel">
      <header class="box-head">
        <button class="round" aria-label="Boîte précédente (Pg↑)" @click="changeBox(-1)"><Icon name="chevron-left" /></button>
        <input
          v-if="renaming"
          ref="renameInput"
          v-model="newName"
          class="rename"
          :maxlength="view.boxNameMax"
          @keydown.enter.prevent="commitRename"
          @keydown.esc.stop.prevent="renaming = false"
          @blur="commitRename"
        />
        <h2 v-else title="Double-clic pour renommer" @dblclick="startRename">{{ view.boxNames[saveState.box] }}</h2>
        <button class="round" aria-label="Boîte suivante (Pg↓)" @click="changeBox(1)"><Icon name="chevron-right" /></button>
        <span class="fill">{{ view.boxFill[saveState.box] }}/30</span>
        <div class="dots">
          <button
            v-for="(name, i) in view.boxNames"
            :key="i"
            :class="{ on: i === saveState.box, full: view.boxFill[i] === 30, empty: !view.boxFill[i] }"
            :title="`${name} (${view.boxFill[i]}/30)`"
            :aria-label="name"
            @click="loadBox(i)"
          />
        </div>
        <button class="sv-btn" title="Équipes d'exemple de Smogon, prêtes à importer" @click="showdownUi.teams = true">
          <Icon name="star" :size="15" /> Équipes
        </button>
        <button class="sv-btn" title="Importer ou exporter une équipe au format Pokémon Showdown (Ctrl+I)" @click="openShowdown()">
          <Icon name="swords" :size="15" /> Showdown
        </button>
      </header>
      <div class="grid">
        <button
          v-for="(p, i) in saveState.slots"
          :key="i"
          class="slot"
          :class="{ selected: isCursor(boxSlot(i)), over: dropTarget === key(boxSlot(i)), filled: !!p }"
          :data-slot="key(boxSlot(i))"
          :title="p ? `${p.nickname || p.speciesName} · N. ${p.level}` : 'Case vide'"
          @pointerdown="onPointerDown($event, boxSlot(i), p)"
          @click="onSlotClick(boxSlot(i), p)"
          @dblclick="p && editPokemon(p)"
        >
          <template v-if="p">
            <Icon v-if="p.shiny" name="sparkle" :size="16" class="shiny" />
            <Icon v-if="p.isEgg" name="egg" :size="14" class="egg" />
            <Sprite :id="p.species" :shiny="p.shiny" :size="72" />
            <span v-if="isCursor(boxSlot(i))" class="lvl">N. {{ p.level }}</span>
            <span class="bar" :class="p.gender" />
          </template>
        </button>
      </div>
    </section>

    <aside class="side">
      <!-- Équipe -->
      <section class="panel party">
        <h3>Équipe</h3>
        <div class="party-grid">
          <button
            v-for="(p, i) in partySlots"
            :key="i"
            class="slot"
            :class="{ selected: isCursor(partySlot(i)), over: dropTarget === key(partySlot(i)), filled: !!p }"
            :data-slot="key(partySlot(i))"
            @pointerdown="onPointerDown($event, partySlot(i), p)"
            @click="onSlotClick(partySlot(i), p)"
            @dblclick="p && editPokemon(p)"
          >
            <template v-if="p">
              <Icon v-if="p.shiny" name="sparkle" :size="14" class="shiny" />
              <Sprite :id="p.species" :shiny="p.shiny" :size="64" />
              <span class="bar" :class="p.gender" />
            </template>
          </button>
        </div>
      </section>

      <!-- Fiche du Pokémon sélectionné -->
      <section v-if="sel" class="panel detail">
        <div class="detail-head">
          <Sprite :id="sel.species" :shiny="sel.shiny" :size="96" />
          <div>
            <h3>{{ sel.nickname || sel.speciesName }} <span class="gender">{{ genderSymbol(sel.gender) }}</span></h3>
            <div class="chips">
              <span class="chip-w">N. {{ sel.level }}</span>
              <span v-if="sel.shiny" class="chip-gold">★ Chromatique</span>
              <span v-if="sel.isEgg" class="chip-soft">Œuf</span>
              <span v-if="sel.isNicknamed" class="chip-soft">{{ sel.speciesName }}</span>
            </div>
          </div>
        </div>
        <dl>
          <dt>Emplacement</dt>
          <dd>{{ locationText(sel.slot) }}</dd>
          <dt>Nature</dt>
          <dd>{{ NATURES[sel.nature] ?? sel.natureName }}</dd>
          <dt>Talent</dt>
          <dd>{{ sel.abilityName }}</dd>
          <dt>Objet</dt>
          <dd>{{ sel.itemName ?? "—" }}</dd>
          <dt>Ball</dt>
          <dd>{{ ballName(sel.ball) }}</dd>
          <dt>Dresseur <Tip term="ot" /></dt>
          <dd>{{ sel.otName }} · {{ String(sel.tid).padStart(5, "0") }}</dd>
          <dt>Données <Tip term="checksum" /></dt>
          <dd :class="sel.checksumValid ? 'ok' : 'bad'">{{ sel.checksumValid ? "Intactes" : "Somme de contrôle invalide" }}</dd>
        </dl>
        <ul class="moves">
          <li v-for="m in sel.moveNames" :key="m">{{ m }}</li>
        </ul>
        <button class="btn-big" @click="editPokemon(sel)"><Icon name="pencil" :size="16" /> Modifier</button>
        <div class="row">
          <button class="btn-line" @click="copySel"><Icon name="copy" :size="15" /> Copier</button>
          <button class="btn-line" @click="exportSelected"><Icon name="file" :size="15" /> Exporter</button>
          <button class="btn-line" :class="{ danger: confirmDelete }" @click="doDelete">
            <Icon name="trash" :size="15" /> {{ confirmDelete ? "Confirmer" : "Supprimer" }}
          </button>
        </div>
      </section>

      <!-- Case vide -->
      <section v-else-if="emptySel" class="panel detail">
        <h3>Case vide</h3>
        <p class="dim">{{ locationText(emptySel) }}</p>
        <label class="lbl">Nouveau Pokémon</label>
        <Combo v-model="newSpecies" :options="speciesOptions" sprites placeholder="Espèce…" />
        <label class="lbl">Niveau</label>
        <input v-model.number="newLevel" class="num-input" type="number" min="1" max="100" />
        <p class="dim small">Il sera attrapé par toi ({{ view.trainer.name }}), dans une Poké Ball, avec ses données de base. Tu pourras tout modifier ensuite.</p>
        <button class="btn-big" @click="createPokemon(emptySel, newSpecies, newLevel)"><Icon name="plus" :size="16" /> Créer ici</button>
        <button class="btn-line full" title="Sets conseillés par Smogon pour cette espèce" @click="showdownUi.smogon = true"><Icon name="swords" :size="15" /> Sets stratégiques</button>
        <div class="row">
          <button class="btn-line" @click="importInto(emptySel)"><Icon name="download" :size="15" /> Importer un .pk{{ view.generation }}</button>
          <button class="btn-line" :disabled="!clipboard" @click="paste"><Icon name="copy" :size="15" /> Coller</button>
        </div>
      </section>

      <section v-else class="panel detail empty-help">
        <Icon name="grid" :size="36" />
        <p>Choisis un Pokémon pour voir sa fiche, ou une case vide pour en créer un.</p>
        <p class="dim small">Glisser pour déplacer · Maj : copier · Alt : écraser <Tip term="dragModes" /></p>
      </section>
    </aside>

    <!-- Sprite qui suit la souris pendant un déplacement -->
    <div v-if="ghost" class="ghost" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
      <Sprite :id="ghost.species" :shiny="ghost.shiny" :size="84" />
      <span v-if="ghost.mode !== 'move'" class="mode">{{ ghost.mode === "copy" ? "Copier" : "Écraser" }}</span>
    </div>

    <ShowdownDialog :target="cursor" />
    <SmogonSets v-if="emptyTarget" :empty="emptyTarget" />
    <TeamsDialog />
  </div>
</template>

<style scoped>
.boxes {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 360px;
  gap: 20px;
  height: 100%;
}

.panel {
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--panel);
  backdrop-filter: blur(14px);
  box-shadow: var(--shadow);
}

.box {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px 18px 18px;
}

.box-head {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 14px;
}

.box-head h2 {
  min-width: 160px;
  font-size: 22px;
  font-weight: 600;
  text-align: center;
  cursor: text;
}

.rename {
  width: 180px;
  padding: 6px 10px;
  border: 1px solid var(--accent-2);
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  color: var(--text);
  font: 600 18px var(--font);
  text-align: center;
  outline: none;
}

.round {
  display: grid;
  place-items: center;
  width: 40px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.round:hover {
  background: var(--panel-hover);
}

.fill {
  color: var(--text-dim);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.dots {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 5px;
  margin-left: auto;
  max-width: 55%;
}

.dots button {
  width: 9px;
  height: 9px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 55%, transparent);
}

.dots button.empty {
  background: color-mix(in srgb, var(--text) 22%, transparent);
}

.dots button.on {
  width: 14px;
  height: 14px;
  background: var(--text);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 60%, transparent);
}

.grid {
  display: grid;
  flex: 1;
  grid-template-columns: repeat(6, 1fr);
  grid-template-rows: repeat(5, 1fr);
  gap: 10px;
  min-height: 0;
  padding: 12px;
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.slot {
  position: relative;
  display: grid;
  place-items: center;
  min-height: 0;
  border: 1px solid color-mix(in srgb, var(--text) 18%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  touch-action: none;
  user-select: none;
  transition: transform 0.12s, background 0.12s, box-shadow 0.12s;
}

.slot:hover {
  background: color-mix(in srgb, var(--text) 20%, transparent);
}

.slot.selected {
  background: color-mix(in srgb, var(--text) 26%, transparent);
  box-shadow: 0 0 0 3px #fff, 0 0 0 7px color-mix(in srgb, var(--accent-2) 70%, transparent);
  z-index: 1;
}

.slot.over {
  box-shadow: 0 0 0 3px var(--accent-2);
  transform: scale(1.04);
}

.slot .lvl {
  position: absolute;
  bottom: 9px;
  left: 8px;
  padding: 1px 6px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.35);
  font-size: 11px;
  font-weight: 700;
}

.slot .shiny {
  position: absolute;
  top: 6px;
  left: 6px;
  color: #ff5a7a;
  fill: #ff5a7a;
}

.slot .egg {
  position: absolute;
  top: 6px;
  right: 6px;
  color: #fff1c4;
}

.bar {
  position: absolute;
  bottom: 5px;
  left: 18%;
  right: 18%;
  height: 4px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.bar.male {
  background: #5aa9ff;
}

.bar.female {
  background: #ff7eb6;
}

.side {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-height: 0;
  overflow-y: auto;
  padding-right: 2px;
}

.party {
  padding: 14px;
}

h3 {
  font-size: 18px;
}

.party h3 {
  margin-bottom: 10px;
  color: var(--text-dim);
  font-size: 12px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.party-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.party-grid .slot {
  height: 76px;
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
  font-size: 22px;
}

.gender {
  color: var(--accent-2);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 4px;
}

.chip-w,
.chip-gold,
.chip-soft {
  padding: 2px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}

.chip-w {
  background: var(--text);
  color: var(--bg);
}

.chip-gold {
  background: #ffe27a;
  color: #6b4b00;
}

.chip-soft {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 5px 12px;
  margin: 0;
  font-size: 13px;
}

dt {
  display: flex;
  align-items: center;
  color: var(--text-dim);
}

dd {
  margin: 0;
  font-weight: 600;
  text-align: right;
}

dd.ok {
  color: #8ff0b5;
}

dd.bad {
  color: var(--danger);
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
  background: color-mix(in srgb, var(--text) 12%, transparent);
  font-size: 13px;
  font-weight: 600;
}

.btn-big {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 11px;
  border: none;
  border-radius: 999px;
  background: var(--text);
  color: var(--bg);
  font-weight: 700;
}

.row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(90px, 1fr));
  gap: 8px;
}

.btn-line.full {
  width: 100%;
  margin-top: 8px;
}

.btn-line {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 9px 10px;
  border: 1.5px solid color-mix(in srgb, var(--text) 70%, transparent);
  border-radius: 999px;
  background: transparent;
  font-weight: 600;
  font-size: 13px;
}

.btn-line:hover:not(:disabled) {
  background: color-mix(in srgb, var(--text) 12%, transparent);
}

.btn-line:disabled {
  opacity: 0.4;
}

.btn-line.danger {
  border-color: var(--danger);
  color: var(--danger);
}

.lbl {
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.num-input {
  width: 100%;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 7%, transparent);
  color: var(--text);
  font: 600 14px var(--font);
}

.dim {
  margin: 0;
  color: var(--text-dim);
}

.small {
  font-size: 12px;
  line-height: 1.45;
}

.empty-help {
  align-items: center;
  padding: 30px 20px;
  text-align: center;
  color: var(--text-dim);
}

.empty-help p {
  margin: 0;
}

.ghost {
  position: fixed;
  z-index: 60;
  display: flex;
  flex-direction: column;
  align-items: center;
  pointer-events: none;
  transform: translate(-50%, -60%) scale(1.1);
  filter: drop-shadow(0 8px 12px rgba(0, 0, 0, 0.35));
}

.ghost .mode {
  padding: 2px 10px;
  border-radius: 999px;
  background: var(--text);
  color: var(--bg);
  font-size: 12px;
  font-weight: 700;
}
</style>
