import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeTimelineSnapshot, Timeline } from "../lib/types";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
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
    projectOpenedHandlers: [] as Array<
      (path: string, projectEpoch: number, version: number) => Promise<void> | void
    >,
    playbackResponses: [] as Array<Promise<void>>,
    order: [] as string[],
    onProjectOpened: null as null | ((path: string, projectEpoch: number, version: number) => Promise<void> | void),
    invalidate: vi.fn(async () => {
      srv.order.push("invalidate");
      const queued = srv.playbackResponses.shift();
      if (queued) await queued;
    }),
    resetMediaTransient: vi.fn(),
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
  onTimelineChanged: async () => {
    srv.timelineListenerCalls += 1;
    return (await srv.timelineListenerResponses.shift()) ?? (() => {});
  },
  onProjectOpened: async (
    handler: (path: string, projectEpoch: number, version: number) => Promise<void> | void,
  ) => {
    srv.openedListenerCalls += 1;
    srv.onProjectOpened = handler;
    srv.projectOpenedHandlers.push(handler);
    return (await srv.openedListenerResponses.shift()) ?? (() => {});
  },
}));

vi.mock("../components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.invalidate,
}));

vi.mock("./mediaStore", () => ({
  resetProjectMediaTransientState: srv.resetMediaTransient,
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
  srv.undoCalls = 0;
  srv.redoCalls = 0;
  srv.timelineListenerCalls = 0;
  srv.openedListenerCalls = 0;
  srv.projectOpenedHandlers.length = 0;
  srv.playbackResponses.length = 0;
  srv.resetMediaTransient.mockClear();
  srv.refreshMedia.mockClear();
});

afterEach(() => {
  stopSync();
  srv.order.length = 0;
  srv.invalidate.mockClear();
  srv.onProjectOpened = null;
  srv.projectPath = null;
  srv.compatibilityReadOnly = false;
});

describe("project event sync", () => {
  it("invalidates project scoped playback on externally initiated project_opened", async () => {
    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 77,
      activeFrame: 77,
      selectedClipIds: new Set(["old-clip"]),
      layoutPreset: "media",
    });
    await startSync();
    srv.order.length = 0;

    srv.projectPath = "/tmp/snapshot.opentake";
    await srv.onProjectOpened?.("/tmp/event-payload.opentake", 7, 0);

    expect(srv.order.slice(0, 2)).toEqual(["invalidate", "refresh"]);
    expect(useProjectStore.getState().projectEpoch).toBe(1);
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

    useEditorUiStore.setState({
      isPlaying: true,
      currentFrame: 88,
      activeFrame: 88,
      selectedClipIds: new Set(["new-selection"]),
    });
    const oldPlayback = deferred<void>();
    srv.playbackResponses.push(oldPlayback.promise);
    const oldCallback = oldHandler("/tmp/old-event.opentake", 1, 0);
    await vi.waitFor(() => expect(srv.invalidate).toHaveBeenCalledTimes(1));

    stopSync();
    srv.projectPath = "/tmp/new.opentake";
    srv.compatibilityReadOnly = true;
    await startSync();
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
