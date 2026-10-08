<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Backdrop from "./Backdrop.vue";
import TitleScene from "./TitleScene.vue";
import PadHints from "./PadHints.vue";
import Icon from "../components/Icon.vue";
import { openRom } from "../editor";
import { UPDATE_LABEL, games as libraryState, hideGame, libraryUi, setCartridgeLook } from "../games";
import { cartridgeBlocker, cartridgeMode, presentation } from "./presentation";
import { FINISHES, WEARS, cartridgeKey, resolveLook, type StoredLook } from "./scene/models";
import { glyphs, layoutOf, type HintAction } from "./scene/hints";
import { removeItem } from "../library";
import { PLATFORM_LABEL, RECOMMENDED, defaultEmulator, emus, formatMo, installs, loadEmulators, play as playGame } from "../play/play";
import NewAdventure from "./NewAdventure.vue";
import { adventure, closeAdventure, openAdventure, writeAdventure } from "./adventure";
import { modsDialog, openMods } from "../play/mods";
import type { Detection } from "../types";
import { isKaleidoRom } from "../types";
import { canRandomize, coverUrl, formatDuration, lastPlayed, launchGame, openCompanionOf, openSaveOf, platformOf, playTime, randomize, statusOf, timeAgo } from "./actions";
import { audio, duckMusic, prefetchMusic, previewMusic, sfx, stopMusic } from "./audio";
import { dominantColor } from "./color";
import { useGamepad, type PadAction } from "./gamepad";

// three.js et la scène : chunk séparé, chargé seulement en mode Cartouche.
const CartridgeScene = defineAsyncComponent(() => import("./scene/CartridgeScene.vue"));

const props = defineProps<{ games: Detection[] }>();

try {
  performance.clearMarks("launcher-open");
  performance.mark("launcher-open");
} catch {
  /* mesure indisponible */
}

// --- Onglets

type Tab = "all" | "gb" | "gba" | "nds" | "3ds" | "switch";
const TABS: { id: Tab; label: string }[] = [
  { id: "all", label: "Tous" },
  { id: "gb", label: "Game Boy" },
  { id: "gba", label: "Game Boy Advance" },
  { id: "nds", label: "Nintendo DS" },
  { id: "3ds", label: "Nintendo 3DS" },
  { id: "switch", label: "Nintendo Switch" },
];
const tab = ref<Tab>("all");
const list = computed(() => props.games.filter((g) => tab.value === "all" || g.platform === tab.value));

// --- Sélection

const index = ref(Math.max(0, list.value.findIndex((g) => g.path === libraryUi.selected)));
const game = computed<Detection | null>(() => list.value[index.value] ?? null);

watch(list, (l) => {
  const keep = l.findIndex((g) => g.path === libraryUi.selected);
  index.value = keep >= 0 ? keep : Math.min(index.value, Math.max(0, l.length - 1));
});

function move(delta: number) {
  const next = Math.min(Math.max(index.value + delta, 0), list.value.length - 1);
  if (next === index.value) {
    sfx("edge");
    return;
  }
  index.value = next;
  sfx("move");
}

function switchTab(delta: number) {
  const i = (TABS.findIndex((t) => t.id === tab.value) + delta + TABS.length) % TABS.length;
  tab.value = TABS[i].id;
  sfx("back");
}

// Couleur du jeu, musique et préchargement des voisins.
const color = ref("hsl(250 80% 66%)");
watch(
  game,
  async (g) => {
    libraryUi.selected = g?.path ?? null;
    previewMusic(g);
    prefetchMusic(list.value[index.value + 1]);
    prefetchMusic(list.value[index.value - 1]);
    const url = g ? coverUrl(g) : null;
    const c = url ? await dominantColor(url) : null;
    if (g === game.value && c) color.value = c;
  },
  { immediate: true },
);

watch(
  () => audio.music,
  (on) => (on ? previewMusic(game.value, 0) : stopMusic()),
);

// --- Carrousel

function itemStyle(i: number) {
  const k = i - index.value;
  const abs = Math.abs(k);
  const sign = Math.sign(k);
  // Pendant un glisser, tout le carrousel suit la souris (décalage partiel avant le pas suivant).
  const x = (k === 0 ? 0 : sign * (250 + (abs - 1) * 168)) + dragShift.value;
  const rot = k === 0 ? 0 : -sign * 44;
  const z = k === 0 ? 40 : -140 - abs * 26;
  const scale = k === 0 ? 1.16 : 0.9;
  return {
    transform: `translateX(${x}px) translateZ(${z}px) rotateY(${rot}deg) scale(${scale})`,
    zIndex: 100 - abs,
    opacity: abs > 6 ? 0 : 1 - Math.max(0, abs - 2) * 0.18,
    filter: k === 0 ? "none" : `brightness(${Math.max(0.45, 0.85 - abs * 0.08)})`,
  };
}

/** Seuls les jeux proches sont dessinés. */
const visible = computed(() => list.value.map((g, i) => ({ g, i })).filter(({ i }) => Math.abs(i - index.value) <= 7));

function clickCover(i: number) {
  // Fin d'un glisser : ce n'est pas un clic.
  if (dragged) {
    dragged = false;
    return;
  }
  if (i === index.value) play();
  else {
    index.value = i;
    sfx("move");
  }
}

// --- Glisser à la souris (ou au doigt), avec élan au lâcher

/** Distance de glisser pour passer au jeu suivant. */
const STEP = 170;
const dragShift = ref(0);
const dragging = ref(false);
let drag: { start: number; anchor: number; lastX: number; lastT: number; v: number; id: number } | null = null;
let dragged = false;
let glide: number | undefined;

function onDragStart(e: PointerEvent) {
  if (e.button !== 0 || menuOpen.value || launching.value) return;
  clearInterval(glide);
  drag = { start: e.clientX, anchor: e.clientX, lastX: e.clientX, lastT: performance.now(), v: 0, id: e.pointerId };
  dragged = false;
}

function onDragMove(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return;
  if (!dragging.value) {
    // Seuil : un clic qui bouge d'un pixel reste un clic.
    if (Math.abs(e.clientX - drag.start) < 8) return;
    dragging.value = true;
    try {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    } catch {
      /* pointeur déjà relâché */
    }
  }
  const now = performance.now();
  const dt = Math.max(1, now - drag.lastT);
  drag.v = 0.7 * drag.v + 0.3 * ((e.clientX - drag.lastX) / dt);
  drag.lastX = e.clientX;
  drag.lastT = now;
  let dx = e.clientX - drag.anchor;
  while (Math.abs(dx) >= STEP) {
    const dir = dx > 0 ? -1 : 1;
    const before = index.value;
    move(dir);
    // Au bord de la liste, on ne déplace plus l'ancre : le carrousel résiste.
    if (index.value === before) break;
    drag.anchor += dir === -1 ? STEP : -STEP;
    dx = e.clientX - drag.anchor;
  }
  // Résistance aux extrémités.
  const atEdge = (dx > 0 && index.value === 0) || (dx < 0 && index.value === list.value.length - 1);
  dragShift.value = atEdge ? dx * 0.25 : dx * 0.85;
}

function onDragEnd(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return;
  const wasDragging = dragging.value;
  const v = performance.now() - drag.lastT > 80 ? 0 : drag.v;
  const shift = dragShift.value;
  drag = null;
  dragging.value = false;
  dragShift.value = 0;
  if (!wasDragging) return;
  // Le clic qui suit le lâcher arrive avant ce délai : il est ignoré, pas les suivants.
  dragged = true;
  setTimeout(() => (dragged = false), 0);
  // Plus de la moitié d'un pas : on termine le pas ; un lancer rapide ajoute de l'élan.
  let steps = Math.abs(shift) > STEP / 2 ? -Math.sign(shift) : 0;
  steps += -Math.round(Math.max(-5, Math.min(5, v * 2.2)));
  if (!steps) return;
  const dir = Math.sign(steps);
  let left = Math.abs(steps);
  move(dir);
  left--;
  glide = window.setInterval(() => {
    if (left-- <= 0) return clearInterval(glide);
    move(dir);
  }, 90);
}

let wheelAt = 0;
function onWheel(e: WheelEvent) {
  const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
  const now = performance.now();
  if (Math.abs(d) < 4 || now - wheelAt < 110) return;
  wheelAt = now;
  move(d > 0 ? 1 : -1);
}

// Inclinaison de la jaquette sélectionnée sous la souris.
const tilt = ref({ x: 0, y: 0 });
function onTilt(e: PointerEvent) {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  tilt.value = { x: ((e.clientY - r.top) / r.height - 0.5) * -14, y: ((e.clientX - r.left) / r.width - 0.5) * 18 };
}

// --- Actions

/** Sauvegarde et temps de jeu du jeu sélectionné. */
const status = computed(() => (game.value ? statusOf(game.value) : null));
const time = computed(() => playTime(status.value));

const emulator = computed(() => (game.value && emus.loaded ? defaultEmulator(platformOf(game.value)) : null));
const installing = computed(() => (game.value ? installs[RECOMMENDED[platformOf(game.value)]] ?? null : null));
const playLabel = computed(() => {
  if (installing.value) {
    const p = installing.value;
    return p.step === "download" && p.total ? `Téléchargement ${formatMo(p.done)} / ${formatMo(p.total)}` : "Installation…";
  }
  if (emus.loaded && !emulator.value && game.value) return `Installer ${{ gba: "mGBA", nds: "melonDS", "3ds": "Azahar", switch: "Eden" }[platformOf(game.value)]} et jouer`;
  return "Jouer";
});

const launching = ref<{ cover: string | null; title: string; status: string } | null>(null);
const notice = ref<string | null>(null);

async function play() {
  const g = game.value;
  if (!g || launching.value || installing.value || inserting.value) return;
  // Mode Cartouche : la cartouche s'insère dans la console (si l'émulateur est prêt).
  if (cartridgeMode.value && scene.value && emulator.value && insertAndPlay(g)) return;
  sfx("select");
  // Sans l'oublier : la musique reprend quand on ferme l'émulateur et revient dans Kaleido.
  stopMusic(false);
  launching.value = { cover: coverUrl(g), title: g.title, status: "Lancement…" };
  const emu = await launchGame(g);
  if (emu) {
    launching.value.status = `Bon jeu ! (${emu})`;
    setTimeout(() => (launching.value = null), 1600);
  } else {
    launching.value = null;
    previewMusic(game.value);
  }
}

// --- Mode Cartouche

type SceneApi = {
  startInsert(hooks: { launch(): void; done(): void; cancelled(): void }): boolean;
  cancelInsert(): boolean;
  resetInsert(): void;
  reloadLabel(path: string): void;
};
const scene = ref<SceneApi | null>(null);
const stageEl = ref<HTMLElement | null>(null);
const ringEl = ref<HTMLElement | null>(null);
const inserting = ref(false);
/** Fondu au noir de la fin d'insertion (0 à 1). */
const fade = ref(0);
let musicCut = false;

function onFade(v: number) {
  fade.value = v;
  // La musique baisse pendant l'insertion et se coupe au fondu.
  if (v > 0 && inserting.value && !musicCut) {
    musicCut = true;
    stopMusic(false);
  }
}

/**
 * Insertion de la cartouche : l'émulateur démarre au point d'enfoncement (l'animation
 * ne retarde pas le lancement) ; Échap ou B l'annule avant ce point.
 */
function insertAndPlay(g: Detection): boolean {
  let launched: Promise<string | null> | null = null;
  musicCut = false;
  const started = scene.value!.startInsert({
    launch: () => {
      launched = launchGame(g);
    },
    done: async () => {
      launching.value = { cover: null, title: g.title, status: "Lancement…" };
      const emu = await (launched ?? Promise.resolve(null));
      duckMusic(false);
      scene.value?.resetInsert();
      inserting.value = false;
      if (emu && launching.value) {
        launching.value.status = `Bon jeu ! (${emu})`;
        setTimeout(() => (launching.value = null), 1600);
      } else {
        launching.value = null;
        previewMusic(game.value);
      }
    },
    cancelled: () => {
      inserting.value = false;
      duckMusic(false);
    },
  });
  if (!started) return false;
  inserting.value = true;
  sfx("select");
  duckMusic(true);
  return true;
}

function onSceneFailed(reason: string) {
  presentation.wanted = "covers";
  notice.value = `Mode Cartouche indisponible (${reason}) : retour aux jaquettes.`;
}

function choosePresentation(p: "covers" | "cartridges") {
  if (p === "cartridges" && cartridgeBlocker.value) {
    notice.value = `Mode Cartouche indisponible : ${cartridgeBlocker.value}. Le lanceur reste en jaquettes.`;
    return;
  }
  presentation.wanted = p;
  notice.value = null;
}

/** Choix « Inspecter » de tous les jeux, transmis à la scène. */
const looks = computed<Record<string, StoredLook>>(() => libraryState.config.cartridge ?? {});

async function openSave() {
  const g = game.value;
  if (!g) return;
  stopMusic();
  notice.value = await openSaveOf(g);
}

// --- Nouvelle aventure : preset, aperçu, puis la partie est écrite et lancée.

const adventureView = ref<InstanceType<typeof NewAdventure> | null>(null);

function startAdventure() {
  const g = game.value;
  if (!g || !canRandomize(g)) return;
  sfx("select");
  void openAdventure(g);
}

async function playAdventure() {
  const g = adventure.game;
  if (!g) return;
  sfx("select");
  stopMusic(false);
  launching.value = { cover: coverUrl(g), title: g.game?.name ?? g.title, status: "Écriture de la partie…" };
  closeAdventure();
  try {
    const options = await writeAdventure();
    launching.value.status = "Lancement…";
    const result = await playGame(options);
    if (result) {
      launching.value.status = `Bon jeu ! (${result.emulator})`;
      setTimeout(() => (launching.value = null), 1600);
    } else {
      launching.value = null;
    }
  } catch (e) {
    launching.value = null;
    notice.value = `Nouvelle aventure impossible : ${e}`;
  }
}

// Menu « Plus d'actions ».
const menuOpen = ref(false);
const menuIndex = ref(0);
const menu = computed(() => {
  const g = game.value;
  if (!g) return [];
  const items: { label: string; icon: string; run: () => void; danger?: boolean; keep?: boolean }[] = [];
  if (cartridgeMode.value) items.push({ label: "Inspecter la cartouche", icon: "sliders", keep: true, run: () => openInspect(true) });
  items.push({ label: "Mods et réglages", icon: "wand", run: () => openMods(g) });
  if (statusOf(g)?.saveExists) items.push({ label: "Ouvrir sa sauvegarde", icon: "save", run: openSave });
  if (g.platform !== "switch" && statusOf(g)?.savePath) items.push({ label: "Compagnon de partie", icon: "pin", run: () => void openCompanionOf(g) });
  if (canRandomize(g)) items.push({ label: "Randomiser", icon: "dice", run: () => randomize(g) });
  if (g.platform !== "switch") items.push({ label: "Éditer la ROM", icon: "pencil", run: () => openRom(g.path) });
  items.push({ label: "Afficher le fichier", icon: "folder", run: () => revealItemInDir(g.path) });
  items.push({
    label: "Retirer de la bibliothèque",
    icon: "x",
    danger: true,
    run: async () => {
      removeItem(g.path);
      await hideGame(g.path);
    },
  });
  return items;
});

function openMenu() {
  menuIndex.value = 0;
  sheetMode.value = "menu";
  menuOpen.value = true;
  sfx("select");
}

function runMenu(i: number) {
  const item = menu.value[i];
  if (item?.keep) return item.run();
  menuOpen.value = false;
  if (!item) return;
  if (item.label !== "Ouvrir sa sauvegarde" && item.label !== "Retirer de la bibliothèque") stopMusic();
  item.run();
}

// --- Inspecter : apparence de la cartouche (bloc « Cartouche » de la fiche du jeu)

const sheetMode = ref<"menu" | "inspect">("menu");
const inspectIndex = ref(0);
/** Inspecter ouvert depuis le menu : Retour y revient au lieu de fermer la fiche. */
const inspectFromMenu = ref(false);

const lookKey = computed(() => (game.value ? cartridgeKey(game.value) : null));
const stored = computed<StoredLook>(() => (lookKey.value ? looks.value[lookKey.value] ?? {} : {}));

interface InspectRow {
  id: keyof StoredLook;
  label: string;
  options: { id: string | undefined; label: string; color?: string }[];
}

const inspectRows = computed<InspectRow[]>(() => {
  const g = game.value;
  if (!g) return [];
  const natural = resolveLook(g, null);
  return [
    {
      id: "shell",
      label: "Couleur de coque",
      options: [{ id: undefined, label: `Auto · ${natural.shell.label}`, color: natural.shell.color }, ...natural.support.shells.map((s) => ({ id: s.id, label: s.label, color: s.color }))],
    },
    {
      id: "finish",
      label: "Finition",
      options: [{ id: undefined, label: `Auto · ${FINISHES.find((f) => f.id === natural.finish)!.label}` }, ...FINISHES],
    },
    {
      id: "label",
      label: "Étiquette",
      options: [
        { id: undefined, label: "Automatique" },
        { id: "custom", label: stored.value.label === "custom" ? "Image personnelle" : "Choisir une image…" },
      ],
    },
    { id: "wear", label: "Usure", options: WEARS.map((w) => ({ id: w.id === "neuve" ? undefined : w.id, label: w.label })) },
  ];
});

function openInspect(fromMenu = false) {
  if (!game.value || !cartridgeMode.value) return;
  inspectFromMenu.value = fromMenu;
  inspectIndex.value = 0;
  sheetMode.value = "inspect";
  menuOpen.value = true;
  if (!fromMenu) sfx("select");
}

function closeInspect() {
  if (inspectFromMenu.value) sheetMode.value = "menu";
  else menuOpen.value = false;
}

async function chooseLook(row: InspectRow, id: string | undefined) {
  const g = game.value;
  const key = lookKey.value;
  if (!g || !key) return;
  if (row.id === "label") {
    if (id === "custom") {
      const file = await openDialog({ title: "Image de l'étiquette", filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg"] }] });
      if (typeof file !== "string") return;
      try {
        await invoke("label_custom", { key, source: file });
      } catch (e) {
        notice.value = `Étiquette impossible : ${e}`;
        return;
      }
    } else {
      await invoke("label_custom", { key, source: null }).catch(() => undefined);
    }
    await setCartridgeLook(key, { label: id === "custom" ? "custom" : undefined });
    scene.value?.reloadLabel(g.path);
    return;
  }
  await setCartridgeLook(key, { [row.id]: id } as Partial<StoredLook>);
}

/** Valeur choisie d'une ligne (index dans ses options). */
function optionIndex(row: InspectRow) {
  return Math.max(0, row.options.findIndex((o) => o.id === stored.value[row.id]));
}

function cycleLook(delta: number) {
  const row = inspectRows.value[inspectIndex.value];
  if (!row) return;
  // L'étiquette personnelle demande un fichier : seulement avec A / Entrée.
  if (row.id === "label") return;
  const n = row.options.length;
  void chooseLook(row, row.options[(optionIndex(row) + delta + n) % n].id);
  sfx("move");
}

// --- Plein écran

async function setImmersive(on: boolean) {
  libraryUi.immersive = on;
  await getCurrentWindow().setFullscreen(on).catch(() => undefined);
}

// --- Clavier et manette

function action(a: PadAction) {
  // La fenêtre « Mods et réglages » garde le clavier et la manette.
  if (modsDialog.game) return;
  // Pendant l'insertion, seul Retour compte : il remet la cartouche en place.
  if (inserting.value) {
    if (a === "back" && scene.value?.cancelInsert()) sfx("back");
    return;
  }
  if (launching.value) return;
  if (adventure.open) {
    // X sert de bouton Menu dans la Nouvelle aventure (corbeille des presets).
    adventureView.value?.handle(a === "inspect" ? "menu" : a);
    return;
  }
  if (menuOpen.value && sheetMode.value === "inspect") {
    const n = inspectRows.value.length;
    if (a === "up" || a === "down") {
      inspectIndex.value = (inspectIndex.value + (a === "up" ? -1 : 1) + n) % n;
      sfx("move");
    } else if (a === "left" || a === "right") cycleLook(a === "left" ? -1 : 1);
    else if (a === "accept") {
      const row = inspectRows.value[inspectIndex.value];
      if (row?.id === "label") void chooseLook(row, stored.value.label === "custom" ? undefined : "custom");
      else cycleLook(1);
    } else if (a === "back" || a === "inspect" || a === "menu") {
      sfx("back");
      closeInspect();
    }
    return;
  }
  if (menuOpen.value) {
    if (a === "up") menuIndex.value = (menuIndex.value - 1 + menu.value.length) % menu.value.length;
    else if (a === "down") menuIndex.value = (menuIndex.value + 1) % menu.value.length;
    else if (a === "accept") runMenu(menuIndex.value);
    else if (a === "back" || a === "menu") menuOpen.value = false;
    if (a === "up" || a === "down") sfx("move");
    if (a === "back") sfx("back");
    return;
  }
  if (a === "left") move(-1);
  else if (a === "right") move(1);
  else if (a === "accept") play();
  else if (a === "menu" || a === "down") openMenu();
  else if (a === "inspect") openInspect();
  else if (a === "adventure") startAdventure();
  else if (a === "prevTab") switchTab(-1);
  else if (a === "nextTab") switchTab(1);
  else if (a === "back" && libraryUi.immersive) {
    sfx("back");
    setImmersive(false);
  }
}

const { connected: padConnected, id: padId, stick } = useGamepad(action);

/** Aides en bas d'écran (cliquables à la souris). */
const hintItems = computed(() => {
  const g = game.value;
  const items: { action: HintAction; label: string; run?: () => void }[] = [{ action: "move", label: "Choisir" }];
  items.push({ action: "accept", label: "Jouer", run: play });
  if (g && cartridgeMode.value) items.push({ action: "inspect", label: "Inspecter", run: () => openInspect() });
  if (g && canRandomize(g)) items.push({ action: "adventure", label: "Nouvelle aventure", run: startAdventure });
  items.push({ action: "tabs", label: "Plateformes", run: () => switchTab(1) });
  items.push({ action: "menu", label: "Plus", run: openMenu });
  if (libraryUi.immersive) items.push({ action: "back", label: "Quitter le plein écran", run: () => setImmersive(false) });
  return items;
});

const sheetHints = computed<{ action: HintAction; label: string }[]>(() =>
  sheetMode.value === "inspect"
    ? [
        { action: "move", label: "Changer" },
        { action: "back", label: "Retour" },
      ]
    : [
        { action: "accept", label: "Valider" },
        { action: "back", label: "Retour" },
      ],
);

const tabGlyphs = computed(() => glyphs("tabs", padConnected.value, layoutOf(padId.value)));

const KEYS: Record<string, PadAction> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Enter: "accept",
  " ": "accept",
  Escape: "back",
  Backspace: "back",
  q: "prevTab",
  e: "nextTab",
  PageUp: "prevTab",
  PageDown: "nextTab",
  m: "menu",
  ContextMenu: "menu",
  i: "inspect",
  n: "adventure",
};

function onKey(e: KeyboardEvent) {
  const target = e.target as HTMLElement | null;
  if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
  if (e.key === "Home" || e.key === "End") {
    index.value = e.key === "Home" ? 0 : list.value.length - 1;
    sfx("move");
    e.preventDefault();
    return;
  }
  const a = KEYS[e.key];
  if (!a) return;
  e.preventDefault();
  action(a);
}

// --- Horloge

const now = ref(new Date());
let clock: number | undefined;

onMounted(() => {
  if (!emus.loaded) loadEmulators();
  window.addEventListener("keydown", onKey);
  clock = window.setInterval(() => (now.value = new Date()), 15000);
});

onUnmounted(() => {
  clearInterval(glide);
  window.removeEventListener("keydown", onKey);
  clearInterval(clock);
  stopMusic();
  if (libraryUi.immersive) setImmersive(false);
});

const clockText = computed(() => now.value.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" }));
const date = computed(() => now.value.toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long" }));

const meta = computed(() => {
  const g = game.value;
  if (!g) return [];
  const parts = [PLATFORM_LABEL[platformOf(g)]];
  if (g.generation) parts.push(`${g.generation}ᵉ génération`);
  if (g.language) parts.push(g.language.replace("Multilingue (français inclus)", "Multilingue"));
  // Jeu Switch : mise à jour trouvée à côté, seulement si sa version est connue.
  const update = g.details.find((d) => d.label === UPDATE_LABEL)?.value;
  if (update && /^\d/.test(update)) parts.push(`${UPDATE_LABEL} ${update}`);
  return parts;
});
</script>

<template>
  <section class="launcher" :class="{ cartridges: cartridgeMode }" :style="{ '--tint': color }" @wheel.passive="onWheel">
    <Backdrop :image="game ? coverUrl(game) : null" :color="color" />
    <TitleScene :game="game ?? null" />
    <CartridgeScene
      v-if="cartridgeMode"
      ref="scene"
      :games="list"
      :index="index"
      :drag-shift="dragShift"
      :dragging="dragging"
      :tilt="tilt"
      :stick="stick"
      :looks="looks"
      :tint="color"
      :stage="stageEl"
      :ring="ringEl"
      @fade="onFade"
      @failed="onSceneFailed"
    />

    <header class="topbar">
      <nav class="tabs">
        <span class="key" title="Q ou L">{{ tabGlyphs[0].text }}</span>
        <button v-for="t in TABS" :key="t.id" :class="{ active: tab === t.id }" @click="tab = t.id">{{ t.label }}</button>
        <span class="key" title="E ou R">{{ tabGlyphs[1].text }}</span>
      </nav>
      <div class="status">
        <div class="present" role="group" aria-label="Présentation du lanceur">
          <button :class="{ on: !cartridgeMode }" :aria-pressed="!cartridgeMode" @click="choosePresentation('covers')">Jaquettes</button>
          <button
            :class="{ on: cartridgeMode, blocked: presentation.wanted === 'cartridges' && !cartridgeMode }"
            :aria-pressed="cartridgeMode"
            :title="presentation.wanted === 'cartridges' && cartridgeBlocker ? `Indisponible : ${cartridgeBlocker}` : 'Cartouches en 3D'"
            @click="choosePresentation('cartridges')"
          >
            Cartouches
          </button>
        </div>
        <button class="round" :class="{ off: !audio.music }" :title="audio.music ? 'Couper la musique' : 'Activer la musique'" @click="audio.music = !audio.music">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" />
            <circle cx="18" cy="16" r="3" />
            <path v-if="!audio.music" d="M3 3l18 18" />
          </svg>
        </button>
        <input v-if="audio.music" v-model.number="audio.volume" class="volume" type="range" min="0" max="0.5" step="0.01" :title="`Volume : ${Math.round(audio.volume * 100)} %`" aria-label="Volume de la musique" />
        <button class="round" title="Affichage en grille" @click="libraryUi.mode = 'grid'"><Icon name="grid" :size="15" /></button>
        <button class="round" :title="libraryUi.immersive ? 'Quitter le plein écran (Échap)' : 'Plein écran'" @click="setImmersive(!libraryUi.immersive)">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path v-if="!libraryUi.immersive" d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
            <path v-else d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
          </svg>
        </button>
        <span class="clock" :title="date">{{ clockText }}</span>
      </div>
    </header>

    <div v-if="game" class="hero">
      <Transition name="hero" mode="out-in">
        <div :key="game.path" class="hero-inner">
          <p class="meta">
            <span v-for="m in meta" :key="m">{{ m }}</span>
          </p>
          <h1>{{ game.title }}</h1>
          <p class="sub">
            <span v-if="isKaleidoRom(game)" class="tag">Randomisée<template v-if="game.kaleido"> · seed {{ game.kaleido.seed }}</template></span>
            <span v-if="game.kind === 'ctr_dump'">Mod joué par-dessus le jeu d'origine</span>
            <span v-if="status?.trainer">Dresseur {{ status.trainer }}</span>
            <span v-if="time" :title="`Temps de jeu ${time.source}`">{{ formatDuration(time.seconds) }} de jeu</span>
            <span v-if="lastPlayed[game.path]">Dernière partie {{ timeAgo(lastPlayed[game.path]) }}</span>
            <span v-if="status && !status.saveExists && !lastPlayed[game.path]" class="dim">Pas encore de partie</span>
          </p>
          <div class="cta">
            <button class="play" :disabled="!!installing" @click="play">
              <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M7 4l13 8-13 8z" /></svg>
              <span>{{ playLabel }}</span>
              <span class="key light">{{ padConnected ? "A" : "Entrée" }}</span>
            </button>
            <button v-if="canRandomize(game)" class="ghost" @click="startAdventure"><Icon name="dice" :size="16" /> Nouvelle aventure</button>
            <button v-if="status?.saveExists" class="ghost" @click="openSave"><Icon name="save" :size="16" /> Sauvegarde</button>
            <button class="ghost" @click="openMods(game)"><Icon name="wand" :size="16" /> Mods</button>
            <button class="ghost icon" title="Plus d'actions" aria-label="Plus d'actions" @click="openMenu">⋯</button>
          </div>
          <p v-if="notice" class="notice">{{ notice }}</p>
        </div>
      </Transition>
      <div v-if="audio.running.length" class="now-playing on ingame">
        <span class="dot" />
        <span>En jeu dans {{ audio.running.join(", ") }} · musique en pause</span>
      </div>
      <div v-else class="now-playing" :class="{ on: audio.playing === game.path, loading: audio.loading === game.path }">
        <span class="eq"><i /><i /><i /><i /></span>
        <span>{{ audio.loading === game.path ? "Chargement de la musique…" : "Thème de l'écran titre" }}</span>
      </div>
    </div>

    <div
      ref="stageEl"
      class="stage"
      :class="{ dragging }"
      @pointerdown="onDragStart"
      @pointermove="onDragMove"
      @pointerup="onDragEnd"
      @pointercancel="onDragEnd"
    >
      <div ref="ringEl" class="ring">
        <button
          v-for="{ g, i } in visible"
          :key="g.path"
          class="cover"
          :class="{ selected: i === index, kaleido: isKaleidoRom(g) }"
          :style="itemStyle(i)"
          :aria-label="g.title"
          tabindex="-1"
          @click="clickCover(i)"
          @pointermove="i === index && onTilt($event)"
          @pointerleave="tilt = { x: 0, y: 0 }"
        >
          <div class="card" :style="i === index ? { transform: `rotateX(${tilt.x}deg) rotateY(${tilt.y}deg)` } : undefined">
            <img v-if="coverUrl(g)" :src="coverUrl(g)!" :alt="g.title" draggable="false" />
            <span v-else class="fallback">{{ g.title }}</span>
            <span v-if="isKaleidoRom(g)" class="ribbon">Randomisée</span>
          </div>
          <div class="reflection" aria-hidden="true">
            <img v-if="coverUrl(g)" :src="coverUrl(g)!" alt="" draggable="false" />
          </div>
        </button>
      </div>
      <p class="counter">{{ index + 1 }} / {{ list.length }}</p>
    </div>

    <footer class="hints">
      <PadHints :items="hintItems" :pad="padConnected" :pad-id="padId">
        <span v-if="padConnected" class="pad"><Icon name="check" :size="13" /> Manette connectée</span>
      </PadHints>
    </footer>

    <Transition name="sheet">
      <div v-if="menuOpen && game" class="menu-layer" @click.self="menuOpen = false">
        <aside class="sheet">
          <div class="sheet-head">
            <img v-if="coverUrl(game)" :src="coverUrl(game)!" alt="" />
            <div>
              <p>{{ PLATFORM_LABEL[platformOf(game)] }}</p>
              <h3>{{ game.title }}</h3>
            </div>
          </div>
          <div v-if="sheetMode === 'menu'" class="sheet-list">
            <button v-for="(m, i) in menu" :key="m.label" :class="{ active: i === menuIndex, danger: m.danger }" @mouseenter="menuIndex = i" @click="runMenu(i)">
              <Icon :name="m.icon" :size="18" />
              <span>{{ m.label }}</span>
            </button>
          </div>
          <div v-else class="inspect">
            <p class="inspect-title">Cartouche · {{ resolveLook(game, stored).support.label }}</p>
            <div v-for="(row, r) in inspectRows" :key="row.id" class="inspect-row" :class="{ active: r === inspectIndex }" @mouseenter="inspectIndex = r">
              <span class="inspect-label">{{ row.label }}</span>
              <div class="chips" role="radiogroup" :aria-label="row.label">
                <button
                  v-for="(o, k) in row.options"
                  :key="o.label"
                  class="chip"
                  :class="{ on: k === optionIndex(row) }"
                  role="radio"
                  :aria-checked="k === optionIndex(row)"
                  @click="chooseLook(row, o.id)"
                >
                  <span v-if="o.color" class="swatch" :style="{ background: o.color }" />
                  {{ o.label }}
                </button>
              </div>
            </div>
            <button v-if="inspectFromMenu" class="inspect-back" @click="closeInspect"><Icon name="chevron-left" :size="15" /> Toutes les actions</button>
          </div>
          <footer>
            <PadHints :items="sheetHints" :pad="padConnected" :pad-id="padId" />
          </footer>
        </aside>
      </div>
    </Transition>

    <Transition name="fade">
      <NewAdventure
        v-if="adventure.open"
        ref="adventureView"
        :cover="adventure.game ? coverUrl(adventure.game) : null"
        :pad-connected="padConnected"
        @play="playAdventure"
      />
    </Transition>

    <div v-if="fade > 0" class="insert-fade" :class="{ settle: !inserting }" :style="{ opacity: fade }" aria-hidden="true" />

    <Transition name="fade">
      <div v-if="launching" class="launch-layer">
        <img v-if="launching.cover" :src="launching.cover" alt="" />
        <h2>{{ launching.title }}</h2>
        <p>{{ launching.status }}</p>
      </div>
    </Transition>
  </section>
</template>

<style scoped>
.launcher {
  position: absolute;
  inset: 0;
  display: grid;
  grid-template-rows: auto 1fr auto auto;
  overflow: hidden;
  color: #fff;
  font-family: var(--font);
  user-select: none;
}

.launcher > :not(.menu-layer):not(.launch-layer):not(.backdrop):not(.title-scene):not(.adventure):not(.cartridge-scene):not(.insert-fade) {
  position: relative;
}

/* --- Mode Cartouche : la scène 3D dessine, les boutons du carrousel restent (clics, glisser, lecteur d'écran). */

.cartridges .cover > * {
  visibility: hidden;
}

.insert-fade {
  position: absolute;
  inset: 0;
  z-index: 190;
  background: #000;
  pointer-events: none;
}

.insert-fade.settle {
  transition: opacity 0.4s ease;
}

.present {
  display: flex;
  padding: 2px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
}

.present button {
  padding: 5px 12px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  font-weight: 700;
}

.present button:hover:not(.on) {
  color: #fff;
}

.present button.on {
  background: rgba(255, 255, 255, 0.95);
  color: #0b0d18;
}

.present button.blocked {
  text-decoration: line-through;
  color: rgba(255, 255, 255, 0.45);
}

/* --- Barre du haut */

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 22px 34px 0;
}

.tabs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tabs button {
  padding: 7px 16px;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.65);
  font-size: 14px;
  font-weight: 600;
  transition: background 0.2s, color 0.2s;
}

.tabs button:hover:not(.active) {
  color: #fff;
}

.tabs button.active {
  background: rgba(255, 255, 255, 0.95);
  color: #0b0d18;
}

.key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  padding: 2px 7px;
  border: 1px solid rgba(255, 255, 255, 0.35);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 11px;
  font-weight: 700;
}

.key.light {
  border-color: rgba(0, 0, 0, 0.25);
  color: inherit;
  opacity: 0.7;
}

.status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 6px 5px 6px;
  border-radius: 999px;
  background: rgba(10, 12, 28, 0.55);
  backdrop-filter: blur(14px);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
}

.round {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: #fff;
}

.round:hover {
  background: rgba(255, 255, 255, 0.12);
}

.round.off {
  color: rgba(255, 255, 255, 0.45);
}

.volume {
  width: 80px;
  accent-color: var(--tint);
}

.clock {
  padding: 0 12px 0 4px;
  font-size: 15px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

/* --- Fiche du jeu */

.hero {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 18px;
  min-height: 0;
  padding: 0 64px 8px;
}

.hero-inner {
  max-width: 760px;
}

.meta {
  display: flex;
  gap: 14px;
  margin: 0 0 10px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.meta span + span::before {
  content: "•";
  margin-right: 14px;
  color: var(--tint);
}

h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: clamp(36px, 5.2vw, 68px);
  font-weight: 800;
  line-height: 1.02;
  letter-spacing: -0.02em;
  text-shadow: 0 4px 30px rgba(0, 0, 0, 0.5);
}

.sub {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin: 12px 0 0;
  color: rgba(255, 255, 255, 0.78);
  font-size: 14px;
}

.sub .dim {
  color: rgba(255, 255, 255, 0.5);
}

.tag {
  padding: 2px 10px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.92);
  color: #0b0d18;
  font-size: 12px;
  font-weight: 700;
}

.cta {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 26px;
}

.play {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  padding: 15px 22px 15px 26px;
  border: none;
  border-radius: 999px;
  background: #fff;
  color: #0b0d18;
  font-size: 17px;
  font-weight: 800;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  transition: transform 0.15s ease, background 0.15s;
}

.play:hover:not(:disabled) {
  transform: scale(1.04);
}

.play:disabled {
  cursor: progress;
  opacity: 0.85;
}

.ghost {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 13px 20px;
  border: none;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  font-size: 15px;
  font-weight: 600;
  backdrop-filter: blur(12px);
  transition: background 0.2s;
}

.ghost:hover {
  background: rgba(255, 255, 255, 0.22);
}

.ghost.icon {
  width: 48px;
  justify-content: center;
  padding: 13px 0;
  font-size: 20px;
  line-height: 1;
}

.notice {
  margin: 14px 0 0;
  color: rgba(255, 255, 255, 0.8);
  font-size: 13px;
}

.now-playing {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  align-self: flex-start;
  padding: 6px 14px 6px 10px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.35);
  color: rgba(255, 255, 255, 0.85);
  font-size: 12px;
  opacity: 0;
  transform: translateY(6px);
  transition: opacity 0.4s, transform 0.4s;
}

.now-playing.on,
.now-playing.loading {
  opacity: 1;
  transform: none;
}

.ingame .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #4ade80;
}

.eq {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 14px;
}

.eq i {
  width: 3px;
  height: 30%;
  border-radius: 2px;
  background: var(--tint);
}

.now-playing.on .eq i {
  animation: eq 0.9s ease-in-out infinite alternate;
}

.eq i:nth-child(2) {
  animation-delay: -0.3s !important;
}

.eq i:nth-child(3) {
  animation-delay: -0.6s !important;
}

.eq i:nth-child(4) {
  animation-delay: -0.15s !important;
}

@keyframes eq {
  from {
    height: 20%;
  }
  to {
    height: 100%;
  }
}

.hero-enter-active {
  transition: opacity 0.35s ease, transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.hero-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.hero-enter-from {
  opacity: 0;
  transform: translateX(-24px);
}

.hero-leave-to {
  opacity: 0;
  transform: translateX(12px);
}

/* --- Carrousel */

.stage {
  height: 430px;
  perspective: 1100px;
}

.ring {
  position: absolute;
  left: 50%;
  top: 47%;
  transform-style: preserve-3d;
}

.stage {
  cursor: grab;
  touch-action: pan-y;
}

.stage.dragging {
  cursor: grabbing;
}

/* Pendant le glisser, les jaquettes suivent la souris sans traîner. */
.stage.dragging .cover {
  transition-duration: 0.12s;
}

.cover img {
  -webkit-user-drag: none;
}

.cover {
  position: absolute;
  left: -150px;
  top: -133px;
  width: 300px;
  height: 266px;
  padding: 0;
  border: none;
  background: none;
  transform-style: preserve-3d;
  transition: transform 0.5s cubic-bezier(0.2, 0.85, 0.25, 1), opacity 0.4s, filter 0.4s;
}

.reflection {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 14px;
  transform: scaleY(-1);
  opacity: 0.32;
  pointer-events: none;
  -webkit-mask-image: linear-gradient(to top, #000 0%, transparent 38%);
  mask-image: linear-gradient(to top, #000 0%, transparent 38%);
}

.reflection img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.card {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.08);
  box-shadow: 0 16px 34px rgba(0, 0, 0, 0.5);
  transition: transform 0.2s ease-out, box-shadow 0.45s ease;
}

/* Sélection : un anneau blanc net, sans halo coloré. */
.selected .card {
  box-shadow:
    0 0 0 4px #fff,
    0 28px 50px rgba(0, 0, 0, 0.55);
}

.card img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.fallback {
  display: flex;
  align-items: flex-end;
  height: 100%;
  padding: 16px;
  background: color-mix(in srgb, var(--tint) 45%, #1b1f3b);
  font-size: 18px;
  font-weight: 800;
  text-align: left;
}

.ribbon {
  position: absolute;
  right: 8px;
  bottom: 8px;
  padding: 2px 8px;
  border-radius: 5px;
  background: rgba(255, 255, 255, 0.92);
  color: #0b0d18;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.02em;
}

.counter {
  position: absolute;
  right: 40px;
  bottom: 10px;
  margin: 0;
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

/* --- Aide */

.hints {
  display: flex;
  justify-content: center;
  gap: 26px;
  padding: 12px 24px 20px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
}

.pad {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  color: var(--tint);
}

/* --- Menu et lancement */

.menu-layer,
.launch-layer {
  position: absolute;
  inset: 0;
  z-index: 200;
  display: grid;
  place-items: center;
  background: rgba(3, 4, 12, 0.6);
  backdrop-filter: blur(10px);
}

.menu-layer {
  place-items: stretch end;
  background: linear-gradient(to right, rgba(3, 4, 12, 0.15), rgba(3, 4, 12, 0.7));
  backdrop-filter: blur(4px);
}

.sheet {
  display: flex;
  flex-direction: column;
  width: 400px;
  height: 100%;
  padding: 36px 26px 22px;
  background: rgba(12, 14, 26, 0.94);
  box-shadow: -30px 0 60px rgba(0, 0, 0, 0.45);
}

.sheet-head {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 6px 26px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.sheet-head img {
  width: 76px;
  height: 67px;
  object-fit: cover;
  border-radius: 8px;
}

.sheet-head p {
  margin: 0 0 4px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.sheet-head h3 {
  margin: 0;
  font-size: 19px;
  line-height: 1.2;
}

.sheet-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
  padding-top: 18px;
}

.sheet-list button {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border: none;
  border-radius: 12px;
  background: transparent;
  color: rgba(255, 255, 255, 0.88);
  font-size: 15px;
  font-weight: 600;
  text-align: left;
  transition: background 0.12s, color 0.12s;
}

.sheet-list button.active {
  background: #fff;
  color: #0b0d18;
}

.sheet-list button.danger {
  margin-top: auto;
  color: #ff9b9b;
}

.sheet-list button.danger.active {
  background: #ff9b9b;
  color: #2a0b0b;
}

.sheet footer {
  padding: 18px 6px 0;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.65);
  font-size: 12px;
}

.sheet footer :deep(.pad-hints) {
  justify-content: flex-start;
}

/* --- Inspecter */

.inspect {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 4px;
  padding-top: 18px;
  overflow-y: auto;
}

.inspect-title {
  margin: 0 6px 10px;
  color: rgba(255, 255, 255, 0.55);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.inspect-row {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px 14px 14px;
  border-radius: 12px;
  transition: background 0.12s;
}

.inspect-row.active {
  background: rgba(255, 255, 255, 0.08);
}

.inspect-label {
  font-size: 14px;
  font-weight: 700;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 6px 11px;
  border: none;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.82);
  font-size: 12px;
  font-weight: 600;
}

.chip:hover:not(.on) {
  background: rgba(255, 255, 255, 0.16);
}

.chip.on {
  background: #fff;
  color: #0b0d18;
}

.swatch {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
}

.inspect-back {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  margin: auto 6px 0;
  padding: 8px 0;
  border: none;
  background: none;
  color: rgba(255, 255, 255, 0.65);
  font-size: 13px;
  font-weight: 600;
}

.inspect-back:hover {
  color: #fff;
}

.sheet-enter-active,
.sheet-leave-active {
  transition: opacity 0.25s ease;
}

.sheet-enter-active .sheet,
.sheet-leave-active .sheet {
  transition: transform 0.3s cubic-bezier(0.2, 0.85, 0.25, 1);
}

.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}

.sheet-enter-from .sheet,
.sheet-leave-to .sheet {
  transform: translateX(100%);
}

.launch-layer {
  align-content: center;
  gap: 14px;
  text-align: center;
}

.launch-layer img {
  width: 300px;
  border-radius: 18px;
  box-shadow: 0 0 0 4px #fff, 0 30px 80px rgba(0, 0, 0, 0.6);
  animation: launch 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) both;
}

.launch-layer h2 {
  margin: 10px 0 0;
  font-size: 26px;
}

.launch-layer p {
  margin: 0;
  color: rgba(255, 255, 255, 0.75);
}

@keyframes launch {
  from {
    transform: scale(0.7) rotateY(-25deg);
    opacity: 0;
  }
  to {
    transform: none;
    opacity: 1;
  }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@media (max-height: 820px) {
  .stage {
    zoom: 0.78;
  }

  h1 {
    font-size: clamp(30px, 4.4vw, 54px);
  }
}
</style>
