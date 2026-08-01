// @vitest-environment happy-dom

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PanelShell } from "./components/ui/PanelShell";
import { accessibleClipRects } from "./components/timeline/TimelineContainer";
import type { Timeline } from "./lib/types";
import { useEditorUiStore } from "./store/uiStore";

vi.mock("./i18n", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./i18n")>();
  return { ...actual, useT: () => (key: string) => key };
});

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const webRoot = process.cwd();
const globalCss = readFileSync(resolve(webRoot, "src/styles/global.css"), "utf8");
const homeTests = readFileSync(
  resolve(webRoot, "src/components/home/HomeView.test.tsx"),
  "utf8",
);
const projectActionTests = readFileSync(
  resolve(webRoot, "src/store/projectActions.test.ts"),
  "utf8",
);
const sampleBackend = readFileSync(
  resolve(webRoot, "../src-tauri/src/samples.rs"),
  "utf8",
);

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  localStorage.clear();
  useEditorUiStore.setState({ focusedPanel: "timeline" });
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

it("sample_projects_accessibility_visual_and_interaction_gate", async () => {
  await act(async () => {
    root.render(
      <PanelShell panel="preview">
        <span>Preview</span>
      </PanelShell>,
    );
  });

  const panel = container.querySelector<HTMLElement>('[data-editor-panel="preview"]');
  expect(panel).not.toBeNull();
  expect(panel?.tabIndex).toBe(0);
  await act(async () => panel?.focus());
  expect(useEditorUiStore.getState().focusedPanel).toBe("preview");
  expect(panel?.getAttribute("role")).toBe("region");
  expect(panel?.getAttribute("aria-label")).toBe("layout.panel.preview");

  const timeline: Timeline = {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [{
      id: "video-track",
      type: "video",
      muted: false,
      hidden: false,
      syncLocked: true,
      clips: [{
        id: "intro",
        mediaRef: "intro-media",
        mediaType: "video",
        sourceClipType: "video",
        startFrame: 0,
        durationFrames: 1,
        trimStartFrame: 0,
        trimEndFrame: 0,
        speed: 1,
        volume: 1,
        fadeInFrames: 0,
        fadeOutFrames: 0,
        fadeInInterpolation: "smooth",
        fadeOutInterpolation: "smooth",
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
      }],
    }],
  };
  const [clipRect] = accessibleClipRects(timeline, 1, {}, 0, 0, 600, 240);
  expect(clipRect).toMatchObject({ clipId: "intro", width: 24, height: 46 });
  expect(clipRect.label).toContain("V1");

  expect(globalCss).toContain(":focus-visible");
  expect(globalCss).toContain("@media (prefers-reduced-motion: reduce)");
  expect(globalCss).toContain("@media (forced-colors: active)");

  // The release umbrella is backed by the existing executable sample owners:
  // Home routing, project lifecycle success/failure, and atomic offline bundle
  // materialization. This test keeps those owners discoverable from one gate.
  expect(homeTests).toContain("new_open_sample_register_only_after_success_and_route_tutorial");
  expect(projectActionTests).toContain("opens a completed tutorial sample");
  expect(projectActionTests).toContain("does not open or mutate recents when materialization fails");
  expect(sampleBackend).toContain("built_in_tutorial_is_offline_and_contains_editing_steps");
  expect(sampleBackend).toContain("failed_materialization_rolls_back_entire_sample_directory");
});
