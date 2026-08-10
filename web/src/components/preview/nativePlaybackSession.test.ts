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

  it("freezes publication as soon as pause is requested, before IPC settles", async () => {
    let finishPause!: () => void;
    const api = {
      playbackStart: vi.fn(async () => {}),
      playbackPause: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            finishPause = resolve;
          }),
      ),
      playbackSeek: vi.fn(async () => {}),
      playbackStop: vi.fn(async () => {}),
    };
    const controller = createNativePlaybackController(api, () => "pause-freeze");
    const identity = await controller.start(revision(1, 7), 0);
    controller.acceptFrame({ ...identity, frame: 40, sequence: 1, terminal: false });

    const pausing = controller.pause(identity, 40);
    await Promise.resolve();
    controller.acceptFrame({ ...identity, frame: 41, sequence: 2, terminal: false });

    expect(getNativePlaybackPublication()).toMatchObject({ frame: 40, sequence: 1 });
    finishPause();
    await pausing;
  });

  it("does not let a late pause completion re-freeze a resumed session", async () => {
    let finishPause!: () => void;
    const api = {
      playbackStart: vi.fn(async () => {}),
      playbackPause: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            finishPause = resolve;
          }),
      ),
      playbackSeek: vi.fn(async () => {}),
      playbackStop: vi.fn(async () => {}),
    };
    const controller = createNativePlaybackController(api, () => "pause-resume-race");
    const identity = await controller.start(revision(1, 7), 0);
    const pausing = controller.pause(identity, 40);
    await Promise.resolve();

    const resumed = await controller.start(revision(1, 7), 40);
    finishPause();
    await pausing;
    controller.acceptFrame({ ...resumed, frame: 41, sequence: 1, terminal: false });

    expect(getNativePlaybackPublication()).toMatchObject({ frame: 41, sequence: 1 });
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

  it("does not resume a retained source session for a different media asset", async () => {
    const h = harness();
    const first = await h.controller.start(revision(4, 6), 0, { mediaId: "main10-a" });
    await h.controller.pause(first, 12);

    const second = await h.controller.start(revision(4, 6), 12, { mediaId: "main10-b" });

    expect(second.sessionId).not.toBe(first.sessionId);
    expect(h.api.playbackStop).toHaveBeenCalledWith(first);
    expect(h.api.playbackStart).toHaveBeenLastCalledWith(12, second, "main10-b");
  });

  it("retains the exact source session across pause seek and resume", async () => {
    const h = harness();
    const first = await h.controller.start(revision(4, 6), 1_572, { mediaId: "main10" });
    await h.controller.pause(first, 1_600);
    const resumed = await h.controller.start(revision(4, 6), 1_600, { mediaId: "main10" });

    expect(resumed.sessionId).toBe(first.sessionId);
    expect(h.api.playbackStart).toHaveBeenLastCalledWith(1_600, first, "main10");
  });

  it("ignores an old source terminal event after switching media", async () => {
    const h = harness();
    const first = await h.controller.start(revision(4, 6), 0, { mediaId: "source-a" });
    const second = await h.controller.start(revision(4, 6), 0, {
      mediaId: "source-b",
      forceNewSession: true,
    });

    h.emit({ ...first, frame: 299, sequence: 99, terminal: true });
    expect(getNativePlaybackPublication()).toBeNull();

    h.emit({ ...second, frame: 1, sequence: 1, terminal: false });
    expect(getNativePlaybackPublication()).toMatchObject({
      sessionId: second.sessionId,
      frame: 1,
      terminal: false,
    });
  });

  it("lets the latest source start win while an older replacement awaits stop", async () => {
    let finishStop!: () => void;
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
    let nextId = 0;
    const controller = createNativePlaybackController(api, () => `source-${++nextId}`);
    await controller.start(revision(4, 6), 0, { mediaId: "source-old" });

    const older = controller.start(revision(4, 6), 0, { mediaId: "source-a" });
    await Promise.resolve();
    const latest = await controller.start(revision(4, 6), 0, { mediaId: "source-b" });
    finishStop();

    await expect(older).rejects.toMatchObject({ code: "superseded" });
    expect(controller.currentIdentity()).toEqual(latest);
    expect(api.playbackStart).toHaveBeenLastCalledWith(0, latest, "source-b");
  });

  it("does not dispatch a source start cancelled by its identity callback", async () => {
    const h = harness();

    await expect(
      h.controller.start(revision(4, 6), 0, {
        mediaId: "source-a",
        onIdentity: (identity) => {
          void h.controller.stop(identity);
        },
      }),
    ).rejects.toMatchObject({ code: "superseded" });

    expect(h.api.playbackStart).not.toHaveBeenCalled();
    expect(h.controller.currentIdentity()).toBeNull();
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
