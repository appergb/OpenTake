import { describe, expect, it } from "vitest";
import {
  clipRect,
  contentHeight,
  contentWidth,
  dropTargetAt,
  endFrame,
  formatClipDuration,
  formatTimecode,
  frameAt,
  trackAt,
  trackDisplayHeight,
  trackY,
  totalFrames,
  xForFrame,
} from "./geometry";
import { LAYOUT, TRACK_SIZE } from "./theme";
import type { Clip, ClipType, Timeline, Track } from "./types";

// Constants recap (theme.ts / SPEC §5.2):
//   rulerHeight = 24, dropZoneHeight = 60 → first track top = 84
//   defaultHeight = 50, minHeight = 32, maxHeight = 200
//   headerWidth = 0 inside the canvas (track-header is a separate column)

const RULER = LAYOUT.rulerHeight + LAYOUT.dropZoneHeight; // 84
const H = TRACK_SIZE.defaultHeight; // 50

function clip(over: Partial<Clip> = {}): Clip {
  return {
    id: "c1",
    mediaRef: "m1",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 30,
    trimStartFrame: 0,
    trimEndFrame: 0,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "smooth",
    fadeOutInterpolation: "smooth",
    opacity: 1,
    transform: {
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...over,
  };
}

function track(type: ClipType, id: string, clips: Clip[] = []): Track {
  return { id, type, muted: false, hidden: false, syncLocked: true, clips };
}

function timeline(tracks: Track[], fps = 30): Timeline {
  return {
    fps,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks,
  };
}

describe("trackDisplayHeight", () => {
  it("falls back to defaultHeight when the track id has no entry", () => {
    expect(trackDisplayHeight(track("video", "t0"), {})).toBe(TRACK_SIZE.defaultHeight);
  });

  it("uses the supplied per-track height", () => {
    expect(trackDisplayHeight(track("video", "t0"), { t0: 80 })).toBe(80);
  });

  it("clamps to minHeight", () => {
    expect(trackDisplayHeight(track("video", "t0"), { t0: 10 })).toBe(TRACK_SIZE.minHeight);
  });

  it("clamps to maxHeight", () => {
    expect(trackDisplayHeight(track("video", "t0"), { t0: 9999 })).toBe(TRACK_SIZE.maxHeight);
  });

  it("clamps within the legal range", () => {
    expect(trackDisplayHeight(track("video", "t0"), { t0: 40 })).toBe(40);
  });
});

describe("trackY", () => {
  it("track 0 starts at rulerHeight + dropZoneHeight", () => {
    const t = timeline([track("video", "t0")]);
    expect(trackY(t, 0, {})).toBe(RULER);
  });

  it("track i = RULER + sum of displayHeight[0..i-1]", () => {
    const t = timeline([
      track("video", "t0"),
      track("video", "t1"),
      track("audio", "t2"),
    ]);
    expect(trackY(t, 0, {})).toBe(RULER); // 84
    expect(trackY(t, 1, {})).toBe(RULER + H); // 134
    expect(trackY(t, 2, {})).toBe(RULER + 2 * H); // 184
  });

  it("honours per-track custom heights", () => {
    const t = timeline([track("video", "t0"), track("audio", "t1")]);
    const heights = { t0: 100, t1: 32 };
    expect(trackY(t, 0, heights)).toBe(RULER); // 84
    expect(trackY(t, 1, heights)).toBe(RULER + 100); // 184
  });

  it("clamps custom heights before summing", () => {
    const t = timeline([track("video", "t0"), track("video", "t1")]);
    expect(trackY(t, 1, { t0: 5 })).toBe(RULER + TRACK_SIZE.minHeight);
  });
});

describe("clipRect", () => {
  it("matches SPEC §5.2: x=frame*ppf, y=trackY+2, width=dur*ppf, height=trackH-4", () => {
    const t = timeline([track("video", "t0", [clip({ startFrame: 10, durationFrames: 20 })])]);
    const ppf = 4;
    const r = clipRect(t, 0, t.tracks[0].clips[0], ppf, {});
    // x = 10 * 4 = 40, y = 84 + 2 = 86, width = 20 * 4 = 80, height = 50 - 4 = 46
    expect(r).toEqual({ x: 40, y: RULER + 2, width: 80, height: H - 4 });
  });

  it("uses 0 headerWidth inside the canvas", () => {
    const t = timeline([track("video", "t0", [clip({ startFrame: 0, durationFrames: 5 })])]);
    const r = clipRect(t, 0, t.tracks[0].clips[0], 2, {});
    expect(r.x).toBe(0); // no header offset
    expect(r.width).toBe(10);
  });

  it("uses per-track custom height (clamped) for the rect height", () => {
    const t = timeline([track("video", "t0", [clip()])]);
    const r = clipRect(t, 0, t.tracks[0].clips[0], 4, { t0: 100 });
    expect(r.height).toBe(100 - 4);
  });

  it("y offset follows trackY for tracks below the first", () => {
    const t = timeline([
      track("video", "t0", [clip({ id: "a" })]),
      track("video", "t1", [clip({ id: "b", startFrame: 0, durationFrames: 10 })]),
    ]);
    const r1 = clipRect(t, 1, t.tracks[1].clips[0], 4, {});
    expect(r1.y).toBe(RULER + H + 2); // 136
  });
});

describe("frameAt", () => {
  it("truncates (does not round) per SPEC §5.2 / AGENTS.md", () => {
    // 1.9 / 1 = 1.9 → trunc → 1
    expect(frameAt(1.9, 1)).toBe(1);
  });

  it("matches Int(x / ppf) for typical zoom values", () => {
    expect(frameAt(39, 4)).toBe(9); // 39/4 = 9.75 → 9
    expect(frameAt(40, 4)).toBe(10); // exactly 10
    expect(frameAt(41, 4)).toBe(10); // 10.25 → 10
  });

  it("clamps to 0 for negative x", () => {
    expect(frameAt(-50, 4)).toBe(0);
    expect(frameAt(-0.1, 4)).toBe(0);
  });

  it("returns 0 at x=0", () => {
    expect(frameAt(0, 4)).toBe(0);
  });
});

describe("xForFrame", () => {
  it("is frame * pixelsPerFrame (headerWidth = 0 inside canvas)", () => {
    expect(xForFrame(10, 4)).toBe(40);
    expect(xForFrame(0, 4)).toBe(0);
    expect(xForFrame(100, 0.5)).toBe(50);
  });
});

describe("trackAt", () => {
  it("returns 0 for a y inside the first track", () => {
    const t = timeline([track("video", "t0"), track("video", "t1")]);
    expect(trackAt(t, RULER + 5, {})).toBe(0);
    expect(trackAt(t, RULER + H - 1, {})).toBe(0);
  });

  it("returns i for a y inside track i", () => {
    const t = timeline([track("video", "t0"), track("video", "t1"), track("audio", "t2")]);
    expect(trackAt(t, RULER + H + 5, {})).toBe(1);
    expect(trackAt(t, RULER + 2 * H + 5, {})).toBe(2);
  });

  it("returns 0 for any y below the first track's bottom (current behaviour)", () => {
    // The implementation starts acc at RULER (=84) and only returns null when
    // y >= acc after every track. y in [0, RULER+H) therefore falls in track 0,
    // including y values in the ruler / drop-zone area. This pins the current
    // behaviour; a follow-up could gate on `y < RULER` if upstream differs.
    const t = timeline([track("video", "t0")]);
    expect(trackAt(t, 0, {})).toBe(0);
    expect(trackAt(t, RULER - 1, {})).toBe(0);
    expect(trackAt(t, RULER, {})).toBe(0);
    expect(trackAt(t, RULER + H - 1, {})).toBe(0);
  });

  it("returns null only for a y at or below all tracks' bottom", () => {
    const t = timeline([track("video", "t0")]);
    // First track bottom = RULER + H = 134. y == bottom → not `y < acc` → null.
    expect(trackAt(t, RULER + H, {})).toBeNull();
    expect(trackAt(t, RULER + H + 1, {})).toBeNull();
    expect(trackAt(t, 9999, {})).toBeNull();
  });

  it("uses clamped custom heights", () => {
    const t = timeline([track("video", "t0"), track("video", "t1")]);
    // t0 forced to minHeight=32; first track spans [84, 116), second [116, 166).
    expect(trackAt(t, 100, { t0: 5 })).toBe(0);
    expect(trackAt(t, 120, { t0: 5 })).toBe(1);
  });
});

describe("totalFrames", () => {
  it("returns 0 for an empty timeline", () => {
    expect(totalFrames(timeline([]))).toBe(0);
  });

  it("returns the largest endFrame across tracks (Timeline.swift:16-22)", () => {
    const t = timeline([
      track("video", "t0", [clip({ startFrame: 0, durationFrames: 100 })]),
      track("audio", "t1", [clip({ startFrame: 50, durationFrames: 200 })]), // ends at 250
      track("video", "t2", [clip({ startFrame: 0, durationFrames: 80 })]),
    ]);
    expect(totalFrames(t)).toBe(250);
  });

  it("ignores empty tracks", () => {
    const t = timeline([
      track("video", "t0"),
      track("audio", "t1", [clip({ startFrame: 0, durationFrames: 50 })]),
    ]);
    expect(totalFrames(t)).toBe(50);
  });
});

describe("endFrame", () => {
  it("is startFrame + durationFrames", () => {
    expect(endFrame(clip({ startFrame: 100, durationFrames: 50 }))).toBe(150);
    expect(endFrame(clip({ startFrame: 0, durationFrames: 1 }))).toBe(1);
  });
});

describe("contentWidth", () => {
  it("is ppf * totalFrames + visibleWidth * 0.5 (TimelineView:116-129)", () => {
    expect(contentWidth(100, 4, 800)).toBe(400 + 400); // 800
    expect(contentWidth(0, 4, 800)).toBe(400); // 0 + 400
  });
});

describe("contentHeight", () => {
  it("returns at least visibleHeight when there are no tracks", () => {
    expect(contentHeight(timeline([]), 600, {})).toBe(
      Math.max(600, LAYOUT.rulerHeight + LAYOUT.dropZoneHeight),
    );
  });

  it("returns last track bottom + dropZoneHeight when that exceeds visibleHeight", () => {
    const t = timeline([track("video", "t0"), track("video", "t1")]);
    const lastBottom = trackY(t, 1, {}) + H; // 184
    const expected = lastBottom + LAYOUT.dropZoneHeight; // 244
    expect(contentHeight(t, 100, {})).toBe(Math.max(100, expected));
  });

  it("returns visibleHeight when that exceeds last track bottom + dropZone", () => {
    const t = timeline([track("video", "t0")]);
    const lastBottom = trackY(t, 0, {}) + H; // 134
    const expected = lastBottom + LAYOUT.dropZoneHeight; // 194
    expect(contentHeight(t, 1000, {})).toBe(Math.max(1000, expected));
  });
});

describe("formatTimecode", () => {
  it("formats as MM:SS:FF under one hour", () => {
    expect(formatTimecode(0, 30)).toBe("00:00:00");
    // 1 second + 5 frames at 30 fps
    expect(formatTimecode(35, 30)).toBe("00:01:05");
    // 1 minute, 0 seconds, 10 frames
    expect(formatTimecode(30 * 60 + 10, 30)).toBe("01:00:10");
  });

  it("formats as HH:MM:SS:FF at one hour or above", () => {
    // 1 hour, 0 minutes, 0 seconds, 0 frames
    expect(formatTimecode(30 * 3600, 30)).toBe("01:00:00:00");
    // 1h 2m 3s 4f at 30 fps
    const f = 30 * 3600 + 30 * 123 + 4;
    expect(formatTimecode(f, 30)).toBe("01:02:03:04");
  });

  it("clamps negative input to 0", () => {
    expect(formatTimecode(-50, 30)).toBe("00:00:00");
  });

  it("uses a safe fps of 30 when fps <= 0", () => {
    expect(formatTimecode(30, 0)).toBe("00:01:00");
    expect(formatTimecode(30, -1)).toBe("00:01:00");
  });

  it("truncates fractional frame input (no rounding)", () => {
    expect(formatTimecode(29.9, 30)).toBe("00:00:29");
  });
});

describe("formatClipDuration", () => {
  it("is an alias for formatTimecode", () => {
    expect(formatClipDuration(35, 30)).toBe(formatTimecode(35, 30));
    expect(formatClipDuration(30 * 3600 + 5, 30)).toBe(formatTimecode(30 * 3600 + 5, 30));
  });
});

// Drop-target boundary coverage (merged from beta's geometry.test.ts).
// Local helpers are named distinctly to avoid clashing with the
// track()/timeline() factories above, which use a different signature.
function dtTrack(id: string, type: ClipType): Track {
  return { id, type, muted: false, hidden: false, syncLocked: true, clips: [] };
}

function dtTimeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

describe("dropTargetAt boundaries", () => {
  it("matches upstream dropTargetAt boundary behavior", () => {
    const tl = dtTimeline([dtTrack("v1", "video"), dtTrack("v2", "video"), dtTrack("a1", "audio")]);
    const heights = {};

    expect(trackY(tl, 0, heights)).toBe(84);
    expect(trackY(tl, 1, heights)).toBe(134);
    expect(trackY(tl, 2, heights)).toBe(184);

    expect(dropTargetAt(tl, 50, heights)).toEqual({ kind: "newTrack", index: 0 });
    expect(dropTargetAt(tl, 100, heights)).toEqual({ kind: "existing", trackIndex: 0 });
    expect(dropTargetAt(tl, 130, heights)).toEqual({ kind: "newTrack", index: 1 });
    expect(dropTargetAt(tl, 134, heights)).toEqual({ kind: "newTrack", index: 1 });
    expect(dropTargetAt(tl, 200, heights)).toEqual({ kind: "existing", trackIndex: 2 });
    expect(dropTargetAt(tl, 250, heights)).toEqual({ kind: "newTrack", index: 3 });
  });

  it("targets a new first track on an empty timeline", () => {
    expect(dropTargetAt(dtTimeline([]), 100, {})).toEqual({ kind: "newTrack", index: 0 });
  });
});
