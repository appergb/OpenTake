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
import type {
  Clip,
  ClipType,
  MediaItem,
  ProjectEditIdentity,
  Timeline,
  Track,
  Transform,
} from "../lib/types";

const srv = vi.hoisted(() => {
  let blockedApply: Promise<void> | null = null;
  let releaseBlockedApply: (() => void) | null = null;
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
  type SCommand = {
    type: string;
    kind?: string;
    at?: number;
    fps?: number;
    width?: number;
    height?: number;
    settings?: { fps: number; width: number; height: number };
    sequenceId?: string;
    target?:
      | { kind: "existingTrack"; trackId: string }
      | { kind: "newTrack"; trackType: ClipType; at?: number };
    entry?: {
      mediaRef: string;
      mediaType: ClipType;
      sourceClipType: ClipType;
      startFrame: number;
      durationFrames: number;
      trimStartFrame?: number;
      trimEndFrame?: number;
      hasAudio?: boolean;
      addLinkedAudio?: boolean;
      transform?: Transform;
    };
    entries?: Array<{
      mediaRef?: string;
      mediaType?: ClipType;
      sourceClipType?: ClipType;
      trackIndex?: number;
      startFrame: number;
      durationFrames: number;
      trimStartFrame?: number;
      trimEndFrame?: number;
      transform?: Transform;
      clip?: Clip;
      targetTrackId?: string;
    }>;
    trackIndex?: number;
    atFrame?: number;
    clipIds?: string[];
    ranges?: Array<{ start: number; end: number }>;
    a?: number;
    b?: number;
  };
  const state: {
    tracks: STrack[];
    version: number;
    seq: number;
    fps: number;
    width: number;
    height: number;
    settingsConfigured: boolean;
    commands: SCommand[];
    projectEpoch: number;
    projectPath: string | null;
    applyEntered: number;
    noopNext: boolean;
    errorNext: Error | null;
  } = {
    tracks: [],
    version: 0,
    seq: 0,
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    commands: [],
    projectEpoch: 1,
    projectPath: null,
    applyEntered: 0,
    noopNext: false,
    errorNext: null,
  };
  // Core overwrite-on-place: clear any clip overlapping [start, end) before placing.
  function clearRegion(track: STrack, start: number, end: number): void {
    track.clips = track.clips.filter(
      (c) => c.startFrame + c.durationFrames <= start || c.startFrame >= end,
    );
  }
  return {
    state,
    blockNextApply(): () => void {
      blockedApply = new Promise<void>((resolve) => {
        releaseBlockedApply = resolve;
      });
      return () => {
        releaseBlockedApply?.();
        releaseBlockedApply = null;
      };
    },
    async beforeApply(): Promise<void> {
      state.applyEntered += 1;
      const gate = blockedApply;
      blockedApply = null;
      if (gate) await gate;
    },
    noopNext(): void {
      state.noopNext = true;
    },
    errorNext(error: Error): void {
      state.errorNext = error;
    },
    reset(): void {
      releaseBlockedApply?.();
      releaseBlockedApply = null;
      blockedApply = null;
      state.tracks = [];
      state.version = 0;
      state.seq = 0;
      state.fps = 30;
      state.width = 1920;
      state.height = 1080;
      state.settingsConfigured = true;
      state.commands = [];
      state.projectEpoch = 1;
      state.projectPath = null;
      state.applyEntered = 0;
      state.noopNext = false;
      state.errorNext = null;
    },
    apply(cmd: SCommand): { changed: boolean; affectedClipIds: string[] } {
      state.commands.push(cmd);
      if (state.noopNext) {
        state.noopNext = false;
        return { changed: false, affectedClipIds: [] };
      }
      if (
        cmd.type === "setTimelineSettings" &&
        cmd.fps !== undefined &&
        cmd.width !== undefined &&
        cmd.height !== undefined
      ) {
        state.fps = cmd.fps;
        state.width = cmd.width;
        state.height = cmd.height;
        state.settingsConfigured = true;
        state.version += 1;
        return { changed: true, affectedClipIds: [] };
      }
      if (cmd.type === "placeMedia" && cmd.entry && cmd.target) {
        if (cmd.settings) {
          state.fps = cmd.settings.fps;
          state.width = cmd.settings.width;
          state.height = cmd.settings.height;
          state.settingsConfigured = true;
        }
        let track: STrack | undefined;
        if (cmd.target.kind === "existingTrack") {
          const trackId = cmd.target.trackId;
          track = state.tracks.find((candidate) => candidate.id === trackId);
        } else {
          const firstAudio = state.tracks.findIndex((candidate) => candidate.type === "audio");
          const zone = firstAudio < 0 ? state.tracks.length : firstAudio;
          const requested = Math.max(0, Math.min(state.tracks.length, cmd.target.at ?? state.tracks.length));
          const at = cmd.target.trackType === "audio" ? Math.max(requested, zone) : Math.min(requested, zone);
          track = {
            id: `t${++state.seq}`,
            type: cmd.target.trackType === "audio" ? "audio" : "video",
            clips: [],
          };
          state.tracks.splice(at, 0, track);
        }
        if (!track) return { changed: false, affectedClipIds: [] };
        const entry = cmd.entry;
        clearRegion(track, entry.startFrame, entry.startFrame + entry.durationFrames);
        const id = `c${++state.seq}`;
        track.clips.push({
          id,
          mediaRef: entry.mediaRef,
          mediaType: entry.mediaType,
          sourceClipType: entry.sourceClipType,
          startFrame: entry.startFrame,
          durationFrames: entry.durationFrames,
          trimStartFrame: entry.trimStartFrame ?? 0,
          trimEndFrame: entry.trimEndFrame ?? 0,
          transform: entry.transform,
        });
        state.version += 1;
        return { changed: true, affectedClipIds: [id] };
      }
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
          const track = state.tracks[e.trackIndex ?? -1];
          if (!track) continue;
          clearRegion(track, e.startFrame, e.startFrame + e.durationFrames);
          const id = `c${++state.seq}`;
          track.clips.push({
            id,
            mediaRef: e.mediaRef ?? "",
            mediaType: e.mediaType ?? "video",
            sourceClipType: e.sourceClipType ?? "video",
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
      if (cmd.type === "pasteClips" && cmd.entries) {
        const affectedClipIds: string[] = [];
        for (const entry of cmd.entries) {
          if (!entry.clip || !entry.targetTrackId) continue;
          const track = state.tracks.find((candidate) => candidate.id === entry.targetTrackId);
          if (!track) continue;
          clearRegion(track, entry.startFrame, entry.startFrame + entry.clip.durationFrames);
          const id = `c${++state.seq}`;
          track.clips.push({
            id,
            mediaRef: entry.clip.mediaRef,
            mediaType: entry.clip.mediaType,
            sourceClipType: entry.clip.sourceClipType,
            startFrame: entry.startFrame,
            durationFrames: entry.clip.durationFrames,
            trimStartFrame: entry.clip.trimStartFrame,
            trimEndFrame: entry.clip.trimEndFrame,
            transform: structuredClone(entry.clip.transform),
          });
          affectedClipIds.push(id);
        }
        if (affectedClipIds.length === 0) return { changed: false, affectedClipIds };
        state.version += 1;
        return { changed: true, affectedClipIds };
      }
      if (
        cmd.type === "insertClips" &&
        cmd.entries &&
        cmd.trackIndex !== undefined &&
        cmd.atFrame !== undefined
      ) {
        const track = state.tracks[cmd.trackIndex];
        if (!track) return { changed: false, affectedClipIds: [] };
        const duration = cmd.entries.reduce((sum, entry) => sum + entry.durationFrames, 0);
        for (const existing of track.clips) {
          if (existing.startFrame >= cmd.atFrame) existing.startFrame += duration;
        }
        const affectedClipIds: string[] = [];
        for (const entry of cmd.entries) {
          const id = `c${++state.seq}`;
          track.clips.push({
            id,
            mediaRef: entry.mediaRef ?? "",
            mediaType: entry.mediaType ?? "video",
            sourceClipType: entry.sourceClipType ?? "video",
            startFrame: entry.startFrame,
            durationFrames: entry.durationFrames,
            trimStartFrame: entry.trimStartFrame ?? 0,
            trimEndFrame: entry.trimEndFrame ?? 0,
            transform: entry.transform,
          });
          affectedClipIds.push(id);
        }
        state.version += 1;
        return { changed: true, affectedClipIds };
      }
      if (cmd.type === "addTextsAutoTrack" && cmd.entries && cmd.entries.length > 0) {
        // Mirrors the ops-layer command: always insert a fresh video track at
        // index 0, then place every entry there (#194 — never reuse whatever
        // track the caller already has at index 0).
        state.tracks.splice(0, 0, {
          id: `t${++state.seq}`,
          type: "video",
          clips: [],
        });
        const track = state.tracks[0];
        const affectedClipIds: string[] = [];
        for (const e of cmd.entries) {
          clearRegion(track, e.startFrame, e.startFrame + e.durationFrames);
          const id = `c${++state.seq}`;
          track.clips.push({
            id,
            mediaRef: "",
            mediaType: "text",
            sourceClipType: "text",
            startFrame: e.startFrame,
            durationFrames: e.durationFrames,
            trimStartFrame: 0,
            trimEndFrame: 0,
            transform: e.transform,
          });
          affectedClipIds.push(id);
        }
        state.version += 1;
        return { changed: true, affectedClipIds };
      }
      if (cmd.type === "swapTracks" && cmd.a !== undefined && cmd.b !== undefined) {
        const first = state.tracks[cmd.a];
        const second = state.tracks[cmd.b];
        if (!first || !second || first.type !== second.type || cmd.a === cmd.b) {
          return { changed: false, affectedClipIds: [] };
        }
        [state.tracks[cmd.a], state.tracks[cmd.b]] = [second, first];
        state.version += 1;
        return { changed: true, affectedClipIds: [] };
      }
      if (cmd.type === "rippleDeleteClips" && cmd.clipIds) {
        const selected = new Set(cmd.clipIds);
        const removed: string[] = [];
        for (const track of state.tracks) {
          for (const clip of track.clips) {
            if (selected.has(clip.id)) removed.push(clip.id);
          }
          track.clips = track.clips.filter((clip) => !selected.has(clip.id));
        }
        if (removed.length === 0) return { changed: false, affectedClipIds: [] };
        state.version += 1;
        return { changed: true, affectedClipIds: removed };
      }
      if (
        cmd.type === "rippleDeleteRanges" &&
        cmd.trackIndex !== undefined &&
        cmd.ranges
      ) {
        state.version += 1;
        return { changed: true, affectedClipIds: [] };
      }
      return { changed: false, affectedClipIds: [] };
    },
  };
});

vi.mock("../lib/api", () => ({
  isTauri: true,
  editApply: async (command: { type: string }, expected?: ProjectEditIdentity) => {
    await srv.beforeApply();
    if (
      command.type === "placeMedia" &&
      expected &&
      (expected.projectEpoch !== srv.state.projectEpoch ||
        expected.projectPath !== srv.state.projectPath ||
        expected.timelineVersion !== srv.state.version)
    ) {
      throw new Error("stale project edit identity");
    }
    if (srv.state.errorNext) {
      const error = srv.state.errorNext;
      srv.state.errorNext = null;
      throw error;
    }
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
      fps: srv.state.fps,
      width: srv.state.width,
      height: srv.state.height,
      settingsConfigured: srv.state.settingsConfigured,
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
    projectEpoch: srv.state.projectEpoch,
    version: srv.state.version,
    projectPath: srv.state.projectPath,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  }),
  canUndo: async () => false,
  canRedo: async () => false,
}));

// Imported after the mock is registered (vitest hoists vi.mock above imports).
import {
  addMediaToTimeline,
  addMediaToTimelineAt,
  addMomentToTimelineAt,
  addTextClip,
  applyAutomationCommands,
  buildMediaInsertPlan,
  insertClips,
  insertTrack,
  mediaDurationFrames,
  momentDurationFrames,
  pasteClipsAtPlayhead,
  rippleDeleteMarkedRange,
  rippleDeleteSelectedClips,
  rippleDeleteSelectedGap,
  resolveMediaDropTrack,
  swapTracks,
} from "./editActions";
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

function setMirror(timeline: Timeline, version: number, projectEpoch: number): void {
  srv.state.version = version;
  srv.state.projectEpoch = projectEpoch;
  srv.state.projectPath = null;
  useProjectStore.getState().clearProjectSnapshot();
  useProjectStore.getState().replaceProjectSnapshot({
    timeline,
    version,
    projectEpoch,
    projectPath: null,
    compatibilityReadOnly: false,
    compatibilityBlockers: [],
  });
}

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

function rippleClip(id: string, startFrame: number, durationFrames: number): Clip {
  return {
    ...clipboardClip({
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    }),
    id,
    mediaRef: id,
    startFrame,
    durationFrames,
  };
}

function setRippleTimeline(timeline: Timeline): void {
  srv.state.tracks = timeline.tracks.map((track) => ({
    id: track.id,
    type: track.type,
    clips: track.clips.map((clip) => ({
      id: clip.id,
      mediaRef: clip.mediaRef,
      mediaType: clip.mediaType,
      sourceClipType: clip.sourceClipType,
      startFrame: clip.startFrame,
      durationFrames: clip.durationFrames,
      trimStartFrame: clip.trimStartFrame,
      trimEndFrame: clip.trimEndFrame,
      transform: clip.transform,
    })),
  }));
  setMirror(timeline, 0, 1);
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

describe("ripple delete action routing", () => {
  beforeEach(() => {
    srv.reset();
    setRippleTimeline({
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: [
        {
          id: "video-track",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: false,
          clips: [rippleClip("lead", 0, 30), rippleClip("tail", 60, 30)],
        },
      ],
    });
    useEditorUiStore.setState({
      activeFrame: 0,
      currentFrame: 0,
      selectedClipIds: new Set(),
      selectedTimelineRange: null,
      selectedGap: null,
    });
  });

  it("routes Shift+Backspace on selected clips and clears selection after refresh", async () => {
    useEditorUiStore.getState().selectClips(new Set(["lead"]));

    await rippleDeleteSelectedClips();

    expect(srv.state.commands.at(-1)).toEqual({
      type: "rippleDeleteClips",
      clipIds: ["lead"],
    });
    expect(useEditorUiStore.getState().selectedClipIds.size).toBe(0);
    expect(useProjectStore.getState().timeline.tracks[0]?.clips.map((clip) => clip.id)).toEqual([
      "tail",
    ]);
  });

  it("routes a marked range from the selected clip track and clears range plus selection", async () => {
    useEditorUiStore.getState().selectClips(new Set(["lead"]));
    useEditorUiStore.setState({ selectedTimelineRange: { startFrame: 10, endFrame: 20 } });

    await expect(rippleDeleteMarkedRange()).resolves.toBe(true);

    expect(srv.state.commands.at(-1)).toEqual({
      type: "rippleDeleteRanges",
      trackIndex: 0,
      ranges: [{ start: 10, end: 20 }],
    });
    expect(useEditorUiStore.getState().selectedTimelineRange).toBeNull();
    expect(useEditorUiStore.getState().selectedClipIds.size).toBe(0);
  });

  it("routes a bounded selected gap and refuses an out-of-band filled gap", async () => {
    useEditorUiStore.getState().selectGap({ trackIndex: 0, startFrame: 30, endFrame: 60 });

    await expect(rippleDeleteSelectedGap()).resolves.toBe(true);
    expect(srv.state.commands.at(-1)).toEqual({
      type: "rippleDeleteRanges",
      trackIndex: 0,
      ranges: [{ start: 30, end: 60 }],
    });
    expect(useEditorUiStore.getState().selectedGap).toBeNull();

    setRippleTimeline({
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: [
        {
          id: "video-track",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: false,
          clips: [
            rippleClip("lead", 0, 30),
            rippleClip("filled", 45, 15),
            rippleClip("tail", 60, 30),
          ],
        },
      ],
    });
    useEditorUiStore.getState().selectGap({ trackIndex: 0, startFrame: 30, endFrame: 60 });
    await expect(rippleDeleteSelectedGap()).resolves.toBe(false);
    expect(srv.state.commands).toHaveLength(1);
    expect(useEditorUiStore.getState().selectedGap).toBeNull();
  });
});

describe("addMediaToTimeline", () => {
  beforeEach(() => {
    srv.reset();
    setMirror(EMPTY, 0, 1);
    useClipboardStore.getState().clear();
    useEditorUiStore.getState().exitNestedSequence();
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

  it("retains the preferred track id when an earlier queued add shifts indexes", async () => {
    const tracks: Track[] = [
      { id: "track-a", type: "video", muted: false, hidden: false, syncLocked: true, clips: [] },
      { id: "track-b", type: "video", muted: false, hidden: false, syncLocked: true, clips: [] },
    ];
    srv.state.tracks = tracks.map((track) => ({ id: track.id, type: track.type, clips: [] }));
    setMirror({ ...EMPTY, tracks }, 0, 1);

    const insertsBefore = addMediaToTimelineAt(video("first"), 0, null, 0);
    const targetsTrackB = addMediaToTimelineAt(video("second"), 0, 1);
    await Promise.all([insertsBefore, targetsTrackB]);

    const timeline = useProjectStore.getState().timeline;
    expect(timeline.tracks.find((track) => track.id === "track-b")?.clips).toEqual([
      expect.objectContaining({ mediaRef: "second", startFrame: 0 }),
    ]);
    expect(timeline.tracks.find((track) => track.id === "track-a")?.clips).toEqual([]);
  });

  it("breaks an already queued chain when project A is replaced by project B", async () => {
    const release = srv.blockNextApply();
    const first = addMediaToTimeline(video("a"));
    const second = addMediaToTimeline(video("b"));
    await vi.waitFor(() => expect(srv.state.applyEntered).toBe(1));

    srv.state.projectEpoch = 2;
    srv.state.projectPath = "/project-b.opentake";
    srv.state.version = 0;
    srv.state.tracks = [];
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: EMPTY,
      projectEpoch: 2,
      version: 0,
      projectPath: "/project-b.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    });
    release();

    const settled = await Promise.allSettled([first, second]);
    expect(settled.map((entry) => entry.status)).toEqual(["rejected", "rejected"]);
    expect(srv.state.commands).toEqual([]);

    await addMediaToTimeline(video("project-b-media"));
    expect(useProjectStore.getState().timeline.tracks[0].clips[0].mediaRef).toBe("project-b-media");
  });

  it("breaks an already queued chain after an external version advance", async () => {
    const release = srv.blockNextApply();
    const first = addMediaToTimeline(video("a"));
    const second = addMediaToTimeline(video("b"));
    await vi.waitFor(() => expect(srv.state.applyEntered).toBe(1));

    srv.state.version = 1;
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: EMPTY,
      projectEpoch: 1,
      version: 1,
      projectPath: null,
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    });
    release();

    const settled = await Promise.allSettled([first, second]);
    expect(settled.map((entry) => entry.status)).toEqual(["rejected", "rejected"]);
    expect(srv.state.commands).toEqual([]);

    await addMediaToTimeline(video("after-external-edit"));
    expect(useProjectStore.getState().timelineVersion).toBe(2);
  });

  it("breaks an already queued chain when Save As changes the project path", async () => {
    const release = srv.blockNextApply();
    const first = addMediaToTimeline(video("a"));
    const second = addMediaToTimeline(video("b"));
    await vi.waitFor(() => expect(srv.state.applyEntered).toBe(1));

    srv.state.projectPath = "/saved-as.opentake";
    useProjectStore.getState().setProjectPath("/saved-as.opentake");
    release();

    const settled = await Promise.allSettled([first, second]);
    expect(settled.map((entry) => entry.status)).toEqual(["rejected", "rejected"]);
    expect(srv.state.commands).toEqual([]);

    await addMediaToTimeline(video("after-save-as"));
    expect(useProjectStore.getState().projectPath).toBe("/saved-as.opentake");
  });

  it("breaks queued placement before dispatch when the active sequence changes", async () => {
    const mismatched = { ...video("mismatch", 3840, 2160), sourceFps: 24 };
    const first = addMediaToTimeline(mismatched);
    const second = addMediaToTimeline(video("queued"));
    await vi.waitFor(() => expect(useEditorUiStore.getState().projectSettingsPrompt).not.toBeNull());

    useEditorUiStore.getState().enterNestedSequence("another-sequence");
    useEditorUiStore.getState().resolveProjectSettingsPrompt(false);
    const settled = await Promise.allSettled([first, second]);

    expect(settled.map((entry) => entry.status)).toEqual(["rejected", "rejected"]);
    expect(srv.state.commands).toEqual([]);
    useEditorUiStore.getState().exitNestedSequence();
    await addMediaToTimeline(video("after-sequence-switch"));
    expect(srv.state.commands).toHaveLength(1);
  });

  it.each(["no-op", "error"] as const)(
    "breaks queued placement after a %s and lets a later queue recover",
    async (failure) => {
      if (failure === "no-op") srv.noopNext();
      else srv.errorNext(new Error("injected placement failure"));
      const first = addMediaToTimeline(video("a"));
      const second = addMediaToTimeline(video("b"));

      const settled = await Promise.allSettled([first, second]);
      expect(settled.map((entry) => entry.status)).toEqual(["rejected", "rejected"]);
      expect(srv.state.version).toBe(0);

      await addMediaToTimeline(video("recovered"));
      expect(useProjectStore.getState().timeline.tracks[0].clips[0].mediaRef).toBe("recovered");
      expect(srv.state.version).toBe(1);
    },
  );

  it("applies first-video settings before placing the first clip", async () => {
    srv.state.settingsConfigured = false;
    setMirror({ ...EMPTY, settingsConfigured: false }, 0, 1);

    await addMediaToTimeline({
      ...video("first", 3840, 2160),
      sourceFps: 23.976,
    });

    expect(srv.state.commands.map((command) => command.type)).toEqual(["placeMedia"]);
    expect(srv.state.commands[0]).toMatchObject({
      type: "placeMedia",
      settings: { fps: 24, width: 3840, height: 2160 },
      target: { kind: "newTrack", trackType: "video" },
    });
    expect(srv.state.commands[0].entry).toMatchObject({
      mediaRef: "first",
      durationFrames: 48,
    });
    expect(useProjectStore.getState().timeline).toMatchObject({
      fps: 24,
      width: 3840,
      height: 2160,
      settingsConfigured: true,
    });
  });

  it("waits for configured-empty mismatch choice and preserves either decision", async () => {
    const mismatched = { ...video("mismatch", 3840, 2160), sourceFps: 24 };
    const keep = addMediaToTimeline(mismatched);
    await vi.waitFor(() => expect(useEditorUiStore.getState().projectSettingsPrompt).not.toBeNull());
    useEditorUiStore.getState().resolveProjectSettingsPrompt(false);
    await keep;
    expect(srv.state.commands[0].settings).toBeUndefined();
    expect(srv.state.commands.at(-1)?.entry).toMatchObject({ durationFrames: 60 });

    srv.reset();
    setMirror(EMPTY, 0, 1);
    const match = addMediaToTimeline(mismatched);
    await vi.waitFor(() => expect(useEditorUiStore.getState().projectSettingsPrompt).not.toBeNull());
    useEditorUiStore.getState().resolveProjectSettingsPrompt(true);
    await match;
    expect(srv.state.commands.map((command) => command.type)).toEqual(["placeMedia"]);
    expect(srv.state.commands[0].settings).toEqual({ fps: 24, width: 3840, height: 2160 });
    expect(srv.state.commands.at(-1)?.entry).toMatchObject({ durationFrames: 48 });
  });

  it("drops overlapping media onto a new top overlay track instead of overwriting", async () => {
    await addMediaToTimeline(video("base"));
    await addMediaToTimelineAt(video("overlay"), 0, 0);

    const videoTracks = useProjectStore.getState().timeline.tracks.filter((t) => t.type === "video");
    expect(videoTracks).toHaveLength(2);
    expect(videoTracks[0].clips.map((c) => [c.mediaRef, c.startFrame])).toEqual([["overlay", 0]]);
    expect(videoTracks[1].clips.map((c) => [c.mediaRef, c.startFrame])).toEqual([["base", 0]]);
  });

  it("moves the playhead to the dropped clip so preview shows the new media", async () => {
    await addMediaToTimeline(video("base"));
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, previewMediaId: "base" });

    await addMediaToTimelineAt(video("later"), 180, 0);

    const ui = useEditorUiStore.getState();
    expect(ui.previewMediaId).toBeNull();
    expect(ui.currentFrame).toBe(180);
    expect(ui.activeFrame).toBe(180);
  });

  it("selects ripple-inserted media and moves the playhead to the insertion frame", async () => {
    await addMediaToTimeline(video("base"));
    useEditorUiStore.setState({
      activeFrame: 240,
      currentFrame: 240,
      previewMediaId: "base",
      selectedClipIds: new Set(["old-selection"]),
    });

    await insertClips(0, 30, [
      {
        mediaRef: "ripple",
        mediaType: "video",
        sourceClipType: "video",
        trackIndex: 0,
        startFrame: 30,
        durationFrames: 60,
      },
    ]);

    const inserted = useProjectStore.getState().timeline.tracks[0]?.clips.find((clip) => clip.mediaRef === "ripple");
    const ui = useEditorUiStore.getState();
    expect(inserted).toBeDefined();
    expect(Array.from(ui.selectedClipIds)).toEqual([inserted?.id]);
    expect(ui.previewMediaId).toBeNull();
    expect(ui.currentFrame).toBe(30);
    expect(ui.activeFrame).toBe(30);
  });

  it("adds vertical media with the upstream aspect-fit transform", async () => {
    const add = addMediaToTimeline(video("vertical", 1080, 1920));
    await vi.waitFor(() => expect(useEditorUiStore.getState().projectSettingsPrompt).not.toBeNull());
    useEditorUiStore.getState().resolveProjectSettingsPrompt(false);
    await add;

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

  it("forwards swapTracks for whole-track reordering", async () => {
    await insertTrack("video");
    await insertTrack("video");
    await swapTracks(0, 1);

    expect(srv.state.tracks.map((track) => track.id)).toEqual(["t2", "t1"]);
  });
});

describe("applyAutomationCommands", () => {
  beforeEach(() => {
    srv.reset();
    setMirror(EMPTY, 0, 1);
  });

  it("accepts one atomic request and refuses a multi-command pseudo-transaction", async () => {
    const result = await applyAutomationCommands([{ type: "insertTrack", kind: "video" }]);
    expect(result).toHaveLength(1);
    expect(srv.state.commands.map((command) => command.type)).toEqual(["insertTrack"]);

    await expect(
      applyAutomationCommands([
        { type: "insertTrack", kind: "video" },
        { type: "insertTrack", kind: "audio" },
      ]),
    ).rejects.toThrow("one atomic EditRequest");
    expect(srv.state.commands.map((command) => command.type)).toEqual(["insertTrack"]);
  });
});

// The drop ghost must show EXACTLY where the clip will land, so its track
// resolver has to mirror `addMediaToTimelineAtInner`'s placement rules.
describe("resolveMediaDropTrack (drop-ghost truthfulness)", () => {
  function mkClip(id: string, startFrame: number, durationFrames: number, type: ClipType = "video"): Clip {
    return {
      id,
      mediaRef: id,
      mediaType: type,
      sourceClipType: type,
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
    };
  }
  function mkTrack(id: string, type: ClipType, clips: Clip[]): Track {
    return { id, type, muted: false, hidden: false, syncLocked: true, clips };
  }
  function mkTl(tracks: Track[]): Timeline {
    return { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks };
  }
  const videoItem: MediaItem = { id: "v", name: "v", type: "video", duration: 2, hasAudio: false };
  const audioItem: MediaItem = { id: "a", name: "a", type: "audio", duration: 2, hasAudio: true };

  it("lands on the hovered track when it is free", () => {
    const tl = mkTl([mkTrack("t1", "video", [])]);
    expect(resolveMediaDropTrack(tl, videoItem, 0, { kind: "existing", trackIndex: 0 })).toEqual({
      trackIndex: 0,
      newTrack: null,
    });
  });

  it("passes an insert-zone hover through as a new track", () => {
    const tl = mkTl([mkTrack("t1", "video", [])]);
    expect(resolveMediaDropTrack(tl, videoItem, 90, { kind: "newTrack", index: 0 })).toEqual({
      trackIndex: null,
      newTrack: { index: 0, type: "video" },
    });
  });

  it("falls back to a new lane when the only compatible track is occupied at the drop point", () => {
    // Same scenario the addMediaToTimelineAt overlap test exercises: a clip sits
    // at [0,60) on the sole video track, so a video dropped at 0 opens a new lane.
    const tl = mkTl([mkTrack("t1", "video", [mkClip("c", 0, 60)])]);
    expect(resolveMediaDropTrack(tl, videoItem, 0, { kind: "existing", trackIndex: 0 })).toEqual({
      trackIndex: null,
      newTrack: { index: 0, type: "video" },
    });
  });

  it("stays on the occupied lane when the drop point itself is free", () => {
    const tl = mkTl([mkTrack("t1", "video", [mkClip("c", 0, 60)])]);
    expect(resolveMediaDropTrack(tl, videoItem, 120, { kind: "existing", trackIndex: 0 })).toEqual({
      trackIndex: 0,
      newTrack: null,
    });
  });

  it("routes audio to a compatible audio lane even when hovering a video track", () => {
    const tl = mkTl([mkTrack("t1", "video", []), mkTrack("t2", "audio", [])]);
    expect(resolveMediaDropTrack(tl, audioItem, 0, { kind: "existing", trackIndex: 0 })).toEqual({
      trackIndex: 1,
      newTrack: null,
    });
  });

  it("creates an audio track when none exists", () => {
    const tl = mkTl([mkTrack("t1", "video", [])]);
    expect(resolveMediaDropTrack(tl, audioItem, 0, { kind: "existing", trackIndex: 0 })).toEqual({
      trackIndex: null,
      newTrack: { index: 0, type: "audio" },
    });
  });
});

describe("mediaDurationFrames", () => {
  it("converts source seconds to frames", () => {
    const item: MediaItem = { id: "v", name: "v", type: "video", duration: 2, hasAudio: false };
    expect(mediaDurationFrames(item, 30)).toBe(60);
  });

  it("uses the still-image default for zero-duration items", () => {
    const item: MediaItem = { id: "i", name: "i", type: "image", duration: 0, hasAudio: false };
    expect(mediaDurationFrames(item, 30)).toBe(150);
  });

  it("never returns less than one frame", () => {
    const item: MediaItem = { id: "v", name: "v", type: "video", duration: 0.001, hasAudio: false };
    expect(mediaDurationFrames(item, 30)).toBe(1);
  });
});

describe("momentDurationFrames", () => {
  it("returns the range length in frames", () => {
    expect(momentDurationFrames({ startSec: 3, endSec: 6 }, 30)).toBe(90);
  });

  it("never returns less than one frame for a tiny range", () => {
    expect(momentDurationFrames({ startSec: 3, endSec: 3.001 }, 30)).toBe(1);
  });

  it("seconds_to_frame_truncates_fractional_boundaries", async () => {
    for (const fps of [24, 30]) {
      for (const [frames, expected] of [
        [0.49, 1],
        [0.5, 1],
        [0.99, 1],
        [1.01, 1],
        [10.49, 10],
        [10.5, 10],
        [10.99, 10],
        [11.01, 11],
      ] as const) {
        const duration = frames / fps;
        const item: MediaItem = {
          id: `v-${fps}-${frames}`,
          name: "fractional.mp4",
          type: "video",
          duration,
          hasAudio: false,
        };
        expect(mediaDurationFrames(item, fps)).toBe(expected);
        expect(momentDurationFrames({ startSec: 5, endSec: 5 + duration }, fps)).toBe(expected);
        const plan = buildMediaInsertPlan(
          {
            ...EMPTY,
            fps,
            tracks: [
              {
                id: "video-track",
                type: "video",
                muted: false,
                hidden: false,
                syncLocked: true,
                clips: [],
              },
            ],
          },
          item,
          0,
          0,
        );
        expect(plan?.entries[0].durationFrames).toBe(expected);
      }
    }

    expect(momentDurationFrames({ startSec: 2, endSec: 1 }, 30)).toBe(1);
    expect(momentDurationFrames({ startSec: Number.NaN, endSec: 1 }, 30)).toBe(1);
    expect(momentDurationFrames({ startSec: 0, endSec: Number.POSITIVE_INFINITY }, 30)).toBe(1);

    srv.reset();
    setMirror({ ...EMPTY, fps: 29.97 }, 0, 1);
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, selectedClipIds: new Set() });
    await addTextClip();
    expect(useProjectStore.getState().timeline.tracks[0].clips[0].durationFrames).toBe(89);
  });
});

describe("addMomentToTimelineAt (trimmed source-range drop from a search hit)", () => {
  beforeEach(() => {
    srv.reset();
    setMirror(EMPTY, 0, 1);
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, selectedClipIds: new Set() });
  });

  /** The first video clip's [trimStart, duration, trimEnd] after a placement. */
  function firstVideoTrim(): [number, number, number] {
    const tl = useProjectStore.getState().timeline;
    const track = tl.tracks.find((t) => t.type === "video");
    const c = track?.clips[0];
    return c ? [c.trimStartFrame, c.durationFrames, c.trimEndFrame] : [-1, -1, -1];
  }

  it("places only the source range as a trimmed clip", async () => {
    // 10s @ 30fps = 300 source frames. Range [3s,6s] → trimStart 90, duration 90,
    // trimEnd 300-90-90 = 120. Lands at timeline frame 0.
    const item: MediaItem = { id: "v", name: "v", type: "video", duration: 10, hasAudio: false };
    await addMomentToTimelineAt(item, 0, null, { startSec: 3, endSec: 6 });
    expect(visualClipStarts()).toEqual([0]);
    expect(firstVideoTrim()).toEqual([90, 90, 120]);
  });

  it("clamps a range that runs past the source end", async () => {
    // 5s = 150 frames. Range [4s, 9s] would want duration 150 but only 30 frames
    // of source remain after trimStart 120 → duration clamps to 30, trimEnd 0.
    const item: MediaItem = { id: "v", name: "v", type: "video", duration: 5, hasAudio: false };
    await addMomentToTimelineAt(item, 0, null, { startSec: 4, endSec: 9 });
    expect(firstVideoTrim()).toEqual([120, 30, 0]);
  });

  it("falls back to the whole asset for a still image (no range)", async () => {
    // Images have no meaningful sub-range → placed full (default 5s = 150 frames),
    // untrimmed.
    const item: MediaItem = { id: "i", name: "i", type: "image", duration: 0, hasAudio: false };
    await addMomentToTimelineAt(item, 0, null, { startSec: 0, endSec: 0 });
    expect(firstVideoTrim()).toEqual([0, 150, 0]);
  });

  it("lands the trimmed clip at the drop start frame", async () => {
    const item: MediaItem = { id: "v", name: "v", type: "video", duration: 10, hasAudio: false };
    await addMomentToTimelineAt(item, 45, null, { startSec: 1, endSec: 2 });
    expect(visualClipStarts()).toEqual([45]);
    // 1s..2s → trimStart 30, duration 30, trimEnd 300-30-30 = 240.
    expect(firstVideoTrim()).toEqual([30, 30, 240]);
  });
});

// #194 regression: addTextClip (Toolbar "T") used to reuse the first existing
// visual track (or insert a new one only when none existed at all). A
// pre-existing top video track's clip would get overwritten to make room for
// the new text clip. It must always land on a brand-new track instead.
describe("addTextClip (Toolbar 'T' button)", () => {
  beforeEach(() => {
    srv.reset();
    setMirror(EMPTY, 0, 1);
    useEditorUiStore.setState({ activeFrame: 0, currentFrame: 0, selectedClipIds: new Set() });
  });

  it("creates a fresh track and leaves an existing visual track's clip untouched", async () => {
    // Seed a pre-existing video track with unrelated content, exactly the
    // #194 regression scenario.
    await addMediaToTimeline(video("existing"));
    const before = useProjectStore.getState().timeline;
    expect(before.tracks.length).toBe(1);
    expect(before.tracks[0].clips.length).toBe(1);

    await addTextClip();

    const after = useProjectStore.getState().timeline;
    // A new track was inserted at index 0 — two tracks total now.
    expect(after.tracks.length).toBe(2);
    // The new text clip is on the new top track.
    expect(after.tracks[0].clips.length).toBe(1);
    expect(after.tracks[0].clips[0].mediaType).toBe("text");
    // The pre-existing track (now at index 1) and its clip are unchanged.
    expect(after.tracks[1].clips.length).toBe(1);
    expect(after.tracks[1].clips[0].mediaRef).toBe("existing");
    expect(after.tracks[1].clips[0].startFrame).toBe(0);
  });

  it("creates a track on an empty timeline too", async () => {
    await addTextClip();
    const tl = useProjectStore.getState().timeline;
    expect(tl.tracks.length).toBe(1);
    expect(tl.tracks[0].clips.length).toBe(1);
    expect(tl.tracks[0].clips[0].mediaType).toBe("text");
  });

  it("selects the newly created text clip", async () => {
    await addTextClip();
    const tl = useProjectStore.getState().timeline;
    const newClipId = tl.tracks[0].clips[0].id;
    expect(useEditorUiStore.getState().selectedClipIds.has(newClipId)).toBe(true);
  });

  it("adding two text clips creates two separate top tracks, not one shared track", async () => {
    // Each call is independent — mirrors upstream addTextClip's unconditional
    // insertTrack(at: 0), not a "reuse the track I made last time" cache.
    await addTextClip();
    await addTextClip();
    const tl = useProjectStore.getState().timeline;
    expect(tl.tracks.length).toBe(2);
    expect(tl.tracks[0].clips.length).toBe(1);
    expect(tl.tracks[1].clips.length).toBe(1);
  });
});
