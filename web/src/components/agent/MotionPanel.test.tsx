import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  addMotion: vi.fn(),
  cancelMotion: vi.fn(),
  motionCapability: vi.fn(),
  onMotionProgress: vi.fn(),
}));
const sync = vi.hoisted(() => ({ forceRefresh: vi.fn() }));

vi.mock("../../lib/api", () => ({
  ...api,
  isTauri: true,
}));
vi.mock("../../store/sync", () => sync);

import { useI18nStore } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { MotionPanel } from "./MotionPanel";

describe("MotionPanel", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    vi.clearAllMocks();
    useI18nStore.setState({ locale: "en" });
    useProjectStore.setState({
      projectPath: "/tmp/demo.opentake",
      timeline: { fps: 24, width: 1920, height: 1080, settingsConfigured: true, tracks: [] },
    });
    useEditorUiStore.setState({ activeFrame: 48, selectedClipIds: new Set() });
    api.motionCapability.mockResolvedValue(true);
    api.onMotionProgress.mockResolvedValue(() => {});
    api.addMotion.mockResolvedValue({
      clipId: "motion-clip",
      assetId: "motion-asset",
      contentHash: "hash",
      actionName: "Add Motion Graphic",
      output: {
        renderer: "motion-canvas",
        rendererVersion: "3.17.2",
        outputFile: "output.mp4",
        fps: 24,
        width: 1920,
        height: 1080,
        durationFrames: 72,
        durationSeconds: 3,
        contentHash: "hash",
      },
    });
    sync.forceRefresh.mockResolvedValue(undefined);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    await act(async () => root.render(<MotionPanel />));
  });

  afterEach(async () => {
    if (root) await act(async () => root.unmount());
    container?.remove();
  });

  it("submits a frame-exact template at the playhead and selects the committed clip", async () => {
    const add = Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.includes("Add at playhead"),
    );
    expect(add).toBeDefined();
    await act(async () => add!.click());

    expect(api.addMotion).toHaveBeenCalledWith(
      expect.objectContaining({
        templateId: "title-card",
        startFrame: 48,
        durationFrames: 72,
      }),
    );
    expect(sync.forceRefresh).toHaveBeenCalledOnce();
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set(["motion-clip"]));
  });
});
// @vitest-environment happy-dom
