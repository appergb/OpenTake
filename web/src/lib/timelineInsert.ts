/**
 * Ripple-insert drop planning — pure helper. Builds the `InsertClips` payload
 * for a media item dropped onto the timeline while the ripple modifier is held
 * (upstream `TimelineView.performDragOperation`: `let ripple = mods.contains(.command)`
 * routes to `rippleInsertClips` instead of `addClips`). The backend `ripple_insert`
 * opens a gap at `atFrame` on the target track + every sync-locked track (and the
 * linked-audio track when a video clip carries audio), so this only needs to
 * resolve the target track + build one entry. See SPEC §5 / upstream
 * `EditorViewModel+Ripple.swift rippleInsertClips`.
 */

import type { ClipEntryReq, ClipType, MediaItem, Timeline, Transform } from "./types";

export interface InsertPlan {
  trackIndex: number;
  atFrame: number;
  entries: ClipEntryReq[];
}

/** Frame length a media item occupies (duplicated tiny rule from editActions to
 *  keep this module import-free of the store; stills get a default length). */
function durationFramesFor(item: MediaItem, fps: number, defaultImageSeconds: number): number {
  const seconds = item.duration > 0 ? item.duration : defaultImageSeconds;
  return Math.max(1, Math.round(seconds * fps));
}

function isVisual(type: ClipType): boolean {
  return type === "video" || type === "image" || type === "text" || type === "lottie";
}

/** First existing track whose kind matches the item, preferring `preferred`
 *  when compatible; null when the timeline has no compatible track. Ripple
 *  insert never creates the *target* track here (unlike overwrite drops) — a
 *  drop with no compatible track simply can't ripple-insert, so the caller
 *  falls back to a plain add. */
export function resolveInsertTrack(
  timeline: Timeline,
  type: ClipType,
  preferred: number | null,
): number | null {
  const wantAudio = type === "audio";
  const compatible = (i: number): boolean => {
    const t = timeline.tracks[i]?.type;
    if (!t) return false;
    return wantAudio ? t === "audio" : !(t === "audio") && isVisual(t);
  };
  if (preferred !== null && compatible(preferred)) return preferred;
  for (let i = 0; i < timeline.tracks.length; i++) if (compatible(i)) return i;
  return null;
}

/**
 * Build the ripple-insert plan for `item` dropped at `atFrame` over
 * `preferredTrackIndex`, or null when no compatible target track exists. The
 * entry mirrors the overwrite-drop entry (`entryForMediaAt`) except placement is
 * an insert: the backend pushes existing clips right by the duration.
 */
export function buildInsertPlan(
  timeline: Timeline,
  item: MediaItem,
  atFrame: number,
  preferredTrackIndex: number | null,
  fitTransform: (
    mw: number | null | undefined,
    mh: number | null | undefined,
    tw: number,
    th: number,
  ) => Transform,
  defaultImageSeconds: number,
): InsertPlan | null {
  const trackIndex = resolveInsertTrack(timeline, item.type, preferredTrackIndex);
  if (trackIndex === null) return null;
  const at = Math.max(0, atFrame);
  const durationFrames = durationFramesFor(item, timeline.fps, defaultImageSeconds);
  const entry: ClipEntryReq = {
    mediaRef: item.id,
    mediaType: item.type,
    sourceClipType: item.type,
    trackIndex,
    startFrame: at,
    durationFrames,
    hasAudio: item.hasAudio,
    addLinkedAudio: item.type === "video" && item.hasAudio,
    transform: fitTransform(item.width, item.height, timeline.width, timeline.height),
  };
  return { trackIndex, atFrame: at, entries: [entry] };
}
