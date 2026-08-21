import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeTimelineSnapshot, Timeline } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const srv = vi.hoisted(() => {
  const timeline: Timeline = {
    fps: 30,
    width: 1920,
    height: 1080,
    settingsConfigured: true,
    tracks: [],
  };
  return {
    timeline,
    projectPath: null as string | null,
    compatibilityReadOnly: false,
    snapshotResponses: [] as Array<Promise<RuntimeTimelineSnapshot>>,
    undoResponses: [] as Array<Promise<boolean>>,
    redoResponses: [] as Array<Promise<boolean>>,
    timelineListenerResponses: [] as Array<Promise<() => void>>,
    openedListenerResponses: [] as Array<Promise<() => void>>,
    undoCalls: 0,
    redoCalls: 0,
    timelineListenerCalls: 0,
    openedListenerCalls: 0,
    mediaError: null as string | null,
    projectOpenedHandlers: [] as Array<
      (path: string, projectEpoch: number, version: number) => Promise<void> | void
    >,
    playbackResponses: [] as Array<Promise<void>>,
    order: [] as string[],
    onProjectOpened: null as null | ((path: string, projectEpoch: number, version: number) => Promise<void> | void),
    onTimelineChanged: null as null | ((projectEpoch: number, version: number) => Promise<void> | void),
    onProjectSaved: null as null | ((path: string, projectEpoch: number) => void),
    savedListenerResponses: [] as Array<Promise<() => void>>,
    savedListenerCalls: 0,
    invalidate: vi.fn(async () => {
      srv.order.push("invalidate");
      const queued = srv.playbackResponses.shift();
      if (queued) await queued;
    }),
    resetMediaTransient: vi.fn(() => {
      srv.mediaError = null;
    }),
    refreshMedia: vi.fn(async () => true),
  };
});

vi.mock("../lib/api", () => ({
  getTimeline: async () => {
    srv.order.push("refresh");
    const queued = srv.snapshotResponses.shift();
    if (queued) return queued;
    return {
      timeline: srv.timeline,
      projectEpoch: 1,
      version: 0,
      projectPath: srv.projectPath,
      compatibilityReadOnly: srv.compatibilityReadOnly,
      compatibilityBlockers: srv.compatibilityReadOnly ? ["manifest.futureField"] : [],
    };
  },
  canUndo: async () => {
    srv.undoCalls += 1;
    return (await srv.undoResponses.shift()) ?? false;
  },
  canRedo: async () => {
    srv.redoCalls += 1;
    return (await srv.redoResponses.shift()) ?? false;
  },
  onTimelineChanged: async (
    handler: (projectEpoch: number, version: number) => Promise<void> | void,
  ) => {
    srv.timelineListenerCalls += 1;
    srv.order.push("timeline-listen");
    srv.onTimelineChanged = handler;
    return (await srv.timelineListenerResponses.shift()) ?? (() => {});
  },
  onProjectOpened: async (
    handler: (path: string, projectEpoch: number, version: number) => Promise<void> | void,
  ) => {
    srv.openedListenerCalls += 1;
    srv.order.push("opened-listen");
    srv.onProjectOpened = handler;
    srv.projectOpenedHandlers.push(handler);
    return (await srv.openedListenerResponses.shift()) ?? (() => {});
  },
  onProjectSaved: async (handler: (path: string, projectEpoch: number) => void) => {
    srv.savedListenerCalls += 1;
    srv.order.push("saved-listen");
    srv.onProjectSaved = handler;
    return (await srv.savedListenerResponses.shift()) ?? (() => {});
  },
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.invalidate,
}));

vi.mock("./mediaStore", () => ({
  resetProjectMediaState: srv.resetMediaTransient,
  refreshMedia: srv.refreshMedia,
}));

import { forceRefresh, startSync, stopSync } from "./sync";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";

function snapshot(
  projectEpoch: number,
  version: number,
  projectPath: string | null,
  compatibilityReadOnly = false,
): RuntimeTimelineSnapshot {
  return {
    timeline: srv.timeline,
    projectEpoch,
    version,
    projectPath,
    compatibilityReadOnly,
    compatibilityBlockers: compatibilityReadOnly ? ["manifest.futureField"] : [],
  };
}

beforeEach(() => {
  useProjectStore.getState().clearProjectSnapshot();
  srv.snapshotResponses.length = 0;
  srv.undoResponses.length = 0;
  srv.redoResponses.length = 0;
  srv.timelineListenerResponses.length = 0;
  srv.openedListenerResponses.length = 0;
  srv.savedListenerResponses.length = 0;
  srv.undoCalls = 0;
  srv.redoCalls = 0;
  srv.timelineListenerCalls = 0;
  srv.openedListenerCalls = 0;
  srv.savedListenerCalls = 0;
  srv.mediaError = null;
  srv.projectOpenedHandlers.length = 0;
  srv.onTimelineChanged = null;
  srv.onProjectSaved = null;
  srv.playbackResponses.length = 0;
  srv.resetMediaTransient.mockClear();
  srv.refreshMedia.mockClear();
  useEditorUiStore.setState({ toast: null });
});

afterEach(() => {
  stopSync();
  srv.order.length = 0;
  srv.invalidate.mockClear();
  srv.onProjectOpened = null;
  srv.onProjectSaved = null;
  srv.projectPath = null;
  srv.compatibilityReadOnly = false;
});

describe("project event sync", () => {
  it("clamps both playhead values when a refresh shortens the timeline", async () => {
    const previousTimeline = srv.timeline;
    srv.timeline = {
      fps: 30,
      width: 1280,
      height: 720,
      settingsConfigured: true,
      tracks: [
        {
          id: "v1",
          type: "video",
          muted: false,
          hidden: false,
          syncLocked: true,
          clips: [
            {
              id: "clip-short",
              mediaRef: "media",
              mediaType: "video",
              sourceClipType: "video",
              startFrame: 0,
              durationFrames: 90,
              trimStartFrame: 0,
              trimEndFrame: 0,
              speed: 1,
              volume: 1,
              fadeInFrames: 0,
              fadeOutFrames: 0,
              fadeInInterpolation: "linear",
              fadeOutInterpolation: "linear",
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
            },
          ],
        },
      ],
    };
    useProjectStore.setState({ projectEpoch: 1, timelineVersion: 0, projectPath: null });
    useEditorUiStore.setState({ currentFrame: 231, activeFrame: 231 });
    srv.snapshotResponses.push(Promise.resolve(snapshot(1, 1, null)));

    try {
      await forceRefresh();
      expect(useEditorUiStore.getState().currentFrame).toBe(90);
      expect(useEditorUiStore.getState().activeFrame).toBe(90);
    } finally {
      srv.timeline = previousTimeline;
    }
  });

  it("refreshes again after both listeners register so an event in the setup gap is not lost", async () => {
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(1, 0, "/tmp/old.opentake")),
      Promise.resolve(snapshot(2, 0, "/tmp/new.opentake")),
    );

    await startSync();

    expect(srv.order.slice(0, 5)).toEqual([
      "refresh",
      "timeline-listen",
      "opened-listen",
      "saved-listen",
      "refresh",
    ]);
    expect(useProjectStore.getState().projectEpoch).toBe(2);
    expect(useProjectStore.getState().projectPath).toBe("/tmp/new.opentake");
  });

  it("does not let the startup gap refresh supersede an observed timeline floor", async () => {
    const openedRegistration = deferred<() => void>();
    const eventSnapshot = deferred<RuntimeTimelineSnapshot>();
    const startupGapSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.openedListenerResponses.push(openedRegistration.promise);
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(1, 0, null)),
      eventSnapshot.promise,
      startupGapSnapshot.promise,
      Promise.resolve(snapshot(1, 2, null)),
    );

    const startup = startSync();
    await vi.waitFor(() => expect(srv.openedListenerCalls).toBe(1));

    const event = srv.onTimelineChanged?.(1, 2);
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(2);
    });
    openedRegistration.resolve(() => {});

    eventSnapshot.resolve(snapshot(1, 2, null));
    await event;
    const versionWhenEventSettled = useProjectStore.getState().timelineVersion;

    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(3);
    });
    startupGapSnapshot.resolve(snapshot(1, 1, null));
    await startup;

    expect(versionWhenEventSettled).toBe(2);
    expect(useProjectStore.getState().timelineVersion).toBe(2);
  });

  it("can retry startup after the initial mirror refresh rejects", async () => {
    srv.snapshotResponses.push(Promise.reject(new Error("initial timeline unavailable")));

    await expect(startSync()).rejects.toThrow("initial timeline unavailable");
    await startSync();

    expect(srv.timelineListenerCalls).toBe(1);
    expect(srv.openedListenerCalls).toBe(1);
  });

  it("cleans a partial listener and can retry when the second registration rejects", async () => {
    const timelineUnsubscribe = vi.fn();
    srv.timelineListenerResponses.push(Promise.resolve(timelineUnsubscribe));
    srv.openedListenerResponses.push(Promise.reject(new Error("project listener unavailable")));

    await expect(startSync()).rejects.toThrow("project listener unavailable");
    expect(timelineUnsubscribe).toHaveBeenCalledOnce();

    await startSync();
    expect(srv.timelineListenerCalls).toBe(2);
    expect(srv.openedListenerCalls).toBe(2);
  });

  it("invalidates project scoped playback on externally initiated project_opened", async () => {
    await startSync();
    srv.order.length = 0;
    srv.resetMediaTransient.mockClear();
    srv.refreshMedia.mockClear();
    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 77,
      activeFrame: 77,
      selectedClipIds: new Set(["old-clip"]),
      layoutPreset: "media",
    });

    srv.projectPath = "/tmp/snapshot.opentake";
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(7, 0, "/tmp/snapshot.opentake")),
    );
    await srv.onProjectOpened?.("/tmp/event-payload.opentake", 7, 0);

    expect(srv.order.slice(0, 2)).toEqual(["invalidate", "refresh"]);
    expect(useProjectStore.getState().projectEpoch).toBe(7);
    expect(useProjectStore.getState().projectPath).toBe("/tmp/snapshot.opentake");
    const ui = useEditorUiStore.getState();
    expect(ui.isPlaying).toBe(false);
    expect(ui.currentFrame).toBe(0);
    expect(ui.activeFrame).toBe(0);
    expect(ui.selectedClipIds.size).toBe(0);
    expect(ui.layoutPreset).toBe("media");
    expect(srv.resetMediaTransient).toHaveBeenCalledTimes(1);
    expect(srv.refreshMedia).toHaveBeenCalledTimes(1);
  });

  it("retries a rejected timeline event refresh and converges without leaking a rejection", async () => {
    await startSync();
    srv.snapshotResponses.push(
      Promise.reject(new Error("transient timeline read failed")),
      Promise.resolve(snapshot(1, 1, null)),
    );

    await srv.onTimelineChanged?.(1, 1);

    expect(useProjectStore.getState().timelineVersion).toBe(1);
    expect(useEditorUiStore.getState().toast).toBeNull();
  });

  it("reports a timeline event refresh that still fails after its bounded retry", async () => {
    await startSync();
    srv.snapshotResponses.push(
      Promise.reject(new Error("timeline read unavailable")),
      Promise.reject(new Error("timeline read still unavailable")),
    );

    await srv.onTimelineChanged?.(1, 1);

    expect(useEditorUiStore.getState().toast?.message).toContain(
      "timeline read still unavailable",
    );
    expect(useProjectStore.getState().timelineVersion).toBe(0);
  });

  it("waits for a force-refresh owner that reaches the observed event floor", async () => {
    await startSync();
    const eventSnapshot = deferred<RuntimeTimelineSnapshot>();
    const forceSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(eventSnapshot.promise, forceSnapshot.promise);

    const event = srv.onTimelineChanged?.(1, 1);
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(3);
    });
    const forced = forceRefresh();
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
    });

    eventSnapshot.resolve(snapshot(1, 1, null));
    forceSnapshot.resolve(snapshot(1, 1, null));
    await Promise.all([event, forced]);

    expect(useProjectStore.getState().timelineVersion).toBe(1);
    expect(useEditorUiStore.getState().toast).toBeNull();
  });

  it("chases the observed event floor when the force-refresh owner stops below it", async () => {
    await startSync();
    const eventSnapshot = deferred<RuntimeTimelineSnapshot>();
    const forceSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(
      eventSnapshot.promise,
      forceSnapshot.promise,
      Promise.resolve(snapshot(1, 2, null)),
    );

    const event = srv.onTimelineChanged?.(1, 2);
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(3);
    });
    const forced = forceRefresh();
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
    });

    eventSnapshot.resolve(snapshot(1, 2, null));
    forceSnapshot.resolve(snapshot(1, 1, null));
    await Promise.all([event, forced]);

    expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(5);
    expect(useProjectStore.getState().timelineVersion).toBe(2);
    expect(useEditorUiStore.getState().toast).toBeNull();
  });

  it("propagates the final event-owner failure to a superseded force refresh", async () => {
    await startSync();
    const forcedSnapshot = deferred<RuntimeTimelineSnapshot>();
    const firstEventAttempt = deferred<RuntimeTimelineSnapshot>();
    const finalEventAttempt = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(
      forcedSnapshot.promise,
      firstEventAttempt.promise,
      finalEventAttempt.promise,
    );

    const forced = forceRefresh();
    const forcedOutcome = forced.then(
      () => ({ error: null as Error | null }),
      (error: unknown) => ({
        error: error instanceof Error ? error : new Error(String(error)),
      }),
    );
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(3);
    });

    const event = srv.onTimelineChanged?.(1, 1);
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
    });
    forcedSnapshot.resolve(snapshot(1, 0, null));

    firstEventAttempt.reject(new Error("first event refresh failed"));
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(5);
    });
    finalEventAttempt.reject(new Error("final event refresh failed"));

    const [, outcome] = await Promise.all([event, forcedOutcome]);
    expect(outcome.error?.message).toBe("final event refresh failed");
    expect(useProjectStore.getState().timelineVersion).toBe(0);
  });

  it("continues project-open convergence when stopping old playback rejects", async () => {
    await startSync();
    srv.playbackResponses.push(Promise.reject(new Error("playback stop unavailable")));
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(2, 0, "/tmp/new-project.opentake")),
    );

    await srv.onProjectOpened?.("/tmp/new-project.opentake", 2, 0);

    expect(useProjectStore.getState().projectEpoch).toBe(2);
    expect(useEditorUiStore.getState().toast?.message).toContain("playback stop unavailable");
  });

  it("resets media at snapshot commit before later new-project errors can appear", async () => {
    await startSync();
    srv.resetMediaTransient.mockClear();
    srv.refreshMedia.mockClear();
    const pendingUndo = deferred<boolean>();
    const pendingRedo = deferred<boolean>();
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(2, 0, "/tmp/new-project.opentake")),
    );
    srv.undoResponses.push(pendingUndo.promise);
    srv.redoResponses.push(pendingRedo.promise);
    srv.mediaError = "old project error";

    const opened = srv.onProjectOpened?.("/tmp/new-project.opentake", 2, 0);
    await vi.waitFor(() => {
      expect(useProjectStore.getState().projectPath).toBe("/tmp/new-project.opentake");
    });
    expect(srv.resetMediaTransient).toHaveBeenCalledTimes(1);
    srv.mediaError = "new project error";

    pendingUndo.resolve(false);
    pendingRedo.resolve(false);
    await opened;

    expect(srv.mediaError).toBe("new project error");
    expect(srv.refreshMedia).toHaveBeenCalledTimes(1);
  });

  it("applies media boundary effects when a later refresh wins the project-open race", async () => {
    await startSync();
    srv.resetMediaTransient.mockClear();
    srv.refreshMedia.mockClear();
    const openedSnapshot = deferred<RuntimeTimelineSnapshot>();
    const winningSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(openedSnapshot.promise, winningSnapshot.promise);

    const opened = srv.onProjectOpened?.("/tmp/new-project.opentake", 2, 0);
    await vi.waitFor(() => expect(srv.invalidate).toHaveBeenCalled());
    const winningRefresh = forceRefresh();
    openedSnapshot.resolve(snapshot(2, 0, "/tmp/new-project.opentake"));
    winningSnapshot.resolve(snapshot(2, 0, "/tmp/new-project.opentake"));
    await Promise.all([opened, winningRefresh]);

    expect(useProjectStore.getState().projectPath).toBe("/tmp/new-project.opentake");
    expect(srv.resetMediaTransient).toHaveBeenCalledTimes(1);
    expect(srv.refreshMedia).toHaveBeenCalledTimes(1);
  });

  it("does not let a late old snapshot replace a newer project", async () => {
    const oldSnapshot = deferred<RuntimeTimelineSnapshot>();
    const newSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(oldSnapshot.promise, newSnapshot.promise);

    const oldRefresh = forceRefresh();
    const newRefresh = forceRefresh();
    newSnapshot.resolve(snapshot(2, 0, "/tmp/new.opentake", true));
    await newRefresh;
    oldSnapshot.resolve(snapshot(1, 7, "/tmp/old.opentake"));
    await oldRefresh;

    const state = useProjectStore.getState();
    expect(state.projectEpoch).toBe(2);
    expect(state.projectPath).toBe("/tmp/new.opentake");
    expect(state.compatibilityReadOnly).toBe(true);
  });

  it("refetches when an event-promised version is newer than the first snapshot", async () => {
    await startSync();
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(1, 1, null)),
      Promise.resolve(snapshot(1, 2, null)),
    );

    await srv.onTimelineChanged?.(1, 2);

    expect(useProjectStore.getState().timelineVersion).toBe(2);
    expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
  });

  it("coalesces N+1 and N+2 events into the highest observed floor", async () => {
    await startSync();
    const n1 = deferred<RuntimeTimelineSnapshot>();
    const n2 = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(n1.promise, n2.promise);

    const first = srv.onTimelineChanged?.(1, 1);
    const second = srv.onTimelineChanged?.(1, 2);
    n1.resolve(snapshot(1, 1, null));
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
    });
    n2.resolve(snapshot(1, 2, null));
    await Promise.all([first, second]);

    expect(useProjectStore.getState().timelineVersion).toBe(2);
  });

  it("hands a reused convergence caller the newer floor after the owner exhausts retries", async () => {
    await startSync();
    const firstAttempt = deferred<RuntimeTimelineSnapshot>();
    const secondAttempt = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(
      firstAttempt.promise,
      secondAttempt.promise,
      Promise.resolve(snapshot(1, 2, null)),
    );

    const n1 = srv.onTimelineChanged?.(1, 1);
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(3);
    });
    const n2 = srv.onTimelineChanged?.(1, 2);

    firstAttempt.reject(new Error("N+1 first attempt failed"));
    await vi.waitFor(() => {
      expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(4);
    });
    secondAttempt.reject(new Error("N+1 final attempt failed"));

    await Promise.all([n1, n2]);
    expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(5);
    expect(useProjectStore.getState().timelineVersion).toBe(2);
  });

  it("never publishes a stale snapshot when catch-up retries are exhausted", async () => {
    await startSync();
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(1, 1, null)),
      Promise.resolve(snapshot(1, 2, null)),
      Promise.resolve(snapshot(1, 2, null)),
    );

    await srv.onTimelineChanged?.(1, 3);

    expect(useProjectStore.getState().timelineVersion).toBe(0);
  });

  it("does not let late history results cross a project epoch", async () => {
    const oldUndo = deferred<boolean>();
    const oldRedo = deferred<boolean>();
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(1, 4, "/tmp/old.opentake")),
      Promise.resolve(snapshot(2, 0, "/tmp/new.opentake")),
    );
    srv.undoResponses.push(oldUndo.promise, Promise.resolve(false));
    srv.redoResponses.push(oldRedo.promise, Promise.resolve(false));

    const oldRefresh = forceRefresh();
    await vi.waitFor(() => {
      expect(srv.undoCalls).toBe(1);
      expect(srv.redoCalls).toBe(1);
    });
    const newRefresh = forceRefresh();
    await newRefresh;
    oldUndo.resolve(true);
    oldRedo.resolve(true);
    await oldRefresh;

    const state = useProjectStore.getState();
    expect(state.projectEpoch).toBe(2);
    expect(state.canUndo).toBe(false);
    expect(state.canRedo).toBe(false);
  });

  it("keeps a newer saved path when an older same-revision null-path snapshot arrives late", async () => {
    const beforeSave = deferred<RuntimeTimelineSnapshot>();
    const afterSave = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(beforeSave.promise, afterSave.promise);

    const oldRefresh = forceRefresh();
    const newRefresh = forceRefresh();
    afterSave.resolve(snapshot(3, 0, "/tmp/saved.opentake"));
    await newRefresh;
    beforeSave.resolve(snapshot(3, 0, null));
    await oldRefresh;

    expect(useProjectStore.getState().projectPath).toBe("/tmp/saved.opentake");
  });

  it("invalidates an in-flight snapshot when sync stops", async () => {
    useProjectStore.getState().replaceProjectSnapshot(snapshot(9, 2, "/tmp/current.opentake"));
    const staleSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(staleSnapshot.promise);

    const refresh = forceRefresh();
    stopSync();
    staleSnapshot.resolve(snapshot(8, 1, "/tmp/stale.opentake"));
    await refresh;

    expect(useProjectStore.getState().projectPath).toBe("/tmp/current.opentake");
  });

  it("does not register listeners after stop while initial refresh is pending", async () => {
    const initialSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(initialSnapshot.promise);

    const startup = startSync();
    stopSync();
    initialSnapshot.resolve(snapshot(1, 0, "/tmp/stale.opentake"));
    await startup;

    expect(srv.timelineListenerCalls).toBe(0);
    expect(srv.openedListenerCalls).toBe(0);
  });

  it("cleans both listeners when stop occurs during second registration", async () => {
    const firstUnsubscribe = vi.fn();
    const secondUnsubscribe = vi.fn();
    const openedRegistration = deferred<() => void>();
    srv.timelineListenerResponses.push(Promise.resolve(firstUnsubscribe));
    srv.openedListenerResponses.push(openedRegistration.promise);

    const startup = startSync();
    await vi.waitFor(() => expect(srv.openedListenerCalls).toBe(1));
    stopSync();
    openedRegistration.resolve(secondUnsubscribe);
    await startup;

    expect(firstUnsubscribe).toHaveBeenCalledTimes(1);
    expect(secondUnsubscribe).toHaveBeenCalledTimes(1);
  });

  it("registers exactly one listener pair after restarting a stopped pending startup", async () => {
    const staleSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(staleSnapshot.promise);

    const staleStartup = startSync();
    stopSync();
    await startSync();
    staleSnapshot.resolve(snapshot(1, 0, "/tmp/stale.opentake"));
    await staleStartup;

    expect(srv.timelineListenerCalls).toBe(1);
    expect(srv.openedListenerCalls).toBe(1);
  });

  it("invalidates an executing old project-opened callback across stop and restart", async () => {
    await startSync();
    const oldHandler = srv.projectOpenedHandlers[0]!;
    expect(oldHandler).toBeTypeOf("function");

    const oldPlayback = deferred<void>();
    srv.playbackResponses.push(oldPlayback.promise);
    const oldCallback = oldHandler("/tmp/old-event.opentake", 1, 0);
    await vi.waitFor(() => expect(srv.invalidate).toHaveBeenCalledTimes(1));

    stopSync();
    srv.projectPath = "/tmp/new.opentake";
    srv.compatibilityReadOnly = true;
    await startSync();
    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 88,
      activeFrame: 88,
      selectedClipIds: new Set(["new-selection"]),
    });
    const newHandler = srv.projectOpenedHandlers[1]!;
    expect(newHandler).toBeTypeOf("function");
    const refreshesBeforeResume = srv.order.filter((entry) => entry === "refresh").length;
    const undoCallsBeforeResume = srv.undoCalls;
    const redoCallsBeforeResume = srv.redoCalls;
    srv.snapshotResponses.push(Promise.resolve(snapshot(1, 0, "/tmp/old.opentake")));

    oldPlayback.resolve();
    await oldCallback;

    expect(srv.order.filter((entry) => entry === "refresh")).toHaveLength(
      refreshesBeforeResume,
    );
    expect(srv.undoCalls).toBe(undoCallsBeforeResume);
    expect(srv.redoCalls).toBe(redoCallsBeforeResume);
    expect(useProjectStore.getState().projectPath).toBe("/tmp/new.opentake");
    expect(useProjectStore.getState().compatibilityReadOnly).toBe(true);
    expect(useEditorUiStore.getState().isPlaying).toBe(true);
    expect(useEditorUiStore.getState().currentFrame).toBe(88);
    expect(useEditorUiStore.getState().selectedClipIds).toEqual(new Set(["new-selection"]));

    srv.snapshotResponses.length = 0;
    srv.projectPath = "/tmp/newer.opentake";
    srv.compatibilityReadOnly = false;
    await newHandler("/tmp/new-event.opentake", 1, 0);

    expect(useProjectStore.getState().projectPath).toBe("/tmp/newer.opentake");
    expect(useProjectStore.getState().compatibilityReadOnly).toBe(false);
    expect(useEditorUiStore.getState().isPlaying).toBe(false);
    expect(useEditorUiStore.getState().currentFrame).toBe(0);
    expect(useEditorUiStore.getState().selectedClipIds.size).toBe(0);
  });

  it("does not let a pending refresh overwrite a direct project replacement", async () => {
    const staleSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(staleSnapshot.promise);
    const refresh = forceRefresh();

    useProjectStore
      .getState()
      .replaceProjectSnapshot(snapshot(12, 0, "/tmp/direct.opentake", true));
    const undoCalls = srv.undoCalls;
    const redoCalls = srv.redoCalls;
    staleSnapshot.resolve(snapshot(11, 9, "/tmp/stale.opentake"));
    await refresh;

    const state = useProjectStore.getState();
    expect(state.projectEpoch).toBe(12);
    expect(state.projectPath).toBe("/tmp/direct.opentake");
    expect(state.compatibilityReadOnly).toBe(true);
    expect(srv.undoCalls).toBe(undoCalls);
    expect(srv.redoCalls).toBe(redoCalls);
  });

  it("does not let a pending refresh repopulate a cleared project", async () => {
    useProjectStore
      .getState()
      .replaceProjectSnapshot(snapshot(7, 3, "/tmp/active.opentake", true));
    const staleSnapshot = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(staleSnapshot.promise);
    const refresh = forceRefresh();

    useProjectStore.getState().clearProjectSnapshot();
    staleSnapshot.resolve(snapshot(7, 4, "/tmp/active.opentake", true));
    await refresh;

    const state = useProjectStore.getState();
    expect(state.projectEpoch).toBe(0);
    expect(state.projectPath).toBeNull();
    expect(state.timeline.tracks).toEqual([]);
    expect(state.compatibilityReadOnly).toBe(false);
    expect(srv.undoCalls).toBe(0);
    expect(srv.redoCalls).toBe(0);
  });

  it("preserves a direct saved path against an older same-revision response", async () => {
    useProjectStore.getState().replaceProjectSnapshot(snapshot(5, 0, null));
    const beforeSave = deferred<RuntimeTimelineSnapshot>();
    srv.snapshotResponses.push(beforeSave.promise);
    const staleRefresh = forceRefresh();

    useProjectStore.getState().setProjectPath("/tmp/saved.opentake");
    beforeSave.resolve(snapshot(5, 0, null));
    await staleRefresh;
    expect(useProjectStore.getState().projectPath).toBe("/tmp/saved.opentake");

    srv.projectPath = "/tmp/refreshed.opentake";
    srv.snapshotResponses.push(
      Promise.resolve(snapshot(5, 0, "/tmp/refreshed.opentake")),
    );
    await forceRefresh();
    expect(useProjectStore.getState().projectPath).toBe("/tmp/refreshed.opentake");
  });

  it("does not let pending history cross a same-identity compatibility replacement", async () => {
    const staleUndo = deferred<boolean>();
    const staleRedo = deferred<boolean>();
    srv.snapshotResponses.push(Promise.resolve(snapshot(6, 2, "/tmp/same.opentake")));
    srv.undoResponses.push(staleUndo.promise);
    srv.redoResponses.push(staleRedo.promise);
    const refresh = forceRefresh();
    await vi.waitFor(() => {
      expect(srv.undoCalls).toBe(1);
      expect(srv.redoCalls).toBe(1);
    });

    useProjectStore
      .getState()
      .replaceProjectSnapshot(snapshot(6, 2, "/tmp/same.opentake", true));
    staleUndo.resolve(true);
    staleRedo.resolve(true);
    await refresh;

    const state = useProjectStore.getState();
    expect(state.compatibilityReadOnly).toBe(true);
    expect(state.canUndo).toBe(false);
    expect(state.canRedo).toBe(false);
  });
});

describe("project_saved event sync", () => {
  it("registers the save listener during startup", async () => {
    srv.snapshotResponses.push(Promise.resolve(snapshot(1, 0, "/tmp/p.opentake")));
    await startSync();
    expect(srv.savedListenerCalls).toBe(1);
    expect(srv.order).toContain("saved-listen");
  });

  it("records the save completion for the current session without touching the dirty-state version", async () => {
    srv.snapshotResponses.push(Promise.resolve(snapshot(3, 2, "/tmp/p.opentake")));
    await startSync();
    // Simulate an un-saved document: nothing persisted yet, dirty version floor 0.
    useProjectStore.setState({ lastSavedAt: null, lastSavedVersion: 0 });

    srv.onProjectSaved?.("/tmp/p.opentake", 3);

    const state = useProjectStore.getState();
    expect(state.lastSavedAt).not.toBeNull();
    expect(state.lastSavedAt).toBeGreaterThan(0);
    // The event carries no document version, so the dirty-state floor is
    // deliberately left untouched.
    expect(state.lastSavedVersion).toBe(0);
  });

  it("ignores saves of a different session", async () => {
    srv.snapshotResponses.push(Promise.resolve(snapshot(3, 2, "/tmp/p.opentake")));
    await startSync();
    useProjectStore.setState({ lastSavedAt: null, lastSavedVersion: 0 });

    srv.onProjectSaved?.("/tmp/other.opentake", 4);

    expect(useProjectStore.getState().lastSavedAt).toBeNull();
    expect(useProjectStore.getState().lastSavedVersion).toBe(0);
  });

  it("does not record saves after sync has been stopped", async () => {
    srv.snapshotResponses.push(Promise.resolve(snapshot(3, 2, "/tmp/p.opentake")));
    await startSync();
    useProjectStore.setState({ lastSavedAt: null, lastSavedVersion: 0 });
    const handler = srv.onProjectSaved;
    expect(handler).not.toBeNull();

    stopSync();
    handler?.("/tmp/p.opentake", 3);

    expect(useProjectStore.getState().lastSavedAt).toBeNull();
  });
});
