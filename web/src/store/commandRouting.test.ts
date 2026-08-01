import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Clip, EditRequest, TextStyle, Timeline, Transform } from "../lib/types";

const ipc = vi.hoisted(() => ({
  calls: [] as EditRequest[],
  failure: null as Error | null,
}));

vi.mock("../lib/api", () => ({
  isTauri: true,
  editApply: async (command: EditRequest) => {
    ipc.calls.push(structuredClone(command));
    if (ipc.failure) {
      const failure = ipc.failure;
      ipc.failure = null;
      throw failure;
    }
    return {
      changed: false,
      actionName: command.type,
      affectedClipIds: [],
      timelineVersion: 0,
      summary: "",
    };
  },
}));

import {
  EDIT_GESTURE_COMMAND_MATRIX,
  EDIT_GESTURE_COMMAND_MATRIX_IS_EXHAUSTIVE,
  addCaptions,
  addClips,
  addTexts,
  addTextsAutoTrack,
  adjustStabilization,
  applyStabilization,
  createNestedSequence,
  copyClips,
  currentTimelineEndFrame,
  editNestedSequence,
  renameNestedSequence,
  dissolveNestedSequence,
  createFolder,
  deleteFolder,
  deleteMedia,
  duplicateClips,
  freezeFrame,
  insertClips,
  insertTrack,
  linkClips,
  moveClips,
  moveKeyframe,
  moveToFolder,
  pasteClipsAtPlayhead,
  removeClips,
  removeKeyframe,
  removeTracks,
  renameFolder,
  renameMedia,
  resetTransform,
  resetStabilization,
  rippleDeleteClips,
  rippleDeleteRanges,
  setChromaKey,
  setClipProperties,
  setColorGrade,
  setLut,
  setLoudnessNormalization,
  setAudioDenoise,
  setEffects,
  setKeyframeInterpolation,
  setKeyframes,
  setMasks,
  setTimelineSettings,
  setTrackProps,
  setTransition,
  splitClip,
  stampKeyframe,
  swapClips,
  swapMedia,
  swapTracks,
  trimClips,
  unlinkClips,
  upsertKeyframe,
} from "./editActions";
import { useClipboardStore } from "./clipboardStore";
import { useProjectStore } from "./projectStore";
import { createEditorUiStore, useEditorUiStore } from "./uiStore";

const transform: Transform = {
  centerX: 0.5,
  centerY: 0.5,
  width: 1,
  height: 1,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
};

const textStyle: TextStyle = {
  fontName: "Helvetica-Bold",
  fontSize: 48,
  fontScale: 1,
  color: { r: 1, g: 1, b: 1, a: 1 },
  alignment: "center",
  shadow: {
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 1 },
    offsetX: 0,
    offsetY: 0,
    blur: 0,
  },
  background: { enabled: false, color: { r: 0, g: 0, b: 0, a: 0 } },
  border: { enabled: false, color: { r: 0, g: 0, b: 0, a: 0 } },
};

const clipEntry = {
  mediaRef: "media-a",
  mediaType: "video" as const,
  sourceClipType: "video" as const,
  trackIndex: 0,
  startFrame: 10,
  durationFrames: 20,
};

function sourceFiles(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return [".ts", ".tsx"].includes(extname(path)) && !path.includes(".test.") ? [path] : [];
  });
}

describe("edit gesture command routing", () => {
  beforeEach(() => {
    ipc.calls.length = 0;
    ipc.failure = null;
    useEditorUiStore.getState().exitNestedSequence();
    useClipboardStore.getState().clear();
  });

  afterEach(() => vi.unstubAllGlobals());

  it("every_edit_action_emits_exact_edit_request", async () => {
    expect(EDIT_GESTURE_COMMAND_MATRIX_IS_EXHAUSTIVE).toBe(true);
    expect(new Set(EDIT_GESTURE_COMMAND_MATRIX.map((row) => row.requestType)).size).toBe(
      EDIT_GESTURE_COMMAND_MATRIX.length,
    );

    const route = async (expected: EditRequest, invoke: () => Promise<unknown>) => {
      const before = ipc.calls.length;
      await invoke();
      expect(ipc.calls.slice(before)).toEqual([expected]);
    };

    useEditorUiStore.getState().selectClips(new Set(["clip-a"]));
    await route(
      { type: "createNestedSequence", name: "Scene", clipIds: ["clip-a"] },
      () => createNestedSequence("Scene"),
    );
    await route(
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: { type: "removeClips", clipIds: ["child-a"] },
      },
      () => editNestedSequence("sequence-a", { type: "removeClips", clipIds: ["child-a"] }),
    );
    await route(
      { type: "renameNestedSequence", sequenceId: "sequence-a", name: "Edited" },
      () => renameNestedSequence("sequence-a", "Edited"),
    );
    await route(
      { type: "dissolveNestedSequence", clipId: "compound-a" },
      () => dissolveNestedSequence("compound-a"),
    );
    await route({ type: "addClips", entries: [clipEntry] }, () => addClips([clipEntry]));
    await route(
      { type: "insertClips", trackIndex: 0, atFrame: 5, entries: [clipEntry] },
      () => insertClips(0, 5, [clipEntry]),
    );
    await route(
      { type: "moveClips", moves: [{ clipId: "clip-a", toTrack: 1, toFrame: 30 }] },
      () => moveClips([{ clipId: "clip-a", toTrack: 1, toFrame: 30 }]),
    );
    await route(
      { type: "duplicateClips", clipIds: ["clip-a"], offsetFrames: 3, targetTrackIndexes: [1] },
      () => duplicateClips(["clip-a"], 3, [1]),
    );
    await route({ type: "removeClips", clipIds: ["clip-a"] }, () => removeClips(["clip-a"]));
    await route({ type: "splitClip", clipId: "clip-a", atFrame: 15 }, () => splitClip("clip-a", 15));
    await route(
      { type: "freezeFrame", clipId: "clip-a", atFrame: 15, durationFrames: 30 },
      () => freezeFrame("clip-a", 15, 30),
    );
    await route(
      { type: "trimClips", edits: [{ clipId: "clip-a", trimStartFrame: 2, trimEndFrame: 4 }] },
      () => trimClips([{ clipId: "clip-a", trimStartFrame: 2, trimEndFrame: 4 }]),
    );
    await route(
      { type: "setClipProperties", clipIds: ["clip-a"], properties: { opacity: 0.5 } },
      () => setClipProperties(["clip-a"], { opacity: 0.5 }),
    );
    await route(
      { type: "setKeyframes", clipId: "clip-a", property: "opacity", payload: { kind: "scalar", keyframes: [] } },
      () => setKeyframes("clip-a", "opacity", { kind: "scalar", keyframes: [] }),
    );
    await route(
      { type: "stampKeyframe", clipId: "clip-a", property: "opacity", frame: 12 },
      () => stampKeyframe("clip-a", "opacity", 12),
    );
    await route(
      { type: "upsertKeyframe", clipId: "clip-a", property: "opacity", frame: 12, value: { kind: "scalar", value: 0.75 } },
      () => upsertKeyframe("clip-a", "opacity", 12, { kind: "scalar", value: 0.75 }),
    );
    await route(
      { type: "removeKeyframe", clipId: "clip-a", property: "opacity", frame: 12 },
      () => removeKeyframe("clip-a", "opacity", 12),
    );
    await route(
      { type: "moveKeyframe", clipId: "clip-a", property: "opacity", fromFrame: 12, toFrame: 14 },
      () => moveKeyframe("clip-a", "opacity", 12, 14),
    );
    await route(
      { type: "setKeyframeInterpolation", clipId: "clip-a", property: "opacity", frame: 14, interpolation: "hold" },
      () => setKeyframeInterpolation("clip-a", "opacity", 14, "hold"),
    );
    await route({ type: "setColorGrade", clipIds: ["clip-a"], grade: null }, () => setColorGrade(["clip-a"], null));
    await route({ type: "setLut", clipIds: ["clip-a"], lut: null }, () => setLut(["clip-a"], null));
    await route({ type: "setChromaKey", clipIds: ["clip-a"], chromaKey: null }, () => setChromaKey(["clip-a"], null));
    await route(
      { type: "setMasks", clipIds: ["clip-a"], masks: [{ shape: { kind: "circle", center: { x: 0.5, y: 0.5 }, radius: { x: 0.25, y: 0.25 } }, feather: 0, invert: false }] },
      () => setMasks(["clip-a"], [{ shape: { kind: "circle", center: { x: 0.5, y: 0.5 }, radius: { x: 0.25, y: 0.25 } }, feather: 0, invert: false }]),
    );
    await route(
      { type: "setEffects", clipIds: ["clip-a"], effects: [{ name: "grayscale", params: { amount: 0.4 }, enabled: true }] },
      () => setEffects(["clip-a"], [{ name: "grayscale", params: { amount: 0.4 }, enabled: true }]),
    );
    const loudness = {
      targetLufs: -16,
      truePeakCeilingDbtp: -1,
      inputIntegratedLufs: -23,
      inputTruePeakDbtp: -8,
      gainDb: 7,
      outputIntegratedLufs: -16,
      outputTruePeakDbtp: -1,
    };
    await route(
      { type: "setLoudnessNormalization", clipId: "clip-a", normalization: loudness },
      () => setLoudnessNormalization("clip-a", loudness),
    );
    const denoise = { mode: "voice" as const, strength: 0.85, previewEnabled: true };
    await route(
      { type: "setAudioDenoise", clipId: "clip-a", denoise },
      () => setAudioDenoise("clip-a", denoise),
    );
    const stabilization = {
      model: "opentake.motion-smoothing",
      modelVersion: 1,
      sourceIdentity: "media-a",
      strength: 1,
      cropMargin: 0,
      keyframes: [
        { frame: 0, translationX: 0, translationY: 0, rotationDegrees: 0 },
        { frame: 10, translationX: 0.01, translationY: 0, rotationDegrees: 0 },
      ],
    };
    await route(
      { type: "applyStabilization", clipId: "clip-a", solution: stabilization },
      () => applyStabilization("clip-a", stabilization),
    );
    await route(
      { type: "adjustStabilization", clipId: "clip-a", strength: 0.75, cropMargin: 0.02 },
      () => adjustStabilization("clip-a", { strength: 0.75, cropMargin: 0.02 }),
    );
    await route(
      { type: "resetStabilization", clipId: "clip-a" },
      () => resetStabilization("clip-a"),
    );
    await route(
      { type: "setTransition", fromClipId: "clip-a", toClipId: "clip-b", kind: "crossDissolve", durationFrames: 8 },
      () => setTransition("clip-a", "clip-b", "crossDissolve", 8),
    );
    await route(
      { type: "rippleDeleteRanges", trackIndex: 0, ranges: [{ start: 4, end: 9 }] },
      () => rippleDeleteRanges(0, [{ start: 4, end: 9 }]),
    );
    await route({ type: "rippleDeleteClips", clipIds: ["clip-a"] }, () => rippleDeleteClips(["clip-a"]));
    await route(
      { type: "addTexts", entries: [{ trackIndex: 0, startFrame: 0, durationFrames: 30, content: "Title", textStyle, transform }] },
      () => addTexts([{ trackIndex: 0, startFrame: 0, durationFrames: 30, content: "Title", textStyle, transform }]),
    );
    await route(
      { type: "addTextsAutoTrack", entries: [{ startFrame: 0, durationFrames: 30, content: "Title", textStyle, transform }] },
      () => addTextsAutoTrack([{ startFrame: 0, durationFrames: 30, content: "Title", textStyle, transform }]),
    );
    await route(
      { type: "addCaptions", entries: [{ startFrame: 0, durationFrames: 10, content: "Caption", textStyle, transform, captionGroupId: "captions-a" }] },
      () => addCaptions([{ startFrame: 0, durationFrames: 10, content: "Caption", textStyle, transform, captionGroupId: "captions-a" }]),
    );
    await route({ type: "link", clipIds: ["clip-a", "clip-b"] }, () => linkClips(["clip-a", "clip-b"]));
    await route({ type: "unlink", clipIds: ["clip-a"] }, () => unlinkClips(["clip-a"]));
    await route({ type: "removeTracks", trackIndexes: [1, 3] }, () => removeTracks([1, 3]));
    await route({ type: "swapTracks", a: 0, b: 1 }, () => swapTracks(0, 1));
    await route({ type: "swapClips", clipA: "clip-a", clipB: "clip-b" }, () => swapClips("clip-a", "clip-b"));
    await route({ type: "insertTrack", kind: "video", at: 1 }, () => insertTrack("video", 1));
    await route(
      { type: "setTrackProps", trackIndex: 0, muted: true, syncLocked: false },
      () => setTrackProps(0, { muted: true, syncLocked: false }),
    );
    await route({ type: "createFolder", name: "B-roll", parentFolderId: "root" }, () => createFolder("B-roll", "root"));
    await route({ type: "moveToFolder", assetIds: ["media-a"], folderId: "folder-a" }, () => moveToFolder(["media-a"], "folder-a"));
    await route({ type: "renameMedia", entries: [{ id: "media-a", name: "Intro" }] }, () => renameMedia([{ id: "media-a", name: "Intro" }]));
    await route({ type: "renameFolder", entries: [{ id: "folder-a", name: "Selects" }] }, () => renameFolder([{ id: "folder-a", name: "Selects" }]));
    await route({ type: "deleteMedia", assetIds: ["media-a"] }, () => deleteMedia(["media-a"]));
    await route({ type: "deleteFolder", folderIds: ["folder-a"] }, () => deleteFolder(["folder-a"]));
    await route({ type: "swapMedia", clipId: "clip-a", mediaRef: "media-b" }, () => swapMedia("clip-a", "media-b"));
    await route({ type: "resetTransform", clipIds: ["clip-a"] }, () => resetTransform(["clip-a"]));
    await route({ type: "setTimelineSettings", fps: 24, width: 3840, height: 2160 }, () => setTimelineSettings(24, 3840, 2160));

    expect(ipc.calls.map((request) => request.type)).toEqual(
      EDIT_GESTURE_COMMAND_MATRIX.map((row) => row.requestType),
    );

    const beforeNoOps = ipc.calls.length;
    await addClips([]);
    await moveClips([]);
    await removeTracks([]);
    await swapTracks(2, 2);
    await swapClips("same", "same");
    expect(ipc.calls).toHaveLength(beforeNoOps);

    const src = fileURLToPath(new URL("..", import.meta.url));
    const forbidden = /(?:\.tracks|\.clips)\.(?:push|splice)\s*\(|\.(?:startFrame|durationFrames|trimStartFrame|trimEndFrame)\s*=(?!=)|useProjectStore\.setState\s*\(/;
    const illegal = sourceFiles(src)
      .filter((path) => {
        const portablePath = path.replaceAll("\\", "/");
        return !portablePath.endsWith("/lib/fallback.ts") && !portablePath.endsWith("/store/projectStore.ts");
      })
      .filter((path) => forbidden.test(readFileSync(path, "utf8")));
    expect(illegal).toEqual([]);
  });

  it("routes child edits to the active sequence but keeps global commands at root", async () => {
    useEditorUiStore.getState().enterNestedSequence("sequence-a");

    await addClips([clipEntry]);
    await deleteMedia(["media-a"]);
    await createFolder("Selects");
    await setTimelineSettings(24, 1280, 720);

    expect(ipc.calls).toEqual([
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: { type: "addClips", entries: [clipEntry] },
      },
      { type: "deleteMedia", assetIds: ["media-a"] },
      { type: "createFolder", name: "Selects" },
      { type: "setTimelineSettings", fps: 24, width: 1280, height: 720 },
    ]);
  });

  it("project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events", () => {
    const store = useProjectStore.getState();
    expect("setMirror" in store).toBe(false);

    const newest = {
      timeline: {
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: true,
        tracks: [],
      },
      projectEpoch: 4,
      version: 12,
      projectPath: "/new.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    };
    store.replaceProjectSnapshot(newest);
    store.replaceProjectSnapshot({ ...newest, version: 11, projectPath: "/stale.opentake" });

    const committed = useProjectStore.getState();
    expect(committed.timelineVersion).toBe(12);
    expect(committed.projectPath).toBe("/new.opentake");
    expect(Object.isFrozen(committed.timeline)).toBe(true);
    newest.timeline.fps = 60;
    expect(useProjectStore.getState().timeline.fps).toBe(30);
    expect(() => {
      useProjectStore.getState().timeline.fps = 120;
    }).toThrow();
  });

  it("nested timeline gestures are wrapped with the active sequence authority", async () => {
    useEditorUiStore.getState().enterNestedSequence("sequence-a");

    await moveClips([{ clipId: "child-a", toTrack: 0, toFrame: 12 }]);

    expect(ipc.calls).toEqual([
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: {
          type: "moveClips",
          moves: [{ clipId: "child-a", toTrack: 0, toFrame: 12 }],
        },
      },
    ]);
  });

  it("nested clipboard reads, bounds, and paste target the active sequence", async () => {
    const childClip: Clip = {
      id: "child-a",
      mediaRef: "media-a",
      mediaType: "video",
      sourceClipType: "video",
      startFrame: 15,
      durationFrames: 110,
      trimStartFrame: 10,
      trimEndFrame: 0,
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
    const childTimeline: Timeline = {
      fps: 30,
      width: 1920,
      height: 1080,
      settingsConfigured: true,
      tracks: [
        {
          id: "child-video",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [childClip],
        },
      ],
    };
    useProjectStore.getState().replaceProjectSnapshot({
      timeline: {
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: true,
        tracks: [],
        nestedSequences: [{ id: "sequence-a", name: "Scene", timeline: childTimeline }],
      },
      projectEpoch: 5,
      version: 2,
      projectPath: "/nested.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    });
    useEditorUiStore.getState().enterNestedSequence("sequence-a");
    useEditorUiStore.setState({
      selectedClipIds: new Set(["child-a"]),
      activeFrame: 150,
      currentFrame: 150,
    });

    expect(currentTimelineEndFrame()).toBe(125);
    copyClips();
    await pasteClipsAtPlayhead();

    expect(ipc.calls).toEqual([
      {
        type: "editNestedSequence",
        sequenceId: "sequence-a",
        command: {
          type: "addClips",
          entries: [
            {
              mediaRef: "media-a",
              mediaType: "video",
              sourceClipType: "video",
              trackIndex: 0,
              startFrame: 150,
              durationFrames: 110,
              trimStartFrame: 10,
              trimEndFrame: 0,
              hasAudio: true,
              addLinkedAudio: false,
              transform,
            },
          ],
        },
      },
    ]);
  });

  it("rust_authority_and_ui_persistence_are_independently_owned", async () => {
    const values = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
      clear: () => values.clear(),
    });

    const ui = createEditorUiStore();
    ui.getState().setLayoutPreset("vertical");
    ui.getState().toggleAgentPanel();
    ui.getState().setZoomScale(8);
    ui.getState().setCurrentFrame(240);
    ui.getState().selectClips(new Set(["project-a-clip"]));
    ui.getState().setPreviewMedia("project-a-media");

    expect([...values.keys()].sort()).toEqual([
      "opentake.ui.v1.agentPanelVisible",
      "opentake.ui.v1.layoutPreset",
      "opentake.ui.v1.zoomScale",
    ]);
    expect([...values.values()].join(" ")).not.toContain("project-a");

    const restarted = createEditorUiStore().getState();
    expect(restarted).toMatchObject({
      layoutPreset: "vertical",
      agentPanelVisible: true,
      zoomScale: 8,
      currentFrame: 0,
      previewMediaId: null,
    });
    expect(restarted.selectedClipIds).toEqual(new Set());

    const project = useProjectStore.getState();
    const snapshot = {
      timeline: {
        fps: 30,
        width: 1920,
        height: 1080,
        settingsConfigured: true,
        tracks: [],
      },
      projectEpoch: 50,
      version: 2,
      projectPath: "/project-a.opentake",
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
    };
    project.replaceProjectSnapshot(snapshot);
    project.replaceProjectSnapshot({ ...snapshot, version: 1, projectPath: "/stale.opentake" });
    expect(useProjectStore.getState()).toMatchObject({
      projectEpoch: 50,
      timelineVersion: 2,
      projectPath: "/project-a.opentake",
    });

    const authoritativeTimeline = useProjectStore.getState().timeline;
    await Promise.all([removeClips(["clip-a"]), removeClips(["clip-b"])]);
    expect(ipc.calls.slice(-2)).toEqual([
      { type: "removeClips", clipIds: ["clip-a"] },
      { type: "removeClips", clipIds: ["clip-b"] },
    ]);
    expect(useProjectStore.getState()).toMatchObject({
      projectEpoch: 50,
      timelineVersion: 2,
      projectPath: "/project-a.opentake",
      timeline: authoritativeTimeline,
    });

    ipc.failure = new Error("native edit failed");
    await expect(removeClips(["failed-clip"])).rejects.toThrow("native edit failed");
    expect(useProjectStore.getState()).toMatchObject({
      projectEpoch: 50,
      timelineVersion: 2,
      projectPath: "/project-a.opentake",
      timeline: authoritativeTimeline,
    });

    ui.getState().resetProjectRuntimeState();
    expect(ui.getState()).toMatchObject({
      layoutPreset: "vertical",
      agentPanelVisible: true,
      zoomScale: 8,
      currentFrame: 0,
      previewMediaId: null,
    });

    const uiSource = readFileSync(new URL("./uiStore.ts", import.meta.url), "utf8");
    expect(uiSource).not.toMatch(/localStorage[^\n]*(?:timeline|clip|media|credential|secret)/i);
  });
});
