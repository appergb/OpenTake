/**
 * Clip-derived UI helpers (pure). Track color, display name, link flag.
 * See SPEC §5.4 (label = name + double-space + duration, underline when linked)
 * and §1.5 (track colors by ClipType).
 */

import { TRACK_COLOR } from "./theme";
import { formatClipDuration } from "./geometry";
import type {
  AnimPair,
  Clip,
  ClipType,
  Crop,
  KeyframeTrack,
  Timeline,
  Transform,
  TrimEditReq,
} from "./types";

export function trackColor(type: ClipType): string {
  return TRACK_COLOR[type] ?? TRACK_COLOR.video;
}

/** First non-empty line of textContent, else a friendly fallback from mediaRef. */
export function clipDisplayName(clip: Clip): string {
  if (clip.textContent) {
    const firstLine = clip.textContent.split("\n").find((l) => l.trim().length > 0);
    if (firstLine) return firstLine.trim();
  }
  if (clip.mediaRef) return clip.mediaRef;
  return clip.mediaType;
}

/** Clip label-bar text: "<name>  <duration timecode>" (ClipRenderer:598-609). */
export function clipLabel(clip: Clip, fps: number): string {
  return `${clipDisplayName(clip)}  ${formatClipDuration(clip.durationFrames, fps)}`;
}

export function isLinked(clip: Clip): boolean {
  return clip.linkGroupId != null;
}

const ASPECT_TOLERANCE = 0.02;

function positiveFinite(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

function defaultTransform(): Transform {
  return {
    centerX: 0.5,
    centerY: 0.5,
    width: 1,
    height: 1,
    rotation: 0,
    flipHorizontal: false,
    flipVertical: false,
  };
}

/** Source aspect ratio relative to the timeline canvas, matching upstream
 *  `EditorViewModel.mediaCanvasAspect(for:)`. */
export function mediaCanvasAspect(
  sourceWidth: number | null | undefined,
  sourceHeight: number | null | undefined,
  canvasWidth: number,
  canvasHeight: number,
): number | null {
  if (
    !positiveFinite(sourceWidth ?? NaN) ||
    !positiveFinite(sourceHeight ?? NaN) ||
    !positiveFinite(canvasWidth) ||
    !positiveFinite(canvasHeight)
  ) {
    return null;
  }
  const canvasAspect = canvasWidth / canvasHeight;
  return ((sourceWidth as number) / (sourceHeight as number)) / canvasAspect;
}

/** Initial aspect-fit transform for a media asset on the canvas. This is a 1:1
 *  port of upstream `fitTransform(for:canvasWidth:canvasHeight:)`. */
export function fitTransformForMedia(
  sourceWidth: number | null | undefined,
  sourceHeight: number | null | undefined,
  canvasWidth: number,
  canvasHeight: number,
): Transform {
  if (
    !positiveFinite(sourceWidth ?? NaN) ||
    !positiveFinite(sourceHeight ?? NaN) ||
    !positiveFinite(canvasWidth) ||
    !positiveFinite(canvasHeight)
  ) {
    return defaultTransform();
  }
  const canvasAspect = canvasWidth / canvasHeight;
  const sourceAspect = (sourceWidth as number) / (sourceHeight as number);
  if (Math.abs(canvasAspect - sourceAspect) < ASPECT_TOLERANCE) return defaultTransform();
  if (sourceAspect > canvasAspect) {
    return { ...defaultTransform(), width: 1, height: canvasAspect / sourceAspect };
  }
  return { ...defaultTransform(), width: sourceAspect / canvasAspect, height: 1 };
}

/** Inspector scale edits use normalized canvas width as the displayed scale.
 *  Height is derived from the source/canvas aspect so resizing never changes
 *  the media's pixel aspect, matching upstream `writeScale(into:newScale:)`.
 *  If source metadata is unavailable, preserve the clip's current transform
 *  aspect instead of collapsing to a square. */
export function resizeTransformKeepingSourceAspect(
  transform: Transform,
  width: number,
  aspect: number | null,
): Transform {
  const nextWidth = positiveFinite(width) ? width : transform.width;
  const currentAspect =
    positiveFinite(transform.width) && positiveFinite(transform.height)
      ? transform.width / transform.height
      : 1;
  const effectiveAspect = aspect !== null && positiveFinite(aspect) ? aspect : currentAspect;
  return {
    ...transform,
    width: nextWidth,
    height: nextWidth / effectiveAspect,
  };
}

export type TransformResizeCorner = "topLeft" | "topRight" | "bottomLeft" | "bottomRight";

function topLeftForTransform(transform: Transform): { x: number; y: number } {
  return {
    x: transform.centerX - transform.width / 2,
    y: transform.centerY - transform.height / 2,
  };
}

function transformFromTopLeft(
  start: Transform,
  left: number,
  top: number,
  width: number,
  height: number,
): Transform {
  return {
    ...start,
    centerX: left + width / 2,
    centerY: top + height / 2,
    width,
    height,
  };
}

function snapToBoundary(value: number, threshold: number): number {
  if (Math.abs(value) < threshold) return 0;
  if (Math.abs(value - 1) < threshold) return 1;
  return value;
}

/** Upstream TransformOverlayView resizedTransform port. Dragging a corner moves
 *  that corner, keeps the opposite corner anchored, clamps at 0.05 normalized
 *  size, preserves media aspect when known, and skips canvas-edge snap under
 *  rotation. */
export function resizeTransformFromCorner(
  start: Transform,
  corner: TransformResizeCorner,
  translationPx: { width: number; height: number },
  canvasPx: { width: number; height: number },
  mediaCanvasAspect: number | null,
  rotated: boolean,
  snapThresholdPx = 0,
): Transform {
  if (!positiveFinite(canvasPx.width) || !positiveFinite(canvasPx.height)) return start;
  const minSize = 0.05;
  const dx = translationPx.width / canvasPx.width;
  const dy = translationPx.height / canvasPx.height;
  const tl = topLeftForTransform(start);
  let left = tl.x;
  let top = tl.y;
  let right = left + start.width;
  let bottom = top + start.height;

  switch (corner) {
    case "topLeft":
      left += dx;
      top += dy;
      break;
    case "topRight":
      right += dx;
      top += dy;
      break;
    case "bottomLeft":
      left += dx;
      bottom += dy;
      break;
    case "bottomRight":
      right += dx;
      bottom += dy;
      break;
  }

  switch (corner) {
    case "topLeft":
      left = Math.min(left, right - minSize);
      top = Math.min(top, bottom - minSize);
      break;
    case "topRight":
      right = Math.max(right, left + minSize);
      top = Math.min(top, bottom - minSize);
      break;
    case "bottomLeft":
      left = Math.min(left, right - minSize);
      bottom = Math.max(bottom, top + minSize);
      break;
    case "bottomRight":
      right = Math.max(right, left + minSize);
      bottom = Math.max(bottom, top + minSize);
      break;
  }

  const aspect =
    mediaCanvasAspect !== null && positiveFinite(mediaCanvasAspect) ? mediaCanvasAspect : null;
  if (aspect !== null) {
    const w = right - left;
    const h = bottom - top;
    const widthFromHeight = h * aspect;
    if (w >= widthFromHeight) {
      const adjustedH = w / aspect;
      if (corner === "topLeft" || corner === "topRight") top = bottom - adjustedH;
      else bottom = top + adjustedH;
    } else {
      const adjustedW = h * aspect;
      if (corner === "topLeft" || corner === "bottomLeft") left = right - adjustedW;
      else right = left + adjustedW;
    }
  }

  if (!rotated && snapThresholdPx > 0) {
    const snapH = snapThresholdPx / canvasPx.width;
    const snapV = snapThresholdPx / canvasPx.height;
    const movesLeft = corner === "topLeft" || corner === "bottomLeft";
    const movesTop = corner === "topLeft" || corner === "topRight";
    const hEdge = movesLeft ? left : right;
    const vEdge = movesTop ? top : bottom;
    const snappedH = snapToBoundary(hEdge, snapH);
    const snappedV = snapToBoundary(vEdge, snapV);

    if (snappedH !== hEdge) {
      if (movesLeft) left = snappedH;
      else right = snappedH;
      if (aspect !== null) {
        if (movesTop) top = bottom - (right - left) / aspect;
        else bottom = top + (right - left) / aspect;
      }
    } else if (snappedV !== vEdge) {
      if (movesTop) top = snappedV;
      else bottom = snappedV;
      if (aspect !== null) {
        if (movesLeft) left = right - (bottom - top) * aspect;
        else right = left + (bottom - top) * aspect;
      }
    }
  }

  return transformFromTopLeft(
    start,
    left,
    top,
    Math.max(minSize, right - left),
    Math.max(minSize, bottom - top),
  );
}

/** Which edge a trim drag grabs. */
export type TrimEdge = "left" | "right";

type TrimClip = Pick<Clip, "durationFrames" | "speed" | "trimStartFrame" | "trimEndFrame" | "mediaType">;

function isUnbounded(clip: TrimClip): boolean {
  return clip.mediaType === "image" || clip.mediaType === "text";
}

/**
 * Clamp a trim-edge drag (`delta` in TIMELINE frames) so the clip keeps a ≥1
 * frame duration and — for bounded media (video/audio) — can't extend past the
 * available leading/trailing source. Mirrors upstream's `mouseDragged` trim
 * clamp; the unbounded source clamp for image/text is left to `trimSourceValues`.
 */
export function clampTrimDeltaFrames(clip: TrimClip, edge: TrimEdge, delta: number): number {
  const speed = clip.speed > 0 ? clip.speed : 1;
  if (edge === "left") {
    // Positive delta shrinks duration (left edge moves right): keep ≥1 frame.
    let d = Math.min(delta, clip.durationFrames - 1);
    if (!isUnbounded(clip)) {
      // Negative delta extends into leading source; bounded by what's trimmed.
      d = Math.max(d, -Math.floor(clip.trimStartFrame / speed));
    }
    return d;
  }
  // Right: negative delta shrinks duration (right edge moves left): keep ≥1 frame.
  let d = Math.max(delta, -(clip.durationFrames - 1));
  if (!isUnbounded(clip)) {
    d = Math.min(d, Math.floor(clip.trimEndFrame / speed));
  }
  return d;
}

/**
 * New SOURCE-frame `(trimStartFrame, trimEndFrame)` for an edge drag of `delta`
 * TIMELINE frames. 1:1 with opentake-ops `trim_values`: source delta =
 * round(delta * speed); video/audio clamp the moved edge at 0, image/text are
 * unbounded.
 */
export function trimSourceValues(
  clip: TrimClip,
  edge: TrimEdge,
  delta: number,
): { trimStartFrame: number; trimEndFrame: number } {
  const speed = clip.speed > 0 ? clip.speed : 1;
  const sourceDelta = Math.round(delta * speed);
  if (edge === "left") {
    const ns = clip.trimStartFrame + sourceDelta;
    return {
      trimStartFrame: isUnbounded(clip) ? ns : Math.max(0, ns),
      trimEndFrame: clip.trimEndFrame,
    };
  }
  const ne = clip.trimEndFrame - sourceDelta;
  return {
    trimStartFrame: clip.trimStartFrame,
    trimEndFrame: isUnbounded(clip) ? ne : Math.max(0, ne),
  };
}

/**
 * Trim-edit reqs that move each clip's IN (`edge:"left"`) or OUT (`edge:"right"`)
 * point to `frame` — 剪映's Q / W ("删除播放头左/右"). Only clips the playhead is
 * strictly inside are affected (a clip whose edge already sits at the playhead,
 * or that the playhead misses, is skipped). The delta is the same TIMELINE-frame
 * edge move the trim drag computes, so the source conversion + clamps match it.
 */
export function trimToPlayheadEdits(clips: Clip[], frame: number, edge: TrimEdge): TrimEditReq[] {
  const edits: TrimEditReq[] = [];
  for (const c of clips) {
    const end = c.startFrame + c.durationFrames;
    if (frame <= c.startFrame || frame >= end) continue; // playhead must be strictly inside
    const rawDelta = edge === "left" ? frame - c.startFrame : frame - end;
    const delta = clampTrimDeltaFrames(c, edge, rawDelta);
    if (delta === 0) continue;
    const { trimStartFrame, trimEndFrame } = trimSourceValues(c, edge, delta);
    edits.push({ clipId: c.id, trimStartFrame, trimEndFrame });
  }
  return edits;
}

// MARK: - Live sampling (1:1 port of opentake-domain::Clip::*_at)
//
// These mirror the Rust `Clip` sampling methods so the Inspector can display
// the value at the current playhead frame (live preview), matching upstream
// `InspectorView.livePreview`. Frames are absolute timeline frames; the helpers
// convert to clip-relative offsets internally. See `crates/opentake-domain/src/clip.rs`.

/** `smoothstep(t) = t*t*(3 - 2t)`. 1:1 with `keyframe::smoothstep`. */
function smoothstep(t: number): number {
  return t * t * (3.0 - 2.0 * t);
}

/** Linear amplitude <-> dB mapping (1:1 port of `VolumeScale`). */
const VOLUME_FLOOR_DB = -60.0;
const VOLUME_CEILING_DB = 15.0;

export function dbFromLinear(linear: number): number {
  if (linear > 0.0) {
    return Math.min(VOLUME_CEILING_DB, Math.max(VOLUME_FLOOR_DB, 20.0 * Math.log10(linear)));
  }
  return VOLUME_FLOOR_DB;
}

export function linearFromDb(db: number): number {
  if (db > VOLUME_FLOOR_DB) {
    return Math.pow(10, Math.min(db, VOLUME_CEILING_DB) / 20.0);
  }
  return 0.0;
}

/** Interpolate between two scalar keyframe values. */
function lerpNumber(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

/** Interpolate between two `AnimPair` values component-wise. */
function lerpAnimPair(a: AnimPair, b: AnimPair, t: number): AnimPair {
  return { a: lerpNumber(a.a, b.a, t), b: lerpNumber(a.b, b.b, t) };
}

/** Interpolate between two `Crop` values component-wise. */
function lerpCrop(a: Crop, b: Crop, t: number): Crop {
  return {
    left: lerpNumber(a.left, b.left, t),
    top: lerpNumber(a.top, b.top, t),
    right: lerpNumber(a.right, b.right, t),
    bottom: lerpNumber(a.bottom, b.bottom, t),
  };
}

/**
 * Sample a keyframe track at clip-relative `frame`, clamping at the endpoints
 * (no extrapolation). Inside a span, the *left* keyframe's `interpolationOut`
 * selects hold / linear / smooth. 1:1 port of `KeyframeTrack::sample`.
 */
export function sampleKeyframeTrack<V extends number | AnimPair | Crop>(
  track: KeyframeTrack<V> | undefined,
  frame: number,
  fallback: V,
  lerp: (a: V, b: V, t: number) => V,
): V {
  if (!track || track.keyframes.length === 0) return fallback;
  const kfs = track.keyframes;
  if (kfs.length === 1) return kfs[0].value;
  if (frame <= kfs[0].frame) return kfs[0].value;
  const last = kfs[kfs.length - 1];
  if (frame >= last.frame) return last.value;

  let bIdx = kfs.findIndex((k) => k.frame > frame);
  if (bIdx === -1) return last.value;
  const a = kfs[bIdx - 1];
  const b = kfs[bIdx];
  const raw = (frame - a.frame) / (b.frame - a.frame);
  switch (a.interpolationOut) {
    case "hold":
      return a.value;
    case "linear":
      return lerp(a.value, b.value, raw);
    case "smooth":
      return lerp(a.value, b.value, smoothstep(raw));
  }
}

/** Sample a scalar (`number`) keyframe track. */
function sampleScalarTrack(
  track: KeyframeTrack<number> | undefined,
  frame: number,
  fallback: number,
): number {
  return sampleKeyframeTrack(track, frame, fallback, lerpNumber);
}

/** Sample an `AnimPair` keyframe track. */
function samplePairTrack(
  track: KeyframeTrack<AnimPair> | undefined,
  frame: number,
  fallback: AnimPair,
): AnimPair {
  return sampleKeyframeTrack(track, frame, fallback, lerpAnimPair);
}

/** Sample a `Crop` keyframe track. */
function sampleCropTrack(
  track: KeyframeTrack<Crop> | undefined,
  frame: number,
  fallback: Crop,
): Crop {
  return sampleKeyframeTrack(track, frame, fallback, lerpCrop);
}

/** Absolute timeline frame -> clip-relative offset used by track storage. */
function keyframeOffset(clip: Clip, frame: number): number {
  return frame - clip.startFrame;
}

/** A track is active iff it holds at least one keyframe. */
function trackIsActive<V>(track: KeyframeTrack<V> | undefined): boolean {
  return !!track && track.keyframes.length > 0;
}

/**
 * 0..=1 envelope from the fade head/tail ramps. `min(in, out)`. Returns 0
 * outside `[0, durationFrames]` (closed interval, as upstream). 1:1 port of
 * `Clip::fade_multiplier`.
 */
export function fadeMultiplier(clip: Clip, frame: number): number {
  const rel = frame - clip.startFrame;
  if (rel < 0 || rel > clip.durationFrames) return 0.0;
  const inMul =
    clip.fadeInFrames > 0
      ? clip.fadeInInterpolation === "smooth"
        ? smoothstep(Math.min(rel / clip.fadeInFrames, 1.0))
        : Math.min(rel / clip.fadeInFrames, 1.0)
      : 1.0;
  const outRem = clip.durationFrames - rel;
  const outMul =
    clip.fadeOutFrames > 0
      ? clip.fadeOutInterpolation === "smooth"
        ? smoothstep(Math.min(outRem / clip.fadeOutFrames, 1.0))
        : Math.min(outRem / clip.fadeOutFrames, 1.0)
      : 1.0;
  return Math.min(inMul, outMul);
}

/** Authored opacity without the fade envelope. 1:1 port of `Clip::raw_opacity_at`. */
export function rawOpacityAt(clip: Clip, frame: number): number {
  return sampleScalarTrack(clip.opacityTrack, keyframeOffset(clip, frame), clip.opacity);
}

/**
 * Effective opacity at `frame`: authored value × fade envelope (visual clips
 * only; audio short-circuits before fade). 1:1 port of `Clip::opacity_at`.
 */
export function opacityAt(clip: Clip, frame: number): number {
  const base = rawOpacityAt(clip, frame);
  if (clip.mediaType === "audio" || (clip.fadeInFrames === 0 && clip.fadeOutFrames === 0)) {
    return base;
  }
  return base * fadeMultiplier(clip, frame);
}

/**
 * Effective linear volume: keyframe envelope (dB) first, fade ramp on top,
 * static volume as outer gain. 1:1 port of `Clip::volume_at`.
 */
export function volumeAt(clip: Clip, frame: number): number {
  const kfGain = trackIsActive(clip.volumeTrack)
    ? linearFromDb(sampleScalarTrack(clip.volumeTrack, keyframeOffset(clip, frame), 0.0))
    : 1.0;
  return clip.volume * kfGain * fadeMultiplier(clip, frame);
}

/**
 * Raw volume-track sample at `frame` as LINEAR amplitude — the authored keyframe
 * gain WITHOUT the static outer `volume` or the fade envelope. Mirrors upstream
 * `Clip.liveVolumeKfDb` (kept linear here to match the linear-valued volume
 * field). Returns `null` when the track has no keyframes. The Inspector seeds
 * its editable value from this when volume is animated, so editing upserts the
 * authored keyframe value rather than the composited output.
 */
export function liveVolumeKfLinearAt(clip: Clip, frame: number): number | null {
  if (!trackIsActive(clip.volumeTrack)) return null;
  return linearFromDb(sampleScalarTrack(clip.volumeTrack, keyframeOffset(clip, frame), 0.0));
}

/** Sampled rotation (degrees) at `frame`. 1:1 port of `Clip::rotation_at`. */
export function rotationAt(clip: Clip, frame: number): number {
  return sampleScalarTrack(clip.rotationTrack, keyframeOffset(clip, frame), clip.transform.rotation);
}

/** Sampled `(width, height)` at `frame`. 1:1 port of `Clip::size_at`. */
export function sizeAt(clip: Clip, frame: number): [number, number] {
  const fallback: AnimPair = { a: clip.transform.width, b: clip.transform.height };
  const s = samplePairTrack(clip.scaleTrack, keyframeOffset(clip, frame), fallback);
  return [s.a, s.b];
}

/** Sampled top-left (normalized canvas space) at `frame`. 1:1 port of `Clip::top_left_at`. */
export function topLeftAt(clip: Clip, frame: number): { x: number; y: number } {
  if (trackIsActive(clip.positionTrack)) {
    const p = samplePairTrack(clip.positionTrack, keyframeOffset(clip, frame), { a: 0, b: 0 });
    return { x: p.a, y: p.b };
  }
  const [w, h] = sizeAt(clip, frame);
  return {
    x: clip.transform.centerX - w / 2.0,
    y: clip.transform.centerY - h / 2.0,
  };
}

/** Sampled crop insets at `frame`. 1:1 port of `Clip::crop_at`. */
export function cropAt(clip: Clip, frame: number): Crop {
  return sampleCropTrack(clip.cropTrack, keyframeOffset(clip, frame), clip.crop);
}

/** Whether any transform-related track is active. 1:1 port of `Clip::has_transform_animation`. */
export function hasTransformAnimation(clip: Clip): boolean {
  return (
    trackIsActive(clip.positionTrack) ||
    trackIsActive(clip.scaleTrack) ||
    trackIsActive(clip.rotationTrack)
  );
}

/**
 * Frame offset of `clipId` within its link group, relative to the group's lead
 * (earliest-starting) clip. Returns `null` when the clip isn't linked, or when
 * it IS the lead (offset 0 → no badge needed). A positive result means this
 * clip starts LATER than the lead (e.g. audio trailing video by 3 frames → 3);
 * negative means it starts earlier. Used by the offset badge renderer (SPEC
 * §5.4 linked-offset indicator).
 */
export function linkOffsetForClip(timeline: Timeline, clipId: string): number | null {
  let target: Clip | null = null;
  let groupId: string | undefined;
  for (const track of timeline.tracks) {
    for (const c of track.clips) {
      if (c.id === clipId) {
        target = c;
        groupId = c.linkGroupId;
      }
    }
  }
  if (!target || !groupId) return null;
  // Collect every clip in the same link group, find the lead (min startFrame).
  let leadStart = Number.POSITIVE_INFINITY;
  for (const track of timeline.tracks) {
    for (const c of track.clips) {
      if (c.linkGroupId === groupId && c.startFrame < leadStart) {
        leadStart = c.startFrame;
      }
    }
  }
  if (!Number.isFinite(leadStart)) return null;
  const offset = target.startFrame - leadStart;
  if (offset === 0) return null; // lead clip → no badge
  return offset;
}
