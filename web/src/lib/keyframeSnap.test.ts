import { describe, expect, it } from "vitest";
import { snapFrame } from "./keyframeSnap";

describe("snapFrame", () => {
  it("snaps to the nearest target within threshold", () => {
    expect(snapFrame(100, [90, 103, 120], 5)).toEqual({ frame: 103, snappedTo: 103 });
  });

  it("returns the candidate unchanged with snappedTo=null when nothing is within threshold", () => {
    expect(snapFrame(100, [50, 200], 5)).toEqual({ frame: 100, snappedTo: null });
  });

  it("returns the candidate unchanged with snappedTo=null for an empty target list", () => {
    expect(snapFrame(100, [], 5)).toEqual({ frame: 100, snappedTo: null });
  });

  it("resolves equal-distance ties deterministically by picking the first target encountered", () => {
    // 98 and 102 are both distance 2 from candidate 100.
    expect(snapFrame(100, [98, 102], 5)).toEqual({ frame: 98, snappedTo: 98 });
    // Reversed order still picks the first-encountered tie.
    expect(snapFrame(100, [102, 98], 5)).toEqual({ frame: 102, snappedTo: 102 });
  });

  it("snaps exactly at the threshold boundary (inclusive)", () => {
    expect(snapFrame(100, [105], 5)).toEqual({ frame: 105, snappedTo: 105 });
  });

  it("does not snap just past the threshold boundary", () => {
    expect(snapFrame(100, [106], 5)).toEqual({ frame: 100, snappedTo: null });
  });

  it("snaps when the candidate already equals a target (distance 0)", () => {
    expect(snapFrame(100, [100, 90], 5)).toEqual({ frame: 100, snappedTo: 100 });
  });

  it("considers playhead, cross-property, and clip-bound targets together, nearest wins", () => {
    // Simulates: playhead=50, clip start=10, clip end=90, other-property kf=97.
    const targets = [50, 10, 90, 97];
    expect(snapFrame(95, targets, 5)).toEqual({ frame: 97, snappedTo: 97 });
  });

  it("ignores negative or non-finite thresholds gracefully (no snap)", () => {
    expect(snapFrame(100, [100], 0)).toEqual({ frame: 100, snappedTo: 100 });
    expect(snapFrame(100, [101], 0)).toEqual({ frame: 100, snappedTo: null });
  });
});
