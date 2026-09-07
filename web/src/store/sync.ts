/**
 * Mirror sync (SPEC §11.2). Fetches the initial timeline, then on every
 * `timeline_changed{version}` re-fetches `get_timeline` if the version advanced
 * past the local mirror, and refreshes the undo/redo affordance flags.
 */

import * as api from "../lib/api";
import { totalFrames } from "../lib/geometry";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";
import { stopNativePlaybackForProjectBoundary } from "../components/preview/nativePlaybackSession";
import { refreshMedia, resetProjectMediaState } from "./mediaStore";

let started = false;
let unlistenTimeline: (() => void) | null = null;
let unlistenOpened: (() => void) | null = null;
let unlistenSaved: (() => void) | null = null;
let refreshGeneration = 0;
let lifecycleGeneration = 0;
const MAX_SNAPSHOT_CATCHUP_ATTEMPTS = 3;
const MAX_EVENT_REFRESH_ATTEMPTS = 2;

interface SnapshotFloor {
  projectEpoch: number;
  version: number;
}

type MirrorRefreshOutcome = "converged" | "superseded";

interface ObservedFloor {
  floor: SnapshotFloor;
  failureLabel: string;
  sequence: number;
}

interface LifecycleConvergence {
  generation: number;
  targetSequence: number;
  promise: Promise<void>;
}

interface MirrorRefreshOwner {
  generation: number;
  promise: Promise<MirrorRefreshOutcome>;
}

let highestObservedFloor: ObservedFloor | null = null;
let lifecycleConvergence: LifecycleConvergence | null = null;
let observedFloorSequence = 0;
let convergedFloorSequence = 0;
let mirrorRefreshOwner: MirrorRefreshOwner | null = null;

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function reportSyncFailure(label: string, error: unknown): void {
  useEditorUiStore
    .getState()
    .pushToast(`${label}: ${errorMessage(error)}`);
}

/** Keep the UI playhead inside the authoritative timeline after an edit or
 * undo/redo changes its duration. The scrubber intentionally permits the
 * duration boundary (not only the last drawable frame), so a parked end frame
 * remains representable while stale positions are never sent to preview. */
function reconcilePlayheadToTimeline(): void {
  const ui = useEditorUiStore.getState();
  const project = useProjectStore.getState();
  const timeline =
    project.timeline.nestedSequences?.find(
      (sequence) => sequence.id === ui.activeNestedSequenceId,
    )?.timeline ?? project.timeline;
  const maxFrame = Math.max(0, totalFrames(timeline));
  const frame = Math.max(
    0,
    Math.min(maxFrame, Math.round(Number.isFinite(ui.currentFrame) ? ui.currentFrame : 0)),
  );
  if (ui.currentFrame !== frame || ui.activeFrame !== frame) {
    ui.setCurrentFrame(frame);
  }
}

async function convergeEventRefresh(
  floor: SnapshotFloor,
  lifecycleActive: () => boolean,
  label: string,
): Promise<boolean> {
  let lastError: unknown;
  for (let attempt = 0; attempt < MAX_EVENT_REFRESH_ATTEMPTS; attempt += 1) {
    if (!lifecycleActive()) return false;
    try {
      const outcome = await refreshMirror(floor);
      if (outcome === "converged") return true;
      await waitForMirrorRefreshOwner();
      if (!lifecycleActive()) return false;
      if (mirrorReachesFloor(floor)) return true;
      lastError = new Error(
        "timeline mirror refresh was superseded before convergence",
      );
    } catch (error) {
      lastError = error;
    }
  }
  if (lifecycleActive()) reportSyncFailure(label, lastError);
  return false;
}

function reachesFloor(
  snapshot: Awaited<ReturnType<typeof api.getTimeline>>,
  floor: SnapshotFloor,
): boolean {
  return (
    snapshot.projectEpoch > floor.projectEpoch ||
    (snapshot.projectEpoch === floor.projectEpoch && snapshot.version >= floor.version)
  );
}

function mirrorReachesFloor(floor: SnapshotFloor): boolean {
  const current = useProjectStore.getState();
  return (
    current.projectEpoch > floor.projectEpoch ||
    (current.projectEpoch === floor.projectEpoch &&
      current.timelineVersion >= floor.version)
  );
}

function isNewerFloor(candidate: SnapshotFloor, current: SnapshotFloor): boolean {
  return (
    candidate.projectEpoch > current.projectEpoch ||
    (candidate.projectEpoch === current.projectEpoch &&
      candidate.version > current.version)
  );
}

function observeLifecycleFloor(
  generation: number,
  floor: SnapshotFloor,
  failureLabel: string,
): void {
  if (!started || generation !== lifecycleGeneration) return;
  observedFloorSequence += 1;
  const highestFloor =
    !highestObservedFloor || isNewerFloor(floor, highestObservedFloor.floor)
      ? floor
      : highestObservedFloor.floor;
  highestObservedFloor = {
    floor: highestFloor,
    failureLabel,
    sequence: observedFloorSequence,
  };
}

async function runMirrorRefresh(
  generation: number,
  floor?: SnapshotFloor,
): Promise<MirrorRefreshOutcome> {
  const mutationRevision = useProjectStore.getState().snapshotMutationRevision;
  let snap: Awaited<ReturnType<typeof api.getTimeline>> | null = null;
  for (let attempt = 0; attempt < MAX_SNAPSHOT_CATCHUP_ATTEMPTS; attempt += 1) {
    let candidate: Awaited<ReturnType<typeof api.getTimeline>>;
    try {
      candidate = await api.getTimeline();
    } catch (error) {
      if (generation !== refreshGeneration) return "superseded";
      throw error;
    }
    if (generation !== refreshGeneration) return "superseded";
    if (floor && !reachesFloor(candidate, floor)) continue;
    snap = candidate;
    break;
  }
  // An event promises that core has already reached `floor`. Never publish a
  // stale response if the transport cannot observe it within the bounded retry.
  if (!snap) {
    if (floor) {
      throw new Error(
        `timeline snapshot did not reach project ${floor.projectEpoch} version ${floor.version}`,
      );
    }
    return "superseded";
  }
  const beforeCommit = useProjectStore.getState();
  if (beforeCommit.snapshotMutationRevision !== mutationRevision) {
    return "superseded";
  }
  beforeCommit.replaceProjectSnapshot(snap);
  reconcilePlayheadToTimeline();
  const committed = useProjectStore.getState();
  const committedRevision = committed.snapshotMutationRevision;
  const projectChanged =
    beforeCommit.projectEpoch !== committed.projectEpoch ||
    beforeCommit.projectPath !== committed.projectPath;
  if (projectChanged) {
    resetProjectMediaState();
    useEditorUiStore.getState().resetProjectRuntimeState();
    try {
      await refreshMedia();
    } catch (error) {
      if (generation !== refreshGeneration) return "superseded";
      throw error;
    }
    if (generation !== refreshGeneration) return "superseded";
  }
  let canUndo: boolean;
  let canRedo: boolean;
  try {
    [canUndo, canRedo] = await Promise.all([api.canUndo(), api.canRedo()]);
  } catch (error) {
    if (generation !== refreshGeneration) return "superseded";
    throw error;
  }
  if (generation !== refreshGeneration) return "superseded";
  const current = useProjectStore.getState();
  if (
    current.snapshotMutationRevision !== committedRevision ||
    current.projectEpoch !== snap.projectEpoch ||
    current.timelineVersion !== snap.version ||
    current.projectPath !== snap.projectPath
  ) {
    return "superseded";
  }
  useProjectStore.getState().setHistory(canUndo, canRedo);
  return "converged";
}

function refreshMirror(floor?: SnapshotFloor): Promise<MirrorRefreshOutcome> {
  const generation = ++refreshGeneration;
  const promise = runMirrorRefresh(generation, floor);
  const owner = { generation, promise };
  mirrorRefreshOwner = owner;
  return promise;
}

async function waitForMirrorRefreshOwner(): Promise<MirrorRefreshOutcome | null> {
  while (mirrorRefreshOwner) {
    const owner = mirrorRefreshOwner;
    try {
      const outcome = await owner.promise;
      if (mirrorRefreshOwner === owner) return outcome;
    } catch (error) {
      // A superseded owner is no longer authoritative. Follow the newer owner,
      // but preserve the terminal owner's failure for every waiter.
      if (mirrorRefreshOwner === owner) throw error;
    }
  }
  return null;
}

function requestLifecycleConvergence(
  generation: number,
  lifecycleActive: () => boolean,
): Promise<void> {
  if (!lifecycleActive()) return Promise.resolve();
  if (lifecycleConvergence?.generation === generation) {
    const owner = lifecycleConvergence;
    const requestedSequence = highestObservedFloor?.sequence ?? 0;
    return owner.promise.then(() => {
      if (!lifecycleActive()) return;
      const latest = highestObservedFloor;
      if (
        requestedSequence > owner.targetSequence &&
        latest &&
        !mirrorReachesFloor(latest.floor)
      ) {
        return requestLifecycleConvergence(generation, lifecycleActive);
      }
    });
  }

  const targetSequence = highestObservedFloor?.sequence ?? 0;

  const run = async () => {
    while (lifecycleActive()) {
      const observed = highestObservedFloor;
      if (
        !observed ||
        (observed.sequence <= convergedFloorSequence &&
          mirrorReachesFloor(observed.floor))
      ) {
        return;
      }
      const converged = await convergeEventRefresh(
        observed.floor,
        lifecycleActive,
        observed.failureLabel,
      );
      if (!lifecycleActive()) return;
      if (!converged) return;
      convergedFloorSequence = Math.max(
        convergedFloorSequence,
        observed.sequence,
      );
      const latest = highestObservedFloor;
      if (
        !latest ||
        (latest.sequence <= convergedFloorSequence &&
          mirrorReachesFloor(latest.floor))
      ) {
        return;
      }
    }
  };

  let owner!: LifecycleConvergence;
  const promise = run().finally(() => {
    if (lifecycleConvergence === owner) lifecycleConvergence = null;
  });
  owner = { generation, targetSequence, promise };
  lifecycleConvergence = owner;
  return promise;
}

async function waitForLifecycleConvergence(
  generation: number,
  lifecycleActive: () => boolean,
): Promise<void> {
  while (lifecycleActive()) {
    const active = lifecycleConvergence;
    if (!active || active.generation !== generation) return;
    await active.promise;
  }
}

async function closeStartupGap(
  generation: number,
  lifecycleActive: () => boolean,
): Promise<void> {
  while (lifecycleActive()) {
    await waitForLifecycleConvergence(generation, lifecycleActive);
    if (!lifecycleActive()) return;

    const observed = highestObservedFloor;
    const outcome = await refreshMirror(observed?.floor);
    if (!lifecycleActive()) return;
    if (outcome === "superseded") {
      await waitForMirrorRefreshOwner();
      if (!lifecycleActive()) return;
      const active = lifecycleConvergence;
      if (active?.generation === generation) {
        await active.promise;
        continue;
      }
      if (
        observed &&
        (observed.sequence > convergedFloorSequence ||
          !mirrorReachesFloor(observed.floor))
      ) {
        await requestLifecycleConvergence(generation, lifecycleActive);
        continue;
      }
      return;
    }

    if (observed) {
      convergedFloorSequence = Math.max(
        convergedFloorSequence,
        observed.sequence,
      );
    }

    const latest = highestObservedFloor;
    if (
      !latest ||
      (latest.sequence <= convergedFloorSequence &&
        mirrorReachesFloor(latest.floor))
    ) {
      return;
    }
    await requestLifecycleConvergence(generation, lifecycleActive);
  }
}

/** Idempotent bootstrap; safe to call once on mount. */
export async function startSync(): Promise<void> {
  if (started) return;
  started = true;
  const generation = ++lifecycleGeneration;
  const lifecycleActive = () => started && generation === lifecycleGeneration;
  highestObservedFloor = null;
  lifecycleConvergence = null;
  observedFloorSequence = 0;
  convergedFloorSequence = 0;

  try {
    const initialOutcome = await refreshMirror();
    if (initialOutcome === "superseded") await waitForMirrorRefreshOwner();
    if (!lifecycleActive()) return;

    const timelineUnlisten = await api.onTimelineChanged((projectEpoch, version) => {
      if (!lifecycleActive()) return;
      const current = useProjectStore.getState();
      if (projectEpoch < current.projectEpoch) return;
      if (projectEpoch === current.projectEpoch && version <= current.timelineVersion) return;
      observeLifecycleFloor(
        generation,
        { projectEpoch, version },
        "时间线事件同步失败 / Timeline event sync failed",
      );
      return requestLifecycleConvergence(generation, lifecycleActive);
    });
    if (!lifecycleActive()) {
      timelineUnlisten();
      return;
    }
    unlistenTimeline = timelineUnlisten;

    const openedUnlisten = await api.onProjectOpened(async (_path, projectEpoch, version) => {
      if (!lifecycleActive()) return;
      if (projectEpoch < useProjectStore.getState().projectEpoch) return;
      try {
        await stopNativePlaybackForProjectBoundary();
      } catch (error) {
        if (lifecycleActive()) {
          reportSyncFailure(
            "停止旧项目预览失败 / Failed to stop previous project preview",
            error,
          );
        }
      }
      if (!lifecycleActive()) return;
      observeLifecycleFloor(
        generation,
        { projectEpoch, version },
        "项目切换同步失败 / Project transition sync failed",
      );
      await requestLifecycleConvergence(generation, lifecycleActive);
    });
    if (!lifecycleActive()) {
      openedUnlisten();
      if (unlistenTimeline === timelineUnlisten) {
        timelineUnlisten();
        unlistenTimeline = null;
      }
      return;
    }
    unlistenOpened = openedUnlisten;

    // `project_saved` fires on every bundle write, including core-internal
    // saves that never resolve the explicit save promises (e.g. the media
    // manifest). It carries no document version, so the mirror cannot advance
    // its dirty-state floor from it — record the completion timestamp for the
    // observed session instead.
    const savedUnlisten = await api.onProjectSaved((_path, projectEpoch) => {
      if (!lifecycleActive()) return;
      if (projectEpoch !== useProjectStore.getState().projectEpoch) return;
      useProjectStore.getState().recordSaveCompleted();
    });
    if (!lifecycleActive()) {
      savedUnlisten();
      if (unlistenTimeline === timelineUnlisten) {
        timelineUnlisten();
        unlistenTimeline = null;
      }
      if (unlistenOpened === openedUnlisten) {
        openedUnlisten();
        unlistenOpened = null;
      }
      return;
    }
    unlistenSaved = savedUnlisten;

    // Close the fetch-before-subscribe window. Any timeline/project event that
    // landed between the first refresh and listener registration is reflected by
    // this authoritative second snapshot even if that event itself was missed.
    await closeStartupGap(generation, lifecycleActive);
    if (!lifecycleActive()) return;
  } catch (error) {
    // A failed initial fetch or listener registration must leave bootstrap
    // retryable. Only the generation that still owns the lifecycle may tear
    // down globals; a stopped, stale startup must not cancel its replacement.
    if (generation === lifecycleGeneration) {
      lifecycleGeneration += 1;
      refreshGeneration += 1;
      unlistenTimeline?.();
      unlistenOpened?.();
      unlistenSaved?.();
      unlistenTimeline = null;
      unlistenOpened = null;
      unlistenSaved = null;
      highestObservedFloor = null;
      lifecycleConvergence = null;
      mirrorRefreshOwner = null;
      observedFloorSequence = 0;
      convergedFloorSequence = 0;
      started = false;
    }
    throw error;
  }
}

export function stopSync(): void {
  lifecycleGeneration += 1;
  refreshGeneration += 1;
  unlistenTimeline?.();
  unlistenOpened?.();
  unlistenSaved?.();
  unlistenTimeline = null;
  unlistenOpened = null;
  unlistenSaved = null;
  highestObservedFloor = null;
  lifecycleConvergence = null;
  mirrorRefreshOwner = null;
  observedFloorSequence = 0;
  convergedFloorSequence = 0;
  started = false;
}

/** Force a mirror refresh (e.g. after a fallback edit that emits no event). */
export async function forceRefresh(): Promise<void> {
  const outcome = await refreshMirror();
  if (outcome === "superseded") await waitForMirrorRefreshOwner();
}
