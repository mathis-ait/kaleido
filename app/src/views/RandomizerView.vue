<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Banner from "../components/Banner.vue";
import Dialog from "../components/Dialog.vue";
import Icon from "../components/Icon.vue";
import Segmented from "../components/Segmented.vue";
import Sprite from "../components/Sprite.vue";
import Tip from "../components/Tip.vue";
import Toggle from "../components/Toggle.vue";
import PlayPanel from "../play/PlayPanel.vue";
import { library } from "../library";
import { deleteUserPreset, saveUserPreset, settingsOf, userPresets, type AdventurePreset } from "../presets";
import { nav } from "../nav";
import { open } from "@tauri-apps/plugin-dialog";
import {
  RANDOMIZABLE,
  isKaleidoRom,
  isRom,
  romExt,
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

const NDS_SHINY_PRESETS = [
  { odds: 8192, label: "Normal" },
  { odds: 4096, label: "×2" },
  { odds: 1024, label: "×8" },
  { odds: 512, label: "×16" },
  { odds: 257, label: "Max (1/257)" },
  { odds: 1, label: "Tous" },
];
/** 3DS : le taux normal est 1 / 4 096 ; « Normal » garde la valeur par défaut des réglages. */
const CTR_SHINY_PRESETS = [
  { odds: 8192, label: "Normal" },
  { odds: 1365, label: "×3 (Charme Chroma)" },
  { odds: 512, label: "×8" },
  { odds: 256, label: "×16" },
  { odds: 100, label: "1 / 100" },
  { odds: 1, label: "Tous" },
];
const shinyPresets = computed(() => (isCtr.value ? CTR_SHINY_PRESETS : NDS_SHINY_PRESETS));
const shinyPresetOn = (odds: number) => {
  const n = Number(settings.shinyOdds) || 8192;
  return isCtr.value && odds === 8192 ? n >= 4096 : n === odds;
};

/** 3DS, même calcul que le moteur (data/shiny_ctr.rs) : N tirages de PID, N exprimable
 *  par `mov r0, #N` en ARM (un octet tourné d'un nombre pair de bits). */
const armImmediate = (v: number) => {
  for (let rot = 0; rot < 16; rot++) {
    const r = 2 * rot;
    const imm = r === 0 ? v : ((v << r) | (v >>> (32 - r))) >>> 0;
    if (imm <= 0xff) return true;
  }
  return false;
};
const ctrRerolls = (odds: number): number | "always" | null => {
  if (odds <= 1) return "always";
  if (odds >= 4096) return null;
  const n = Math.min(0x4000, Math.max(1, Math.round(Math.log(1 - 1 / odds) / Math.log(4095 / 4096))));
  for (let d = 0; d <= n; d++) {
    for (const v of [n - d, n + d]) if (v >= 1 && v <= 0x4000 && armImmediate(v)) return v === 1 ? null : v;
  }
  return null;
};
const ctrShinyLabel = (odds: number) => {
  const n = ctrRerolls(odds);
  if (n === "always") return "tous les Pokémon";
  if (n === null) return "1 / 4096 (normal)";
  return `1 / ${Math.max(1, Math.round(1 / (1 - Math.pow(4095 / 4096, n))))}`;
};

/** Même calcul que le moteur : seuil arrondi entre 1 et 255 (sur 65 536). */
const shinyLabel = computed(() => {
  const n = Number(settings.shinyOdds) || 8192;
  if (isCtr.value) return ctrShinyLabel(n);
  if (n <= 1) return "tous les Pokémon";
  const threshold = Math.min(255, Math.max(1, Math.round(65536 / n)));
  return `1 / ${Math.floor(65536 / threshold)}`;
});
/** Le taux sera-t-il vraiment modifié ? (3DS : rien sous 1 / 4 096) */
const shinyChanged = computed(() => {
  const n = Number(settings.shinyOdds) || 8192;
  return isCtr.value ? ctrRerolls(n) !== null : n !== 8192;
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
const isGba = computed(() => selected.value?.platform === "gba" || selected.value?.platform === "gb");
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

// Réglages repris de « Nouvelle aventure » (Personnaliser) : partent des réglages du moteur.
function takeAdventure() {
  const a = nav.randomizerSettings;
  if (!a) return;
  Object.assign(settings, JSON.parse(JSON.stringify(a.settings)) as RandomizerSettings);
  seed.value = a.seed;
  customNames.value = ["", "", ""];
  nav.randomizerSettings = null;
}
watch(() => nav.randomizerSettings, takeAdventure);

// ROM envoyée depuis la bibliothèque alors que le Randomizer est déjà ouvert.
watch(
  () => nav.randomizerRom,
  (p) => {
    if (p) romPath.value = p;
  },
);

onMounted(async () => {
  romPath.value = nav.randomizerRom ?? roms.value.find((r) => supported(r.game?.id))?.path ?? null;
  takeAdventure();
  presets.value = await invoke<Preset[]>("randomizer_presets").catch(() => []);
  void loadMyPresets();
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
  { id: "shiny", label: "Chromatiques", intro: "La probabilité de croiser un Pokémon chromatique." },
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
/** Onglets cachés de chaque côté : le bord correspondant s'estompe au lieu de couper un libellé net. */
const tabEdges = reactive({ start: false, end: false });
function updateTabEdges() {
  const el = tabsEl.value;
  if (!el) return;
  tabEdges.start = el.scrollLeft > 1;
  tabEdges.end = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
}
let tabsObserver: ResizeObserver | undefined;
onMounted(() => {
  tabsObserver = new ResizeObserver(updateTabEdges);
  watch(
    tabsEl,
    (el) => {
      tabsObserver?.disconnect();
      if (el) tabsObserver?.observe(el);
      updateTabEdges();
    },
    { immediate: true },
  );
});
onBeforeUnmount(() => tabsObserver?.disconnect());
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

  add("shiny", shinyChanged.value, `Chromatiques : ${shinyLabel.value}`);
  return out;
});
const changeCount = (id: TabId) => changes.value.filter((c) => c.tab === id).length;
// Les pastilles « n modifs » élargissent la barre sans la redimensionner.
watch(
  () => TABS.map((t) => changeCount(t.id)).join(),
  () => nextTick(updateTabEdges),
);
const modifs = (n: number) => `${n} modif${n > 1 ? "s" : ""}`;
const tabLabel = (id: TabId) => TABS.find((t) => t.id === id)?.label ?? id;

// --- Préréglages (fournis par le moteur)

const presets = ref<Preset[]>([]);
function applyPreset(p: Preset) {
  // Les réglages absents (Pokémon de Kanto inchangés) sont omis : on repart des valeurs par défaut.
  Object.assign(settings, { kantoStarters: "unchanged", customKantoStarters: [0, 0, 0] }, JSON.parse(JSON.stringify(p.settings)) as RandomizerSettings);
  customNames.value = ["", "", ""];
}

// --- Preset personnel (carte de plus dans « Nouvelle aventure »)

const savingPreset = ref(false);
const presetName = ref("");
const presetSaved = ref<string | null>(null);

const presetError = ref<string | null>(null);
const myPresets = ref<AdventurePreset[]>([]);

async function loadMyPresets() {
  myPresets.value = await userPresets();
}

async function savePreset() {
  const name = presetName.value.trim();
  if (!name) return;
  presetError.value = null;
  try {
    await saveUserPreset(name, JSON.parse(JSON.stringify(settings)) as RandomizerSettings);
  } catch (e) {
    presetError.value = `Enregistrement impossible : ${e}`;
    return;
  }
  savingPreset.value = false;
  presetSaved.value = name;
  presetName.value = "";
  await loadMyPresets();
  setTimeout(() => (presetSaved.value = null), 3000);
}

async function applyMyPreset(p: AdventurePreset) {
  const full = await settingsOf(p, isCtr.value ? "3ds" : "nds");
  Object.assign(settings, JSON.parse(JSON.stringify(full)) as RandomizerSettings);
  customNames.value = ["", "", ""];
}

async function removeMyPreset(p: AdventurePreset) {
  await deleteUserPreset(p.id).catch((e) => (presetError.value = String(e)));
  await loadMyPresets();
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
// Le message d'erreur disparaît dès qu'on corrige le code.
watch(shareInput, () => (shareError.value = false));

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
  const base = selected.value.path.replace(/\.(nds|gba|gbc|gb)$/i, "");
  const ext = romExt(selected.value.path);
  const output = await save({
    title: "Enregistrer la ROM randomisée",
    defaultPath: `${base} - Kaleido ${seed.value}.${ext}`,
    filters: [{ name: ext === "nds" ? "ROM Nintendo DS" : ext === "gba" ? "ROM Game Boy Advance" : "ROM Game Boy", extensions: [ext] }],
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

// Déclaration de fonction (hissée) : le résumé des modifs l'appelle dès l'initialisation.
function levelLabel(p: number) {
  return p === 100 ? "inchangés" : `${p > 100 ? "+" : ""}${p - 100} %`;
}
</script>

<template>
  <section class="rando">
    <header class="page-head">
      <div>
        <h1>Randomizer</h1>
        <p class="lead">Une nouvelle aventure à chaque seed. Partage le code pour que tes amis jouent exactement la même.</p>
      </div>
      <button class="sv-btn" title="Revenir aux réglages par défaut" @click="reset">Réinitialiser</button>
    </header>

    <!-- Choix de la ROM -->
    <div v-if="!roms.length" class="sv-panel empty">
      <p>Ajoute d'abord une ROM Pokémon DS ou 3DS (de Diamant / Perle à Ultra-Soleil / Ultra-Lune) dans la bibliothèque.</p>
      <button class="sv-btn solid" @click="nav.view = 'home'">Ajouter une ROM</button>
    </div>

    <template v-else>
      <div class="roms">
        <button
          v-for="r in roms"
          :key="r.path"
          class="rom sv-card"
          :class="{ on: romPath === r.path }"
          :aria-pressed="romPath === r.path"
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
            <button type="button" class="cap" tabindex="-1" title="Onglet précédent (Q)" aria-label="Onglet précédent" @click="stepTab(-1)">Q</button>
            <div class="tabs-frame">
              <nav
                ref="tabsEl"
                class="tabs"
                :class="{ 'more-start': tabEdges.start, 'more-end': tabEdges.end }"
                role="tablist"
                aria-label="Réglages du randomizer"
                @scroll.passive="updateTabEdges"
              >
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
            </div>
            <button type="button" class="cap" tabindex="-1" title="Onglet suivant (E)" aria-label="Onglet suivant" @click="stepTab(1)">E</button>
          </div>
          <p class="tab-intro">{{ currentTab.intro }}</p>

          <!-- Général : préréglages, sortie 3DS, résumé -->
          <template v-if="tab === 'general'">
          <div v-if="isCtr" class="section sv-panel">
            <h3>Sortie 3DS</h3>
            <div class="row field first">
              <span class="sv-label">
                Format
                <Tip term="randomizer.ctrOutput" />
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
            <div v-if="settings.ctrOutput !== 'rom3ds'" class="row field">
              <span class="sv-label">Dossier pour</span>
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

            <div v-if="presets.length" class="section sv-panel">
              <h3>
                Préréglages
                <Tip term="randomizer.presets" />
              </h3>
              <div class="presets">
                <button v-for="p in presets" :key="p.id" class="preset sv-card" @click="applyPreset(p)">
                  <strong>{{ p.name }}</strong>
                  <small>{{ p.description }}</small>
                </button>
                <div v-for="p in myPresets" :key="p.id" class="preset mine sv-card">
                  <button type="button" class="apply" @click="applyMyPreset(p)">
                    <strong>{{ p.name }}</strong>
                    <small>{{ p.changes.map((c) => c.label).join(" · ") }}</small>
                  </button>
                  <button type="button" class="remove" :aria-label="`Supprimer le preset ${p.name}`" title="Supprimer" @click="removeMyPreset(p)">
                    <Icon name="trash" :size="14" />
                  </button>
                </div>
              </div>
              <p class="save-preset">
                <button type="button" class="sv-btn small" @click="savingPreset = true"><Icon name="save" :size="14" /> Enregistrer comme preset</button>
                <small v-if="presetSaved" class="dim">« {{ presetSaved }} » enregistré, ici et dans Nouvelle aventure</small>
                <Tip term="adventure.adventure" />
              </p>
            </div>
            <Dialog v-model="savingPreset" title="Enregistrer comme preset" icon="save" :width="440">
              <p class="sv-help">Ces réglages deviennent une carte de plus dans les préréglages, ici et dans « Nouvelle aventure » depuis la bibliothèque.</p>
              <input v-model="presetName" class="sv-input" placeholder="Nom du preset" aria-label="Nom du preset" autofocus @keydown.enter="savePreset" />
              <p v-if="presetError" class="sv-help preset-error">{{ presetError }}</p>
              <template #foot>
                <span class="grow" />
                <button type="button" class="sv-btn" @click="savingPreset = false">Annuler</button>
                <button type="button" class="sv-btn solid" :disabled="!presetName.trim()" @click="savePreset">Enregistrer</button>
              </template>
            </Dialog>

            <div class="section sv-panel">
              <div class="summary-head">
                <h3>Résumé <span class="dim count">{{ changes.length ? modifs(changes.length) : "réglages par défaut" }}</span></h3>
                <button v-if="changes.length" class="sv-btn small" @click="reset">Tout réinitialiser</button>
              </div>
              <p v-if="!changes.length" class="dim note">
                Rien n'a été modifié : starters Plante · Feu · Eau, sauvages par zone et dresseurs aléatoires de puissance
                similaire. Parcours les onglets pour personnaliser ta partie.
              </p>
              <ul v-else class="summary">
                <li v-for="(c, i) in changes" :key="i">
                  <button class="sv-chip summary-tab" :title="`Aller à l'onglet ${tabLabel(c.tab)}`" @click="tab = c.tab">{{ tabLabel(c.tab) }}</button>
                  <span>{{ c.text }}</span>
                </li>
              </ul>
            </div>
          </template>

          <!-- Pokémon : statistiques, types, talents, évolutions -->
          <template v-else-if="tab === 'pokemon'">
            <div class="section sv-panel">
              <h3>Statistiques, types et talents</h3>
              <Segmented v-model="settings.stats" :options="STATS_OPTS" />
              <div class="toggles">
                <Toggle v-model="settings.randomTypes" label="Types aléatoires" term="randomizer.randomTypes" />
                <Toggle v-model="settings.randomAbilities" label="Talents aléatoires" term="randomizer.randomAbilities" />
                <Toggle v-model="settings.noLegendaries" label="Sans légendaires" term="randomizer.noLegendaries" />
              </div>
            </div>

            <div class="section sv-panel">
              <h3>Évolutions, attaques et capture</h3>
              <div class="toggles first">
                <Toggle
                  v-model="settings.easyEvolutions"
                  label="Évolutions sans échange"
                  term="randomizer.easyEvolutions"
                />
                <Toggle
                  v-model="settings.randomMovesets"
                  label="Attaques apprises aléatoires"
                  term="randomizer.randomMovesets"
                />
              </div>
              <div class="row field">
                <span class="sv-label">Capture</span>
                <Segmented v-model="settings.catchRate" :options="CATCH_OPTS" />
              </div>
            </div>
          </template>

          <!-- Starters -->
          <div v-else-if="tab === 'starters'" class="section sv-panel">
            <h3>Starters</h3>
            <Segmented v-model="settings.starters" :options="STARTER_OPTS" />
            <div v-if="settings.starters === 'custom'" class="custom-starters">
              <label v-for="(_, i) in customNames" :key="i">
                <Sprite v-if="settings.customStarters[i]" :id="settings.customStarters[i]" :size="44" />
                <input
                  v-model="customNames[i]"
                  class="sv-input"
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
                <Tip term="randomizer.kantoStarters" />
              </h3>
              <Segmented v-model="settings.kantoStarters" :options="STARTER_OPTS" />
              <div v-if="settings.kantoStarters === 'custom'" class="custom-starters">
                <label v-for="(_, i) in customKantoNames" :key="i">
                  <Sprite v-if="settings.customKantoStarters?.[i]" :id="settings.customKantoStarters[i]" :size="44" />
                  <input
                    v-model="customKantoNames[i]"
                    class="sv-input"
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
          <div v-else-if="tab === 'wild'" class="section sv-panel">
            <h3>Pokémon sauvages</h3>
            <Segmented v-model="settings.wild" :options="WILD_OPTS" />
            <div class="toggles">
              <Toggle v-model="settings.wildSimilarStrength" label="Puissance similaire" term="randomizer.wildSimilarStrength" />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.wildLevelPercent) }}</strong>
                <input v-model.number="settings.wildLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
          </div>

          <!-- Dresseurs -->
          <div v-else-if="tab === 'trainers'" class="section sv-panel">
            <h3>Dresseurs</h3>
            <Segmented v-model="settings.trainers" :options="TRAINER_OPTS" />
            <div class="toggles">
              <Toggle v-model="settings.trainersSimilarStrength" label="Puissance similaire" term="randomizer.trainersSimilarStrength" />
              <label class="slider">
                Niveaux <strong>{{ levelLabel(settings.trainerLevelPercent) }}</strong>
                <input v-model.number="settings.trainerLevelPercent" type="range" min="50" max="200" step="5" />
              </label>
            </div>
            <div class="toggles">
              <Toggle
                v-model="settings.trainerEvolutions"
                label="Pokémon évolués selon leur niveau"
                term="randomizer.trainerEvolutions"
              />
              <Toggle v-model="settings.trainerMaxIvs" label="IV au maximum" term="randomizer.trainerMaxIvs" />
            </div>
          </div>

          <!-- CT & capacités -->
          <div v-else-if="tab === 'moves'" class="section sv-panel">
            <h3>CT &amp; capacités <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span></h3>
            <div class="toggles first">
              <Toggle v-model="settings.moves.randomTms" label="CT aléatoires" term="randomizer.randomTms" />
              <Toggle v-model="settings.moves.randomTutors" label="Maîtres des capacités aléatoires" term="randomizer.randomTutors" />
              <Toggle v-model="settings.moves.keepFieldMoves" label="Garder les attaques de terrain" term="randomizer.keepFieldMoves" />
              <Toggle v-model="settings.moves.noGameBreaking" label="Sans Sonicboom / Draco-Rage" term="randomizer.noGameBreaking" />
            </div>
            <div class="row field">
              <span class="sv-label">Compatibilité CT</span>
              <Segmented v-model="settings.moves.tmCompat" :options="TM_COMPAT_OPTS" />
            </div>
            <div class="row field">
              <span class="sv-label">Maîtres des capacités</span>
              <Segmented v-model="settings.moves.tutorCompat" :options="TUTOR_COMPAT_OPTS" />
            </div>
            <div class="toggles">
              <Toggle v-model="settings.moves.fullHmCompat" label="Toutes les CS pour tous" term="randomizer.fullHmCompat" />
              <Toggle v-model="settings.moves.followEvolutions" label="Les évolutions héritent" term="randomizer.followEvolutions" />
              <Toggle v-model="settings.moves.levelupSanity" label="Garder les CT des attaques apprises" term="randomizer.levelupSanity" />
            </div>
            <p v-if="isGba" class="dim note">Sur Game Boy et GBA, les maîtres des capacités et l'héritage par évolution ne sont pas encore pris en charge.</p>
          </div>

          <!-- Objets & boutiques -->
          <div v-else-if="tab === 'items'" class="section sv-panel">
            <h3>Objets &amp; boutiques <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span><span v-else-if="isGba" class="soon">Pas encore sur Game Boy / GBA</span></h3>
            <div class="row field first">
              <span class="sv-label">Objets au sol</span>
              <Segmented v-model="settings.items.fieldItems" :options="FIELD_OPTS" />
            </div>
            <div class="row field">
              <span class="sv-label">Boutiques</span>
              <Segmented v-model="settings.items.shops" :options="SHOP_OPTS" />
            </div>
            <div class="toggles">
              <Toggle v-model="settings.items.banBadFieldItems" label="Pas d'objets inutiles" term="randomizer.banBadFieldItems" />
              <Toggle v-model="settings.items.guaranteeEvolutionItems" label="Pierres d'évolution en vente" term="randomizer.guaranteeEvolutionItems" />
              <Toggle v-model="settings.items.guaranteeXItems" label="Objets X en vente" term="randomizer.guaranteeXItems" />
              <Toggle v-model="settings.items.banOpShopItems" label="Pas d'objets trop forts en boutique" term="randomizer.banOpShopItems" />
              <Toggle v-model="settings.items.noRareCandy" label="Sans Super Bonbon" term="randomizer.noRareCandy" />
              <Toggle v-model="settings.items.noMasterBall" label="Sans Master Ball" term="randomizer.noMasterBall" />
            </div>
            <p class="dim note">Objets clés et CS ne bougent jamais ; une CT est toujours remplacée par une CT.</p>
          </div>

          <!-- Pokémon fixes & échanges -->
          <div v-else-if="tab === 'statics'" class="section sv-panel">
            <h3>Pokémon fixes &amp; échanges <span v-if="isCtr" class="soon">DS uniquement pour l'instant</span></h3>
            <div class="row field first">
              <span class="sv-label">Fixes et dons <Tip term="randomizer.statics" /></span>
              <Segmented v-model="settings.statics.mode" :options="STATIC_OPTS" />
            </div>
            <div v-if="settings.statics.mode !== 'unchanged'" class="row">
              <label class="slider">
                Niveaux <strong>{{ settings.statics.levelModifier >= 0 ? "+" : "" }}{{ settings.statics.levelModifier }} %</strong>
                <input v-model.number="settings.statics.levelModifier" type="range" min="-50" max="50" step="5" />
              </label>
              <Tip term="randomizer.staticLevels" />
            </div>
            <div class="row field">
              <span class="sv-label">Échanges <Tip term="randomizer.trades" /></span>
              <Segmented v-model="settings.statics.trades" :options="TRADE_OPTS" />
            </div>
            <div v-if="settings.statics.trades !== 'unchanged'" class="toggles">
              <Toggle v-model="settings.statics.tradeRandomItems" label="Objets tenus aléatoires" term="randomizer.tradeRandomItems" />
              <Toggle v-model="settings.statics.tradeRandomIvs" label="IV aléatoires" term="randomizer.tradeRandomIvs" />
            </div>
            <p v-if="isGba" class="dim note">Sur Game Boy et GBA, les échanges en jeu ne changent pas encore.</p>
          </div>

          <!-- Chromatiques -->
          <div v-else-if="tab === 'shiny'" class="section sv-panel">
            <h3>Chromatiques <span v-if="isGba" class="soon">Pas encore sur Game Boy / GBA</span></h3>
            <div class="row shiny-row first">
              <label class="shiny-input">
                1 chance sur
                <input v-model.number="settings.shinyOdds" class="sv-input" type="number" min="1" max="65536" />
              </label>
              <div class="chips-row">
                <button
                  v-for="q in shinyPresets"
                  :key="q.odds"
                  type="button"
                  class="sv-chip"
                  :class="{ on: shinyPresetOn(q.odds) }"
                  :aria-pressed="shinyPresetOn(q.odds)"
                  @click="settings.shinyOdds = q.odds"
                >
                  {{ q.label }}
                </button>
              </div>
            </div>
            <p class="dim note">
              <template v-if="isCtr">
                Taux réel : <strong>{{ shinyLabel }}</strong>. Modifie le programme du jeu (code.bin
                <Tip term="randomizer.codeBin" />)
                pour les Pokémon sauvages, fixes et offerts. Sur 3DS, on ne peut pas descendre sous 1 / 4 096, et le Charme Chroma
                n'a plus d'effet (le taux choisi le remplace).
              </template>
              <template v-else>
                Taux réel : <strong>{{ shinyLabel }}</strong>. Modifie la fonction du jeu qui décide si un Pokémon est chromatique
                (sauvages, dons, œufs, dresseurs).
              </template>
            </p>
            <p v-if="isCtr && settings.shinyOdds > 1 && settings.shinyOdds < 4096 && shinyLabel.endsWith('(normal)')" class="dim note">
              Ce taux est trop proche du taux normal : rien ne sera modifié.
            </p>
            <p v-if="isCtr && settings.shinyOdds <= 1" class="dim note">
              Les Pokémon qui ne doivent jamais être chromatiques (cadeaux et légendaires protégés) le restent.
            </p>
            <Banner v-if="!isCtr && settings.shinyOdds <= 1" tone="warn" class="note-banner">
              Expérimental : certains événements relancent le tirage tant que le Pokémon est chromatique (Pokémon qui ne
              doivent jamais l'être, comme Reshiram / Zekrom). Avec 100 %, ces scènes peuvent bloquer le jeu.
            </Banner>
          </div>
        </div>

        <!-- Colonne de droite : aperçu et génération -->
        <aside class="side">
          <div class="sv-panel card">
            <div class="card-head">
              <h3>Tes starters</h3>
              <Toggle v-if="preview.length" v-model="showStarters" label="Voir" term="randomizer.showStarters" />
            </div>
            <div class="starters">
              <div v-for="(p, i) in preview" :key="`${p.id}-${i}`" class="starter" :class="{ hidden: !showStarters }">
                <template v-if="showStarters">
                  <Sprite :id="p.id" variant="model" :size="88" />
                  <span>{{ p.name }}</span>
                </template>
                <template v-else>
                  <span class="mystery" aria-label="Starter caché"><Icon name="ball" :size="38" /></span>
                  <span>Surprise</span>
                </template>
              </div>
              <p v-if="!preview.length && !previewError" class="dim">Choisis une ROM compatible.</p>
              <p v-if="previewError" class="error-text">{{ previewError }}</p>
            </div>
          </div>

          <div class="sv-panel card">
            <h3>Seed</h3>
            <div class="seed">
              <input v-model.number="seed" type="number" min="0" class="sv-input" aria-label="Seed" />
              <button class="sv-btn" title="Nouvelle seed" aria-label="Nouvelle seed" @click="seed = newSeed()"><Icon name="dice" :size="16" /></button>
            </div>
            <button class="sv-btn share" @click="shareCode">
              <Icon :name="copied ? 'check' : 'copy'" :size="14" /> {{ copied ? "Copié" : "Copier le code de partage" }}
            </button>
            <div class="seed">
              <input
                v-model="shareInput"
                class="sv-input"
                placeholder="Coller un code KLD1-…"
                aria-label="Code de partage"
                :class="{ invalid: shareError }"
                @keydown.enter="importCode"
              />
              <button class="sv-btn" :disabled="!shareInput" @click="importCode">Importer</button>
            </div>
            <p v-if="shareError" class="error-text share-error">Code de partage illisible : vérifie qu'il commence par KLD1- et qu'il est complet.</p>
          </div>

          <button class="recap sv-card" :title="changes.map((c) => c.text).join('\n') || 'Réglages par défaut'" @click="tab = 'general'">
            <span class="recap-text">
              <span>{{ changes.length ? `${modifs(changes.length)} par rapport aux réglages par défaut` : "Réglages par défaut" }}</span>
              <small>Voir le résumé</small>
            </span>
            <Icon name="chevron-right" :size="16" />
          </button>

          <button class="sv-btn solid generate" :disabled="!selected || !supported(selected.game?.id) || running" @click="generate">
            {{ running ? "Génération…" : !isCtr ? "Générer la ROM" : settings.ctrOutput === "layered_fs" ? "Générer le mod 3DS" : "Générer la ROM 3DS" }}
          </button>

          <Banner v-if="runError" tone="danger" :dismiss="() => (runError = null)">{{ runError }}</Banner>

          <Transition name="pop">
            <div v-if="outcome" class="sv-panel card done">
              <h3 class="with-icon"><Icon name="check" :size="18" /> {{ lastWasCtr ? "Mod prêt" : "ROM prête" }}</h3>
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
                <button v-if="outputPath" class="sv-btn" @click="revealItemInDir(outputPath)">Ouvrir le dossier</button>
                <button class="sv-btn" @click="showLog = true">Voir le journal</button>
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

    <Dialog v-model="showLog" title="Journal de randomisation" icon="book" :width="1000">
      <pre v-if="outcome" class="log">{{ outcome.log }}</pre>
    </Dialog>
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
  margin-bottom: var(--sp-3);
  font-size: 15px;
}

.lead,
.dim {
  color: var(--text-dim);
}

.lead {
  font-size: var(--fs-lg);
}

.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--sp-4);
}

.empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-4);
  margin-top: var(--sp-5);
  padding: var(--sp-5) var(--sp-6);
}

.empty p {
  margin: 0;
}

/* Choix de la ROM : cartes partagées (.sv-card), la ROM choisie est cerclée. */
.roms {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin: var(--sp-5) 0;
}

.rom {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  min-width: 150px;
  padding: var(--sp-3) var(--sp-4);
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
  gap: var(--sp-6);
  align-items: start;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.section {
  padding: 18px var(--sp-5);
}

/* Barre d'onglets façon éditeur de sauvegardes : l'onglet actif est inversé, sans halo. */
.tabbar {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.tabs-frame {
  flex: 1;
  min-width: 0;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 5%, transparent);
}

.tabs {
  display: flex;
  gap: var(--sp-1);
  overflow-x: auto;
  scrollbar-width: none;
  border-radius: var(--radius-pill);
  /* L'onglet amené à l'écran ne finit pas sous le bord estompé. */
  scroll-padding-inline: 40px;
}

.tabs::-webkit-scrollbar {
  display: none;
}

/* Onglets cachés d'un côté : ce bord s'estompe au lieu de couper un libellé net. */
.tabs.more-end {
  mask-image: linear-gradient(to right, black calc(100% - 40px), transparent);
}

.tabs.more-start {
  mask-image: linear-gradient(to left, black calc(100% - 40px), transparent);
}

.tabs.more-start.more-end {
  mask-image: linear-gradient(to right, transparent, black 40px, black calc(100% - 40px), transparent);
}

/* Une seule ligne : les onglets se partagent la largeur et défilent si elle manque. */
.tabs button {
  display: inline-flex;
  flex: 1 0 auto;
  justify-content: center;
  align-items: center;
  gap: 6px;
  padding: var(--sp-2) 14px;
  border: none;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--text-dim);
  font-weight: 600;
  font-size: var(--fs-md);
  white-space: nowrap;
  transition: background-color 0.15s, color 0.15s;
}

.tabs button:hover:not(.on) {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.tabs button.on {
  background: var(--text);
  color: var(--bg);
}

/* La barre défile (overflow) : l'anneau de focus passe à l'intérieur pour ne pas être rogné. */
.tabs button:focus-visible {
  outline-offset: -2px;
}

/* Nombre de modifications : teinte du texte de l'onglet, lisible qu'il soit actif ou non. */
.badge {
  padding: 1px 7px;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, currentColor 16%, transparent);
  color: inherit;
  font-size: var(--fs-xs);
  font-weight: 700;
}

.cap {
  display: inline-grid;
  flex: none;
  place-items: center;
  min-width: 24px;
  height: 22px;
  padding: 0 6px;
  border: none;
  border-radius: var(--radius-xs);
  background: var(--text);
  color: var(--bg);
  font: 700 var(--fs-xs) / 1 var(--font);
  transition: background-color 0.15s;
}

.cap:hover {
  background: color-mix(in srgb, var(--text) 80%, var(--bg));
}

.tab-intro {
  margin: -2px var(--sp-1) 0;
  color: var(--text-dim);
  font-size: var(--fs-md);
}

.presets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 10px;
}

.preset {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-3) 14px;
}

.preset small {
  color: var(--text-dim);
  line-height: 1.35;
}

.preset.mine {
  position: relative;
  padding: 0;
}

.preset.mine .apply {
  display: flex;
  flex-direction: column;
  gap: var(--sp-1);
  padding: var(--sp-3) 36px var(--sp-3) 14px;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.preset.mine .remove {
  position: absolute;
  top: 8px;
  right: 8px;
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 6px;
  background: none;
  color: var(--text-dim);
  cursor: pointer;
}

.preset.mine .remove:hover {
  color: var(--text);
  background: var(--panel-hover);
}

.preset-error {
  color: var(--danger);
}

.save-preset {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  margin: var(--sp-3) 0 0;
}

.summary-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
}

.summary-head h3 {
  margin: 0;
}

.count {
  margin-left: 6px;
  font-size: var(--fs-md);
  font-weight: 500;
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
  font-size: var(--fs-base);
}

.summary-tab {
  flex: none;
  justify-content: center;
  min-width: 118px;
}

/* Lignes de réglages : un libellé au-dessus de son sélecteur, pour un alignement constant. */
.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-3) var(--sp-6);
  margin-top: var(--sp-4);
}

.row.field {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--sp-2);
}

/* Interrupteurs en colonnes alignées plutôt qu'en ligne qui se replie au hasard. */
.toggles {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  align-items: center;
  gap: var(--sp-3) var(--sp-6);
  margin-top: var(--sp-4);
}

.row.first,
.toggles.first {
  margin-top: var(--sp-1);
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

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--sp-3);
}

.card-head h3 {
  margin: 0;
}

/* Aperçu des starters : même hauteur, que les sprites soient visibles ou cachés. */
.starters {
  display: flex;
  justify-content: space-around;
  min-height: 110px;
}

.starters > p {
  align-self: center;
  margin: 0;
  font-size: var(--fs-md);
  text-align: center;
}

.starter {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-1);
  min-width: 0;
  font-size: var(--fs-md);
  font-weight: 600;
  text-align: center;
  animation: rise 0.25s ease both;
}

/* Starter caché : une Poké Ball à la place du sprite. */
.mystery {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  margin: var(--sp-3);
  border: 1px solid var(--border);
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  color: var(--text-dim);
}

.starter.hidden span:last-child {
  color: var(--text-dim);
}

.starter:nth-child(2) {
  animation-delay: 0.05s;
}

.starter:nth-child(3) {
  animation-delay: 0.1s;
}

@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
}

.grow {
  flex: 1;
}

.seed {
  display: flex;
  gap: var(--sp-2);
  margin-top: var(--sp-2);
}

.seed .sv-input {
  flex: 1;
}

.share {
  width: 100%;
  margin-top: var(--sp-2);
}

.share-error {
  margin: var(--sp-2) 0 0;
  font-size: var(--fs-sm);
  line-height: 1.4;
}

/* Rappel des modifications : carte cliquable qui mène au résumé (onglet Général). */
.recap {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-3);
  padding: 10px 14px;
  color: var(--text-dim);
}

.recap-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  color: var(--text);
  font-size: var(--fs-md);
  font-weight: 600;
}

.recap-text small {
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 500;
}

.generate {
  padding: 14px;
  font-size: var(--fs-lg);
  font-weight: 700;
}

.with-icon {
  display: flex;
  align-items: center;
  gap: var(--sp-2);
}

.error-text {
  color: var(--danger);
}

.shiny-input {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-dim);
  font-weight: 600;
}

.shiny-input .sv-input {
  width: 110px;
}

.chips-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chips-row .sv-chip {
  padding: 5px 12px;
  font-size: var(--fs-md);
}

.note-banner {
  margin-top: var(--sp-3);
}

.soon {
  margin-left: var(--sp-2);
  color: var(--text-dim);
  font-size: var(--fs-sm);
  font-weight: 500;
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
  gap: var(--sp-1);
  min-width: 0;
}

.note {
  margin: var(--sp-3) 0 0;
  font-size: var(--fs-md);
  line-height: 1.45;
}

.done h3 {
  font-size: 18px;
}

.note .path {
  font-size: var(--fs-sm);
  word-break: break-all;
}

.done-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--sp-2);
  margin-top: var(--sp-3);
}

.pop-enter-active {
  transition: opacity 0.2s, transform 0.2s ease-out;
}

.pop-enter-from {
  opacity: 0;
  transform: translateY(6px);
}

.log {
  margin: 0;
  font-family: "Cascadia Code", Consolas, monospace;
  font-size: var(--fs-sm);
  line-height: 1.5;
  user-select: text;
  white-space: pre-wrap;
}
</style>
