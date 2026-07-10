/**
 * Mirror sync (SPEC §11.2). Fetches the initial timeline, then on every
 * `timeline_changed{version}` re-fetches `get_timeline` if the version advanced
 * past the local mirror, and refreshes the undo/redo affordance flags.
 */

import * as api from "../lib/api";
import { useProjectStore } from "./projectStore";
import { stopNativePlaybackForProjectBoundary } from "../components/preview/nativePlaybackSession";

let started = false;
let unlistenTimeline: (() => void) | null = null;
let unlistenOpened: (() => void) | null = null;

async function refreshMirror(): Promise<void> {
  const snap = await api.getTimeline();
  useProjectStore.getState().setMirror(snap.timeline, snap.version, snap.projectEpoch);
  const [canUndo, canRedo] = await Promise.all([api.canUndo(), api.canRedo()]);
  useProjectStore.getState().setHistory(canUndo, canRedo);
}

/** Idempotent bootstrap; safe to call once on mount. */
export async function startSync(): Promise<void> {
  if (started) return;
  started = true;

  await refreshMirror();

  unlistenTimeline = await api.onTimelineChanged(async (projectEpoch, version) => {
    const current = useProjectStore.getState();
    if (projectEpoch !== current.projectEpoch || version > current.timelineVersion) {
      await refreshMirror();
    }
  });
  unlistenOpened = await api.onProjectOpened(async (path) => {
    await stopNativePlaybackForProjectBoundary();
    useProjectStore.getState().setProjectPath(path || null);
    await refreshMirror();
  });
}

export function stopSync(): void {
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
