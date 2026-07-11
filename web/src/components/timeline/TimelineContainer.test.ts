import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import * as timelineContainer from "./TimelineContainer";
import { findSnapDelta } from "../../lib/snap";
import { LAYOUT } from "../../lib/theme";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";

const timelineContainerSource = readFileSync(new URL("./TimelineContainer.tsx", import.meta.url), "utf8");

function clip(over: Partial<Clip> & { id: string; mediaType: ClipType }): Clip {
  return {
    id: over.id,
    mediaRef: over.mediaRef ?? "asset",
    mediaType: over.mediaType,
    sourceClipType: over.mediaType,
    startFrame: over.startFrame ?? 0,
    durationFrames: over.durationFrames ?? 30,
    trimStartFrame: over.trimStartFrame ?? 0,
    trimEndFrame: over.trimEndFrame ?? 0,
    speed: over.speed ?? 1,
    volume: over.volume ?? 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "smooth",
    fadeOutInterpolation: "smooth",
    opacity: over.opacity ?? 1,
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

function track(id: string, clips: Clip[], type: ClipType = "video"): Track {
  return {
    id,
    type,
    muted: false,
    hidden: false,
    syncLocked: true,
    clips,
  };
}

function timeline(tracks: Track[]): Timeline {
  return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
}

function moveParticipants(tl: Timeline, ids: string[]) {
  return ids.map((id) => {
    for (let trackIndex = 0; trackIndex < tl.tracks.length; trackIndex++) {
      const found = tl.tracks[trackIndex].clips.find((c) => c.id === id);
      if (found) {
        return {
          id,
          trackIndex,
          startFrame: found.startFrame,
          clip: found,
        };
      }
    }
    throw new Error(`missing clip ${id}`);
  });
}

describe("collectMoveSnapTargets", () => {
  it("snaps a moved clip start to the playhead within the 8px/zoom threshold", () => {
    const fn = (
      timelineContainer as {
        collectMoveSnapTargets?: (
          timeline: Timeline,
          excluded: Set<string>,
          activeFrame: number,
        ) => Array<{ frame: number; kind: string }>;
      }
    ).collectMoveSnapTargets;
    const tl = timeline([
      track("v1", [
        clip({ id: "dragged", mediaType: "video", startFrame: 10, durationFrames: 20 }),
        clip({ id: "other", mediaType: "video", startFrame: 80, durationFrames: 10 }),
      ]),
    ]);

    expect(typeof fn).toBe("function");
    const targets = fn?.(tl, new Set(["dragged"]), 42) ?? [];
    expect(targets).toEqual([
      { frame: 80, kind: "clipEdge" },
      { frame: 90, kind: "clipEdge" },
      { frame: 42, kind: "playhead" },
    ]);
    const zoom = 4;
    const movedStartWithinThreshold = 40; // |42 - 40| === 8px / zoom.
    const snap = findSnapDelta([movedStartWithinThreshold], targets, zoom, null, [0]);

    expect(snap).toEqual({ delta: 2, snappedFrame: 42, probeOffset: 0 });
    expect(targets.find((target) => target.frame === snap?.snappedFrame)?.kind).toBe("playhead");
  });
});

describe("resolveExistingTrackMove", () => {
  it("pins linked audio in the audio zone while the video lands on the target video track", () => {
    const tl = timeline([
      track("v2", []),
      track("v1", [
        clip({
          id: "video",
          mediaType: "video",
          startFrame: 100,
          durationFrames: 50,
          linkGroupId: "linked",
        }),
      ]),
      track("a1", [
        clip({
          id: "audio",
          mediaType: "audio",
          startFrame: 106,
          durationFrames: 50,
          linkGroupId: "linked",
        }),
      ], "audio"),
      track("a2", [], "audio"),
    ]);

    const frameDelta = 12;
    const resolved = timelineContainer.resolveExistingTrackMove?.(
      tl,
      moveParticipants(tl, ["video", "audio"]),
      "video",
      -1,
      frameDelta,
    );

    const videoTarget = resolved?.targets.find((target) => target.clipId === "video");
    const audioTarget = resolved?.targets.find((target) => target.clipId === "audio");

    expect(resolved?.trackDelta).toBe(-1);
    expect(videoTarget).toEqual({ clipId: "video", toTrack: 0, toFrame: 112, pinned: false });
    expect(audioTarget).toEqual({ clipId: "audio", toTrack: 2, toFrame: 118, pinned: true });
    expect(tl.tracks[videoTarget?.toTrack ?? -1]?.type).toBe("video");
    expect(tl.tracks[audioTarget?.toTrack ?? -1]?.type).toBe("audio");
    expect((audioTarget?.toFrame ?? 0) - (videoTarget?.toFrame ?? 0)).toBe(6);
  });

  it("moves a pure video multi-selection as one rigid track delta", () => {
    const tl = timeline([
      track("v3", []),
      track("v2", [clip({ id: "upper", mediaType: "video", startFrame: 30, durationFrames: 20 })]),
      track("v1", [clip({ id: "lead", mediaType: "video", startFrame: 50, durationFrames: 20 })]),
      track("a1", [], "audio"),
    ]);

    const resolved = timelineContainer.resolveExistingTrackMove?.(
      tl,
      moveParticipants(tl, ["lead", "upper"]),
      "lead",
      -1,
      5,
    );

    expect(resolved?.trackDelta).toBe(-1);
    expect(Array.from(resolved?.pinnedIds ?? [])).toEqual([]);
    expect(resolved?.targets).toEqual([
      { clipId: "lead", toTrack: 1, toFrame: 55, pinned: false },
      { clipId: "upper", toTrack: 0, toFrame: 35, pinned: false },
    ]);
  });
});

describe("volumeKeyframeMenuItems", () => {
  it("builds delete plus linear/smooth/hold interpolation actions", () => {
    const onDelete = vi.fn();
    const onSetInterpolation = vi.fn();
    const items = timelineContainer.volumeKeyframeMenuItems?.({
      currentInterpolation: "smooth",
      labels: {
        delete: "Delete Keyframe",
        linear: "Linear",
        smooth: "Smooth",
        hold: "Hold",
      },
      onDelete,
      onSetInterpolation,
    });

    expect(items?.map((item) => ({ label: item.label, checked: item.checked }))).toEqual([
      { label: "Delete Keyframe", checked: undefined },
      { label: "Linear", checked: false },
      { label: "Smooth", checked: true },
      { label: "Hold", checked: false },
    ]);

    items?.[0]?.action();
    items?.[1]?.action();
    items?.[2]?.action();
    items?.[3]?.action();

    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onSetInterpolation).toHaveBeenNthCalledWith(1, "linear");
    expect(onSetInterpolation).toHaveBeenNthCalledWith(2, "smooth");
    expect(onSetInterpolation).toHaveBeenNthCalledWith(3, "hold");
  });
});

describe("accessibleClipRects", () => {
  it("maps canvas clips to stable accessible button rectangles", () => {
    const tl = timeline([
      track("v1", [clip({ id: "c1", mediaType: "video", startFrame: 10, durationFrames: 20 })]),
    ]);

    const rects = timelineContainer.accessibleClipRects?.(tl, 5, {}, 12, 7, 500, 200);

    expect(rects).toEqual([
      {
        clipId: "c1",
        trackIndex: 0,
        left: LAYOUT.trackHeaderWidth + 10 * 5 - 12,
        top: LAYOUT.rulerHeight + LAYOUT.dropZoneHeight + 2 - 7,
        width: 20 * 5,
        height: 46,
        label: "Clip c1 on V1",
      },
    ]);
  });

  it("uses the visible timeline track labels for multi-track accessibility", () => {
    const tl = timeline([
      track("v2", [clip({ id: "top", mediaType: "video", startFrame: 0, durationFrames: 10 })]),
      track("v1", [clip({ id: "base", mediaType: "video", startFrame: 20, durationFrames: 10 })]),
      track("a1", [clip({ id: "voice", mediaType: "audio", startFrame: 0, durationFrames: 10 })], "audio"),
    ]);

    const rects = timelineContainer.accessibleClipRects?.(tl, 5, {}, 0, 0, 500, 300);

    expect(rects?.map((r) => r.label)).toEqual([
      "Clip top on V2",
      "Clip base on V1",
      "Clip voice on A1",
    ]);
  });

  it("keeps narrow clip proxies at the WCAG 2.2 minimum target size", () => {
    expect(timelineContainer.clipAccessTargetSize?.(1, 12)).toEqual({ width: 24, height: 24 });
    expect(timelineContainer.clipAccessTargetSize?.(80, 46)).toEqual({ width: 80, height: 46 });
  });

  it("keeps the full 24px canvas and AX footprint aligned at the timeline boundary", () => {
    const tl = timeline([
      track("v1", [clip({ id: "narrow", mediaType: "video", startFrame: 0, durationFrames: 1 })]),
    ]);
    const rect = timelineContainer.accessibleClipRects?.(tl, 1, {}, 0, 0, 500, 200)?.[0];

    expect(timelineContainer.clipAccessTargetRect?.(0, rect?.top ?? 0, 1, 46, 0, 24)).toEqual({
      left: 0,
      top: rect?.top,
      width: 24,
      height: 46,
    });
    expect(rect).toMatchObject({ left: LAYOUT.trackHeaderWidth, width: 24, height: 46 });
    const exactHit = timelineContainer.hitTestAccessibleClip?.(tl, 0, (rect?.top ?? 0) + 23, 1, {});
    expect(exactHit?.clip.id).toBe("narrow");
    expect(exactHit?.region).toBe("trimLeft");
    const haloHit = timelineContainer.hitTestAccessibleClip?.(tl, 24, (rect?.top ?? 0) + 23, 1, {});
    expect(haloHit?.clip.id).toBe("narrow");
    expect(haloHit?.region).toBe("body");
    expect(timelineContainer.hitTestAccessibleClip?.(tl, 24.1, (rect?.top ?? 0) + 23, 1, {})).toBeNull();
  });

  it("uses the same linked-group selection semantics for canvas and AX proxies", () => {
    const tl = timeline([
      track("v1", [clip({ id: "video", mediaType: "video", linkGroupId: "pair" })]),
      track("a1", [clip({ id: "audio", mediaType: "audio", linkGroupId: "pair" })], "audio"),
    ]);

    expect(
      Array.from(timelineContainer.clipSelectionForInteraction?.(tl, new Set(), "video", {}) ?? []).sort(),
    ).toEqual(["audio", "video"]);
    expect(
      Array.from(
        timelineContainer.clipSelectionForInteraction?.(tl, new Set(), "video", { altKey: true }) ?? [],
      ),
    ).toEqual(["video"]);
    expect(timelineContainerSource).not.toContain("selectClips(new Set([rect.clipId]))");
  });

  it("exposes button selection through the pressed state", () => {
    expect(timelineContainerSource).toContain("aria-pressed={selectedClipIds.has(rect.clipId)}");
    expect(timelineContainerSource).not.toContain("aria-selected={selectedClipIds.has(rect.clipId)}");
  });

  it("groups the keyboard clip proxies under a named timeline region", () => {
    expect(timelineContainerSource).toContain('role="group"');
    expect(timelineContainerSource).toContain('aria-label="Timeline clips"');
  });
});

describe("structured media prewarm coordination", () => {
  it("does not let an old project admission block or satisfy the current project", () => {
    const oldKey = timelineContainer.timelinePrewarmKey?.(3, "shared", "/same.mov|online") ?? "";
    const currentKey = timelineContainer.timelinePrewarmKey?.(4, "shared", "/same.mov|online") ?? "";

    expect(oldKey).not.toBe(currentKey);
    expect(
      timelineContainer.timelinePrewarmShouldStart?.(
        currentKey,
        new Set([oldKey]),
        new Map([[oldKey, "cached"]]),
        new Map(),
      ),
    ).toBe(true);
    expect(
      timelineContainer.timelinePrewarmShouldStart?.(
        oldKey,
        new Set([oldKey]),
        new Map([[oldKey, "cached"]]),
        new Map(),
      ),
    ).toBe(false);
  });

  it("retries queued duplicate and busy admissions without retrying terminal states", () => {
    expect(timelineContainer.prewarmResultNeedsRetry?.("queued")).toBe(true);
    expect(timelineContainer.prewarmResultNeedsRetry?.("duplicate")).toBe(true);
    expect(timelineContainer.prewarmResultNeedsRetry?.("busy")).toBe(true);
    expect(timelineContainer.prewarmResultNeedsRetry?.("cached")).toBe(false);
    expect(timelineContainer.prewarmResultNeedsRetry?.("staleProject")).toBe(false);
    expect(timelineContainer.prewarmResultNeedsRetry?.(null)).toBe(false);
  });

  it("reads prewarmed timeline visuals only after the backend reports cached", () => {
    expect(timelineContainer.prewarmResultAllowsCacheRead?.("cached")).toBe(true);
    for (const result of ["queued", "duplicate", "busy", "staleProject", null] as const) {
      expect(timelineContainer.prewarmResultAllowsCacheRead?.(result)).toBe(false);
    }
  });
});
