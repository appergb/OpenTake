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

/** Whether `clip` is alone in its link group (SPEC §5.10 "单链组" gate for
 *  Swap Media). True when the clip has no `linkGroupId`, or when no OTHER clip
 *  in the timeline shares the same `linkGroupId`. A multi-clip link group
 *  (e.g. linked A/V pair) disables Swap Media to avoid desyncing the partners.
 *  1:1 with upstream `TimelineView.menu` Swap Media availability condition. */
export function isSingleLinkGroup(clip: Clip, timeline: Timeline): boolean {
  if (!clip.linkGroupId) return true;
  for (const track of timeline.tracks) {
    for (const c of track.clips) {
      if (c.id !== clip.id && c.linkGroupId === clip.linkGroupId) return false;
    }
  }
  return true;
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
