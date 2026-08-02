/**
 * Mirror sync (SPEC §11.2). Fetches the initial timeline, then on every
 * `timeline_changed{version}` re-fetches `get_timeline` if the version advanced
 * past the local mirror, and refreshes the undo/redo affordance flags.
 */

import * as api from "../lib/api";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";
import { stopNativePlaybackForProjectBoundary } from "../components/preview/nativePlaybackSession";
import { refreshMedia, resetProjectMediaState } from "./mediaStore";

let started = false;
let unlistenTimeline: (() => void) | null = null;
let unlistenOpened: (() => void) | null = null;
let refreshGeneration = 0;
let lifecycleGeneration = 0;
const MAX_SNAPSHOT_CATCHUP_ATTEMPTS = 3;

interface SnapshotFloor {
  projectEpoch: number;
  version: number;
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

async function refreshMirror(floor?: SnapshotFloor): Promise<void> {
  const generation = ++refreshGeneration;
  const mutationRevision = useProjectStore.getState().snapshotMutationRevision;
  let snap: Awaited<ReturnType<typeof api.getTimeline>> | null = null;
  for (let attempt = 0; attempt < MAX_SNAPSHOT_CATCHUP_ATTEMPTS; attempt += 1) {
    const candidate = await api.getTimeline();
    if (generation !== refreshGeneration) return;
    if (floor && !reachesFloor(candidate, floor)) continue;
    snap = candidate;
    break;
  }
  // An event promises that core has already reached `floor`. Never publish a
  // stale response if the transport cannot observe it within the bounded retry.
  if (!snap) return;
  const beforeCommit = useProjectStore.getState();
  if (beforeCommit.snapshotMutationRevision !== mutationRevision) return;
  beforeCommit.replaceProjectSnapshot(snap);
  const committed = useProjectStore.getState();
  const committedRevision = committed.snapshotMutationRevision;
  const projectChanged =
    beforeCommit.projectEpoch !== committed.projectEpoch ||
    beforeCommit.projectPath !== committed.projectPath;
  if (projectChanged) {
    resetProjectMediaState();
    useEditorUiStore.getState().resetProjectRuntimeState();
    await refreshMedia();
    if (generation !== refreshGeneration) return;
  }
  const [canUndo, canRedo] = await Promise.all([api.canUndo(), api.canRedo()]);
  if (generation !== refreshGeneration) return;
  const current = useProjectStore.getState();
  if (
    current.snapshotMutationRevision !== committedRevision ||
    current.projectEpoch !== snap.projectEpoch ||
    current.timelineVersion !== snap.version ||
    current.projectPath !== snap.projectPath
  ) {
    return;
  }
  useProjectStore.getState().setHistory(canUndo, canRedo);
}

/** Idempotent bootstrap; safe to call once on mount. */
export async function startSync(): Promise<void> {
  if (started) return;
  started = true;
  const generation = ++lifecycleGeneration;
  const lifecycleActive = () => started && generation === lifecycleGeneration;

  await refreshMirror();
  if (!lifecycleActive()) return;

  const timelineUnlisten = await api.onTimelineChanged(async (projectEpoch, version) => {
    if (!lifecycleActive()) return;
    const current = useProjectStore.getState();
    if (projectEpoch < current.projectEpoch) return;
    if (projectEpoch === current.projectEpoch && version <= current.timelineVersion) return;
    if (!lifecycleActive()) return;
    await refreshMirror({ projectEpoch, version });
    if (!lifecycleActive()) return;
  });
  if (!lifecycleActive()) {
    timelineUnlisten();
    return;
  }
  unlistenTimeline = timelineUnlisten;

  const openedUnlisten = await api.onProjectOpened(async (_path, projectEpoch, version) => {
    if (!lifecycleActive()) return;
    if (projectEpoch < useProjectStore.getState().projectEpoch) return;
    await stopNativePlaybackForProjectBoundary();
    if (!lifecycleActive()) return;
    await refreshMirror({ projectEpoch, version });
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
}

export function stopSync(): void {
  lifecycleGeneration += 1;
  refreshGeneration += 1;
  unlistenTimeline?.();
  unlistenOpened?.();
  unlistenTimeline = null;
  unlistenOpened = null;
  started = false;
}

/** Force a mirror refresh (e.g. after a fallback edit that emits no event). */
export async function forceRefresh(): Promise<void> {
  await refreshMirror();
}
