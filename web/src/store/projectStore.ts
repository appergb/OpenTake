/**
 * Read-only timeline mirror (SPEC §10.1). Updated ONLY by `timeline_changed` ->
 * `get_timeline`. The UI never mutates `timeline` directly — every edit is an
 * `edit_apply` command to Rust, whose event triggers a re-fetch.
 */

import { create } from "zustand";
import type { RuntimeTimelineSnapshot, Timeline } from "../lib/types";

const EMPTY_TIMELINE: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: false,
  tracks: [],
};

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
  canUndo: boolean;
  canRedo: boolean;
  /** Replace the mirror (called by the sync layer after get_timeline). */
  setMirror: (timeline: Timeline, version: number, projectEpoch?: number) => void;
  replaceProjectSnapshot: (snapshot: RuntimeTimelineSnapshot) => void;
  clearProjectSnapshot: () => void;
  setProjectPath: (path: string | null) => void;
  setHistory: (canUndo: boolean, canRedo: boolean) => void;
  /** Mark the current version as persisted (called after a successful save / on
   *  open, so a freshly opened project is not considered dirty). */
  markSaved: (version?: number) => void;
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
  canUndo: false,
  canRedo: false,
  setMirror: (timeline, timelineVersion, projectEpoch) =>
    set((state) => ({
      snapshotMutationRevision: state.snapshotMutationRevision + 1,
      timeline,
      timelineVersion,
      projectEpoch: projectEpoch ?? state.projectEpoch,
    })),
  replaceProjectSnapshot: (snapshot) =>
    set((state) => {
      const projectChanged = state.projectEpoch !== snapshot.projectEpoch;
      return {
        snapshotMutationRevision: state.snapshotMutationRevision + 1,
        projectEpoch: snapshot.projectEpoch,
        timelineVersion: snapshot.version,
        timeline: snapshot.timeline,
        projectPath: snapshot.projectPath,
        compatibilityReadOnly: snapshot.compatibilityReadOnly,
        compatibilityBlockers: snapshot.compatibilityBlockers,
        ...(projectChanged
          ? {
              lastSavedVersion: snapshot.version,
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
    set((state) => ({ lastSavedVersion: version ?? state.timelineVersion })),
}));
