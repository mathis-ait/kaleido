import { describe, expect, it } from "vitest";
import { CANCEL_MS, INITIAL, TIMELINE, insertState, startInsertion, type Clock, type InsertState } from "./insert";

/** Horloge manuelle : `advance` fait tourner les images une par une (16 ms). */
function manualClock() {
  let now = 0;
  let queue: (() => void)[] = [];
  const clock: Clock = {
    now: () => now,
    request: (cb) => queue.push(cb),
    cancel: () => (queue = []),
  };
  const advance = (ms: number) => {
    const end = now + ms;
    while (now < end) {
      now = Math.min(end, now + 16);
      const q = queue;
      queue = [];
      q.forEach((cb) => cb());
    }
  };
  return { clock, advance };
}

function run() {
  const { clock, advance } = manualClock();
  const log = { frames: [] as InsertState[], launched: 0, clicked: 0, done: 0, cancelled: 0 };
  const insertion = startInsertion(
    {
      frame: (s) => log.frames.push(s),
      launch: () => log.launched++,
      click: () => log.clicked++,
      done: () => log.done++,
      cancelled: () => log.cancelled++,
    },
    clock,
  );
  return { insertion, advance, log };
}

describe("insertion", () => {
  it("commence et finit sur les bons états", () => {
    expect(insertState(0)).toEqual(INITIAL);
    expect(insertState(TIMELINE.end)).toEqual({ spread: 1, slot: 1, descend: 1, sink: 1, fade: 1 });
  });

  it("lance le jeu une fois au point d'enfoncement, puis finit", () => {
    const { advance, log } = run();
    advance(TIMELINE.launch - 20);
    expect(log.launched).toBe(0);
    advance(40);
    expect(log.launched).toBe(1);
    advance(TIMELINE.end);
    expect(log).toMatchObject({ launched: 1, clicked: 1, done: 1, cancelled: 0 });
  });

  // Chaque image clé avant le lancement : l'annulation ramène à l'état initial.
  for (const t of [0, 100, 200, 300, 400, TIMELINE.launch - 20]) {
    it(`annulation à ${t} ms : retour à l'état initial, jeu non lancé`, () => {
      const { insertion, advance, log } = run();
      advance(t);
      expect(insertion.cancel()).toBe(true);
      advance(CANCEL_MS + 40);
      expect(log.frames.at(-1)).toEqual(INITIAL);
      expect(log).toMatchObject({ launched: 0, done: 0, cancelled: 1 });
      expect(insertion.running).toBe(false);
    });
  }

  it("plus d'annulation une fois le jeu lancé", () => {
    const { insertion, advance, log } = run();
    advance(TIMELINE.launch + 20);
    expect(insertion.cancel()).toBe(false);
    advance(TIMELINE.end);
    expect(log.done).toBe(1);
  });
});
