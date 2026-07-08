// Tests for the Timeline -> mpv EDL translation (community-engine playback).
// Pure logic: escaping, gap fill, trim mapping, primary-track selection, and
// the "nothing playable" null cases.

import { describe, expect, it } from "vitest";
import { primaryVideoTrack, timelineToEdl } from "./mpvEdl";
import type { Clip, ClipType, Timeline, Track } from "./types";

function clip(over: Partial<Clip> & { id: string }): Clip {
  return {
    mediaRef: "m-1",
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
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform: { position: { a: 0, b: 0 }, scale: { a: 1, b: 1 }, rotation: 0, anchor: { a: 0, b: 0 } },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...over,
  } as Clip;
}

function track(type: ClipType, clips: Clip[], id = `t-${type}`): Track {
  return { id, type, muted: false, hidden: false, syncLocked: true, clips } as Track;
}

function timeline(tracks: Track[], fps = 30): Timeline {
  return { fps, width: 1280, height: 720, settingsConfigured: true, tracks };
}

const pathOf = (ref: string) => (ref === "m-1" ? "/media/a.mp4" : ref === "m-2" ? "/media/b.mov" : null);

describe("primaryVideoTrack", () => {
  it("picks the lowest-index visual track, skipping audio", () => {
    const audio = track("audio", []);
    const video = track("video", [], "t-main");
    expect(primaryVideoTrack(timeline([audio, video]))?.id).toBe("t-main");
  });
});

describe("timelineToEdl", () => {
  it("returns null with no visual track or no media-backed clips", () => {
    expect(timelineToEdl(timeline([track("audio", [])]), pathOf)).toBeNull();
    // Text-only visual track -> nothing playable.
    const textOnly = track("video", [clip({ id: "c-t", mediaType: "text", mediaRef: "" })]);
    expect(timelineToEdl(timeline([textOnly]), pathOf)).toBeNull();
  });

  it("emits a byte-length-escaped source segment with start/length", () => {
    const t = track("video", [clip({ id: "c1", trimStartFrame: 15, durationFrames: 60 })]);
    const edl = timelineToEdl(timeline([t]), pathOf);
    // %<utf8-bytes>%<path>: "/media/a.mp4" is 12 bytes.
    expect(edl).toBe("edl://%12%/media/a.mp4,start=0.500000,length=2.000000");
  });

  it("does not create an mpv edl for reversed clips", () => {
    const t = timeline([track("video", [clip({ id: "c1", reversed: true })])]);
    expect(timelineToEdl(t, pathOf)).toBeNull();
  });

  it("fills a leading gap with lavfi black at project size/fps", () => {
    const t = track("video", [clip({ id: "c1", startFrame: 30 })]);
    const edl = timelineToEdl(timeline([t]), pathOf)!;
    const [gap, seg] = edl.replace("edl://", "").split(";");
    expect(gap).toContain("av://lavfi:[color=c=black:s=1280x720:r=30:d=1.000000]");
    expect(gap).toContain("length=1.000000");
    expect(seg).toContain("%12%/media/a.mp4");
  });

  it("covers a missing-media clip by widening the gap to the next clip", () => {
    const missing = clip({ id: "c-x", mediaRef: "m-gone", startFrame: 0, durationFrames: 30 });
    const next = clip({ id: "c2", mediaRef: "m-2", startFrame: 30, durationFrames: 30 });
    const edl = timelineToEdl(timeline([track("video", [missing, next])]), pathOf)!;
    const parts = edl.replace("edl://", "").split(";");
    expect(parts).toHaveLength(2);
    expect(parts[0]).toContain("av://lavfi");
    expect(parts[0]).toContain("d=1.000000"); // 30 frames @30fps over the missing clip
    expect(parts[1]).toContain("/media/b.mov");
  });

  it("orders segments by startFrame regardless of clip array order", () => {
    const late = clip({ id: "c2", mediaRef: "m-2", startFrame: 30 });
    const early = clip({ id: "c1", startFrame: 0 });
    const edl = timelineToEdl(timeline([track("video", [late, early])]), pathOf)!;
    const idxA = edl.indexOf("a.mp4");
    const idxB = edl.indexOf("b.mov");
    expect(idxA).toBeGreaterThan(-1);
    expect(idxB).toBeGreaterThan(idxA);
  });

  it("counts multi-byte paths in UTF-8 bytes, not code units", () => {
    const cjk = (ref: string) => (ref === "m-1" ? "/素材/片段.mp4" : null);
    const edl = timelineToEdl(timeline([track("video", [clip({ id: "c1" })])]), cjk)!;
    const bytes = new TextEncoder().encode("/素材/片段.mp4").length;
    expect(edl).toContain(`%${bytes}%/素材/片段.mp4`);
  });

  it("falls back to 30fps when timeline fps is invalid", () => {
    const t = timeline([track("video", [clip({ id: "c1", startFrame: 30 })])], 0);
    const edl = timelineToEdl(t, pathOf)!;
    // 30 frames at the 30fps fallback = 1s leading gap.
    expect(edl).toContain("d=1.000000");
  });
});
