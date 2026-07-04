/**
 * Read-only timeline mirror (SPEC §10.1). Updated ONLY by `timeline_changed` ->
 * `get_timeline`. The UI never mutates `timeline` directly — every edit is an
 * `edit_apply` command to Rust, whose event triggers a re-fetch.
 */

import { create } from "zustand";
import type { Timeline } from "../lib/types";

const EMPTY_TIMELINE: Timeline = {
  fps: 30,
  width: 1920,
  height: 1080,
  settingsConfigured: false,
  tracks: [],
};

interface ProjectState {
  timelineVersion: number;
  timeline: Timeline;
  projectPath: string | null;
  /** Document version last persisted to disk; `timelineVersion` ahead of this
   *  means there are unsaved edits (drives autosave / the dirty state). */
  lastSavedVersion: number;
  canUndo: boolean;
  canRedo: boolean;
  /** Replace the mirror (called by the sync layer after get_timeline). */
  setMirror: (timeline: Timeline, version: number) => void;
  setProjectPath: (path: string | null) => void;
  setHistory: (canUndo: boolean, canRedo: boolean) => void;
  /** Mark the current version as persisted (called after a successful save / on
   *  open, so a freshly opened project is not considered dirty). */
  markSaved: () => void;
}

export const useProjectStore = create<ProjectState>((set) => ({
  timelineVersion: 0,
  timeline: EMPTY_TIMELINE,
  projectPath: null,
  lastSavedVersion: 0,
  canUndo: false,
  canRedo: false,
  setMirror: (timeline, timelineVersion) => set({ timeline, timelineVersion }),
  setProjectPath: (projectPath) => set({ projectPath }),
  setHistory: (canUndo, canRedo) => set({ canUndo, canRedo }),
  markSaved: () => set((s) => ({ lastSavedVersion: s.timelineVersion })),
}));
