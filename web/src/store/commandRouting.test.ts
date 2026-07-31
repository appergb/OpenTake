import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EditRequest, TextStyle, Transform } from "../lib/types";

const ipc = vi.hoisted(() => ({ calls: [] as EditRequest[] }));

vi.mock("../lib/api", () => ({
  isTauri: true,
  editApply: async (command: EditRequest) => {
    ipc.calls.push(structuredClone(command));
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
  removeClips,
  removeKeyframe,
  removeTracks,
  renameFolder,
  renameMedia,
  resetTransform,
  rippleDeleteClips,
  rippleDeleteRanges,
  setChromaKey,
  setClipProperties,
  setColorGrade,
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
  });

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
    await route({ type: "setChromaKey", clipIds: ["clip-a"], chromaKey: null }, () => setChromaKey(["clip-a"], null));
    await route(
      { type: "setMasks", clipIds: ["clip-a"], masks: [{ shape: { kind: "circle", center: { x: 0.5, y: 0.5 }, radius: { x: 0.25, y: 0.25 } }, feather: 0, invert: false }] },
      () => setMasks(["clip-a"], [{ shape: { kind: "circle", center: { x: 0.5, y: 0.5 }, radius: { x: 0.25, y: 0.25 } }, feather: 0, invert: false }]),
    );
    await route(
      { type: "setEffects", clipIds: ["clip-a"], effects: [{ name: "blur", params: { radius: 2 }, enabled: true }] },
      () => setEffects(["clip-a"], [{ name: "blur", params: { radius: 2 }, enabled: true }]),
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
      .filter((path) => !path.endsWith("/lib/fallback.ts") && !path.endsWith("/store/projectStore.ts"))
      .filter((path) => forbidden.test(readFileSync(path, "utf8")));
    expect(illegal).toEqual([]);
  });
});
