<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import PlayPanel from "../play/PlayPanel.vue";
import { library } from "../library";
import { nav } from "../nav";
import { open } from "@tauri-apps/plugin-dialog";
import {
  RANDOMIZABLE,
  isKaleidoRom,
  isRom,
  type CompatMode,
  type CtrOutcome,
  type ItemSettings,
  type Outcome,
  type PokemonRef,
  type Preset,
  type RandomizerSettings,
  type StaticSettings,
} from "../types";

/** Réglages par défaut : une randomisation « classique », à ajuster librement. */
const defaults = (): RandomizerSettings => ({
  starters: "triangle",
  customStarters: [0, 0, 0],
  kantoStarters: "unchanged",
  customKantoStarters: [0, 0, 0],
  wild: "area",
  wildSimilarStrength: true,
  wildLevelPercent: 100,
  trainers: "random",
  trainersSimilarStrength: true,
  trainerLevelPercent: 100,
  trainerEvolutions: false,
  trainerMaxIvs: false,
  stats: "unchanged",
  randomTypes: false,
  randomAbilities: false,
  noLegendaries: true,
  catchRate: "unchanged",
  easyEvolutions: true,
  randomMovesets: false,
  shinyOdds: 8192,
  moves: {
    randomTms: false,
    randomTutors: false,
    noGameBreaking: true,
    keepFieldMoves: true,
    goodDamagingPercent: 0,
    tmCompat: "unchanged",
    fullHmCompat: false,
    tutorCompat: "unchanged",
    followEvolutions: true,
    levelupSanity: true,
  },
  statics: { mode: "unchanged", levelModifier: 0, trades: "unchanged", tradeRandomItems: false, tradeRandomIvs: false },
  ctrOutput: "layered_fs",
  items: {
    fieldItems: "unchanged",
    banBadFieldItems: true,
    shops: "unchanged",
    banBadShopItems: true,
    banRegularShopItems: false,
    banOpShopItems: false,
    guaranteeEvolutionItems: true,
    guaranteeXItems: false,
    noRareCandy: false,
    noMasterBall: false,
  },
});
const settings = reactive<RandomizerSettings>(defaults());
const reset = () => {
  Object.assign(settings, defaults());
  customNames.value = ["", "", ""];
};

const SHINY_PRESETS = [
  { odds: 8192, label: "Normal" },
  { odds: 4096, label: "×2" },
  { odds: 1024, label: "×8" },
  { odds: 512, label: "×16" },
  { odds: 257, label: "Max (1/257)" },
  { odds: 1, label: "Tous ✨" },
];

/** Même calcul que le moteur : seuil arrondi entre 1 et 255 (sur 65 536). */
const shinyLabel = computed(() => {
  const n = Number(settings.shinyOdds) || 8192;
  if (n <= 1) return "tous les Pokémon";
  const threshold = Math.min(255, Math.max(1, Math.round(65536 / n)));
  return `1 / ${Math.floor(65536 / threshold)}`;
});

/** Noms des espèces (français), pour choisir ses starters. */
const speciesNames = ref<string[]>([]);
const speciesCount = computed(() => ({ 4: 493, 5: 649, 6: 721, 7: 807 })[selected.value?.generation ?? 5] ?? 649);
const customNames = ref(["", "", ""]);
const speciesId = (name: string) => {
  const i = speciesNames.value.findIndex((n, idx) => idx > 0 && idx <= speciesCount.value && n.toLowerCase() === name.trim().toLowerCase());
  return i > 0 ? i : 0;
};
watch(customNames, (names) => (settings.customStarters = names.map(speciesId)), { deep: true });
/** X / Y : Pokémon de Kanto du Professeur Platane, choisis à la main. */
const customKantoNames = ref(["", "", ""]);
watch(customKantoNames, (names) => (settings.customKantoStarters = names.map(speciesId)), { deep: true });

const newSeed = () => Math.floor(Math.random() * 4_294_967_295);
const seed = ref(newSeed());
const romPath = ref<string | null>(null);
const preview = ref<PokemonRef[]>([]);
const previewError = ref<string | null>(null);
const shareInput = ref("");
const shareError = ref(false);
const running = ref(false);
const outcome = ref<Outcome | null>(null);
const outputPath = ref<string | null>(null);
const runError = ref<string | null>(null);
const showLog = ref(false);

/** ROMs d'origine seulement : on ne randomise pas une ROM déjà générée. */
const roms = computed(() => library.items.filter((d) => isRom(d) && !isKaleidoRom(d)));
const selected = computed(() => roms.value.find((r) => r.path === romPath.value) ?? null);
const isCtr = computed(() => selected.value?.platform === "3ds");
/** Pokémon X / Y : second trio de starters (Professeur Platane). */
const isXy = computed(() => ["x", "y"].includes(selected.value?.game?.id ?? ""));
const target = ref<"luma" | "emulator">("luma");
const lastWasCtr = ref(false);
/** 3DS : jeu d'origine à lancer avec le mod (bouton « Jouer »). */
const playBase = ref<string | null>(null);
/** Fichiers produits par la dernière génération 3DS (dossier LayeredFS et/ou ROM). */
const ctrResult = ref<CtrOutcome | null>(null);
/** Les starters sont cachés par défaut pour garder la surprise. */
const showStarters = ref(false);
const supported = (id?: string) => !!id && RANDOMIZABLE.includes(id);

onMounted(async () => {
  romPath.value = nav.randomizerRom ?? roms.value.find((r) => supported(r.game?.id))?.path ?? null;
  presets.value = await invoke<Preset[]>("randomizer_presets").catch(() => []);
  speciesNames.value = (await invoke<{ species: string[] }>("name_lists")).species;
});

// --- Options des choix exclusifs (partagées entre les onglets et le résumé)

type Opt<T extends string> = { value: T; label: string; hint?: string };

const STARTER_OPTS: Opt<RandomizerSettings["starters"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Aléatoires" },
  { value: "three_stage", label: "Trio évolutif", hint: "Pokémon de base avec deux évolutions" },
  { value: "triangle", label: "Plante · Feu · Eau", hint: "Trio évolutif qui garde le triangle des types" },
  { value: "custom", label: "Je choisis", hint: "Tape le nom des trois Pokémon de ton choix" },
];
const WILD_OPTS: Opt<RandomizerSettings["wild"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Totalement aléatoires" },
  { value: "area", label: "Par zone", hint: "Dans une zone, chaque espèce est remplacée par une même nouvelle espèce" },
  { value: "global", label: "Global", hint: "Une espèce devient la même partout dans le jeu" },
];
const TRAINER_OPTS: Opt<RandomizerSettings["trainers"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Aléatoires" },
  { value: "type_themed", label: "Thématiques", hint: "Chaque dresseur garde un type dominant (champions compris)" },
];
const STATS_OPTS: Opt<RandomizerSettings["stats"]>[] = [
  { value: "unchanged", label: "Statistiques inchangées" },
  { value: "shuffle", label: "Mélangées", hint: "Les 6 statistiques sont permutées, le total ne change pas" },
  { value: "random", label: "Redistribuées", hint: "Nouvelle répartition du même total" },
];
const CATCH_OPTS: Opt<RandomizerSettings["catchRate"]>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "doubled", label: "Facile (×2)" },
  { value: "max", label: "Garantie", hint: "Taux de capture maximal pour toutes les espèces" },
];
const TM_COMPAT_OPTS: Opt<CompatMode>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "random_prefer_type", label: "Aléatoire (selon le type)" },
  { value: "random", label: "Aléatoire" },
  { value: "full", label: "Toutes les CT pour tous" },
];
const TUTOR_COMPAT_OPTS: Opt<CompatMode>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "random_prefer_type", label: "Aléatoire (selon le type)" },
  { value: "full", label: "Tout pour tous" },
];
const FIELD_OPTS: Opt<ItemSettings["fieldItems"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "shuffle", label: "Mélangés", hint: "Les mêmes objets, à d'autres endroits" },
  { value: "random", label: "Aléatoires" },
  { value: "random_even", label: "Aléatoires équilibrés", hint: "Chaque objet sort une fois avant toute répétition" },
];
const SHOP_OPTS: Opt<ItemSettings["shops"]>[] = [
  { value: "unchanged", label: "Inchangées" },
  { value: "shuffle", label: "Mélangées" },
  { value: "random", label: "Aléatoires", hint: "Les comptoirs principaux et les boutiques de CT ne changent pas" },
];
const STATIC_OPTS: Opt<StaticSettings["mode"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "swap_legendaries", label: "Légendaire contre légendaire", hint: "Un légendaire devient un autre légendaire, un Pokémon ordinaire un autre ordinaire" },
  { value: "similar_strength", label: "Puissance similaire" },
  { value: "random", label: "Aléatoires" },
];
const TRADE_OPTS: Opt<StaticSettings["trades"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "given", label: "Pokémon reçu aléatoire", hint: "Le Pokémon demandé reste le même" },
  { value: "given_and_requested", label: "Reçu et demandé aléatoires" },
];

// --- Onglets des réglages

type TabId = "general" | "pokemon" | "starters" | "wild" | "trainers" | "moves" | "items" | "statics" | "shiny";
const TABS: { id: TabId; label: string; intro: string }[] = [
  { id: "general", label: "Général", intro: "Préréglages, sortie et résumé de tout ce qui change par rapport à une randomisation classique." },
  { id: "pokemon", label: "Pokémon", intro: "Ce qui change pour toutes les espèces : statistiques, types, talents, évolutions et attaques apprises." },
  { id: "starters", label: "Starters", intro: "Les trois Pokémon proposés au début de l'aventure." },
  { id: "wild", label: "Sauvages", intro: "Les Pokémon rencontrés dans les hautes herbes, les grottes et sur l'eau." },
  { id: "trainers", label: "Dresseurs", intro: "Les équipes des dresseurs, champions d'arène et Conseil 4 compris." },
  { id: "moves", label: "Attaques & CT", intro: "Le contenu des CT, les maîtres des capacités et qui peut les apprendre." },
  { id: "items", label: "Objets", intro: "Les objets ramassés par terre et le stock des boutiques." },
  { id: "statics", label: "Fixes & échanges", intro: "Les Pokémon rencontrés à un endroit précis, les dons et les échanges en jeu." },
  { id: "shiny", label: "Chromatiques", intro: "La probabilité de croiser un Pokémon chromatique ✨." },
];
const TAB_KEY = "kaleido.randomizer.tab";
function readTab(): TabId {
  try {
    const t = localStorage.getItem(TAB_KEY);
    return TABS.some((x) => x.id === t) ? (t as TabId) : "general";
  } catch {
    return "general";
  }
}
const tab = ref<TabId>(readTab());
const tabsEl = ref<HTMLElement | null>(null);
watch(tab, (t) => {
  // Garde l'onglet actif visible quand la barre défile (fenêtre étroite).
  nextTick(() =>
    tabsEl.value?.querySelector<HTMLElement>(".on")?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" }),
  );
  try {
    localStorage.setItem(TAB_KEY, t);
  } catch {
    /* stockage indisponible : l'onglet ne sera simplement pas mémorisé */
  }
});
const currentTab = computed(() => TABS.find((t) => t.id === tab.value) ?? TABS[0]);
function stepTab(delta: number) {
  const i = TABS.findIndex((t) => t.id === tab.value);
  tab.value = TABS[(i + delta + TABS.length) % TABS.length].id;
}

/** Q / E ou Ctrl+Tab (Maj pour reculer) changent d'onglet, sauf pendant la saisie de texte. */
function onKey(e: KeyboardEvent) {
  if (e.defaultPrevented || showLog.value) return;
  if (e.key === "Tab" && e.ctrlKey) {
    e.preventDefault();
    stepTab(e.shiftKey ? -1 : 1);
    return;
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  const t = e.target as HTMLElement | null;
  const textField =
    !!t &&
    (t.isContentEditable ||
      t.tagName === "TEXTAREA" ||
      t.tagName === "SELECT" ||
      (t.tagName === "INPUT" && !["checkbox", "radio", "range", "button"].includes((t as HTMLInputElement).type)));
  if (textField) return;
  const k = e.key.toLowerCase();
  if (k === "q" || k === "e") {
    e.preventDefault();
    stepTab(k === "q" ? -1 : 1);
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

// --- Résumé : chaque option différente des réglages par défaut, en français courant

const DEFAULTS = defaults();
const lab = <T extends string>(opts: Opt<T>[], v: T) => opts.find((o) => o.value === v)?.label ?? v;
interface Change {
  tab: TabId;
  text: string;
}
const changes = computed<Change[]>(() => {
  const s = settings;
  const d = DEFAULTS;
  const out: Change[] = [];
  const add = (tab: TabId, changed: boolean, text: string) => {
    if (changed) out.push({ tab, text });
  };
  const flag = (tab: TabId, label: string, v: boolean, dv: boolean) => add(tab, v !== dv, `${label} : ${v ? "oui" : "non"}`);
  const pct = (p: number) => `${p >= 0 ? "+" : ""}${p} %`;

  add("pokemon", s.stats !== d.stats, `Statistiques : ${lab(STATS_OPTS, s.stats).toLowerCase()}`);
  flag("pokemon", "Types aléatoires", s.randomTypes, d.randomTypes);
  flag("pokemon", "Talents aléatoires", s.randomAbilities, d.randomAbilities);
  flag("pokemon", "Sans légendaires", s.noLegendaries, d.noLegendaries);
  flag("pokemon", "Évolutions sans échange", s.easyEvolutions, d.easyEvolutions);
  flag("pokemon", "Attaques apprises aléatoires", s.randomMovesets, d.randomMovesets);
  add("pokemon", s.catchRate !== d.catchRate, `Capture : ${lab(CATCH_OPTS, s.catchRate).toLowerCase()}`);

  const chosen = s.starters === "custom" ? customNames.value.filter((n) => n.trim()).join(", ") : "";
  add("starters", s.starters !== d.starters, `Starters : ${lab(STARTER_OPTS, s.starters).toLowerCase()}${chosen ? ` (${chosen})` : ""}`);
  const kanto = s.kantoStarters;
  const kantoChosen = kanto === "custom" ? customKantoNames.value.filter((n) => n.trim()).join(", ") : "";
  add("starters", isXy.value && kanto !== "unchanged", `Pokémon de Kanto : ${lab(STARTER_OPTS, kanto).toLowerCase()}${kantoChosen ? ` (${kantoChosen})` : ""}`);

  add("wild", s.wild !== d.wild, `Pokémon sauvages : ${lab(WILD_OPTS, s.wild).toLowerCase()}`);
  flag("wild", "Sauvages de puissance similaire", s.wildSimilarStrength, d.wildSimilarStrength);
  add("wild", s.wildLevelPercent !== d.wildLevelPercent, `Niveaux des sauvages : ${levelLabel(s.wildLevelPercent)}`);

  add("trainers", s.trainers !== d.trainers, `Dresseurs : ${lab(TRAINER_OPTS, s.trainers).toLowerCase()}`);
  flag("trainers", "Dresseurs de puissance similaire", s.trainersSimilarStrength, d.trainersSimilarStrength);
  add("trainers", s.trainerLevelPercent !== d.trainerLevelPercent, `Niveaux des dresseurs : ${levelLabel(s.trainerLevelPercent)}`);
  flag("trainers", "Pokémon des dresseurs évolués", s.trainerEvolutions, d.trainerEvolutions);
  flag("trainers", "IV des dresseurs au maximum", s.trainerMaxIvs, d.trainerMaxIvs);

  const m = s.moves;
  const dm = d.moves;
  flag("moves", "CT aléatoires", m.randomTms, dm.randomTms);
  flag("moves", "Maîtres des capacités aléatoires", m.randomTutors, dm.randomTutors);
  flag("moves", "Garder les attaques de terrain", m.keepFieldMoves, dm.keepFieldMoves);
  flag("moves", "Sans Sonicboom / Draco-Rage", m.noGameBreaking, dm.noGameBreaking);
  add("moves", m.goodDamagingPercent !== dm.goodDamagingPercent, `Attaques offensives garanties : ${m.goodDamagingPercent} %`);
  add("moves", m.tmCompat !== dm.tmCompat, `Compatibilité CT : ${lab(TM_COMPAT_OPTS, m.tmCompat).toLowerCase()}`);
  add("moves", m.tutorCompat !== dm.tutorCompat, `Maîtres des capacités : ${lab(TUTOR_COMPAT_OPTS, m.tutorCompat).toLowerCase()}`);
  flag("moves", "Toutes les CS pour tous", m.fullHmCompat, dm.fullHmCompat);
  flag("moves", "Les évolutions héritent des CT", m.followEvolutions, dm.followEvolutions);
  flag("moves", "Garder les CT des attaques apprises", m.levelupSanity, dm.levelupSanity);

  const it = s.items;
  const di = d.items;
  add("items", it.fieldItems !== di.fieldItems, `Objets au sol : ${lab(FIELD_OPTS, it.fieldItems).toLowerCase()}`);
  add("items", it.shops !== di.shops, `Boutiques : ${lab(SHOP_OPTS, it.shops).toLowerCase()}`);
  flag("items", "Pas d'objets inutiles", it.banBadFieldItems, di.banBadFieldItems);
  flag("items", "Pas d'objets inutiles en boutique", it.banBadShopItems, di.banBadShopItems);
  flag("items", "Pas d'objets ordinaires en boutique", it.banRegularShopItems, di.banRegularShopItems);
  flag("items", "Pierres d'évolution en vente", it.guaranteeEvolutionItems, di.guaranteeEvolutionItems);
  flag("items", "Objets X en vente", it.guaranteeXItems, di.guaranteeXItems);
  flag("items", "Pas d'objets trop forts en boutique", it.banOpShopItems, di.banOpShopItems);
  flag("items", "Sans Super Bonbon", it.noRareCandy, di.noRareCandy);
  flag("items", "Sans Master Ball", it.noMasterBall, di.noMasterBall);

  const st = s.statics;
  const ds = d.statics;
  add("statics", st.mode !== ds.mode, `Pokémon fixes et dons : ${lab(STATIC_OPTS, st.mode).toLowerCase()}`);
  add("statics", st.levelModifier !== ds.levelModifier, `Niveaux des Pokémon fixes : ${pct(st.levelModifier)}`);
  add("statics", st.trades !== ds.trades, `Échanges : ${lab(TRADE_OPTS, st.trades).toLowerCase()}`);
  flag("statics", "Objets tenus aléatoires (échanges)", st.tradeRandomItems, ds.tradeRandomItems);
  flag("statics", "IV aléatoires (échanges)", st.tradeRandomIvs, ds.tradeRandomIvs);

  add("shiny", s.shinyOdds !== d.shinyOdds, `Chromatiques : ${shinyLabel.value}`);
  return out;
});
const changeCount = (id: TabId) => changes.value.filter((c) => c.tab === id).length;
const modifs = (n: number) => `${n} modif${n > 1 ? "s" : ""}`;
const tabLabel = (id: TabId) => TABS.find((t) => t.id === id)?.label ?? id;

// --- Préréglages (fournis par le moteur)

const presets = ref<Preset[]>([]);
function applyPreset(p: Preset) {
  // Les réglages absents (Pokémon de Kanto inchangés) sont omis : on repart des valeurs par défaut.
  Object.assign(settings, { kantoStarters: "unchanged", customKantoStarters: [0, 0, 0] }, JSON.parse(JSON.stringify(p.settings)) as RandomizerSettings);
  customNames.value = ["", "", ""];
}

// Aperçu des starters, recalculé quand la ROM, la seed ou les réglages changent.
let previewTimer: number | undefined;
watch(
  [romPath, seed, () => settings.starters, () => settings.noLegendaries, () => settings.randomTypes, () => [...settings.customStarters]],
  () => {
    clearTimeout(previewTimer);
    previewTimer = window.setTimeout(refreshPreview, 200);
  },
  { immediate: true },
);

async function refreshPreview() {
  preview.value = [];
  previewError.value = null;
  if (!romPath.value || !supported(selected.value?.game?.id)) return;
  try {
    preview.value = await invoke<PokemonRef[]>("preview_starters", { path: romPath.value, settings: { ...settings }, seed: seed.value });
  } catch (e) {
    previewError.value = String(e);
  }
}

async function shareCode() {
  // Même format que le moteur : KLD1- + JSON en base64 URL.
  const json = JSON.stringify({ seed: seed.value, settings });
  const b64 = btoa(String.fromCharCode(...new TextEncoder().encode(json))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  await navigator.clipboard.writeText(`KLD1-${b64}`);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
const copied = ref(false);

async function importCode() {
  const parsed = await invoke<[number, RandomizerSettings] | null>("parse_share_code", { code: shareInput.value });
  shareError.value = !parsed;
  if (parsed) {
    seed.value = parsed[0];
    Object.assign(settings, { kantoStarters: "unchanged", customKantoStarters: [0, 0, 0] }, parsed[1]);
    shareInput.value = "";
  }
}

async function generate() {
  if (!selected.value) return;
  if (isCtr.value) return generateCtr();
  const base = selected.value.path.replace(/\.nds$/i, "");
  const output = await save({
    title: "Enregistrer la ROM randomisée",
    defaultPath: `${base} - Kaleido ${seed.value}.nds`,
    filters: [{ name: "ROM Nintendo DS", extensions: ["nds"] }],
  });
  if (!output) return;
  running.value = true;
  runError.value = null;
  outcome.value = null;
  try {
    outcome.value = await invoke<Outcome>("randomize_rom", { path: selected.value.path, settings: { ...settings }, seed: seed.value, output });
    outputPath.value = output;
    lastWasCtr.value = false;
  } catch (e) {
    runError.value = String(e);
  } finally {
    running.value = false;
  }
}

/** 3DS : les fichiers modifiés sont écrits dans un dossier LayeredFS. */
async function generateCtr() {
  const output = await open({ directory: true, title: settings.ctrOutput === "layered_fs" ? "Choisis le dossier où créer le mod" : "Choisis le dossier où créer la ROM" });
  if (typeof output !== "string" || !selected.value) return;
  playBase.value = selected.value.path;
  running.value = true;
  runError.value = null;
  outcome.value = null;
  try {
    const res = await invoke<CtrOutcome>("randomize_ctr", {
      path: selected.value.path,
      settings: { ...settings },
      seed: seed.value,
      output,
      target: target.value,
    });
    outcome.value = res;
    ctrResult.value = res;
    outputPath.value = res.image ?? res.romfs;
    lastWasCtr.value = true;
  } catch (e) {
    runError.value = String(e);
  } finally {
    running.value = false;
  }
}

const levelLabel = (p: number) => (p === 100 ? "inchangés" : `${p > 100 ? "+" : ""}${p - 100} %`);
</script>

<template>
  <section class="rando">
    <header class="page-head">
      <div>
        <h1>Randomizer</h1>
        <p class="lead">Une nouvelle aventure à chaque seed. Partage le code pour que tes amis jouent exactement la même.</p>
      </div>
      <button class="btn" title="Revenir aux réglages par défaut" @click="reset">Réinitialiser</button>
    </header>

    <!-- Choix de la ROM -->
    <div v-if="!roms.length" class="panel empty">
      <p>Ajoute d'abord une ROM DS (Platine, Noire ou Blanche) dans la bibliothèque.</p>
      <button class="btn btn-primary" @click="nav.view = 'home'">Ajouter une ROM</button>
    </div>

    <template v-else>
      <div class="roms">
        <button
          v-for="r in roms"
          :key="r.path"
          class="rom panel"
          :class="{ active: romPath === r.path }"
          :disabled="!supported(r.game?.id)"
          :title="supported(r.game?.id) ? r.path : 'Pas encore pris en charge par le randomizer'"
          @click="romPath = r.path"
        >
          <strong>{{ r.title }}</strong>
          <small>{{ supported(r.game?.id) ? r.language : "Bientôt" }}</small>
        </button>
      </div>

      <div class="layout">
        <div class="options">
          <!-- Onglets des réglages (Q / E ou Ctrl+Tab) -->
          <div class="tabbar">
            <kbd class="cap" title="Onglet précédent (Q)" @click="stepTab(-1)">Q</kbd>
            <nav ref="tabsEl" class="tabs" role="tablist" aria-label="Réglages du randomizer">
              <button
                v-for="t in TABS"
                :key="t.id"
                role="tab"
                :aria-selected="tab === t.id"
                :class="{ on: tab === t.id }"
                @click="tab = t.id"
              >
                {{ t.label }}
                <span v-if="t.id !== 'general' && changeCount(t.id)" class="badge" :title="`${modifs(changeCount(t.id))} par rapport aux réglages par défaut`">
                  {{ modifs(changeCount(t.id)) }}
                </span>
              </button>
            </nav>
            <kbd class="cap" title="Onglet suivant (E)" @click="stepTab(1)">E</kbd>
          </div>
          <p class="tab-intro">{{ currentTab.intro }}</p>

          <!-- Général : préréglages, sortie 3DS, résumé -->
          <template v-if="tab === 'general'">
          <div v-if="isCtr" class="section panel">
            <h3>Sortie 3DS</h3>
            <div class="row first">
              <span class="row-label">
                Format
                <Tip title="LayeredFS" text="LayeredFS : un dossier de mod léger, ta ROM reste intacte. Seuls les fichiers modifiés sont écrits ; Luma3DS ou l'émulateur les charge à la place des originaux." />
                <Tip
                  title="Fichier .3ds"
                  text="Fichier .3ds : une ROM complète à ouvrir directement dans Azahar/Citra, plus lourde (~2 Go). Elle est déchiffrée et ses signatures ne sont plus valides : parfait pour un émulateur. Sur une vraie console, il faut la convertir en CIA (par ex. « Build CIA from file » dans GodMode9) puis l'installer avec FBI sous Luma3DS — ou plus simplement utiliser la sortie LayeredFS."
                />
              </span>
              <Segmented
                v-model="settings.ctrOutput"
                :options="[
                  { value: 'layered_fs', label: 'LayeredFS', hint: 'Un dossier de mod léger, ta ROM reste intacte' },
                  { value: 'rom3ds', label: 'Fichier .3ds', hint: 'Une ROM complète à ouvrir directement dans Azahar/Citra, plus lourde (~2 Go)' },
                  { value: 'both', label: 'Les deux' },
                ]"
              />
            </div>
            <div v-if="settings.ctrOutput !== 'rom3ds'" class="row">
              <span class="row-label">Dossier pour</span>
              <Segmented
                v-model="target"
                :options="[
                  { value: 'luma', label: 'Console (Luma3DS)', hint: 'Crée luma/titles/…/romfs : copie le dossier « luma » à la racine de la carte SD' },
                  { value: 'emulator', label: 'Émulateur', hint: 'Crée <title ID>/romfs : à placer dans le dossier « mods » de l\'émulateur (Azahar, Citra…)' },
                ]"
              />
            </div>
            <p v-if="settings.ctrOutput !== 'rom3ds'" class="dim note">
              LayeredFS : seuls les fichiers modifiés sont écrits, ta ROM d'origine n'est jamais touchée. Sur console, active
              « Enable game patching » dans la configuration de Luma3DS.
            </p>
            <p v-if="settings.ctrOutput !== 'layered_fs'" class="dim note">
              Le .3ds créé est une copie complète et déchiffrée de ta ROM (environ sa taille, compte quelques secondes) : il se lance
              tel quel dans un émulateur. Sur console, convertis-le en CIA et installe-le, ou préfère LayeredFS.
            </p>
          </div>

            <div v-if="presets.length" class="section panel">
              <h3>
                Préréglages
                <Tip title="Préréglages" text="Remplace tous les réglages par une combinaison toute prête. Tu peux ensuite ajuster chaque option dans les onglets." />
              </h3>
              <div class="presets">
                <button v-for="p in presets" :key="p.id" class="preset" @click="applyPreset(p)">
                  <strong>{{ p.name }}</strong>
                  <small>{{ p.description }}</small>
                </button>
              </div>
            </div>

            <div class="section panel">
              <div class="summary-head">
                <h3>Résumé <span class="dim count">{{ changes.length ? modifs(changes.length) : "réglages par défaut" }}</span></h3>
                <button v-if="changes.length" class="btn small" @click="reset">Tout réinitialiser</button>
              </div>
              <p v-if="!changes.length" class="dim note">
                Rien n'a été modifié : starters Plante · Feu · Eau, sauvages par zone et dresseurs aléatoires de puissance
                similaire. Parcours les onglets pour personnaliser ta partie.
              </p>
              <ul v-else class="summary">
                <li v-for="(c, i) in changes" :key="i">
                  <button class="summary-tab" :title="`Aller à l'onglet ${tabLabel(c.tab)}`" @click="tab = c.tab">{{ tabLabel(c.tab) }}</button>
                  <span>{{ c.text }}</span>
                </li>
              </ul>
            </div>
          </template>

          <!-- Pokémon : statistiques, types, talents, évolutions -->
          <template v-else-if="tab === 'pokemon'">
            <div class="section panel">
              <h3>Statistiques, types et talents</h3>
              <Segmented v-model="settings.stats" :options="STATS_OPTS" />
              <div class="row">
                <Toggle v-model="settings.randomTypes" label="Types aléatoires" hint="Une famille d'évolution garde les mêmes types" />
                <Toggle v-model="settings.randomAbilities" label="Talents aléatoires" hint="Chaque espèce reçoit des talents tirés au sort. Garde Mystik, Multitype, Illusion et Mode Transe ne sont jamais attribués." />
                <Toggle v-model="settings.noLegendaries" label="Sans légendaires" hint="Aucun légendaire ni fabuleux n'est tiré au sort pour remplacer un autre Pokémon." />
              </div>
            </div>

            <div class="section panel">
              <h3>Évolutions, attaques et capture</h3>
              <div class="row first">
                <Toggle
                  v-model="settings.easyEvolutions"
                  label="Évolutions sans échange"
                  hint="Les évolutions par échange se font au niveau 37, ou avec l'objet habituel (Peau Métal…)"
                />
                <Toggle
                  v-model="settings.randomMovesets"
                  label="Attaques apprises aléatoires"
                  hint="Chaque Pokémon garde sa première attaque, les suivantes sont tirées au hasard"
                />
              </div>
              <div class="row">
                <span class="row-label">Capture</span>
                <Segmented v-model="settings.catchRate" :options="CATCH_OPTS" />
              </div>
            </div>
          </template>

          <!-- Starters -->
          <div v-else-if="tab === 'starters'" class="section panel">
            <h3>Starters</h3>
            <Segmented v-model="settings.starters" :options="STARTER_OPTS" />
            <div v-if="settings.starters === 'custom'" class="custom-starters">
              <label v-for="(_, i) in customNames" :key="i">
                <Sprite v-if="settings.customStarters[i]" :id="settings.customStarters[i]" :size="44" />
                <input
                  v-model="customNames[i]"
                  class="input"
                  list="kaleido-species"
                  :placeholder="['Starter Plante', 'Starter Feu', 'Starter Eau'][i]"
                  :class="{ invalid: customNames[i] && !settings.customStarters[i] }"
                />
              </label>
              <datalist id="kaleido-species">
                <option v-for="n in speciesNames.slice(1, speciesCount + 1)" :key="n" :value="n" />
              </datalist>
            </div>
            <p class="dim note">L'aperçu à droite se met à jour avec la seed : coche « Voir » pour découvrir tes starters.</p>

            <template v-if="isXy">
              <h3 class="kanto-title">
                Pokémon de Kanto (Professeur Platane)
                <Tip
                  title="Pokémon de Kanto"
                  text="Dans X et Y, le Professeur Platane offre à Illumis un second starter au choix : Bulbizarre, Salamèche ou Carapuce (niveau 10). Kaleido les remplace sans reprendre les starters ci-dessus ; le niveau 10 est conservé."
                />
              </h3>
              <Segmented v-model="settings.kantoStarters" :options="STARTER_OPTS" />
              <div v-if="settings.kantoStarters === 'custom'" class="custom-starters">
                <label v-for="(_, i) in customKantoNames" :key="i">
                  <Sprite v-if="settings.customKantoStarters?.[i]" :id="settings.customKantoStarters[i]" :size="44" />
                  <input
                    v-model="customKantoNames[i]"
                    class="input"
                    list="kaleido-species"
                    :placeholder="['Au lieu de Bulbizarre', 'Au lieu de Salamèche', 'Au lieu de Carapuce'][i]"
                    :class="{ invalid: customKantoNames[i] && !settings.customKantoStarters?.[i] }"
                  />
                </label>
                <datalist v-if="settings.starters !== 'custom'" id="kaleido-species">
                  <option v-for="n in speciesNames.slice(1, speciesCount + 1)" :key="n" :value="n" />
                </datalist>
              </div>
            </template>
          </div>

          <!-- Pokémon sauvages -->
          <div v-else-if="tab === 'wild'" class="section panel">
            <h3>Pokémon sauvages</h3>
            <Segmented v-model="settings.wild" :options="WILD_OPTS" />
            <div class="row">
              <Toggle v-model="settings.wildSimilarStrength" label="Puissance similaire" hint="Un Pokémon sauvage est remplacé par une espèce de force comparable (total des statistiques de base proche) : pas de Dracolosse sur la Route 1." />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.wildLevelPercent) }}</strong>
                <input v-model.number="settings.wildLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
          </div>

          <!-- Dresseurs -->
          <div v-else-if="tab === 'trainers'" class="section panel">
            <h3>Dresseurs</h3>
            <Segmented v-model="settings.trainers" :options="TRAINER_OPTS" />
            <div class="row">
              <Toggle v-model="settings.trainersSimilarStrength" label="Puissance similaire" hint="Chaque Pokémon des dresseurs est remplacé par une espèce de force comparable, pour garder la difficulté d'origine." />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.trainerLevelPercent) }}</strong>
                <input v-model.number="settings.trainerLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
            <div class="row">
              <Toggle
                v-model="settings.trainerEvolutions"
                label="Pokémon évolués selon leur niveau"
                hint="Un Machoc niveau 40 devient Mackogneur… (niveau 40 pour les évolutions sans niveau)"
              />
              <Toggle v-model="settings.trainerMaxIvs" label="IV au maximum" hint="Tous les Pokémon des dresseurs ont des IV parfaits" />
            </div>
          </div>

          <!-- CT & capacités -->
          <div v-else-if="tab === 'moves'" class="section panel">
            <h3>CT &amp; capacités <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span></h3>
            <div class="row first">
              <Toggle v-model="settings.moves.randomTms" label="CT aléatoires" hint="Les CS ne changent jamais" />
              <Toggle v-model="settings.moves.randomTutors" label="Maîtres des capacités aléatoires" hint="Platine, Noire 2 et Blanche 2" />
              <Toggle v-model="settings.moves.keepFieldMoves" label="Garder les attaques de terrain" hint="Tunnel, Flash… restent à leur place" />
              <Toggle v-model="settings.moves.noGameBreaking" label="Sans Sonicboom / Draco-Rage" hint="Ces attaques infligent des dégâts fixes (20 et 40 PV) : très fortes en début de partie, elles cassent l'équilibre." />
            </div>
            <div class="row">
              <span class="row-label">Compatibilité CT</span>
              <Segmented v-model="settings.moves.tmCompat" :options="TM_COMPAT_OPTS" />
            </div>
            <div class="row">
              <span class="row-label">Maîtres des capacités</span>
              <Segmented v-model="settings.moves.tutorCompat" :options="TUTOR_COMPAT_OPTS" />
            </div>
            <div class="row">
              <Toggle v-model="settings.moves.fullHmCompat" label="Toutes les CS pour tous" hint="Pratique pour ne jamais être bloqué" />
              <Toggle v-model="settings.moves.followEvolutions" label="Les évolutions héritent" hint="Une évolution garde les compatibilités CT de sa forme précédente (plus logique : Dracaufeu sait tout ce que savait Salamèche)." />
              <Toggle v-model="settings.moves.levelupSanity" label="Garder les CT des attaques apprises" hint="Si un Pokémon apprend une attaque par niveau, il reste compatible avec la CT de cette attaque." />
            </div>
          </div>

          <!-- Objets & boutiques -->
          <div v-else-if="tab === 'items'" class="section panel">
            <h3>Objets &amp; boutiques <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span></h3>
            <div class="row first">
              <span class="row-label">Objets au sol</span>
              <Segmented v-model="settings.items.fieldItems" :options="FIELD_OPTS" />
            </div>
            <div class="row">
              <span class="row-label">Boutiques</span>
              <Segmented v-model="settings.items.shops" :options="SHOP_OPTS" />
            </div>
            <div class="row">
              <Toggle v-model="settings.items.banBadFieldItems" label="Pas d'objets inutiles" hint="Lettres, Fertilisants, Baies sans effet…" />
              <Toggle v-model="settings.items.guaranteeEvolutionItems" label="Pierres d'évolution en vente" hint="Les Pierres Feu, Eau, Foudre… et autres objets d'évolution sont toujours achetables quelque part, pour ne pas bloquer une évolution." />
              <Toggle v-model="settings.items.guaranteeXItems" label="Objets X en vente" hint="Les objets X (Attaque +, Défense +, Vitesse +, Précision +…) augmentent une statistique pendant un combat. Cette option les garde en vente dans les boutiques après randomisation, pratique contre les combats difficiles." />
              <Toggle v-model="settings.items.banOpShopItems" label="Pas d'objets trop forts en boutique" hint="Super Bonbon, Pépites, Œuf Chance…" />
              <Toggle v-model="settings.items.noRareCandy" label="Sans Super Bonbon" hint="Le Super Bonbon (un niveau gratuit) n'apparaît ni au sol ni en boutique." />
              <Toggle v-model="settings.items.noMasterBall" label="Sans Master Ball" hint="La Master Ball (capture garantie) n'apparaît pas dans les objets randomisés." />
            </div>
            <p class="dim note">Objets clés et CS ne bougent jamais ; une CT est toujours remplacée par une CT.</p>
          </div>

          <!-- Pokémon fixes & échanges -->
          <div v-else-if="tab === 'statics'" class="section panel">
            <h3>Pokémon fixes &amp; échanges <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span></h3>
            <div class="row first">
              <span class="row-label">Fixes et dons <Tip title="Pokémon fixes et dons" text="Les Pokémon qu'on rencontre à un endroit précis (légendaires, Ronflex qui bloque la route…) et ceux qu'on reçoit en cadeau (fossiles, œufs, starters secondaires)." /></span>
              <Segmented v-model="settings.statics.mode" :options="STATIC_OPTS" />
            </div>
            <div v-if="settings.statics.mode !== 'unchanged'" class="row">
              <label class="slider">
                Niveaux <strong>{{ settings.statics.levelModifier >= 0 ? "+" : "" }}{{ settings.statics.levelModifier }} %</strong>
                <input v-model.number="settings.statics.levelModifier" type="range" min="-50" max="50" step="5" />
              </label>
              <Tip title="Niveau des Pokémon fixes" text="Augmente ou baisse le niveau de ces rencontres (les œufs ne changent pas). +20 % : un légendaire niveau 70 passe niveau 84." />
            </div>
            <div class="row">
              <span class="row-label">Échanges <Tip title="Échanges en jeu" text="Les Pokémon que des personnages proposent d'échanger contre l'un des tiens (par exemple Kéké le Chétiflor dans Platine)." /></span>
              <Segmented v-model="settings.statics.trades" :options="TRADE_OPTS" />
            </div>
            <div v-if="settings.statics.trades !== 'unchanged'" class="row">
              <Toggle v-model="settings.statics.tradeRandomItems" label="Objets tenus aléatoires" hint="Le Pokémon reçu tient un objet tiré au sort." />
              <Toggle v-model="settings.statics.tradeRandomIvs" label="IV aléatoires" hint="Les IV (le « potentiel génétique ») du Pokémon reçu sont tirés au sort au lieu d'être fixés par le jeu." />
            </div>
          </div>

          <!-- Chromatiques -->
          <div v-else-if="tab === 'shiny'" class="section panel">
            <h3>Chromatiques ✨</h3>
            <div class="row shiny-row first">
              <label class="shiny-input">
                1 chance sur
                <input v-model.number="settings.shinyOdds" class="input" type="number" min="1" max="65536" :disabled="isCtr" />
              </label>
              <div class="chips-row">
                <button v-for="q in SHINY_PRESETS" :key="q.odds" class="chip-btn" :class="{ on: settings.shinyOdds === q.odds }" :disabled="isCtr" @click="settings.shinyOdds = q.odds">
                  {{ q.label }}
                </button>
              </div>
            </div>
            <p class="dim note">
              <template v-if="isCtr">Pas encore disponible sur 3DS : le taux est défini dans le code du jeu (code.bin).</template>
              <template v-else>
                Taux réel : <strong>{{ shinyLabel }}</strong>. Modifie la fonction du jeu qui décide si un Pokémon est chromatique
                (sauvages, dons, œufs, dresseurs).
              </template>
            </p>
            <p v-if="!isCtr && settings.shinyOdds <= 1" class="warn-text">
              ⚠ Expérimental : certains événements relancent le tirage tant que le Pokémon est chromatique (Pokémon qui ne
              doivent jamais l'être, comme Reshiram / Zekrom). Avec 100 %, ces scènes peuvent bloquer le jeu.
            </p>
          </div>
        </div>

        <!-- Colonne de droite : aperçu et génération -->
        <aside class="side">
          <div class="panel card">
            <div class="card-head">
              <h3>Tes starters</h3>
              <Toggle v-if="preview.length" v-model="showStarters" label="Voir" hint="Les starters restent cachés pour garder la surprise" />
            </div>
            <div class="starters">
              <div v-for="(p, i) in preview" :key="`${p.id}-${i}`" class="starter" :class="{ hidden: !showStarters }">
                <template v-if="showStarters">
                  <Sprite :id="p.id" variant="model" :size="88" />
                  <span>{{ p.name }}</span>
                </template>
                <template v-else>
                  <span class="mystery" aria-label="Starter caché">?</span>
                  <span>Surprise</span>
                </template>
              </div>
              <p v-if="!preview.length && !previewError" class="dim">Choisis une ROM compatible.</p>
              <p v-if="previewError" class="error-text">{{ previewError }}</p>
            </div>
          </div>

          <div class="panel card">
            <h3>Seed</h3>
            <div class="seed">
              <input v-model.number="seed" type="number" min="0" class="input" />
              <button class="btn icon" title="Nouvelle seed" @click="seed = newSeed()">🎲</button>
            </div>
            <div class="share">
              <button class="btn" @click="shareCode">{{ copied ? "Copié !" : "Copier le code de partage" }}</button>
            </div>
            <div class="seed">
              <input v-model="shareInput" class="input" placeholder="Coller un code KLD1-…" :class="{ invalid: shareError }" @keydown.enter="importCode" />
              <button class="btn" :disabled="!shareInput" @click="importCode">Importer</button>
            </div>
          </div>

          <button class="recap" :title="changes.map((c) => c.text).join('\n') || 'Réglages par défaut'" @click="tab = 'general'">
            <span>{{ changes.length ? `${modifs(changes.length)} par rapport aux réglages par défaut` : "Réglages par défaut" }}</span>
            <small>Voir le résumé</small>
          </button>

          <button class="btn btn-primary generate" :disabled="!selected || !supported(selected.game?.id) || running" @click="generate">
            {{ running ? "Génération…" : !isCtr ? "Générer la ROM" : settings.ctrOutput === "layered_fs" ? "Générer le mod 3DS" : "Générer la ROM 3DS" }}
          </button>

          <div v-if="runError" class="panel card error">{{ runError }}</div>

          <Transition name="pop">
            <div v-if="outcome" class="panel card done">
              <h3>✨ {{ lastWasCtr ? "Mod prêt !" : "ROM prête !" }}</h3>
              <p class="dim">{{ outcome.wildSlots }} Pokémon sauvages et {{ outcome.trainerPokemon }} Pokémon de dresseurs modifiés.</p>
              <template v-if="lastWasCtr && ctrResult">
                <p v-if="ctrResult.image" class="dim note">
                  ROM : <code class="path">{{ ctrResult.image }}</code><br />
                  Ouvre-la directement dans Azahar ou Citra (fichier déchiffré, pour émulateur).
                </p>
                <p v-if="ctrResult.romfs" class="dim note">
                  LayeredFS : <code class="path">{{ ctrResult.romfs }}</code><br />
                  {{ target === "luma" ? "Copie le dossier « luma » à la racine de ta carte SD et active « Game patching » dans Luma." : "Place le dossier du title ID dans le dossier « mods » de ton émulateur." }}
                </p>
              </template>
              <div class="done-actions">
                <button v-if="outputPath" class="btn" @click="revealItemInDir(outputPath)">Ouvrir le dossier</button>
                <button class="btn" @click="showLog = true">Voir le journal</button>
              </div>
              <PlayPanel
                :platform="lastWasCtr ? '3ds' : 'nds'"
                :rom="lastWasCtr ? (ctrResult?.image ?? playBase) : outputPath"
                :mod-romfs="lastWasCtr && !ctrResult?.image ? (ctrResult?.romfs ?? null) : null"
              />
            </div>
          </Transition>
        </aside>
      </div>
    </template>

    <div v-if="showLog && outcome" class="modal" @click.self="showLog = false">
      <div class="panel log">
        <header>
          <h3>Journal de randomisation</h3>
          <button class="btn" @click="showLog = false">Fermer</button>
        </header>
        <pre>{{ outcome.log }}</pre>
      </div>
    </div>
  </section>
</template>

<style scoped>
.rando {
  max-width: 1680px;
  margin: 0 auto;
}

h1 {
  font-size: 34px;
}

h3 {
  margin-bottom: 12px;
  font-size: 15px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
  font-size: 16px;
}

.empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 24px;
}

.roms {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin: 20px 0;
}

.rom {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 12px 16px;
  outline: 2px solid transparent;
  outline-offset: 2px;
  text-align: left;
}

.rom.active {
  outline-color: var(--accent);
}

.rom:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.rom small {
  color: var(--text-dim);
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) clamp(320px, 24vw, 380px);
  gap: 24px;
  align-items: start;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.section {
  padding: 18px 20px;
}

/* Barre d'onglets façon éditeur de sauvegardes : pastille blanche, halo cyan. */
.tabbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tabs {
  display: flex;
  flex: 1;
  min-width: 0;
  gap: 4px;
  padding: 4px;
  overflow-x: auto;
  scrollbar-width: none;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--text) 5%, transparent);
}

.tabs::-webkit-scrollbar {
  display: none;
}

/* Une seule ligne : les onglets se partagent la largeur et défilent si elle manque. */
.tabs button {
  display: inline-flex;
  flex: 1 0 auto;
  justify-content: center;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-weight: 600;
  font-size: 13px;
  white-space: nowrap;
  transition: background 0.15s, color 0.15s;
}

.tabs button:hover {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.tabs button.on {
  background: var(--text);
  color: var(--bg);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-2) 60%, transparent);
}

.badge {
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent-2) 30%, transparent);
  color: var(--text);
  font-size: 11px;
  font-weight: 700;
}

.tabs button.on .badge {
  background: color-mix(in srgb, var(--accent-2) 35%, transparent);
  color: var(--bg);
}

.cap {
  display: inline-grid;
  place-items: center;
  min-width: 24px;
  height: 22px;
  padding: 0 6px;
  border-radius: 6px;
  background: var(--text);
  color: var(--bg);
  font: 700 11px/1 var(--font);
  cursor: pointer;
}

.tab-intro {
  margin: -2px 4px 0;
  color: var(--text-dim);
  font-size: 13.5px;
}

.presets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 10px;
}

.preset {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--text) 4%, transparent);
  color: var(--text);
  text-align: left;
  transition: background 0.15s, border-color 0.15s;
}

.preset:hover {
  border-color: var(--accent);
  background: var(--panel-hover);
}

.preset small {
  color: var(--text-dim);
  line-height: 1.35;
}

.summary-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.summary-head h3 {
  margin: 0;
}

.count {
  margin-left: 6px;
  font-size: 13px;
  font-weight: 500;
}

.btn.small {
  padding: 6px 12px;
  font-size: 13px;
}

.summary {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 14px 0 0;
  padding: 0;
  list-style: none;
}

.summary li {
  display: flex;
  align-items: baseline;
  gap: 10px;
  font-size: 14px;
}

.summary-tab {
  flex: none;
  min-width: 118px;
  padding: 2px 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
  text-align: center;
}

.summary-tab:hover {
  color: var(--text);
  border-color: var(--accent);
}

.recap {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 10px 14px;
  border: 1px dashed var(--border);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  text-align: left;
}

.recap small {
  color: var(--text-dim);
  font-weight: 500;
}

.recap:hover {
  border-color: var(--accent);
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 24px;
  margin-top: 14px;
}

.row.first {
  margin-top: 4px;
}

.slider {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-dim);
}

.slider strong {
  min-width: 80px;
  color: var(--text);
}

.slider input {
  accent-color: var(--accent-2);
}

.side {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card {
  padding: 18px;
}

.starters {
  display: flex;
  justify-content: space-around;
  min-height: 110px;
}

.starter {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  font-weight: 600;
  animation: rise 0.35s ease both;
}

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.card-head h3 {
  margin: 0;
}

/* Starter caché : une Poké Ball stylisée à la place du sprite. */
.mystery {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  margin: 12px;
  border-radius: 50%;
  border: 3px solid color-mix(in srgb, var(--text) 70%, transparent);
  background: linear-gradient(to bottom, #e3463f 0 46%, color-mix(in srgb, var(--text) 70%, transparent) 46% 54%, #f2f2f2 54%);
  color: #1d1d1d;
  font-size: 22px;
  font-weight: 800;
  text-shadow: 0 0 6px #fff;
}

.starter.hidden span:last-child {
  color: var(--text-dim);
}

.starter:nth-child(2) {
  animation-delay: 0.06s;
}

.starter:nth-child(3) {
  animation-delay: 0.12s;
}

@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.9);
  }
}

.seed {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.share {
  margin-top: 8px;
}

.share .btn {
  width: 100%;
  justify-content: center;
}

.input {
  flex: 1;
  min-width: 0;
  padding: 9px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font: inherit;
  outline: none;
}

.input:focus {
  border-color: var(--accent);
}

.input.invalid {
  border-color: var(--danger);
}

.icon {
  padding: 8px 12px;
}

.generate {
  justify-content: center;
  padding: 16px;
  font-size: 16px;
  font-weight: 700;
}

.error,
.error-text {
  color: var(--danger);
}

.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.shiny-input {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-dim);
  font-weight: 600;
}

.shiny-input .input {
  width: 110px;
  flex: none;
}

.chips-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip-btn {
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
}

.chip-btn:hover:not(:disabled) {
  color: var(--text);
  background: var(--panel-hover);
}

.chip-btn.on {
  background: var(--prism);
  color: var(--on-accent);
  border-color: transparent;
}

:root[data-theme="lagon"] .chip-btn.on {
  background: #fff;
}

.chip-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.warn-text {
  margin: 10px 0 0;
  color: var(--warn);
  font-size: 13px;
  line-height: 1.45;
}

.soon {
  margin-left: 8px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 500;
}

.row-label {
  color: var(--text-dim);
  font-weight: 600;
}

.kanto-title {
  margin-top: 26px;
}

.custom-starters {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
  margin-top: 14px;
}

.custom-starters label {
  display: flex;
  align-items: center;
  gap: 4px;
}

.note {
  margin: 12px 0 0;
  font-size: 13px;
  line-height: 1.45;
}

.done h3 {
  font-size: 18px;
}

.row.first {
  margin-top: 4px;
}

.note .path {
  font-size: 12px;
  word-break: break-all;
}

.done-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.pop-enter-active {
  transition: opacity 0.3s, transform 0.3s cubic-bezier(0.2, 0.9, 0.3, 1.3);
}

.pop-enter-from {
  opacity: 0;
  transform: scale(0.95);
}

.modal {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: grid;
  place-items: center;
  padding: 40px;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
}

.log {
  display: flex;
  flex-direction: column;
  width: min(1000px, 100%);
  max-height: 100%;
  padding: 18px;
  background: var(--bg);
}

.log header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.log pre {
  overflow: auto;
  margin: 12px 0 0;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  user-select: text;
  white-space: pre-wrap;
}
</style>
