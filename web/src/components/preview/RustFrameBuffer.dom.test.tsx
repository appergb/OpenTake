// @vitest-environment happy-dom

import React from "react";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackFrameEvent } from "../../lib/types";
import { RustFrameBuffer } from "./RustFrameBuffer.tsx";

afterEach(() => {
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

describe("paused native composite", () => {
  it("cancels an in-flight settled composite when scrubbing starts", async () => {
    const requestCompositeStill = vi.fn(() => new Promise(() => undefined));
    const cancelCompositeStill = vi.fn().mockResolvedValue(undefined);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={null}
          endpoint={null}
          projectEpoch={3}
          timelineVersion={7}
          engineDriving={false}
          stillFrame={100}
          requestCompositeStill={requestCompositeStill}
          cancelCompositeStill={cancelCompositeStill}
          onTerminalFailure={vi.fn()}
        />,
      );
    });
    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={null}
          endpoint={null}
          projectEpoch={3}
          timelineVersion={7}
          engineDriving={false}
          stillFrame={null}
          requestCompositeStill={requestCompositeStill}
          cancelCompositeStill={cancelCompositeStill}
          onTerminalFailure={vi.fn()}
        />,
      );
    });

    expect(cancelCompositeStill).toHaveBeenCalledWith(
      expect.objectContaining({
        projectEpoch: 3,
        timelineVersion: 7,
        minimumSeekGeneration: 1,
      }),
    );
    await act(async () => root.unmount());
  });

  it("retains the last settled composite while scrub cancels exact-frame work", async () => {
    const requestCompositeStill = vi.fn().mockResolvedValue({
      width: 640,
      height: 360,
      dataUrl: "data:image/png;base64,last-good",
    });
    const cancelCompositeStill = vi.fn().mockResolvedValue(undefined);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const render = (stillFrame: number | null) => (
      <RustFrameBuffer
        event={null}
        endpoint={null}
        projectEpoch={3}
        timelineVersion={7}
        engineDriving={false}
        stillFrame={stillFrame}
        requestCompositeStill={requestCompositeStill}
        cancelCompositeStill={cancelCompositeStill}
        onTerminalFailure={vi.fn()}
      />
    );

    await act(async () => {
      root.render(render(100));
      await Promise.resolve();
    });
    expect(container.querySelector('[src="data:image/png;base64,last-good"]')).not.toBeNull();

    await act(async () => root.render(render(null)));

    expect(cancelCompositeStill).toHaveBeenCalledWith(
      expect.objectContaining({ minimumSeekGeneration: 1 }),
    );
    expect(container.querySelector('[src="data:image/png;base64,last-good"]')).not.toBeNull();
    await act(async () => root.unmount());
  });

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

    expect(requestCompositeStill).toHaveBeenCalledWith(
      expect.objectContaining({
        frame: 480,
        projectEpoch: 3,
        timelineVersion: 7,
        seekGeneration: 0,
      }),
    );
    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).not.toBeNull();
    await act(async () => root.unmount());
  });

  it("coalesces rapid frame changes to one in-flight request plus the latest frame", async () => {
    const pending = new Map<
      number,
      { resolve: (value: { width: number; height: number; dataUrl: string }) => void }
    >();
    const requestCompositeStill = vi.fn(
      (request: { frame: number }) =>
        new Promise<{ width: number; height: number; dataUrl: string }>((resolve) => {
          pending.set(request.frame, { resolve });
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
    expect(requestCompositeStill.mock.calls.map(([request]) => request.frame)).toEqual([100]);

    await act(async () => {
      pending.get(100)!.resolve({ width: 640, height: 360, dataUrl: "data:old" });
      await Promise.resolve();
    });
    expect(requestCompositeStill.mock.calls.map(([request]) => request.frame)).toEqual([100, 102]);
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
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
      drawImage: vi.fn(),
    } as never);
    Object.defineProperties(liveFrame!, {
      currentSrc: { configurable: true, value: liveFrame!.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });
    await act(async () => liveFrame!.dispatchEvent(new Event("load", { bubbles: true })));

    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).toBeNull();
    await act(async () => root.unmount());
  });

  it("paints live frames onto one stable canvas instead of exposing decoder images", async () => {
    const drawImage = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage } as never);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const event: PlaybackFrameEvent = {
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "session-canvas",
      frame: 481,
      sequence: 1,
      terminal: false,
    };

    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={event}
          endpoint="http://127.0.0.1/frame"
          projectEpoch={3}
          timelineVersion={7}
          engineDriving
          requestCompositeStill={vi.fn().mockResolvedValue(null)}
          onTerminalFailure={vi.fn()}
        />,
      );
    });

    const decoder = container.querySelector<HTMLImageElement>('img[data-rust-frame-slot][src]');
    const canvas = container.querySelector<HTMLCanvasElement>('[data-testid="rust-live-canvas"]');
    expect(decoder).not.toBeNull();
    expect(canvas).not.toBeNull();
    expect(decoder!.style.visibility).toBe("hidden");
    Object.defineProperties(decoder!, {
      currentSrc: { configurable: true, value: decoder!.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });

    await act(async () => decoder!.dispatchEvent(new Event("load", { bubbles: true })));

    expect(drawImage).toHaveBeenCalledWith(decoder, 0, 0);
    expect(canvas!.width).toBe(640);
    expect(canvas!.height).toBe(360);
    expect(canvas!.style.visibility).toBe("visible");
    await act(async () => root.unmount());
  });

  it("does not paint a live frame that finishes loading after transport paused", async () => {
    const drawImage = vi.fn();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage } as never);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const event: PlaybackFrameEvent = {
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "pause-late-frame",
      frame: 481,
      sequence: 1,
      terminal: false,
    };
    const render = (engineDriving: boolean) => (
      <RustFrameBuffer
        event={event}
        endpoint="http://127.0.0.1/frame"
        projectEpoch={3}
        timelineVersion={7}
        engineDriving={engineDriving}
        stillFrame={engineDriving ? null : 480}
        requestCompositeStill={vi.fn().mockResolvedValue(null)}
        onTerminalFailure={vi.fn()}
      />
    );

    await act(async () => root.render(render(true)));
    const decoder = container.querySelector<HTMLImageElement>('img[data-rust-frame-slot][src]')!;
    Object.defineProperties(decoder, {
      currentSrc: { configurable: true, value: decoder.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });

    await act(async () => root.render(render(false)));
    await act(async () => decoder.dispatchEvent(new Event("load", { bubbles: true })));

    expect(drawImage).not.toHaveBeenCalled();
    await act(async () => root.unmount());
  });

  it("retains the paused still when a live frame cannot get a canvas context", async () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
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
      sessionId: "session-no-context",
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
    await act(async () => {
      root.render(
        <RustFrameBuffer
          event={event}
          endpoint="http://127.0.0.1/frame"
          projectEpoch={3}
          timelineVersion={7}
          engineDriving
          requestCompositeStill={requestCompositeStill}
          onTerminalFailure={vi.fn()}
        />,
      );
    });

    const decoder = container.querySelector<HTMLImageElement>('img[data-rust-frame-slot][src]');
    const canvas = container.querySelector<HTMLCanvasElement>('[data-testid="rust-live-canvas"]');
    Object.defineProperties(decoder!, {
      currentSrc: { configurable: true, value: decoder!.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });
    await act(async () => decoder!.dispatchEvent(new Event("load", { bubbles: true })));

    expect(container.querySelector('[data-testid="rust-idle-composite-still"]')).not.toBeNull();
    expect(canvas!.style.visibility).toBe("hidden");
    await act(async () => root.unmount());
  });

  it("keeps the last good frame active when the next canvas draw throws", async () => {
    const drawImage = vi
      .fn()
      .mockImplementationOnce(() => undefined)
      .mockImplementationOnce(() => {
        throw new Error("draw failed");
      });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage } as never);
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    const renderFrame = (sequence: number) => (
      <RustFrameBuffer
        event={{
          projectEpoch: 3,
          timelineVersion: 7,
          sessionId: "session-draw-failure",
          frame: 480 + sequence,
          sequence,
          terminal: false,
        }}
        endpoint="http://127.0.0.1/frame"
        projectEpoch={3}
        timelineVersion={7}
        engineDriving
        requestCompositeStill={vi.fn().mockResolvedValue(null)}
        onTerminalFailure={vi.fn()}
      />
    );

    await act(async () => root.render(renderFrame(1)));
    let decoder = container.querySelector<HTMLImageElement>('img[data-rust-frame-slot][src]')!;
    Object.defineProperties(decoder, {
      currentSrc: { configurable: true, value: decoder.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });
    await act(async () => decoder.dispatchEvent(new Event("load", { bubbles: true })));

    await act(async () => root.render(renderFrame(2)));
    const decoders = Array.from(
      container.querySelectorAll<HTMLImageElement>('img[data-rust-frame-slot][src]'),
    );
    decoder = decoders.find((candidate) => candidate.src.includes("sequence=2"))!;
    Object.defineProperties(decoder, {
      currentSrc: { configurable: true, value: decoder.src },
      naturalWidth: { configurable: true, value: 640 },
      naturalHeight: { configurable: true, value: 360 },
    });
    await act(async () => decoder.dispatchEvent(new Event("load", { bubbles: true })));

    expect(drawImage).toHaveBeenCalledTimes(2);
    expect(container.querySelector<HTMLCanvasElement>('[data-testid="rust-live-canvas"]')!.style.visibility)
      .toBe("visible");
    await act(async () => root.unmount());
  });
});
