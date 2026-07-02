/**
 * TransformOverlay (T3-10) render tests. Renders the component directly with
 * an explicit `canvasPx`, instead of through <Preview/>'s ResizeObserver-driven
 * `fittedCanvas` — `renderToStaticMarkup` never runs `useEffect`, so that value
 * is always null in Preview.test.tsx and can't exercise the positive render
 * path there. See Preview.test.tsx's "Transform overlay mount guard" describe
 * block for the (negative-only) guard coverage at the Preview.tsx level.
 */
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Clip } from "../../lib/types";

const uiStore = vi.hoisted(() => ({ activeFrame: 0 }));

vi.mock("../../store/uiStore", () => ({
  useEditorUiStore: Object.assign((selector: (state: typeof uiStore) => unknown) => selector(uiStore), {
    getState: () => uiStore,
  }),
}));

vi.mock("../../store/editActions", () => ({
  setClipProperties: vi.fn(),
}));

import { TransformOverlay } from "./TransformOverlay";

function clip(over: Partial<Clip> = {}): Clip {
  return {
    id: "clip",
    mediaRef: "asset",
    mediaType: "video",
    sourceClipType: "video",
    startFrame: 0,
    durationFrames: 100,
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
      width: 0.4,
      height: 0.3,
      rotation: 0,
      flipHorizontal: false,
      flipVertical: false,
    },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    ...over,
  };
}

describe("TransformOverlay", () => {
  it("positions and sizes the box from transform * canvasPx", () => {
    const html = renderToStaticMarkup(
      <TransformOverlay clip={clip()} canvasPx={{ width: 1000, height: 500 }} mediaAspect={null} />,
    );

    expect(html).toContain('data-testid="transform-overlay"');
    expect(html).toContain("left:500"); // centerX(0.5) * 1000
    expect(html).toContain("top:250"); // centerY(0.5) * 500
    expect(html).toContain("width:400"); // width(0.4) * 1000
    expect(html).toContain("height:150"); // height(0.3) * 500
    expect(html).toContain("translate(-50%, -50%) rotate(0deg)");
  });

  it("rotates the box via the transform's rotation degrees", () => {
    const html = renderToStaticMarkup(
      <TransformOverlay
        clip={clip({ transform: { ...clip().transform, rotation: 45 } })}
        canvasPx={{ width: 1000, height: 500 }}
        mediaAspect={null}
      />,
    );

    expect(html).toContain("rotate(45deg)");
  });

  it("renders 4 corner handles at the OpenTake spacing/opacity tokens matching upstream AppTheme.Spacing.smMd / Opacity.strong", () => {
    const html = renderToStaticMarkup(
      <TransformOverlay clip={clip()} canvasPx={{ width: 1000, height: 500 }} mediaAspect={null} />,
    );

    expect(html.match(/cursor:nwse-resize/g)?.length).toBe(2); // topLeft + bottomRight
    expect(html.match(/cursor:nesw-resize/g)?.length).toBe(2); // topRight + bottomLeft
    expect(html).toContain("width:8px;height:8px"); // AppTheme.Spacing.smMd
    expect(html).toContain("background:rgba(255,255,255,0.55)"); // white @ Opacity.strong
    expect(html).toContain("border:1px solid rgba(255,255,255,0.55)"); // BorderWidth.thin
  });

  it("keeps the outer container pointer-events:none and only the move-surface/handles pointer-events:auto", () => {
    const html = renderToStaticMarkup(
      <TransformOverlay clip={clip()} canvasPx={{ width: 1000, height: 500 }} mediaAspect={null} />,
    );
    const overlayStart = html.indexOf('data-testid="transform-overlay"');

    // The outer <div data-testid=...> itself carries pointer-events:none.
    expect(html.slice(overlayStart, overlayStart + 300)).toContain("pointer-events:none");
    // 1 move-surface + 4 corner handles = 5 pointer-events:auto elements.
    expect(html.match(/pointer-events:auto/g)?.length).toBe(5);
  });

  it("follows a live keyframe track (sampledTransform), not the static transform, when one is active", () => {
    uiStore.activeFrame = 60;
    const animated = clip({
      startFrame: 0,
      positionTrack: {
        keyframes: [{ frame: 60, value: { a: 0.1, b: 0.2 }, interpolationOut: "hold" }],
      },
    });

    const html = renderToStaticMarkup(
      <TransformOverlay clip={animated} canvasPx={{ width: 1000, height: 1000 }} mediaAspect={null} />,
    );

    // topLeft (0.1, 0.2) + half of the static size (0.4/2, 0.3/2) = center (0.3, 0.35).
    expect(html).toContain("left:300");
    expect(html).toContain("top:350");
    uiStore.activeFrame = 0;
  });

  it("renders nothing for a degenerate (zero-size) canvas", () => {
    const html = renderToStaticMarkup(
      <TransformOverlay clip={clip()} canvasPx={{ width: 0, height: 0 }} mediaAspect={null} />,
    );

    expect(html).toBe("");
  });
});
