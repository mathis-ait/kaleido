<script setup lang="ts">
import { BANK_TERMS } from "./bank/terms";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import Icon from "../components/Icon.vue";
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

const TIPS = BANK_TERMS;

const view = computed(() => saveState.view);
const info = computed(() => bankState.info);
const boxCount = computed(() => info.value?.boxes.length ?? 0);
const bankBox = computed(() => info.value?.boxes[bankState.box]);
const detail = computed(() => bankState.detail);
const compat = computed(() => detail.value?.compatibility ?? null);
const partySlots = computed(() => Array.from({ length: 6 }, (_, i) => view.value?.party[i] ?? null));
const bslot = (index: number): BankSlot => ({ box: bankState.box, index });
const boxSlot = (index: number): Slot => ({ kind: "box", box: saveState.box, index });
const partySlot = (index: number): Slot => ({ kind: "party", index });
const bkey = (s: BankSlot) => `b:${s.box}:${s.index}`;
const skey = (s: Slot) => `s:${JSON.stringify(s)}`;

/** Pokémon de la sauvegarde sélectionné (pour le déposer avec un bouton). */
const saveSel = ref<SlotView | null>(null);

onMounted(async () => {
  // Capture : passe avant la page (Échap ferme d'abord la fenêtre de confirmation).
  window.addEventListener("keydown", onKey, true);
  await loadBankInfo();
  await loadBankBox(Math.min(bankState.box, Math.max(0, boxCount.value - 1)));
  if (view.value && !saveState.slots.length) await loadBox(saveState.box);
  if (bankState.selected) await selectBank(bankState.selected);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey, true));

function changeBankBox(step: number) {
  const n = boxCount.value;
  if (n) loadBankBox((bankState.box + step + n) % n);
}

function changeSaveBox(step: number) {
  const n = view.value?.boxNames.length ?? 0;
  if (n) loadBox((saveState.box + step + n) % n);
}

// ---- Renommer une boîte de la banque (double-clic)
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

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement;
  if (["INPUT", "TEXTAREA"].includes(t?.tagName) || e.ctrlKey) return;
  if (e.key === "PageUp" || e.key === "PageDown") {
    e.preventDefault();
    changeBankBox(e.key === "PageUp" ? -1 : 1);
  } else if (e.key === "Escape" && stripAsk.value) {
    e.preventDefault();
    stripAsk.value = null;
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
    <section class="panel col">
      <header class="head">
        <h3 class="kicker">Banque Kaleido <Tip v-bind="TIPS.bank" /></h3>
        <span class="count">{{ info?.count ?? 0 }} Pokémon</span>
        <button class="mini" title="Importer des fichiers .pk4 à .pk7 (I)" @click="importFiles"><Icon name="download" :size="14" /> Importer</button>
      </header>
      <div class="searchbar">
        <Icon name="search" :size="15" />
        <input v-model="search" class="sv-input" placeholder="Chercher (espèce, surnom, dresseur)…" />
        <button class="chip" :class="{ on: shinyOnly }" title="Chromatiques seulement" @click="shinyOnly = !shinyOnly">★</button>
        <button v-for="g in GENS" :key="g" class="chip" :class="{ on: genFilter === g }" :title="`Format PK${g}`" @click="genFilter = genFilter === g ? null : g">
          PK{{ g }}
        </button>
      </div>

      <template v-if="searching">
        <p class="dim small">{{ results.length }} résultat{{ results.length > 1 ? "s" : "" }} dans toute la banque</p>
        <div class="grid results">
          <button
            v-for="r in results"
            :key="r.id"
            class="slot filled"
            :class="{ selected: sameBankSlot(bankState.selected, r.slot), over: dropTarget === bkey(r.slot) }"
            :data-bslot="JSON.stringify(r.slot)"
            :title="`${r.nickname || r.speciesName} · N. ${r.level} · ${info?.boxes[r.slot.box]?.name} case ${r.slot.index + 1}`"
            @pointerdown="onPointerDown($event, { kind: 'bank', slot: r.slot }, r)"
            @click="pickBank(r.slot, r)"
          >
            <Icon v-if="r.shiny" name="sparkle" :size="13" class="shiny" />
            <Sprite :id="r.species" :shiny="r.shiny" :size="52" />
            <span class="fmt">{{ r.format }}</span>
          </button>
        </div>
      </template>

      <template v-else>
        <div class="box-head">
          <button class="round" aria-label="Boîte précédente (Pg↑)" @click="changeBankBox(-1)"><Icon name="chevron-left" /></button>
          <input
            v-if="renaming"
            ref="renameInput"
            v-model="newName"
            class="rename"
            maxlength="40"
            @keydown.enter.prevent="commitRename"
            @keydown.esc.stop.prevent="renaming = false"
            @blur="commitRename"
          />
          <h2 v-else title="Double-clic pour renommer" @dblclick="startRename">{{ bankBox?.name ?? "…" }}</h2>
          <button class="round" aria-label="Boîte suivante (Pg↓)" @click="changeBankBox(1)"><Icon name="chevron-right" /></button>
          <span class="fill">{{ bankBox?.count ?? 0 }}/30</span>
          <span class="spacer" />
          <button class="mini" title="Ajouter une boîte" @click="bankAddBox"><Icon name="plus" :size="14" /></button>
          <button class="mini" title="Supprimer cette boîte (vide)" :disabled="!!bankBox?.count || boxCount <= 1" @click="bankDeleteBox(bankState.box)">
            <Icon name="trash" :size="14" />
          </button>
        </div>
        <div class="dots">
          <button
            v-for="(b, i) in info?.boxes ?? []"
            :key="i"
            :class="{ on: i === bankState.box, full: b.count === 30, empty: !b.count }"
            :title="`${b.name} (${b.count}/30)`"
            :aria-label="b.name"
            @click="loadBankBox(i)"
          />
        </div>
        <div class="grid">
          <button
            v-for="(p, i) in bankState.slots"
            :key="i"
            class="slot"
            :class="{
              selected: sameBankSlot(bankState.selected, bslot(i)),
              over: dropTarget === bkey(bslot(i)),
              filled: !!p,
              blocked: !!p && view && bankState.compat[i] === false,
            }"
            :data-bslot="JSON.stringify(bslot(i))"
            :title="p ? `${p.nickname || p.speciesName} · N. ${p.level} · ${p.format}` : 'Case vide'"
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
      <p class="dim small path" :title="info?.path">
        <Icon name="folder" :size="13" /> {{ info?.path }}
        <button class="link" @click="chooseFolder">Changer</button>
        <button v-if="info && info.path !== info.defaultPath" class="link" @click="bankSetPath(null)">Par défaut</button>
        <button v-if="info" class="link" @click="openPath(info.path)">Ouvrir</button>
        <Tip v-bind="TIPS.trash" />
      </p>
    </section>

    <!-- Sauvegarde ouverte -->
    <section v-if="view" class="panel col">
      <header class="head">
        <h3 class="kicker">{{ view.game.replace("Pokémon ", "") }} · {{ view.trainer.name }} <Tip v-bind="TIPS.drag" /></h3>
      </header>
      <div class="box-head">
        <button class="round" aria-label="Boîte précédente" @click="changeSaveBox(-1)"><Icon name="chevron-left" /></button>
        <h2>{{ view.boxNames[saveState.box] }}</h2>
        <button class="round" aria-label="Boîte suivante" @click="changeSaveBox(1)"><Icon name="chevron-right" /></button>
        <span class="fill">{{ view.boxFill[saveState.box] }}/30</span>
      </div>
      <div class="grid">
        <button
          v-for="(p, i) in saveState.slots"
          :key="i"
          class="slot"
          :class="{ selected: !!p && !!saveSel && sameSlot(saveSel.slot, boxSlot(i)), over: dropTarget === skey(boxSlot(i)), filled: !!p }"
          :data-sslot="JSON.stringify(boxSlot(i))"
          :title="p ? `${p.nickname || p.speciesName} · N. ${p.level}` : 'Case vide'"
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
      <div class="party">
        <button
          v-for="(p, i) in partySlots"
          :key="i"
          class="slot"
          :class="{ selected: !!p && !!saveSel && sameSlot(saveSel.slot, partySlot(i)), over: dropTarget === skey(partySlot(i)), filled: !!p }"
          :data-sslot="JSON.stringify(partySlot(i))"
          :title="p ? `${p.nickname || p.speciesName} · N. ${p.level} (équipe)` : 'Équipe : place libre'"
          @pointerdown="onPointerDown($event, { kind: 'save', slot: partySlot(i) }, p)"
          @click="pickSave(p)"
        >
          <Sprite v-if="p" :id="p.species" :shiny="p.shiny" :size="44" />
        </button>
      </div>
    </section>

    <!-- Fiche -->
    <aside class="panel side">
      <template v-if="detail">
        <div class="detail-head">
          <Sprite :id="detail.species" :shiny="detail.shiny" :size="88" />
          <div>
            <h3>{{ detail.nickname || detail.speciesName }} <span class="gender">{{ genderSymbol(detail.pokemon.gender) }}</span></h3>
            <div class="chips">
              <span class="chip-w">N. {{ detail.level }}</span>
              <span class="chip-fmt">{{ detail.format }} <Tip v-bind="TIPS.format" /></span>
              <span v-if="detail.shiny" class="chip-gold">★ Chromatique</span>
              <span v-if="detail.isEgg" class="chip-soft">Œuf</span>
              <span v-if="detail.pokemon.isNicknamed" class="chip-soft">{{ detail.speciesName }}</span>
            </div>
          </div>
        </div>

        <!-- Compatibilité avec la sauvegarde ouverte -->
        <div v-if="compat" class="compat" :class="compat.ok ? 'ok' : compat.fixable ? 'warn' : 'bad'">
          <template v-if="compat.blocker">
            <strong><Icon name="alert" :size="15" /> Incompatible avec cette sauvegarde <Tip v-bind="TIPS.direction" /></strong>
            <p>{{ compat.blocker }}</p>
          </template>
          <template v-else-if="compat.ok">
            <strong><Icon name="check" :size="15" /> Compatible avec cette sauvegarde</strong>
            <p v-if="compat.converts">Il sera converti : {{ compat.from.replace("gen", "PK") }} → {{ compat.to.replace("gen", "PK") }}. <Tip v-bind="TIPS.changes" /></p>
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
        <p v-else-if="!view" class="dim small">Ouvre une sauvegarde pour savoir si ce Pokémon peut y aller. <Tip v-bind="TIPS.direction" /></p>

        <dl>
          <dt>Origine</dt>
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
            <dt>Dresseur actuel <Tip v-bind="TIPS.handler" /></dt>
            <dd>{{ detail.handler }}</dd>
          </template>
          <dt>Nature</dt>
          <dd>{{ NATURES[detail.pokemon.nature] ?? detail.pokemon.natureName }}</dd>
          <dt>Talent</dt>
          <dd>{{ detail.pokemon.abilityName }}</dd>
          <dt>Objet</dt>
          <dd>{{ detail.pokemon.itemName ?? "—" }}</dd>
          <dt>Rencontre</dt>
          <dd>{{ detail.pokemon.metLocationName ?? `Lieu n°${detail.pokemon.metLocation}` }}</dd>
        </dl>
        <ul class="moves">
          <li v-for="m in detail.pokemon.moveNames" :key="m">{{ m }}</li>
        </ul>
        <button v-if="view" class="btn-big" :disabled="!compat || !!compat.blocker || (!compat.ok && !compat.fixable)" @click="sendSelected(false)">
          <Icon name="upload" :size="16" /> Envoyer dans la sauvegarde
        </button>
        <div class="row">
          <button v-if="view" class="btn-line" :disabled="!compat || !!compat.blocker || (!compat.ok && !compat.fixable)" title="L'original reste dans la banque" @click="sendSelected(true)">
            <Icon name="copy" :size="15" /> Copier
          </button>
          <button class="btn-line" @click="exportSelected"><Icon name="file" :size="15" /> Exporter</button>
          <button class="btn-line" :class="{ danger: confirmDelete }" @click="doDelete">
            <Icon name="trash" :size="15" /> {{ confirmDelete ? "Confirmer" : "Supprimer" }}
          </button>
        </div>
      </template>

      <template v-else-if="saveSel">
        <div class="detail-head">
          <Sprite :id="saveSel.species" :shiny="saveSel.shiny" :size="88" />
          <div>
            <h3>{{ saveSel.nickname || saveSel.speciesName }}</h3>
            <div class="chips">
              <span class="chip-w">N. {{ saveSel.level }}</span>
              <span class="chip-fmt">PK{{ view?.generation }}</span>
            </div>
          </div>
        </div>
        <p class="dim small">Ranger ce Pokémon dans la banque pour le garder à l'abri ou l'envoyer plus tard dans un autre jeu (même génération ou plus récente).</p>
        <button class="btn-big" @click="depositSelected(false)"><Icon name="download" :size="16" /> Déposer dans la banque</button>
        <button class="btn-line" title="Il reste aussi dans la sauvegarde" @click="depositSelected(true)"><Icon name="copy" :size="15" /> Déposer une copie</button>
      </template>

      <div v-else class="empty-help">
        <Icon name="bank" :size="40" />
        <p>
          La banque garde tes Pokémon hors de tes sauvegardes, pour les faire passer d'un jeu à l'autre.
          <Tip v-bind="TIPS.bank" />
        </p>
        <p class="dim small">Transferts vers une génération égale ou plus récente seulement. <Tip v-bind="TIPS.direction" /></p>
        <template v-if="!view">
          <button class="btn-big" @click="emit('open')"><Icon name="folder-open" :size="16" /> Ouvrir une sauvegarde</button>
        </template>
        <p v-else class="dim small">Glisse un Pokémon d'un côté à l'autre · Maj : copier <Tip v-bind="TIPS.drag" /></p>
      </div>
    </aside>

    <!-- Confirmation : retirer les attaques / l'objet absents du jeu -->
    <div v-if="stripAsk" class="modal-back" @click.self="stripAsk = null">
      <div class="modal panel" role="dialog" aria-modal="true">
        <h3>Envoyer quand même ?</h3>
        <p class="dim">Ces éléments n'existent pas dans ce jeu. Kaleido peut les retirer du Pokémon envoyé (l'exemplaire de la banque n'est pas modifié s'il s'agit d'une copie) :</p>
        <ul>
          <li v-for="pb in stripAsk.problems" :key="pb.kind + pb.id">{{ pb.message }}</li>
        </ul>
        <div class="row">
          <button class="btn-line" @click="stripAsk = null">Annuler</button>
          <button class="btn-big" @click="confirmStrip">Retirer et envoyer</button>
        </div>
      </div>
    </div>

    <!-- Sprite qui suit la souris -->
    <div v-if="ghost" class="ghost" :style="{ left: `${ghost.x}px`, top: `${ghost.y}px` }">
      <Sprite :id="ghost.species" :shiny="ghost.shiny" :size="72" />
      <span v-if="ghost.copy" class="mode">Copier</span>
    </div>
  </div>
</template>

<style scoped>
.bank {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 320px;
  gap: 16px;
  height: 100%;
}

.bank.solo {
  grid-template-columns: minmax(0, 1fr) 340px;
}

.panel {
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--panel);
  backdrop-filter: blur(14px);
  box-shadow: var(--shadow);
}

.col {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 14px 16px;
}

.head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.kicker {
  display: flex;
  align-items: center;
  gap: 2px;
  color: var(--text-dim);
  font-size: 12px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.count {
  margin-left: auto;
  color: var(--text-dim);
  font-size: 13px;
}

.mini {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  font-size: 12px;
  font-weight: 600;
}

.mini:disabled {
  opacity: 0.35;
}

.searchbar {
  display: flex;
  align-items: center;
  gap: 6px;
}

.searchbar .sv-input {
  flex: 1;
  min-width: 0;
  padding: 6px 10px;
}

.chip {
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
}

.chip.on {
  background: var(--text);
  color: var(--bg);
}

.box-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.box-head h2 {
  min-width: 120px;
  font-size: 19px;
  font-weight: 600;
  text-align: center;
  cursor: text;
}

.spacer {
  flex: 1;
}

.rename {
  width: 160px;
  padding: 5px 10px;
  border: 1px solid var(--accent-2);
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  color: var(--text);
  font: 600 16px var(--font);
  text-align: center;
  outline: none;
}

.round {
  display: grid;
  place-items: center;
  width: 36px;
  height: 30px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.fill {
  color: var(--text-dim);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.dots {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
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
  width: 13px;
  height: 13px;
  background: var(--text);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 60%, transparent);
}

.grid {
  display: grid;
  flex: 1;
  grid-template-columns: repeat(6, 1fr);
  grid-template-rows: repeat(5, 1fr);
  gap: 7px;
  min-height: 0;
  padding: 10px;
  border-radius: 14px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.grid.results {
  grid-template-rows: none;
  grid-auto-rows: 74px;
  align-content: start;
  overflow-y: auto;
}

.slot {
  position: relative;
  display: grid;
  place-items: center;
  min-height: 0;
  border: 1px solid color-mix(in srgb, var(--text) 18%, transparent);
  border-radius: 11px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  touch-action: none;
  user-select: none;
  transition: transform 0.12s, background 0.12s, box-shadow 0.12s, opacity 0.12s;
}

.slot:hover {
  background: color-mix(in srgb, var(--text) 20%, transparent);
}

.slot.selected {
  background: color-mix(in srgb, var(--text) 26%, transparent);
  box-shadow: 0 0 0 3px #fff, 0 0 0 6px color-mix(in srgb, var(--accent-2) 70%, transparent);
  z-index: 1;
}

.slot.over {
  box-shadow: 0 0 0 3px var(--accent-2);
  transform: scale(1.05);
}

.slot.blocked {
  opacity: 0.45;
}

.slot .shiny {
  position: absolute;
  top: 4px;
  left: 4px;
  color: #ff5a7a;
  fill: #ff5a7a;
}

.slot .egg {
  position: absolute;
  top: 4px;
  right: 4px;
  color: #fff1c4;
}

.slot .fmt {
  position: absolute;
  right: 4px;
  bottom: 7px;
  padding: 0 4px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.35);
  font-size: 9px;
  font-weight: 700;
}

.bar {
  position: absolute;
  bottom: 3px;
  left: 18%;
  right: 18%;
  height: 3px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.bar.male {
  background: #5aa9ff;
}

.bar.female {
  background: #ff7eb6;
}

.party {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 7px;
}

.party .slot {
  height: 58px;
}

.path {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  white-space: nowrap;
}

.link {
  border: none;
  background: none;
  color: var(--accent-2);
  font-size: 12px;
  font-weight: 600;
  text-decoration: underline;
}

.side {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding: 16px;
  overflow-y: auto;
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

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 4px;
}

.chip-w,
.chip-gold,
.chip-soft,
.chip-fmt {
  display: inline-flex;
  align-items: center;
  padding: 2px 9px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}

.chip-w {
  background: var(--text);
  color: var(--bg);
}

.chip-fmt {
  background: color-mix(in srgb, var(--accent-2) 35%, transparent);
}

.chip-gold {
  background: #ffe27a;
  color: #6b4b00;
}

.chip-soft {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

.compat {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border-radius: 12px;
  font-size: 13px;
}

.compat strong {
  display: flex;
  align-items: center;
  gap: 6px;
}

.compat p,
.compat ul {
  margin: 0;
}

.compat ul {
  padding-left: 18px;
}

.compat.ok {
  background: color-mix(in srgb, #2fc27a 18%, transparent);
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
  font-size: 12px;
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

.ellipsis {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  padding: 5px 9px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  font-size: 12px;
  font-weight: 600;
}

.btn-big {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 10px;
  border: none;
  border-radius: 999px;
  background: var(--text);
  color: var(--bg);
  font-weight: 700;
}

.btn-big:disabled {
  opacity: 0.4;
}

.row {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(80px, 1fr));
  gap: 8px;
}

.btn-line {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 10px;
  border: 1.5px solid color-mix(in srgb, var(--text) 70%, transparent);
  border-radius: 999px;
  background: transparent;
  font-weight: 600;
  font-size: 13px;
}

.btn-line:disabled {
  opacity: 0.4;
}

.btn-line.danger {
  border-color: var(--danger);
  color: var(--danger);
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
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 24px 12px;
  color: var(--text-dim);
  text-align: center;
}

.empty-help p {
  margin: 0;
}

.modal-back {
  position: fixed;
  inset: 0;
  z-index: 70;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.45);
}

.modal {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: min(440px, calc(100vw - 32px));
  padding: 20px;
  background: var(--bg);
}

.modal ul {
  margin: 0;
  padding-left: 20px;
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
