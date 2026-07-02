/**
 * Gesture -> EditCommand mapping (SPEC §11.1). Every editing action funnels
 * through `editApply`; after a successful change we force a mirror refresh so
 * the browser fallback (which emits no event) and Tauri behave identically.
 */

import * as api from "../lib/api";
import { isTauri } from "../lib/api";
import { forceRefresh } from "./sync";
import { useEditorUiStore } from "./uiStore";
import { useProjectStore } from "./projectStore";
import { fitTransformForMedia, trimToPlayheadEdits } from "../lib/clip";
import type { TrackDropTarget } from "../lib/geometry";
import { useClipboardStore } from "./clipboardStore";
import type {
  Clip,
  ClipEntryReq,
  ClipMoveReq,
  ClipPropertiesReq,
  ClipType,
  ChromaKeyInput,
  ColorGradeInput,
  Crop,
  EffectInput,
  EditRequest,
  FrameRangeReq,
  Interpolation,
  KeyframePayloadReq,
  KeyframeProperty,
  MaskInput,
  MediaItem,
  RenameEntryReq,
  TextEntryReq,
  TextStyle,
  Timeline,
  Transform,
  TrimEditReq,
} from "../lib/types";

async function applyAndRefresh(cmd: Parameters<typeof api.editApply>[0]) {
  const res = await api.editApply(cmd);
  // Tauri pushes timeline_changed -> sync re-fetches. The browser fallback has
  // no event channel, so refresh explicitly there.
  if (!isTauri && res.changed) await forceRefresh();
  return res;
}

export async function addClips(entries: ClipEntryReq[]) {
  if (entries.length === 0) return;
  return applyAndRefresh({ type: "addClips", entries });
}

export async function moveClips(moves: ClipMoveReq[]) {
  if (moves.length === 0) return;
  await applyAndRefresh({ type: "moveClips", moves });
}

/** Swap the positions of two clips (the cross-track "exchange places" gesture)
 *  so neither overwrites the other. The backend refuses (no change) if the swap
 *  would overlap a third clip, keeping it lossless. */
export async function swapClips(clipA: string, clipB: string) {
  if (clipA === clipB) return;
  await applyAndRefresh({ type: "swapClips", clipA, clipB });
}

/** Option/Alt-drag duplicate: deep-copy each clip to a new position. The
 *  backend clones every field (keyframe tracks / grade / masks / effects /
 *  text / transform / crop / fades), mints a fresh id, shifts `startFrame` by
 *  `offsetFrames`, lands on `targetTrackIndexes[i]`, and clears the link group
 *  (a copy is not linked to the original's partners). Returns the new clip ids
 *  via the EditResult so the caller can select them. */
export async function duplicateClips(
  clipIds: string[],
  offsetFrames: number,
  targetTrackIndexes: number[],
) {
  if (clipIds.length === 0) return;
  return applyAndRefresh({
    type: "duplicateClips",
    clipIds,
    offsetFrames,
    targetTrackIndexes,
  });
}

export async function removeClips(clipIds: string[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "removeClips", clipIds });
}

export async function splitClip(clipId: string, atFrame: number) {
  await applyAndRefresh({ type: "splitClip", clipId, atFrame });
}

export async function trimClips(edits: TrimEditReq[]) {
  if (edits.length === 0) return;
  await applyAndRefresh({ type: "trimClips", edits });
}

export async function setClipProperties(clipIds: string[], properties: ClipPropertiesReq) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setClipProperties", clipIds, properties });
}

export async function setColorGrade(clipIds: string[], grade: ColorGradeInput | null) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setColorGrade", clipIds, grade });
}

export async function setChromaKey(clipIds: string[], chromaKey: ChromaKeyInput | null) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setChromaKey", clipIds, chromaKey });
}

export async function setMasks(clipIds: string[], masks: MaskInput[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setMasks", clipIds, masks });
}

export async function setEffects(clipIds: string[], effects: EffectInput[]) {
  if (clipIds.length === 0) return;
  await applyAndRefresh({ type: "setEffects", clipIds, effects });
}

export async function linkClips(clipIds: string[]) {
  await applyAndRefresh({ type: "link", clipIds });
}

/** Insert a new empty track of `kind` (clamped into its zone by the core). Used
 *  by the drop flow to create a track on demand when the timeline has none
 *  compatible. */
export async function insertTrack(kind: ClipType, at?: number) {
  return applyAndRefresh({ type: "insertTrack", kind, at });
}

export async function unlinkClips(clipIds: string[]) {
  await applyAndRefresh({ type: "unlink", clipIds });
}

/** Toggle a track head's mute / hide / sync-lock. Omitted fields are unchanged. */
export async function setTrackProps(
  trackIndex: number,
  props: { muted?: boolean; hidden?: boolean; syncLocked?: boolean },
) {
  await applyAndRefresh({ type: "setTrackProps", trackIndex, ...props });
}

/** Swap two same-kind tracks as whole rows; cross-kind swaps are a no-op in core. */
export async function swapTracks(a: number, b: number) {
  if (a === b) return;
  await applyAndRefresh({ type: "swapTracks", a, b });
}

/** Replace (or clear) a clip's keyframe track for one property. */
export async function setKeyframes(
  clipId: string,
  property: KeyframeProperty,
  payload: KeyframePayloadReq,
) {
  await applyAndRefresh({ type: "setKeyframes", clipId, property, payload });
}

/** Stamp a keyframe at `frame` using the clip's current sampled value. */
export async function stampKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
) {
  await applyAndRefresh({ type: "stampKeyframe", clipId, property, frame });
}

/** Remove the keyframe at `frame`. */
export async function removeKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
) {
  await applyAndRefresh({ type: "removeKeyframe", clipId, property, frame });
}

/** Move a keyframe from `fromFrame` to `toFrame`. */
export async function moveKeyframe(
  clipId: string,
  property: KeyframeProperty,
  fromFrame: number,
  toFrame: number,
) {
  await applyAndRefresh({ type: "moveKeyframe", clipId, property, fromFrame, toFrame });
}

/** Change the interpolation mode of the keyframe at `frame`. */
export async function setKeyframeInterpolation(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
  interpolation: Interpolation,
) {
  await applyAndRefresh({ type: "setKeyframeInterpolation", clipId, property, frame, interpolation });
}

/** Ripple-delete project-frame ranges on a track, closing the gaps. */
export async function rippleDeleteRanges(trackIndex: number, ranges: FrameRangeReq[]) {
  if (ranges.length === 0) return;
  await applyAndRefresh({ type: "rippleDeleteRanges", trackIndex, ranges });
}

/** Create a media-library folder (optionally nested under `parentFolderId`). */
export async function createFolder(name: string, parentFolderId?: string) {
  await applyAndRefresh({ type: "createFolder", name, parentFolderId });
}

/** Move media assets into a folder (or to root with no `folderId`). */
export async function moveToFolder(assetIds: string[], folderId?: string) {
  if (assetIds.length === 0) return;
  await applyAndRefresh({ type: "moveToFolder", assetIds, folderId });
}

export async function renameMedia(entries: RenameEntryReq[]) {
  if (entries.length === 0) return;
  await applyAndRefresh({ type: "renameMedia", entries });
}

export async function renameFolder(entries: RenameEntryReq[]) {
  if (entries.length === 0) return;
  await applyAndRefresh({ type: "renameFolder", entries });
}

export async function deleteMedia(assetIds: string[]) {
  if (assetIds.length === 0) return;
  await applyAndRefresh({ type: "deleteMedia", assetIds });
}

export async function deleteFolder(folderIds: string[]) {
  if (folderIds.length === 0) return;
  await applyAndRefresh({ type: "deleteFolder", folderIds });
}

/** Replace a clip's media source in place, preserving all editing attributes.
 *  The backend intentionally consumes only `clipId` + `mediaRef`; it does not
 *  rewrite trim, duration, or type metadata. */
export async function swapMedia(clipId: string, mediaRef: string) {
  await applyAndRefresh({ type: "swapMedia", clipId, mediaRef });
}

export async function applyAutomationCommands(commands: EditRequest[]) {
  if (commands.length === 0) return [];
  const results = [];
  for (const command of commands) {
    results.push(await applyAndRefresh(command));
  }
  return results;
}

export async function applySmartReframe(clipIds: string[], crop: Crop, transform?: Transform) {
  if (clipIds.length === 0) return;
  await setClipProperties(clipIds, { crop, transform });
}

export async function addClipsToBeatFrames(entries: ClipEntryReq[], beatFrames: number[]) {
  if (entries.length === 0) return;
  const placed = entries.map((entry, index) => ({
    ...entry,
    startFrame: beatFrames[index] ?? entry.startFrame,
  }));
  await addClips(placed);
}

export async function tightenSilenceRanges(trackIndex: number, ranges: FrameRangeReq[]) {
  await rippleDeleteRanges(trackIndex, ranges);
}

export async function undo() {
  await api.undo();
  if (!isTauri) await forceRefresh();
}

export async function redo() {
  await api.redo();
  if (!isTauri) await forceRefresh();
}

/** Split at the current playhead (Toolbar / ⌘K). Splits the SELECTED clips the
 *  playhead intersects; if nothing is selected, splits every clip under the
 *  playhead (so split works without first selecting — matches editor norms).
 *  A clip the playhead doesn't intersect is a no-op in the core. */
export async function splitAtPlayhead() {
  const ui = useEditorUiStore.getState();
  const frame = Math.round(ui.activeFrame);
  const selected = [...ui.selectedClipIds];
  let ids = selected;
  if (ids.length === 0) {
    // No selection: target every clip the playhead currently intersects.
    const timeline = useProjectStore.getState().timeline;
    ids = [];
    for (const track of timeline.tracks) {
      for (const c of track.clips) {
        if (frame > c.startFrame && frame < c.startFrame + c.durationFrames) ids.push(c.id);
      }
    }
  }
  for (const id of ids) {
    await splitClip(id, frame);
  }
}

/** Clips the playhead is strictly inside, restricted to the selection when one
 *  exists (else all clips under the playhead) — the target set for trim-to-
 *  playhead, matching `splitAtPlayhead`'s "act on what's under the playhead". */
function clipsUnderPlayhead(): Clip[] {
  const ui = useEditorUiStore.getState();
  const frame = Math.round(ui.activeFrame);
  const selected = new Set(ui.selectedClipIds);
  const restrict = selected.size > 0;
  const out: Clip[] = [];
  for (const track of useProjectStore.getState().timeline.tracks) {
    for (const c of track.clips) {
      if (frame <= c.startFrame || frame >= c.startFrame + c.durationFrames) continue;
      if (restrict && !selected.has(c.id)) continue;
      out.push(c);
    }
  }
  return out;
}

/** Trim each target clip's IN point to the playhead (Q / Toolbar `[` — 剪映
 *  "删除播放头左侧"). The right edge stays put; the left part is removed. */
export async function trimStartToPlayhead() {
  const frame = Math.round(useEditorUiStore.getState().activeFrame);
  await trimClips(trimToPlayheadEdits(clipsUnderPlayhead(), frame, "left"));
}

/** Trim each target clip's OUT point to the playhead (W / Toolbar `]` — 剪映
 *  "删除播放头右侧"). The left edge stays put; the right part is removed. */
export async function trimEndToPlayhead() {
  const frame = Math.round(useEditorUiStore.getState().activeFrame);
  await trimClips(trimToPlayheadEdits(clipsUnderPlayhead(), frame, "right"));
}

/** The subset of `selected` that still exists as a clip in the current timeline.
 *  A stale id (a clip already removed/replaced/split by a prior edit, left behind
 *  in the selection set) makes the core's RemoveClips/RippleDelete reject the
 *  WHOLE batch — so one orphan silently blocks deletion of everything. Filtering
 *  to live ids first is what makes ⌫ reliably delete. */
function liveSelectedClipIds(): string[] {
  const live = new Set<string>();
  for (const track of useProjectStore.getState().timeline.tracks) {
    for (const clip of track.clips) live.add(clip.id);
  }
  return [...useEditorUiStore.getState().selectedClipIds].filter((id) => live.has(id));
}

/** Delete selected clips (⌫ / menu). Wrapped in try/catch so that even if the
 *  backend RemoveClips rejects (IPC error, an edge the live-id filter missed),
 *  the selection is still cleared and the failure is surfaced as a toast instead
 *  of silently doing nothing (the reported "delete does nothing"). */
export async function deleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = liveSelectedClipIds();
  if (ids.length > 0) {
    try {
      await removeClips(ids);
      // Tauri normally refreshes via the timeline_changed event; force it too so
      // a missed/raced event can't leave the just-deleted clip painted on screen.
      if (isTauri) await forceRefresh();
    } catch (err) {
      ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  ui.clearSelection();
}

/** Ripple-delete selected clips (⇧⌫): remove and close the gaps, shifting
 *  sync-locked followers (the core refuses if a follower would collide). */
export async function rippleDeleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = liveSelectedClipIds();
  if (ids.length > 0) {
    try {
      await applyAndRefresh({ type: "rippleDeleteClips", clipIds: ids });
      if (isTauri) await forceRefresh();
    } catch (err) {
      ui.pushToast(`删除失败 / Delete failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  ui.clearSelection();
}

// MARK: - Media -> timeline (drag and drop)

/** Stills get a fixed default duration (upstream `Constants.defaultImageDuration`
 *  ≈ 5s) since they have no intrinsic length. */
const DEFAULT_IMAGE_SECONDS = 5;

/** Frame length a media item occupies when placed on the timeline: its source
 *  duration (seconds → frames), or the still-image default when it has none.
 *  Shared by the clip-entry builders and the drop-ghost preview so the ghost
 *  width matches the clip that actually lands. */
export function mediaDurationFrames(item: MediaItem, fps: number): number {
  const seconds = item.duration > 0 ? item.duration : DEFAULT_IMAGE_SECONDS;
  return Math.max(1, Math.round(seconds * fps));
}

function isVisual(type: MediaItem["type"]): boolean {
  return type === "video" || type === "image" || type === "text" || type === "lottie";
}

/** First existing track whose kind is compatible with `type`, else null. When
 *  none exists, the drop flow ([`addMediaToTimeline`]) creates one on demand
 *  (`insertTrack`) — mirroring upstream `placeClip` auto-track-creation — so a
 *  drop onto an empty timeline still produces a clip. */
function firstCompatibleTrackIndex(timeline: Timeline, type: MediaItem["type"]): number | null {
  const wantAudio = type === "audio";
  for (let i = 0; i < timeline.tracks.length; i++) {
    const trackIsAudio = timeline.tracks[i].type === "audio";
    if (wantAudio ? trackIsAudio : !trackIsAudio && isVisual(timeline.tracks[i].type)) {
      return i;
    }
  }
  return null;
}

function trackIsCompatible(timeline: Timeline, trackIndex: number, type: MediaItem["type"]): boolean {
  const track = timeline.tracks[trackIndex];
  if (!track) return false;
  const wantAudio = type === "audio";
  const trackIsAudio = track.type === "audio";
  return wantAudio ? trackIsAudio : !trackIsAudio && isVisual(track.type);
}

/** Append position on a track: just past its last clip (clamped to >= 0). */
function appendStartFrame(timeline: Timeline, trackIndex: number): number {
  return timeline.tracks[trackIndex].clips.reduce(
    (max, c) => Math.max(max, c.startFrame + c.durationFrames),
    0,
  );
}

function trackOverlaps(timeline: Timeline, trackIndex: number, startFrame: number, durationFrames: number): boolean {
  const endFrame = startFrame + durationFrames;
  return timeline.tracks[trackIndex].clips.some((c) => c.startFrame < endFrame && c.startFrame + c.durationFrames > startFrame);
}

export function firstOpenCompatibleTrackIndex(
  timeline: Timeline,
  type: MediaItem["type"],
  startFrame: number,
  durationFrames: number,
  preferredTrackIndex: number | null,
): number | null {
  const candidates: number[] = [];
  if (preferredTrackIndex !== null && trackIsCompatible(timeline, preferredTrackIndex, type)) {
    candidates.push(preferredTrackIndex);
  }
  for (let i = 0; i < timeline.tracks.length; i++) {
    if (i !== preferredTrackIndex && trackIsCompatible(timeline, i, type)) candidates.push(i);
  }
  for (const trackIndex of candidates) {
    if (!trackOverlaps(timeline, trackIndex, startFrame, durationFrames)) return trackIndex;
  }
  return null;
}

/** Build the clip entry for a media item dropped on the timeline, or null when
 *  no compatible track exists. */
function entryForMedia(timeline: Timeline, item: MediaItem): ClipEntryReq | null {
  const trackIndex = firstCompatibleTrackIndex(timeline, item.type);
  if (trackIndex === null) return null;
  const durationFrames = mediaDurationFrames(item, timeline.fps);
  return {
    mediaRef: item.id,
    mediaType: item.type,
    sourceClipType: item.type,
    trackIndex,
    startFrame: appendStartFrame(timeline, trackIndex),
    durationFrames,
    hasAudio: item.hasAudio,
    addLinkedAudio: item.type === "video" && item.hasAudio,
    transform: fitTransformForMedia(item.width, item.height, timeline.width, timeline.height),
  };
}

function entryForMediaAt(
  timeline: Timeline,
  item: MediaItem,
  startFrame: number,
  preferredTrackIndex: number | null,
): ClipEntryReq | null {
  const durationFrames = mediaDurationFrames(item, timeline.fps);
  const trackIndex = firstOpenCompatibleTrackIndex(
    timeline,
    item.type,
    startFrame,
    durationFrames,
    preferredTrackIndex,
  );
  if (trackIndex === null) return null;
  return {
    mediaRef: item.id,
    mediaType: item.type,
    sourceClipType: item.type,
    trackIndex,
    startFrame: Math.max(0, startFrame),
    durationFrames,
    hasAudio: item.hasAudio,
    addLinkedAudio: item.type === "video" && item.hasAudio,
    transform: fitTransformForMedia(item.width, item.height, timeline.width, timeline.height),
  };
}

/** Where a media item dropped at `startFrame` over `dropTarget` will actually
 *  land — the truthful target for the drop-ghost preview. Pure mirror of
 *  [`addMediaToTimelineAtInner`]'s resolution: an insert-zone hover makes a new
 *  track; an over-a-track hover lands on the first open compatible track (the
 *  hovered one when free), and falls back to a fresh track when none is open. */
export function resolveMediaDropTrack(
  timeline: Timeline,
  item: MediaItem,
  startFrame: number,
  dropTarget: TrackDropTarget,
): { trackIndex: number | null; newTrack: { index: number; type: ClipType } | null } {
  const newType: ClipType = item.type === "audio" ? "audio" : "video";
  if (dropTarget.kind === "newTrack") {
    return { trackIndex: null, newTrack: { index: dropTarget.index, type: newType } };
  }
  const durationFrames = mediaDurationFrames(item, timeline.fps);
  const landed = firstOpenCompatibleTrackIndex(
    timeline,
    item.type,
    Math.max(0, startFrame),
    durationFrames,
    dropTarget.trackIndex,
  );
  if (landed !== null) return { trackIndex: landed, newTrack: null };
  // No open compatible track under/near the hover: the drop inserts a fresh one
  // at the hovered index (fallback in `addMediaToTimelineAtInner`).
  return { trackIndex: null, newTrack: { index: dropTarget.trackIndex, type: newType } };
}

/** Serialized tail for media -> timeline adds. Both call sites fire-and-forget
 *  (`void addMediaToTimeline(...)`), so this chains adds to keep them from
 *  racing on the shared mirror. See [`addMediaToTimeline`]. */
let mediaAddQueue: Promise<void> = Promise.resolve();

function enqueueMediaAdd(run: () => Promise<void>): Promise<void> {
  const result = mediaAddQueue.then(run, run);
  // Keep the queue alive even if an individual add rejects.
  mediaAddQueue = result.catch(() => {});
  return result;
}

/** Add a media-library item to the timeline (drag-drop / double-click from the
 *  media panel). Resolves the target track and append position from the current
 *  mirror; if the timeline has no compatible track (e.g. a brand-new empty
 *  project), creates one on demand first (upstream `placeClip` auto-creates),
 *  then places the clip.
 *
 *  Adds are **serialized**: a rapid second drop/double-click would otherwise
 *  start while the first is still in flight, read a stale (clip-less) mirror,
 *  compute `startFrame` 0 again, and have the core's overwrite-on-place drop the
 *  first clip. The queue makes each add observe the previous one's result. */
export function addMediaToTimeline(item: MediaItem): Promise<void> {
  return enqueueMediaAdd(() => addMediaToTimelineInner(item));
}

export function addMediaToTimelineAt(
  item: MediaItem,
  startFrame: number,
  preferredTrackIndex: number | null,
  insertTrackAt?: number,
): Promise<void> {
  return enqueueMediaAdd(() => addMediaToTimelineAtInner(item, startFrame, preferredTrackIndex, insertTrackAt));
}

async function addMediaToTimelineInner(item: MediaItem): Promise<void> {
  let timeline = useProjectStore.getState().timeline;
  if (firstCompatibleTrackIndex(timeline, item.type) === null) {
    await insertTrack(item.type === "audio" ? "audio" : "video");
    // Ensure the mirror reflects the new track before computing the entry
    // (Tauri's timeline_changed refresh is async; force it synchronously here).
    await forceRefresh();
    timeline = useProjectStore.getState().timeline;
  }
  const entry = entryForMedia(timeline, item);
  if (!entry) return;
  await addClips([entry]);
  // Tauri refreshes the mirror via the async `timeline_changed` event, which may
  // not have fired yet; refresh now so the next queued add computes its append
  // position from a mirror that already includes this clip. (Browser mode
  // already refreshed inside `applyAndRefresh` — guard to avoid a double fetch.)
  if (isTauri) await forceRefresh();
}

async function addMediaToTimelineAtInner(
  item: MediaItem,
  startFrame: number,
  preferredTrackIndex: number | null,
  insertTrackAt?: number,
): Promise<void> {
  let timeline = useProjectStore.getState().timeline;
  if (insertTrackAt !== undefined) {
    const res = await insertTrack(item.type === "audio" ? "audio" : "video", insertTrackAt);
    await forceRefresh();
    timeline = useProjectStore.getState().timeline;
    const insertedTrackId = res?.affectedClipIds[0];
    const insertedIndex = insertedTrackId
      ? timeline.tracks.findIndex((track) => track.id === insertedTrackId)
      : -1;
    if (insertedIndex >= 0) preferredTrackIndex = insertedIndex;
  }
  let entry = entryForMediaAt(timeline, item, Math.max(0, startFrame), preferredTrackIndex);
  if (!entry) {
    const fallbackInsertAt = preferredTrackIndex ?? undefined;
    const res = await insertTrack(item.type === "audio" ? "audio" : "video", fallbackInsertAt);
    await forceRefresh();
    timeline = useProjectStore.getState().timeline;
    const insertedTrackId = res?.affectedClipIds[0];
    const insertedIndex = insertedTrackId
      ? timeline.tracks.findIndex((track) => track.id === insertedTrackId)
      : -1;
    if (insertedIndex >= 0) {
      preferredTrackIndex = insertedIndex;
    } else if (fallbackInsertAt !== undefined) {
      preferredTrackIndex = Math.max(0, Math.min(fallbackInsertAt, timeline.tracks.length - 1));
    }
    entry = entryForMediaAt(timeline, item, Math.max(0, startFrame), preferredTrackIndex);
  }
  if (!entry) return;
  const res = await addClips([entry]);
  if (res && res.affectedClipIds.length > 0) {
    useEditorUiStore.getState().selectClips(new Set(res.affectedClipIds));
  }
  if (isTauri) await forceRefresh();
}

// MARK: - Text tool (Toolbar "T" button, SPEC §4)

/** Default text clip duration: 3 seconds at the timeline's fps. */
const DEFAULT_TEXT_SECONDS = 3;

/** Default transform for a newly created text clip (centered, unit size). */
const DEFAULT_TEXT_TRANSFORM: Transform = {
  centerX: 0.5,
  centerY: 0.5,
  width: 1,
  height: 1,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
};

const DEFAULT_TEXT_STYLE: TextStyle = {
  fontName: "Helvetica-Bold",
  fontSize: 96,
  fontScale: 1,
  color: { r: 1, g: 1, b: 1, a: 1 },
  alignment: "center",
  shadow: {
    enabled: true,
    color: { r: 0, g: 0, b: 0, a: 0.6 },
    offsetX: 0,
    offsetY: -2,
    blur: 6,
  },
  background: {
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 0.6 },
  },
  border: {
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 1 },
  },
};

/** Find the first visual track (video/image/text/lottie) index, or null. */
function firstVisualTrackIndex(timeline: Timeline): number | null {
  for (let i = 0; i < timeline.tracks.length; i++) {
    const t = timeline.tracks[i].type;
    if (t === "video" || t === "image" || t === "text" || t === "lottie") return i;
  }
  return null;
}

/** Add a text clip at the playhead on the first visual track (creating one if
 *  none exists). Selects the new clip afterwards so the Inspector opens its
 *  Text tab. Used by the Toolbar "T" button. */
export async function addTextClip() {
  const ui = useEditorUiStore.getState();
  const startFrame = ui.activeFrame;
  let timeline = useProjectStore.getState().timeline;

  let trackIndex = firstVisualTrackIndex(timeline);
  if (trackIndex === null) {
    await insertTrack("video");
    await forceRefresh();
    timeline = useProjectStore.getState().timeline;
    trackIndex = firstVisualTrackIndex(timeline);
    if (trackIndex === null) return;
  }

  const durationFrames = Math.max(1, Math.round(DEFAULT_TEXT_SECONDS * timeline.fps));
  const entry: TextEntryReq = {
    trackIndex,
    startFrame,
    durationFrames,
    content: "",
    textStyle: DEFAULT_TEXT_STYLE,
    transform: DEFAULT_TEXT_TRANSFORM,
  };

  const res = await applyAndRefresh({ type: "addTexts", entries: [entry] });
  if (res && res.affectedClipIds.length > 0) {
    ui.selectClips(new Set(res.affectedClipIds));
  }
}

// MARK: - Clipboard (copy / cut / paste, Issue #94)
//
// Front-end paste buffer: copy snapshots the selected clips; paste re-places
// them at the playhead with a fresh `linkGroupId` (cleared so the backend
// re-assigns, mirroring upstream `pasteClipsAtPlayhead` link re-reflection).
// Track placement is preserved (clip stays on its original track index); if
// the target track no longer exists the clip is skipped.

/** Collect selected clips with their track index into the clipboard store.
 *  If any selected clip belongs to a link group, the entire group is copied
 *  (mirrors upstream `copyClips` which expands the selection to include
 *  linked companions, so a paste reproduces the video+audio pair). */
export function copyClips() {
  const ui = useEditorUiStore.getState();
  const tl = useProjectStore.getState().timeline;
  const ids = ui.selectedClipIds;
  if (ids.size === 0) return;
  // Expand selection to include linked companions.
  const expanded = new Set<string>(ids);
  for (let ti = 0; ti < tl.tracks.length; ti++) {
    for (const clip of tl.tracks[ti].clips) {
      if (ids.has(clip.id) && clip.linkGroupId) {
        for (let tj = 0; tj < tl.tracks.length; tj++) {
          for (const c2 of tl.tracks[tj].clips) {
            if (c2.linkGroupId === clip.linkGroupId) expanded.add(c2.id);
          }
        }
      }
    }
  }
  const entries: { clip: Clip; sourceTrackIndex: number }[] = [];
  for (let ti = 0; ti < tl.tracks.length; ti++) {
    for (const clip of tl.tracks[ti].clips) {
      if (expanded.has(clip.id)) entries.push({ clip, sourceTrackIndex: ti });
    }
  }
  if (entries.length === 0) return;
  const sourceFirstFrame = entries.reduce(
    (min, e) => Math.min(min, e.clip.startFrame),
    Number.POSITIVE_INFINITY,
  );
  useClipboardStore.getState().set(entries, sourceFirstFrame);
}

/** Copy then delete — the standard cut semantics. */
export async function cutClips() {
  copyClips();
  await deleteSelectedClips();
}

/** Paste clipboard entries at the current playhead. Each clip's `startFrame`
 *  is offset by `activeFrame - sourceFirstFrame`. After the clips are created,
 *  link groups are re-established: clips that shared a `linkGroupId` in the
 *  clipboard are re-linked via `linkClips` so the paste preserves video+audio
 *  linkage. Clips whose source track no longer exists are silently skipped
 *  (upstream drops them too). */
export async function pasteClipsAtPlayhead() {
  const cb = useClipboardStore.getState();
  if (!cb.hasContent || cb.entries.length === 0) return;
  const ui = useEditorUiStore.getState();
  const tl = useProjectStore.getState().timeline;
  const offset = ui.activeFrame - cb.sourceFirstFrame;
  const entries: ClipEntryReq[] = [];
  const sourceLinkGroups: (string | undefined)[] = [];
  for (const e of cb.entries) {
    if (e.sourceTrackIndex >= tl.tracks.length) continue;
    const startFrame = Math.max(0, e.clip.startFrame + offset);
    entries.push({
      mediaRef: e.clip.mediaRef,
      mediaType: e.clip.mediaType,
      sourceClipType: e.clip.sourceClipType,
      trackIndex: e.sourceTrackIndex,
      startFrame,
      durationFrames: e.clip.durationFrames,
      trimStartFrame: e.clip.trimStartFrame,
      trimEndFrame: e.clip.trimEndFrame,
      hasAudio: e.clip.mediaType === "audio" || e.clip.mediaType === "video",
      transform: e.clip.transform,
      // Don't auto-create a linked audio: the linked audio clip is already in
      // the clipboard (copyClips expands link groups) and will be pasted as
      // its own entry; addLinkedAudio=true would create a duplicate.
      addLinkedAudio: false,
    });
    sourceLinkGroups.push(e.clip.linkGroupId);
  }
  if (entries.length === 0) return;
  const res = await addClips(entries);
  if (!res || res.affectedClipIds.length === 0) return;

  // Re-establish link groups: map each old linkGroupId to the set of newly
  // created clip ids, then call linkClips for each group.
  const newGroupMap = new Map<string, string[]>();
  for (let i = 0; i < res.affectedClipIds.length && i < sourceLinkGroups.length; i++) {
    const oldGroup = sourceLinkGroups[i];
    if (!oldGroup) continue;
    const newId = res.affectedClipIds[i];
    const arr = newGroupMap.get(oldGroup);
    if (arr) arr.push(newId);
    else newGroupMap.set(oldGroup, [newId]);
  }
  for (const ids of newGroupMap.values()) {
    if (ids.length >= 2) await linkClips(ids);
  }

  // Select the freshly pasted clips so the user can immediately move/trim them.
  ui.selectClips(new Set(res.affectedClipIds));
  if (isTauri) await forceRefresh();
}
