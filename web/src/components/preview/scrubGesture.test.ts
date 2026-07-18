import { describe, expect, it } from "vitest";
import {
  createScrubGesture,
  transitionScrubGesture,
} from "./scrubGesture";

describe("scrub gesture lifecycle", () => {
  it("stays interactive through 20 moves and commits exactly one final exact seek", () => {
    let state = createScrubGesture();
    let exactSeeks = 0;

    ({ state } = transitionScrubGesture(state, "down"));
    expect(state.active).toBe(true);

    for (let index = 0; index < 20; index += 1) {
      const transition = transitionScrubGesture(state, "move");
      state = transition.state;
      exactSeeks += transition.effect === "exact-seek" ? 1 : 0;
      expect(state.active).toBe(true);
      expect(transition.scrubbing).toBe(true);
    }

    let transition = transitionScrubGesture(state, "up");
    state = transition.state;
    exactSeeks += transition.effect === "exact-seek" ? 1 : 0;
    expect(transition.scrubbing).toBe(false);

    transition = transitionScrubGesture(state, "cancel");
    exactSeeks += transition.effect === "exact-seek" ? 1 : 0;
    expect(transition.effect).toBe("none");
    expect(exactSeeks).toBe(1);
  });

  it("cancels a stolen pointer without publishing a final seek", () => {
    const started = transitionScrubGesture(createScrubGesture(), "down");
    const cancelled = transitionScrubGesture(started.state, "cancel");

    expect(cancelled.state.active).toBe(false);
    expect(cancelled.scrubbing).toBe(false);
    expect(cancelled.effect).toBe("none");
  });
});
