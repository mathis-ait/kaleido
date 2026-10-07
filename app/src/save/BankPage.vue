<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import Banner from "../components/Banner.vue";
import Dialog from "../components/Dialog.vue";
import EmptyState from "../components/EmptyState.vue";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import {
  bankAddBox,
  bankDelete,
  bankDeleteBox,
  bankDeposit,
  bankExport,
  bankImport,
  bankMove,
  bankRenameBox,
  bankSearch,
  bankSetPath,
  bankState,
  bankWithdraw,
  loadBankBox,
  loadBankInfo,
  sameBankSlot,
  selectBank,
  type BankSlot,
  type BankSlotView,
  type Problem,
} from "../bankStore";
import { copyPokemon, loadBox, movePokemon, sameSlot, saveState } from "../saveStore";
import type { Slot, SlotView } from "../types";
import { genderSymbol, NATURES } from "./refdata";
import { useShell } from "./shell";

const emit = defineEmits<{ open: [] }>();

const view = computed(() => saveState.view);
const info = computed(() => bankState.info);
const boxCount = computed(() => info.value?.boxes.length ?? 0);
const bankBox = computed(() => info.value?.boxes[bankState.box]);
const detail = computed(() => bankState.detail);
const compat = computed(() => detail.value?.compatibility ?? null);
const canSend = computed(() => !!compat.value && !compat.value.blocker && (compat.value.ok || compat.value.fixable));
const partySlots = computed(() => Array.from({ length: 6 }, (_, i) => view.value?.party[i] ?? null));
/** « Platine · Aurore » : on n'affiche que les parties connues (pas de point orphelin). */
const saveTitle = computed(() => (view.value ? [view.value.game.replace("Pokémon ", ""), view.value.trainer.name.trim()].filter(Boolean).join(" · ") : ""));
const bslot = (index: number): BankSlot => ({ box: bankState.box, index });
const boxSlot = (index: number): Slot => ({ kind: "box", box: saveState.box, index });
const partySlot = (index: number): Slot => ({ kind: "party", index });
const bkey = (s: BankSlot) => `b:${s.box}:${s.index}`;
const skey = (s: Slot) => `s:${JSON.stringify(s)}`;

/** Pokémon de la sauvegarde sélectionné (pour le déposer avec un bouton). */
const saveSel = ref<SlotView | null>(null);

// ---- Chargement de la banque (état de chargement, erreur avec « Réessayer »)
const loading = ref(true);
const loadError = ref<string | null>(null);

async function init() {
  loading.value = true;
  loadError.value = null;
  const ok = await loadBankInfo();
  if (!ok) {
    loadError.value = saveState.error ?? "La banque n'a pas pu être lue.";
    saveState.error = null;
    loading.value = false;
    return;
  }
  await loadBankBox(Math.min(bankState.box, Math.max(0, boxCount.value - 1)));
  loading.value = false;
  if (view.value && !saveState.slots.length) await loadBox(saveState.box);
  if (bankState.selected) await selectBank(bankState.selected);
}

onMounted(() => {
  // Capture : passe avant la page (Pg↑ / Pg↓ changent la boîte de la banque).
  window.addEventListener("keydown", onKey, true);
  init();
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey, true));

async function changeBankBox(step: number) {
  const n = boxCount.value;
  if (n) await loadBankBox((bankState.box + step + n) % n);
}

async function changeSaveBox(step: number) {
  const n = view.value?.boxNames.length ?? 0;
  if (n) await loadBox((saveState.box + step + n) % n);
}

// ---- Renommer une boîte de la banque (clic sur son nom)
const renaming = ref(false);
const newName = ref("");
const renameInput = ref<HTMLInputElement | null>(null);
async function startRename() {
  newName.value = bankBox.value?.name ?? "";
  renaming.value = true;
  await nextTick();
  renameInput.value?.select();
}
async function commitRename() {
  if (!renaming.value) return;
  renaming.value = false;
  const name = newName.value.trim();
  if (name && name !== bankBox.value?.name) await bankRenameBox(bankState.box, name);
}

// ---- Recherche dans toute la banque
const search = ref("");
const shinyOnly = ref(false);
const genFilter = ref<number | null>(null);
const results = ref<BankSlotView[]>([]);
const searching = computed(() => !!search.value.trim() || shinyOnly.value || genFilter.value !== null);
let searchTimer: number | undefined;
async function runSearch() {
  results.value = searching.value ? await bankSearch(search.value, shinyOnly.value ? true : null, genFilter.value) : [];
}
watch([search, shinyOnly, genFilter], () => {
  clearTimeout(searchTimer);
  searchTimer = window.setTimeout(runSearch, 150);
});
watch(() => info.value?.count, () => searching.value && runSearch());
function clearSearch() {
  search.value = "";
  shinyOnly.value = false;
  genFilter.value = null;
}

// ---- Sélection
const confirmDelete = ref(false);
function pickBank(slot: BankSlot, p: BankSlotView | null) {
  if (justDragged) return;
  confirmDelete.value = false;
  saveSel.value = null;
  if (p && slot.box !== bankState.box) loadBankBox(slot.box);
  selectBank(slot);
}
function pickSave(p: SlotView | null) {
  if (justDragged) return;
  saveSel.value = p;
}

// ---- Glisser-déposer à la souris, entre la banque et la sauvegarde (Maj = copier)
type Source = { kind: "bank"; slot: BankSlot } | { kind: "save"; slot: Slot };
const dropTarget = ref<string | null>(null);
const ghost = ref<{ species: number; shiny: boolean; x: number; y: number; copy: boolean } | null>(null);
let pending: { source: Source; species: number; shiny: boolean; x: number; y: number } | null = null;
let justDragged = false;

function targetUnder(x: number, y: number): { key: string; bank?: BankSlot; save?: Slot } | null {
  const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-bslot],[data-sslot]");
  if (!el) return null;
  if (el.dataset.bslot) {
    const s = JSON.parse(el.dataset.bslot) as BankSlot;
    return { key: bkey(s), bank: s };
  }
  const s = JSON.parse(el.dataset.sslot!) as Slot;
  return { key: skey(s), save: s };
}

function onPointerDown(e: PointerEvent, source: Source, p: { species: number; shiny: boolean } | null) {
  if (!p || e.button !== 0) return;
  pending = { source, species: p.species, shiny: p.shiny, x: e.clientX, y: e.clientY };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp, { once: true });
}

function onPointerMove(e: PointerEvent) {
  if (!pending) return;
  if (!ghost.value && Math.hypot(e.clientX - pending.x, e.clientY - pending.y) < 6) return;
  ghost.value = { species: pending.species, shiny: pending.shiny, x: e.clientX, y: e.clientY, copy: e.shiftKey };
  dropTarget.value = targetUnder(e.clientX, e.clientY)?.key ?? null;
}

async function onPointerUp(e: PointerEvent) {
  window.removeEventListener("pointermove", onPointerMove);
  const p = pending;
  const dragged = !!ghost.value;
  pending = null;
  ghost.value = null;
  dropTarget.value = null;
  if (!p || !dragged) return;
  justDragged = true;
  setTimeout(() => (justDragged = false), 0);
  const t = targetUnder(e.clientX, e.clientY);
  if (!t) return;
  const copy = e.shiftKey;
  const src = p.source;
  if (src.kind === "bank" && t.bank) await bankMove(src.slot, t.bank);
  else if (src.kind === "bank" && t.save) await withdraw(src.slot, t.save, copy);
  else if (src.kind === "save" && t.bank) await bankDeposit(src.slot, t.bank, copy);
  else if (src.kind === "save" && t.save && !sameSlot(src.slot, t.save)) {
    if (copy) await copyPokemon(src.slot, t.save, false);
    else await movePokemon(src.slot, t.save);
  }
}

// ---- Retrait vers la sauvegarde (avec confirmation si des attaques / objets doivent partir)
const stripAsk = ref<{ from: BankSlot; to: Slot; copy: boolean; problems: Problem[] } | null>(null);
const stripOpen = computed({
  get: () => !!stripAsk.value,
  set: (v: boolean) => {
    if (!v) stripAsk.value = null;
  },
});

async function withdraw(from: BankSlot, to: Slot, copy: boolean) {
  if (!view.value) return;
  if (!sameBankSlot(bankState.selected, from)) await selectBank(from);
  const c = bankState.detail?.compatibility;
  if (c?.blocker) {
    saveState.error = c.blocker;
    return;
  }
  if (c && !c.ok && c.fixable) {
    stripAsk.value = { from, to, copy, problems: c.problems };
    return;
  }
  await bankWithdraw(from, to, copy, false);
}

async function confirmStrip() {
  const a = stripAsk.value;
  stripAsk.value = null;
  if (a) await bankWithdraw(a.from, a.to, a.copy, true);
}

/** Première case vide de la boîte affichée, sinon de l'équipe. */
function freeSaveSlot(): Slot | null {
  const i = saveState.slots.findIndex((s) => !s);
  if (i >= 0) return boxSlot(i);
  if ((view.value?.party.length ?? 6) < 6) return partySlot(view.value!.party.length);
  return null;
}

async function sendSelected(copy: boolean) {
  const s = bankState.selected;
  if (!s || !detail.value) return;
  const to = freeSaveSlot();
  if (!to) {
    saveState.error = "Aucune case libre dans la boîte affichée de la sauvegarde : change de boîte.";
    return;
  }
  await withdraw(s, to, copy);
}

async function depositSelected(copy: boolean) {
  if (!saveSel.value) return;
  const target = bankState.selected && !bankState.slots[bankState.selected.index] && bankState.selected.box === bankState.box ? bankState.selected : null;
  if (await bankDeposit(saveSel.value.slot, target, copy)) saveSel.value = null;
}

// ---- Import / export / suppression
async function importFiles() {
  const files = await open({ title: "Importer dans la banque", multiple: true, filters: [{ name: "Pokémon", extensions: ["pk4", "pk5", "pk6", "pk7"] }] });
  const list = Array.isArray(files) ? files : typeof files === "string" ? [files] : [];
  if (!list.length) return;
  const target = bankState.selected && !bankState.slots[bankState.selected.index] ? bankState.selected : null;
  await bankImport(list, target);
}

async function exportSelected() {
  const d = detail.value;
  if (!d) return;
  const ext = `pk${d.generation}`;
  const output = await save({ title: "Exporter le Pokémon", defaultPath: `${d.speciesName}.${ext}`, filters: [{ name: "Pokémon", extensions: [ext] }] });
  if (output) bankExport(d.slot, output);
}

async function doDelete() {
  const s = bankState.selected;
  if (!s || !detail.value) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    return;
  }
  confirmDelete.value = false;
  await bankDelete(s);
}

// ---- Dossier de la banque
async function chooseFolder() {
  const dir = await open({ title: "Dossier de la Banque Kaleido", directory: true });
  if (typeof dir === "string") bankSetPath(dir);
}

// ---- Clavier : flèches dans les grilles (comme la page Boîtes), Pg↑ / Pg↓ pour la banque
const DIRS: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

async function focusSlot(grid: string, index: number) {
  await nextTick();
  const el = document.querySelector<HTMLElement>(`.bank [data-grid="${grid}"] [data-i="${index}"]`);
  if (!el) return;
  el.focus();
  el.click();
}

/** Flèche dans une grille de 6 colonnes ; au bord gauche / droit d'une boîte, passe à la boîte voisine. */
async function moveInGrid(btn: HTMLElement, d: [number, number]) {
  const grid = btn.closest<HTMLElement>("[data-grid]")!;
  const kind = grid.dataset.grid!;
  const count = grid.querySelectorAll("[data-i]").length;
  const i = Number(btn.dataset.i);
  const col = (i % 6) + d[0];
  const row = Math.floor(i / 6) + d[1];
  let next = row * 6 + col;
  if (kind === "results") next = i + d[0] + d[1] * 6;
  else if (col < 0 || col > 5) {
    if (kind !== "bank" && kind !== "save") return;
    await (kind === "bank" ? changeBankBox : changeSaveBox)(col < 0 ? -1 : 1);
    next = Math.floor(i / 6) * 6 + (col < 0 ? 5 : 0);
  }
  if (next >= 0 && next < count) await focusSlot(kind, next);
}

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement;
  if (stripAsk.value || ["INPUT", "TEXTAREA"].includes(t?.tagName) || e.ctrlKey) return;
  const d = DIRS[e.key];
  if (d) {
    const btn = t?.closest?.<HTMLElement>("[data-grid] [data-i]");
    if (btn) {
      e.preventDefault();
      moveInGrid(btn, d);
    } else if (!t || t === document.body) {
      // Rien n'a le focus : on entre dans la grille de la banque, sur la case choisie.
      e.preventDefault();
      const s = bankState.selected;
      focusSlot(searching.value ? "results" : "bank", s && s.box === bankState.box && !searching.value ? s.index : 0);
    }
  } else if (e.key === "PageUp" || e.key === "PageDown") {
    e.preventDefault();
    changeBankBox(e.key === "PageUp" ? -1 : 1);
  }
}

useShell(() => ({
  hint: view.value ? "Glisser : déplacer · Maj + glisser : copier · Pg↑/Pg↓ : boîte de la banque" : "Ouvre une sauvegarde pour y envoyer ou y déposer des Pokémon",
  actions: [
    { key: "i", cap: "I", label: "Importer", run: importFiles },
    ...(detail.value
      ? [
          { key: "x", cap: "X", label: "Exporter", run: exportSelected },
          { key: "Delete", cap: "Suppr", label: confirmDelete.value ? "Confirmer" : "Supprimer", run: doDelete },
          { key: "Enter", cap: "Entrée", label: "Envoyer", run: () => sendSelected(false), disabled: !view.value || !compat.value || !!compat.value.blocker },
        ]
      : saveSel.value
        ? [{ key: "Enter", cap: "Entrée", label: "Déposer", run: () => depositSelected(false) }]
        : []),
  ],
}));

const dateFr = (iso: string) => {
  const d = new Date(`${iso}T12:00:00`);
  return isNaN(d.getTime()) ? iso : d.toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" });
};
const GENS = [4, 5, 6, 7];
</script>

<template>
  <div class="bank" :class="{ solo: !view }">
    <!-- Banque -->
    <section class="sv-panel col">
      <header class="head">
        <h3 class="sv-label">Banque Kaleido <Tip term="bank.bank" /></h3>
        <span v-if="info" class="count">{{ info.count }} Pokémon</span>
        <button class="sv-btn" title="Importer des fichiers .pk4 à .pk7 (I)" @click="importFiles"><Icon name="download" :size="15" /> Importer</button>
      </header>

      <Banner v-if="loadError" :retry="init">Impossible de lire la Banque Kaleido : {{ loadError }}</Banner>
      <EmptyState v-else-if="loading" loading compact title="Ouverture de la banque…" class="fill-area" />

      <template v-else-if="info">
        <SearchField v-model="search" placeholder="Espèce, surnom ou dresseur" />
        <div class="filters">
          <button class="sv-chip" :class="{ on: shinyOnly }" :aria-pressed="shinyOnly" title="Chromatiques seulement" @click="shinyOnly = !shinyOnly">
            <Icon name="sparkle" :size="12" /> Chromatique
          </button>
          <Tip term="shiny" />
          <span class="sep" />
          <button
            v-for="g in GENS"
            :key="g"
            class="sv-chip"
            :class="{ on: genFilter === g }"
            :aria-pressed="genFilter === g"
            :title="`Format PK${g} seulement`"
            @click="genFilter = genFilter === g ? null : g"
          >
            PK{{ g }}
          </button>
          <Tip term="bank.format" />
        </div>

        <template v-if="searching">
          <template v-if="results.length">
            <p class="sv-help">{{ results.length }} résultat{{ results.length > 1 ? "s" : "" }} dans toute la banque</p>
            <div class="grid results" data-grid="results">
              <button
                v-for="(r, i) in results"
                :key="r.id"
                type="button"
                class="slot filled"
                :class="{ selected: sameBankSlot(bankState.selected, r.slot), over: dropTarget === bkey(r.slot) }"
                :data-bslot="JSON.stringify(r.slot)"
                :data-i="i"
                :title="`${r.nickname || r.speciesName} · N. ${r.level} · ${info.boxes[r.slot.box]?.name} case ${r.slot.index + 1}`"
                :aria-label="`${r.nickname || r.speciesName}, N. ${r.level}, ${info.boxes[r.slot.box]?.name} case ${r.slot.index + 1}`"
                @pointerdown="onPointerDown($event, { kind: 'bank', slot: r.slot }, r)"
                @click="pickBank(r.slot, r)"
              >
                <Icon v-if="r.shiny" name="sparkle" :size="13" class="shiny" />
                <Sprite :id="r.species" :shiny="r.shiny" :size="52" />
                <span class="fmt">{{ r.format }}</span>
              </button>
            </div>
          </template>
          <EmptyState v-else compact icon="search" title="Aucun résultat" class="fill-area">
            Aucun Pokémon de la banque ne correspond à cette recherche.
            <template #actions>
              <button class="sv-btn" @click="clearSearch"><Icon name="x" :size="15" /> Effacer la recherche</button>
            </template>
          </EmptyState>
        </template>

        <template v-else>
          <div class="box-head">
            <button class="sv-round" aria-label="Boîte précédente (Pg↑)" title="Boîte précédente (Pg↑)" :disabled="boxCount <= 1" @click="changeBankBox(-1)">
              <Icon name="chevron-left" />
            </button>
            <input
              v-if="renaming"
              ref="renameInput"
              v-model="newName"
              class="sv-input rename"
              maxlength="40"
              aria-label="Nom de la boîte"
              @keydown.enter.prevent="commitRename"
              @keydown.esc.stop.prevent="renaming = false"
              @blur="commitRename"
            />
            <button v-else type="button" class="box-name" title="Renommer la boîte" @click="startRename">
              <span>{{ bankBox?.name }}</span>
              <Icon name="pencil" :size="13" />
            </button>
            <button class="sv-round" aria-label="Boîte suivante (Pg↓)" title="Boîte suivante (Pg↓)" :disabled="boxCount <= 1" @click="changeBankBox(1)">
              <Icon name="chevron-right" />
            </button>
            <span class="fill">{{ bankBox?.count ?? 0 }}/30</span>
            <span class="spacer" />
            <button class="sv-round sq" aria-label="Ajouter une boîte" title="Ajouter une boîte" @click="bankAddBox"><Icon name="plus" :size="15" /></button>
            <button
              class="sv-round sq"
              aria-label="Supprimer cette boîte"
              :title="bankBox?.count ? 'Vide la boîte pour pouvoir la supprimer' : 'Supprimer cette boîte (vide)'"
              :disabled="!!bankBox?.count || boxCount <= 1"
              @click="bankDeleteBox(bankState.box)"
            >
              <Icon name="trash" :size="15" />
            </button>
          </div>
          <div v-if="boxCount > 1" class="sv-dots">
            <button
              v-for="(b, i) in info.boxes"
              :key="i"
              type="button"
              :class="{ on: i === bankState.box, empty: !b.count }"
              :title="`${b.name} (${b.count}/30)`"
              :aria-label="`${b.name} (${b.count}/30)`"
              :aria-current="i === bankState.box"
              @click="loadBankBox(i)"
            />
          </div>
          <div class="grid box" data-grid="bank">
            <button
              v-for="(p, i) in bankState.slots"
              :key="i"
              type="button"
              class="slot"
              :class="{
                selected: sameBankSlot(bankState.selected, bslot(i)),
                over: dropTarget === bkey(bslot(i)),
                filled: !!p,
                blocked: !!p && view && bankState.compat[i] === false,
              }"
              :data-bslot="JSON.stringify(bslot(i))"
              :data-i="i"
              :title="p ? `${p.nickname || p.speciesName} · N. ${p.level} · ${p.format}` : 'Case vide'"
              :aria-label="p ? `${p.nickname || p.speciesName}, N. ${p.level}, ${p.format}` : `Case ${i + 1} vide`"
              @pointerdown="onPointerDown($event, { kind: 'bank', slot: bslot(i) }, p)"
              @click="pickBank(bslot(i), p)"
            >
              <template v-if="p">
                <Icon v-if="p.shiny" name="sparkle" :size="13" class="shiny" />
                <Icon v-if="p.isEgg" name="egg" :size="12" class="egg" />
                <Sprite :id="p.species" :shiny="p.shiny" :size="52" />
                <span class="fmt">{{ p.format }}</span>
                <span class="bar" :class="p.gender" />
              </template>
            </button>
          </div>
        </template>

        <div class="path">
          <Icon name="folder" :size="13" />
          <span class="path-text" :title="info.path">{{ info.path }}</span>
          <button type="button" class="sv-link" @click="chooseFolder">Changer</button>
          <button v-if="info.path !== info.defaultPath" type="button" class="sv-link" @click="bankSetPath(null)">Par défaut</button>
          <button type="button" class="sv-link" @click="openPath(info.path)">Ouvrir</button>
          <Tip term="bank.trash" />
        </div>
      </template>
    </section>

    <!-- Sauvegarde ouverte -->
    <section v-if="view" class="sv-panel col">
      <header class="head">
        <h3 class="sv-label save-title" :title="saveTitle"><span>{{ saveTitle }}</span> <Tip term="bank.drag" /></h3>
      </header>
      <div class="box-head">
        <button class="sv-round" aria-label="Boîte précédente" title="Boîte précédente" @click="changeSaveBox(-1)"><Icon name="chevron-left" /></button>
        <h2 class="box-title">{{ view.boxNames[saveState.box] }}</h2>
        <button class="sv-round" aria-label="Boîte suivante" title="Boîte suivante" @click="changeSaveBox(1)"><Icon name="chevron-right" /></button>
        <span class="fill">{{ view.boxFill[saveState.box] }}/30</span>
      </div>
      <EmptyState v-if="!saveState.slots.length" loading compact title="Lecture de la boîte…" class="fill-area" />
      <div v-else class="grid box" data-grid="save">
        <button
          v-for="(p, i) in saveState.slots"
          :key="i"
          type="button"
          class="slot"
          :class="{ selected: !!p && !!saveSel && sameSlot(saveSel.slot, boxSlot(i)), over: dropTarget === skey(boxSlot(i)), filled: !!p }"
          :data-sslot="JSON.stringify(boxSlot(i))"
          :data-i="i"
          :title="p ? `${p.nickname || p.speciesName} · N. ${p.level}` : 'Case vide'"
          :aria-label="p ? `${p.nickname || p.speciesName}, N. ${p.level}` : `Case ${i + 1} vide`"
          @pointerdown="onPointerDown($event, { kind: 'save', slot: boxSlot(i) }, p)"
          @click="pickSave(p)"
        >
          <template v-if="p">
            <Icon v-if="p.shiny" name="sparkle" :size="13" class="shiny" />
            <Icon v-if="p.isEgg" name="egg" :size="12" class="egg" />
            <Sprite :id="p.species" :shiny="p.shiny" :size="52" />
            <span class="bar" :class="p.gender" />
          </template>
        </button>
      </div>
      <div class="grid party" data-grid="party">
        <button
          v-for="(p, i) in partySlots"
          :key="i"
          type="button"
          class="slot"
          :class="{ selected: !!p && !!saveSel && sameSlot(saveSel.slot, partySlot(i)), over: dropTarget === skey(partySlot(i)), filled: !!p }"
          :data-sslot="JSON.stringify(partySlot(i))"
          :data-i="i"
          :title="p ? `${p.nickname || p.speciesName} · N. ${p.level} (équipe)` : 'Équipe : place libre'"
          :aria-label="p ? `${p.nickname || p.speciesName}, N. ${p.level}, équipe` : `Équipe : place ${i + 1} libre`"
          @pointerdown="onPointerDown($event, { kind: 'save', slot: partySlot(i) }, p)"
          @click="pickSave(p)"
        >
          <Sprite v-if="p" :id="p.species" :shiny="p.shiny" :size="44" />
        </button>
      </div>
    </section>

    <!-- Fiche -->
    <aside class="sv-panel side">
      <template v-if="detail">
        <div class="detail-head">
          <Sprite :id="detail.species" :shiny="detail.shiny" :size="88" />
          <div class="detail-id">
            <h3>{{ detail.nickname || detail.speciesName }} <span class="gender">{{ genderSymbol(detail.pokemon.gender) }}</span></h3>
            <div class="chips">
              <span class="sv-chip on">N. {{ detail.level }}</span>
              <span class="sv-chip">{{ detail.format }} <Tip term="bank.format" /></span>
              <span v-if="detail.shiny" class="sv-chip shiny"><Icon name="sparkle" :size="12" /> Chromatique</span>
              <span v-if="detail.isEgg" class="sv-chip dim">Œuf</span>
              <span v-if="detail.pokemon.isNicknamed" class="sv-chip dim">{{ detail.speciesName }}</span>
            </div>
          </div>
        </div>

        <!-- Compatibilité avec la sauvegarde ouverte -->
        <div v-if="compat" class="compat" :class="compat.ok ? 'ok' : compat.fixable ? 'warn' : 'bad'">
          <template v-if="compat.blocker">
            <strong><Icon name="alert" :size="15" /> Incompatible avec cette sauvegarde <Tip term="bank.direction" /></strong>
            <p>{{ compat.blocker }}</p>
          </template>
          <template v-else-if="compat.ok">
            <strong><Icon name="check" :size="15" /> Compatible avec cette sauvegarde</strong>
            <p v-if="compat.converts">Il sera converti : {{ compat.from.replace("gen", "PK") }} → {{ compat.to.replace("gen", "PK") }}. <Tip term="bank.changes" /></p>
            <p v-else>Même format : aucune conversion.</p>
          </template>
          <template v-else>
            <strong><Icon name="alert" :size="15" /> {{ compat.fixable ? "Compatible en retirant :" : "Incompatible avec cette sauvegarde" }}</strong>
            <ul>
              <li v-for="pb in compat.problems" :key="pb.kind + pb.id">{{ pb.message }}</li>
            </ul>
          </template>
          <ul v-if="compat.changes.length && !compat.blocker" class="changes">
            <li v-for="c in compat.changes" :key="c">{{ c }}</li>
          </ul>
        </div>
        <p v-else-if="!view" class="sv-help">Ouvre une sauvegarde pour savoir si ce Pokémon peut y aller. <Tip term="bank.direction" /></p>

        <dl class="sv-dl">
          <dt>Origine <Tip term="origin" /></dt>
          <dd>{{ detail.originGame ?? "Fichier importé" }}</dd>
          <template v-if="detail.originSave">
            <dt>Fichier</dt>
            <dd class="ellipsis" :title="detail.originSave">{{ detail.originSave }}</dd>
          </template>
          <dt>Arrivé le</dt>
          <dd>{{ dateFr(detail.added) }}</dd>
          <dt>Dresseur <Tip term="ot" /></dt>
          <dd>{{ detail.pokemon.otName }} · {{ String(detail.pokemon.tid).padStart(5, "0") }}</dd>
          <template v-if="detail.handler">
            <dt>Soigneur <Tip term="handler" /></dt>
            <dd>{{ detail.handler }}</dd>
          </template>
          <dt>Nature <Tip term="nature" /></dt>
          <dd>{{ NATURES[detail.pokemon.nature] ?? detail.pokemon.natureName }}</dd>
          <dt>Talent <Tip term="ability" /></dt>
          <dd>{{ detail.pokemon.abilityName }}</dd>
          <dt>Objet tenu <Tip term="heldItem" /></dt>
          <dd>{{ detail.pokemon.itemName ?? "—" }}</dd>
          <dt>Lieu de rencontre <Tip term="metLocation" /></dt>
          <dd>{{ detail.pokemon.metLocationName ?? `Lieu n°${detail.pokemon.metLocation}` }}</dd>
        </dl>
        <ul class="sv-moves">
          <li v-for="m in detail.pokemon.moveNames" :key="m">{{ m }}</li>
        </ul>
        <button v-if="view" class="sv-btn solid" :disabled="!canSend" @click="sendSelected(false)">
          <Icon name="upload" :size="16" /> Envoyer dans la sauvegarde
        </button>
        <div class="actions">
          <button v-if="view" class="sv-btn" :disabled="!canSend" title="L'original reste dans la banque" @click="sendSelected(true)">
            <Icon name="copy" :size="15" /> Copier
          </button>
          <button class="sv-btn" @click="exportSelected"><Icon name="file" :size="15" /> Exporter</button>
          <button class="sv-btn" :class="{ danger: confirmDelete }" @click="doDelete">
            <Icon name="trash" :size="15" /> {{ confirmDelete ? "Confirmer" : "Supprimer" }}
          </button>
        </div>
      </template>

      <template v-else-if="saveSel">
        <div class="detail-head">
          <Sprite :id="saveSel.species" :shiny="saveSel.shiny" :size="88" />
          <div class="detail-id">
            <h3>{{ saveSel.nickname || saveSel.speciesName }}</h3>
            <div class="chips">
              <span class="sv-chip on">N. {{ saveSel.level }}</span>
              <span class="sv-chip">PK{{ view?.generation }} <Tip term="bank.format" /></span>
              <span v-if="saveSel.shiny" class="sv-chip shiny"><Icon name="sparkle" :size="12" /> Chromatique</span>
            </div>
          </div>
        </div>
        <p class="sv-help">
          Range ce Pokémon dans la banque pour le garder à l'abri ou l'envoyer plus tard dans un autre jeu (même génération ou plus récente).
          <Tip term="bank.direction" />
        </p>
        <button class="sv-btn solid" @click="depositSelected(false)"><Icon name="download" :size="16" /> Déposer dans la banque</button>
        <button class="sv-btn" title="Il reste aussi dans la sauvegarde" @click="depositSelected(true)"><Icon name="copy" :size="15" /> Déposer une copie</button>
      </template>

      <EmptyState v-else-if="info && !info.count" icon="bank" title="Ta banque est vide" term="bank.bank">
        Importe des fichiers .pk4 à .pk7, ou glisse un Pokémon de ta sauvegarde vers la banque.
        <template #actions>
          <button class="sv-btn solid" @click="importFiles"><Icon name="download" :size="15" /> Importer des fichiers</button>
          <button v-if="!view" class="sv-btn" @click="emit('open')"><Icon name="folder-open" :size="15" /> Ouvrir une sauvegarde</button>
        </template>
      </EmptyState>

      <EmptyState v-else icon="bank" title="Banque Kaleido" term="bank.bank">
        La banque garde tes Pokémon hors de tes sauvegardes, pour les faire passer d'un jeu à l'autre. Transferts vers une génération égale ou plus récente seulement.
        <template #details>
          <p v-if="view" class="sv-help">Choisis un Pokémon, ou glisse-le d'un côté à l'autre · Maj : copier <Tip term="bank.drag" /></p>
          <p v-else class="sv-help">Sens des transferts <Tip term="bank.direction" /></p>
        </template>
        <template v-if="!view" #actions>
          <button class="sv-btn solid" @click="emit('open')"><Icon name="folder-open" :size="15" /> Ouvrir une sauvegarde</button>
        </template>
      </EmptyState>
    </aside>

    <!-- Confirmation : retirer les attaques / l'objet absents du jeu -->
    <Dialog v-model="stripOpen" title="Envoyer quand même ?" icon="alert" term="bank.changes" :width="460">
      <p class="sv-help">
        Ces éléments n'existent pas dans ce jeu. Kaleido peut les retirer du Pokémon envoyé (l'exemplaire de la banque n'est pas modifié s'il s'agit d'une copie) :
      </p>
      <ul class="strip-list">
        <li v-for="pb in stripAsk?.problems ?? []" :key="pb.kind + pb.id">{{ pb.message }}</li>
      </ul>
      <template #foot>
        <span class="spacer" />
        <button class="sv-btn" @click="stripAsk = null">Annuler</button>
        <button class="sv-btn solid" autofocus @click="confirmStrip">Retirer et envoyer</button>
      </template>
    </Dialog>

    <!-- Sprite qui suit la souris -->
    <div v-if="ghost" class="ghost" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
      <Sprite :id="ghost.species" :shiny="ghost.shiny" :size="72" />
      <span v-if="ghost.copy" class="sv-chip on">Copier</span>
    </div>
  </div>
</template>

<style scoped>
.bank {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 320px;
  gap: var(--sp-4);
  height: 100%;
  min-height: 0;
}

.bank.solo {
  grid-template-columns: minmax(0, 1fr) 340px;
}

.col {
  display: flex;
  flex-direction: column;
  gap: var(--sp-2);
  min-width: 0;
  min-height: 0;
  padding: var(--sp-3) var(--sp-4);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-height: 34px;
}

.save-title {
  min-width: 0;
}

.save-title span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.count {
  margin-left: auto;
  color: var(--text-dim);
  font-size: var(--fs-md);
  white-space: nowrap;
}

.fill-area {
  flex: 1;
  max-width: none;
}

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-1);
}

.filters .sep {
  width: var(--sp-2);
}

.box-head {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
}

.box-name,
.box-title {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--sp-2);
  flex: 0 1 auto;
  min-width: 0;
  max-width: 220px;
  font-size: var(--fs-xl);
  font-weight: 600;
}

.box-name {
  padding: var(--sp-1) var(--sp-2);
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
}

.box-name span,
.box-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.box-name .icon {
  flex-shrink: 0;
  color: var(--text-dim);
  opacity: 0;
  transition: opacity 0.12s;
}

.box-name:hover,
.box-name:focus-visible {
  border-color: var(--border);
}

.box-name:hover .icon,
.box-name:focus-visible .icon {
  opacity: 1;
}

.rename {
  width: 180px;
  flex: 0 1 auto;
  padding: var(--sp-1) var(--sp-2);
  font-size: var(--fs-lg);
  text-align: center;
}

.spacer {
  flex: 1;
}

.fill {
  color: var(--text-dim);
  font-size: var(--fs-md);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}







/* Grilles : 6 colonnes égales qui tiennent dans le panneau, quelle que soit la largeur. */
.grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: var(--sp-2);
  min-width: 0;
  padding: var(--sp-2);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.grid.box {
  flex: 1;
  grid-template-rows: repeat(5, minmax(0, 1fr));
  min-height: 0;
}

.grid.results {
  flex: 1;
  grid-auto-rows: 64px;
  align-content: start;
  min-height: 0;
  overflow-y: auto;
}

.grid.party {
  flex-shrink: 0;
  grid-auto-rows: 56px;
}

.slot {
  position: relative;
  display: grid;
  place-items: center;
  min-width: 0;
  min-height: 0;
  padding: 0;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--text) 18%, transparent);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--text) 12%, transparent);
  color: var(--text);
  touch-action: none;
  user-select: none;
  transition: transform 0.12s, background 0.12s, box-shadow 0.12s, opacity 0.12s;
}

/* Le sprite rétrécit avec la case au lieu de la pousser. */
.slot :deep(.sprite) {
  max-width: 100%;
  max-height: 100%;
}

.slot:hover:not(.selected) {
  background: color-mix(in srgb, var(--text) 20%, transparent);
}

.slot.selected {
  background: color-mix(in srgb, var(--text) 26%, transparent);
  box-shadow: 0 0 0 3px var(--text);
  z-index: 1;
}

.slot.over {
  box-shadow: 0 0 0 3px var(--accent-2);
  transform: scale(1.04);
}

.slot.blocked {
  opacity: 0.45;
}

.slot .shiny {
  position: absolute;
  top: var(--sp-1);
  left: var(--sp-1);
  color: var(--shiny);
  fill: var(--shiny);
}

.slot .egg {
  position: absolute;
  top: var(--sp-1);
  right: var(--sp-1);
  color: var(--text);
}

.slot .fmt {
  position: absolute;
  right: var(--sp-1);
  bottom: 7px;
  padding: 0 var(--sp-1);
  border-radius: var(--radius-xs);
  background: color-mix(in srgb, var(--bg) 65%, transparent);
  color: var(--text);
  font-size: var(--fs-xs);
  font-weight: 700;
  line-height: 1.3;
}

.bar {
  position: absolute;
  bottom: 3px;
  left: 18%;
  right: 18%;
  height: 3px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.bar.male {
  background: var(--male);
}

.bar.female {
  background: var(--female);
}

.path {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
  min-width: 0;
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.path > .icon {
  flex-shrink: 0;
}

.path-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}


.side {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  min-width: 0;
  min-height: 0;
  padding: var(--sp-4);
  overflow-y: auto;
}

.detail-head {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.detail-id {
  min-width: 0;
}

.detail-head h3 {
  font-size: var(--fs-xl);
  overflow-wrap: anywhere;
}

.gender {
  color: var(--accent-2);
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-1);
  margin-top: var(--sp-1);
}

.compat {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-2) var(--sp-3);
  border-radius: var(--radius-card);
  font-size: var(--fs-md);
}

.compat strong {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.compat p,
.compat ul {
  margin: 0;
}

.compat ul {
  padding-left: var(--sp-4);
}

.compat.ok {
  background: color-mix(in srgb, var(--ok) 18%, transparent);
}

.compat.warn {
  background: var(--warn-bg);
  color: var(--warn);
}

.compat.bad {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
}

.changes {
  color: var(--text-dim);
  font-size: var(--fs-sm);
}

.ellipsis {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(84px, 1fr));
  gap: var(--sp-2);
}

.actions .sv-btn {
  padding-inline: var(--sp-2);
}

.strip-list {
  margin: var(--sp-3) 0 0;
  padding-left: var(--sp-5);
  font-size: var(--fs-md);
}

.ghost {
  position: fixed;
  z-index: 60;
  display: flex;
  flex-direction: column;
  align-items: center;
  pointer-events: none;
  transform: translate(-50%, -60%) scale(1.1);
  filter: drop-shadow(0 8px 12px color-mix(in srgb, var(--bg) 70%, transparent));
}
</style>
