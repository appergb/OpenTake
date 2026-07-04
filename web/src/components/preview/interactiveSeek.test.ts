import { describe, expect, it } from "vitest";
import {
  createInteractiveSeekQueue,
  enqueueInteractiveSeek,
  flushPendingInteractiveSeek,
  interactiveToleranceSec,
  INTERACTIVE_SEEK_INTERVAL_MS,
} from "./interactiveSeek";

describe("interactive seek throttling", () => {
  it("dispatches the first seek immediately", () => {
    const queue = createInteractiveSeekQueue();

    const result = enqueueInteractiveSeek(queue, { frame: 12, toleranceSec: 0.15 }, 1000);

    expect(result).toEqual({ kind: "flush", request: { frame: 12, toleranceSec: 0.15 } });
    expect(queue.pending).toBeNull();
    expect(queue.lastDispatchMs).toBe(1000);
  });

  it("coalesces rapid scrub seeks to the latest pending frame", () => {
    const queue = createInteractiveSeekQueue();
    enqueueInteractiveSeek(queue, { frame: 1, toleranceSec: 0.15 }, 0);

    const early = enqueueInteractiveSeek(queue, { frame: 2, toleranceSec: 0.3 }, 5);
    const latest = enqueueInteractiveSeek(queue, { frame: 7, toleranceSec: 0.45 }, 10);

    expect(early.kind).toBe("schedule");
    expect(latest).toEqual({
      kind: "schedule",
      delayMs: INTERACTIVE_SEEK_INTERVAL_MS - 10,
    });
    expect(flushPendingInteractiveSeek(queue, INTERACTIVE_SEEK_INTERVAL_MS)).toEqual({
      frame: 7,
      toleranceSec: 0.45,
    });
    expect(queue.pending).toBeNull();
  });

  it("matches upstream interactive tolerance scaling and cap", () => {
    expect(interactiveToleranceSec(0)).toBe(0.15);
    expect(interactiveToleranceSec(3)).toBeCloseTo(0.45);
    expect(interactiveToleranceSec(10)).toBe(0.75);
  });
});
