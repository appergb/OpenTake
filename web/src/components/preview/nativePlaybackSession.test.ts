import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  PlaybackCommandError,
  PlaybackFrameEvent,
  PlaybackIdentity,
  ProjectRevision,
} from "../../lib/types";
import {
  clearNativePlaybackPublication,
  createNativePlaybackController,
  getNativePlaybackPublication,
} from "./nativePlaybackSession";

function revision(projectEpoch: number, timelineVersion: number): ProjectRevision {
  return { projectEpoch, timelineVersion };
}

function frame(identity: PlaybackIdentity, sequence = 1): PlaybackFrameEvent {
  return { ...identity, frame: 12, sequence, terminal: false };
}

function harness() {
  const order: string[] = [];
  const api = {
    playbackStart: vi.fn(async () => {
      order.push("start");
    }),
    playbackPause: vi.fn(async () => {}),
    playbackSeek: vi.fn(async () => {}),
    playbackStop: vi.fn(async () => {}),
  };
  let nextId = 0;
  const controller = createNativePlaybackController(api, () => `session-${++nextId}`);
  return {
    api,
    controller,
    order,
    emit(event: PlaybackFrameEvent) {
      controller.acceptFrame(event);
    },
  };
}

beforeEach(() => clearNativePlaybackPublication());

describe("native playback identity", () => {
  it("stops a retained session when project epoch changes but both versions are zero", async () => {
    const h = harness();
    const first = await h.controller.start(revision(10, 0), 0);
    await h.controller.pause(first, 4);

    const replacement = await h.controller.start(revision(11, 0), 0);

    expect(h.api.playbackStop).toHaveBeenCalledWith(first);
    expect(replacement.projectEpoch).toBe(11);
    expect(replacement.sessionId).not.toBe(first.sessionId);
  });

  it("rejects a frame from a stale backend session even when revisions collide", async () => {
    const h = harness();
    const current = await h.controller.start(revision(5, 8), 0);

    h.emit(frame({ ...current, sessionId: "stale-session" }));

    expect(getNativePlaybackPublication()).toBeNull();
  });

  it("does not let stale cleanup pause a replacement session", async () => {
    const h = harness();
    const old = await h.controller.start(revision(2, 3), 0);
    const replacement = await h.controller.start(revision(2, 3), 0, { forceNewSession: true });

    await h.controller.cleanup(old, "pause", 9);

    expect(h.api.playbackPause).not.toHaveBeenCalledWith(old, 9);
    expect(h.controller.currentIdentity()).toEqual(replacement);
  });

  it("retains one session id across pause and resume of the exact revision", async () => {
    const h = harness();
    const first = await h.controller.start(revision(1, 7), 0);
    await h.controller.pause(first, 11);
    const resumed = await h.controller.start(revision(1, 7), 11);

    expect(resumed.sessionId).toBe(first.sessionId);
  });

  it("mints a new session id after project or timeline revision changes", async () => {
    const h = harness();
    const first = await h.controller.start(revision(1, 7), 0);
    await h.controller.pause(first, 1);
    const timelineChange = await h.controller.start(revision(1, 8), 1);
    await h.controller.pause(timelineChange, 2);
    const projectChange = await h.controller.start(revision(2, 8), 2);

    expect(new Set([first.sessionId, timelineChange.sessionId, projectChange.sessionId]).size).toBe(3);
  });

  it("session scopes pause seek and stop commands", async () => {
    const h = harness();
    const current = await h.controller.start(revision(4, 6), 0);

    await h.controller.pause(current, 3);
    await h.controller.seek(current, 8);
    await h.controller.stop(current);

    expect(h.api.playbackPause).toHaveBeenCalledWith(current, 3);
    expect(h.api.playbackSeek).toHaveBeenCalledWith(current, 8);
    expect(h.api.playbackStop).toHaveBeenCalledWith(current);
  });

  it("retires a stopped identity before the stop IPC resolves", async () => {
    let finishStop: (() => void) | undefined;
    const api = {
      playbackStart: vi.fn(async () => {}),
      playbackPause: vi.fn(async () => {}),
      playbackSeek: vi.fn(async () => {}),
      playbackStop: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            finishStop = resolve;
          }),
      ),
    };
    const controller = createNativePlaybackController(api, () => "retired-session");
    const current = await controller.start(revision(4, 6), 0);

    const stopping = controller.stop(current);
    controller.acceptFrame(frame(current, 1));

    expect(controller.currentIdentity()).toBeNull();
    expect(getNativePlaybackPublication()).toBeNull();
    finishStop?.();
    await stopping;
  });

  it("publishes only increasing matching frame sequences", async () => {
    const h = harness();
    const current = await h.controller.start(revision(3, 1), 0);
    h.emit(frame(current, 2));
    h.emit(frame(current, 1));

    expect(getNativePlaybackPublication()?.sequence).toBe(2);
  });

  it("allows fallback only for engine failures and unavailable playback commands", () => {
    const controller = harness().controller;
    for (const code of ["superseded", "cancelled", "busy"] as const) {
      const error: PlaybackCommandError = { code, message: code };
      expect(controller.shouldFallback(error)).toBe(false);
    }
    expect(controller.shouldFallback({ code: "engine", message: "gpu" })).toBe(true);
    expect(controller.shouldFallback("Command playback_start not found")).toBe(true);
    expect(controller.shouldFallback("Command project_save not found")).toBe(false);
  });
});
