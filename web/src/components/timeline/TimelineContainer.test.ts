import { describe, expect, it, vi } from "vitest";
import * as timelineContainer from "./TimelineContainer";
import { findSnapDelta } from "../../lib/snap";
import type { Clip, ClipType, Timeline, Track } from "../../lib/types";

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
