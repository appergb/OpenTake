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
import type { MediaItem, Timeline } from "../lib/types";

const srv = vi.hoisted(() => {
  type SClip = { startFrame: number; durationFrames: number };
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
      entries?: Array<{ trackIndex: number; startFrame: number; durationFrames: number }>;
    }): boolean {
      if (cmd.type === "insertTrack") {
        state.tracks.push({
          id: `t${++state.seq}`,
          type: cmd.kind === "audio" ? "audio" : "video",
          clips: [],
        });
        state.version += 1;
        return true;
      }
      if (cmd.type === "addClips" && cmd.entries) {
        for (const e of cmd.entries) {
          const track = state.tracks[e.trackIndex];
          if (!track) continue;
          clearRegion(track, e.startFrame, e.startFrame + e.durationFrames);
          track.clips.push({ startFrame: e.startFrame, durationFrames: e.durationFrames });
        }
        state.version += 1;
        return true;
      }
      return false;
    },
  };
});

vi.mock("../lib/api", () => ({
  isTauri: true,
  editApply: async (command: { type: string }) => ({
    changed: srv.apply(command as never),
    actionName: command.type,
    affectedClipIds: [],
    timelineVersion: srv.state.version,
    summary: "",
  }),
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
        clips: t.clips.map((c, i) => ({
          id: `${t.id}-c${i}`,
          startFrame: c.startFrame,
          durationFrames: c.durationFrames,
        })),
      })),
    },
    version: srv.state.version,
  }),
  canUndo: async () => false,
  canRedo: async () => false,
}));

// Imported after the mock is registered (vitest hoists vi.mock above imports).
import { addMediaToTimeline } from "./editActions";
import { useProjectStore } from "./projectStore";

const EMPTY: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: true,
  tracks: [],
};

function video(name: string): MediaItem {
  // duration 2s * 30fps = 60 frames per clip.
  return { id: name, name, type: "video", duration: 2, hasAudio: false };
}

function visualClipStarts(): number[] {
  const tl = useProjectStore.getState().timeline;
  const track = tl.tracks.find((t) => t.type === "video");
  return (track?.clips ?? []).map((c) => c.startFrame).sort((a, b) => a - b);
}

describe("addMediaToTimeline", () => {
  beforeEach(() => {
    srv.reset();
    useProjectStore.getState().setMirror(EMPTY, 0);
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
});
