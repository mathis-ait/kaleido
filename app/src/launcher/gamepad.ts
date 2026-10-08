import { onMounted, onUnmounted, ref, type Ref } from "vue";

/** Actions du lanceur, au clavier comme à la manette. */
export type PadAction = "left" | "right" | "up" | "down" | "accept" | "back" | "prevTab" | "nextTab" | "menu" | "inspect" | "adventure";

/** Nombre de vues qui gèrent elles-mêmes la manette (le lanceur) : la navigation globale se met alors en retrait. */
export const padClaims = ref(0);

export interface PadState {
  connected: Ref<boolean>;
  /** Nom annoncé par la manette (disposition des glyphes, voir hints.ts). */
  id: Ref<string | null>;
  /** Stick droit, de -1 à 1 (zone morte retirée). */
  stick: Ref<{ x: number; y: number }>;
}

/**
 * Manette (API Gamepad, disposition « standard ») : croix et stick gauche pour
 * naviguer, A pour valider, B pour revenir, X pour inspecter, Y pour une nouvelle
 * aventure, LB / RB pour changer d'onglet, Start pour le menu, stick droit pour
 * incliner la cartouche. Répétition automatique quand on garde une direction.
 */
export function useGamepad(onAction: (a: PadAction) => void): PadState {
  const poller = createPadPoller(onAction);
  onMounted(() => {
    padClaims.value++;
    poller.start();
  });
  onUnmounted(() => {
    padClaims.value--;
    poller.stop();
  });
  return { connected: poller.connected, id: poller.id, stick: poller.stick };
}

/** Lecture de la manette à chaque image, hors composant (voir `useGamepad`). */
export function createPadPoller(onAction: (a: PadAction) => void): PadState & { start: () => void; stop: () => void } {
  const connected = ref(false);
  const id = ref<string | null>(null);
  const stick = ref({ x: 0, y: 0 });
  let frame = 0;
  const held = new Map<PadAction, number>();
  /** Actions vues relâchées au moins une fois : un bouton (ou un stick) déjà enfoncé à
   * l'arrivée de la manette, ou bloqué par un pilote, ne déclenche rien. */
  const armed = new Set<PadAction>();
  let padId: string | null = null;

  const BUTTONS: [number, PadAction][] = [
    [0, "accept"],
    [1, "back"],
    [2, "inspect"],
    [3, "adventure"],
    [4, "prevTab"],
    [5, "nextTab"],
    [9, "menu"],
    [12, "up"],
    [13, "down"],
    [14, "left"],
    [15, "right"],
  ];
  const ALL: PadAction[] = ["accept", "back", "inspect", "adventure", "prevTab", "nextTab", "menu", "up", "down", "left", "right"];

  /** Zone morte du stick droit, puis remise à l'échelle de 0 à 1. */
  const dead = (v: number) => (Math.abs(v) < 0.18 ? 0 : (v - Math.sign(v) * 0.18) / 0.82);

  function poll(now: number) {
    const pads = navigator.getGamepads?.() ?? [];
    // Une vraie manette (disposition « standard ») plutôt qu'un périphérique quelconque.
    const live = pads.filter((p): p is Gamepad => !!p && p.connected);
    const pad = live.find((p) => p.mapping === "standard") ?? live[0] ?? null;
    connected.value = !!pad;
    if (pad && pad.id !== padId) {
      padId = pad.id;
      id.value = pad.id;
      armed.clear();
      held.clear();
    }
    if (pad) {
      const active = new Set<PadAction>();
      for (const [i, action] of BUTTONS) if (pad.buttons[i]?.pressed) active.add(action);
      const [x = 0, y = 0, rx = 0, ry = 0] = pad.axes;
      if (x < -0.55) active.add("left");
      if (x > 0.55) active.add("right");
      if (y < -0.55) active.add("up");
      if (y > 0.55) active.add("down");
      // Stick droit : mis à jour seulement quand il bouge, pour ne rien redessiner au repos.
      const sx = dead(rx);
      const sy = dead(ry);
      if (Math.abs(sx - stick.value.x) > 0.02 || Math.abs(sy - stick.value.y) > 0.02) stick.value = { x: sx, y: sy };
      for (const action of ALL) {
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
    id,
    stick,
    start: () => (frame = requestAnimationFrame(poll)),
    stop: () => cancelAnimationFrame(frame),
  };
}
