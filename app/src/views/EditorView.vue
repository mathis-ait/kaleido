<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Banner from "../components/Banner.vue";
import Combo, { type ComboOption } from "../components/Combo.vue";
import Icon from "../components/Icon.vue";
import SearchField from "../components/SearchField.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import StatRadar from "../components/StatRadar.vue";
import Tip from "../components/Tip.vue";
import TypeBadge from "../components/TypeBadge.vue";
import { closeRom, editCount, editor, openRom, speciesEdit } from "../editor";
import { addPaths, library } from "../library";
import { nav } from "../nav";
import { isRom, romExt, type BaseStats, type CtrOutput, type PokeTypeKey, type SpeciesEdit, type TypeTag } from "../types";

const STATS: { key: keyof BaseStats; label: string }[] = [
  { key: "hp", label: "PV" },
  { key: "attack", label: "Attaque" },
  { key: "defense", label: "Défense" },
  { key: "spAttack", label: "Atq. Spé." },
  { key: "spDefense", label: "Déf. Spé." },
  { key: "speed", label: "Vitesse" },
];

const openableRoms = computed(() => library.items.filter(isRom));
const readOnly = computed(() => !editor.data);
/** Jeu 3DS : la sauvegarde produit un mod LayeredFS et/ou une ROM .3ds reconstruite. */
const isCtr = computed(() => editor.overview?.game.platform === "3ds");
/** Niveau minimal d'une attaque apprise : 0 (apprise à l'évolution) à partir de la Gen 7. */
const minLevel = computed(() => ((editor.overview?.game.generation ?? 0) >= 7 ? 0 : 1));

/** Recherche insensible à la casse et aux accents. */
const normalize = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

const typeTag = (index: number) => editor.data?.types.find((t) => t.index === index)?.tag;
const abilityName = (id: number) => editor.data?.abilities.find((a) => a.id === id)?.name;

const abilityOptions = computed<ComboOption[]>(() => (editor.data?.abilities ?? []).map((a) => ({ value: a.id, label: a.name, hint: `${a.id}` })));
const moveOptions = computed<ComboOption[]>(() => (editor.data?.moves ?? []).map((m) => ({ value: m.id, label: m.name, hint: `${m.id}` })));

// ---------- Liste ----------

const query = ref("");
const typeFilter = ref<PokeTypeKey | "">("");
const onlyModified = ref(false);
type SortKey = "id" | "name" | "total";
const sortKey = ref<SortKey>("id");

interface Row {
  id: number;
  name: string;
  types: TypeTag[];
  total: number;
  abilities: string[];
  modified: boolean;
}

const allRows = computed<Row[]>(() =>
  (editor.overview?.species ?? []).map((s) => {
    const d = editor.edits[s.id];
    if (!d) return { id: s.id, name: s.name, types: s.types, total: s.total, abilities: [...s.abilities, s.hiddenAbility ?? ""], modified: false };
    return {
      id: s.id,
      name: s.name,
      types: [...new Set(d.types)].map(typeTag).filter((t): t is TypeTag => !!t),
      total: d.stats.reduce((a, b) => a + b, 0),
      abilities: d.abilities.map((a) => abilityName(a) ?? ""),
      modified: true,
    };
  }),
);

const types = computed(() => {
  const seen = new Map<PokeTypeKey, string>();
  for (const r of allRows.value) for (const t of r.types) seen.set(t.key, t.name);
  return [...seen].sort((a, b) => a[1].localeCompare(b[1], "fr"));
});

const rows = computed(() => {
  const q = normalize(query.value.trim());
  const list = allRows.value.filter(
    (r) =>
      (!q || normalize(r.name).includes(q) || String(r.id) === q || r.abilities.some((a) => normalize(a).includes(q))) &&
      (!typeFilter.value || r.types.some((t) => t.key === typeFilter.value)) &&
      (!onlyModified.value || r.modified),
  );
  const by = sortKey.value;
  return [...list].sort((a, b) => (by === "name" ? a.name.localeCompare(b.name, "fr") : by === "total" ? b.total - a.total : 0) || a.id - b.id);
});

const selectedId = ref(1);
const selected = computed(() => editor.overview?.species.find((s) => s.id === selectedId.value) ?? editor.overview?.species[0]);
const listEl = ref<HTMLElement | null>(null);

function select(id: number) {
  selectedId.value = id;
  nextTick(() => listEl.value?.querySelector<HTMLElement>(".item.on")?.scrollIntoView({ block: "nearest" }));
}

/** Pokémon voisin dans la liste filtrée et triée. */
const neighbour = (step: number) => {
  const i = rows.value.findIndex((r) => r.id === selectedId.value);
  return i < 0 ? null : (rows.value[i + step] ?? null);
};

watch(
  () => editor.overview?.path,
  () => (selectedId.value = 1),
);

// ---------- Fiche ----------

const original = computed(() => editor.data?.species[selectedId.value - 1]);
const cur = computed(() => speciesEdit(selectedId.value));
const modified = computed(() => !!editor.edits[selectedId.value]);

/** Valeurs affichées (statistiques et types) même en lecture seule. */
const shownStats = computed<number[]>(() => {
  if (cur.value) return cur.value.stats;
  const b = selected.value?.baseStats;
  return b ? STATS.map((s) => b[s.key]) : [0, 0, 0, 0, 0, 0];
});
const radarStats = computed<BaseStats>(() => Object.fromEntries(STATS.map((s, i) => [s.key, shownStats.value[i]])) as unknown as BaseStats);
const total = computed(() => shownStats.value.reduce((a, b) => a + b, 0));
const originalTotal = computed(() => original.value?.stats.reduce((a, b) => a + b, 0) ?? total.value);
const shownTypes = computed(() => allRows.value.find((r) => r.id === selectedId.value)?.types ?? []);

const same = (a: SpeciesEdit, b: SpeciesEdit) => JSON.stringify(a) === JSON.stringify(b);

/** Modifie l'espèce affichée ; revenir aux valeurs d'origine efface la modification. */
function mutate(fn: (d: SpeciesEdit) => void) {
  const base = cur.value;
  const orig = original.value;
  if (!base || !orig || readOnly.value) return;
  const d: SpeciesEdit = JSON.parse(JSON.stringify(base));
  fn(d);
  if (same(d, orig)) delete editor.edits[d.id];
  else editor.edits[d.id] = d;
}

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, Math.round(v) || min));
const numberOf = (e: Event) => Number((e.target as HTMLInputElement).value);

function setStat(i: number, e: Event) {
  mutate((d) => (d.stats[i] = clamp(numberOf(e), 1, 255)));
}

/** Types proposés : « ??? » (Gen 4, inutilisé par les espèces) seulement s'il est déjà en place. */
const typeOptions = (current: number) => (editor.data?.types ?? []).filter((t) => t.tag.key !== "mystery" || t.index === current);

const secondType = computed(() => (cur.value && cur.value.types[1] !== cur.value.types[0] ? cur.value.types[1] : -1));

function setType(slot: 0 | 1, e: Event) {
  const v = Number((e.target as HTMLSelectElement).value);
  mutate((d) => {
    if (slot === 0) {
      const mono = d.types[0] === d.types[1];
      d.types = [v, mono ? v : d.types[1]];
    } else {
      d.types[1] = v < 0 ? d.types[0] : v;
    }
  });
}

// Un second talent identique au premier équivaut à « aucun » : il suit le premier.
function setAbility(slot: number, v: number) {
  mutate((d) => {
    if (slot === 0 && d.abilities[1] === d.abilities[0]) d.abilities[1] = v;
    d.abilities[slot] = v;
  });
}

function setCatchRate(e: Event) {
  mutate((d) => (d.catchRate = clamp(numberOf(e), 1, 255)));
}

const byLevel = (d: SpeciesEdit) => d.learnset.sort((a, b) => a.level - b.level);

function setLevel(i: number, e: Event) {
  mutate((d) => {
    d.learnset[i].level = clamp(numberOf(e), minLevel.value, 100);
    byLevel(d);
  });
}

function setMove(i: number, v: number) {
  mutate((d) => (d.learnset[i].move = v));
}

function removeMove(i: number) {
  mutate((d) => d.learnset.splice(i, 1));
}

function addMove() {
  mutate((d) => {
    const last = d.learnset[d.learnset.length - 1];
    d.learnset.push({ level: last ? Math.min(100, last.level + 1) : 1, move: editor.data?.moves[0]?.id ?? 1 });
  });
}

function revert() {
  delete editor.edits[selectedId.value];
}

function revertAll() {
  if (confirm(`Annuler les modifications des ${editCount.value} Pokémon ?`)) editor.edits = {};
}

// ---------- Enregistrement ----------

const saving = ref(false);
const saveError = ref<string | null>(null);
const saved = ref<{ path: string; count: number } | null>(null);

/** 3DS : format de sortie et destination du dossier LayeredFS. */
const ctrOutput = ref<CtrOutput>("layered_fs");
const ctrTarget = ref<"luma" | "emulator">("luma");
/** 3DS : fichiers produits par le dernier enregistrement. */
const ctrSaved = ref<{ count: number; romfs: string | null; image: string | null; reopened: boolean } | null>(null);

watch(
  () => editor.overview?.path,
  (path) => {
    if (path !== ctrSaved.value?.image) ctrSaved.value = null;
  },
);

async function saveRom() {
  const src = editor.overview?.path;
  if (!src || !editCount.value) return;
  if (isCtr.value) return saveCtr(src);
  const output = await save({
    title: "Enregistrer la ROM modifiée",
    defaultPath: `${src.replace(/\.(nds|gba|gbc|gb)$/i, "")} - modifiée.${romExt(src)}`,
    filters: [{ name: romExt(src) === "nds" ? "ROM Nintendo DS" : romExt(src) === "gba" ? "ROM Game Boy Advance" : "ROM Game Boy", extensions: [romExt(src)] }],
  });
  if (!output) return;
  saving.value = true;
  saveError.value = null;
  try {
    const count = await invoke<number>("rom_editor_save", { path: src, edits: Object.values(editor.edits), output });
    const keep = selectedId.value;
    await addPaths([output]);
    // La suite de l'édition se fait sur la copie qui vient d'être écrite (sinon les
    // modifications restent en cours, pour un nouvel essai).
    if (await openRom(output, true)) {
      selectedId.value = keep;
      saved.value = { path: output, count };
    } else {
      saveError.value = `ROM enregistrée (${output}), mais impossible de la rouvrir : ${editor.error ?? "erreur inconnue"}`;
    }
  } catch (e) {
    saveError.value = String(e);
  } finally {
    saving.value = false;
  }
}

/**
 * 3DS : écrit le mod LayeredFS et/ou la ROM reconstruite dans un dossier. Avec une ROM,
 * la suite de l'édition se fait sur elle ; avec LayeredFS seul, les modifications restent
 * en cours (un nouvel enregistrement réécrit le mod complet).
 */
async function saveCtr(src: string) {
  const output = await open({ directory: true, title: ctrOutput.value === "layered_fs" ? "Choisis le dossier où créer le mod" : "Choisis le dossier où créer la ROM" });
  if (typeof output !== "string") return;
  saving.value = true;
  saveError.value = null;
  saved.value = null;
  ctrSaved.value = null;
  try {
    const res = await invoke<{ count: number; romfs: string | null; image: string | null }>("rom_editor_save_ctr", {
      path: src,
      edits: Object.values(editor.edits),
      output,
      format: ctrOutput.value,
      target: ctrTarget.value,
    });
    let reopened = false;
    if (res.image) {
      const keep = selectedId.value;
      await addPaths([res.image]);
      reopened = await openRom(res.image, true);
      if (reopened) selectedId.value = keep;
      else saveError.value = `ROM enregistrée (${res.image}), mais impossible de la rouvrir : ${editor.error ?? "erreur inconnue"}`;
    }
    ctrSaved.value = { ...res, reopened };
  } catch (e) {
    saveError.value = String(e);
  } finally {
    saving.value = false;
  }
}

</script>

<template>
  <section class="editor">
    <!-- Aucune ROM ouverte -->
    <template v-if="!editor.overview && !editor.loadingPath">
      <h1>Éditeur de ROM</h1>
      <p class="lead">Modifie les Pokémon d'un jeu : statistiques, types, talents, taux de capture et attaques apprises.</p>

      <Banner v-if="editor.error" class="msg" :dismiss="() => (editor.error = null)">{{ editor.error }}</Banner>

      <div v-if="openableRoms.length" class="picker">
        <button v-for="rom in openableRoms" :key="rom.path" class="pick panel" @click="openRom(rom.path)">
          <span class="chip chip-accent">Gen {{ rom.generation }}</span>
          <strong>{{ rom.title }}</strong>
          <small>{{ rom.fileName }}</small>
        </button>
      </div>
      <div v-else class="empty panel">
        <p>Aucune ROM DS ou 3DS dans la bibliothèque.</p>
        <button class="btn btn-primary" @click="nav.view = 'home'">Ajouter une ROM</button>
      </div>
    </template>

    <!-- Chargement -->
    <div v-else-if="editor.loadingPath" class="loading">
      <Icon name="refresh" :size="32" class="sv-spin" />
      <p>Lecture de la ROM…</p>
    </div>

    <template v-else-if="editor.overview">
      <header class="head">
        <div>
          <div class="chips">
            <span class="chip chip-accent">Gen {{ editor.overview.game.generation }}</span>
            <span class="chip">{{ editor.overview.gameCode }}</span>
            <span v-if="!editor.overview.verified" class="chip warn" title="Emplacements des données pas encore vérifiés sur ce jeu">Non vérifié</span>
            <span v-if="readOnly" class="chip">Lecture seule</span>
          </div>
          <h1>{{ editor.overview.game.name }}</h1>
        </div>
        <div class="actions">
          <button class="btn" @click="closeRom">Changer de ROM</button>
          <button v-if="editCount" class="btn" @click="revertAll">Tout annuler</button>
          <button class="btn btn-primary" :disabled="!editCount || saving" @click="saveRom">
            {{ saving ? "Enregistrement…" : !isCtr ? "Enregistrer la ROM" : ctrOutput === "layered_fs" ? "Enregistrer le mod 3DS" : "Enregistrer la ROM 3DS" }}
            <span v-if="editCount" class="count-badge">{{ editCount }}</span>
          </button>
        </div>
      </header>

      <Banner v-if="readOnly" tone="warn" class="msg">
        L'édition de ce jeu n'est pas encore disponible : seuls Diamant, Perle, Platine, HeartGold, SoulSilver, Noire, Blanche et, sur 3DS, X, Y,
        Rubis Oméga, Saphir Alpha, Soleil, Lune, Ultra-Soleil et Ultra-Lune sont modifiables pour l'instant. Tu peux toujours consulter son Pokédex.
      </Banner>
      <Banner v-if="saveError" class="msg" :dismiss="() => (saveError = null)">{{ saveError }}</Banner>
      <Banner v-else-if="editor.error" class="msg" :dismiss="() => (editor.error = null)">{{ editor.error }}</Banner>
      <div v-if="saved" class="success">
        <span>
          ROM enregistrée ({{ saved.count }} Pokémon modifié{{ saved.count > 1 ? "s" : "" }}). La suite de l'édition se fait sur cette copie ; l'originale n'a pas été touchée.
        </span>
        <button class="btn" @click="revealItemInDir(saved.path)">Afficher dans le dossier</button>
        <button class="close-x" aria-label="Fermer" @click="saved = null"><Icon name="x" :size="14" /></button>
      </div>
      <div v-if="ctrSaved" class="success">
        <span>
          {{ ctrSaved.count }} Pokémon modifié{{ ctrSaved.count > 1 ? "s" : "" }}, ta ROM d'origine n'a pas été touchée.
          <template v-if="ctrSaved.romfs">
            <br />LayeredFS : <code class="path">{{ ctrSaved.romfs }}</code> —
            {{ ctrTarget === "luma" ? "copie le dossier « luma » à la racine de ta carte SD et active « Game patching » dans Luma3DS." : "place le dossier du title ID dans le dossier « mods » de ton émulateur." }}
          </template>
          <template v-if="ctrSaved.image">
            <br />ROM : <code class="path">{{ ctrSaved.image }}</code> — à ouvrir directement dans Azahar ou Citra.
          </template>
          <br />
          {{
            ctrSaved.reopened
              ? "La suite de l'édition se fait sur cette ROM."
              : "Les modifications restent en cours : un nouvel enregistrement réécrit le mod avec toutes les modifications."
          }}
        </span>
        <button class="btn" @click="revealItemInDir(ctrSaved.image ?? ctrSaved.romfs ?? '')">Afficher dans le dossier</button>
        <button class="close-x" aria-label="Fermer" @click="ctrSaved = null"><Icon name="x" :size="14" /></button>
      </div>

      <div v-if="isCtr && !readOnly" class="ctr-output panel">
        <span class="row-label">
          Sortie 3DS
          <Tip term="outputLayeredFs" />
          <Tip term="outputRom3ds" />
        </span>
        <Segmented
          v-model="ctrOutput"
          :options="[
            { value: 'layered_fs', label: 'LayeredFS', hint: 'Un dossier de mod léger, ta ROM reste intacte' },
            { value: 'rom3ds', label: 'Fichier .3ds', hint: 'Une ROM complète à ouvrir directement dans Azahar/Citra, plus lourde' },
            { value: 'both', label: 'Les deux' },
          ]"
        />
        <template v-if="ctrOutput !== 'rom3ds'">
          <span class="row-label">Dossier pour</span>
          <Segmented
            v-model="ctrTarget"
            :options="[
              { value: 'luma', label: 'Console (Luma3DS)', hint: 'Crée luma/titles/…/romfs : copie le dossier « luma » à la racine de la carte SD' },
              { value: 'emulator', label: 'Émulateur', hint: 'Crée <title ID>/romfs : à placer dans le dossier « mods » de l\'émulateur (Azahar, Citra…)' },
            ]"
          />
        </template>
      </div>

      <div class="workspace">
        <!-- Liste des espèces -->
        <aside class="list panel">
          <div class="filters">
            <SearchField v-model="query" placeholder="Nom, numéro, talent…" />
            <div class="filter-row">
              <select v-model="typeFilter" class="sv-select" aria-label="Type">
                <option value="">Tous les types</option>
                <option v-for="[key, name] in types" :key="key" :value="key">{{ name }}</option>
              </select>
              <select v-model="sortKey" class="sv-select" aria-label="Trier">
                <option value="id">N°</option>
                <option value="name">Nom</option>
                <option value="total">Total</option>
              </select>
            </div>
            <label v-if="editCount" class="only">
              <input v-model="onlyModified" type="checkbox" />
              Modifiés seulement ({{ editCount }})
            </label>
          </div>
          <div ref="listEl" class="items">
            <button v-for="r in rows" :key="r.id" class="item" :class="{ on: r.id === selectedId }" @click="select(r.id)">
              <Sprite :id="r.id" :size="48" class="icon" />
              <span class="item-main">
                <strong>{{ r.name }}</strong>
                <small>#{{ String(r.id).padStart(3, "0") }} · {{ r.types.map((t) => t.name).join(" / ") }}</small>
              </span>
              <span v-if="r.modified" class="dot" title="Modifié" />
              <span class="item-total">{{ r.total }}</span>
            </button>
            <p v-if="!rows.length" class="none">Aucun Pokémon</p>
          </div>
        </aside>

        <!-- Fiche -->
        <div v-if="selected" class="sheet">
          <div class="hero panel">
            <div class="halo">
              <Sprite :id="selected.id" variant="model" :size="140" />
            </div>
            <div class="hero-main">
              <span class="id">#{{ String(selected.id).padStart(3, "0") }}</span>
              <h2>
                {{ selected.name }}
                <span v-if="modified" class="chip chip-mod">Modifié</span>
              </h2>
              <div v-if="cur && editor.data" class="type-pickers">
                <select class="sv-select" :value="cur.types[0]" aria-label="Type 1" @change="setType(0, $event)">
                  <option v-for="t in typeOptions(cur.types[0])" :key="t.index" :value="t.index">{{ t.tag.name }}</option>
                </select>
                <select class="sv-select" :value="secondType" aria-label="Type 2" @change="setType(1, $event)">
                  <option :value="-1">Aucun second type</option>
                  <option v-for="t in typeOptions(cur.types[1]).filter((t) => t.index !== cur!.types[0])" :key="t.index" :value="t.index">{{ t.tag.name }}</option>
                </select>
              </div>
              <div v-else class="types"><TypeBadge v-for="t in shownTypes" :key="t.key" :type="t" /></div>
            </div>
            <div class="hero-nav">
              <button v-if="modified" class="btn" title="Revenir aux valeurs du jeu d'origine" @click="revert">Rétablir l'original</button>
              <button class="btn" :disabled="!neighbour(-1)" title="Pokémon précédent" aria-label="Pokémon précédent" @click="select(neighbour(-1)!.id)"><Icon name="chevron-left" :size="16" /></button>
              <button class="btn" :disabled="!neighbour(1)" title="Pokémon suivant" aria-label="Pokémon suivant" @click="select(neighbour(1)!.id)"><Icon name="chevron-right" :size="16" /></button>
            </div>
          </div>

          <div class="cards">
            <section class="card panel stats-card">
              <h3>Statistiques de base</h3>
              <div class="stats-body">
                <div class="stat-rows">
                  <div v-for="(s, i) in STATS" :key="s.key" class="stat-row" :class="{ changed: original && original.stats[i] !== shownStats[i] }">
                    <span class="stat-label">{{ s.label }}</span>
                    <input
                      type="range"
                      min="1"
                      max="255"
                      class="slider"
                      :value="shownStats[i]"
                      :disabled="readOnly"
                      :style="{ '--pct': `${(shownStats[i] / 255) * 100}%`, '--hue': Math.min(190, (shownStats[i] / 150) * 190) }"
                      :aria-label="s.label"
                      @input="setStat(i, $event)"
                    />
                    <input type="number" min="1" max="255" class="sv-input num" :value="shownStats[i]" :disabled="readOnly" :aria-label="s.label" @change="setStat(i, $event)" />
                  </div>
                  <div class="stat-row total-row">
                    <span class="stat-label">Total</span>
                    <span />
                    <strong class="num-total">
                      {{ total }}
                      <small v-if="total !== originalTotal" :class="total > originalTotal ? 'up' : 'down'">
                        {{ total > originalTotal ? "+" : "" }}{{ total - originalTotal }}
                      </small>
                    </strong>
                  </div>
                </div>
                <StatRadar :stats="radarStats" :size="200" class="radar" />
              </div>
            </section>

            <section class="card panel">
              <h3>Talents et capture</h3>
              <div v-if="cur" class="form">
                <label>
                  <span>Talent 1</span>
                  <Combo :model-value="cur.abilities[0]" :options="abilityOptions" @update:model-value="setAbility(0, $event)" />
                </label>
                <label>
                  <span>Talent 2</span>
                  <Combo :model-value="cur.abilities[1] === cur.abilities[0] ? 0 : cur.abilities[1]" :options="abilityOptions" none-label="Aucun" @update:model-value="setAbility(1, $event)" />
                </label>
                <label v-if="editor.data?.hiddenAbility">
                  <span>Talent caché <Tip term="hiddenAbility" /></span>
                  <Combo :model-value="cur.abilities[2]" :options="abilityOptions" none-label="Aucun" @update:model-value="setAbility(2, $event)" />
                </label>
                <label>
                  <span>
                    Taux de capture
                    <Tip term="catchRate" />
                  </span>
                  <input type="number" min="1" max="255" class="sv-input" :value="cur.catchRate" @change="setCatchRate" />
                </label>
              </div>
              <dl v-else class="facts">
                <dt>Talents</dt>
                <dd>{{ selected.abilities.join(" / ") || "—" }}</dd>
                <template v-if="selected.hiddenAbility">
                  <dt>Talent caché</dt>
                  <dd>{{ selected.hiddenAbility }}</dd>
                </template>
                <dt>Taux de capture</dt>
                <dd>{{ selected.catchRate }}</dd>
              </dl>
            </section>

            <section v-if="cur && editor.data" class="card panel moves-card">
              <div class="card-head">
                <h3>Attaques apprises par niveau</h3>
                <span class="dim">{{ cur.learnset.length }} / {{ editor.data.maxLearnset }}</span>
              </div>
              <div class="moves">
                <div v-for="(m, i) in cur.learnset" :key="`${i}-${m.level}-${m.move}`" class="move-row">
                  <label class="lvl">
                    <span>Niv.</span>
                    <input
                      type="number"
                      :min="minLevel"
                      max="100"
                      class="sv-input"
                      :value="m.level"
                      :title="m.level === 0 ? 'Niveau 0 : attaque apprise à l\'évolution' : undefined"
                      aria-label="Niveau"
                      @change="setLevel(i, $event)"
                    />
                  </label>
                  <Combo :model-value="m.move" :options="moveOptions" @update:model-value="setMove(i, $event)" />
                  <button class="remove" title="Retirer cette attaque" aria-label="Retirer cette attaque" @click="removeMove(i)"><Icon name="x" :size="14" /></button>
                </div>
              </div>
              <button class="btn add" :disabled="cur.learnset.length >= editor.data.maxLearnset" @click="addMove"><Icon name="plus" :size="15" /> Ajouter une attaque</button>
            </section>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.editor {
  max-width: 1680px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

h3 {
  margin: 0 0 14px;
  font-size: 15px;
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
  transition: background 0.15s;
}

.pick:hover {
  background: var(--panel-hover);
}

.pick strong {
  font-size: 17px;
}

.pick small {
  color: var(--text-dim);
}

.empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 24px;
  padding: 20px 24px;
}

.msg {
  margin-top: 16px;
}

.success {
  margin-top: 16px;
  padding: 12px 16px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  gap: 12px;
  border: 1px solid color-mix(in srgb, var(--accent-2) 60%, transparent);
  background: color-mix(in srgb, var(--accent-2) 12%, transparent);
}

.success span {
  flex: 1;
}

.success .path {
  word-break: break-all;
}

.ctr-output {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px 14px;
  margin-top: 16px;
  padding: 12px 16px;
}

.ctr-output .row-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
}

.close-x {
  display: grid;
  place-items: center;
  padding: 4px;
  border: none;
  border-radius: 50%;
  background: none;
  color: var(--text-dim);
}

.close-x:hover {
  color: var(--text);
}

.loading {
  display: grid;
  place-items: center;
  gap: 16px;
  margin-top: 120px;
  color: var(--text-dim);
}

.head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}

.chips {
  display: flex;
  gap: 6px;
  margin-bottom: 8px;
}

.chip.warn {
  color: var(--warn);
  border-color: var(--warn);
}

.actions {
  display: flex;
  gap: 8px;
}

.count-badge {
  min-width: 22px;
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--bg) 35%, transparent);
  font-size: 12px;
  font-weight: 700;
}

/* ---------- Espace de travail : liste + fiche ---------- */

.workspace {
  display: grid;
  grid-template-columns: clamp(280px, 22vw, 340px) minmax(0, 1fr);
  gap: 20px;
  align-items: start;
  margin-top: 20px;
}

.list {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  max-height: calc(100vh - 84px);
  overflow: hidden;
}

.filters {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-bottom: 1px solid var(--border);
}

.filter-row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px;
}

.only {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-dim);
  font-size: 13px;
}

.items {
  overflow-y: auto;
  padding: 6px;
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 46px;
  padding: 4px 10px 4px 4px;
  border: none;
  border-radius: 8px;
  background: none;
  color: var(--text);
  text-align: left;
}

.item:hover:not(.on) {
  background: var(--panel-hover);
}

/* Sélection : inversion fond / texte, comme .sv-list-item. */
.item.on {
  background: var(--text);
  color: var(--bg);
}

.item.on .item-main small,
.item.on .item-total {
  color: color-mix(in srgb, var(--bg) 72%, transparent);
}

.item.on .dot {
  background: var(--bg);
}

/* Les icônes pokesprite ont beaucoup de marge transparente. */
.item .icon {
  flex: none;
  margin: -8px -6px -6px -4px;
}

.item-main {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.item-main strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-main small {
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent-2);
}

.item-total {
  color: var(--text-dim);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}

.none {
  padding: 16px;
  color: var(--text-dim);
  text-align: center;
}

/* ---------- Fiche ---------- */

.sheet {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.hero {
  display: flex;
  align-items: center;
  gap: 20px;
  padding: 16px 20px;
}

.halo {
  display: grid;
  flex: none;
  place-items: center;
  width: 150px;
  height: 150px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  border: 2px solid color-mix(in srgb, var(--text) 15%, transparent);
}

.hero-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.hero-main h2 {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 28px;
}

.chip-mod {
  color: var(--accent-2);
  border-color: var(--accent-2);
}

.id {
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.type-pickers {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.type-pickers .sv-select {
  width: auto;
  min-width: 170px;
}

.types {
  display: flex;
  gap: 4px;
}

.hero-nav {
  display: flex;
  align-self: flex-start;
  gap: 8px;
}

.cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(440px, 1fr));
  gap: 16px;
  align-items: start;
}

.card {
  padding: 18px 20px;
}

.card-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.stats-body {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.stat-rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-width: 260px;
}

.stat-row {
  display: grid;
  grid-template-columns: 76px 1fr 70px;
  align-items: center;
  gap: 12px;
}

.stat-label {
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
}

.stat-row.changed .stat-label {
  color: var(--accent-2);
}

.num {
  padding: 6px 8px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.slider {
  width: 100%;
  height: 6px;
  border-radius: 3px;
  appearance: none;
  background: linear-gradient(to right, hsl(var(--hue) 80% 55%) var(--pct), color-mix(in srgb, var(--text) 14%, transparent) var(--pct));
  cursor: pointer;
}

.slider:disabled {
  cursor: default;
}

.slider::-webkit-slider-thumb {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  appearance: none;
  background: var(--text);
  box-shadow: 0 0 0 3px color-mix(in srgb, hsl(var(--hue) 80% 55%) 45%, transparent);
}

.slider:disabled::-webkit-slider-thumb {
  visibility: hidden;
}

.total-row {
  margin-top: 4px;
  padding-top: 8px;
  border-top: 1px solid var(--border);
}

.num-total {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.num-total small {
  display: block;
  font-size: 12px;
}

.up {
  color: var(--ok);
}

.down {
  color: var(--danger);
}

.radar {
  flex: none;
  margin: 0 auto;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form label {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form label > span {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
}

.facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 6px 14px;
  margin: 0;
}

.facts dt {
  color: var(--text-dim);
}

.facts dd {
  margin: 0;
  font-weight: 600;
}

.moves-card {
  grid-column: 1 / -1;
}

.moves {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 8px 16px;
}

.move-row {
  display: grid;
  grid-template-columns: 96px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 8px;
}

.lvl {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dim);
  font-size: 13px;
}

.lvl .sv-input {
  width: 58px;
  padding: 7px 8px;
  text-align: right;
}

.remove {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: none;
  color: var(--text-dim);
}

.remove:hover {
  background: color-mix(in srgb, var(--danger) 18%, transparent);
  color: var(--danger);
}

.add {
  margin-top: 12px;
}

@media (max-width: 900px) {
  .workspace {
    grid-template-columns: 1fr;
  }

  .list {
    position: static;
    max-height: 320px;
  }
}
</style>
