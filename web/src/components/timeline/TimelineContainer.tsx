/**
 * Timeline container (SPEC §5). Owns the scroll area, the content + ruler
 * canvases, the fixed track-header column, and the playhead/snap overlays, plus
 * the pointer-gesture decision tree (SPEC §5.8, §9): scrub, select, move, trim,
 * razor split, marquee, and the CapCut/剪映 wheel model (pinch or Cmd/Ctrl
 * zoom, Option horizontal scroll, bare/two-finger pan).
 */

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { LAYOUT, ZOOM } from "../../lib/theme";
import {
  contentHeight,
  contentWidth,
  clipRect,
  dropTargetAt,
  frameAt,
  totalFrames,
  trackAt,
} from "../../lib/geometry";
import { gapAtFrame } from "../../lib/timelineGap";
import { firstAudioIndex, trackDisplayLabel } from "../../lib/zones";
import { clampTrimDeltaFrames, trimSourceValues } from "../../lib/clip";
import { collectTargets, findSnap, findSnapDelta } from "../../lib/snap";
import { paintTimeline, type DragPaint, type MediaGhostPaint } from "./timelineCanvas";
import { useT } from "../../i18n";
import { paintRuler } from "./rulerCanvas";
import { TrackHeaderColumn } from "./TrackHeaderColumn";
import { Playhead } from "./Playhead";
import { SnapIndicator } from "./SnapIndicator";
import {
  hitTestClip,
  expandLinkGroup,
  clipsInRect,
  audioVolumeKfHit,
  fadeKneeHit,
  fadeFramesForDrag,
  type ClipHit,
  type FadeEdge,
} from "./hitTest";
import { ClipContextMenu } from "./ClipContextMenu";
import { TimelineRangeContextMenu } from "./TimelineRangeContextMenu";
import { SwapMediaPicker } from "./SwapMediaPicker";
import { MEDIA_DND_TYPE } from "../media/MediaPanel";
import { getDraggingMedia, setDraggingMedia } from "../../lib/mediaDragState";
import { getDraggingMomentRange, setDraggingMomentRange } from "../../lib/momentDragState";
import { maybeSnapFeedback } from "../../lib/haptic";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import {
  generateThumbnail,
  getWaveform,
  preloadMedia,
  requestTimelineSprite,
  setTimelineSpriteInteractive,
  type PrewarmResult,
} from "../../lib/api";
import { assetUrl } from "../../lib/asset";
import type { Clip, ClipType, Interpolation, Timeline } from "../../lib/types";
import {
  rangeContains,
  validRange,
  type TimelineRange,
} from "../../lib/timelineRange";
import type { ClipThumbnailStrip } from "./clipRenderer";

/** Keep cold long-video drops bounded: 24 representative frames preserve a
 * useful filmstrip while avoiding the former 240 random-access decodes. */
export const TIMELINE_SPRITE_FRAME_LIMIT = 24;

/** Where a move/duplicate drag will land. `newTrack` inserts before `index`
 *  (upstream `newTrackAt(index)`), clamped into visual/audio zones by the core. */
type DropTarget =
  | { kind: "existing"; trackIndex: number }
  | { kind: "newTrack"; index: number; trackType: ClipType };

type DragState =
  | { kind: "scrub" }
  | {
      kind: "move";
      hit: ClipHit;
      grabFrame: number;
      deltaFrames: number;
      startTrack: number;
      targetTrack: number;
      companions: string[];
      /** Option/Alt held at pointer-down: duplicate instead of move. */
      isDuplicate: boolean;
      /** Where the drag will land (existing track or a new track below). */
      dropTarget: DropTarget;
    }
  | { kind: "trimLeft" | "trimRight"; hit: ClipHit; startTrim: number; deltaFrames: number }
  | { kind: "marquee"; startDocX: number; startDocY: number; curDocX: number; curDocY: number }
  | {
      kind: "audioVolumeKf";
      clipId: string;
      fromFrame: number;
      ghostFrame: number;
      editContext: edit.ProjectEditContext;
    }
  | {
      kind: "fadeKnee";
      clipId: string;
      edge: FadeEdge;
      originalFrames: number;
      grabFrame: number;
      currentFrames: number;
    }
  | null;

export interface TimelineCursorState {
  toolMode: "pointer" | "razor";
  inRuler?: boolean;
  shiftKey?: boolean;
  hitRegion?: ClipHit["region"];
  dragKind?: Exclude<DragState, null>["kind"];
  disabled?: boolean;
}

/** One auditable cursor projection for the canvas interaction state. */
export function timelineInteractionCursor(state: TimelineCursorState): string {
  if (state.disabled) return "not-allowed";
  if (state.dragKind === "move" || state.dragKind === "scrub") return "grabbing";
  if (state.dragKind === "trimLeft" || state.dragKind === "trimRight") return "ew-resize";
  if (state.dragKind === "marquee") return "crosshair";
  if (state.inRuler) return state.shiftKey ? "crosshair" : "pointer";
  if (state.toolMode === "razor") return "crosshair";
  if (state.hitRegion === "trimLeft" || state.hitRegion === "trimRight") return "ew-resize";
  if (state.hitRegion === "body") return "grab";
  return "default";
}

type TimelineContextMenu =
  | {
      kind: "clip";
      clipId: string;
      x: number;
      y: number;
      fadeEdge?: FadeEdge;
      range?: TimelineRange;
    }
  | { kind: "range"; range: TimelineRange; x: number; y: number }
  | { kind: "audioVolumeKeyframe"; clipId: string; frame: number; x: number; y: number };

export function rangeAtContextFrame(
  range: TimelineRange | null,
  frame: number,
): TimelineRange | null {
  const normalized = validRange(range);
  return normalized && rangeContains(normalized, frame) ? normalized : null;
}

type VolumeKeyframeInterpolation = Extract<Interpolation, "linear" | "smooth" | "hold">;

type VolumeKeyframeMenuItem = {
  label: string;
  action: () => void;
  danger?: boolean;
  checked?: boolean;
};

const VOLUME_KEYFRAME_INTERPOLATIONS: Array<{
  labelKey: "linear" | "smooth" | "hold";
  value: VolumeKeyframeInterpolation;
}> = [
  { labelKey: "linear", value: "linear" },
  { labelKey: "smooth", value: "smooth" },
  { labelKey: "hold", value: "hold" },
];
const CHECKMARK = "\u2713";

/** Stable empty exclude-set for media-drop snapping (no clip is being dragged,
 *  so every clip edge is a snap target). Never mutated. */
const EMPTY_EXCLUDE = new Set<string>();

export function volumeKeyframeMenuItems({
  currentInterpolation,
  labels,
  onDelete,
  onSetInterpolation,
}: {
  currentInterpolation?: Interpolation;
  labels: {
    delete: string;
    linear: string;
    smooth: string;
    hold: string;
  };
  onDelete: () => void;
  onSetInterpolation: (interpolation: VolumeKeyframeInterpolation) => void;
}): VolumeKeyframeMenuItem[] {
  return [
    {
      label: labels.delete,
      action: onDelete,
      danger: true,
    },
    ...VOLUME_KEYFRAME_INTERPOLATIONS.map(({ labelKey, value }) => ({
      label: labels[labelKey],
      checked: currentInterpolation === value,
      action: () => onSetInterpolation(value),
    })),
  ];
}

/** Convert the timeline canvas' clip-relative volume-envelope coordinate to
 *  the absolute frame expected by every keyframe edit command. */
export function volumeKeyframeAbsoluteFrame(
  clip: Pick<Clip, "startFrame" | "durationFrames">,
  relativeFrame: number,
): number {
  return clip.startFrame + Math.round(relativeFrame);
}

/** Clamp a newly stamped or moved-to keyframe to the writable half-open span.
 *  Existing split-boundary keyframes may legitimately live at offset=duration,
 *  so reads/removes/move-sources use `volumeKeyframeAbsoluteFrame` instead. */
export function writableVolumeKeyframeAbsoluteFrame(
  clip: Pick<Clip, "startFrame" | "durationFrames">,
  relativeFrame: number,
): number {
  const lastRelativeFrame = Math.max(0, clip.durationFrames - 1);
  const clampedRelative = Math.max(
    0,
    Math.min(lastRelativeFrame, Math.round(relativeFrame)),
  );
  return clip.startFrame + clampedRelative;
}

/** Razor splits are valid only strictly inside a half-open clip span. */
export function strictSplitFrameForClip(
  clip: Pick<Clip, "startFrame" | "durationFrames">,
  candidateFrame: number,
): number | null {
  const frame = Math.round(candidateFrame);
  return frame > clip.startFrame && frame < clip.startFrame + clip.durationFrames
    ? frame
    : null;
}

/** New-track kind for a dragged clip: audio clips → "audio", everything else
 *  → "video" (visual types share the video zone, matching `addMediaToTimeline`
 *  and upstream `placeClip`). */
function newTrackTypeFor(clip: { mediaType: ClipType }): ClipType {
  return clip.mediaType === "audio" ? "audio" : "video";
}

function loadImageElement(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.decoding = "async";
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`thumbnail image failed to load: ${src}`));
    img.src = src;
  });
}

export function collectMoveSnapTargets(
  timeline: Timeline,
  excluded: Set<string>,
  activeFrame: number,
) {
  return collectTargets(timeline, excluded, activeFrame, true);
}

export function prewarmResultNeedsRetry(result: PrewarmResult | null): boolean {
  return result === "queued" || result === "duplicate" || result === "busy" || result === "cancelled";
}

export function prewarmResultAllowsCacheRead(result: PrewarmResult | null): boolean {
  return result === "cached";
}

export function timelinePrewarmKey(
  projectEpoch: number,
  mediaRef: string,
  sourceKey: string,
): string {
  return JSON.stringify([projectEpoch, mediaRef, sourceKey]);
}

export function timelinePrewarmShouldStart(
  key: string,
  inFlight: Set<string>,
  admissions: Map<string, PrewarmResult | null>,
  retryCounts: Map<string, number>,
): boolean {
  if (inFlight.has(key)) return false;
  if (prewarmResultAllowsCacheRead(admissions.get(key) ?? null)) return false;
  return (retryCounts.get(key) ?? 0) <= 8;
}

export function timelineVisualCacheIsCurrent(
  currentKey: string,
  cachedKey: string | undefined,
): boolean {
  return cachedKey === currentKey;
}

export function timelineVisualRequestShouldStart(
  currentKey: string,
  cachedKey: string | undefined,
  inFlight: Set<string>,
): boolean {
  return !timelineVisualCacheIsCurrent(currentKey, cachedKey) && !inFlight.has(currentKey);
}

/** Acquire the component-wide sprite request lease only for a live effect.
 * Poster decoding may finish after its owning effect was retired; admitting a
 * sprite poll from that callback would leave an in-flight key with no live
 * poller to release it. */
export function acquireTimelineSpriteRequest(
  key: string,
  inFlight: Set<string>,
  disposed: boolean,
): boolean {
  if (disposed || inFlight.has(key)) return false;
  inFlight.add(key);
  return true;
}

const TIMELINE_SPRITE_TRANSPORT_RETRY_LIMIT = 8;

/** Back off transient IPC failures without pinning the poster-only fallback.
 * The lease is released before each retry so a newer effect can take over. */
export function timelineSpriteTransportRetryDelay(failureCount: number): number | null {
  if (failureCount >= TIMELINE_SPRITE_TRANSPORT_RETRY_LIMIT) return null;
  return Math.min(2_000, 250 * 2 ** failureCount);
}

export function shouldRetryTimelineVisualAfterPosterSettlement(
  disposed: boolean,
  mounted: boolean,
): boolean {
  return disposed && mounted;
}

export function clipAccessTargetSize(width: number, height: number): { width: number; height: number } {
  return { width: Math.max(24, width), height: Math.max(24, height) };
}

export function clipAccessTargetRect(
  left: number,
  top: number,
  width: number,
  height: number,
  minLeft: number,
  maxRight: number,
  minTop = 0,
  maxBottom = Number.POSITIVE_INFINITY,
): { left: number; top: number; width: number; height: number } {
  const target = clipAccessTargetSize(width, height);
  const centeredLeft = left - (target.width - width) / 2;
  const centeredTop = top - (target.height - height) / 2;
  const availableWidth = Math.max(0, maxRight - minLeft);
  const availableHeight = Math.max(0, maxBottom - minTop);
  const clampedLeft = Number.isFinite(maxRight) && availableWidth >= target.width
    ? Math.max(minLeft, Math.min(maxRight - target.width, centeredLeft))
    : Math.max(minLeft, centeredLeft);
  const clampedTop = Number.isFinite(maxBottom) && availableHeight >= target.height
    ? Math.max(minTop, Math.min(maxBottom - target.height, centeredTop))
    : Math.max(minTop, centeredTop);
  return { left: clampedLeft, top: clampedTop, ...target };
}

export function clipSelectionForInteraction(
  timeline: Timeline,
  selectedClipIds: Set<string>,
  clipId: string,
  modifiers: { shiftKey?: boolean; altKey?: boolean },
): Set<string> {
  const linked = !modifiers.altKey;
  const already = selectedClipIds.has(clipId);
  const group = linked
    ? expandLinkGroup(timeline, new Set([clipId]))
    : new Set([clipId]);
  if (modifiers.shiftKey) {
    const next = new Set(selectedClipIds);
    if (already) group.forEach((id) => next.delete(id));
    else group.forEach((id) => next.add(id));
    return next;
  }
  if (modifiers.altKey && !already) return new Set([clipId]);
  if (!already) return group;
  return selectedClipIds;
}

/**
 * Canvas clips keep their precise edit geometry, while the surrounding pointer
 * target grows to WCAG 2.2's 24px minimum. Exact clip hits always win; only the
 * padded halo is treated as a body hit, so it cannot accidentally grab a trim
 * handle. When neighbouring halos overlap, the nearest visible clip wins.
 */
export function hitTestAccessibleClip(
  timeline: Timeline,
  docX: number,
  docY: number,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
  documentWidth = Math.max(24, totalFrames(timeline) * pixelsPerFrame),
  documentHeight = contentHeight(timeline, 0, trackHeights),
): ClipHit | null {
  const exact = hitTestClip(timeline, docX, docY, pixelsPerFrame, trackHeights);
  if (exact) return exact;

  let nearest: { hit: ClipHit; distance: number } | null = null;
  for (let trackIndex = 0; trackIndex < timeline.tracks.length; trackIndex++) {
    const track = timeline.tracks[trackIndex];
    if (track.hidden) continue;
    for (let clipIndex = 0; clipIndex < track.clips.length; clipIndex++) {
      const clip = track.clips[clipIndex];
      const rect = clipRect(timeline, trackIndex, clip, pixelsPerFrame, trackHeights);
      const target = clipAccessTargetRect(
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        0,
        documentWidth,
        0,
        documentHeight,
      );
      if (
        docX < target.left ||
        docX > target.left + target.width ||
        docY < target.top ||
        docY > target.top + target.height
      ) {
        continue;
      }
      const dx = docX < rect.x ? rect.x - docX : Math.max(0, docX - (rect.x + rect.width));
      const dy = docY < rect.y ? rect.y - docY : Math.max(0, docY - (rect.y + rect.height));
      const distance = dx * dx + dy * dy;
      if (!nearest || distance < nearest.distance) {
        nearest = {
          hit: {
            trackIndex,
            clipIndex,
            clip,
            region: "body",
            localX: Math.max(0, Math.min(rect.width, docX - rect.x)),
          },
          distance,
        };
      }
    }
  }
  return nearest?.hit ?? null;
}

export interface AccessibleClipRect {
  clipId: string;
  trackIndex: number;
  left: number;
  top: number;
  width: number;
  height: number;
  label: string;
}

export function accessibleClipRects(
  timeline: Timeline,
  pixelsPerFrame: number,
  trackHeights: Record<string, number>,
  scrollLeft: number,
  scrollTop: number,
  viewWidth: number,
  viewHeight: number,
): AccessibleClipRect[] {
  const rects: AccessibleClipRect[] = [];
  const right = LAYOUT.trackHeaderWidth + viewWidth;
  const documentWidth = contentWidth(totalFrames(timeline), pixelsPerFrame, viewWidth);
  const documentHeight = contentHeight(timeline, viewHeight, trackHeights);
  for (let trackIndex = 0; trackIndex < timeline.tracks.length; trackIndex++) {
    const track = timeline.tracks[trackIndex];
    for (const clip of track.clips) {
      const clipGeometry = clipRect(timeline, trackIndex, clip, pixelsPerFrame, trackHeights);
      const rect = clipAccessTargetRect(
        clipGeometry.x,
        clipGeometry.y,
        clipGeometry.width,
        clipGeometry.height,
        0,
        documentWidth,
        0,
        documentHeight,
      );
      const left = LAYOUT.trackHeaderWidth + rect.left - scrollLeft;
      const top = rect.top - scrollTop;
      if (
        left + rect.width < LAYOUT.trackHeaderWidth ||
        left > right ||
        top + rect.height < 0 ||
        top > viewHeight
      ) {
        continue;
      }
      rects.push({
        clipId: clip.id,
        trackIndex,
        left,
        top,
        width: rect.width,
        height: rect.height,
        label: `Clip ${clip.id} on ${trackDisplayLabel(timeline, trackIndex)}`,
      });
    }
  }
  return rects;
}

export interface MoveParticipant {
  id: string;
  trackIndex: number;
  startFrame: number;
  clip: Pick<Clip, "mediaType" | "linkGroupId">;
}

export interface ResolvedMoveTarget {
  clipId: string;
  toTrack: number;
  toFrame: number;
  pinned: boolean;
}

function isVisualType(type: ClipType): boolean {
  return type === "video" || type === "image" || type === "text" || type === "lottie";
}

function trackCompatibleWithClip(trackType: ClipType | undefined, clipType: ClipType | undefined): boolean {
  if (!trackType || !clipType) return false;
  return trackType === clipType || (isVisualType(trackType) && isVisualType(clipType));
}

export function pinnedMoveCompanionIds(timeline: Timeline, leadClipId: string): Set<string> {
  const leadLoc = findClipLoc(timeline, leadClipId);
  if (!leadLoc) return new Set();
  const leadTrackType = timeline.tracks[leadLoc[0]]?.type;
  const leadClip = timeline.tracks[leadLoc[0]]?.clips[leadLoc[1]];
  const leadLink = leadClip?.linkGroupId;
  const pinned = new Set<string>();

  for (const track of timeline.tracks) {
    for (const clip of track.clips) {
      if (clip.id === leadClipId) continue;
      if (leadLink && clip.linkGroupId === leadLink) {
        pinned.add(clip.id);
      } else if (!trackCompatibleWithClip(leadTrackType, clip.mediaType)) {
        pinned.add(clip.id);
      }
    }
  }

  return pinned;
}

export function resolveExistingTrackMove(
  timeline: Timeline,
  participants: MoveParticipant[],
  leadClipId: string,
  proposedTrackDelta: number,
  frameDelta: number,
): { trackDelta: number; pinnedIds: Set<string>; targets: ResolvedMoveTarget[] } {
  const pinnedIds = pinnedMoveCompanionIds(timeline, leadClipId);
  if (!participants.some((p) => p.id === leadClipId)) {
    return {
      trackDelta: 0,
      pinnedIds,
      targets: participants.map((p) => ({
        clipId: p.id,
        toTrack: p.trackIndex,
        toFrame: p.startFrame + frameDelta,
        pinned: pinnedIds.has(p.id),
      })),
    };
  }

  let trackDelta = proposedTrackDelta;
  const step = proposedTrackDelta >= 0 ? -1 : 1;
  while (trackDelta !== 0) {
    const ok = participants
      .filter((p) => !pinnedIds.has(p.id))
      .every((p) => {
        const dest = p.trackIndex + trackDelta;
        return trackCompatibleWithClip(timeline.tracks[dest]?.type, p.clip.mediaType);
      });
    if (ok) break;
    trackDelta += step;
  }

  return {
    trackDelta,
    pinnedIds,
    targets: participants.map((p) => {
      const pinned = pinnedIds.has(p.id);
      return {
        clipId: p.id,
        toTrack: pinned ? p.trackIndex : p.trackIndex + trackDelta,
        toFrame: p.startFrame + frameDelta,
        pinned,
      };
    }),
  };
}

export function resolveNewTrackMove(
  timeline: Timeline,
  participants: MoveParticipant[],
  leadClipId: string,
  insertedTrackIndex: number,
  frameDelta: number,
): { pinnedIds: Set<string>; targets: ResolvedMoveTarget[] } {
  const pinnedIds = pinnedMoveCompanionIds(timeline, leadClipId);
  const lead = participants.find((p) => p.id === leadClipId);
  return {
    pinnedIds,
    targets: participants.map((p) => {
      const pinned = pinnedIds.has(p.id);
      const hopsToNewTrack = !pinned && lead !== undefined && p.trackIndex === lead.trackIndex;
      const shiftedTrack = p.trackIndex >= insertedTrackIndex ? p.trackIndex + 1 : p.trackIndex;
      return {
        clipId: p.id,
        toTrack: hopsToNewTrack ? insertedTrackIndex : shiftedTrack,
        toFrame: p.startFrame + frameDelta,
        pinned,
      };
    }),
  };
}

function moveParticipantsForIds(timeline: Timeline, ids: string[]): MoveParticipant[] {
  const wanted = new Set(ids);
  const participants: MoveParticipant[] = [];
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    for (const clip of timeline.tracks[ti].clips) {
      if (wanted.has(clip.id)) {
        participants.push({
          id: clip.id,
          trackIndex: ti,
          startFrame: clip.startFrame,
          clip,
        });
      }
    }
  }
  return participants;
}

export function TimelineContainer() {
  const rootTimeline = useProjectStore((s) => s.timeline);
  const compatibilityReadOnly = useProjectStore((s) => s.compatibilityReadOnly);
  const activeNestedSequenceId = useEditorUiStore((s) => s.activeNestedSequenceId);
  const enterNestedSequence = useEditorUiStore((s) => s.enterNestedSequence);
  const exitNestedSequence = useEditorUiStore((s) => s.exitNestedSequence);
  const timeline = useMemo(
    () =>
      rootTimeline.nestedSequences?.find(
        (sequence) => sequence.id === activeNestedSequenceId,
      )?.timeline ?? rootTimeline,
    [rootTimeline, activeNestedSequenceId],
  );
  const projectEpoch = useProjectStore((s) => s.projectEpoch);
  const zoomScale = useEditorUiStore((s) => s.zoomScale);
  const setZoomScale = useEditorUiStore((s) => s.setZoomScale);
  const setMinZoomScale = useEditorUiStore((s) => s.setMinZoomScale);
  const scrollLeft = useEditorUiStore((s) => s.scrollLeft);
  const scrollTop = useEditorUiStore((s) => s.scrollTop);
  const setScroll = useEditorUiStore((s) => s.setScroll);
  const setVisibleWidth = useEditorUiStore((s) => s.setVisibleWidth);
  const toolMode = useEditorUiStore((s) => s.toolMode);
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const isScrubbing = useEditorUiStore((s) => s.isScrubbing);
  const setCurrentFrame = useEditorUiStore((s) => s.setCurrentFrame);
  const setScrubbing = useEditorUiStore((s) => s.setScrubbing);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const selectClips = useEditorUiStore((s) => s.selectClips);
  const clearSelection = useEditorUiStore((s) => s.clearSelection);
  const selectedTimelineRange = useEditorUiStore((s) => s.selectedTimelineRange);
  const selectedGap = useEditorUiStore((s) => s.selectedGap);
  const selectGap = useEditorUiStore((s) => s.selectGap);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const trackHeights = useEditorUiStore((s) => s.trackDisplayHeights);
  const mediaItems = useMediaStore((s) => s.items);
  const [canvasCursor, setCanvasCursor] = useState("default");

  useEffect(() => {
    if (
      activeNestedSequenceId &&
      !rootTimeline.nestedSequences?.some((sequence) => sequence.id === activeNestedSequenceId)
    ) {
      exitNestedSequence();
    }
  }, [activeNestedSequenceId, rootTimeline.nestedSequences, exitNestedSequence]);

  // Asset ids whose source file is offline → clips referencing them get the
  // error wash. Recomputed when the catalog changes (so a relink clears it).
  const missingMediaRefs = useMemo(
    () => new Set(mediaItems.filter((m) => m.missing).map((m) => m.id)),
    [mediaItems],
  );
  const thumbnailSourceKeys = useMemo(
    () =>
      new Map(
        mediaItems.map((m) => [
          m.id,
          `${m.path ?? ""}|${m.missing ? "missing" : "online"}`,
        ]),
      ),
    [mediaItems],
  );
  const visualCacheKeys = useMemo(
    () =>
      new Map(
        Array.from(thumbnailSourceKeys, ([ref, sourceKey]) => [
          ref,
          timelinePrewarmKey(projectEpoch, ref, sourceKey),
        ]),
      ),
    [projectEpoch, thumbnailSourceKeys],
  );

  const viewportRef = useRef<HTMLDivElement>(null);
  const contentCanvasRef = useRef<HTMLCanvasElement>(null);
  const rulerCanvasRef = useRef<HTMLCanvasElement>(null);
  const [viewport, setViewport] = useState({ width: 0, height: 0 });
  const dragRef = useRef<DragState>(null);
  // Drop-ghost for a media-panel drag hovering the timeline (null when none).
  // Read by the paint effect; bumped via `forceTick` on each dragover. Mutually
  // exclusive with `dragRef` (a clip move/trim and a media drag never overlap).
  const mediaGhostRef = useRef<MediaGhostPaint | null>(null);
  // Snap hysteresis: keeps the snapped {frame, probeOffset} across pointer
  // events so the sticky band (1.5x threshold) holds the clip on its target
  // instead of jittering at the edge (SPEC §5.7). Cleared on pointerUp.
  const snapStateRef = useRef<{ frame: number; probeOffset: number } | null>(null);
  // Sticky snap state for the playhead scrub (independent of the clip-move snap
  // above), so dragging the playhead magnetizes to clip start/end edges.
  const scrubSnapRef = useRef<{ frame: number; probeOffset: number } | null>(null);
  const [snapFrame, setSnapFrame] = useState<number | null>(null);
  const [dragTick, forceTick] = useState(0);
  const [menu, setMenu] = useState<TimelineContextMenu | null>(null);
  const t = useT();
  // Waveform sample cache (media id → buckets), loaded on demand from Rust.
  const waveformsRef = useRef<Map<string, number[]>>(new Map());
  // Visual thumbnail cache (media id → decoded sprite/single image).
  const thumbnailsRef = useRef<Map<string, ClipThumbnailStrip>>(new Map());
  const waveformCacheKeysRef = useRef<Map<string, string>>(new Map());
  const thumbnailCacheKeysRef = useRef<Map<string, string>>(new Map());
  const latestVisualCacheKeysRef = useRef(visualCacheKeys);
  // Refs of media whose waveform fetch is currently in flight — kept separate from
  // the resolved-cache `waveformsRef` so a failed/empty fetch can be retried on a
  // later effect run instead of being permanently suppressed by a placeholder (#127).
  const inFlightRef = useRef<Set<string>>(new Set());
  const thumbnailPosterInFlightRef = useRef<Set<string>>(new Set());
  const thumbnailSpriteInFlightRef = useRef<Set<string>>(new Set());
  const prewarmInFlightRef = useRef<Set<string>>(new Set());
  const timelinePrewarmRef = useRef<Map<string, PrewarmResult | null>>(new Map());
  const prewarmRetryRef = useRef<Map<string, number>>(new Map());
  const cacheRetryTimerRef = useRef<number | null>(null);
  const [cacheRetryTick, setCacheRetryTick] = useState(0);
  // Guards `setWaveformVersion` against firing after unmount (the cache write itself
  // is mount-independent and must NOT be discarded on re-render — see #127).
  const mountedRef = useRef(true);
  useEffect(() => () => {
    mountedRef.current = false;
    if (cacheRetryTimerRef.current !== null) window.clearTimeout(cacheRetryTimerRef.current);
  }, []);
  const [waveformVersion, setWaveformVersion] = useState(0);
  const [thumbnailVersion, setThumbnailVersion] = useState(0);
  const [thumbnailRequestVersion, setThumbnailRequestVersion] = useState(0);
  latestVisualCacheKeysRef.current = visualCacheKeys;
  const currentWaveforms = useMemo(() => {
    const current = new Map<string, number[]>();
    for (const [ref, samples] of waveformsRef.current) {
      const key = visualCacheKeys.get(ref);
      if (key && timelineVisualCacheIsCurrent(key, waveformCacheKeysRef.current.get(ref))) {
        current.set(ref, samples);
      }
    }
    return current;
  }, [visualCacheKeys, waveformVersion]);
  const currentThumbnails = useMemo(() => {
    const current = new Map<string, ClipThumbnailStrip>();
    for (const [ref, strip] of thumbnailsRef.current) {
      const key = visualCacheKeys.get(ref);
      if (key && timelineVisualCacheIsCurrent(key, thumbnailCacheKeysRef.current.get(ref))) {
        current.set(ref, strip);
      }
    }
    return current;
  }, [visualCacheKeys, thumbnailVersion]);

  const scheduleCacheRetry = useCallback(() => {
    if (cacheRetryTimerRef.current !== null) return;
    cacheRetryTimerRef.current = window.setTimeout(() => {
      cacheRetryTimerRef.current = null;
      if (mountedRef.current) setCacheRetryTick((value) => value + 1);
    }, 800);
  }, []);

  const recordPrewarmResult = useCallback(
    (key: string, result: PrewarmResult | null) => {
      timelinePrewarmRef.current.set(key, result);
      if (prewarmResultAllowsCacheRead(result)) {
        prewarmRetryRef.current.delete(key);
        if (mountedRef.current) setCacheRetryTick((value) => value + 1);
        return;
      }
      if (!prewarmResultNeedsRetry(result)) return;
      const count = (prewarmRetryRef.current.get(key) ?? 0) + 1;
      prewarmRetryRef.current.set(key, count);
      if (count <= 8) scheduleCacheRetry();
    },
    [scheduleCacheRetry],
  );

  const total = useMemo(() => totalFrames(timeline), [timeline]);
  const docWidth = useMemo(
    () => contentWidth(total, zoomScale, viewport.width),
    [total, zoomScale, viewport.width],
  );
  const docHeight = useMemo(
    () => contentHeight(timeline, viewport.height, trackHeights),
    [timeline, viewport.height, trackHeights],
  );
  const accessibilityRects = useMemo(
    () => accessibleClipRects(timeline, zoomScale, trackHeights, scrollLeft, scrollTop, viewport.width, viewport.height),
    [timeline, zoomScale, trackHeights, scrollLeft, scrollTop, viewport.width, viewport.height],
  );
  const firstAudio = useMemo(() => firstAudioIndex(timeline), [timeline]);

  // Observe viewport size.
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const update = () => {
      const w = el.clientWidth - LAYOUT.trackHeaderWidth;
      const h = el.clientHeight;
      setViewport({ width: Math.max(0, w), height: h });
      setVisibleWidth(Math.max(0, w));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [setVisibleWidth]);

  // minZoomScale = fit all frames into the visible width (lower bound).
  useEffect(() => {
    if (viewport.width > 0 && total > 0) {
      const fit = viewport.width / total;
      setMinZoomScale(Math.min(ZOOM.default, Math.max(0.01, fit)));
    }
  }, [viewport.width, total, setMinZoomScale]);

  // Auto-scroll to keep the playhead visible during playback (upstream follows
  // the playhead, but never auto-selects the clip under it). Gated on isPlaying
  // so it never fights manual scrolling while paused; when the playhead nears a
  // horizontal edge it recenters to a quarter from the left.
  useEffect(() => {
    if (!isPlaying || viewport.width <= 0) return;
    const playheadX = activeFrame * zoomScale;
    const margin = 60;
    if (playheadX < scrollLeft + margin || playheadX > scrollLeft + viewport.width - margin) {
      const maxScroll = Math.max(0, docWidth - viewport.width);
      const target = Math.min(maxScroll, Math.max(0, playheadX - viewport.width * 0.25));
      if (target !== scrollLeft) setScroll(target, scrollTop);
    }
  }, [isPlaying, activeFrame, zoomScale, viewport.width, scrollLeft, scrollTop, docWidth, setScroll]);

  // Paint content canvas.
  useEffect(() => {
    const canvas = contentCanvasRef.current;
    if (!canvas || viewport.width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(viewport.width * dpr);
    canvas.height = Math.ceil(viewport.height * dpr);
    canvas.style.width = `${viewport.width}px`;
    canvas.style.height = `${viewport.height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    // Project the active drag so dragged clips render at their live position
    // (ghost) — `dragTick` (bumped each pointer-move) re-runs this effect.
    const d = dragRef.current;
    let drag: DragPaint | undefined;
    if (d?.kind === "move") {
      const participants = moveParticipantsForIds(timeline, d.companions);
      const lead = participants.find((p) => p.id === d.hit.clip.id);
      const proposedTrackDelta =
        d.dropTarget.kind === "existing" && lead ? d.dropTarget.trackIndex - lead.trackIndex : 0;
      const resolved = resolveExistingTrackMove(
        timeline,
        participants,
        d.hit.clip.id,
        proposedTrackDelta,
        0,
      );
      // Single clip crossing onto exactly one existing clip → preview the swap:
      // the displaced clip will ghost at the slot the lead is vacating. Mirrors
      // the drop-side decision in `endDrag` so what you see is what you get.
      let swap: { clipId: string; toTrackIndex: number; toFrame: number } | undefined;
      if (
        !d.isDuplicate &&
        d.dropTarget.kind === "existing" &&
        participants.length === 1 &&
        lead &&
        resolved.trackDelta !== 0
      ) {
        const leadDur = timeline.tracks
          .flatMap((tk) => tk.clips)
          .find((c) => c.id === d.hit.clip.id)?.durationFrames;
        const destTrack = timeline.tracks[lead.trackIndex + resolved.trackDelta];
        if (leadDur && destTrack) {
          const leadToFrame = lead.startFrame + d.deltaFrames;
          const leadEnd = leadToFrame + leadDur;
          const overlap = destTrack.clips.filter(
            (c) =>
              c.id !== d.hit.clip.id &&
              c.startFrame < leadEnd &&
              c.startFrame + c.durationFrames > leadToFrame,
          );
          if (overlap.length === 1) {
            swap = { clipId: overlap[0].id, toTrackIndex: lead.trackIndex, toFrame: lead.startFrame };
          }
        }
      }
      drag = {
        kind: "move",
        ids: new Set(d.companions),
        deltaFrames: d.deltaFrames,
        trackDelta: d.dropTarget.kind === "existing" ? resolved.trackDelta : 0,
        pinnedIds: resolved.pinnedIds,
        leadTrackIndex: lead?.trackIndex ?? d.startTrack,
        isDuplicate: d.isDuplicate,
        newTrackType: d.dropTarget.kind === "newTrack" ? d.dropTarget.trackType : undefined,
        newTrackIndex: d.dropTarget.kind === "newTrack" ? d.dropTarget.index : undefined,
        swap,
      };
    } else if (d?.kind === "trimLeft" || d?.kind === "trimRight") {
      drag = {
        kind: "trim",
        clipId: d.hit.clip.id,
        edge: d.kind === "trimLeft" ? "left" : "right",
        deltaFrames: d.deltaFrames,
      };
    } else if (d?.kind === "audioVolumeKf") {
      drag = {
        kind: "volumeKf",
        clipId: d.clipId,
        fromFrame: d.fromFrame,
        ghostFrame: d.ghostFrame,
      };
    } else if (d?.kind === "fadeKnee") {
      drag = {
        kind: "fadeKnee",
        clipId: d.clipId,
        edge: d.edge,
        currentFrames: d.currentFrames,
      };
    }
    paintTimeline(ctx, {
      timeline,
      pixelsPerFrame: zoomScale,
      trackHeights,
      selectedClipIds,
      dpr,
      width: docWidth,
      height: docHeight,
      firstAudioIndex: firstAudio,
      scrollLeft,
      scrollTop,
      viewWidth: viewport.width,
      viewHeight: viewport.height,
      waveforms: currentWaveforms,
      thumbnails: currentThumbnails,
      missingMediaRefs,
      emptyLabel: t("timeline.dropHint"),
      drag,
      mediaGhost: mediaGhostRef.current ?? undefined,
      selectedRange: selectedTimelineRange,
      selectedGap,
    });
  }, [
    timeline,
    zoomScale,
    trackHeights,
    selectedClipIds,
    selectedTimelineRange,
    selectedGap,
    scrollLeft,
    scrollTop,
    viewport,
    docWidth,
    docHeight,
    firstAudio,
    waveformVersion,
    thumbnailVersion,
    currentWaveforms,
    currentThumbnails,
    missingMediaRefs,
    dragTick,
    t,
  ]);

  // Admit every timeline source through the bounded project-scoped scheduler.
  // Audio waveform reads below wait for `cached`; queued/duplicate/busy results
  // are polled without starting a second synchronous decoder on the UI path.
  useEffect(() => {
    const wanted = new Set<string>();
    for (const track of timeline.tracks) {
      for (const clip of track.clips) {
        if (!missingMediaRefs.has(clip.mediaRef)) wanted.add(clip.mediaRef);
      }
    }
    for (const ref of wanted) {
      const sourceKey = thumbnailSourceKeys.get(ref);
      if (!sourceKey) continue;
      const key = timelinePrewarmKey(projectEpoch, ref, sourceKey);
      if (
        !timelinePrewarmShouldStart(
          key,
          prewarmInFlightRef.current,
          timelinePrewarmRef.current,
          prewarmRetryRef.current,
        )
      ) {
        continue;
      }
      prewarmInFlightRef.current.add(key);
      void preloadMedia(ref)
        .then((result) => recordPrewarmResult(key, result))
        .finally(() => prewarmInFlightRef.current.delete(key));
    }
  }, [
    timeline,
    projectEpoch,
    missingMediaRefs,
    thumbnailSourceKeys,
    cacheRetryTick,
    recordPrewarmResult,
  ]);

  // Load waveform samples only after the scheduler confirms the cache is ready,
  // then trigger a repaint. The real bars replace the faint band without doing
  // an expensive decode inside the drop/paint interaction.
  useEffect(() => {
    const wanted = new Set<string>();
    for (const track of timeline.tracks) {
      for (const clip of track.clips) {
        if (clip.mediaType === "audio" && !missingMediaRefs.has(clip.mediaRef)) {
          wanted.add(clip.mediaRef);
        }
      }
    }
    for (const ref of wanted) {
      const sourceKey = thumbnailSourceKeys.get(ref);
      const key = visualCacheKeys.get(ref);
      const admission = key ? timelinePrewarmRef.current.get(key) : null;
      if (
        !sourceKey ||
        !key ||
        !prewarmResultAllowsCacheRead(admission ?? null)
      ) {
        continue;
      }
      // Skip only if already resolved (cached) or a fetch is in flight. A failed or
      // empty fetch leaves no placeholder, so a later effect run retries it.
      if (
        !timelineVisualRequestShouldStart(
          key,
          waveformCacheKeysRef.current.get(ref),
          inFlightRef.current,
        )
      ) {
        continue;
      }
      inFlightRef.current.add(key);
      void getWaveform(ref)
        .then((samples) => {
          // Same-project timeline edits keep the key stable, while a project/source
          // replacement changes it and rejects the stale async result.
          if (
            samples &&
            samples.length > 0 &&
            latestVisualCacheKeysRef.current.get(ref) === key
          ) {
            waveformsRef.current.set(ref, samples);
            waveformCacheKeysRef.current.set(ref, key);
            if (mountedRef.current) setWaveformVersion((v) => v + 1);
          }
        })
        .finally(() => {
          // Clear in-flight so a failed/empty ref is retried on the next effect run.
          inFlightRef.current.delete(key);
        });
    }
  }, [timeline, missingMediaRefs, thumbnailSourceKeys, visualCacheKeys, cacheRetryTick]);

  useEffect(() => {
    latestVisualCacheKeysRef.current = visualCacheKeys;
    let waveformChanged = false;
    for (const ref of waveformsRef.current.keys()) {
      const key = visualCacheKeys.get(ref);
      if (!key || !timelineVisualCacheIsCurrent(key, waveformCacheKeysRef.current.get(ref))) {
        waveformsRef.current.delete(ref);
        waveformCacheKeysRef.current.delete(ref);
        waveformChanged = true;
      }
    }
    let thumbnailChanged = false;
    for (const ref of thumbnailsRef.current.keys()) {
      const key = visualCacheKeys.get(ref);
      if (!key || !timelineVisualCacheIsCurrent(key, thumbnailCacheKeysRef.current.get(ref))) {
        thumbnailsRef.current.delete(ref);
        thumbnailCacheKeysRef.current.delete(ref);
        thumbnailChanged = true;
      }
    }
    if (waveformChanged && mountedRef.current) setWaveformVersion((v) => v + 1);
    if (thumbnailChanged && mountedRef.current) setThumbnailVersion((v) => v + 1);
  }, [visualCacheKeys]);

  // Load visual thumbnails in two phases: a poster first so dropped clips paint
  // immediately, then a video sprite that upgrades the same cache entry.
  useEffect(() => {
    let disposed = false;
    const retryTimers = new Set<ReturnType<typeof setTimeout>>();
    const startedSpriteKeys = new Set<string>();
    const wanted = new Map<string, ClipType>();
    for (const track of timeline.tracks) {
      for (const clip of track.clips) {
        if ((clip.mediaType === "video" || clip.mediaType === "image") && !missingMediaRefs.has(clip.mediaRef)) {
          if (wanted.get(clip.mediaRef) !== "video") wanted.set(clip.mediaRef, clip.mediaType);
        }
      }
    }

    const storeThumbnail = async (
      ref: string,
      key: string,
      result: Awaited<ReturnType<typeof generateThumbnail>>,
      requireSprite: boolean,
    ): Promise<boolean> => {
      if (!result) return false;
      const hasSprite =
        Boolean(result.spritePath) &&
        Boolean(result.tileWidth) &&
        Boolean(result.tileHeight) &&
        Boolean(result.columns) &&
        result.times.length > 0;
      if (requireSprite && !hasSprite) return false;
      const path = hasSprite ? result.spritePath : result.thumbnailPath;
      const url = assetUrl(path);
      if (!url) return false;
      const image = await loadImageElement(url);
      if (latestVisualCacheKeysRef.current.get(ref) !== key) return false;
      const strip: ClipThumbnailStrip = {
        image,
        kind: hasSprite ? "sprite" : "single",
        tileWidth: result.tileWidth ?? image.naturalWidth,
        tileHeight: result.tileHeight ?? image.naturalHeight,
        columns: Math.max(1, result.columns ?? 1),
        times: result.times,
      };
      thumbnailsRef.current.set(ref, strip);
      thumbnailCacheKeysRef.current.set(ref, key);
      if (mountedRef.current) setThumbnailVersion((v) => v + 1);
      return true;
    };

    const releaseSpriteLease = (key: string) => {
      thumbnailSpriteInFlightRef.current.delete(key);
      startedSpriteKeys.delete(key);
    };

    const startSpriteLoad = (ref: string, key: string, transportFailureCount = 0) => {
      if (!acquireTimelineSpriteRequest(key, thumbnailSpriteInFlightRef.current, disposed)) return;
      startedSpriteKeys.add(key);
      const poll = () => {
        if (disposed) {
          releaseSpriteLease(key);
          return;
        }
        void requestTimelineSprite(ref, {
          maxFrames: TIMELINE_SPRITE_FRAME_LIMIT,
        })
          .then(async (result) => {
            if (disposed) {
              releaseSpriteLease(key);
              return;
            }
            if (!result) {
              releaseSpriteLease(key);
              const delay = timelineSpriteTransportRetryDelay(transportFailureCount);
              if (delay == null) return;
              const timer = setTimeout(() => {
                retryTimers.delete(timer);
                startSpriteLoad(ref, key, transportFailureCount + 1);
              }, delay);
              retryTimers.add(timer);
              return;
            }
            if (result.thumbnail) {
              await storeThumbnail(ref, key, result.thumbnail, true);
            }
            if (result.status === "cached" || result.status === "failed" || result.status === "staleProject") {
              releaseSpriteLease(key);
              return;
            }
            const delay = result.status === "running" || result.status === "partial" ? 150 : 250;
            const timer = setTimeout(() => {
              retryTimers.delete(timer);
              poll();
            }, delay);
            retryTimers.add(timer);
          })
          .catch((err) => {
            console.warn(`thumbnail sprite load failed for ${ref}:`, err);
            releaseSpriteLease(key);
          });
      };
      poll();
    };

    const cleanup = () => {
      disposed = true;
      for (const timer of retryTimers) clearTimeout(timer);
      for (const key of startedSpriteKeys) {
          thumbnailSpriteInFlightRef.current.delete(key);
      }
    };

    for (const [ref, mediaType] of wanted) {
      const key = visualCacheKeys.get(ref);
      if (!key) continue;
      const cachedKey = thumbnailCacheKeysRef.current.get(ref);
      const existing = timelineVisualCacheIsCurrent(key, cachedKey)
        ? thumbnailsRef.current.get(ref)
        : undefined;

      if (
        !existing &&
        timelineVisualRequestShouldStart(key, cachedKey, thumbnailPosterInFlightRef.current)
      ) {
        thumbnailPosterInFlightRef.current.add(key);
        void generateThumbnail(ref, { includeSprite: false })
          .then(async (result) => {
            const stored = await storeThumbnail(ref, key, result, false);
            if (stored && mediaType === "video") startSpriteLoad(ref, key);
          })
          .catch((err) => {
            console.warn(`thumbnail poster load failed for ${ref}:`, err);
          })
          .finally(() => {
            thumbnailPosterInFlightRef.current.delete(key);
            if (
              shouldRetryTimelineVisualAfterPosterSettlement(disposed, mountedRef.current)
            ) {
              setThumbnailRequestVersion((version) => version + 1);
            }
          });
      } else if (mediaType === "video" && existing && existing.kind !== "sprite") {
        startSpriteLoad(ref, key);
      }
    }
    return cleanup;
  }, [
    timeline,
    missingMediaRefs,
    visualCacheKeys,
    isPlaying,
    isScrubbing,
    thumbnailRequestVersion,
  ]);

  useEffect(() => {
    void setTimelineSpriteInteractive(isPlaying || isScrubbing).catch(() => undefined);
  }, [isPlaying, isScrubbing]);

  // Paint ruler canvas (sticky top).
  useEffect(() => {
    const canvas = rulerCanvasRef.current;
    if (!canvas || viewport.width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(viewport.width * dpr);
    canvas.height = Math.ceil(LAYOUT.rulerHeight * dpr);
    canvas.style.width = `${viewport.width}px`;
    canvas.style.height = `${LAYOUT.rulerHeight}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    paintRuler(ctx, {
      fps: timeline.fps,
      pixelsPerFrame: zoomScale,
      scrollLeft,
      width: viewport.width,
      dpr,
      selectedRange: selectedTimelineRange,
    });
  }, [timeline.fps, zoomScale, scrollLeft, viewport.width, selectedTimelineRange]);

  // --- Coordinate helpers (event -> document space) ---
  const toDoc = useCallback(
    (e: { clientX: number; clientY: number }) => {
      const el = viewportRef.current;
      if (!el) return { docX: 0, docY: 0, inRuler: false };
      const rect = el.getBoundingClientRect();
      const vx = e.clientX - rect.left - LAYOUT.trackHeaderWidth;
      const vy = e.clientY - rect.top;
      return { docX: vx + scrollLeft, docY: vy + scrollTop, inRuler: vy < LAYOUT.rulerHeight };
    },
    [scrollLeft, scrollTop],
  );

  // --- Wheel: 1:1 with CapCut/剪映's scroll-wheel & trackpad model ---
  //   • pinch (ctrlKey, set by the browser on a trackpad pinch) OR Cmd (Mac) /
  //     Ctrl (Win) + scroll → cursor-anchored ZOOM (剪映: "Ctrl/Cmd + 滚轮 缩放，
  //     以当前位置为原点").
  //   • Option (altKey) + scroll → HORIZONTAL scroll (剪映: "Alt + 滚轮 = 左右").
  //   • bare scroll / two-finger swipe → pan (剪映: "滚轮 = 上下"); on a trackpad
  //     deltaX also pans horizontally, so a two-finger swipe moves the timeline
  //     in any direction.
  const onWheel = useCallback(
    (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const { docX } = toDoc(e);
        const pointerViewX = docX - scrollLeft;
        const hasPointerAnchor = pointerViewX >= 0 && pointerViewX <= viewport.width;
        const anchorFrame = hasPointerAnchor ? docX / zoomScale : activeFrame;
        const viewX = hasPointerAnchor ? pointerViewX : activeFrame * zoomScale - scrollLeft;
        const factor = Math.exp(-e.deltaY * ZOOM.scrollSensitivity);
        const newScale = Math.max(
          useEditorUiStore.getState().minZoomScale,
          Math.min(ZOOM.max, zoomScale * factor),
        );
        setZoomScale(newScale);
        // Keep the frame under the cursor/playhead stationary.
        const newDocX = anchorFrame * newScale;
        setScroll(Math.max(0, newDocX - viewX), scrollTop);
      } else if (e.altKey) {
        e.preventDefault();
        // Option + scroll = horizontal (剪映 Alt+滚轮). A mouse wheel only has
        // deltaY, so fall back to it when there's no deltaX.
        const maxLeft = Math.max(0, docWidth - viewport.width);
        const dx = (e.deltaX || e.deltaY) * ZOOM.panSpeed;
        setScroll(Math.max(0, Math.min(maxLeft, scrollLeft + dx)), scrollTop);
      } else {
        // Bare scroll / two-finger swipe pans the timeline: vertical (剪映 上下)
        // plus horizontal on a trackpad. preventDefault stops the macOS
        // two-finger swipe from triggering browser back/forward navigation.
        e.preventDefault();
        const maxLeft = Math.max(0, docWidth - viewport.width);
        const maxTop = Math.max(0, docHeight - viewport.height);
        setScroll(
          Math.max(0, Math.min(maxLeft, scrollLeft + e.deltaX)),
          Math.max(0, Math.min(maxTop, scrollTop + e.deltaY)),
        );
      }
    },
    [toDoc, zoomScale, activeFrame, scrollLeft, scrollTop, setZoomScale, setScroll, docWidth, docHeight, viewport],
  );

  // Attach the wheel handler natively with { passive: false }. React's onWheel
  // is passive, so preventDefault() there silently no-ops — but a trackpad pinch
  // is Ctrl+wheel, which the webview would otherwise turn into a PAGE zoom, and a
  // two-finger swipe could trigger back/forward navigation. A latest-ref keeps
  // the listener stable while always running the current closure.
  const onWheelRef = useRef(onWheel);
  onWheelRef.current = onWheel;
  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => onWheelRef.current(e);
    el.addEventListener("wheel", handler, { passive: false });
    return () => el.removeEventListener("wheel", handler);
  }, []);

  const updateRulerScrubFrame = useCallback(
    (docX: number) => {
      const raw = frameAt(docX, zoomScale);
      const targets = collectTargets(timeline, EMPTY_EXCLUDE, null, false);
      const snap = findSnapDelta([raw], targets, zoomScale, scrubSnapRef.current, [0]);
      scrubSnapRef.current = snap
        ? { frame: snap.snappedFrame, probeOffset: snap.probeOffset }
        : null;
      setCurrentFrame(Math.max(0, Math.round(snap ? raw + snap.delta : raw)));
      maybeSnapFeedback(snap ? snap.snappedFrame : null);
    },
    [setCurrentFrame, timeline, zoomScale],
  );

  // --- Pointer down: the decision tree (SPEC §5.8) ---
  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      const { docX, docY, inRuler } = toDoc(e);
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);

      // Ruler -> scrub playhead.
      if (inRuler) {
        dragRef.current = { kind: "scrub" };
        setCanvasCursor("grabbing");
        scrubSnapRef.current = null;
        setScrubbing(true);
        updateRulerScrubFrame(docX);
        return;
      }

      const hit = hitTestAccessibleClip(
        timeline,
        docX,
        docY,
        zoomScale,
        trackHeights,
        docWidth,
        docHeight,
      );
      const fadeHit =
        !e.metaKey && !e.shiftKey
          ? fadeKneeHit(timeline, docX, docY, zoomScale, trackHeights)
          : null;

      // Compatibility-read-only projects remain selectable and scrubbable, but
      // no canvas edit gesture may start behind the disabled cursor.
      if (compatibilityReadOnly && hit) {
        selectClips(clipSelectionForInteraction(timeline, selectedClipIds, hit.clip.id, e));
        dragRef.current = null;
        setCanvasCursor("not-allowed");
        return;
      }

      // Razor tool + clip -> split at the (snapped) click frame. Snapping to
      // clip edges / playhead matches upstream's razor (a cut landing on the
      // clip's own edge is a backend no-op, which is fine).
      if (toolMode === "razor" && hit) {
        const raw = frameAt(docX, zoomScale);
        const targets = collectTargets(timeline, new Set(), activeFrame, true);
        const snap = findSnap(raw, targets, zoomScale, null);
        const cutFrame = strictSplitFrameForClip(hit.clip, snap ? snap.frame : raw);
        if (cutFrame !== null) {
          void edit.splitClip(hit.clip.id, cutFrame).catch((error: unknown) => {
            const message = error instanceof Error ? error.message : String(error);
            pushToast(t("timeline.splitFailed", { error: message }));
          });
        }
        dragRef.current = null;
        return;
      }

      // Volume-keyframe dot drag (non-Cmd, non-shift): grab a volume kf dot to
      // move it (SPEC §5.4 volume envelope). Checked before the clip-body hit so
      // a dot click drags the kf instead of starting a clip move.
      if (!e.metaKey && !e.shiftKey) {
        const kfHit = audioVolumeKfHit(timeline, docX, docY, zoomScale, trackHeights);
        if (kfHit) {
          selectClips(new Set([kfHit.clipId]));
          dragRef.current = {
            kind: "audioVolumeKf",
            clipId: kfHit.clipId,
            fromFrame: kfHit.frame,
            ghostFrame: kfHit.frame,
            editContext: edit.captureProjectEditContext(),
          };
          return;
        }
      }

      // Cmd+click on an audio clip's volume line (not a kf dot) → stamp a new
      // volume keyframe at the clicked frame (SPEC §5.4). A click landing on an
      // existing dot is a no-op (the kf already exists there).
      if (e.metaKey && hit && hit.clip.mediaType === "audio") {
        const onDot = audioVolumeKfHit(timeline, docX, docY, zoomScale, trackHeights) !== null;
        if (!onDot) {
          const absoluteFrame = writableVolumeKeyframeAbsoluteFrame(
            hit.clip,
            frameAt(docX, zoomScale) - hit.clip.startFrame,
          );
          void edit.stampKeyframe(hit.clip.id, "volume", absoluteFrame).catch((error: unknown) => {
            const message = error instanceof Error ? error.message : String(error);
            pushToast(t("inspector.keyframes.stampFailed", { error: message }));
          });
        }
        selectClips(new Set([hit.clip.id]));
        dragRef.current = null;
        return;
      }

      if (hit) {
        // Selection logic (linkedOn = !Option).
        const nextSel = clipSelectionForInteraction(timeline, selectedClipIds, hit.clip.id, e);
        selectClips(nextSel);

        // Fade knees sit in a 14px upstream hit square that can overlap the 4px
        // trim handle, so they win before trim/body routing.
        if (fadeHit && fadeHit.clipId === hit.clip.id && !e.altKey) {
          dragRef.current = {
            kind: "fadeKnee",
            clipId: hit.clip.id,
            edge: fadeHit.edge,
            originalFrames: fadeHit.currentFrames,
            grabFrame: frameAt(docX, zoomScale),
            currentFrames: fadeHit.currentFrames,
          };
        } else if (hit.region === "trimLeft" && !e.altKey) {
          dragRef.current = {
            kind: "trimLeft",
            hit,
            startTrim: hit.clip.trimStartFrame,
            deltaFrames: 0,
          };
        } else if (hit.region === "trimRight" && !e.altKey) {
          dragRef.current = {
            kind: "trimRight",
            hit,
            startTrim: hit.clip.trimEndFrame,
            deltaFrames: 0,
          };
        } else {
          const grabFrame = frameAt(docX, zoomScale);
          dragRef.current = {
            kind: "move",
            hit,
            grabFrame,
            deltaFrames: 0,
            startTrack: hit.trackIndex,
            targetTrack: hit.trackIndex,
            companions: [...nextSel],
            isDuplicate: e.altKey,
            dropTarget: { kind: "existing", trackIndex: hit.trackIndex },
          };
        }
        return;
      }

      // Empty space -> clear selection (non-shift) + start marquee. If the click
      // lands in an empty gap between clips, select that gap (upstream sets
      // `selectedGap = hitTestGap(...)` here; gap & clip selection are mutually
      // exclusive). A gap is only selectable when NOT shift-extending a marquee.
      if (!e.shiftKey) {
        clearSelection();
        const ti = trackAt(timeline, docY, trackHeights);
        const gap = ti !== null ? gapAtFrame(timeline, ti, frameAt(docX, zoomScale)) : null;
        selectGap(gap); // null clears any prior gap; a hit selects it
      }
      dragRef.current = {
        kind: "marquee",
        startDocX: docX,
        startDocY: docY,
        curDocX: docX,
        curDocY: docY,
      };
    },
    [
      toDoc,
      timeline,
      zoomScale,
      trackHeights,
      toolMode,
      selectedClipIds,
      selectClips,
      clearSelection,
      selectGap,
      setCurrentFrame,
      setScrubbing,
      updateRulerScrubFrame,
      docWidth,
      docHeight,
      compatibilityReadOnly,
      pushToast,
      t,
    ],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      const { docX, docY, inRuler } = toDoc(e);
      if (!d) {
        const hit = inRuler
          ? null
          : hitTestAccessibleClip(
              timeline,
              docX,
              docY,
              zoomScale,
              trackHeights,
              docWidth,
              docHeight,
            );
        setCanvasCursor(
          timelineInteractionCursor({
            toolMode,
            inRuler,
            shiftKey: e.shiftKey,
            hitRegion: hit?.region,
            disabled: compatibilityReadOnly && Boolean(hit),
          }),
        );
        return;
      }
      setCanvasCursor(timelineInteractionCursor({ toolMode, dragKind: d.kind }));

      if (d.kind === "scrub") {
        // Interactive moves update only the playhead. The settled Rust
        // composite remains disabled until pointer-up publishes one exact seek.
        updateRulerScrubFrame(docX);
        return;
      }

      if (d.kind === "move") {
        const rawFrame = frameAt(docX, zoomScale);
        let deltaFrames = rawFrame - d.grabFrame;
        // Snap: probe every companion's start+end (multi-probe, SPEC §5.8) and
        // keep the snap engaged across moves via snapStateRef (sticky band).
        const excluded = new Set(d.companions);
        const targets = collectMoveSnapTargets(timeline, excluded, activeFrame);
        const leadStart = d.hit.clip.startFrame;
        const probes: number[] = [];
        const probeOffsets: number[] = [];
        for (const id of d.companions) {
          const loc = findClipLoc(timeline, id);
          if (!loc) continue;
          const c = timeline.tracks[loc[0]].clips[loc[1]];
          const startOff = c.startFrame - leadStart;
          const endOff = startOff + c.durationFrames;
          // Moved absolute frame = lead's moved start + this probe's offset.
          probes.push(leadStart + deltaFrames + startOff);
          probeOffsets.push(startOff);
          probes.push(leadStart + deltaFrames + endOff);
          probeOffsets.push(endOff);
        }
        const snap = findSnapDelta(
          probes,
          targets,
          zoomScale,
          snapStateRef.current,
          probeOffsets,
        );
        let snapped: number | null = null;
        if (snap) {
          deltaFrames += snap.delta;
          snapStateRef.current = { frame: snap.snappedFrame, probeOffset: snap.probeOffset };
          snapped = snap.snappedFrame;
        } else {
          snapStateRef.current = null;
        }
        // Clamp so the clip can't go before frame 0.
        if (d.hit.clip.startFrame + deltaFrames < 0) {
          deltaFrames = -d.hit.clip.startFrame;
          snapped = null;
          snapStateRef.current = null;
        }
        // Drop target: upstream insert zones can create a new track above,
        // between, or below existing tracks.
        const hovered = dropTargetAt(timeline, docY, trackHeights);
        let targetTrack: number;
        let dropTarget: DropTarget;
        if (hovered.kind === "existing") {
          const participants = moveParticipantsForIds(timeline, d.companions);
          const lead = participants.find((p) => p.id === d.hit.clip.id);
          const resolved = resolveExistingTrackMove(
            timeline,
            participants,
            d.hit.clip.id,
            lead ? hovered.trackIndex - lead.trackIndex : 0,
            0,
          );
          targetTrack = lead ? lead.trackIndex + resolved.trackDelta : hovered.trackIndex;
          dropTarget = { kind: "existing", trackIndex: targetTrack };
        } else {
          const trackType = newTrackTypeFor(d.hit.clip);
          dropTarget = { kind: "newTrack", index: hovered.index, trackType };
          targetTrack = Math.max(0, Math.min(timeline.tracks.length - 1, hovered.index));
        }
        dragRef.current = { ...d, deltaFrames, targetTrack, dropTarget };
        setSnapFrame(snapped);
        maybeSnapFeedback(snapped);
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "audioVolumeKf") {
        const loc = findClipLoc(timeline, d.clipId);
        if (!loc) return;
        const clip = timeline.tracks[loc[0]].clips[loc[1]];
        // Cursor → clip-relative frame, clamped to the clip's span.
        let ghostFrame = frameAt(docX, zoomScale) - clip.startFrame;
        // Snap to the playhead (±5 frames, clip-relative) so a kf can be parked
        // exactly on the playhead for precise editing.
        const playheadRel = activeFrame - clip.startFrame;
        if (Math.abs(ghostFrame - playheadRel) <= 5) {
          ghostFrame = playheadRel;
          setSnapFrame(activeFrame);
        } else {
          setSnapFrame(null);
        }
        ghostFrame = Math.max(0, Math.min(Math.max(0, clip.durationFrames - 1), ghostFrame));
        dragRef.current = { ...d, ghostFrame };
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "fadeKnee") {
        const loc = findClipLoc(timeline, d.clipId);
        if (!loc) return;
        const clip = timeline.tracks[loc[0]].clips[loc[1]];
        const currentFrames = fadeFramesForDrag(
          clip,
          d.edge,
          d.originalFrames,
          d.grabFrame,
          frameAt(docX, zoomScale),
        );
        dragRef.current = { ...d, currentFrames };
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "trimLeft" || d.kind === "trimRight") {
        const rawFrame = frameAt(docX, zoomScale);
        const edge = d.kind === "trimLeft" ? d.hit.clip.startFrame : d.hit.clip.startFrame + d.hit.clip.durationFrames;
        let deltaFrames = rawFrame - edge;
        const targets = collectTargets(timeline, new Set([d.hit.clip.id]), activeFrame, true);
        const snap = findSnap(rawFrame, targets, zoomScale, null);
        if (snap) {
          deltaFrames = snap.frame - edge;
          setSnapFrame(snap.frame);
        } else {
          setSnapFrame(null);
        }
        // Clamp so the clip keeps a ≥1-frame duration and can't run past the
        // available source (upstream's mouseDragged trim clamp).
        deltaFrames = clampTrimDeltaFrames(d.hit.clip, d.kind === "trimLeft" ? "left" : "right", deltaFrames);
        dragRef.current = { ...d, deltaFrames };
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "marquee") {
        dragRef.current = { ...d, curDocX: docX, curDocY: docY };
        const ids = clipsInRect(timeline, d.startDocX, d.startDocY, docX, docY, zoomScale, trackHeights);
        const expanded = e.altKey ? ids : expandLinkGroup(timeline, ids);
        selectClips(expanded);
        forceTick((n) => n + 1);
      }
    },
    [
      toDoc,
      zoomScale,
      timeline,
      trackHeights,
      activeFrame,
      setCurrentFrame,
      selectClips,
      updateRulerScrubFrame,
      docWidth,
      docHeight,
      toolMode,
      compatibilityReadOnly,
    ],
  );

  // Abandon an in-progress drag WITHOUT committing — fires on pointercancel (a
  // touch/trackpad gesture, or an HTML5 DnD started over the canvas) and on
  // lostpointercapture (capture stolen by a reflow, e.g. importing a second media
  // item triggers insertTrack→refresh→addClips→refresh mid-gesture). Without these
  // the gesture never reaches pointerup, so dragRef and the pointer capture stay
  // stuck and the whole timeline becomes undraggable (#126).
  const endDrag = useCallback((e: React.PointerEvent) => {
    dragRef.current = null;
    setSnapFrame(null);
    maybeSnapFeedback(null); // re-arm snap feedback for the next gesture
    setScrubbing(false);
    setCanvasCursor("default");
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }, []);

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      dragRef.current = null;
      snapStateRef.current = null;
      setSnapFrame(null);
      setScrubbing(false);
      setCanvasCursor("default");
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      if (!d) return;

      if (d.kind === "scrub") {
        const { docX } = toDoc(e);
        updateRulerScrubFrame(docX);
        return;
      }

      if (d.kind === "move") {
        // No-op: no movement, no track change, and not dropping on a new track.
        if (
          d.deltaFrames === 0 &&
          d.dropTarget.kind === "existing" &&
          d.dropTarget.trackIndex === d.startTrack
        ) {
          return;
        }
        // Resolve every dragged clip's current location.
        const participants = moveParticipantsForIds(timeline, d.companions);
        const lead = participants.find((p) => p.id === d.hit.clip.id);
        if (!lead) return;

        // One group-floor FRAME delta so the earliest clip lands at >=0 and the
        // whole selection keeps its relative spacing (not per-clip max(0,...)).
        const minStart = Math.min(...participants.map((p) => p.startFrame));
        const frameDelta = Math.max(d.deltaFrames, -minStart);

        // Drop on an insert zone → create a new track first, then move/dup.
        if (d.dropTarget.kind === "newTrack") {
          void edit.moveOrDuplicateClipsToNewTrack(
            participants.map((participant) => participant.id),
            d.hit.clip.id,
            frameDelta,
            d.dropTarget.index,
            d.isDuplicate ? "duplicate" : "move",
          ).catch((error: unknown) => {
            const message = error instanceof Error ? error.message : String(error);
            pushToast(t("timeline.moveFailed", { error: message }));
          });
          return;
        }

        const resolved = resolveExistingTrackMove(
          timeline,
          participants,
          d.hit.clip.id,
          d.dropTarget.trackIndex - lead.trackIndex,
          frameDelta,
        );

        if (frameDelta === 0 && resolved.trackDelta === 0) return; // nothing actually moves
        if (d.isDuplicate) {
          // Option/Alt-drag duplicate: deep-copy each clip to its target. The
          // backend mints fresh ids, shifts start_frame by offsetFrames, and
          // clears link_group_id (copies aren't linked to the originals).
          void edit.duplicateClips(
            resolved.targets.map((target) => target.clipId),
            frameDelta,
            resolved.targets.map((target) => target.toTrack),
          );
        } else {
          // Single clip dragged across tracks onto exactly one existing clip:
          // swap their places (exchange track + start) instead of overwriting —
          // so the displaced clip relocates rather than getting swallowed.
          const leadTarget = resolved.targets.find((t) => t.clipId === d.hit.clip.id);
          const leadDur = timeline.tracks
            .flatMap((t) => t.clips)
            .find((c) => c.id === d.hit.clip.id)?.durationFrames;
          if (
            participants.length === 1 &&
            leadTarget &&
            leadDur &&
            leadTarget.toTrack !== lead.trackIndex
          ) {
            const destTrack = timeline.tracks[leadTarget.toTrack];
            const movedIds = new Set(resolved.targets.map((t) => t.clipId));
            const leadEnd = leadTarget.toFrame + leadDur;
            const overlap = destTrack
              ? destTrack.clips.filter(
                  (c) =>
                    !movedIds.has(c.id) &&
                    c.startFrame < leadEnd &&
                    c.startFrame + c.durationFrames > leadTarget.toFrame,
                )
              : [];
            if (overlap.length === 1) {
              void edit.swapClips(d.hit.clip.id, overlap[0].id);
              return;
            }
          }
          const moves = resolved.targets.map((target) => ({
            clipId: target.clipId,
            toTrack: target.toTrack,
            toFrame: target.toFrame,
          }));
          void edit.moveClips(moves);
        }
        return;
      }

      if (d.kind === "audioVolumeKf") {
        // Commit the keyframe move only when the frame actually changed (a bare
        // click on a dot is a no-op). The backend `moveKeyframe` is idempotent
        // for fromFrame === toFrame, but skipping the round-trip avoids an
        // unnecessary history entry.
        if (d.ghostFrame !== d.fromFrame) {
          const loc = findClipLoc(timeline, d.clipId);
          if (!loc) return;
          const clip = timeline.tracks[loc[0]].clips[loc[1]];
          const fromFrame = volumeKeyframeAbsoluteFrame(clip, d.fromFrame);
          const toFrame = writableVolumeKeyframeAbsoluteFrame(clip, d.ghostFrame);
          void edit.moveKeyframe(
            d.clipId,
            "volume",
            fromFrame,
            toFrame,
            d.editContext,
          ).catch((error: unknown) => {
            const message = error instanceof Error ? error.message : String(error);
            pushToast(t("inspector.keyframes.moveFailed", { error: message }));
          });
        }
        return;
      }

      if (d.kind === "fadeKnee") {
        if (d.currentFrames !== d.originalFrames) {
          const properties =
            d.edge === "left"
              ? { fadeInFrames: d.currentFrames }
              : { fadeOutFrames: d.currentFrames };
          void edit.setClipProperties([d.clipId], properties);
        }
        return;
      }

      if (d.kind === "trimLeft" || d.kind === "trimRight") {
        if (d.deltaFrames === 0) return;
        const edge = d.kind === "trimLeft" ? "left" : "right";
        // Linked partners trim together (upstream commitTrim): apply the SAME
        // timeline-frame edge delta to every clip in the link group, each
        // converted to its own SOURCE-frame trim via round(delta*speed).
        const groupIds = expandLinkGroup(timeline, new Set([d.hit.clip.id]));
        const edits = [...groupIds]
          .map((id) => {
            const loc = findClipLoc(timeline, id);
            if (!loc) return null;
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            const v = trimSourceValues(clip, edge, d.deltaFrames);
            return { clipId: id, trimStartFrame: v.trimStartFrame, trimEndFrame: v.trimEndFrame };
          })
          .filter((e): e is NonNullable<typeof e> => e !== null);
        void edit.trimClips(edits);
      }
    },
    [timeline, setScrubbing, toDoc, updateRulerScrubFrame, pushToast, t],
  );

  // Ghost preview offsets for the active drag (read from dragRef during render).
  const drag = dragRef.current;

  // Right-click on a clip -> context menu.
  const onContextMenu = useCallback(
    (e: React.MouseEvent) => {
      const { docX, docY } = toDoc(e);
      const contextRange = rangeAtContextFrame(
        selectedTimelineRange,
        frameAt(docX, zoomScale),
      );
      const keyframeHit = audioVolumeKfHit(timeline, docX, docY, zoomScale, trackHeights);
      if (keyframeHit) {
        e.preventDefault();
        selectClips(new Set([keyframeHit.clipId]));
        const loc = findClipLoc(timeline, keyframeHit.clipId);
        if (!loc) return;
        const clip = timeline.tracks[loc[0]].clips[loc[1]];
        setMenu({
          kind: "audioVolumeKeyframe",
          clipId: keyframeHit.clipId,
          frame: volumeKeyframeAbsoluteFrame(clip, keyframeHit.frame),
          x: e.clientX,
          y: e.clientY,
        });
        return;
      }
      const hit = hitTestAccessibleClip(
        timeline,
        docX,
        docY,
        zoomScale,
        trackHeights,
        docWidth,
        docHeight,
      );
      if (!hit) {
        if (contextRange) {
          e.preventDefault();
          setMenu({ kind: "range", range: contextRange, x: e.clientX, y: e.clientY });
        }
        return;
      }
      e.preventDefault();
      const fadeHit = fadeKneeHit(timeline, docX, docY, zoomScale, trackHeights);
      if (fadeHit?.clipId === hit.clip.id) {
        setMenu({
          kind: "clip",
          clipId: hit.clip.id,
          fadeEdge: fadeHit.edge,
          range: contextRange ?? undefined,
          x: e.clientX,
          y: e.clientY,
        });
        return;
      }
      // If the clip isn't already selected, select it with the same linked-group
      // semantics as a primary click so menu actions target the expected group.
      if (!selectedClipIds.has(hit.clip.id)) {
        selectClips(clipSelectionForInteraction(timeline, selectedClipIds, hit.clip.id, {}));
      }
      setMenu({
        kind: "clip",
        clipId: hit.clip.id,
        range: contextRange ?? undefined,
        x: e.clientX,
        y: e.clientY,
      });
    },
    [
      toDoc,
      timeline,
      zoomScale,
      trackHeights,
      selectedClipIds,
      selectClips,
      docWidth,
      docHeight,
      selectedTimelineRange,
    ],
  );

  const onDoubleClick = useCallback(
    (event: React.MouseEvent) => {
      const { docX, docY } = toDoc(event);
      const hit = hitTestAccessibleClip(
        timeline,
        docX,
        docY,
        zoomScale,
        trackHeights,
        docWidth,
        docHeight,
      );
      if (hit?.clip.nestedSequenceId) {
        event.preventDefault();
        enterNestedSequence(hit.clip.nestedSequenceId);
      }
    },
    [toDoc, timeline, zoomScale, trackHeights, docWidth, docHeight, enterNestedSequence],
  );

  // Media dropped from the panel lands AT the cursor: its start frame = the drop
  // X, on the track under the drop Y. `addMediaToTimelineAt` skips tracks where it
  // would overlap an existing clip (and makes a new track if none is free), so a
  // drop onto an occupied audio lane opens a second lane instead of overwriting.
  // Drop the ghost + snap state when the media drag ends or leaves the timeline.
  const clearMediaGhost = useCallback(() => {
    const had = mediaGhostRef.current !== null;
    mediaGhostRef.current = null;
    snapStateRef.current = null;
    setSnapFrame(null);
    if (had) forceTick((n) => n + 1);
  }, []);

  // While a media item hovers the timeline, paint a gray ghost at the exact
  // track + frame span it will land on (snapped to clip edges / playhead), so
  // the drop reads like other NLEs instead of a whole-region highlight. The
  // dragged item's duration/type come from the shared drag state (dataTransfer
  // is unreadable during dragover), so a foreign drag simply shows no ghost.
  const onMediaDragOver = useCallback(
    (e: React.DragEvent) => {
      if (!e.dataTransfer.types.includes(MEDIA_DND_TYPE)) return;
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = "copy";
      const item = getDraggingMedia();
      if (!item) return;
      const { docX, docY } = toDoc(e);
      // A search "Moments"/"Spoken" hit drags a trimmed source range: size the
      // ghost to that range (unless it's a still, which places the whole asset).
      const momentRange = getDraggingMomentRange();
      const durationFrames =
        momentRange && item.type !== "image" && item.duration > 0
          ? edit.momentDurationFrames(momentRange, timeline.fps)
          : edit.mediaDurationFrames(item, timeline.fps);
      const rawStart = frameAt(docX, zoomScale);
      // Snap the start OR end edge to a clip edge / playhead (multi-probe, sticky
      // — same engine as a clip move), so the ghost clicks onto neighbours.
      const targets = collectMoveSnapTargets(timeline, EMPTY_EXCLUDE, activeFrame);
      const snap = findSnapDelta(
        [rawStart, rawStart + durationFrames],
        targets,
        zoomScale,
        snapStateRef.current,
        [0, durationFrames],
      );
      let startFrame = rawStart;
      if (snap) {
        startFrame = rawStart + snap.delta;
        snapStateRef.current = { frame: snap.snappedFrame, probeOffset: snap.probeOffset };
      } else {
        snapStateRef.current = null;
      }
      if (startFrame < 0) startFrame = 0;
      const resolved = edit.resolveMediaDropTrack(
        timeline,
        item,
        startFrame,
        dropTargetAt(timeline, docY, trackHeights),
      );
      // ⌘/Ctrl held (and landing on an existing track, not a new-track insert or
      // a trimmed moment drag) → preview a ripple insert (upstream shows
      // `drawRippleInsertIndicator`). Otherwise the plain overwrite ghost.
      const rippleInsert =
        (e.ctrlKey || e.metaKey) &&
        !momentRange &&
        resolved.trackIndex !== null &&
        resolved.newTrack === null;
      const next: MediaGhostPaint = {
        startFrame,
        durationFrames,
        trackIndex: resolved.trackIndex,
        newTrackIndex: resolved.newTrack ? resolved.newTrack.index : null,
        rippleInsert,
      };
      const prev = mediaGhostRef.current;
      mediaGhostRef.current = next;
      const snappedFrame = snap ? snap.snappedFrame : null;
      setSnapFrame(snappedFrame);
      maybeSnapFeedback(snappedFrame);
      const changed =
        !prev ||
        prev.startFrame !== next.startFrame ||
        prev.durationFrames !== next.durationFrames ||
        prev.trackIndex !== next.trackIndex ||
        prev.newTrackIndex !== next.newTrackIndex ||
        prev.rippleInsert !== next.rippleInsert;
      if (changed) forceTick((n) => n + 1);
    },
    [toDoc, timeline, zoomScale, trackHeights, activeFrame],
  );

  const onMediaDragLeave = useCallback(
    (e: React.DragEvent) => {
      // Ignore leaves into child elements (canvas/header); only clear when the
      // pointer truly exits the timeline viewport (mirrors TimelineRegion).
      if (e.currentTarget.contains(e.relatedTarget as Node)) return;
      clearMediaGhost();
    },
    [clearMediaGhost],
  );

  const onMediaDrop = useCallback(
    (e: React.DragEvent) => {
      if (!e.dataTransfer.types.includes(MEDIA_DND_TYPE)) return;
      e.preventDefault();
      e.stopPropagation();
      const id = e.dataTransfer.getData(MEDIA_DND_TYPE);
      const item = useMediaStore.getState().items.find((m) => m.id === id);
      // A search-hit drag carries a source-second range → place a trimmed clip.
      const momentRange = getDraggingMomentRange();
      // Land exactly where the ghost showed: reuse the resolved plan from the
      // last dragover (drop is always preceded by a dragover at the same point).
      const plan = mediaGhostRef.current;
      clearMediaGhost();
      setDraggingMedia(null);
      setDraggingMomentRange(null);
      // Dropping onto the timeline is an HTML5 `drop` (no pointerdown), so the
      // media-preview→timeline switch in TimelineRegion's onPointerDownCapture
      // never fires. Clear the selected media here so the preview shows the
      // timeline composite at the playhead instead of staying on the dropped
      // asset's standalone preview.
      useEditorUiStore.getState().setPreviewMedia(null);
      if (!item) return;
      // Ripple-insert modifier (upstream `performDragOperation`: `let ripple =
      // mods.contains(.command)`). ⌘/Ctrl held at drop → push existing clips
      // right and insert at the drop frame instead of overwriting. Only applies
      // to a plain full-asset drop onto an existing compatible track; moment
      // (trimmed) drags and new-track drops fall through to the overwrite path.
      const ripple = e.ctrlKey || e.metaKey;
      if (
        ripple &&
        plan &&
        plan.newTrackIndex === null &&
        !momentRange &&
        plan.trackIndex !== null
      ) {
        const insertPlan = edit.buildMediaInsertPlan(
          useProjectStore.getState().timeline,
          item,
          plan.startFrame,
          plan.trackIndex,
        );
        if (insertPlan) {
          void edit.insertClips(insertPlan.trackIndex, insertPlan.atFrame, insertPlan.entries);
          return;
        }
      }
      if (plan) {
        const preferredTrackIndex = plan.newTrackIndex !== null ? null : plan.trackIndex;
        const insertTrackAt = plan.newTrackIndex !== null ? plan.newTrackIndex : undefined;
        if (momentRange) {
          void edit.addMomentToTimelineAt(
            item,
            plan.startFrame,
            preferredTrackIndex,
            momentRange,
            insertTrackAt,
          ).catch(edit.reportMediaPlacementFailure);
        } else {
          void edit
            .addMediaToTimelineAt(item, plan.startFrame, preferredTrackIndex, insertTrackAt)
            .catch(edit.reportMediaPlacementFailure);
        }
        return;
      }
      // Fallback (no prior ghost, e.g. a foreign drag): resolve from the point.
      const { docX, docY } = toDoc(e);
      const startFrame = Math.max(0, Math.round(frameAt(docX, zoomScale)));
      const target = dropTargetAt(timeline, docY, trackHeights);
      const preferredTrackIndex = target.kind === "existing" ? target.trackIndex : null;
      const insertTrackAt = target.kind === "newTrack" ? target.index : undefined;
      if (momentRange) {
        void edit
          .addMomentToTimelineAt(
            item,
            startFrame,
            preferredTrackIndex,
            momentRange,
            insertTrackAt,
          )
          .catch(edit.reportMediaPlacementFailure);
      } else {
        void edit
          .addMediaToTimelineAt(item, startFrame, preferredTrackIndex, insertTrackAt)
          .catch(edit.reportMediaPlacementFailure);
      }
    },
    [toDoc, zoomScale, timeline, trackHeights, clearMediaGhost],
  );

  return (
    <div
      ref={viewportRef}
      style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden" }}
      onDragOver={onMediaDragOver}
      onDragLeave={onMediaDragLeave}
      onDrop={onMediaDrop}
    >
      {/* Content canvas (clips + backgrounds), positioned right of header column. */}
      <canvas
        ref={contentCanvasRef}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onContextMenu={onContextMenu}
        onDoubleClick={onDoubleClick}
        onPointerCancel={endDrag}
        onLostPointerCapture={endDrag}
        onPointerLeave={() => {
          if (!dragRef.current) setCanvasCursor("default");
        }}
        style={{
          position: "absolute",
          left: LAYOUT.trackHeaderWidth,
          top: 0,
          touchAction: "none",
          cursor: canvasCursor,
        }}
      />

      {/* Ruler canvas (sticky top, over content). */}
      <canvas
        ref={rulerCanvasRef}
        style={{
          position: "absolute",
          left: LAYOUT.trackHeaderWidth,
          top: 0,
          pointerEvents: "none",
          zIndex: 30,
        }}
      />

      {/* Fixed track header column. */}
      <TrackHeaderColumn timeline={timeline} scrollTop={scrollTop} totalHeight={docHeight} />

      {/* Overlays. */}
      <SnapIndicator
        frame={snapFrame}
        pixelsPerFrame={zoomScale}
        scrollLeft={scrollLeft}
        height={viewport.height}
      />
      <Playhead
        frame={activeFrame}
        pixelsPerFrame={zoomScale}
        scrollLeft={scrollLeft}
        height={viewport.height}
      />

      <div
        role="group"
        aria-label="Timeline clips"
        style={{ position: "absolute", inset: 0, pointerEvents: "none", zIndex: 45 }}
      >
        {accessibilityRects.map((rect) => (
          <button
            key={rect.clipId}
            type="button"
            className="timeline-clip-access-button"
            aria-label={rect.label}
            aria-pressed={selectedClipIds.has(rect.clipId)}
            data-clip-id={rect.clipId}
            onClick={(event) =>
              selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, event))
            }
            onKeyDown={(event) => {
              if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
                event.preventDefault();
                if (!selectedClipIds.has(rect.clipId)) {
                  selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));
                }
                const bounds = event.currentTarget.getBoundingClientRect();
                setMenu({
                  kind: "clip",
                  clipId: rect.clipId,
                  range: rangeAtContextFrame(selectedTimelineRange, activeFrame) ?? undefined,
                  x: bounds.left + bounds.width / 2,
                  y: bounds.top + bounds.height / 2,
                });
              }
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              if (!selectedClipIds.has(rect.clipId)) {
                selectClips(clipSelectionForInteraction(timeline, selectedClipIds, rect.clipId, {}));
              }
              setMenu({
                kind: "clip",
                clipId: rect.clipId,
                range:
                  rangeAtContextFrame(
                    selectedTimelineRange,
                    frameAt(toDoc(event).docX, zoomScale),
                  ) ?? undefined,
                x: event.clientX,
                y: event.clientY,
              });
            }}
            style={{
              position: "absolute",
              left: rect.left,
              top: rect.top,
              width: rect.width,
              height: rect.height,
              pointerEvents: "none",
            }}
          />
        ))}
      </div>

      {/* Marquee box. */}
      {drag?.kind === "marquee" && (
        <MarqueeBox drag={drag} scrollLeft={scrollLeft} scrollTop={scrollTop} />
      )}

      {/* Clip right-click context menu. */}
      {menu?.kind === "clip" && (
        <ClipContextMenu
          clipId={menu.clipId}
          fadeEdge={menu.fadeEdge}
          range={menu.range}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}

      {menu?.kind === "range" && (
        <TimelineRangeContextMenu
          range={menu.range}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}

      {menu?.kind === "audioVolumeKeyframe" && (
        <AudioVolumeKeyframeContextMenu
          clipId={menu.clipId}
          frame={menu.frame}
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
        />
      )}

      <SwapMediaPicker />

      {/* Horizontal scrollbar proxy (thin) — drag handled via wheel; kept minimal. */}
    </div>
  );
}

function AudioVolumeKeyframeContextMenu({
  clipId,
  frame,
  x,
  y,
  onClose,
}: {
  clipId: string;
  frame: number;
  x: number;
  y: number;
  onClose: () => void;
}) {
  const t = useT();
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const rootTimeline = useProjectStore((s) => s.timeline);
  const activeNestedSequenceId = useEditorUiStore((s) => s.activeNestedSequenceId);
  const timeline =
    rootTimeline.nestedSequences?.find(
      (sequence) => sequence.id === activeNestedSequenceId,
    )?.timeline ?? rootTimeline;
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const currentInterpolation = findVolumeKeyframeInterpolation(timeline, clipId, frame);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const margin = 8;
    let left = x;
    let top = y;
    if (left + width + margin > window.innerWidth) left = Math.max(margin, x - width);
    if (top + height + margin > window.innerHeight) top = Math.max(margin, y - height);
    setPos({ left, top });
  }, [x, y]);

  const items = volumeKeyframeMenuItems({
    currentInterpolation,
    labels: {
      delete: t("inspector.keyframes.delete"),
      linear: t("inspector.keyframes.interpolation.linear"),
      smooth: t("inspector.keyframes.interpolation.smooth"),
      hold: t("inspector.keyframes.interpolation.hold"),
    },
    onDelete: () => {
      void edit.removeKeyframe(clipId, "volume", frame).catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        pushToast(t("inspector.keyframes.deleteFailed", { error: message }));
      });
    },
    onSetInterpolation: (interpolation) => {
      void edit.setKeyframeInterpolation(clipId, "volume", frame, interpolation).catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        pushToast(t("inspector.keyframes.interpolationFailed", { error: message }));
      });
    },
  });

  return (
    <div
      ref={ref}
      style={{
        position: "fixed",
        left: pos.left,
        top: pos.top,
        zIndex: 1000,
        minWidth: 160,
        padding: "4px 0",
        background: "var(--bg-elevated)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: 6,
        boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
        fontSize: "var(--fs-sm)",
      }}
      role="menu"
    >
      {items.map((item, i) => (
        <button
          key={i}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            display: "block",
            width: "100%",
            padding: "6px 12px",
            textAlign: "left",
            color: item.danger ? "var(--accent-danger, #ff6b6b)" : "var(--text-primary)",
            background: "transparent",
            border: "none",
            cursor: "pointer",
            fontFamily: "var(--font-sans)",
            fontSize: "var(--fs-sm)",
          }}
          role={item.checked === undefined ? "menuitem" : "menuitemradio"}
          aria-checked={item.checked ?? undefined}
          onMouseEnter={(e) => {
            (e.currentTarget as HTMLElement).style.background = "var(--bg-hover, rgba(255,255,255,0.08))";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLElement).style.background = "transparent";
          }}
        >
          {item.checked === undefined ? item.label : `${item.checked ? `${CHECKMARK} ` : "  "}${item.label}`}
        </button>
      ))}
    </div>
  );
}

export function findVolumeKeyframeInterpolation(
  timeline: Timeline,
  clipId: string,
  frame: number,
): Interpolation | undefined {
  for (const track of timeline.tracks) {
    const clip = track.clips.find((c) => c.id === clipId);
    const keyframe = clip?.volumeTrack?.keyframes.find(
      (kf) => kf.frame === frame - (clip?.startFrame ?? 0),
    );
    if (keyframe) return keyframe.interpolationOut;
  }
  return undefined;
}

function MarqueeBox({
  drag,
  scrollLeft,
  scrollTop,
}: {
  drag: { startDocX: number; startDocY: number; curDocX: number; curDocY: number };
  scrollLeft: number;
  scrollTop: number;
}) {
  const x = Math.min(drag.startDocX, drag.curDocX) - scrollLeft + LAYOUT.trackHeaderWidth;
  const y = Math.min(drag.startDocY, drag.curDocY) - scrollTop;
  const w = Math.abs(drag.curDocX - drag.startDocX);
  const h = Math.abs(drag.curDocY - drag.startDocY);
  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: w,
        height: h,
        background: "rgba(255,255,255,0.1)",
        border: "1px dashed rgba(255,255,255,0.6)",
        zIndex: 80,
        pointerEvents: "none",
      }}
    />
  );
}

function findClipLoc(timeline: { tracks: { clips: { id: string }[] }[] }, id: string): [number, number] | null {
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const ci = timeline.tracks[ti].clips.findIndex((c) => c.id === id);
    if (ci >= 0) return [ti, ci];
  }
  return null;
}
