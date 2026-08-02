// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useI18nStore } from "../../i18n";
import type { MediaItem, ScriptToVideoResult, ScriptToVideoSegmentInput } from "../../lib/types";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { ScriptToVideoTab, type ScriptToVideoDependencies } from "./ScriptToVideoTab";

function media(id: string, type: "image" | "audio", duration = 0): MediaItem {
  return { id, name: id, type, duration, hasAudio: type === "audio", favorite: false };
}

function result(segments: ScriptToVideoSegmentInput[], applied = false): ScriptToVideoResult {
  let cursor = 0;
  return {
    result: {
      projectEpoch: 4,
      version: applied ? 12 : 11,
      planId: "script-plan-test",
      planHash: "a".repeat(64),
      planner: "opentake-script-assembly",
      plannerVersion: 1,
      startFrame: 0,
      endFrame: segments.reduce((sum, segment) => sum + segment.durationFrames, 0),
      segments: segments.map((segment) => {
        const startFrame = cursor;
        cursor += segment.durationFrames;
        return { ...segment, startFrame };
      }),
      applied,
    },
    actionName: applied ? "Build Script Video" : "Plan Script Video",
  };
}

function setText(element: HTMLTextAreaElement, value: string) {
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set?.call(element, value);
  element.dispatchEvent(new Event("input", { bubbles: true }));
}

describe("ScriptToVideoTab", () => {
  let container: HTMLDivElement;
  let root: Root;
  let run: ReturnType<typeof vi.fn>;
  let cancel: ReturnType<typeof vi.fn>;
  let undo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    useI18nStore.setState({ locale: "en" });
    useMediaStore.setState({ items: [media("visual-1", "image"), media("visual-2", "image"), media("visual-3", "image"), media("voice", "audio", 1)], folders: [], importing: false, error: null });
    useProjectStore.setState({ timeline: { fps: 30, width: 1920, height: 1080, settingsConfigured: true, tracks: [] } });
    run = vi.fn();
    cancel = vi.fn().mockResolvedValue(true);
    undo = vi.fn().mockResolvedValue(undefined);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it("persists, reviews, edits, retries, applies, and undoes a three-segment plan", async () => {
    run.mockImplementation((segments: ScriptToVideoSegmentInput[], apply: boolean) => Promise.resolve(result(segments, apply)));
    const dependencies: ScriptToVideoDependencies = { run, cancel, undo };
    await act(async () => root.render(<ScriptToVideoTab dependencies={dependencies} />));
    const textareas = Array.from(container.querySelectorAll("textarea"));
    await act(async () => textareas.forEach((textarea, index) => setText(textarea, `Scene ${index + 1}`)));
    const selects = Array.from(container.querySelectorAll("select"));
    await act(async () => {
      for (let index = 0; index < 3; index += 1) {
        const visual = selects[index * 2];
        visual.value = `visual-${index + 1}`;
        visual.dispatchEvent(new Event("change", { bubbles: true }));
        const narration = selects[index * 2 + 1];
        narration.value = "voice";
        narration.dispatchEvent(new Event("change", { bubbles: true }));
      }
    });
    const button = (label: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(label))!;
    await act(async () => button("Save & Review Plan").click());
    expect(run).toHaveBeenLastCalledWith(expect.arrayContaining([expect.objectContaining({ script: "Scene 1", mediaRef: "visual-1", narrationMediaRef: "voice", durationFrames: 30 })]), false);
    expect(container.textContent).toContain("Plan persisted: 3 segments");

    await act(async () => setText(textareas[1], "Edited second scene"));
    expect(container.textContent).not.toContain("Plan persisted: 3 segments");
    await act(async () => button("Save & Review Plan").click());
    expect(run).toHaveBeenCalledTimes(2);
    await act(async () => button("Start Assembly").click());
    expect(run).toHaveBeenLastCalledWith(expect.arrayContaining([expect.objectContaining({ script: "Edited second scene" })]), true);
    expect(container.textContent).toContain("one undo step");
    await act(async () => button("Undo Assembly").click());
    expect(undo).toHaveBeenCalledOnce();
  });

  it("cancels pending planning and ignores the stale result", async () => {
    let resolve: ((value: ScriptToVideoResult) => void) | undefined;
    run.mockImplementation((segments: ScriptToVideoSegmentInput[]) => new Promise<ScriptToVideoResult>((done) => { resolve = () => done(result(segments)); }));
    const dependencies: ScriptToVideoDependencies = { run, cancel, undo };
    await act(async () => root.render(<ScriptToVideoTab dependencies={dependencies} />));
    const textareas = Array.from(container.querySelectorAll("textarea"));
    await act(async () => textareas.forEach((textarea, index) => setText(textarea, `Scene ${index + 1}`)));
    const button = (label: string) => Array.from(container.querySelectorAll("button")).find((candidate) => candidate.textContent?.includes(label))!;
    await act(async () => button("Save & Review Plan").click());
    await act(async () => button("Cancel").click());
    expect(cancel).toHaveBeenCalledOnce();
    await act(async () => resolve?.(result([])));
    expect(container.textContent).not.toContain("Plan persisted");
  });
});
