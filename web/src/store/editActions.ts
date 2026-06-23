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
import { trimToPlayheadEdits } from "../lib/clip";
import type {
  Clip,
  ClipEntryReq,
  ClipMoveReq,
  ClipPropertiesReq,
  ClipType,
  FrameRangeReq,
  Keyframe,
  KeyframePayloadReq,
  KeyframeProperty,
  MediaItem,
  TextEntryReq,
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
  await applyAndRefresh({ type: "addClips", entries });
}

export async function moveClips(moves: ClipMoveReq[]) {
  if (moves.length === 0) return;
  await applyAndRefresh({ type: "moveClips", moves });
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

export async function linkClips(clipIds: string[]) {
  await applyAndRefresh({ type: "link", clipIds });
}

/** Insert a new empty track of `kind` (clamped into its zone by the core). Used
 *  by the drop flow to create a track on demand when the timeline has none
 *  compatible. */
export async function insertTrack(kind: ClipType) {
  await applyAndRefresh({ type: "insertTrack", kind });
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

/** Replace (or clear) a clip's keyframe track for one property. */
export async function setKeyframes(
  clipId: string,
  property: KeyframeProperty,
  payload: KeyframePayloadReq,
) {
  await applyAndRefresh({ type: "setKeyframes", clipId, property, payload });
}

/** Whether `property` is a scalar (number-valued) keyframe track — the only kind
 *  the move/stamp wrappers below handle. Pair (position/scale) and crop stay on
 *  the full-track `setKeyframes` path. */
function isScalarProperty(property: KeyframeProperty): boolean {
  return property === "volume" || property === "opacity" || property === "rotation";
}

/** The scalar keyframe array for `property` on `clip` (empty when no track yet). */
function scalarKfs(clip: Clip, property: KeyframeProperty): Keyframe<number>[] {
  if (property === "volume") return clip.volumeTrack?.keyframes ?? [];
  if (property === "opacity") return clip.opacityTrack?.keyframes ?? [];
  if (property === "rotation") return clip.rotationTrack?.keyframes ?? [];
  return [];
}

/** Default scalar value for a property (used when stamping the first kf). */
function scalarDefault(clip: Clip, property: KeyframeProperty): number {
  if (property === "volume") return clip.volume;
  if (property === "opacity") return clip.opacity;
  return 0; // rotation
}

/** Linear-interpolate a scalar track at `frame` (flat outside the kf range) so a
 *  stamped keyframe captures the currently-rendered value without jumping. */
function sampleScalar(kfs: Keyframe<number>[], frame: number, fallback: number): number {
  if (kfs.length === 0) return fallback;
  const sorted = [...kfs].sort((a, b) => a.frame - b.frame);
  if (frame <= sorted[0].frame) return sorted[0].value;
  if (frame >= sorted[sorted.length - 1].frame) return sorted[sorted.length - 1].value;
  for (let i = 0; i < sorted.length - 1; i++) {
    const a = sorted[i];
    const b = sorted[i + 1];
    if (frame >= a.frame && frame <= b.frame) {
      const span = b.frame - a.frame;
      return span <= 0 ? b.value : a.value + (b.value - a.value) * ((frame - a.frame) / span);
    }
  }
  return fallback;
}

/** Find a clip by id in the current timeline mirror. */
function findClipInTimeline(clipId: string): Clip | null {
  for (const track of useProjectStore.getState().timeline.tracks) {
    for (const c of track.clips) {
      if (c.id === clipId) return c;
    }
  }
  return null;
}

/**
 * Move a single keyframe from `fromFrame` to `toFrame`. Implemented as a
 * read-modify-write over `setKeyframes` (the only keyframe command the backend
 * currently exposes), keeping this a pure-frontend feature. A no-op when the
 * source kf is missing or the target frame is already occupied (matches
 * upstream `move_keyframe` semantics). Only scalar properties are supported
 * here — pair/crop tracks use `setKeyframes` directly.
 */
export async function moveKeyframe(
  clipId: string,
  property: KeyframeProperty,
  fromFrame: number,
  toFrame: number,
) {
  if (fromFrame === toFrame) return;
  if (!isScalarProperty(property)) return;
  const clip = findClipInTimeline(clipId);
  if (!clip) return;
  const kfs = scalarKfs(clip, property);
  if (!kfs.some((k) => k.frame === fromFrame)) return; // source missing
  if (kfs.some((k) => k.frame === toFrame)) return; // target occupied
  const newKfs = kfs
    .map((k) => (k.frame === fromFrame ? { ...k, frame: toFrame } : k))
    .sort((a, b) => a.frame - b.frame);
  await setKeyframes(clipId, property, { kind: "scalar", keyframes: newKfs });
}

/**
 * Stamp a keyframe at `frame` using the clip's current sampled value, so the
 * curve doesn't jump. Implemented as a read-modify-write over `setKeyframes`.
 * A no-op when a kf already exists at `frame`. Only scalar properties are
 * supported here.
 */
export async function stampKeyframe(
  clipId: string,
  property: KeyframeProperty,
  frame: number,
) {
  if (!isScalarProperty(property)) return;
  const clip = findClipInTimeline(clipId);
  if (!clip) return;
  const kfs = scalarKfs(clip, property);
  if (kfs.some((k) => k.frame === frame)) return; // already stamped
  const value = sampleScalar(kfs, frame, scalarDefault(clip, property));
  const newKf: Keyframe<number> = { frame, value, interpolationOut: "smooth" };
  const newKfs = [...kfs, newKf].sort((a, b) => a.frame - b.frame);
  await setKeyframes(clipId, property, { kind: "scalar", keyframes: newKfs });
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

/** Delete selected clips (⌫ / menu). */
export async function deleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = [...ui.selectedClipIds];
  if (ids.length > 0) {
    await removeClips(ids);
    ui.clearSelection();
  }
}

/** Ripple-delete selected clips (⇧⌫): remove and close the gaps, shifting
 *  sync-locked followers (the core refuses if a follower would collide). */
export async function rippleDeleteSelectedClips() {
  const ui = useEditorUiStore.getState();
  const ids = [...ui.selectedClipIds];
  if (ids.length === 0) return;
  await applyAndRefresh({ type: "rippleDeleteClips", clipIds: ids });
  ui.clearSelection();
}

// MARK: - Media -> timeline (drag and drop)

/** Stills get a fixed default duration (upstream `Constants.defaultImageDuration`
 *  ≈ 5s) since they have no intrinsic length. */
const DEFAULT_IMAGE_SECONDS = 5;

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

/** Append position on a track: just past its last clip (clamped to >= 0). */
function appendStartFrame(timeline: Timeline, trackIndex: number): number {
  return timeline.tracks[trackIndex].clips.reduce(
    (max, c) => Math.max(max, c.startFrame + c.durationFrames),
    0,
  );
}

/** Build the clip entry for a media item dropped on the timeline, or null when
 *  no compatible track exists. */
function entryForMedia(timeline: Timeline, item: MediaItem): ClipEntryReq | null {
  const trackIndex = firstCompatibleTrackIndex(timeline, item.type);
  if (trackIndex === null) return null;
  const seconds = item.duration > 0 ? item.duration : DEFAULT_IMAGE_SECONDS;
  const durationFrames = Math.max(1, Math.round(seconds * timeline.fps));
  return {
    mediaRef: item.id,
    mediaType: item.type,
    sourceClipType: item.type,
    trackIndex,
    startFrame: appendStartFrame(timeline, trackIndex),
    durationFrames,
    hasAudio: item.hasAudio,
    addLinkedAudio: item.type === "video" && item.hasAudio,
  };
}

/** Serialized tail for media -> timeline adds. Both call sites fire-and-forget
 *  (`void addMediaToTimeline(...)`), so this chains adds to keep them from
 *  racing on the shared mirror. See [`addMediaToTimeline`]. */
let mediaAddQueue: Promise<void> = Promise.resolve();

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
  const run = () => addMediaToTimelineInner(item);
  const result = mediaAddQueue.then(run, run);
  // Keep the queue alive even if an individual add rejects.
  mediaAddQueue = result.catch(() => {});
  return result;
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
    textStyle: {},
    transform: DEFAULT_TEXT_TRANSFORM,
  };

  const res = await applyAndRefresh({ type: "addTexts", entries: [entry] });
  if (res && res.affectedClipIds.length > 0) {
    ui.selectClips(new Set(res.affectedClipIds));
  }
}
