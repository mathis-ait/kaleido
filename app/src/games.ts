import { computed, reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { library } from "./library";
import { isRom, type Detection } from "./types";
import { applyState, emus, loadEmulators, type EmulatorId, type EmulatorsState } from "./play/play";

/** Miroir de `LibraryConfig` (library.rs). */
export interface LibraryConfig {
  folders: string[];
  files: string[];
  hidden: string[];
  /** Dossiers retirés : la recherche automatique ne les rajoute plus. */
  ignored: string[];
}

/** Miroir de `SwitchGame` (switch.rs). */
export interface SwitchGame {
  path: string;
  fileName: string;
  titleId: string;
  title: string;
  size: number;
  hasUpdate: boolean;
}

/** Bibliothèque de jeux : dossiers et fichiers mémorisés d'une session à l'autre. */
export const games = reactive({
  loaded: false,
  scanning: false,
  config: { folders: [], files: [], hidden: [], ignored: [] } as LibraryConfig,
  /** Dossiers ajoutés par la recherche automatique de ce démarrage. */
  autoAdded: [] as string[],
  found: [] as Detection[],
  switchFound: [] as SwitchGame[],
  error: null as string | null,
});

/** Jeu Switch présenté comme les autres jeux de la bibliothèque. */
function switchDetection(g: SwitchGame): Detection {
  return {
    path: g.path,
    fileName: g.fileName,
    kind: "switch_game",
    title: g.title,
    game: null,
    platform: "switch",
    generation: null,
    language: g.hasUpdate ? "Mise à jour trouvée" : null,
    isFrench: false,
    size: g.size,
    details: [{ label: "Title ID", value: g.titleId }],
    warnings: [],
    kaleido: null,
    fingerprint: null,
  };
}

/** Title ID d'un jeu (Switch, 3DS), tel que l'affiche la détection. */
export const titleIdOf = (d: Detection) => d.details.find((x) => x.label === "Title ID")?.value.replace(/^0x/i, "").toUpperCase() ?? null;

/** Jeux suivis, plus les ROMs ouvertes pendant la session (sans doublon). */
export const allGames = computed(() => {
  const list: Detection[] = [];
  // Une même ROM rangée à deux endroits (copie de sauvegarde…) n'apparaît qu'une fois.
  const seen = new Set<string>();
  const add = (d: Detection) => {
    if (list.some((g) => g.path === d.path)) return;
    if (d.fingerprint) {
      if (seen.has(d.fingerprint)) return;
      seen.add(d.fingerprint);
    }
    list.push(d);
  };
  games.found.forEach(add);
  games.switchFound.map(switchDetection).forEach(add);
  for (const d of library.items) {
    if (isRom(d) && !games.config.hidden.includes(d.path)) add(d);
  }
  return list;
});

export async function loadGames() {
  if (!games.loaded) games.config = { ...games.config, ...(await invoke<LibraryConfig>("library_config").catch(() => games.config)) };
  games.loaded = true;
  await rescan();
}

export async function rescan() {
  games.scanning = true;
  games.error = null;
  try {
    const [found, switchFound] = await Promise.all([
      invoke<Detection[]>("library_scan", { config: games.config }),
      invoke<SwitchGame[]>("library_scan_switch", { config: games.config }).catch(() => [] as SwitchGame[]),
    ]);
    games.found = found;
    games.switchFound = switchFound;
  } catch (e) {
    games.error = String(e);
  } finally {
    games.scanning = false;
  }
}

async function change(edit: (c: LibraryConfig) => void, scan = true) {
  const config: LibraryConfig = JSON.parse(JSON.stringify(games.config));
  edit(config);
  games.config = config;
  await invoke("library_set_config", { config }).catch((e) => (games.error = String(e)));
  if (scan) await rescan();
}

export const addFolder = (dir: string) =>
  change((c) => {
    if (!c.folders.includes(dir)) c.folders.push(dir);
  });

/** Ajoute plusieurs dossiers d'un coup (une seule relecture). */
export const addFolders = (dirs: string[]) =>
  change((c) => {
    for (const dir of dirs) {
      if (!c.folders.includes(dir)) c.folders.push(dir);
      c.ignored = c.ignored.filter((d) => d !== dir);
    }
  });

interface Discovery {
  folders: { path: string }[];
  emulators: { id: EmulatorId; exe: string; current: boolean }[];
}

let discovered = false;

/**
 * Au démarrage : cherche les jeux et les émulateurs sur le PC (en arrière-plan, moins
 * d'une seconde en général) et les ajoute tout seul. Les dossiers retirés par
 * l'utilisateur ne reviennent pas ; un émulateur déjà configuré n'est pas remplacé.
 */
export async function autoDiscover() {
  if (discovered) return;
  discovered = true;
  if (!games.loaded) await loadGames();
  const found = await invoke<Discovery>("discover_pc").catch(() => null);
  if (!found) return;
  if (!emus.loaded) await loadEmulators();
  for (const e of found.emulators) {
    if (e.current || emus.list.find((x) => x.id === e.id)?.exe) continue;
    const state = await invoke<EmulatorsState>("emulator_locate", { id: e.id, exe: e.exe }).catch(() => null);
    if (state) applyState(state);
  }
  const known = (p: string) => [...games.config.folders, ...games.config.ignored].some((f) => f.toLowerCase() === p.toLowerCase());
  const fresh = found.folders.map((f) => f.path).filter((p) => !known(p));
  if (fresh.length) {
    games.autoAdded = fresh;
    await addFolders(fresh);
  }
}

export const removeFolder = (dir: string) =>
  change((c) => {
    c.folders = c.folders.filter((f) => f !== dir);
    if (!c.ignored.includes(dir)) c.ignored.push(dir);
  });

export const addFiles = (paths: string[], scan = true) =>
  change((c) => {
    for (const p of paths) {
      c.hidden = c.hidden.filter((h) => h !== p);
      if (!c.files.includes(p)) c.files.push(p);
    }
  }, scan);

/** Retire un jeu de la bibliothèque (le fichier n'est pas touché). */
export async function hideGame(path: string) {
  await change((c) => {
    c.files = c.files.filter((f) => f !== path);
    if (!c.hidden.includes(path)) c.hidden.push(path);
  }, false);
  games.found = games.found.filter((g) => g.path !== path);
  games.switchFound = games.switchFound.filter((g) => g.path !== path);
}

// Les ROMs ouvertes (ou générées par le randomizer) restent dans la bibliothèque.
watch(
  () => library.items.filter(isRom).map((d) => d.path),
  async (paths) => {
    if (!games.loaded) await loadGames();
    const fresh = paths.filter((p) => !games.config.files.includes(p) && !games.found.some((g) => g.path === p));
    if (fresh.length) await addFiles(fresh, false);
  },
);

// --- Affichage de la bibliothèque

const UI_KEY = "kaleido.library.mode";

function readMode(): "launcher" | "grid" {
  try {
    return localStorage.getItem(UI_KEY) === "grid" ? "grid" : "launcher";
  } catch {
    return "launcher";
  }
}

/** Mode d'affichage (lanceur ou grille), plein écran, jeu sélectionné dans le lanceur. */
export const libraryUi = reactive({
  mode: readMode(),
  immersive: false,
  selected: null as string | null,
});

watch(
  () => libraryUi.mode,
  (mode) => {
    try {
      localStorage.setItem(UI_KEY, mode);
    } catch {
      /* stockage indisponible */
    }
  },
);
