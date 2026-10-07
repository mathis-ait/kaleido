import { onMounted, onUnmounted, ref, type Ref } from "vue";

/** Actions du lanceur, au clavier comme à la manette. */
export type PadAction = "left" | "right" | "up" | "down" | "accept" | "back" | "prevTab" | "nextTab" | "menu";

/** Nombre de vues qui gèrent elles-mêmes la manette (le lanceur) : la navigation globale se met alors en retrait. */
export const padClaims = ref(0);

/**
 * Manette (API Gamepad, disposition « standard ») : croix et stick gauche pour
 * naviguer, A pour valider, B pour revenir, LB / RB pour changer d'onglet,
 * Start pour le menu. Répétition automatique quand on garde une direction.
 */
export function useGamepad(onAction: (a: PadAction) => void) {
  const poller = createPadPoller(onAction);
  onMounted(() => {
    padClaims.value++;
    poller.start();
  });
  onUnmounted(() => {
    padClaims.value--;
    poller.stop();
  });
  return { connected: poller.connected };
}

/** Lecture de la manette à chaque image, hors composant (voir `useGamepad`). */
export function createPadPoller(onAction: (a: PadAction) => void): { connected: Ref<boolean>; start: () => void; stop: () => void } {
  const connected = ref(false);
  let frame = 0;
  const held = new Map<PadAction, number>();
  /** Actions vues relâchées au moins une fois : un bouton (ou un stick) déjà enfoncé à
   * l'arrivée de la manette, ou bloqué par un pilote, ne déclenche rien. */
  const armed = new Set<PadAction>();
  let padId: string | null = null;

  const BUTTONS: [number, PadAction][] = [
    [0, "accept"],
    [1, "back"],
    [4, "prevTab"],
    [5, "nextTab"],
    [9, "menu"],
    [12, "up"],
    [13, "down"],
    [14, "left"],
    [15, "right"],
  ];

  function poll(now: number) {
    const pads = navigator.getGamepads?.() ?? [];
    // Une vraie manette (disposition « standard ») plutôt qu'un périphérique quelconque.
    const live = pads.filter((p): p is Gamepad => !!p && p.connected);
    const pad = live.find((p) => p.mapping === "standard") ?? live[0] ?? null;
    connected.value = !!pad;
    if (pad && pad.id !== padId) {
      padId = pad.id;
      armed.clear();
      held.clear();
    }
    if (pad) {
      const active = new Set<PadAction>();
      for (const [i, action] of BUTTONS) if (pad.buttons[i]?.pressed) active.add(action);
      const [x = 0, y = 0] = pad.axes;
      if (x < -0.55) active.add("left");
      if (x > 0.55) active.add("right");
      if (y < -0.55) active.add("up");
      if (y > 0.55) active.add("down");
      for (const action of ["accept", "back", "prevTab", "nextTab", "menu", "up", "down", "left", "right"] as PadAction[]) {
        if (!active.has(action)) armed.add(action);
      }
      for (const action of active) {
        if (!armed.has(action)) continue;
        const since = held.get(action);
        const repeats = ["left", "right", "up", "down"].includes(action);
        if (since === undefined) {
          held.set(action, now);
          onAction(action);
        } else if (repeats && now - since > 380) {
          // Répétition : une action toutes les 110 ms après 380 ms d'appui.
          held.set(action, now - 380 + 110);
          onAction(action);
        }
      }
      for (const action of [...held.keys()]) if (!active.has(action)) held.delete(action);
    }
    frame = requestAnimationFrame(poll);
  }

  return {
    connected,
    start: () => (frame = requestAnimationFrame(poll)),
    stop: () => cancelAnimationFrame(frame),
  };
}
