/**
 * Mirror sync (SPEC §11.2). Fetches the initial timeline, then on every
 * `timeline_changed{version}` re-fetches `get_timeline` if the version advanced
 * past the local mirror, and refreshes the undo/redo affordance flags.
 */

import * as api from "../lib/api";
import { useProjectStore } from "./projectStore";
import { useEditorUiStore } from "./uiStore";
import { stopNativePlaybackForProjectBoundary } from "../components/preview/nativePlaybackSession";

let started = false;
let unlistenTimeline: (() => void) | null = null;
let unlistenOpened: (() => void) | null = null;
let refreshGeneration = 0;
let lifecycleGeneration = 0;

async function refreshMirror(): Promise<void> {
  const generation = ++refreshGeneration;
  const snap = await api.getTimeline();
  if (generation !== refreshGeneration) return;
  useProjectStore.getState().replaceProjectSnapshot(snap);
  const [canUndo, canRedo] = await Promise.all([api.canUndo(), api.canRedo()]);
  if (generation !== refreshGeneration) return;
  const current = useProjectStore.getState();
  if (
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
    if (projectEpoch !== current.projectEpoch || version > current.timelineVersion) {
      if (!lifecycleActive()) return;
      await refreshMirror();
      if (!lifecycleActive()) return;
    }
  });
  if (!lifecycleActive()) {
    timelineUnlisten();
    return;
  }
  unlistenTimeline = timelineUnlisten;

  const openedUnlisten = await api.onProjectOpened(async () => {
    if (!lifecycleActive()) return;
    await stopNativePlaybackForProjectBoundary();
    if (!lifecycleActive()) return;
    await refreshMirror();
    if (!lifecycleActive()) return;
    useEditorUiStore.getState().resetProjectRuntimeState();
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
