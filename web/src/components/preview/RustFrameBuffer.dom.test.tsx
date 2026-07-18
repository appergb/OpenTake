// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackFrameEvent } from "../../lib/types";
import { RustFrameBuffer } from "./RustFrameBuffer.tsx";

afterEach(() => document.body.replaceChildren());

describe("paused native composite", () => {
  it("requests and retains the current frame without a playback publication", async () => {
    const requestCompositeStill = vi.fn().mockResolvedValue({
      width: 640,
      height: 360,
      dataUrl: "data:image/png;base64,nonblack",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);

    await act(async () => {
      root.render(
        React.createElement(RustFrameBuffer, {
          event: null,
          endpoint: null,
          projectEpoch: 3,
          timelineVersion: 7,
          engineDriving: false,
          stillFrame: 480,
          requestCompositeStill,
          onTerminalFailure: vi.fn(),
        } as never),
      );
      await Promise.resolve();
    });

    expect(requestCompositeStill).toHaveBeenCalledWith(480);
    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).not.toBeNull();
    await act(async () => root.unmount());
  });

  it("coalesces rapid frame changes to one in-flight request plus the latest frame", async () => {
    const pending = new Map<
      number,
      { resolve: (value: { width: number; height: number; dataUrl: string }) => void }
    >();
    const requestCompositeStill = vi.fn(
      (frame: number) =>
        new Promise<{ width: number; height: number; dataUrl: string }>((resolve) => {
          pending.set(frame, { resolve });
        }),
    );
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const renderFrame = (stillFrame: number) =>
      React.createElement(RustFrameBuffer, {
        event: null,
        endpoint: null,
        projectEpoch: 3,
        timelineVersion: 7,
        engineDriving: false,
        stillFrame,
        requestCompositeStill,
        onTerminalFailure: vi.fn(),
      });

    await act(async () => root.render(renderFrame(100)));
    await act(async () => root.render(renderFrame(101)));
    await act(async () => root.render(renderFrame(102)));
    expect(requestCompositeStill.mock.calls.map(([frame]) => frame)).toEqual([100]);

    await act(async () => {
      pending.get(100)!.resolve({ width: 640, height: 360, dataUrl: "data:old" });
      await Promise.resolve();
    });
    expect(requestCompositeStill.mock.calls.map(([frame]) => frame)).toEqual([100, 102]);
    expect(container.querySelector('[src="data:old"]')).toBeNull();

    await act(async () => {
      pending.get(102)!.resolve({ width: 640, height: 360, dataUrl: "data:latest" });
      await Promise.resolve();
    });
    expect(container.querySelector('[src="data:latest"]')).not.toBeNull();
    await act(async () => root.unmount());
  });

  it("keeps the paused composite until the first live native frame loads", async () => {
    const requestCompositeStill = vi.fn().mockResolvedValue({
      width: 640,
      height: 360,
      dataUrl: "data:image/png;base64,paused",
    });
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const event: PlaybackFrameEvent = {
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "session-1",
      frame: 481,
      sequence: 1,
      terminal: false,
    };

    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={null}
          endpoint="http://127.0.0.1/frame"
          projectEpoch={3}
          timelineVersion={7}
          engineDriving={false}
          stillFrame={480}
          requestCompositeStill={requestCompositeStill}
          onTerminalFailure={vi.fn()}
        />,
      );
      await Promise.resolve();
    });
    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).not.toBeNull();

    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={event}
          endpoint="http://127.0.0.1/frame"
          projectEpoch={3}
          timelineVersion={7}
          engineDriving
          stillFrame={null}
          requestCompositeStill={requestCompositeStill}
          onTerminalFailure={vi.fn()}
        />,
      );
    });
    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).not.toBeNull();

    const liveFrame = container.querySelector<HTMLImageElement>(
      'img[data-rust-frame-slot][src]',
    );
    expect(liveFrame).not.toBeNull();
    Object.defineProperty(liveFrame!, "currentSrc", {
      configurable: true,
      value: liveFrame!.src,
    });
    await act(async () => liveFrame!.dispatchEvent(new Event("load", { bubbles: true })));

    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).toBeNull();
    await act(async () => root.unmount());
  });
});
