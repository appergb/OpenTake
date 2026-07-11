import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { PlaybackFrameEvent } from "../../lib/types";
import { applyRustFrameBufferEffect, RustFrameBuffer } from "./RustFrameBuffer.tsx";
import {
  createRustFrameBufferState,
  failRustFrame,
  loadRustFrame,
  releaseRustFrameAfterComposite,
  requestRustFrame,
  syncRustFrameBufferIdentity,
} from "./rustFrameBuffer";

const endpoint = "http://127.0.0.1:4999/frame";

function frame(sequence: number, overrides: Partial<PlaybackFrameEvent> = {}): PlaybackFrameEvent {
  return {
    projectEpoch: 3,
    timelineVersion: 7,
    sessionId: "session-a",
    frame: 40 + sequence,
    sequence,
    terminal: false,
    ...overrides,
  };
}

function promote(
  state = createRustFrameBufferState(),
  event = frame(1),
) {
  const requested = requestRustFrame(state, event, endpoint).state;
  const pending = requested.pendingSlot;
  if (pending === null) throw new Error("expected a pending slot");
  const src = requested.slots[pending].src;
  if (!src) throw new Error("expected a pending URL");
  return loadRustFrame(requested, pending, src).state;
}

describe("retained Rust frame buffer", () => {
  it("loads the next frame into the inactive slot while keeping the active slot visible", () => {
    const active = promote();
    const next = requestRustFrame(active, frame(2), endpoint).state;

    expect(next.activeSlot).toBe(0);
    expect(next.pendingSlot).toBe(1);
    expect(next.slots[0].visible).toBe(true);
    expect(next.slots[0].frame?.sequence).toBe(1);
    expect(next.slots[1].frame?.sequence).toBe(2);
  });

  it("promotes the already loaded DOM slot without issuing a second URL", () => {
    const requested = requestRustFrame(createRustFrameBufferState(), frame(1), endpoint).state;
    const src = requested.slots[0].src!;
    const promoted = loadRustFrame(requested, 0, src).state;

    expect(promoted.activeSlot).toBe(0);
    expect(promoted.slots[0].src).toBe(src);
    expect(promoted.slots[0].requestCount).toBe(1);
  });

  it("ignores an out of order load replaced by a newer request", () => {
    const active = promote();
    const older = requestRustFrame(active, frame(2), endpoint).state;
    const olderSrc = older.slots[1].src!;
    const newer = requestRustFrame(older, frame(3), endpoint).state;
    const staleLoad = loadRustFrame(newer, 1, olderSrc);

    expect(staleLoad.effect).toBe("none");
    expect(staleLoad.state.activeSlot).toBe(0);
    expect(staleLoad.state.pendingSlot).toBe(1);
    expect(staleLoad.state.slots[1].frame?.sequence).toBe(3);
  });

  it("keeps the last good slot visible when the pending frame errors", () => {
    const active = promote();
    const requested = requestRustFrame(active, frame(2), endpoint).state;
    const failed = failRustFrame(requested, 1, requested.slots[1].src!);

    expect(failed.effect).toBe("none");
    expect(failed.state.activeSlot).toBe(0);
    expect(failed.state.slots[0].visible).toBe(true);
    expect(failed.state.pendingSlot).toBeNull();
  });

  it("does not end playback until the terminal frame is promoted", () => {
    const active = promote();
    const requested = requestRustFrame(active, frame(2, { terminal: true }), endpoint).state;
    const stop = vi.fn().mockResolvedValue(undefined);
    let painted: (() => void) | null = null;

    expect(requested.activeSlot).toBe(0);
    expect(requested.slots[0].visible).toBe(true);
    expect(stop).not.toHaveBeenCalled();
    const loaded = loadRustFrame(requested, 1, requested.slots[1].src!);
    expect(loaded.effect).toBe("terminal-promoted");
    applyRustFrameBufferEffect(loaded.effect, frame(2, { terminal: true }), {
      afterPaint: (callback) => {
        painted = callback;
      },
      setPlaying: vi.fn(),
      stop,
      onTerminalFailure: vi.fn(),
    });
    expect(stop).not.toHaveBeenCalled();
    (painted as (() => void) | null)?.();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("retries a terminal load without clearing the active frame", () => {
    const active = promote();
    const requested = requestRustFrame(active, frame(2, { terminal: true }), endpoint).state;
    const failed = failRustFrame(requested, 1, requested.slots[1].src!);

    expect(failed.effect).toBe("none");
    expect(failed.state.activeSlot).toBe(0);
    expect(failed.state.slots[0].visible).toBe(true);
    expect(failed.state.pendingSlot).toBe(1);
    expect(failed.state.slots[1].src).toContain("retry=1");
  });

  it("stops transport and retains the last good frame after terminal retries are exhausted", () => {
    let state = promote();
    state = requestRustFrame(state, frame(2, { terminal: true }), endpoint).state;
    let result = failRustFrame(state, 1, state.slots[1].src!);
    result = failRustFrame(result.state, 1, result.state.slots[1].src!);
    result = failRustFrame(result.state, 1, result.state.slots[1].src!);

    expect(result.effect).toBe("terminal-exhausted");
    expect(result.state.activeSlot).toBe(0);
    expect(result.state.slots[0].visible).toBe(true);
    expect(result.state.pendingSlot).toBeNull();

    const setPlaying = vi.fn();
    const stop = vi.fn().mockResolvedValue(undefined);
    const onTerminalFailure = vi.fn();
    applyRustFrameBufferEffect("terminal-exhausted", frame(2, { terminal: true }), {
      afterPaint: (callback) => callback(),
      setPlaying,
      stop,
      onTerminalFailure,
    });
    expect(stop).toHaveBeenCalledWith({
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "session-a",
    });
    expect(setPlaying).toHaveBeenCalledWith(false);
    expect(onTerminalFailure).toHaveBeenCalledTimes(1);
  });

  it("clears both slots when project epoch timeline version or session id changes", () => {
    const active = promote();
    for (const identity of [
      { projectEpoch: 4, timelineVersion: 7, sessionId: "session-a" },
      { projectEpoch: 3, timelineVersion: 8, sessionId: "session-a" },
      { projectEpoch: 3, timelineVersion: 7, sessionId: "session-b" },
    ]) {
      const cleared = syncRustFrameBufferIdentity(active, identity);
      expect(cleared.activeSlot).toBeNull();
      expect(cleared.pendingSlot).toBeNull();
      expect(cleared.slots.every((slot) => slot.src === null && !slot.visible)).toBe(true);
    }
  });

  it("keeps two stable Rust frame image slots mounted", () => {
    const html = renderToStaticMarkup(
      React.createElement(RustFrameBuffer, {
        event: null,
        endpoint: null,
        projectEpoch: 3,
        timelineVersion: 7,
        engineDriving: false,
        requestCompositeStill: vi.fn().mockResolvedValue(null),
        onTerminalFailure: vi.fn(),
      }),
    );

    expect(html.match(/data-rust-frame-slot=/g)).toHaveLength(2);
  });

  it("holds the painted terminal frame after engineDriving becomes false", () => {
    const terminal = promote(createRustFrameBufferState(), frame(9, { terminal: true }));
    const held = releaseRustFrameAfterComposite(terminal, {
      engineDriving: false,
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "session-a",
      frame: 48,
      compositeLoaded: false,
    });
    const released = releaseRustFrameAfterComposite(held, {
      engineDriving: false,
      projectEpoch: 3,
      timelineVersion: 7,
      sessionId: "session-a",
      frame: 49,
      compositeLoaded: true,
    });

    expect(held.activeSlot).not.toBeNull();
    expect(held.slots[held.activeSlot!].visible).toBe(true);
    expect(released.activeSlot).toBeNull();
  });
});
