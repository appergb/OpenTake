/**
 * Regression: dragging / double-clicking a second media item onto the timeline
 * used to REPLACE the first instead of appending. Root cause was a stale mirror
 * in Tauri mode — `applyAndRefresh` relied on the async `timeline_changed` event
 * and never refreshed synchronously, so a rapid second add recomputed
 * `appendStartFrame` from a clip-less mirror, got 0 again, and the core's
 * overwrite-on-place dropped the first clip.
 *
 * These tests mock the Tauri bridge with a faithful-enough core emulation:
 * `editApply` mutates ONLY the server-side timeline (never the zustand mirror),
 * exactly like Tauri where the mirror is only updated by the async event.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Clip, ClipType, MediaItem, Timeline, Transform } from "../lib/types";

const srv = vi.hoisted(() => {
  type SClip = {
    id: string;
    mediaRef: string;
    mediaType: ClipType;
    sourceClipType: ClipType;
    startFrame: number;
    durationFrames: number;
    trimStartFrame: number;
    trimEndFrame: number;
    transform?: Transform;
  };
  type STrack = { id: string; type: string; clips: SClip[] };
  const state: { tracks: STrack[]; version: number; seq: number } = {
    tracks: [],
    version: 0,
    seq: 0,
  };
  // Core overwrite-on-place: clear any clip overlapping [start, end) before placing.
  function clearRegion(track: STrack, start: number, end: number): void {
    track.clips = track.clips.filter(
      (c) => c.startFrame + c.durationFrames <= start || c.startFrame >= end,
    );
  }
  return {
    state,
    reset(): void {
      state.tracks = [];
      state.version = 0;
      state.seq = 0;
    },
    apply(cmd: {
      type: string;
      kind?: string;
      at?: number;
      entries?: Array<{
        mediaRef: string;
        mediaType: ClipType;
        sourceClipType: ClipType;
        trackIndex: number;
        startFrame: number;
        durationFrames: number;
        trimStartFrame?: number;
        trimEndFrame?: number;
        transform?: Transform;
      }>;
    }): { changed: boolean; affectedClipIds: string[] } {
      if (cmd.type === "insertTrack") {
        const at = Math.max(0, Math.min(state.tracks.length, cmd.at ?? state.tracks.length));
        state.tracks.splice(at, 0, {
          id: `t${++state.seq}`,
          type: cmd.kind === "audio" ? "audio" : "video",
          clips: [],
        });
        state.version += 1;
        return { changed: true, affectedClipIds: [] };
      }
      if (cmd.type === "addClips" && cmd.entries) {
        const affectedClipIds: string[] = [];
        for (const e of cmd.entries) {
          const track = state.tracks[e.trackIndex];
          if (!track) continue;
          clearRegion(track, e.startFrame, e.startFrame + e.durationFrames);
          const id = `c${++state.seq}`;
          track.clips.push({
            id,
            mediaRef: e.mediaRef,
            mediaType: e.mediaType,
            sourceClipType: e.sourceClipType,
            startFrame: e.startFrame,
            durationFrames: e.durationFrames,
            trimStartFrame: e.trimStartFrame ?? 0,
            trimEndFrame: e.trimEndFrame ?? 0,
            transform: e.transform,
          });
          affectedClipIds.push(id);
        }
        state.version += 1;
        return { changed: true, affectedClipIds };
      }
      return { changed: false, affectedClipIds: [] };
    },
  };
});

vi.mock("../lib/api", () => ({
  isTauri: true,
  editApply: async (command: { type: string }) => {
    const res = srv.apply(command as never);
    return {
      changed: res.changed,
      actionName: command.type,
      affectedClipIds: res.affectedClipIds,
      timelineVersion: srv.state.version,
      summary: "",
    };
  },
  getTimeline: async () => ({
    timeline: {
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: srv.state.tracks.map((t) => ({
        id: t.id,
        type: t.type,
        muted: false,
        hidden: false,
        syncLocked: true,
        clips: t.clips.map((c) => ({
          id: c.id,
          mediaRef: c.mediaRef,
          mediaType: c.mediaType,
          sourceClipType: c.sourceClipType,
          startFrame: c.startFrame,
          durationFrames: c.durationFrames,
          trimStartFrame: c.trimStartFrame,
          trimEndFrame: c.trimEndFrame,
          speed: 1,
          volume: 1,
          fadeInFrames: 0,
          fadeOutFrames: 0,
          fadeInInterpolation: "linear",
          fadeOutInterpolation: "linear",
          opacity: 1,
          transform: c.transform ?? {
            centerX: 0.5,
            centerY: 0.5,
            width: 1,
            height: 1,
            rotation: 0,
            flipHorizontal: false,
            flipVertical: false,
          },
          crop: { left: 0, top: 0, right: 0, bottom: 0 },
        })),
      })),
    },
    version: srv.state.version,
  }),
  canUndo: async () => false,
  canRedo: async () => false,
}));

// Imported after the mock is registered (vitest hoists vi.mock above imports).
import { addMediaToTimeline, insertTrack, pasteClipsAtPlayhead } from "./editActions";
import { useClipboardStore } from "./clipboardStore";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";

const EMPTY: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [],
};

function video(name: string, width?: number, height?: number): MediaItem {
  // duration 2s * 30fps = 60 frames per clip.
  return { id: name, name, type: "video", duration: 2, width, height, hasAudio: false };
}

function visualClipStarts(): number[] {
  const tl = useProjectStore.getState().timeline;
  const track = tl.tracks.find((t) => t.type === "video");
  return (track?.clips ?? []).map((c) => c.startFrame).sort((a, b) => a - b);
}

function visualClipTransforms(): Transform[] {
  const tl = useProjectStore.getState().timeline;
  const track = tl.tracks.find((t) => t.type === "video");
  return (track?.clips ?? []).map((c) => c.transform);
}

function clipboardClip(transform: Transform): Clip {
  return {
    id: "source-clip",
    mediaRef: "vertical",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 120,
    durationFrames: 60,
    trimStartFrame: 3,
    trimEndFrame: 7,
    speed: 1,
    volume: 1,
    fadeInFrames: 0,
    fadeOutFrames: 0,
    fadeInInterpolation: "linear",
    fadeOutInterpolation: "linear",
    opacity: 1,
    transform,
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
  };
}

describe("addMediaToTimeline", () => {
  beforeEach(() => {
    srv.reset();
    useProjectStore.getState().setMirror(EMPTY, 0);
    useClipboardStore.getState().clear();
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, selectedClipIds: new Set() });
  });

  it("appends a second item after the first when awaited sequentially", async () => {
    await addMediaToTimeline(video("a"));
    await addMediaToTimeline(video("b"));
    expect(visualClipStarts()).toEqual([0, 60]);
  });

  it("appends when two adds are fired without awaiting between them", async () => {
    // Mirrors the real call sites (`void addMediaToTimeline(...)`): a rapid second
    // drop / double-click fires before the first has refreshed the mirror.
    const p1 = addMediaToTimeline(video("a"));
    const p2 = addMediaToTimeline(video("b"));
    await Promise.all([p1, p2]);
    expect(visualClipStarts()).toEqual([0, 60]);
  });

  it("adds vertical media with the upstream aspect-fit transform", async () => {
    await addMediaToTimeline(video("vertical", 1080, 1920));

    const [transform] = visualClipTransforms();
    expect(transform.width).toBeCloseTo(0.31640625);
    expect(transform.height).toBe(1);
  });

  it("pastes copied clips without resetting their transform", async () => {
    await addMediaToTimeline(video("seed"));
    const transform: Transform = {
      centerX: 0.5,
      centerY: 0.5,
      width: 0.31640625,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    };
    useClipboardStore.getState().set([{ clip: clipboardClip(transform), sourceTrackIndex: 0 }], 120);
    useEditorUiStore.setState({ activeFrame: 240, currentFrame: 240 });

    await pasteClipsAtPlayhead();

    const transforms = visualClipTransforms();
    expect(transforms.at(-1)?.width).toBeCloseTo(0.31640625);
    expect(transforms.at(-1)?.height).toBe(1);
  });

  it("forwards an explicit insertTrack index", async () => {
    await insertTrack("video");
    await insertTrack("audio");
    await insertTrack("video", 0);

    expect(srv.state.tracks.map((track) => track.id)).toEqual(["t3", "t1", "t2"]);
  });
});
