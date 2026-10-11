/**
 * Séquence d'insertion de la cartouche (mode Cartouche), en images clés
 * interpolées. Sans three.js : la scène lit l'état à chaque image.
 *
 *   0 → 200 ms     les voisines s'écartent et s'estompent
 *   100 → 300 ms   la silhouette de la console monte en bas de la scène
 *   200 → 500 ms   la cartouche se met face à la fente et descend jusqu'à elle
 *   500 ms         point d'enfoncement : le jeu est lancé (plus d'annulation après)
 *   500 → 800 ms   la cartouche s'enfonce ; clic en fin de course
 *   800 → 1100 ms  fondu au noir
 *
 * Jeu DS dans son boîtier : un prélude de `CASE_PRELUDE` ms précède les phases ci-dessus
 * (sauf l'écart des voisines, immédiat) : le couvercle s'ouvre, la carte sort de son
 * logement et vient face à l'écran, le boîtier descend et s'efface.
 */

export interface InsertState {
  /** Voisines écartées et estompées. */
  spread: number;
  /** Silhouette de la console visible. */
  slot: number;
  /** Cartouche face à la fente, descendue jusqu'à elle. */
  descend: number;
  /** Cartouche enfoncée. */
  sink: number;
  /** Fondu au noir. */
  fade: number;
  /** Couvercle du boîtier ouvert (peut dépasser 1 : léger rebond). */
  open: number;
  /** Carte sortie du boîtier, face à l'écran. */
  lift: number;
  /** Boîtier descendu et effacé. */
  out: number;
}

export const INITIAL: InsertState = { spread: 0, slot: 0, descend: 0, sink: 0, fade: 0, open: 0, lift: 0, out: 0 };

export const TIMELINE = {
  spread: [0, 200],
  slot: [100, 300],
  descend: [200, 500],
  sink: [500, 800],
  fade: [800, 1100],
  launch: 500,
  click: 780,
  end: 1100,
} as const;

/** Prélude du boîtier (ms). */
export const CASE_PRELUDE = 950;
export const CASE_TIMELINE = {
  open: [0, 560],
  lift: [380, 950],
  out: [700, 1050],
} as const;

/** Durée du retour en place après une annulation. */
export const CANCEL_MS = 260;

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
const easeInOut = (x: number) => (x < 0.5 ? 2 * x * x : 1 - (-2 * x + 2) ** 2 / 2);
const easeIn = (x: number) => x * x * x;
/** Sortie avec léger dépassement (charnière qui rebondit). */
const easeOutBack = (x: number) => (x <= 0 ? 0 : x >= 1 ? 1 : 1 + 2.2 * (x - 1) ** 3 + 1.2 * (x - 1) ** 2);
const phase = ([a, b]: readonly [number, number], t: number) => clamp01((t - a) / (b - a));

/** État de la séquence au temps `t` (ms depuis le début) ; `prelude` : boîtier à ouvrir d'abord. */
export function insertState(t: number, prelude = 0): InsertState {
  const u = t - prelude;
  return {
    spread: easeInOut(phase(TIMELINE.spread, t)),
    slot: easeInOut(phase(TIMELINE.slot, u)),
    descend: easeInOut(phase(TIMELINE.descend, u)),
    sink: easeIn(phase(TIMELINE.sink, u)),
    fade: phase(TIMELINE.fade, u),
    open: prelude ? easeOutBack(phase(CASE_TIMELINE.open, t)) : 0,
    lift: prelude ? easeInOut(phase(CASE_TIMELINE.lift, t)) : 0,
    out: prelude ? easeInOut(phase(CASE_TIMELINE.out, t)) : 0,
  };
}

const lerpState = (from: InsertState, k: number): InsertState => ({
  spread: from.spread * (1 - k),
  slot: from.slot * (1 - k),
  descend: from.descend * (1 - k),
  sink: from.sink * (1 - k),
  fade: from.fade * (1 - k),
  open: from.open * (1 - k),
  lift: from.lift * (1 - k),
  out: from.out * (1 - k),
});

export interface InsertHooks {
  frame(state: InsertState): void;
  /** Point d'enfoncement : lancer l'émulateur. */
  launch(): void;
  /** Fin de course : bruit de la cartouche qui s'enclenche. */
  click(): void;
  /** Écran noir : la séquence est finie. */
  done(): void;
  /** Annulée : la cartouche est revenue en place. */
  cancelled(): void;
}

export interface Clock {
  now(): number;
  request(cb: () => void): number;
  cancel(id: number): void;
}

const browserClock: Clock = {
  now: () => performance.now(),
  request: (cb) => requestAnimationFrame(cb),
  cancel: (id) => cancelAnimationFrame(id),
};

export interface Insertion {
  /** Annule si le jeu n'est pas encore lancé ; renvoie false sinon. */
  cancel(): boolean;
  /** Arrête net (démontage du lanceur), sans rappel. */
  stop(): void;
  readonly running: boolean;
}

export function startInsertion(hooks: InsertHooks, clock: Clock = browserClock, prelude = 0): Insertion {
  const t0 = clock.now();
  let frame = 0;
  let launched = false;
  let clicked = false;
  let state: InsertState = { ...INITIAL };
  let cancelAt: number | null = null;
  let cancelFrom: InsertState | null = null;
  let running = true;

  function tick() {
    if (!running) return;
    const now = clock.now();
    if (cancelAt !== null && cancelFrom) {
      const k = clamp01((now - cancelAt) / CANCEL_MS);
      state = lerpState(cancelFrom, easeInOut(k));
      hooks.frame(state);
      if (k >= 1) {
        running = false;
        hooks.cancelled();
        return;
      }
      frame = clock.request(tick);
      return;
    }
    const t = now - t0;
    state = insertState(t, prelude);
    hooks.frame(state);
    if (!launched && t >= TIMELINE.launch + prelude) {
      launched = true;
      hooks.launch();
    }
    if (!clicked && t >= TIMELINE.click + prelude) {
      clicked = true;
      hooks.click();
    }
    if (t >= TIMELINE.end + prelude) {
      running = false;
      hooks.done();
      return;
    }
    frame = clock.request(tick);
  }

  hooks.frame(state);
  frame = clock.request(tick);

  return {
    cancel() {
      if (!running || launched || cancelAt !== null) return false;
      cancelAt = clock.now();
      cancelFrom = { ...state };
      return true;
    },
    stop() {
      running = false;
      clock.cancel(frame);
    },
    get running() {
      return running;
    },
  };
}
