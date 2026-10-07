import { createPadPoller, padClaims, type PadAction } from "./launcher/gamepad";

/**
 * Manette sur toutes les pages (le lanceur a sa propre gestion) :
 * croix / stick = passer d'un élément à son voisin dans cette direction, A = activer,
 * B = Échap (retour, fermer), LB / RB = Q / E (page précédente / suivante de l'éditeur).
 */

const FOCUSABLE =
  "button:not(:disabled), a[href], input:not(:disabled):not([type=hidden]), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

/** Zone où chercher : la fenêtre ouverte s'il y en a une, sinon toute l'app. */
function scope(): ParentNode {
  const dialogs = document.querySelectorAll<HTMLElement>("[role=dialog]");
  return dialogs[dialogs.length - 1] ?? document;
}

function visible(el: HTMLElement) {
  if (el.offsetParent === null && getComputedStyle(el).position !== "fixed") return false;
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth;
}

function candidates() {
  return [...scope().querySelectorAll<HTMLElement>(FOCUSABLE)].filter(visible);
}

/** Voisin le plus proche dans la direction voulue (écart perpendiculaire pénalisé). */
function neighbour(from: DOMRect, dir: PadAction, items: HTMLElement[], self: Element | null) {
  const cx = from.left + from.width / 2;
  const cy = from.top + from.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of items) {
    if (el === self) continue;
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    let main: number;
    let cross: number;
    if (dir === "left") [main, cross] = [from.left - r.right, y - cy];
    else if (dir === "right") [main, cross] = [r.left - from.right, y - cy];
    else if (dir === "up") [main, cross] = [from.top - r.bottom, x - cx];
    else [main, cross] = [r.top - from.bottom, x - cx];
    // Doit être devant nous (un léger chevauchement est toléré).
    if (main < -4) continue;
    const score = Math.max(main, 0) + Math.abs(cross) * 2.5;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  return best;
}

function move(dir: PadAction) {
  const items = candidates();
  if (!items.length) return;
  const active = document.activeElement as HTMLElement | null;
  const current = active && active !== document.body && items.includes(active) ? active : null;
  // Rien de sélectionné : on part du premier élément du contenu de la page.
  const target = current ? neighbour(current.getBoundingClientRect(), dir, items, current) : (items.find((el) => el.closest("main")) ?? items[0]);
  if (!target) return;
  target.focus({ preventScroll: true });
  target.scrollIntoView({ block: "nearest", inline: "nearest" });
}

function key(k: string, target: EventTarget = document.activeElement ?? document.body) {
  target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
}

function onAction(a: PadAction) {
  if (padClaims.value > 0 || !document.hasFocus()) return;
  const active = document.activeElement as HTMLElement | null;
  const focused = active && active !== document.body;
  switch (a) {
    case "up":
    case "down":
    case "left":
    case "right":
      move(a);
      break;
    case "accept":
      if (focused) active.click();
      else key("Enter", window);
      break;
    case "back":
      key("Escape");
      break;
    case "prevTab":
      key("q", window);
      break;
    case "nextTab":
      key("e", window);
      break;
    case "menu":
      break;
  }
}

/** Démarre la navigation à la manette (une seule fois, au lancement de l'app). */
export function initGamepadNav() {
  createPadPoller(onAction).start();
}
