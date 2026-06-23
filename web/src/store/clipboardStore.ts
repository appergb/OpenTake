/**
 * Front-end clipboard store for copy/cut/paste (Issue #94). Holds snapshots of
 * the selected clips at copy time plus the source first-frame, so a paste can
 * re-place them relative to the current playhead without touching the original
 * clips. `linkGroupId` is cleared on paste so the backend re-assigns new
 * groups (mirrors upstream `pasteClipsAtPlayhead` link re-reflection).
 *
 * The store is UI-only: the authoritative timeline lives in Rust; this is just
 * a transient paste buffer, never persisted.
 */

import { create } from "zustand";
import type { Clip } from "../lib/types";

export interface ClipboardEntry {
  /** Deep snapshot of the clip at copy time. */
  clip: Clip;
  /** Track index the clip lived on when copied. Used to preserve track
   *  placement on paste (upstream behavior). */
  sourceTrackIndex: number;
}

interface ClipboardState {
  entries: ClipboardEntry[];
  /** The smallest `startFrame` among copied clips. Paste offsets every clip
   *  by `activeFrame - sourceFirstFrame` so the group lands at the playhead. */
  sourceFirstFrame: number;
  hasContent: boolean;
  set: (entries: ClipboardEntry[], sourceFirstFrame: number) => void;
  clear: () => void;
}

export const useClipboardStore = create<ClipboardState>((set) => ({
  entries: [],
  sourceFirstFrame: 0,
  hasContent: false,
  set: (entries, sourceFirstFrame) =>
    set({ entries, sourceFirstFrame, hasContent: entries.length > 0 }),
  clear: () => set({ entries: [], sourceFirstFrame: 0, hasContent: false }),
}));
