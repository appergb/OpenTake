/**
 * Render-path coverage for the marked-range / gap / ripple-insert overlays.
 * Drives `paintTimeline` and `paintRuler` against a recording 2D-context stub
 * and asserts the exact fillRect / stroke calls (color + geometry) that a
 * screenshot would show — deterministic, no browser required.
 */
import { describe, expect, it } from "vitest";
import type { Clip, Timeline, Track } from "../../lib/types";
import { RANGE } from "../../lib/theme";
import { paintTimeline, type PaintState, type MediaGhostPaint } from "./timelineCanvas";
import { paintRuler, type RulerState } from "./rulerCanvas";

function clip(id: string, startFrame: number, durationFrames: number): Clip {
  return {
    id,
    mediaRef: `${id}-m`,
    mediaType: "video",
    sourceClipType: "video",
    startFrame,
    durationFrames,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: { centerX: 0.5, centerY: 0.5, width: 1, height: 1, rotation: 0, flipHorizontal: false, flipVertical: false },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
  };
}

function timeline(): Timeline {
  const v1: Track = { id: "v1", type: "video", muted: false, hidden: false, syncLocked: true, clips: [clip("a", 0, 100), clip("b", 200, 100)] };
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [v1] };
}

/** A minimal recording CanvasRenderingContext2D. Captures fillStyle/strokeStyle
 *  at each fillRect/stroke so the test can assert what colors were painted. */
function recordingCtx() {
  const calls: { op: string; style: string; args: number[] }[] = [];
  let fillStyle = "";
  let strokeStyle = "";
  const stub = {
    set fillStyle(v: string) { fillStyle = v; },
    get fillStyle() { return fillStyle; },
    set strokeStyle(v: string) { strokeStyle = v; },
    get strokeStyle() { return strokeStyle; },
    lineWidth: 1,
    font: "",
    textAlign: "left",
    textBaseline: "alphabetic",
    setTransform() {},
    clearRect() {},
    fillRect(...a: number[]) { calls.push({ op: "fillRect", style: fillStyle, args: a }); },
    strokeRect(...a: number[]) { calls.push({ op: "strokeRect", style: strokeStyle, args: a }); },
    fillText() {},
    beginPath() {},
    moveTo() {},
    lineTo() {},
    stroke() { calls.push({ op: "stroke", style: strokeStyle, args: [] }); },
    fill() { calls.push({ op: "fill", style: fillStyle, args: [] }); },
    setLineDash() {},
    save() {},
    restore() {},
    translate() {},
    scale() {},
    rect() {},
    arc() {},
    arcTo() {},
    ellipse() {},
    bezierCurveTo() {},
    closePath() {},
    quadraticCurveTo() {},
    clip() {},
    createLinearGradient() { return { addColorStop() {} }; },
    measureText() { return { width: 10 }; },
    drawImage() {},
    roundRect() {},
  };
  return { ctx: stub as unknown as CanvasRenderingContext2D, calls };
}

function baseState(over: Partial<PaintState>): PaintState {
  return {
    timeline: timeline(),
    pixelsPerFrame: 4,
    trackHeights: {},
    selectedClipIds: new Set(),
    dpr: 1,
    width: 2000,
    height: 400,
    firstAudioIndex: -1,
    scrollLeft: 0,
    scrollTop: 0,
    viewWidth: 2000,
    viewHeight: 400,
    waveforms: new Map(),
    thumbnails: new Map(),
    missingMediaRefs: new Set(),
    emptyLabel: "",
    ...over,
  };
}

describe("marked-range overlay (content canvas)", () => {
  it("paints the range track fill + two edge strokes at the range x-span", () => {
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ selectedRange: { startFrame: 40, endFrame: 90 } }));
    // Track fill uses RANGE.trackFill starting at x = 40 * 4 = 160.
    const fill = calls.find((c) => c.op === "fillRect" && c.style === RANGE.trackFill);
    expect(fill).toBeDefined();
    expect(fill!.args[0]).toBe(160);
    expect(fill!.args[2]).toBe((90 - 40) * 4); // width = 200
    // Two edge strokes in the accent-timecode color.
    const edges = calls.filter((c) => c.op === "stroke" && c.style === RANGE.edge);
    expect(edges.length).toBe(2);
  });

  it("normalizes an inverted range before painting", () => {
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ selectedRange: { startFrame: 90, endFrame: 40 } }));
    const fill = calls.find((c) => c.op === "fillRect" && c.style === RANGE.trackFill);
    expect(fill!.args[0]).toBe(160); // min edge, not 360
  });

  it("paints nothing for a collapsed range", () => {
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ selectedRange: { startFrame: 50, endFrame: 50 } }));
    expect(calls.some((c) => c.style === RANGE.trackFill)).toBe(false);
    expect(calls.some((c) => c.style === RANGE.edge)).toBe(false);
  });
});

describe("gap overlay (content canvas)", () => {
  it("paints a dashed box on the gap's track", () => {
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ selectedGap: { trackIndex: 0, startFrame: 100, endFrame: 200 } }));
    expect(calls.some((c) => c.op === "fillRect" && c.style === RANGE.gapFill)).toBe(true);
    expect(calls.some((c) => c.op === "strokeRect" && c.style === RANGE.gapStroke)).toBe(true);
  });
});

describe("ripple-insert indicator (content canvas)", () => {
  it("draws a yellow insertion line when the media ghost is a ripple insert", () => {
    const ghost: MediaGhostPaint = { startFrame: 120, durationFrames: 60, trackIndex: 0, newTrackIndex: null, rippleInsert: true };
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ mediaGhost: ghost }));
    // The insert line strokes in GHOST.insertLine (yellow) — distinct from the
    // gray overwrite-ghost border.
    const yellowStroke = calls.filter((c) => c.op === "stroke" && c.style === "rgb(255,204,0)");
    expect(yellowStroke.length).toBeGreaterThan(0);
  });

  it("does NOT draw the insertion line for a plain overwrite ghost", () => {
    const ghost: MediaGhostPaint = { startFrame: 120, durationFrames: 60, trackIndex: 0, newTrackIndex: null, rippleInsert: false };
    const { ctx, calls } = recordingCtx();
    paintTimeline(ctx, baseState({ mediaGhost: ghost }));
    expect(calls.some((c) => c.op === "stroke" && c.style === "rgb(255,204,0)")).toBe(false);
  });
});

describe("marked-range overlay (ruler canvas)", () => {
  function rulerState(over: Partial<RulerState>): RulerState {
    return { fps: 30, pixelsPerFrame: 4, scrollLeft: 0, width: 2000, dpr: 1, ...over };
  }
  it("paints the ruler band fill + edge strokes", () => {
    const { ctx, calls } = recordingCtx();
    paintRuler(ctx, rulerState({ selectedRange: { startFrame: 40, endFrame: 90 } }));
    expect(calls.some((c) => c.op === "fillRect" && c.style === RANGE.rulerFill)).toBe(true);
    expect(calls.filter((c) => c.op === "stroke" && c.style === RANGE.edge).length).toBe(2);
  });
  it("paints no band without a range", () => {
    const { ctx, calls } = recordingCtx();
    paintRuler(ctx, rulerState({ selectedRange: null }));
    expect(calls.some((c) => c.style === RANGE.rulerFill)).toBe(false);
  });
});
