/**
 * Read-only timeline mirror (SPEC §10.1). Updated ONLY by `timeline_changed` ->
 * `get_timeline`. The UI never mutates `timeline` directly — every edit is an
 * `edit_apply` command to Rust, whose event triggers a re-fetch.
 */

import { create } from "zustand";
import type { RuntimeTimelineSnapshot, Timeline } from "../lib/types";

function deepFreeze<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}

function immutableTimeline(timeline: Timeline): Timeline {
  return deepFreeze(structuredClone(timeline));
}

const EMPTY_TIMELINE: Timeline = deepFreeze({
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: false,
  tracks: [],
});

interface ProjectState {
  /** Monotonic authority boundary for snapshot/path writes; never reset. */
  snapshotMutationRevision: number;
  projectEpoch: number;
  timelineVersion: number;
  timeline: Timeline;
  projectPath: string | null;
  compatibilityReadOnly: boolean;
  compatibilityBlockers: string[];
  /** Document version last persisted to disk; `timelineVersion` ahead of this
   *  means there are unsaved edits (drives autosave / the dirty state). */
  lastSavedVersion: number;
  /** Wall-clock time (ms) of the last completed bundle write — set by the
   *  explicit save promises and by the `project_saved` event (which also fires
   *  for core-internal saves like the media manifest). `null` until the first
   *  save of the current session. */
  lastSavedAt: number | null;
  canUndo: boolean;
  canRedo: boolean;
  /** Replace the whole authoritative snapshot. Same-project versions and
   * project epochs may only advance; accepted timelines are cloned + frozen. */
  replaceProjectSnapshot: (snapshot: RuntimeTimelineSnapshot) => void;
  clearProjectSnapshot: () => void;
  setProjectPath: (path: string | null) => void;
  setHistory: (canUndo: boolean, canRedo: boolean) => void;
  /** Mark the current version as persisted (called after a successful save / on
   *  open, so a freshly opened project is not considered dirty). */
  markSaved: (version?: number) => void;
  /** Record a bundle write observed via the `project_saved` event. The event
   *  carries no document version, so unlike `markSaved` this never touches the
   *  dirty-state version — only the last-saved timestamp. */
  recordSaveCompleted: () => void;
}

export const useProjectStore = create<ProjectState>((set) => ({
  snapshotMutationRevision: 0,
  projectEpoch: 0,
  timelineVersion: 0,
  timeline: EMPTY_TIMELINE,
  projectPath: null,
  compatibilityReadOnly: false,
  compatibilityBlockers: [],
  lastSavedVersion: 0,
  lastSavedAt: null,
  canUndo: false,
  canRedo: false,
  replaceProjectSnapshot: (snapshot) =>
    set((state) => {
      if (
        snapshot.projectEpoch < state.projectEpoch ||
        (snapshot.projectEpoch === state.projectEpoch && snapshot.version < state.timelineVersion)
      ) {
        return state;
      }
      const projectChanged = state.projectEpoch !== snapshot.projectEpoch;
      return {
        snapshotMutationRevision: state.snapshotMutationRevision + 1,
        projectEpoch: snapshot.projectEpoch,
        timelineVersion: snapshot.version,
        timeline: immutableTimeline(snapshot.timeline),
        projectPath: snapshot.projectPath,
        compatibilityReadOnly: snapshot.compatibilityReadOnly,
        compatibilityBlockers: snapshot.compatibilityBlockers,
        ...(projectChanged
          ? {
              lastSavedVersion: snapshot.version,
              lastSavedAt: null,
              canUndo: false,
              canRedo: false,
            }
          : {}),
      };
    }),
  clearProjectSnapshot: () =>
    set((state) => ({
      snapshotMutationRevision: state.snapshotMutationRevision + 1,
      projectEpoch: 0,
      timelineVersion: 0,
      timeline: EMPTY_TIMELINE,
      projectPath: null,
      compatibilityReadOnly: false,
      compatibilityBlockers: [],
      lastSavedVersion: 0,
      lastSavedAt: null,
      canUndo: false,
      canRedo: false,
    })),
  setProjectPath: (projectPath) =>
    set((state) => ({
      snapshotMutationRevision: state.snapshotMutationRevision + 1,
      projectPath,
    })),
  setHistory: (canUndo, canRedo) => set({ canUndo, canRedo }),
  markSaved: (version) =>
    set((state) => ({
      lastSavedVersion: version ?? state.timelineVersion,
      lastSavedAt: Date.now(),
    })),
  recordSaveCompleted: () => set({ lastSavedAt: Date.now() }),
}));
